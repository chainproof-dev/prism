# 04 — System Architecture

> Owner doc: [00-INDEX.md](00-INDEX.md) · Upstream: [01-PRODUCT-VISION.md](01-PRODUCT-VISION.md) · Downstream: [05-IPC-PROTOCOL](05-IPC-PROTOCOL.md), [06-RUST-CORE](06-RUST-CORE.md), [07-DATA-MODEL](07-DATA-MODEL.md), [13-LICENSING-SYSTEM](13-LICENSING-SYSTEM.md), [15-BUILD-PACKAGING](15-BUILD-PACKAGING.md).
> Requirement IDs: `PRISM-ARCH-*`.

---

## 1. The one-paragraph architecture

A single Electron desktop process tree: the **main process** owns a **Rust engine loaded in-process as a N-API module** (`prism-core.node`, built with napi-rs v3). The engine exposes a small typed command surface and pushes streaming events through ThreadsafeFunctions. The main process bridges those events to the **renderer** (React 19 + Vite 8, contextBridge-safe IPC) and owns window lifecycle, menus, native dialogs, and the licensing client module. The Rust engine runs all filesystem work on its own worker threads (std threads + crossbeam channels; rayon pools for parallel aggregation phases) — never on the JS thread, never on the Electron main thread. A sibling workspace in the monorepo contains the **license server** (TypeScript, Node 24), which the desktop client talks to over HTTPS (localhost:8080 in first build). Auto-update uses electron-builder's NSIS updater with cryptographically signed manifests.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│ RENDERER PROCESS (sandboxed, contextIsolation: true, nodeIntegration: false) │
│  React 19 + TS 5.9 + Vite 8 + Tailwind 4                                    │
│  ┌────────────┐ ┌──────────────┐ ┌────────────────────┐ ┌────────────────┐   │
│  │ AppShell   │ │ 9 viz modes  │ │ TanStack Table +   │ │ License/trial  │   │
│  │ + Sidebar  │ │ Canvas2D/    │ │ Virtual lists      │ │ gate UI        │   │
│  │ + Inspector│ │ WebGL paint  │ │ (react-virtual)    │ │                │   │
│  └────────────┘ └──────────────┘ └────────────────────┘ └────────────────┘   │
│  state: zustand + immer-free selectors · IPC via preload contextBridge       │
└───────────────▲───────────────────────────────────────────────▲──────────────┘
                │ ipcRenderer.invoke / .on (typed, batched)      │
┌───────────────┴───────────────────────────────────────────────┴──────────────┐
│ PRELOAD (contextBridge) — api surface: window.prism = { invoke, on, off }    │
└───────────────▲───────────────────────────────────────────────▲──────────────┘
                │ ipcMain.handle / webContents.send             │
┌───────────────┴───────────────────────────────────────────────┴──────────────┐
│ MAIN PROCESS (Electron 44, ES2022 CJS/ESM main)                              │
│  ┌────────────────┐  ┌───────────────────┐  ┌─────────────────────────────┐  │
│  │ Window/menu/   │  │ License client    │  │ Updater (electron-updater)  │  │
│  │ dialogs/shell  │  │ (DPAPI storage,   │  │ signed manifests            │  │
│  │ integration    │  │ Ed25519 verify)   │  │                             │  │
│  └────────────────┘  └───────────────────┘  └─────────────────────────────┘  │
│        │ require('prism-core')  ← N-API v3 module, in-process                │
│  ┌─────┴───────────────────────────────────────────────────────────────────┐ │
│  │ RUST ENGINE (crate prism-core, edition 2024, Rust 1.98)                 │ │
│  │  ┌──────────┐ ┌───────────┐ ┌────────────┐ ┌─────────────────────────┐  │ │
│  │  │ Scanner  │→│ Aggregator│→│ Viz layout │ │ Licensing entitlement  │  │ │
│  │  │ std thr. │ │ rayon     │ │ (squarify, │ │ checks (Ed25519,       │  │ │
│  │  │ FindFirst│ │ arena     │ │  sunburst, │ │ engine-gate premium    │  │ │
│  │  │ /MFT opt │ │ columnar  │ │  icicle…)  │ │ operations)            │  │ │
│  │  └──────────┘ └───────────┘ └────────────┘ └─────────────────────────┘  │ │
│  │  events → ThreadsafeFunction (batched, lock-free SPSC ring → main)       │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────┘
                 │ HTTPS (activate/validate/deactivate, webhooks)
