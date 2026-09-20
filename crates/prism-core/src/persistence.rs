//! Persistence layer (P1-006, docs/07 § 4): the `app.db` SQLite database.
//!
//! Owns: versioned migrations, `scans` history, `settings` KV, and the
//! `snapshots` store (zstd-compressed JSON payloads — depth-capped shape
//! captures, docs/12 § 6). The diff engine is pure (snapshot payload in,
//! [`SnapshotDelta`] rows out) so it is property-testable without SQLite.
//!
//! Concurrency: one writer, opened by the main process at boot; WAL mode.
//! All engine-side access is serialized through the [`Db`] handle (parking_lot
//! Mutex inside — SQLite handles are not Sync-safe across threads by default).

use std::path::Path;

use rusqlite::Connection;
use zstd::decode_all as zstd_decode;
use zstd::encode_all as zstd_encode;

use prism_types::commands::{SnapshotDelta, SnapshotDeltaKind, SnapshotInfo};

use crate::arena::{Arena, NodeId, kind};
use crate::error::EngineError;

/// Map a rusqlite failure to the engine error surface. DB faults are engine
/// faults (wire kind `engine`); the io::Error payload preserves the source.
fn db_err(msg: String) -> EngineError {
    EngineError::Internal(format!("app.db: {msg}"))
}

/// Current schema version (migrations are append-only below).
const SCHEMA_VERSION: i64 = 3;

/// One captured directory-shape row (what a snapshot stores per node).
///
/// Path keys are privacy-safe hashes (FNV-1a 64 over the display path) — the
/// PSNP1 share format never persists absolute paths (docs/12 § 10); display
/// paths are stored locally only (the db never leaves the machine), but we
/// hash anyway so the *exported* share file and the db row agree.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapNode {
    /// Privacy-safe path key (hex FNV-1a 64).
    path_key: String,
    /// Depth-capped display path (local only).
    path: String,
    /// Aggregated size bytes.
    bytes: u64,
    /// File count in subtree.
    files: u64,
}

/// The stored snapshot payload (zstd-compressed JSON of this).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotPayload {
    /// Root path the scan ran on.
    root_path: String,
    /// Capture depth.
    depth: u8,
    /// Captured at (unix ms).
    created_at: i64,
    /// Nodes (depth-capped, directories only).
    nodes: Vec<SnapNode>,
    /// Totals.
    files: u64,
    bytes: u64,
}

/// A handle to `app.db`.
pub struct Db {
    conn: parking_lot::Mutex<Connection>,
}

