# 07 — Data Model

> Owner doc: [00-INDEX.md](00-INDEX.md) · Upstream: [04-SYSTEM-ARCHITECTURE](04-SYSTEM-ARCHITECTURE.md), [05-IPC-PROTOCOL](05-IPC-PROTOCOL.md), [06-RUST-CORE](06-RUST-CORE.md) · Downstream: [09-UI-COMPONENTS](09-UI-COMPONENTS.md), [11-VISUALIZATION-ENGINE](11-VISUALIZATION-ENGINE.md), [12-HOMEGROWN-FEATURES](12-HOMEGROWN-FEATURES.md), [16-PERFORMANCE-BUDGETS](16-PERFORMANCE-BUDGETS.md).
> Requirement IDs: `PRISM-DM-*`.

---

## 1. Design goals, in priority order

1. **8M files in ≤ 4 GB** resident set with full aggregation — structural arrays (SoA), interned strings, no per-node heap allocations.
2. **O(1) row access by `NodeId`** for virtualized UI windows; **O(page log page)** sorted reads.
3. **Immutable snapshots** once a scan completes (enables deterministic viz, diff, export, replay) while allowing in-place subtree rescans (WDS-SCN-09) via copy-on-write node ranges.
4. **One arena per `ScanId`**; lifetime managed by the engine; renderer never owns more than the visible window + fetched pages.

## 2. The arena (engine-side)

```rust
pub struct Arena {
    // --- identity ---------------------------------------------------------
    pub(crate) n: u32,                       // node count (NodeId = 0..n)
    parent:    Vec<NodeId>,                  // 4B  ; root.parent = SELF
    first_child: Vec<u32>,                   // 4B  ; index into child_index, u32::MAX = none
    // --- children as CSR (compressed sparse row) ---------------------------
    child_index: Vec<NodeId>,                // 4B × total_children (CSR arrays)
    child_offset: Vec<u32>,                  // n+1 offsets into child_index
    // --- strings (interned) ------------------------------------------------
    names:     Vec<u16>,                     // packed UTF-16; name i at names[name_off[i]..]
    name_off:  Vec<u32>,                     // 4B × n (+ terminal sentinel)
    // --- metrics (SoA) -----------------------------------------------------
    logical:   Vec<u64>,                     // 8B
    allocated: Vec<u64>,                     // 8B
    files:     Vec<u32>,                     // 4B  (subtree file count)
    folders:   Vec<u32>,                     // 4B
    mtime:     Vec<FileTimeI64>,             // 8B (0 = unknown; subtree max)
    // --- classification ----------------------------------------------------
    kind:      Vec<u8>,                      // EntryKind (File|Dir|Reparse|Mount|Link|FreeSpace|Unknown)
    category:  Vec<u16>,                     // CategoryId (extension table index or special)
    ext_id:    Vec<u32>,                     // extension interning table index (files)
    attr_flags: Vec<u32>,                    // FILE_ATTRIBUTE_* + internal flags (sparse, compressed, offline…)
    // --- hard links ----------------------------------------------------------
    link_to:   Vec<FileId>,                  // 8B, 0 = not a link (hardlink secondary instances)
    // --- errors -------------------------------------------------------------
    err_code:  Vec<u16>,                     // 0 = ok; else index into error table
}
```