┌────────────────┴─────────────────────────────────────────────────────────────┐
│ LICENSE SERVER (apps/license-server, TS, Fastify) — localhost:8080 (dev)     │
│  keys · entitlements (Ed25519-signed) · device bindings · rate limits        │
│  payment webhooks (Dodo adapter) · admin CLI · demo key seeded in dev        │
└──────────────────────────────────────────────────────────────────────────────┘
```

## 2. Architectural decisions (ADR summary — full text lives in `docs/adr/`)

### ADR-01 — In-process N-API engine, not a sidecar process
**Decision:** Rust engine loads into the Electron **main** process via napi-rs v3. **Alternatives rejected:** (a) separate Rust sidecar process with stdio/IPC — rejected: serialization tax on multi-MB event batches, process-lifecycle complexity, worse cold-start, no shared memory for the arena snapshot; (b) Rust in the *renderer* via nodeIntegration — rejected: breaks contextIsolation security model; (c) Tauri-style — out of scope; Electron is the chosen shell (R4).
**Consequences:** engine panics must be impossible (panic=abort is set for release; every FFI boundary is `catch_unwind`-wrapped and converts to `EngineError::Internal` — see [19-AGENT-PROTOCOL A5](19-AGENT-PROTOCOL.md#a5--never-panic-across-ffi-contain-every-failure)); engine worker threads must never call blocking N-API APIs; all engine→JS traffic goes through one TSFN with a dedicated drain thread (see [05 § Event Pump](05-IPC-PROTOCOL.md#4-event-pump-architecture)). `PRISM-ARCH-001`.

### ADR-02 — Main-process engine ownership, renderer never sees N-API
The renderer is fully sandboxed. Every engine command is `ipcRenderer.invoke('cmd', payload)` → main → typed Rust call. Response envelope includes `seq` for correlation. This keeps one security boundary, one place for entitlement checks, and clean preload typing. `PRISM-ARCH-002`.

### ADR-03 — Streaming-first, snapshot-second
Scan results stream to the UI as **batches of node deltas** (coalesced ≤ 4 Hz UI-side) *during* the scan, and are simultaneously built into an immutable **arena snapshot** in Rust (and mirrored into renderer-side stores for the currently-visible window only). Post-scan interactions (tree expand, viz relayout, filter) are queries against the engine (`tree:children`, `viz:layout`, `filter:apply`) — the renderer never holds the full 8M-node dataset. See [07-DATA-MODEL § Windowing](07-DATA-MODEL.md#5-windowing--renderer-side-model). `PRISM-ARCH-003`.

### ADR-04 — SQLite for durable artifacts only
SQLite (via `rusqlite` with bundled SQLCipher-free build, `WAL` mode) stores **snapshots** (scan shapes for diffing), **settings backup/migrations**, **scan history metadata**, and the **license entitlement cache** (encrypted row; primary license secret lives in Credential Manager, [13 § Client Storage](13-LICENSING-SYSTEM.md#6-client-storage)). The live scan arena is **memory-only** — SQLite is never on the hot path. `PRISM-ARCH-004`.

### ADR-05 — Fail-closed licensing with engine-boundary enforcement
Premium engine operations (`scan:turbo`, `duplicates:run`, `cleanup:execute`, `apps:uninstall`, `snapshots:save`, `export:csv`) require a valid **entitlement token** passed with the command; the Rust engine verifies the Ed25519 signature + expiry + device binding before executing (public key compiled in). UI gating is a courtesy layer, never the boundary ([13 § Enforcement Depth](13-LICENSING-SYSTEM.md#9-enforcement-depth-engine-boundary)). `PRISM-ARCH-005`.

### ADR-06 — Two first-class scan strategies (not a fallback)
- **Standard scan** — parallel directory enumeration (`FindFirstFileW`/`NtQueryDirectoryFile` fast path), no elevation, works on any readable root. Default.
- **Turbo scan** — raw NTFS MFT walk (volume handle via elevated helper consent, FSCTL_ENUM_USN_DATA + MFT parsing, [06 § Turbo Scan](06-RUST-CORE.md#3-turbo-scan--raw-ntfs-opt-in-elevated)). Opt-in per scan.

Both are complete production paths with separate test matrices. Neither silently substitutes for the other: if the user picks Turbo and elevation is declined, the scan **does not start** — the UI explains and offers Standard explicitly. This satisfies the no-fallback rule ([19 A2](19-AGENT-PROTOCOL.md#a2--no-fallbacks-the-rule-the-owner-cares-about-most)) while keeping both paths first-class. `PRISM-ARCH-006`.

### ADR-07 — Renderer tech (pinned)
Electron 44 (Chromium 148 line) · React 19 · TypeScript 5.9 strict · Vite 8 ( Rolldown-based) · Tailwind CSS 4 (CSS-first config) · shadcn/ui source-owned components on Radix primitives · Lucide icons · TanStack Table 9 + TanStack Virtual · Recharts 3 for auxiliary stat charts · Zustand stores · `motion` (Framer Motion v12+) for choreography. Canvas2D for all dense visualizations, WebGL2 backend for treemap ≥ 50k visible tiles (auto-selected, measured — see [11 § Renderer](11-VISUALIZATION-ENGINE.md#2-renderer-architecture)). Exact pinned versions: [20-DEPENDENCY-MANIFEST](20-DEPENDENCY-MANIFEST.md). `PRISM-ARCH-007`.

### ADR-08 — Workspace-shaped monorepo
pnpm workspaces + cargo workspace in one repo (layout in § 4). License server is an app workspace with its own deployable. Shared TS types live in `packages/shared` and are **generated from Rust type definitions** via a build-time codegen step (`prism-types`) so the IPC surface cannot drift ([05 § Codegen](05-IPC-PROTOCOL.md#6-type-codegen)). `PRISM-ARCH-008`.

## 3. Process & thread topology

| Process/thread | Owns | Never does |
|---|---|---|
| Electron main | windows, menus, dialogs, engine instance, license client, updater | any filesystem scanning, JSON serialization of > 1 MB payloads on the main thread |
| Renderer | UI, viz painting, virtualized lists, interaction state | direct filesystem access, node APIs, N-API |
| Rust engine: coordinator thread | command intake (N-API), scan lifecycle, arena writes (single writer) | blocking calls into JS |
| Rust engine: scan workers (N = min(cores, 32)) | directory enumeration, stat'ing | arena mutation (they send records over crossbeam channel to coordinator) |
| Rust engine: aggregation pool (rayon) | post-scan rollups, extension stats, duplicate candidate grouping, viz layout | UI-thread awareness |
| Rust engine: TSFN drain thread | drains the SPSC event ring, batches ≤ 16 ms / ≤ 512 events, calls TSFN | work other than draining |

Cancellation is cooperative: every worker checks an `AtomicBool`/`CancellationToken` between directory batches; cancel latency budget ≤ 50 ms (see [16](16-PERFORMANCE-BUDGETS.md#2-scan-budgets)).

## 4. Repository layout

```
prism/
├── apps/
│   ├── desktop/                    # Electron app
│   │   ├── src/main/               # main process (TS)
│   │   │   ├── engine/             # N-API bindings, lifecycle, event bridge
│   │   │   ├── licensing/          # license client, DPAPI, credential store
│   │   │   ├── menus/  windows/  updater/  shell/   # integrations
│   │   │   └── index.ts
│   │   ├── src/preload/            # contextBridge surface (TS, typed)
│   │   ├── src/renderer/           # React app (see 09/10/11)
│   │   │   ├── app/ components/ features/ stores/ styles/ canvas/ lib/
│   │   ├── electron.vite.config.ts # vite-plugin-electron pipeline
│   │   ├── resources/              # icons, installer assets
│   │   └── package.json
│   └── license-server/             # Fastify TS service (13-LICENSING-SYSTEM)
│       ├── src/ (routes, services, db, crypto, admin)
│       └── package.json
├── crates/
│   ├── prism-core/                 # engine: scanner, arena, aggregator, layout
│   │   └── src/{scanner,arena,agg,viz,dupes,apps,cleanup,licensing,sysinfo,ipc}.rs
│   ├── prism-ntfs/                 # MFT/USN parsing for turbo scan
│   ├── prism-types/                # serde types ↔ TS codegen source
│   └── prism-benches/              # criterion benches
├── packages/
│   ├── shared/                     # generated TS types + IPC client (no hand edits)
│   └── ui/                         # design-system components (owned shadcn base)
├── docs/                           # THIS build plan + adr/ + amendments/ + worklog.md
├── qa/                             # QA scripts, fixtures generator, parity dashboard
├── script/                         # dev tooling (dev.ps1, codegen, trace-scan)
├── package.json  pnpm-workspace.yaml  rust-toolchain.toml  Cargo.toml  .cargo/config.toml
└── .github/workflows/              # ci.yml, release.yml (see 15, 17)
```

## 5. Data flow walk-throughs

### 5.1 Standard scan (happy path)
1. Renderer: user picks drive `D:` → `invoke('scan:start', { target: {kind:'volume', path:'D:\\'}, strategy:'standard', options:{followReparse:false, sizeMode:'allocated'} , entitlement})`.
2. Main: validates envelope, forwards to `engine.scan_start(...)`, registers a scan lease id.
3. Engine coordinator: creates `ScanContext`, spawns workers; workers enumerate directories ( breadth-first frontier queue with work stealing via crossbeam deque), streaming `FoundRecord`s to the coordinator.
4. Coordinator: writes into the arena (single-writer), emits `ScanProgress` + batched `NodeDelta`s onto the SPSC ring; drain thread batches → TSFN → main → `webContents.send('scan:events', batch)`.
5. Renderer: applies deltas to visible windows only; live counters update at 4 Hz; viz requests relayout at coalesced intervals.
6. Completion: engine finalizes aggregation (rayon rollup), returns `ScanSummary` (root stats, duration, error count); renderer switches from scanning overlay to Explore.

### 5.2 Treemap interaction
1. Hover: renderer does **local** hit-testing against the last layout payload (`viz:layout` returned a `VizFrame` with rects + spatial grid, [11](11-VISUALIZATION-ENGINE.md#41-treemap-parity-contract-wds-tmp-0111)) — no IPC per hover. Tooltip composed from the frame's node refs, enriched by `node:detail` fetch (cached) only when a node first hovered.
2. Selection: `store.selection.set(nodeId)`; other panes react; inspector issues `node:detail` + `node:children` (top-N) queries.
3. Zoom: `viz:layout` with new root + viewport; engine returns the culled frame (≤ `maxTiles` rects by LOD, [11 § LOD](11-VISUALIZATION-ENGINE.md#3-level-of-detail-lod--the-scale-rules)).

### 5.3 Licensing activation
See [13-LICENSING-SYSTEM § Activation Flow](13-LICENSING-SYSTEM.md#51-activation) — main-process client, Ed25519 entitlement persisted to Credential Manager + encrypted SQLite row, engine receives token for premium commands, renderer reflects license state via store events.

## 6. Failure domains & containment

| Failure | Containment |
|---|---|
| Engine worker panic | `catch_unwind` at thread boundary → `EngineError::Internal` event → scan state = `failed` with diagnostics; app stays alive; offer "copy diagnostics" |
| N-API module load failure | Startup diagnostic screen (not a white screen): signature/arch/version mismatch rendered as actionable error; blocks app with copyable details |
| Renderer crash | Electron `render-process-gone` → relaunch renderer, restore last session (scan lease survives in engine; re-attach by `scan:reattach`) |
| License server unreachable | Grace mode with countdown ([13 § Offline Grace](13-LICENSING-SYSTEM.md#53-offline-grace)); core parity features keep working per SKU policy during grace |
| OOM risk (arena growth) | Arena memory budget guard: at 8M nodes engine completes with a `truncated` flag + guidance to rescan subpaths; hard ceiling tested in bench ([16](16-PERFORMANCE-BUDGETS.md#2-scan-budgets)) |
| Update failure | Updater verifies signature+hash+version policy; staged install with rollback ([15 § Auto-Update](15-BUILD-PACKAGING.md#6-auto-update-security)) |

## 7. Security posture summary (details in [14-SECURITY-MODEL](14-SECURITY-MODEL.md))

- Renderer sandbox on, contextIsolation on, `nodeIntegration` off, remote content off; CSP locked to `self`; no `webview` tags; navigation locked to app origin.
- All engine commands validated (payload schemas, size caps, path normalization) in main *and* engine.
- No user file paths/names in any telemetry or logs written outside the machine (in-app "diagnostics bundle" is user-initiated, reviewed in UI before copy).
- Code signing (Authenticode) for every shipped binary; signed update manifests; see [14](14-SECURITY-MODEL.md), [15](15-BUILD-PACKAGING.md).

## 8. What this architecture explicitly does NOT do

- No cloud components other than the license server and update host. No analytics on file data. No auto-updating without signed manifests. No loading remote code. No in-app browser beyond external `shell.openExternal` for help pages (https-only, user-initiated).
- No service/daemon for v1 other than the opt-in elevation helper for turbo scan ([06 § Elevation](06-RUST-CORE.md#7-elevation-strategy) — spawned on demand, narrow interface, exits when scan ends).
