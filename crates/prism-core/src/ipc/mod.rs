//! # IPC surface (T2 — napi commands + the event pump, docs/05)
//!
//! Commands cross as `serde_json::Value` (AMM-003): the TS side validates
//! requests with generated zod schemas at T1; the engine re-validates at the
//! boundary (PRISM-IPC-050 both sides). `viz:layout` returns a binary
//! `Buffer` (PRISM-IPC-030). Events flow through one TSFN as JSON batches
//! (≤ 512 / ≤ 16 ms, PRISM-IPC-020).
//!
//! Panic containment (A5): every `#[napi]` entry catches unwinds and converts
//! to `EngineError::Internal` — never a panic across FFI.

use std::sync::Arc;

use napi::bindgen_prelude::{Buffer, Function, Status};
use napi::threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi::{Env, Result};
use napi_derive::napi;
use parking_lot::Mutex as PlMutex;
use serde_json::Value;

use prism_types::commands::ScanStartQuery;
use prism_types::events::EngineEvent;
use prism_types::scan::{ChildrenQuery, NodeRow, NodeRowsPage, ScanSummary};
use prism_types::viz::VizLayoutQuery;

pub mod extend;

use crate::arena::{Arena, NodeId, kind};
use crate::error::EngineError;
use crate::scanner::coordinator::{CompletedScan, ScanManager};
use crate::{agg, licensing, sysinfo, viz};

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// EngineError → napi error carrying the wire kind (PRISM-IPC-003).
pub(crate) fn ne(e: EngineError) -> napi::Error {
    napi::Error::new(
        Status::GenericFailure,
        format!(
            "{{\"kind\":\"{}\",\"msg\":{}}}",
            e.wire_kind(),
            serde_json::to_string(&e.to_string()).unwrap_or_else(|_| "\"?\"".into())
        ),
    )
}

fn je(e: serde_json::Error) -> napi::Error {
    napi::Error::new(
        Status::GenericFailure,
        format!(
            "{{\"kind\":\"engine\",\"msg\":{}}}",
            serde_json::to_string(&e.to_string()).unwrap_or_else(|_| "\"?\"".into())
        ),
    )
}

pub(crate) fn parse_payload<T: serde::de::DeserializeOwned>(v: Value) -> Result<T> {
    serde_json::from_value(v).map_err(|e| {
        napi::Error::new(
            Status::InvalidArg,
            format!(
                "{{\"kind\":\"invalid-args\",\"detail\":{}}}",
                serde_json::to_string(&e.to_string()).unwrap_or_else(|_| "\"?\"".into())
            ),
        )
    })
}

pub(crate) fn to_json<T: serde::Serialize>(v: T) -> Result<Value> {
    serde_json::to_value(v).map_err(je)
}

fn panic_to_napi(e: Box<dyn std::any::Any + Send>) -> napi::Error {
    let msg = if let Some(s) = e.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic payload".to_string()
    };
    tracing::error!(panic = %msg, "engine panic contained at the NAPI boundary");
    napi::Error::new(
        Status::GenericFailure,
        format!("{{\"kind\":\"engine\",\"code\":\"internal\",\"msg\":\"panic: {msg}\"}}"),
    )
}

/// A5: contain unwinds at the FFI boundary.
pub(crate) fn contain<T>(f: impl FnOnce() -> Result<T> + std::panic::UnwindSafe) -> Result<T> {
    std::panic::catch_unwind(f).unwrap_or_else(|e| Err(panic_to_napi(e)))
}

// ---------------------------------------------------------------------------
// event sink: engine → TSFN (single pump, PRISM-IPC-020)
// ---------------------------------------------------------------------------

/// The engine-wide event sink. Producers `emit()`; the drain thread (spawned
/// on JS attach) batches ≤ 512 events / ≤ 16 ms into JSON strings.
pub struct EngineEventSink {
    tx: crossbeam_channel::Sender<EngineEvent>,
}

