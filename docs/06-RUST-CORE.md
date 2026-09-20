# 06 — Rust Core Engine

> Owner doc: [00-INDEX.md](00-INDEX.md) · Upstream: [04-SYSTEM-ARCHITECTURE](04-SYSTEM-ARCHITECTURE.md), [05-IPC-PROTOCOL](05-IPC-PROTOCOL.md) · Downstream: [07-DATA-MODEL](07-DATA-MODEL.md), [11-VISUALIZATION-ENGINE](11-VISUALIZATION-ENGINE.md), [12-HOMEGROWN-FEATURES](12-HOMEGROWN-FEATURES.md), [16-PERFORMANCE-BUDGETS](16-PERFORMANCE-BUDGETS.md), [20-DEPENDENCY-MANIFEST](20-DEPENDENCY-MANIFEST.md).
> Requirement IDs: `PRISM-ENG-*`.
>
> **Engineering bar for this crate:** code review standard is "a Rust working-group member would approve". Rules: no `unwrap`/`expect`/`panic!` outside `#[cfg(test)]` (CI denies the lints); every public API returns `Result<T, EngineError>`; unsafe blocks require a `// SAFETY:` comment citing the invariant; every module has doc comments with examples for nontrivial APIs; no `unsafe` in scanner hot path except explicitly reviewed FFI call sites.

---

## 1. Crate map

