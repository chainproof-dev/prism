# PRISM Repository Worklog

> Protocol: docs/19-AGENT-PROTOCOL.md § Worklog. Append-only; every session
> records Done/Deferred/Risks/Next with evidence.

---

## Session 1 — Phase 0 + 1 + 2 + 3 (spine) — implementation kickoff

Agent: Super Z (main build agent)
Tasks: P0-001..P0-006, P1-001..P1-005/P1-007/P1-009 + viz + licensing, P2-001..P2-005 (core), P3-004/006/007 partial

Done:
- **Toolchains**: Rust 1.98.1 (pinned `rust-toolchain.toml`), Node 24, pnpm 11.19; real latest versions verified against crates.io + npm (manifest plan versions corrected: crossbeam-channel 0.5, ed25519-dalek 3, rusqlite 0.40, syn 3, electron-vite 5, motion 13, better-sqlite3 13, pino 10, execa 10, TS 7).
- **prism-types**: full DTO surface (commands/events/scan/viz/filter/licensing/cleanup DTOs) + `emit-ts` syn-based codegen → protocol.ts/schemas.ts/commands.ts/events.ts with zod schemas + CommandsMap + premium map + EventsMap. CI drift gate wired. BigInt end-to-end (u64 → bigint, AMM-003).
- **prism-ntfs**: MFT record parsing (fixup application, attribute iteration), run-list decoder (sparse/negative deltas, overflow-safe), $STANDARD_INFORMATION + $FILE_NAME parsers; 9 unit tests incl. synthetic FILE record round-trip and fixup-mismatch detection.
- **prism-core**: arena (SoA, flat-edge→CSR O(n) finalize, name interning, order-independent canonical hash), scanner coordinator (worker pool, cooperative cancel ≤50 ms, pause/resume, completion cascade for live sizes, hardlink/reparse correctness rules, free-space/unknown pseudo-nodes), Win32 NtQueryDirectoryFile backend (cfg windows) + POSIX dev backend (AMM-002), exclusion matcher (own glob + sticky-dir pruning), aggregation (ext table, 34 categories→12 hues, age histogram), viz layouts (squarified per the paper with byte→pixel scaling, sunburst/icicle/pack/mindmap/age-timeline, LOD composites) + VizFrame binary (28-byte tiles, palette-indexed, 250k cap) + TS decoder, licensing verify (Ed25519/CBOR, iat replay bound, skew, device binding), cleanup (12 presets, blocklist, staging queue), dupes pipeline core (size grouping, XXH3-128 partial fingerprints, BLAKE3 full hash), napi IPC (16 commands, TSFN event pump batched ≤512/16 ms, panic containment at every entry).
- **License server**: Fastify 5 + better-sqlite3 (WAL) + @noble/ed25519 v3 + cbor-x + zod; activate/validate/deactivate with the 1:1 error-code mapping; admin API (issue/list/revoke/release-device/extend/stats); PRSM-XXXXX-… key format; dev keypair committed (openssl-generated; engine embeds the matching public key); 3 demo keys seeded; admin CLI; **11/11 lifecycle tests green** including the engine-pubkey cross-check and bit-flip forgery rejection.
- **Desktop app**: electron-vite 5 + React 19 + Tailwind 4; main process (CSP, sandbox, contextIsolation, navigation lock, engine loader with startup diagnostic screen, command router with generated zod validation); preload (contextBridge surface); renderer: 6-theme token system (OKLCH CSS custom properties), data palette, AppShell, TopBar (tabs/search/theme), Welcome (drive cards + DonutGauge + recents), ScanningOverlay (live counters + luminance sweep + pause/cancel), VizCanvas (frame decode, cushion shading, hit grid, overlay layers), FileTree (TanStack Virtual, % bars, badges), TypeList, Inspector, status strip. Renderer builds: 697 kB bundle (budget ≤ 8 MB gz).
- **E2E proof (engine)**: `scan_e2e.rs` — totals (Σ children == parent), exclusions prune subtrees, determinism (identical canonical hash across scans), cancel never hangs, all 6 frame modes decode. **40/40 Rust tests + 11/11 license + 8/8 shared green.**
- **E2E proof (.node)**: smoke through the built `prism-core.node` — hello (honest feature list) → scan → streamed events → summary → children (free-space node present) → detail → types → binary PVF1 frame. Full pipeline verified.
- **Infra**: CI (`ci.yml`: fmt/clippy/test, codegen drift, js, trace, fixtures); trace scanner (found and fixed 26 real violations — WDS- row ids in code comments normalized to parity-*); docs link checker; deterministic fixture generator (FIX-S byte-identical across runs); `script/dev.mjs` one-command startup (R11); `copy-engine.mjs`; dev keypair; LICENSE-PROPRIETARY; .nvmrc.

Deferred:
- `P1-006` SQLite migrations (schema drafted; lands with snapshots P5).
- `P1-008` SHIL icons; `P1-010` criterion FIX-L suite + benches beyond arena scaffold.
- Turbo scan pipeline wiring (prism-ntfs parsers complete; elevation helper + FSCTL wiring is P5).
- `P2-006` command palette; `P2-007` screenshot harness + visual regression; contrast/a11y CI harness.
- `P3-008` context menus + delete ledger flows; `P3-010` rescan subtree/full refresh.
- Electron window-state persistence, Settings sheet, About dialog.
- Windows-host CI matrix + `cargo check --target x86_64-pc-windows-msvc` job (code written for cfg(windows); not cross-checked this session — NEXT SESSION MUST RUN IT).

Risks/Notes:
- napi-rs v3 API learned empirically (ThreadsafeFunction const generics, Function::build_threadsafe_function, dyn-symbols for test linking) — documented inline; compat-mode feature enabled for JsFunction.
- The plan's manifest versions were partially fictional (crates.io reality differs); all pins corrected and verified.
- AMM-002 (dev platform) + AMM-003 (wire conventions) filed in docs/amendments/.

Next:
1. `cargo check --target x86_64-pc-windows-msvc` (validate cfg(windows) code compiles).
2. P2-006 command palette + P2-007 screenshot harness; Settings sheet.
3. P3-008 context menus + cleanup ledger (engine `cleanup:stage/execute` already present).
4. Persistence migrations (P1-006) + snapshots (P5 track).
5. Electron run on Windows host; packaging (electron-builder NSIS) — doc 15.

### Session 1 addendum — windows-msvc cross-check
- `cargo check --target x86_64-pc-windows-msvc` validated for `prism-types` and `prism-ntfs` (pure Rust — green).
- `prism-core` cross-check blocked on this Linux host by native build scripts (libsqlite3-sys/zstd-sys need MSVC). The cfg(windows) modules (`scanner/win32.rs`, `sysinfo/win32.rs`) are Windows-CI territory: the `windows-msvc` job must run on the Windows matrix runner (added to ci.yml as `windows-codegen` note). Not a code defect — a toolchain availability limitation, recorded here per A10.
