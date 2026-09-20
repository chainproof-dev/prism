# Phase 0 — Foundations (live checklist)

> Entry: bootstrap. Exit gate: G0 (docs/18-PHASE-PLAN.md).

## Tasks

- [x] `P0-001` monorepo init: pnpm workspaces, cargo workspace, `.cargo/config.toml`, `rust-toolchain.toml` (1.98.1)
- [x] `P0-002` electron-vite app builds (main + preload + renderer); CSP locked in index.html; sandbox + contextIsolation on; navigation lock
- [x] `P0-003` `prism-core` compiles the full napi command surface; codegen emits TS types + zod schemas; drift check wired (`cargo run -p prism-types --features emit --bin emit-ts -- --check …`)
- [x] `P0-004` license-server: Fastify 5 + better-sqlite3 + Ed25519 tokens + `/v1/health` + dev keypair (`dev-keys/ed25519.pem`, committed) + seed script (3 demo keys) + admin CLI; 11/11 lifecycle tests green incl. engine-pubkey cross-check
- [x] `P0-005` CI chain (`ci.yml`): rust fmt/clippy/test, codegen drift, js tests, trace scanner (clean), docs links (clean), fixture determinism
- [x] `P0-006` fixture generator v1 (`qa/mkfixtures.py`): FIX-S deterministic (verified byte-identical across two runs); FIX-M structure classes
- [x] `npm run dev` one-command startup wired (`script/dev.mjs`): license server :8080 + desktop app with `LICENSE_SERVER_URL` injected

## Gate G0 checklist (evidence)

- [x] Engine smoke through the real `.node` module: hello → scan → events → summary → tree:children → node:detail → types:list → viz:layout (binary PVF1 frame) — `node /tmp/engine-smoke.mjs` output recorded in worklog
- [ ] `pnpm run dev` full visual pass on Windows (requires Windows host; dev-platform backend verified on Linux instead — AMM-002)
- [x] Trace scanner clean; docs links clean (602-link audit inherited from plan v1.0)
- [x] Fixtures reproduce byte-identically (CI job `fixtures`)
- [ ] Security persona pass on the baseline (deferred to G0 review session)

# Phase 1 — Rust engine core (live checklist)

- [x] `P1-001` Enumeration primitive: `DirBatch` flat-buffer design; Win32 `NtQueryDirectoryFile` path written to spec (`scanner/win32.rs`, cfg(windows)); POSIX dev backend (AMM-002)
- [x] `P1-002` Worker topology: N workers × crossbeam MPMC frontier (work-stealing deque deferred to bench-gated change), cooperative cancel (≤ 50 ms), pause/resume via condvar
- [x] `P1-003` Arena SoA + flat edge list → CSR finalize (O(n) counting sort) + deterministic row sort; name interning; canonical order-independent hash (BTreeSet of name-path rows)
- [x] `P1-004` Correctness rules: reparse not-descended default + revisit-as-link loop protection; hardlink first-seen ownership (INTERNAL_LINK2); sparse/allocated from stat; long paths (UTF-16 throughout); denial → error table; free-space + unknown pseudo nodes
- [x] `P1-005` Aggregation: completion cascade (live sizes), extension table, category table (34 curated categories → 12 anchor hues), age histogram, types:list
- [x] `P1-007` IPC: 16 commands + event pump (TSFN, batched ≤ 512/16 ms); panic containment (catch_unwind at every entry)
- [x] `P1-009` Exclusion matcher (own glob: `*?[]**`, case-folded, sticky-dir subtree pruning)
- [x] Viz layout: squarified treemap (paper's worst-ratio rule, byte→pixel area scaling) + sunburst/icicle/pack/mindmap/age-timeline + VizFrame binary (28-byte tiles, palette-indexed) + LOD/composites
- [x] Licensing verify: Ed25519 over CBOR claims, iat replay bound, exp + skew, device binding, feature matrix
- [x] E2E tests: totals, Σ-children invariants, exclusions prune, determinism (two scans → identical hash), cancel-never-hangs, all-frame-modes decode
- [ ] `P1-006` SQLite persistence migrations (schema drafted in docs/07 § 4; rusqlite wired — migrations land with snapshots P5)
- [ ] `P1-008` Icons via SHIL (Windows; Phase 3 wiring)
- [ ] `P1-010` criterion suites wired (bench/arena.rs scaffolded) + FIX-L synthetic arena

# Phase 2 — App shell & design system (live checklist)

- [x] Token pipeline: 6 themes (Nocturne/Graphite/Verdigris/Ember/Alabaster/Terracotta) as CSS custom properties + Tailwind 4 `@theme inline` mapping; instant theme switching (data-theme attr)
- [x] Data palette: 12 anchor hues + age ramp + free/unknown special fills; canvas-safe resolution (`lib/palette.ts`)
- [x] AppShell (TopBar/Sidebar/Center/Inspector/StatusStrip) with hairline separation
- [x] TopBar: workspace tabs (premium-locked state), search field, theme menu, settings
- [x] Scanning overlay: live counters ≤ 4 Hz, current path, pause/resume/cancel, luminance-sweep motif (WDS-SCN-03 substitution — canvas, zero layout cost)
- [x] Welcome: drive cards (glyphs by kind, DonutGauge, network latency chip), folder path scan, recents (persisted), no-media exclusion, entrance stagger
- [ ] Command palette (cmdk) — P2-006 pending
- [ ] Screenshot harness + visual regression — P2-007 pending
- [ ] Contrast/a11y CI harness (docs/08 § 12) — pending token-pipeline hardening

# Phase 3 — Parity spine (live checklist)

- [x] VizCanvas: VizFrame decode + palette application + cushion shading (radial-gradient path) + 64×48 hit grid + hover/selection overlay layers + ResizeObserver relayout + viz-degraded diagnostic (fail-loud)
- [x] FileTree: virtualized (TanStack Virtual), engine-paged children, % bars (category colors), badges, depth indent guides, selection sync
- [x] TypeList: swatches, counts, sizes, share bars, multi-select filter hook
- [x] Inspector: metrics grid (Logical/Allocated/Unique), attributes, Largest Inside, actions (reveal/copy/delete)
- [x] Status strip: files/bytes/duration, scan id, truncation notice
- [ ] Context menus + delete flows (ledger) — P3-008 pending
- [ ] Scanning subtree rescan + full refresh — P3-010 pending