| Crate | Role | Key internal modules |
|---|---|---|
| `prism-core` | engine orchestration, arena, aggregation, viz layout, cleanup, apps, duplicates, licensing verify, IPC (napi) | `scanner/`, `arena/`, `agg/`, `viz/`, `dupes/`, `apps/`, `cleanup/`, `licensing/`, `sysinfo/`, `ipc/` |
| `prism-ntfs` | raw NTFS: MFT record parsing, USN journal, run-list decoding, attribute walk | `mft.rs`, `usn.rs`, `runs.rs`, `attrs.rs`, `volume.rs` |
| `prism-types` | serde/napi DTOs — the single type source for codegen ([05 § 6](05-IPC-PROTOCOL.md#6-type-codegen)) | — |
| `prism-benches` | criterion benches (scan, arena, layout, ipc) | — |

Toolchain: Rust stable 1.98+ (pinned `rust-toolchain.toml`), edition 2024, `codegen-units=1`, `lto = "fat"`, `panic = "abort"` in release (NAPI boundary wraps with `catch_unwind` — see § 10), `-C target-cpu` left default for distribution; PGO is a post-GA optimization, not v1.

### Platform boundary

The engine is Windows-only in v1 by **decision, not by accident**: `prism-core` and `prism-ntfs` are free to use `windows-sys`/`windows` directly, and the scanner's fast paths are NTFS/Win32-specific (enumeration classes, reparse tags, MFT). To keep a future port path open without spending on it now, all Win32 calls live behind the `sys` modules (`scanner/sys.rs`, `sysinfo/sys.rs`, `cleanup/sys.rs`); the arena, aggregation, viz layout, duplicates pipeline, and licensing verify are pure Rust with **zero** Windows dependencies. Cross-platform work (macOS/Linux) remains out of scope for v1 per [01 § 8](01-PRODUCT-VISION.md#8-scope-boundaries-v1) — this section exists so the boundary is explicit and auditable, not as a commitment.

## 2. Scanner — standard strategy

### 2.1 Enumeration primitive

The hot loop enumerates directories with **`NtQueryDirectoryFile`** (via `windows-sys`) using `FileDirectoryInformation` in a single reusable `NTFS_FILE_RECORD...`—class buffer, falling back — **not** as a fallback, but as a **build-time selected path per capability**: on Win10 21H2+ we use `FileIdExtdDirectoryInformation` (gives 128-bit file IDs + reparse tags in one call), else `FileDirectoryInformation`. Both return name, size, times, attributes **in the enumeration itself — no per-file `CreateFile`/`GetFileInformationByHandle` round-trips**. This is the single biggest speed lever vs naive `std::fs` walks (which stat every entry in separate syscalls).

```rust
/// One directory batch returned by the enumerator.
pub struct DirBatch {
    pub dir: NodeId,                 // arena id of the enumerated directory
    pub count: usize,                // valid entries in the buffers below
    names:     Vec<u16>,             // packed UTF-16 names, nul-terminated
    name_off:  Vec<u32>,             // name i → names[name_off[i]..]
    metas:     Vec<Meta>,            // parallel array, 32 bytes/entry (below)
}
pub struct Meta {                    // 32B, cache-line friendly
    size: u64, alloc: u64,           // EndOfFile / AllocationSize
    mtime: i64,                      // FILETIME as i64 (100ns ticks)
    attrs: u32,                      // FILE_ATTRIBUTE_*
    reparse: u32,                    // reparse tag or 0
    file_id: FileId,                 // 64-bit (or 128-bit via Extd) volume-unique id
    kind: EntryKind,                 // File | Dir | Reparse{tag} | Mount
}
```

`PRISM-ENG-001` — the standard scanner must sustain ≥ 150k files/sec on the reference NVMe machine ([16 § Scan Budgets](16-PERFORMANCE-BUDGETS.md#2-scan-budgets)); regressions block merge (criterion gate).

### 2.2 Thread topology

- **Coordinator** (1 thread): owns the arena writer role, the BFS frontier (`crossbeam-deque` work-stealing), the cancellation token, and the SPSC event ring. Workers pull directory descriptors, enumerate, push `DirBatch`es back on a results channel.
- **Workers** (`N = available_parallelism().min(32)`): pure enumeration — no arena writes, no locks beyond the deques. This design (single writer, many readers of the FS) removes the classic contention of shared `RwLock` trees.
- **Aggregation** (rayon pool, post-walk + incremental): subtree rollups run incrementally as directories complete (so the UI sees live sizes, WDS-SCN-02) and once fully at walk end for the final pass.

Directory completion protocol: when all of a directory's subdirectories complete, the worker coordinator emits `DirComplete { node, agg }`; rollups cascade upward with `fetch_add` on the parent's pending counter — lock-free bottom-up aggregation.

### 2.3 Correctness rules (each = unit test module)

| Rule | ID | Behavior |
|---|---|---|
| Reparse points | `PRISM-ENG-010` | Junctions/symlinks are **not descended** by default (WDS-SCN-06). Represented as `EntryKind::Reparse{tag}` with badge; jump-to-target via `FSCTL_GET_REPARSE_POINT`. Loop protection is *unconditional*: per-scan visited set of `(volume_serial, file_id)` — even when `followReparse=true`, a revisit is recorded as a link node, never walked twice. |
| Mount points (VOLUME mount, not junction) | `PRISM-ENG-011` | Mounted volumes are recorded as `EntryKind::Mount` with a "scan this volume" affordance — never auto-descended (prevents surprise cross-volume scans). |
| Hard links | `PRISM-ENG-012` | First-seen `file_id` owns the bytes; later links become `LinkNode { target }`. `unique` sizes count once; `logical`/`allocated` count per link (user switchable, WDS-SCN-07). Requires `file_id` — guaranteed by both enumeration classes above. |
| Sparse & compressed files | `PRISM-ENG-013` | `logical = EndOfFile`, `allocated = AllocationSize` (already sparse-aware). Inspector surfaces both + "sparse" badge when `attrs & FILE_ATTRIBUTE_SPARSE_FILE`. |
| Packages | `PRISM-ENG-014` | Directory bundles matched against the package-extension table (`.zip`-style containers on Windows: `.msix`, `.appx`, `.vhdx`, `.iso`, … full table in `scanner/packages.rs`) are optionally collapsed to single nodes (`treatPackagesAsNodes`). |
| Access denied | `PRISM-ENG-015` | Denied → `unknown` accounting (WDS-SCN-05); error batch carries path + OS error for the Errors drawer; enumeration of the *parent* continues. |
| Long paths | `PRISM-ENG-016` | All opens use `\\?\`-extended paths internally (`Vec<u16>` throughout — **no `PathBuf`/`String` in the hot loop**); conversion to display paths happens only at the DTO edge. |
| Case sensitivity | `PRISM-ENG-017` | Comparisons for duplicate-name grouping use Unicode simple case folding via `widestring` + ICU-lite fold map (no full ICU dependency); NTFS is case-preserving — we never rename. |
| Hidden/system | `PRISM-ENG-018` | Always included (parity); display filters are renderer-side facts, never scan exclusions. |
| Free space node | `PRISM-ENG-019` | After walk: `GetDiskFreeSpaceExW` free bytes → synthetic `free-space` node under volume root (WDS-SCN-04). |

### 2.4 Exclusion matcher

Glob subset with `*`, `?`, `**`; compiled once per scan to a flat matcher (no regex dependency in engine); matching on UTF-16 names via our own fold-aware comparator. Used by `ScanOptions.excludePatterns` and by preset scans ([12 § Presets](12-HOMEGROWN-FEATURES.md#2-cleanup-presets-rule-engine)).

## 3. Turbo scan — raw NTFS (opt-in, elevated)

`PRISM-ENG-030` — Turbo strategy reads the **MFT directly**:

1. Consent → the elevation helper ([§ 7](#7-elevation-strategy)) opens the volume with `FILE_READ_DATA | FILE_READ_ATTRIBUTES` (`\\.\X:`).
2. `FSCTL_ENUM_USN_DATA` (V0/V1) streams USN records — each yields parent FRN, file FRN, name, attributes — building the full namespace in one sequential pass without opening a single directory handle.
3. Sizes require the MFT itself: `DeviceIoControl(FSCTL_GET_NTFS_FILE_RECORD)` per record is too slow at 8M files; instead we read **MFT runs** (from the boot sector + $MFT record 0 run list) and parse `STANDARD_INFORMATION` + `$FILE_NAME` + `DATA` attribute runs directly in `prism-ntfs` (streaming, 64 MiB windows). `allocated` = sum of data runs rounded to cluster; `logical` = `$DATA` valid data length. This is the Everything/WizTree-class approach.
4. Hard-link sets come free (`$FILE_NAME` records per file id) — feeds WDS-SCN-07 accounting.
5. Parent linkage (FRN → parent FRN) builds the tree; names from `$FILE_NAME`; orphaned FRNs (unresolvable parents) go to a synthetic `orphans` node surfaced in the Errors drawer.

Constraints (documented in-UI before consent): NTFS/ReFS only (ReFS falls back — **explicit user choice**, not silent: Turbo button is disabled with a tooltip on non-NTFS, satisfying [19 A2](19-AGENT-PROTOCOL.md#a2--no-fallbacks-the-rule-the-owner-cares-about-most)); requires elevation; excludes non-resident data beyond run-list parse (network filesystems are not applicable — they never expose `\\.\X:`).

`PRISM-ENG-031` — turbo target: ≥ 1M files/sec sustained on the reference NVMe machine ([16](16-PERFORMANCE-BUDGETS.md#2-scan-budgets)).

## 4. Aggregation & statistics

Post-walk rayon pass produces, in one arena sweep:

- Subtree totals (files, folders, logical, allocated, unique) — also maintained incrementally during walk.
- **Category/extension stats**: extension string table (interned), per-extension counts/sizes → `types:list`; category mapping from the curated table ([08 § Data Colors](08-UI-DESIGN-SYSTEM.md#4-data-colors-the-spectrum) + user overrides).
- **Age histogram**: byte-weighted buckets (7d / 30d / 90d / 1y / 2y / older) for the age viz + stale rules.
- **Top-N leaderboards** per directory (top 10 children by each metric) — powers "Largest Inside" without re-sorting on demand.
- **Quick Wins candidates**: preset hits + stale candidates + big caches, ranked by `reclaimScore = bytes × confidence` ([12 § Quick Wins](12-HOMEGROWN-FEATURES.md#1-quick-wins)).

All aggregations are pure functions over the immutable snapshot → deterministic and testable with snapshot fixtures.

## 5. Volume enumeration & system info

- `GetLogicalDrives` + `GetDriveTypeW` + `GetVolumeInformationW` (label, fs, serial) + `GetDiskFreeSpaceExW`; `IVdsService` not used in v1 (COM surface not justified); USB/network identification via drive type + `SetupDi` device path where cheap.
- Monitor sampling ([12 § Monitor](12-HOMEGROWN-FEATURES.md#7-monitor)): `NtQuerySystemInformation(SystemProcessInformation)` snapshots (PID, name, CPU time delta, working set, IO counters via `SYSTEM_PROCESS_INFORMATION` + `GetProcessIoCounters` where available), 1 Hz, top-20 by CPU and by IO; system CPU via `NtQuerySystemInformation(SystemPerformanceInformation)`. No WMI (slow, flaky) — direct NT APIs only.

## 6. Icon extraction

`SHIL`-based: `SHGetFileInfoW` with `SHGFI_SYSICONINDEX | SHGFI_SMALLICON`, then `IImageList::GetIcon` → BGRA pixels cached to `%LOCALAPPDATA%` icon cache (content-keyed by icon index + image-list revision). Extraction is lazy (visible rows only), on a low-priority thread pool, with a hard cap (2048 icons/scan) and LRU eviction. Renderer receives PNG bytes via a `sys:icon` command returning cached `Buffer` (`PRISM-ENG-040`).

## 7. Elevation strategy

- **Default posture:** the app runs as the invoker; no elevation for standard scans (matches [01] principles and the reference tool's behavior).
- **Turbo consent flow:** UI explains what/why → `ShellExecuteW "runas"` on the small `prism-elev.exe` helper (part of install; signed; version-bound to the host app; accepts a one-time nonce + named-pipe handle) → helper opens the volume handle and passes it back over the named pipe (handle duplication) → helper exits. **The helper never scans** — it only brokers the handle; all parsing happens in the unelevated engine. Narrow surface, auditable, and the elevation lasts exactly one consent.
- ReFS/non-NTFS volumes disable Turbo with an explanation (no silent substitution, [ADR-06](04-SYSTEM-ARCHITECTURE.md#2-architectural-decisions-adr-summary--full-text-lives-in-docsadr)).

## 8. Licensing verification inside the engine

`PRISM-ENG-050` — Engine-gate for premium commands (see [13 § Enforcement Depth](13-LICENSING-SYSTEM.md#9-enforcement-depth-engine-boundary)):

```rust
pub fn verify_entitlement(tok: &EntitlementToken, feature: PremiumFeature,
                          device: &DeviceIdentity, now: UnixMs)
    -> Result<EntitlementGrants, EntitlementError>
```

- Ed25519 (ed25519-dalek) signature over the canonical CBOR claims; **public key embedded at build** (env-injected, CI-checked — private key never exists on build machines for client artifacts).
- Checks: signature, `exp` vs `now` (±90 s clock-skew tolerance), `sku` feature matrix, `device_id` equality, `nonce` replay window (token carries `iat`; engine rejects `iat` older than 7 days to bound replay).
- Result cached in-memory (grants map) with re-verification on token refresh events; **no premium code path executes before the gate returns Ok** — enforced by making premium handlers generic over `Verified<Ent>` passed by the dispatcher (typestate; you cannot construct the handler input without verifying).

## 9. Cleanup & uninstaller engine surface

- Deletions: `IFileOperation` COM (with `FOF_ALLOWUNDO`, `FOFX_ADDUNDORECORD`, `FOF_NOERRORUI`, per-item error collection) — recycle bin default, permanent via `FOF_NO_CONFIRM_UI | FOFX_RECYCLEONDELETE` clear. `PRISM-ENG-060`: every execute returns per-item outcomes; partial failures never abort the ledger silently — the UI shows the failure ledger (fail-loud, [19 A2](19-AGENT-PROTOCOL.md#a2--no-fallbacks-the-rule-the-owner-cares-about-most)).
- Uninstaller: uninstall-string execution for the chosen entry (registry `HKLM/HKCU ...\Uninstall\*` via `REG_QUERY`), then leftover scan: known roots (`%LOCALAPPDATA%`, `%APPDATA%`, `%PROGRAMDATA%`, `Documents`, registry keys) matched by publisher-normalized app tokens ([12 § Uninstaller](12-HOMEGROWN-FEATURES.md#5-application-uninstaller--leftovers)).
- Recycle bin ops: `SHEmptyRecycleBinW` with `SHERB_NOCONFIRMATION | SHERB_NOSOUND` behind our own confirmation ledger.

## 10. Error strategy (engine-wide)

```rust
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("io: {path:?}: {source}")] Io { path: PathDisp, source: std::io::Error },
    #[error("os: {code:#x} {msg}")]    Os  { code: i32, msg: &'static str },
    #[error("scan {id}: {stage}: {msg}")] Scan { id: ScanId, stage: &'static str, msg: String },
    #[error("entitlement: {0}")]       Entitlement(#[from] EntitlementError),
    #[error("ntfs: {0}")]              Ntfs(#[from] prism_ntfs::Error),
    #[error("invalid: {field}")]       Invalid { field: &'static str },
    #[error("busy: {op}")]             Busy { op: &'static str },
    #[error("cancelled")]              Cancelled,
    #[error("internal: {0}")]          Internal(String),   // catch_unwind boundary only
}
```

- `anyhow` is **not** used in `prism-core` (allowed only in the tiny `xtask` binary).
- Panic containment: every `#[napi]` entry point wraps its body in `std::panic::catch_unwind` (with `AssertUnwindSafe` on owned args only), converting to `EngineError::Internal` + tracing event. Release builds use `panic=abort` **except** the napi boundary relies on catch before abort — the Cargo profile uses `panic = "unwind"` for the cdylib and abort for test bins via profile split. (Exact profile matrix in `Cargo.toml`; documented in `crates/prism-core/README.md`.)
- Logging: `tracing` with level filtering; **path strings are redacted at the subscriber** (`path=<redacted>` unless `PRISM_DEBUG_PATHS=1` during a support session with user consent) — privacy posture ([01 § 5.1](01-PRODUCT-VISION.md#5-product-principles-decision-tie-breakers)).

## 11. Unsafe policy & FFI checklist

- All Win32 calls via `windows-sys` (zero-cost, `#[link]` extern declarations); no `windows` (COM-heavy) crate in the hot path — COM used only in `cleanup`/`shell` modules via `windows` crate with `implement` where needed.
- Every `unsafe` block: `// SAFETY:` comment + named invariant; clippy `undocumented_unsafe_blocks` denied.
- Buffer sizes for `NtQueryDirectoryFile` are derived from `SYSTEM_INFO` page size and empirically-tuned constants documented in `scanner/nt_query.rs` (benchmarked in `prism-benches`).
- All handles RAII (`struct Handle(HANDLE)` with `Drop = CloseHandle`); leaks caught by a debug-mode handle audit thread in tests.

## 12. Testing requirements (summary — full strategy in [17](17-QUALITY-ENGINEERING.md))

| Class | Requirement |
|---|---|
| Unit | every correctness rule § 2.3 has table-driven tests incl. fixture VHDs (CI mounts a scripted NTFS fixture image with junctions, hardlinks, sparse files, deep chains, long paths, denial ACLs) |
| Property | proptest invariants: `sum(children) == parent` for all three size modes; arena id stability within scan; event stream replay == final snapshot |
| Fuzz | `prism-ntfs` MFT/USN parsers fuzzed with `cargo-fuzz` (MFT record corpus seeded from fixture images); fuzz targets run in CI nightly-class job |
| Bench | criterion gates for § 2.1/§ 3 budgets ([16](16-PERFORMANCE-BUDGETS.md)) |
| Determinism | same fixture scanned twice → identical arena hash (XXH3 over canonical serialization) — catches nondeterministic walk orders leaking into results |

## 13. Crate dependency policy (full pins in [20-DEPENDENCY-MANIFEST](20-DEPENDENCY-MANIFEST.md))

Selection rules: prefer `windows-sys` over `windows` in hot paths; std + `crossbeam` for concurrency; `rayon` for parallel aggregation only; `tokio` **not used** (engine is thread-based; async adds nothing to syscall-bound work — deliberate senior-level decision); `ed25519-dalek` + `serde` + `serde_json` + `cbor` for licensing; `rusqlite` (bundled) for persistence; `tracing` + `tracing-subscriber` for diagnostics; `thiserror` for errors. No crate enters `Cargo.toml` without a justification line + license audit (SPDX field required).
