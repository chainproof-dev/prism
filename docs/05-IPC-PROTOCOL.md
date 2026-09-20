# 05 — IPC Protocol (Renderer ⇄ Main ⇄ Rust Engine)

> Owner doc: [00-INDEX.md](00-INDEX.md) · Upstream: [04-SYSTEM-ARCHITECTURE](04-SYSTEM-ARCHITECTURE.md) · Downstream: [06-RUST-CORE](06-RUST-CORE.md), [07-DATA-MODEL](07-DATA-MODEL.md), [09-UI-COMPONENTS](09-UI-COMPONENTS.md), [13-LICENSING-SYSTEM](13-LICENSING-SYSTEM.md).
> Requirement IDs: `PRISM-IPC-*`.
>
> This document is the **single source of truth** for every message that crosses a process or FFI boundary. The Rust side defines the types (serde); a build-time codegen step emits matching TypeScript into `packages/shared` — hand-written duplicates are forbidden (`PRISM-IPC-001`).

---

## 1. Surfaces

There are exactly three transports, each with one canonical use:

| # | Boundary | Transport | Use |
|---|---|---|---|
| T1 | Renderer ⇄ Main | Electron `ipcMain.handle` / `ipcRenderer.invoke` (request-response) and `webContents.send` / `ipcRenderer.on` (events) via preload `contextBridge` | All commands + all event fan-out |
| T2 | Main ⇄ Rust | napi-rs v3 typed functions (commands) + one `ThreadsafeFunction` (events) | Engine calls |
| T3 | Rust internal | SPSC ring buffer → drain thread | Worker → coordinator event batching |

The renderer never calls T2 directly. The main process never forwards raw untyped payloads — every T1 message is parsed and re-typed at the boundary (`PRISM-IPC-002`).

## 2. Envelope formats

### 2.1 Command (T1 request)

```ts
// packages/shared/src/protocol.ts  (GENERATED from Rust — do not hand-edit)
interface IpcRequest<C extends keyof CommandsMap> {
  cmd: C;
  seq: number;              // monotonic per renderer session, for correlation in logs
  payload: CommandsMap[C]['req'];
}
type IpcResponse<C extends keyof CommandsMap> =
  | { ok: true;  seq: number; data: CommandsMap[C]['res'] }
  | { ok: false; seq: number; error: IpcError };
```

```ts
// Discriminated error — mirrors Rust thiserror variants exactly.
type IpcError =
  | { kind: 'invalid-args';   detail: string }             // schema rejection at main
  | { kind: 'engine';         code: EngineErrorCode; msg: string; internal: string | null }
  | { kind: 'not-licensed';   feature: PremiumFeature; sku: Sku }   // see 13-LICENSING
  | { kind: 'busy';           operation: string }
  | { kind: 'cancelled';      scanId: number }
  | { kind: 'io';             code: number; path?: string }          // engine IO failures
  | { kind: 'entitlement';    reason: 'expired' | 'device-mismatch' | 'revoked' };
```

`PRISM-IPC-003` — error mapping is total: every Rust error variant maps to exactly one `kind`; `any`-typed or stringly errors are forbidden and CI-checked.

### 2.2 Events (T1 push)

```ts
interface IpcEvent<E extends keyof EventsMap> {
  ev: E;
  at: number;                        // engine monotonic ms (perf counter based)
  payload: EventsMap[E]['payload'];
}
```