impl Db {
    /// Open (creating parents) and migrate to [`SCHEMA_VERSION`].
    pub fn open(path: &Path) -> Result<Self, EngineError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| db_err(format!("create db dir: {e}")))?;
        }
        let conn = Connection::open(path).map_err(|e| db_err(format!("open app.db: {e}")))?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|e| db_err(format!("wal: {e}")))?;
        conn.pragma_update(None, "synchronous", "NORMAL")
            .map_err(|e| db_err(format!("synchronous: {e}")))?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(|e| db_err(format!("fk: {e}")))?;
        let db = Self {
            conn: parking_lot::Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    /// Append-only migration chain. Each step runs inside a transaction; the
    /// `user_version` pragma records the applied step count.
    fn migrate(&self) -> Result<(), EngineError> {
        let conn = self.conn.lock();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(|e| db_err(format!("read user_version: {e}")))?;
        if version > SCHEMA_VERSION {
            return Err(db_err(format!(
                "app.db schema v{version} is newer than this build (v{SCHEMA_VERSION}) — update the app"
            )));
        }
        let migrations: &[&str] = &[
            // v1 — scans history + settings KV (P1-006)
            "CREATE TABLE IF NOT EXISTS scans (
                 id INTEGER PRIMARY KEY,
                 started_at INTEGER NOT NULL,
                 finished_at INTEGER NOT NULL,
                 target TEXT NOT NULL,
                 strategy TEXT NOT NULL,
                 files INTEGER NOT NULL,
                 folders INTEGER NOT NULL,
                 bytes INTEGER NOT NULL,
                 duration_ms INTEGER NOT NULL,
                 error_count INTEGER NOT NULL DEFAULT 0
             );
             CREATE INDEX IF NOT EXISTS idx_scans_started ON scans(started_at DESC);
             CREATE TABLE IF NOT EXISTS settings (
                 key TEXT PRIMARY KEY,
                 value TEXT NOT NULL,
                 updated_at INTEGER NOT NULL
             );",
            // v2 — snapshots (P5 track, docs/12 § 6)
            "CREATE TABLE IF NOT EXISTS snapshots (
                 id INTEGER PRIMARY KEY,
                 created_at INTEGER NOT NULL,
                 root_path TEXT NOT NULL,
                 depth INTEGER NOT NULL,
                 files INTEGER NOT NULL,
                 bytes INTEGER NOT NULL,
                 payload BLOB NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_snapshots_root ON snapshots(root_path, created_at DESC);",
            // v3 — type color overrides + exclusion rules (settings rows were
            // JSON blobs already; this table makes color edits queryable)
            "CREATE TABLE IF NOT EXISTS type_colors (
                 ext TEXT PRIMARY KEY,
                 color TEXT NOT NULL,
                 updated_at INTEGER NOT NULL
             );",
        ];
        for (i, sql) in migrations.iter().enumerate() {
            let step = (i + 1) as i64;
            if version < step {
                conn.execute_batch("BEGIN")
                    .map_err(|e| db_err(format!("begin: {e}")))?;
                if let Err(e) = conn.execute_batch(sql) {
                    let _ = conn.execute_batch("ROLLBACK");
                    return Err(db_err(format!("migration v{step}: {e}")));
                }
                conn.pragma_update(None, "user_version", step)
                    .map_err(|e| db_err(format!("set v{step}: {e}")))?;
                conn.execute_batch("COMMIT")
                    .map_err(|e| db_err(format!("commit v{step}: {e}")))?;
                tracing::info!(version = step, "app.db migrated");
            }
        }
        Ok(())
    }

    // -- settings ------------------------------------------------------------

    /// Set one settings KV row (upsert). Values are JSON strings.
    pub fn set_setting(&self, key: &str, value_json: &str) -> Result<(), EngineError> {
        let conn = self.conn.lock();
        let now = now_ms();
        conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = ?3",
            rusqlite::params![key, value_json, now],
        )
        .map_err(|e| db_err(format!("set_setting: {e}")))?;
        Ok(())
    }

    /// Get one settings row (JSON string).
    pub fn get_setting(&self, key: &str) -> Option<String> {
        let conn = self.conn.lock();
        conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
            r.get::<_, String>(0)
        })
        .ok()
    }

    /// All settings rows (key → JSON value) — first-run restore.
    pub fn all_settings(&self) -> Vec<(String, String)> {
        let conn = self.conn.lock();
        let mut stmt = match conn.prepare("SELECT key, value FROM settings ORDER BY key") {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map(|it| it.filter_map(Result::ok).collect::<Vec<_>>())
            .unwrap_or_default()
    }

    // -- scans history ----------------------------------------------------------

    /// Record one completed scan in the history table.
    pub fn record_scan(&self, rec: &ScanRecord) -> Result<(), EngineError> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO scans
               (started_at, finished_at, target, strategy, files, folders, bytes, duration_ms, error_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                rec.started_at,
                rec.finished_at,
                rec.target,
                rec.strategy,
                rec.files as i64,
                rec.folders as i64,
                rec.bytes as i64,
                rec.duration_ms,
                rec.error_count,
            ],
        )
        .map_err(|e| db_err(format!("record_scan: {e}")))?;
        Ok(())
    }

    /// Recent scans (newest first), capped.
    pub fn recent_scans(&self, limit: u32) -> Vec<ScanRecord> {
        let conn = self.conn.lock();
        let mut stmt = match conn.prepare(
            "SELECT started_at, finished_at, target, strategy, files, folders, bytes, duration_ms, error_count
             FROM scans ORDER BY started_at DESC LIMIT ?1",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        stmt.query_map([limit], |r| {
            Ok(ScanRecord {
                started_at: r.get(0)?,
                finished_at: r.get(1)?,
                target: r.get(2)?,
                strategy: r.get(3)?,
                files: r.get::<_, i64>(4)? as u64,
                folders: r.get::<_, i64>(5)? as u64,
                bytes: r.get::<_, i64>(6)? as u64,
                duration_ms: r.get(7)?,
                error_count: r.get(8)?,
            })
        })
        .map(|it| it.filter_map(Result::ok).collect::<Vec<_>>())
        .unwrap_or_default()
    }

    // -- snapshots ---------------------------------------------------------------

    /// Capture a depth-capped shape snapshot of a completed scan's arena.
    pub fn save_snapshot(
        &self,
        arena: &Arena,
        root_path: &str,
        depth: u8,
    ) -> Result<SnapshotInfo, EngineError> {
        let payload = capture_payload(arena, root_path, depth);
        let info = SnapshotInfo {
            id: 0,
            root_path: payload.root_path.clone(),
            created_at: payload.created_at,
            depth: payload.depth,
            files: payload.files,
            bytes: payload.bytes,
        };
        let json =
            serde_json::to_vec(&payload).map_err(|e| db_err(format!("snapshot serialize: {e}")))?;
        let compressed =
            zstd_encode(json.as_slice(), 3).map_err(|e| db_err(format!("snapshot zstd: {e}")))?;
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO snapshots (created_at, root_path, depth, files, bytes, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                { payload.created_at },
                payload.root_path,
                payload.depth as i64,
                payload.files as i64,
                payload.bytes as i64,
                compressed,
            ],
        )
        .map_err(|e| db_err(format!("save_snapshot: {e}")))?;
        let id = conn.last_insert_rowid() as u32;
        Ok(SnapshotInfo { id, ..info })
    }

    /// List snapshots (newest first), optionally filtered by root.
    pub fn list_snapshots(&self, root: &str) -> Vec<SnapshotInfo> {
        let conn = self.conn.lock();
        let mut stmt = match if root.is_empty() {
            conn.prepare(
                "SELECT id, root_path, created_at, depth, files, bytes FROM snapshots
                 ORDER BY created_at DESC LIMIT 200",
            )
        } else {
            conn.prepare(
                "SELECT id, root_path, created_at, depth, files, bytes FROM snapshots
                 WHERE root_path = ?1 ORDER BY created_at DESC LIMIT 200",
            )
        } {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let map = |r: &rusqlite::Row<'_>| -> rusqlite::Result<SnapshotInfo> {
            Ok(SnapshotInfo {
                id: r.get::<_, i64>(0)? as u32,
                root_path: r.get(1)?,
                created_at: r.get(2)?,
                depth: r.get::<_, i64>(3)? as u8,
                files: r.get::<_, i64>(4)? as u64,
                bytes: r.get::<_, i64>(5)? as u64,
            })
        };

        if root.is_empty() {
            stmt.query_map([], map)
                .map(|it| it.filter_map(Result::ok).collect::<Vec<_>>())
                .unwrap_or_default()
        } else {
            stmt.query_map([root], map)
                .map(|it| it.filter_map(Result::ok).collect::<Vec<_>>())
                .unwrap_or_default()
        }
    }

    /// Load two snapshots and diff them (significance floor in bytes).
    pub fn diff_snapshots(
        &self,
        before: u32,
        after: u32,
        floor_bytes: u64,
    ) -> Result<(Vec<SnapshotDelta>, i64), EngineError> {
        let a = self.load_payload(before)?;
        let b = self.load_payload(after)?;
        Ok(diff_payloads(&a, &b, floor_bytes))
    }

    fn load_payload(&self, id: u32) -> Result<SnapshotPayload, EngineError> {
        let conn = self.conn.lock();
        let blob: Vec<u8> = conn
            .query_row(
                "SELECT payload FROM snapshots WHERE id = ?1",
                [id as i64],
                |r| r.get(0),
            )
            .map_err(|_| db_err(format!("snapshot {id} not found")))?;
        drop(conn);
        let json = zstd_decode(blob.as_slice())
            .map_err(|e| db_err(format!("snapshot decompress: {e}")))?;
        serde_json::from_slice(&json).map_err(|e| db_err(format!("snapshot parse: {e}")))
    }

    /// Export a snapshot as a PSNP1 share payload (privacy-safe: path keys
    /// only, display paths stripped — docs/12 § 10).
    pub fn export_share(&self, id: u32) -> Result<Vec<u8>, EngineError> {
        let p = self.load_payload(id)?;
        let share = serde_json::json!({
            "magic": "PSNP1",
            "rootKey": path_key(&p.root_path),
            "createdAt": p.created_at,
            "depth": p.depth,
            "nodes": p.nodes.iter().map(|n| serde_json::json!({
                "k": n.path_key,
                "b": n.bytes,
                "f": n.files,
            })).collect::<Vec<_>>(),
        });
        serde_json::to_vec(&share).map_err(|e| db_err(format!("share: {e}")))
    }

    // -- type colors ------------------------------------------------------------

    /// Persist a user type-color override.
    pub fn set_type_color(&self, ext: &str, color: &str) -> Result<(), EngineError> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO type_colors (ext, color, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(ext) DO UPDATE SET color = ?2, updated_at = ?3",
            rusqlite::params![ext, color, now_ms()],
        )
        .map_err(|e| db_err(format!("set_type_color: {e}")))?;
        Ok(())
    }

    /// All type-color overrides.
    pub fn type_colors(&self) -> Vec<(String, String)> {
        let conn = self.conn.lock();
        let mut stmt = match conn.prepare("SELECT ext, color FROM type_colors") {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map(|it| it.filter_map(Result::ok).collect::<Vec<_>>())
            .unwrap_or_default()
    }
}