**Memory math (per node):** 4+4+4 +4+4 (CSR amortized ≈ 8B) +4 (name offset; names shared via interning ≈ 6–8B avg) +8+8+4+4+8 +1+2+4+4 +8+2 ≈ **77–84 B/node + name bytes**. At 8M nodes ≈ 640 MB + names (~120 MB) + child CSR (~130 MB) ≈ **≈ 900 MB** — comfortably inside the 4 GB ceiling ([16 § Scan Budgets](16-PERFORMANCE-BUDGETS.md#2-scan-budgets)) with headroom for viz frames and SQLite caches.

`PRISM-DM-001` — NodeIds are **stable for the lifetime of the snapshot**; rescans allocate replacement ranges and publish a remap table (`scan:remap` event) so UI selections survive where identity survives.

### 2.1 Extension & category tables

- `ExtensionTable`: interned lowercase extensions (no dot; `.` for dotfiles; empty = none) → `{ ext_id, category_id, display_name, user_color: Option<u32> }`.
- `CategoryTable` (fixed, shipped): ~40 curated categories (video, audio, image, documents, archives, installers, developer/source, binaries, databases, virtual-machines, containers, fonts, system, mail, logs, caches, …) each with default color from the theme's data palette ([08 § Data Colors](08-UI-DESIGN-SYSTEM.md#4-data-colors-the-spectrum)) and match rule (extension list / name pattern). User overrides persist (WDS-EXT-03/04).
- Special pseudo-categories: `free-space`, `unknown`, `system-volume` (WinSxS/SVI subtree gets a system badge in the tree via path rules — display-only classification).

### 2.2 The error table

Path-attributed errors (denials, sharing violations) are stored once per directory (not per file) with a code → OS message map; children of errored dirs inherit `unknown` accounting. Surfaced via `scan:error-batch` and the Errors drawer (WDS-SCN-05).

## 3. Scan lifecycle & snapshots

```
ScanContext { id, target, options, strategy, state: Walking|Aggregating|Done|Failed|Cancelled,
              arena (built incrementally), started_at, token (CancellationToken) }
```

On completion the arena transitions to **frozen** (`Arc<Arena>`), then:

- Aggregation tables attach (`agg/` outputs: subtree stats, type stats, age histogram, leaderboards, quick-wins).
- A **snapshot record** may be persisted (premium, [12 § Snapshots](12-HOMEGROWN-FEATURES.md#6-snapshots--diff)).

Multiple scans may coexist (scan history in-session); the renderer binds views to a `scanId`. Arena drop is refcounted — switching to a new scan does not free the old one until all views release it (supports snapshot diff UX).

## 4. Persistence schema (SQLite, `app.db`)

WAL mode, `synchronous=NORMAL`, single writer (main process). Migrations versioned in `migrations/` (hand-written SQL, tested up+down). Core tables:

```sql
-- scan history (metadata only; never file names)
CREATE TABLE scans (
  id INTEGER PRIMARY KEY, started_at INTEGER NOT NULL, finished_at INTEGER,
  target_kind TEXT NOT NULL, target_path TEXT NOT NULL,   -- path of root only
  strategy TEXT NOT NULL, files INTEGER, folders INTEGER,
  logical_bytes INTEGER, allocated_bytes INTEGER, duration_ms INTEGER, version INTEGER
);

-- snapshots: capped shape capture for diffing (premium)
CREATE TABLE snapshots (
  id INTEGER PRIMARY KEY, scan_id INTEGER REFERENCES scans(id), created_at INTEGER NOT NULL,
  root_path TEXT NOT NULL, depth INTEGER NOT NULL,        -- capture depth (default 6)
  format_version INTEGER NOT NULL, payload BLOB NOT NULL  -- zstd-compressed snapshot format below
);

-- type color/description overrides
CREATE TABLE type_overrides (key TEXT PRIMARY KEY, color TEXT, display_name TEXT, updated_at INTEGER);

-- license entitlement cache (encrypted envelope; primary secret in Credential Manager)
CREATE TABLE license_cache (
  id INTEGER PRIMARY KEY CHECK (id=1), cipher BLOB NOT NULL, updated_at INTEGER NOT NULL
);

-- app settings (UI-synced; schema-versioned)
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL, updated_at INTEGER);
```

**Snapshot payload format** (`payload`, our own binary format `PSNP1`): min(itemCount, top-K per directory at depth ≤ capture_depth) — directory name-hash + 3 sizes + counts, delta-compressed against the previous snapshot when one exists for the same root. Typical 1M-file scan @ depth 6 ≈ 2–6 MB zstd. Diff = pairwise walk emitting `SnapshotDelta{ pathKey, kind: grew|shrank|added|removed, deltaBytes, deltaFiles }` with 10 MB significance floor (parity of concept with competitive intel, own implementation — [02 § 3.3](02-INTEL-DISKBUDDY.md#33-feature-inventory-from-symbols--strings--exhaustive)).

`PRISM-DM-010` — **no user file names or paths beyond the scan root string are persisted to disk** in any table except inside snapshot payloads (which are user-triggered, user-deletable, and this is disclosed in the UI). Privacy posture from [01 § 5.1](01-PRODUCT-VISION.md#5-product-principles-decision-tie-breakers).

## 5. Windowing & renderer-side model

The renderer stores, per active scan:

- **Visible window** of tree rows (page = 500) keyed by `(nodeId, sortSpec)` — TanStack Virtual renders only these; paging prefetches ± 1 page.
- **Viz frames** per (mode, root, viewport-signature) — LRU of 4 frames, binary `VizFrame` ([05 § 5](05-IPC-PROTOCOL.md#5-binary-payloads-vizframe)) decoded into typed arrays.
- **Selection & hover** as `NodeId` refs (never object trees).
- **Filter results** as paged id lists fetched on `filter:updated`.

`PRISM-DM-020` — the renderer must never hold a full materialized node list for scans > 100k nodes (CI perf test asserts renderer heap during a scripted 8M-node session; budget in [16 § UI Budgets](16-PERFORMANCE-BUDGETS.md#4-ui-budgets-renderer-r2-fix-l-active-scan)).

## 6. Invariants (property-tested, `PRISM-DM-030`)

For every completed snapshot:

1. `Σ child.allocated == parent.allocated` (and same for `logical`, `files`, `folders`) except at `unknown`/`free-space` nodes and hardlink secondary nodes (unique mode only counts first link — documented divergence WDS-SCN-07).
2. Every `NodeId` has exactly one parent; the structure is a forest (synthetic root for multi-root scans).
3. CSR arrays are consistent: `child_offset[i] ≤ child_offset[i+1]`, no duplicates within a parent's range (sorted by name for determinism).
4. `ext_id` for files resolves in the extension table; `category` resolves in the category table.
5. XXH3 canonical hash of the frozen arena is deterministic across repeated scans of the same fixture (ties into [06 § 12](06-RUST-CORE.md#12-testing-requirements-summary--full-strategy-in-17)).

## 7. Export formats (premium, `export:scan`)

- **CSV**: RFC 4180, UTF-8 with BOM (Excel-friendly), columns: `path, kind, logical_bytes, allocated_bytes, files, folders, modified_utc, category, extension` — streamed in 64 KiB chunks with progress events; path column only (no content).
- **JSON (NDJSON)**: one node per line, same fields plus `scan_id`, `snapshot_id?`.
- Exports are entitlement-gated ([13](13-LICENSING-SYSTEM.md#9-enforcement-depth-engine-boundary)) and the file dialog runs on main ([10 § Exports](10-SCREENS-AND-FLOWS.md#exports)).