Events are **batched**: the drain thread coalesces into frames of ≤ 512 events or ≤ 16 ms, whichever first ([§ 4](#4-event-pump-architecture)). The renderer applies batches inside one `requestAnimationFrame`-scheduled transaction; per-event `setState` storms are forbidden (`PRISM-IPC-010`).

## 3. Command catalogue (authoritative)

Naming: `domain:action`. All commands are **invoke-style** (request/response). Sizes noted are wire payloads *after* JSON serialization (T1 is JSON-structured via structured clone; T2 uses napi-rs typed objects — zero-copy `Buffer` paths for binary frames, see § 5).

### 3.1 Lifecycle & system

| Command | Request | Response | Notes |
|---|---|---|---|
| `sys:hello` | `{}` | `{ engineVersion, rustc, features: EngineFeature[], volumes: VolumeInfo[], isAdmin }` | Called once at startup by main; renderer receives a filtered view |
| `sys:volumes` | `{ refresh?: boolean }` | `{ volumes: VolumeInfo[] }` | Drive enumeration ([06 § Volumes](06-RUST-CORE.md#5-volume-enumeration--system-info)) |
| `sys:preflight` | `{ target }` | `{ readable: boolean, requiresElevation: boolean, freeBytes: bigint, totalBytes: bigint }` | Pre-scan check; drives Welcome UX |

### 3.2 Scanning

| Command | Request | Response | Notes |
|---|---|---|---|
| `scan:start` | `{ target: ScanTarget, strategy: 'standard'\|'turbo', options: ScanOptions, entitlement: EntitlementToken \| null }` | `{ scanId: number }` | Premium check only for `turbo`; entitlement verified in engine ([13](13-LICENSING-SYSTEM.md#9-enforcement-depth-engine-boundary)) |
| `scan:pause` / `scan:resume` | `{ scanId }` | `{}` | Standard strategy only |
| `scan:cancel` | `{ scanId }` | `{}` | Cooperative; ≤ 50 ms ack |
| `scan:rescan-subtree` | `{ scanId, nodeId, options }` | `{ scanId }` | WDS-SCN-09 |
| `scan:reattach` | `{ scanId }` | `{ summary: ScanSummary \| null }` | After renderer crash |
| `scan:summary` | `{ scanId }` | `ScanSummary` | Totals, duration, errors count, size-mode |

`ScanTarget = { kind: 'volume', path } | { kind: 'folder', paths: string[] } | { kind: 'home' }` (multi-root allowed: `paths[]`, WDS-SEL-04).

`ScanOptions = { followReparse: boolean; sizeMode: 'logical'|'allocated'; includeHidden: true (always true); includeSystem: true; treatPackagesAsNodes: boolean; excludePatterns: string[] }` — exclusions are glob (our own matcher, [06 § Exclusions](06-RUST-CORE.md#24-exclusion-matcher)).

### 3.3 Tree & nodes (post-scan queries)

| Command | Request | Response | Notes |
|---|---|---|---|
| `tree:children` | `{ scanId, nodeId, sort: SortSpec, offset: number, limit: number }` | `{ total, items: NodeRow[] }` | Paged; sort in engine (arena columnar sort, [07](07-DATA-MODEL.md#2-the-arena-engine-side)) |
| `node:detail` | `{ scanId, nodeId }` | `NodeDetail` | All sizes, times, attributes, category, package flag, links, errors |
| `node:resolve-path` | `{ scanId, path: string }` | `{ nodeId } \| null` | Locate (drag-in, WDS-CTX-05) |
| `tree:expand-stats` | `{ scanId, nodeId }` | `{ files, folders, logical, allocated, unique }` | Cheap counters for tree row |

`NodeRow = { id: NodeId; name: string; isDir: boolean; logical: bigint; allocated: bigint; files: number; folders: number; category: CategoryId; badge?: NodeBadge; mtime: number|null }` — `bigint` sizes cross IPC as `BigInt` (napi-rs supports i64→BigInt; renderer uses `bigint` end-to-end, formatted by [ByteFormat](09-UI-COMPONENTS.md#byteformat)).

### 3.4 Visualization

| Command | Request | Response | Notes |
|---|---|---|---|
| `viz:layout` | `{ scanId, mode: VizMode, root: NodeId, viewport: {w,h,dpr}, options: VizOptions }` | `VizFrame` (binary, § 5) | Culled + LOD-limited ([11](11-VISUALIZATION-ENGINE.md)) |
| `viz:color-mapping` | `{ scanId, mode: 'type'\|'branch'\|'age' }` | `{ legend: LegendItem[], version: number }` | One palette per scan/version; cached in renderer |

### 3.5 Types & extensions

| Command | Request | Response | Notes |
|---|---|---|---|
| `types:list` | `{ scanId, sort }` | `{ items: TypeRow[] }` | WDS-EXT-01 |
| `types:set-color` | `{ scanId, key, color: string \| null }` | `{}` | `null` = inherit; persists (WDS-EXT-03) |

### 3.6 Search & filter

| Command | Request | Response | Notes |
|---|---|---|---|
| `filter:apply` | `{ scanId, query: FilterQuery, scope: NodeId \| 'all' }` | `{ filterId, matched: number }` | `FilterQuery = { name: string (glob/regex autodetect), categories?: CategoryId[], size?: Range, age?: Range, kind?: 'files'\|'dirs'\|'both' }` |
| `filter:clear` | `{ scanId }` | `{}` | |
| Engine pushes `filter:updated` events with matched node ids in pages. Debounce contract in [10 § Search](10-SCREENS-AND-FLOWS.md#search--filter). |

### 3.7 Premium operations (entitlement-gated, [13](13-LICENSING-SYSTEM.md))

| Command | Entitlement | Notes |
|---|---|---|
| `duplicates:run` / `duplicates:cancel` / `duplicates:groups` | required | Pipeline in [12 § Duplicates](12-HOMEGROWN-FEATURES.md#4-duplicates) |
| `cleanup:stage` / `cleanup:unstage` / `cleanup:execute` / `cleanup:queue` | required (execute) | Ledger + `IFileOperation` ([10 § Cleanup Safety](10-SCREENS-AND-FLOWS.md#cleanup-safety-the-deletion-contract)) |
| `cleanup:presets` / `cleanup:presets-scan` | required | Rule engine hits ([12 § Presets](12-HOMEGROWN-FEATURES.md#2-cleanup-presets-rule-engine)) |
| `apps:list` / `apps:footprint` / `apps:uninstall` / `apps:leftovers` | required | [12 § Uninstaller](12-HOMEGROWN-FEATURES.md#5-application-uninstaller--leftovers) |
| `snapshots:save` / `snapshots:list` / `snapshots:load` / `snapshots:diff` | required | [12 § Snapshots](12-HOMEGROWN-FEATURES.md#6-snapshots--diff) |
| `monitor:start` / `monitor:stop` | required | [12 § Monitor](12-HOMEGROWN-FEATURES.md#7-monitor) |
| `export:scan` | required | CSV/JSON writer, streamed to file with progress events |
| `scan:start` with `strategy:'turbo'` | required | [06 § Turbo Scan](06-RUST-CORE.md#3-turbo-scan--raw-ntfs-opt-in-elevated) |

Free-tier engine ops (always available post-trial without license? — **No**: after trial expiry the app is activation-gated at startup (LicenseGate), so engine gating applies to premium ops *within* a licensed session; see [13 § SKUs](13-LICENSING-SYSTEM.md#1-skus-and-feature-matrix)).

### 3.8 Licensing (main-process handled, not engine)

Handled entirely in main via the license client ([13](13-LICENSING-SYSTEM.md)): `lic:activate`, `lic:validate`, `lic:deactivate`, `lic:status`, `lic:open-checkout` — the renderer calls these through the same T1 envelope; they never reach the Rust engine except as token verification calls (`lic:verify-token` internally from main to engine).

## 4. Event pump architecture

`PRISM-IPC-020` — The engine has exactly **one** TSFN for all events, fed by a lock-free SPSC ring (`crossbeam` array queue, capacity 8192) drained by a dedicated thread:

```
[scan workers] → crossbeam SPSC → [drain thread: batch ≤512 ev / ≤16ms] → TSFN (napi)
    → main: 'engine:event' batches → webContents.send('prism:events', batch[])
    → renderer: single subscriber → rAF-scheduled store transaction
```

Rules:
- Events are **append-only facts**; ordering per scan is guaranteed by the single drain thread.
- Backpressure: if the ring is full, workers **block** (scan slows rather than drops data); progress events may be *coalesced* (only latest per counter) but **node deltas are never dropped** (`PRISM-IPC-021`).
- Every event type has a schema test (golden JSON fixtures in `crates/prism-types/tests/`).

### Event catalogue

| Event | Payload | Frequency class |
|---|---|---|
| `scan:progress` | `{ scanId, filesSeen, bytesSeen, dirsSeen, currentPath, elapsedMs, rateFilesPerSec }` | coalesced ≤ 4 Hz |
| `scan:nodes` | `{ scanId, deltas: NodeDelta[] }` | batched (≤ 512/16 ms) |
| `scan:phase` | `{ scanId, phase: 'walking'|'aggregating'|'indexing-ext'|'done'|'failed'|'cancelled', detail? }` | transitions |
| `scan:error-batch` | `{ scanId, errors: PathError[] }` | batched |
| `scan:done` | `{ scanId, summary: ScanSummary }` | once |
| `filter:updated` | `{ scanId, filterId, matchedPage: NodeId[], total, done }` | paged |
| `dupes:progress` | `{ scanId, phase, groupsFound, hashedBytes }` | ≤ 2 Hz |
| `cleanup:progress` | `{ itemsDone, itemsTotal, lastError? }` | per item, batched |
| `apps:progress` | `{ phase, appsDone, appsTotal }` | ≤ 2 Hz |
| `monitor:sample` | `{ ts, processes: ProcessSample[], cpuTotal, memTotal }` | 1 Hz |
| `engine:warning` | `{ code, msg }` | rare |

## 5. Binary payloads (VizFrame)

`viz:layout` responses carry high tile counts; JSON is forbidden there (`PRISM-IPC-030`). Layout is serialized into a flat `ArrayBuffer` (napi `Buffer` → Electron transfers it as a real `Buffer` over structured clone; renderer receives `ArrayBuffer` zero-copy):

```
VizFrame (little-endian, 32-byte aligned header):
u32 magic 'PVF1' | u32 version | u32 scanId | u32 rootId
u32 tileCount | u8  mode | u8  flags | u16 reserved
--- tiles: TileRec × tileCount (24 bytes each) ---
i32 nodeId | f32 x | f32 y | f32 w | f32 h | u32 colorARGB | u32 meta(depth:u16|flags:u16)
--- labels: u32 labelCount, then LabelRec × labelCount (16 bytes + utf8) ---
i32 nodeId | f32 x | f32 y | u16 len | u8[len] utf8
```

Validation: renderer checks magic+version and total byte length before reading; malformed frames are a hard error (telemetry event + frame dropped with user-visible "viz degraded" notice — a **diagnostic**, not a silent fallback, per [19 A2](19-AGENT-PROTOCOL.md#a2--no-fallbacks-the-rule-the-owner-cares-about-most)).

## 6. Type codegen

`PRISM-IPC-040` — Source of truth chain:

1. `crates/prism-types/src/lib.rs` defines request/response/event structs with `#[derive(Serialize, Deserialize, NapiObject)]` + doc comments.
2. `script/codegen.ts` runs `cargo run -p prism-types -- emit-ts ../packages/shared/src/generated/` — a small Rust emitter (we own it; no external tool) writes TS types + a zod schema per type (for T1 validation in main) + a `CommandsMap`/`EventsMap` index.
3. `packages/shared` publishes `PrismClient` — typed `invoke`/`on` wrappers with seq assignment, error normalization, and in-flight dedupe for idempotent reads.
4. CI: `pnpm codegen:check` fails if generated files differ from the Rust source (drift is a build break, not a runtime surprise).

## 7. Validation rules at boundaries

| Rule | ID |
|---|---|
| T1 payloads validated with generated zod schemas in main; unknown commands rejected (`invalid-args`) | `PRISM-IPC-050` |
| Path arguments normalized (`\\?\` handling internal to engine; T1 uses Win32 paths) and length-checked ≤ 32k chars | `PRISM-IPC-051` |
| Numeric bounds: `limit ≤ 1000` for `tree:children`; `tileCount ≤ 250_000` for `viz:layout`; batch sizes capped in engine | `PRISM-IPC-052` |
| `seq` monotonic per session; main logs cmd+seq+duration at `debug` level (never payload contents — paths may be sensitive) | `PRISM-IPC-053` |
| No renderer-originated command may carry an entitlement token (tokens are main-injected after engine-gate logic) — prevents renderer-forged entitlements | `PRISM-IPC-054` |

## 8. Performance contract (details in [16](16-PERFORMANCE-BUDGETS.md))

- Command round-trip overhead (excluding engine work) ≤ 2 ms p95 main-process side.
- `tree:children` page of 500 rows ≤ 8 ms engine time; `viz:layout` for 100k-tile frame ≤ 30 ms engine time; both measured in `prism-benches` and enforced in CI perf jobs.
- Event pipeline adds ≤ 4 ms latency from worker to renderer transaction at full scan rate on the reference machine (see [16 § Reference Machines](16-PERFORMANCE-BUDGETS.md#1-reference-machines)).