/// One scans-history row.
#[derive(Debug, Clone)]
pub struct ScanRecord {
    /// Started at (unix ms).
    pub started_at: i64,
    /// Finished at (unix ms).
    pub finished_at: i64,
    /// Root path.
    pub target: String,
    /// `standard` | `turbo`.
    pub strategy: String,
    /// Total files.
    pub files: u64,
    /// Total folders.
    pub folders: u64,
    /// Total bytes.
    pub bytes: u64,
    /// Wall duration ms.
    pub duration_ms: i64,
    /// Walk error count.
    pub error_count: i64,
}

// ---------------------------------------------------------------------------
// capture + diff (pure — property-tested)
// ---------------------------------------------------------------------------

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// FNV-1a 64 path key (privacy-safe, stable across runs/machines for the
/// same path string — good enough for diff joins; NOT a security hash).
pub fn path_key(path: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in path.encode_utf16() {
        h ^= b as u64;
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    format!("{h:016x}")
}

/// Walk the arena to `depth`, capturing directory nodes only.
fn capture_payload(arena: &Arena, root_path: &str, depth: u8) -> SnapshotPayload {
    let mut nodes = Vec::new();
    let roots = arena.roots();
    for &root in &roots {
        capture_walk(arena, root, &arena.name_str(root), 0, depth, &mut nodes);
    }
    let files: u64 = roots.iter().map(|&r| arena.files(r) as u64).sum();
    let bytes: u64 = roots.iter().map(|&r| arena.allocated(r as usize)).sum();
    SnapshotPayload {
        root_path: root_path.to_string(),
        depth,
        created_at: now_ms(),
        nodes,
        files,
        bytes,
    }
}

fn capture_walk(
    arena: &Arena,
    node: NodeId,
    path: &str,
    depth: u8,
    max_depth: u8,
    out: &mut Vec<SnapNode>,
) {
    if arena.kind(node) != kind::DIR && arena.kind(node) != kind::ROOT {
        return;
    }
    out.push(SnapNode {
        path_key: path_key(path),
        path: path.to_string(),
        bytes: arena.allocated(node as usize),
        files: arena.files(node) as u64,
    });
    if depth >= max_depth {
        return;
    }
    for &child in arena.children(node) {
        if arena.kind(child) == kind::DIR {
            let child_path = format!("{path}\\{}", arena.name_str(child));
            capture_walk(arena, child, &child_path, depth + 1, max_depth, out);
        }
    }
}

/// Diff two payloads: keyed by path_key; net + rows with |delta| ≥ floor.
/// Row order: by |delta| descending (biggest movers first — the UI table).
pub fn diff_payloads(
    before: &SnapshotPayload,
    after: &SnapshotPayload,
    floor_bytes: u64,
) -> (Vec<SnapshotDelta>, i64) {
    use std::collections::HashMap;
    let b: HashMap<&str, &SnapNode> = before
        .nodes
        .iter()
        .map(|n| (n.path_key.as_str(), n))
        .collect();
    let a: HashMap<&str, &SnapNode> = after
        .nodes
        .iter()
        .map(|n| (n.path_key.as_str(), n))
        .collect();

    let mut rows: Vec<SnapshotDelta> = Vec::new();
    let mut net: i64 = 0;

    for (k, an) in &a {
        match b.get(*k) {
            Some(bn) => {
                let d = an.bytes as i64 - bn.bytes as i64;
                net += d;
                if d.abs() >= floor_bytes as i64 {
                    let kind = if d > 0 {
                        SnapshotDeltaKind::Grew
                    } else {
                        SnapshotDeltaKind::Shrank
                    };
                    rows.push(SnapshotDelta {
                        path_key: (*k).to_string(),
                        path: an.path.clone(),
                        kind,
                        delta_bytes: d,
                        delta_files: an.files as i64 - bn.files as i64,
                    });
                }
            }
            None => {
                net += an.bytes as i64;
                if an.bytes >= floor_bytes {
                    rows.push(SnapshotDelta {
                        path_key: (*k).to_string(),
                        path: an.path.clone(),
                        kind: SnapshotDeltaKind::Added,
                        delta_bytes: an.bytes as i64,
                        delta_files: an.files as i64,
                    });
                }
            }
        }
    }
    for (k, bn) in &b {
        if !a.contains_key(*k) {
            net -= bn.bytes as i64;
            if bn.bytes >= floor_bytes {
                rows.push(SnapshotDelta {
                    path_key: (*k).to_string(),
                    path: bn.path.clone(),
                    kind: SnapshotDeltaKind::Removed,
                    delta_bytes: -(bn.bytes as i64),
                    delta_files: -(bn.files as i64),
                });
            }
        }
    }
    rows.sort_by_key(|r| std::cmp::Reverse(r.delta_bytes.abs()));
    (rows, net)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::NodeInput;

    fn build_arena() -> Arena {
        let mut a = Arena::with_capacity(64);
        let root = a.push(NodeInput {
            parent: 0,
            name_utf16: &"C:".encode_utf16().collect::<Vec<_>>(),
            logical: 0,
            allocated: 0,
            files: 3,
            folders: 2,
            mtime: 0,
            kind: kind::ROOT,
            category: 0,
            ext_id: 0,
            attr_flags: 0,
            link_to: 0,
            err_code: 0,
        });
        let d1 = a.push(NodeInput {
            parent: root,
            name_utf16: &"proj".encode_utf16().collect::<Vec<_>>(),
            logical: 0,
            allocated: 0,
            files: 2,
            folders: 0,
            mtime: 0,
            kind: kind::DIR,
            category: 0,
            ext_id: 0,
            attr_flags: 0,
            link_to: 0,
            err_code: 0,
        });
        let d2 = a.push(NodeInput {
            parent: root,
            name_utf16: &"media".encode_utf16().collect::<Vec<_>>(),
            logical: 0,
            allocated: 0,
            files: 1,
            folders: 0,
            mtime: 0,
            kind: kind::DIR,
            category: 0,
            ext_id: 0,
            attr_flags: 0,
            link_to: 0,
            err_code: 0,
        });
        a.attach(root, d1);
        a.attach(root, d2);
        a.finalize_children();
        a
    }

    #[test]
    fn migrations_run_and_reopen_is_idempotent() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        let path = dir.path().join("nested").join("app.db");
        {
            let db = Db::open(&path).unwrap_or_else(|e| panic!("{e}"));
            db.set_setting("theme", "\"nocturne\"")
                .unwrap_or_else(|e| panic!("{e}"));
            db.record_scan(&ScanRecord {
                started_at: 1,
                finished_at: 2,
                target: "C:\\".into(),
                strategy: "standard".into(),
                files: 10,
                folders: 2,
                bytes: 1234,
                duration_ms: 7,
                error_count: 0,
            })
            .unwrap_or_else(|e| panic!("{e}"));
        }
        // Reopen: version matches, no re-migration, data intact.
        let db = Db::open(&path).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(db.get_setting("theme"), Some("\"nocturne\"".into()));
        let recs = db.recent_scans(10);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].files, 10);
    }

    /// `build_arena` with "proj" grown by (bytes, extra files).
    fn build_arena_grown(proj_bytes: u64, proj_files: u32) -> Arena {
        let mut a = Arena::with_capacity(64);
        let root = a.push(NodeInput {
            parent: 0,
            name_utf16: &"C:".encode_utf16().collect::<Vec<_>>(),
            logical: 0,
            allocated: 0,
            files: 3,
            folders: 2,
            mtime: 0,
            kind: kind::ROOT,
            category: 0,
            ext_id: 0,
            attr_flags: 0,
            link_to: 0,
            err_code: 0,
        });
        let d1 = a.push(NodeInput {
            parent: root,
            name_utf16: &"proj".encode_utf16().collect::<Vec<_>>(),
            logical: proj_bytes,
            allocated: proj_bytes,
            files: proj_files,
            folders: 0,
            mtime: 0,
            kind: kind::DIR,
            category: 0,
            ext_id: 0,
            attr_flags: 0,
            link_to: 0,
            err_code: 0,
        });
        let d2 = a.push(NodeInput {
            parent: root,
            name_utf16: &"media".encode_utf16().collect::<Vec<_>>(),
            logical: 0,
            allocated: 0,
            files: 1,
            folders: 0,
            mtime: 0,
            kind: kind::DIR,
            category: 0,
            ext_id: 0,
            attr_flags: 0,
            link_to: 0,
            err_code: 0,
        });
        a.attach(root, d1);
        a.attach(root, d2);
        a.finalize_children();
        a
    }

    #[test]
    fn snapshot_roundtrip_and_diff() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        let db = Db::open(&dir.path().join("app.db")).unwrap_or_else(|e| panic!("{e}"));

        let a1 = build_arena();
        let s1 = db
            .save_snapshot(&a1, "C:\\", 4)
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(s1.id > 0);
        assert_eq!(s1.root_path, "C:\\");

        // Same arena, one dir grown: a fresh arena with a bigger "proj".
        let a2 = build_arena_grown(50_000_000, 3);
        let s2 = db
            .save_snapshot(&a2, "C:\\", 4)
            .unwrap_or_else(|e| panic!("{e}"));

        let (rows, net) = db
            .diff_snapshots(s1.id, s2.id, 1024)
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(net >= 50_000_000, "net {net} reflects the grown dir");
        let grew = rows
            .iter()
            .find(|r| matches!(r.kind, SnapshotDeltaKind::Grew));
        assert!(grew.is_some(), "the grown dir is a Grew row: {rows:?}");
        let grew = grew.unwrap_or_else(|| panic!("grew row missing: {rows:?}"));
        assert!(grew.delta_bytes >= 50_000_000);

        // Floor filters noise: 10 MB default hides nothing here at 50 MB.
        let (rows_floor, _) = db
            .diff_snapshots(s1.id, s2.id, 10 * 1024 * 1024)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(rows_floor.len(), rows.len());
    }

    #[test]
    fn diff_keyed_added_removed() {
        let mk = |paths: &[(&str, u64)]| SnapshotPayload {
            root_path: "C:\\".into(),
            depth: 4,
            created_at: 0,
            nodes: paths
                .iter()
                .map(|&(p, b)| SnapNode {
                    path_key: path_key(p),
                    path: p.into(),
                    bytes: b,
                    files: 1,
                })
                .collect(),
            files: paths.len() as u64,
            bytes: 0,
        };
        let before = mk(&[("C:\\a", 100), ("C:\\gone", 5 * 1024 * 1024)]);
        let after = mk(&[("C:\\a", 300), ("C:\\new", 2 * 1024 * 1024)]);
        let (rows, net) = diff_payloads(&before, &after, 1024 * 1024);
        // gone (−5 MB, Removed), new (+2 MB, Added), a (+200 < 1 MB floor — hidden)
        assert_eq!(rows.len(), 2);
        assert!(
            rows.iter()
                .any(|r| matches!(r.kind, SnapshotDeltaKind::Removed) && r.path.ends_with("gone"))
        );
        assert!(
            rows.iter()
                .any(|r| matches!(r.kind, SnapshotDeltaKind::Added) && r.path.ends_with("new"))
        );
        assert_eq!(net, 300 - 100 + 2 * 1024 * 1024 - 5 * 1024 * 1024);
    }

    #[test]
    fn share_export_strips_paths() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        let db = Db::open(&dir.path().join("app.db")).unwrap_or_else(|e| panic!("{e}"));
        let a = build_arena();
        let s = db
            .save_snapshot(&a, "C:\\Users\\me", 4)
            .unwrap_or_else(|e| panic!("{e}"));
        let share = db.export_share(s.id).unwrap_or_else(|e| panic!("{e}"));
        let text = String::from_utf8(share).unwrap_or_else(|e| panic!("{e}"));
        assert!(text.contains("PSNP1"));
        assert!(
            !text.contains("C:\\Users"),
            "share payload has no absolute paths"
        );
    }
}