impl EngineEventSink {
    fn new() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded::<EngineEvent>();
        PENDING_RX.get_or_init(|| rx);
        Self { tx }
    }

    /// Emit one event (append-only fact).
    pub fn emit(&self, ev: EngineEvent) -> std::result::Result<(), EngineError> {
        self.tx
            .send(ev)
            .map_err(|_| EngineError::Internal("event channel closed".into()))
    }
}

impl Default for EngineEventSink {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineEventSink {
    /// Test-only sink: events route to the global channel like production,
    /// so tests observe them through the same pump (no second code path).
    #[cfg(test)]
    pub fn for_tests() -> Self {
        Self::new()
    }
}

static PENDING_RX: std::sync::OnceLock<crossbeam_channel::Receiver<EngineEvent>> =
    std::sync::OnceLock::new();

/// Attach the JS event callback (called once from main at boot). Spawns the
/// drain thread that batches events into JSON batch strings.
#[napi]
pub fn engine_attach_event_sink(callback: Function<'_, String, ()>) -> Result<()> {
    let tsfn: ThreadsafeFunction<String, (), String, napi::Status, false> = callback
        .build_threadsafe_function::<String>()
        .callee_handled::<false>()
        .build()?;
    let rx = PENDING_RX
        .get()
        .ok_or_else(|| napi::Error::new(Status::GenericFailure, "event sink not initialized"))?;
    std::thread::Builder::new()
        .name("prism-event-drain".into())
        .spawn(move || {
            let mut batch: Vec<EngineEvent> = Vec::with_capacity(512);
            loop {
                match rx.recv_timeout(std::time::Duration::from_millis(16)) {
                    Ok(ev) => {
                        batch.push(ev);
                        while batch.len() < 512 {
                            match rx.try_recv() {
                                Ok(ev2) => batch.push(ev2),
                                Err(_) => break,
                            }
                        }
                        flush(&tsfn, &mut batch);
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                        if !batch.is_empty() {
                            flush(&tsfn, &mut batch);
                        }
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                        flush(&tsfn, &mut batch);
                        break;
                    }
                }
            }
        })
        .map_err(|e| napi::Error::new(Status::GenericFailure, format!("spawn drain: {e}")))?;
    Ok(())
}

fn flush(
    tsfn: &ThreadsafeFunction<String, (), String, napi::Status, false>,
    batch: &mut Vec<EngineEvent>,
) {
    if batch.is_empty() {
        return;
    }
    match serde_json::to_string(batch.as_slice()) {
        Ok(json) => {
            tsfn.call(json, ThreadsafeFunctionCallMode::NonBlocking);
        }
        Err(e) => {
            // serialization of engine events cannot fail structurally; if it
            // ever does, drop loudly (logged) — never silently swallow.
            tracing::error!(error = %e, "event batch serialization failed — DROPPING BATCH");
        }
    }
    batch.clear();
}

// ---------------------------------------------------------------------------
// engine state
// ---------------------------------------------------------------------------

/// Global engine state.
pub struct EngineState {
    /// Scan leases + coordinator fan-out.
    pub scans: ScanManager,
    /// Last verified entitlement grants (main injects verified tokens).
    pub grants: PlMutex<Option<prism_types::licensing::EntitlementGrants>>,
    /// Device identity for engine-side binding checks.
    pub instance_id: PlMutex<String>,
    /// App database (opened by main via `engine_open_db`).
    pub db: PlMutex<Option<Arc<crate::persistence::Db>>>,
    /// The cleanup staging queue (ledger source of truth).
    pub queue: PlMutex<crate::cleanup::StagingQueue>,
    /// Duplicate pipeline runs.
    pub dupes: PlMutex<extend::DupesManager>,
    /// Live monitor session.
    pub monitor: PlMutex<Option<crate::monitor::MonitorHandle>>,
    /// User type-color overrides (db-hydrated).
    pub colors: PlMutex<std::collections::HashMap<String, String>>,
}

impl EngineState {
    /// Cloned db handle (must have been opened).
    pub fn db_arc(&self) -> Result<Arc<crate::persistence::Db>> {
        self.db.lock().clone().ok_or_else(|| {
            napi::Error::new(
                Status::GenericFailure,
                "app.db not opened (call engine_open_db first)",
            )
        })
    }
}

static STATE: std::sync::OnceLock<Arc<EngineState>> = std::sync::OnceLock::new();
pub(crate) static SINK: std::sync::OnceLock<Arc<EngineEventSink>> = std::sync::OnceLock::new();

pub(crate) fn state() -> Result<Arc<EngineState>> {
    STATE
        .get()
        .cloned()
        .ok_or_else(|| napi::Error::new(Status::GenericFailure, "engine not initialized"))
}

#[napi]
/// One-time engine init (idempotent).
pub fn engine_init() -> Result<()> {
    let sink = SINK
        .get_or_init(|| Arc::new(EngineEventSink::new()))
        .clone();
    STATE.get_or_init(|| {
        Arc::new(EngineState {
            scans: ScanManager::new(sink),
            grants: PlMutex::new(None),
            instance_id: PlMutex::new(String::new()),
            db: PlMutex::new(None),
            queue: PlMutex::new(crate::cleanup::StagingQueue::new()),
            dupes: PlMutex::new(extend::DupesManager::default()),
            monitor: PlMutex::new(None),
            colors: PlMutex::new(Default::default()),
        })
    });
    Ok(())
}

// ---------------------------------------------------------------------------
// commands (docs/05 § 3)
// ---------------------------------------------------------------------------

#[napi]
/// `sys:hello`
pub fn sys_hello() -> Result<Value> {
    contain(|| to_json(sysinfo::hello()))
}

#[napi]
/// `sys:volumes`
pub fn sys_volumes() -> Result<Value> {
    contain(|| to_json(sysinfo::volumes()))
}

#[napi]
/// `scan:start` — returns `{ scanId }`; progress via events.
pub fn scan_start(payload: Value) -> Result<Value> {
    contain(|| {
        let q: ScanStartQuery = parse_payload(payload)?;
        let st = state()?;
        // Turbo is entitlement-gated at the ENGINE boundary (PRISM-LIC-040),
        // then dispatched by the coordinator (ADR-06: first-class — a
        // non-NTFS or non-elevated request fails honestly from the strategy
        // itself, never silently degrades to a standard walk).
        if q.strategy == prism_types::scan::ScanStrategy::Turbo {
            let gated = st
                .grants
                .lock()
                .as_ref()
                .map(|g| {
                    g.features
                        .contains(&prism_types::licensing::PremiumFeature::Turbo)
                })
                .unwrap_or(false);
            if !gated {
                return Err(napi::Error::new(
                    Status::GenericFailure,
                    "{\"kind\":\"not-licensed\",\"feature\":\"turbo\"}",
                ));
            }
        }
        let st2 = Arc::clone(&st);
        let on_complete: Arc<dyn Fn(u32, Option<Arc<CompletedScan>>) + Send + Sync> =
            Arc::new(move |id, completed| {
                if let Some(c) = completed.as_ref() {
                    extend::record_scan_history(&st2, &c.summary, c.summary.duration_ms);
                }
                st2.scans.complete(id, completed);
            });
        let id = st
            .scans
            .start(q.target, q.strategy, q.options, on_complete)
            .map_err(ne)?;
        Ok(serde_json::json!({ "scanId": id }))
    })
}

#[napi]
/// `scan:pause`
pub fn scan_pause(scan_id: u32) -> Result<()> {
    contain(|| state()?.scans.pause(scan_id).map_err(ne))
}

#[napi]
/// `scan:resume`
pub fn scan_resume(scan_id: u32) -> Result<()> {
    contain(|| state()?.scans.resume(scan_id).map_err(ne))
}

#[napi]
/// `scan:cancel` (≤ 50 ms cooperative ack)
pub fn scan_cancel(scan_id: u32) -> Result<()> {
    contain(|| state()?.scans.cancel(scan_id).map_err(ne))
}

#[napi]
/// `scan:summary`
pub fn scan_summary(scan_id: u32) -> Result<Value> {
    contain(|| {
        let s: ScanSummary = state()?.scans.summary(scan_id).map_err(ne)?;
        to_json(s)
    })
}

#[napi]
/// `scan:phase` (probe)
pub fn scan_phase(scan_id: u32) -> Result<String> {
    contain(|| {
        Ok(match state()?.scans.phase(scan_id) {
            Some(prism_types::scan::ScanPhase::Walking) => "walking".into(),
            Some(prism_types::scan::ScanPhase::Aggregating) => "aggregating".into(),
            Some(prism_types::scan::ScanPhase::IndexingExt) => "indexing-ext".into(),
            Some(prism_types::scan::ScanPhase::Done) => "done".into(),
            Some(prism_types::scan::ScanPhase::Failed) => "failed".into(),
            Some(prism_types::scan::ScanPhase::Cancelled) => "cancelled".into(),
            Some(prism_types::scan::ScanPhase::Paused) => "paused".into(),
            None => "unknown".into(),
        })
    })
}

#[napi]
/// `tree:children` — paged, engine-side sort.
pub fn tree_children(payload: Value) -> Result<Value> {
    contain(|| {
        let q: ChildrenQuery = parse_payload(payload)?;
        if q.limit == 0 || q.limit > 1000 {
            return Err(napi::Error::new(
                Status::InvalidArg,
                "{\"kind\":\"invalid-args\",\"field\":\"limit (1..=1000)\"}",
            ));
        }
        let st = state()?;
        let scan = st.scans.completed(q.scan_id).map_err(ne)?;
        let arena = &scan.arena;
        if q.node_id as usize >= arena.len() {
            return Err(napi::Error::new(
                Status::InvalidArg,
                "{\"kind\":\"invalid-args\",\"field\":\"nodeId\"}",
            ));
        }
        let mut children = arena.children(q.node_id).to_vec();
        match q.sort.key {
            prism_types::ids::SortKey::Logical => {
                children.sort_by_key(|&c| std::cmp::Reverse(arena.logical(c)))
            }
            prism_types::ids::SortKey::Name => children.sort_by_key(|&a| arena.name_str(a)),
            _ => {}
        }
        if q.sort.dir == prism_types::ids::SortDir::Asc {
            children.reverse();
        }
        let total = children.len() as u32;
        let start = (q.offset as usize).min(children.len());
        let end = (start + q.limit as usize).min(children.len());
        let parent_alloc = arena.allocated(q.node_id as usize).max(1);
        let items: Vec<NodeRow> = children[start..end]
            .iter()
            .map(|&c| node_row(arena, c, parent_alloc))
            .collect();
        to_json(NodeRowsPage { total, items })
    })
}

fn node_row(a: &Arena, c: NodeId, parent_alloc: u64) -> NodeRow {
    use prism_types::ids::{EntryKind, NodeBadge};
    let ek = match a.kind(c) {
        kind::FILE => EntryKind::File,
        kind::DIR => EntryKind::Dir,
        kind::REPARSE => EntryKind::Reparse,
        kind::MOUNT => EntryKind::Mount,
        kind::LINK => EntryKind::Link,
        kind::FREE_SPACE => EntryKind::FreeSpace,
        kind::UNKNOWN => EntryKind::Unknown,
        _ => EntryKind::Dir,
    };
    let mut badges = Vec::new();
    let flags = a.attr_flags(c);
    if flags & crate::arena::flags::REPARSE != 0 {
        badges.push(NodeBadge::Junction);
    }
    if flags & crate::arena::flags::SPARSE != 0 {
        badges.push(NodeBadge::Sparse);
    }
    if flags & crate::arena::flags::COMPRESSED != 0 {
        badges.push(NodeBadge::Compressed);
    }
    if flags & crate::arena::flags::INTERNAL_LINK2 != 0 || a.link_to(c) != 0 {
        badges.push(NodeBadge::Hardlinked);
    }
    if flags & crate::arena::flags::OFFLINE != 0 {
        badges.push(NodeBadge::Offline);
    }
    if a.err_code(c) != 0 {
        badges.push(NodeBadge::Denied);
    }
    NodeRow {
        id: c,
        name: a.name_str(c),
        kind: ek,
        logical: a.logical(c),
        allocated: a.allocated(c as usize),
        files: a.files(c),
        folders: a.folders(c),
        category: a.category(c),
        badges,
        mtime: if a.mtime(c) > 0 {
            Some(crate::scanner::filetime_ticks_to_unix_ms(a.mtime(c)))
        } else {
            None
        },
        parent_share: (a.allocated(c as usize) as f32) / (parent_alloc as f32),
    }
}

#[napi]
/// `node:detail`
pub fn node_detail(scan_id: u32, node_id: u32) -> Result<Value> {
    contain(|| {
        let st = state()?;
        let scan = st.scans.completed(scan_id).map_err(ne)?;
        let a = &scan.arena;
        if node_id as usize >= a.len() {
            return Err(napi::Error::new(
                Status::InvalidArg,
                "{\"kind\":\"invalid-args\",\"field\":\"nodeId\"}",
            ));
        }
        let d = prism_types::scan::NodeDetail {
            id: node_id,
            path: node_path(a, node_id, &scan.summary.root),
            name: a.name_str(node_id),
            kind: match a.kind(node_id) {
                kind::FILE => prism_types::ids::EntryKind::File,
                kind::DIR | kind::ROOT => prism_types::ids::EntryKind::Dir,
                kind::REPARSE => prism_types::ids::EntryKind::Reparse,
                kind::MOUNT => prism_types::ids::EntryKind::Mount,
                kind::LINK => prism_types::ids::EntryKind::Link,
                kind::FREE_SPACE => prism_types::ids::EntryKind::FreeSpace,
                kind::UNKNOWN => prism_types::ids::EntryKind::Unknown,
                _ => prism_types::ids::EntryKind::Dir,
            },
            category: a.category(node_id),
            category_name: agg::category_name(a.category(node_id)).to_string(),
            extension: scan.ext_table.display(a.ext_id(node_id)).to_string(),
            logical: a.logical(node_id),
            allocated: a.allocated(node_id as usize),
            unique: if a.link_to(node_id) == 0 {
                a.allocated(node_id as usize)
            } else {
                0
            },
            badges: Vec::new(),
            created: None,
            modified: if a.mtime(node_id) > 0 {
                Some(crate::scanner::filetime_ticks_to_unix_ms(a.mtime(node_id)))
            } else {
                None
            },
            accessed: None,
            attributes: a.attr_flags(node_id),
            link_count: 1,
            link_paths: Vec::new(),
            file_id: a.link_to(node_id),
            parent: a.parent(node_id),
            depth: a.depth(node_id),
            files: a.files(node_id),
            folders: a.folders(node_id),
        };
        to_json(d)
    })
}

/// Rebuild a node's absolute path from the parent chain (zero stored paths
/// for leaves — docs/06 § 2.1 zero-copy discipline).
pub fn node_path(arena: &Arena, node: NodeId, root_path: &str) -> String {
    let mut parts = Vec::new();
    let mut cur = node;
    let mut guard = 0usize;
    while guard <= arena.len() {
        if arena.parent(cur) == cur {
            break;
        }
        parts.push(arena.name_str(cur));
        cur = arena.parent(cur);
        guard += 1;
    }
    parts.reverse();
    let sep = if cfg!(windows) { "\\" } else { "/" };
    let joined = parts.join(sep);
    if joined.is_empty() {
        return root_path.to_string();
    }
    format!(
        "{}{}{}",
        root_path.trim_end_matches(['\\', '/']),
        sep,
        joined
    )
}

#[napi]
/// `tree:expand-stats`
pub fn tree_expand_stats(scan_id: u32, node_id: u32) -> Result<Value> {
    contain(|| {
        let st = state()?;
        let scan = st.scans.completed(scan_id).map_err(ne)?;
        let a = &scan.arena;
        if node_id as usize >= a.len() {
            return Err(napi::Error::new(
                Status::InvalidArg,
                "{\"kind\":\"invalid-args\",\"field\":\"nodeId\"}",
            ));
        }
        to_json(prism_types::scan::ExpandStats {
            files: u64::from(a.files(node_id)),
            folders: u64::from(a.folders(node_id)),
            logical: a.logical(node_id),
            allocated: a.allocated(node_id as usize),
            unique: a.allocated(node_id as usize),
        })
    })
}

#[napi]
/// `viz:layout` — binary VizFrame (Buffer, PRISM-IPC-030).
pub fn viz_layout(payload: Value) -> Result<Buffer> {
    contain(|| {
        let q: VizLayoutQuery = parse_payload(payload)?;
        let st = state()?;
        let scan = st.scans.completed(q.scan_id).map_err(ne)?;
        let frame = viz::layout(
            &scan.arena,
            q.scan_id,
            q.root,
            q.mode,
            (q.viewport.w, q.viewport.h, q.viewport.dpr),
            &q.options,
        )
        .map_err(|e| {
            napi::Error::new(
                Status::GenericFailure,
                format!(
                    "{{\"kind\":\"engine\",\"msg\":{}}}",
                    serde_json::to_string(&e).unwrap_or_else(|_| "\"?\"".into())
                ),
            )
        })?;
        Ok(Buffer::from(frame))
    })
}

#[napi]
/// `types:list`
pub fn types_list(scan_id: u32, sort: String, dir: String) -> Result<Value> {
    contain(|| {
        let st = state()?;
        let scan = st.scans.completed(scan_id).map_err(ne)?;
        let a = &scan.arena;
        let mut rows: Vec<prism_types::types_list::TypeRow> = scan
            .ext_table
            .keys()
            .iter()
            .enumerate()
            .filter_map(|(i, key)| {
                scan.aggregates.ext_stats.get(i).filter(|s| s.0 > 0).map(
                    |&(files, logical, allocated)| prism_types::types_list::TypeRow {
                        key: key.clone(),
                        ext_id: i as u32,
                        category: agg::category_for_ext(key),
                        files: u64::from(files),
                        logical,
                        allocated,
                        share: (allocated as f32) / (a.allocated(0).max(1) as f32),
                        user_color: None,
                    },
                )
            })
            .collect();
        match sort.as_str() {
            "name" | "category" => rows.sort_by(|x, y| x.key.cmp(&y.key)),
            "files" => rows.sort_by_key(|r| std::cmp::Reverse(r.files)),
            _ => rows.sort_by_key(|r| std::cmp::Reverse(r.allocated)),
        }
        if dir == "asc" {
            rows.reverse();
        }
        let total_share: f32 = rows.iter().map(|r| r.share).sum();
        to_json(prism_types::types_list::TypesPage {
            items: rows,
            total_share,
        })
    })
}

#[napi]
/// `filter:apply` — name filter; v1 returns the matched count + first page
/// inline (paged streaming arrives with the filter engine in P4 — documented).
pub fn filter_apply(scan_id: u32, name: String, kind_filter: String) -> Result<Value> {
    contain(|| {
        let st = state()?;
        let scan = st.scans.completed(scan_id).map_err(ne)?;
        let a = &scan.arena;
        let lowered = name.to_lowercase();
        let want_files = kind_filter != "dirs";
        let want_dirs = kind_filter != "files";
        let mut matched = 0u64;
        for i in 0..a.len() {
            let n = i as NodeId;
            let is_file = a.kind(n) == kind::FILE;
            if is_file && !want_files {
                continue;
            }
            if !is_file && !want_dirs {
                continue;
            }
            if a.name_str(n).to_lowercase().contains(&lowered) {
                matched += 1;
                if matched >= 10_000 {
                    break;
                }
            }
        }
        to_json(prism_types::filter::FilterResult {
            filter_id: 1,
            matched,
            pattern_kind: prism_types::filter::FilterPatternKind::Literal,
            pattern_display: name,
        })
    })
}

#[napi]
/// `lic:verify-token` (main → engine, PRISM-LIC-040 engine boundary).
pub fn lic_verify_token(token_b64: String, feature: String) -> Result<Value> {
    contain(|| {
        let feat = match feature.as_str() {
            "turbo" => prism_types::licensing::PremiumFeature::Turbo,
            "dupes" => prism_types::licensing::PremiumFeature::Dupes,
            "cleanup" => prism_types::licensing::PremiumFeature::Cleanup,
            "apps" => prism_types::licensing::PremiumFeature::Apps,
            "snapshots" => prism_types::licensing::PremiumFeature::Snapshots,
            "monitor" => prism_types::licensing::PremiumFeature::Monitor,
            "scheduler" => prism_types::licensing::PremiumFeature::Scheduler,
            "export" => prism_types::licensing::PremiumFeature::Export,
            other => {
                return Err(napi::Error::new(
                    Status::InvalidArg,
                    format!(
                        "{{\"kind\":\"invalid-args\",\"field\":\"feature\",\"detail\":\"{other}\"}}"
                    ),
                ));
            }
        };
        let st = state()?;
        let instance = st.instance_id.lock().clone();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        match licensing::verify_entitlement(&token_b64, feat, &instance, now) {
            Ok(grants) => {
                *st.grants.lock() = Some(grants.clone());
                to_json(grants)
            }
            Err(e) => {
                let reason = match e {
                    prism_types::licensing::EntitlementError::BadSignature => "bad-signature",
                    prism_types::licensing::EntitlementError::Expired => "expired",
                    prism_types::licensing::EntitlementError::StaleIat => "stale-iat",
                    prism_types::licensing::EntitlementError::DeviceMismatch => "device-mismatch",
                    prism_types::licensing::EntitlementError::FeatureNotGranted => {
                        "feature-not-granted"
                    }
                    prism_types::licensing::EntitlementError::Malformed => "malformed",
                    prism_types::licensing::EntitlementError::Revoked => "revoked",
                };
                Err(napi::Error::new(
                    Status::GenericFailure,
                    format!("{{\"kind\":\"entitlement\",\"reason\":\"{reason}\"}}"),
                ))
            }
        }
    })
}

#[napi]
/// Set the device identity (main calls at boot; used by token binding).
pub fn engine_set_instance_id(instance_id: String) -> Result<()> {
    contain(|| {
        let st = state()?;
        *st.instance_id.lock() = instance_id;
        Ok(())
    })
}

#[napi]
/// Engine version string.
pub fn engine_version() -> Result<String> {
    Ok(env!("CARGO_PKG_VERSION").to_string())
}

#[napi]
/// Engine event sink (drain-thread) health probe: pending queue length.
pub fn engine_event_queue_depth() -> Result<u32> {
    Ok(PENDING_RX.get().map(|rx| rx.len() as u32).unwrap_or(0))
}

/// Keep `Env` in the type surface (reserved for future typed callbacks).
#[allow(unused)]
fn _env_surface(_e: Env) {}

/// Keep `EngineError` import referenced (used via `ne` in outer scope paths).
#[allow(unused)]
fn _err_surface(_e: EngineError) {}
