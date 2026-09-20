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

### Session 1 final verification
- Rust: **40/40** · License server: **11/11** · Shared TS: **8/8**
- Trace scanner: clean · Docs links: clean (25 docs) · Fixtures: deterministic
- Desktop (main+preload+renderer): builds clean, typechecks clean (TS 7 strict + noUncheckedIndexedAccess + exactOptionalPropertyTypes)
- Engine smoke through the real `.node`: SMOKE OK (hello → scan → events → summary → children → detail → types → PVF1 frame)
- Windows cross-check: prism-types + prism-ntfs green for x86_64-pc-windows-msvc; prism-core needs the Windows CI runner (native build scripts) — job added to ci.yml

## Session 2 — parity completion + homegrown + licensing client + windows validation

Agent: Super Z (main build agent)
Tasks: worklog "Next" list (command palette, cleanup ledger, SQLite migrations, turbo wiring) + Windows CI validation + Phases 4-10 push.

Done:
- **Windows validation (the ② item)**: built a zig-cc cross toolchain (`script/cc-zig-msvc.py`, `script/ar-msvc-shim.py`) that compiles the bundled C deps (libsqlite3-sys, zstd-sys) to COFF on Linux; `cargo check --target x86_64-pc-windows-msvc -p prism-core` is GREEN — every cfg(windows) module (win32 NtQueryDirectoryFile scanner, sysinfo, monitor NtQuerySystemInformation, apps registry, cleanup SHFileOperation, turbo raw-volume reader) type-checks for Windows. Found and fixed **10 real Windows-only defects** never compiled before: windows-sys 0.61 API relocations (NtQuerySystemInformation→Wdk, DRIVE_*→WindowsProgramming, GetSystemTimes→Threading, UNICODE_STRING→Foundation), HKEY pointer types, FILEOPERATION u16 flags + FOFX misuse (IFileOperation-only flag on SHFileOperation — replaced with FOF_ALLOWUNDO semantics), unsafe extern blocks (edition 2024), NTSTATUS literal overflows, pointer mutability, missing docs. CI: new `windows-cross` ubuntu job + the windows-latest authoritative job.
- **P1-006 closed**: `persistence.rs` — SQLite app.db with append-only migrations v1–v3 (scans history, settings KV, snapshots w/ zstd payloads, type_colors), reopen-idempotent; scan completion auto-records history; PSNP1 privacy-safe share export (path keys only, tested).
- **Turbo scan wired (ADR-06)**: `scanner/turbo.rs` — platform-independent MFTEntry→arena builder (FRN-ordered deterministic, orphan skipping, root merge, reparse-first classification; property-tested incl. hash determinism under input reordering) + Windows raw-volume reader (boot BPB → $MFT runlist → extent-mapped positional reads). Coordinator dispatches strategy=turbo honestly (folder targets + non-NTFS + non-elevated = explicit errors, never silent standard substitution). ipc gate now entitlement-only. AMM-004 filed (exclusions on turbo = viz-layer).
- **20+ IPC commands wired** (`ipc/extend.rs`): sys:preflight, scan:rescan-subtree/reattach, node:resolve-path, viz:color-mapping, types:set-color, duplicates:run/cancel/groups, cleanup:presets-scan/stage/unstage/queue/execute, apps:list/footprint/leftovers, snapshots:save/list/diff, monitor:start/stop, export:scan, engine:open-db/get-setting/set-setting/recent-scans (new protocol command + regenerated TS).
- **New engine modules**: monitor.rs (1 Hz delta sampling; Windows NtQuerySystemInformation + /proc dev backend, tested on Linux), apps/mod.rs (registry inventory cfg(windows) + token footprints with evidence lists + leftovers w/ honest-empty off-Windows), export.rs (streamed CSV RFC4180+BOM / NDJSON, tested), cleanup/execute.rs (SHFileOperation recycle/permanent + fail-loud per-item outcomes + progress events), dupes runner (4-phase pipeline, XXH128→BLAKE3, hardlink collapse, tested end-to-end on tempdirs).
- **Main process**: real LicenseClient (activate/validate/deactivate via Electron net, 24h heartbeat, offline grace, HMAC-stamped trial, safeStorage/DPAPI-encrypted blob, instance identity), settings service (JSON, section resets) + window-state persistence, app menu, license/settings/menu IPC channels, db open at boot.
- **Renderer**: command palette (cmdk; recents, jump-biggest, themes, tabs), LicenseGate + trial + GraceBanner + LockedTab previews, workspace tabs (Duplicates/Monitor/Snapshots/Applications/Leftovers), cleanup ledger sheet + context menu + errors drawer + Quick Wins + queue panel, Settings sheet, viz rail (9 modes) + color-mode rail + scope segmented + Table/Bars DOM modes, search filter (150ms debounce contract), full keyboard map, i18n (en-US + de-DE + pseudo-loc).
- **Packaging (Phase 9 structure)**: electron-builder.yml (NSIS+portable, per-arch resources, differential updates, verifyUpdateCodeSignature), installer.nsh (Explorer context menu WDS-CTX-04), app.config.json, release.yml (tag→matrix→sign-verify→SBOM→attest→staged feed), script/sbom.py (CycloneDX), runbooks (rollback + support).
- **Clippy debt cleared**: `cargo clippy --workspace --all-targets -- -D warnings` fully green (fixed ~50 pre-existing violations that would have failed CI: prism-ntfs expect_used (guarded le-read helpers), sort_by_key, integer-division allows with rationale, test-module lint policies).

Deferred:
- Scheduler (PRISM-HG-080) — Task Scheduler registration is Windows-only; DEFERRED-WINDOWS.
- P2-007 screenshot harness + visual regression — needs headed Electron on Windows.
- Perf benches on R1 hardware (throughput numbers), signing execution, NSIS install matrix — all DEFERRED-WINDOWS (pipelines scripted).
- Dodo checkout round trip — needs provider credentials (ManualProvider covers dev).

Risks/Notes:
- The zig cross-CC shim compiles the C deps for windows-gnu COFF (zig bundles MinGW headers, not UCRT) — sound for `cargo check` (no linking); the windows-latest CI job remains the authoritative MSVC build. Documented in the shim header.
- `filter:apply` wire payload uses the generated FilterApplyQuery {query:{name,categories,kind}, scope} — TopBar sends the zod-valid shape.
- Licensing grants mirror in main is server-response-driven (no CBOR decoder dep in TS — the ENGINE verifies the token itself; UI checks are cosmetic by design, PRISM-LIC-040).

Next:
1. Windows host: run the windows-latest CI job (msvc build + tests + engine smoke), NSIS install matrix (Win10 21H2/Win11 24H2, x64+arm64).
2. Parity matrix row-by-row QA scripts + sign-offs (docs/03 § 12) on Windows.
3. Screenshot harness + visual regression baselines (P2-007).
4. Scheduler implementation (Task Scheduler registration, scope-guarded).
5. Perf bench suite on R1 (bench-scan gate, ≥150k/s standard, ≥1M/s turbo).

### Session 2 final verification
- Rust: **61 green** (47 core + 5 ntfs + 9 types) · clippy `-D warnings` clean · fmt clean
- License server: **11/11** · Shared TS: **8/8**
- Windows cross-check (msvc target): prism-types + prism-ntfs + **prism-core** GREEN (zig shim)
- Desktop: typecheck clean (TS7 strict), build 915 kB renderer
- Codegen drift: regenerated with engine:recent-scans — clean

## Session 3 — scheduler + benches + parity dashboard + screenshot harness + CI/GitHub

Agent: Super Z (main build agent)
Tasks: last open worklog items (scheduler PRISM-HG-080, criterion FIX-L suite P1-010, parity dashboard, screenshot harness P2-007) + GitHub repo + windows/ubuntu CI.

Done:
- **Scheduler (PRISM-HG-080) — CLOSED**: `crates/prism-core/src/scheduler.rs` — spec validation (label ≤120, absolute target, time_min ≤1439, ISO weekday dedup+sort), engine-DB-backed spec store (`scheduler.specs`), Windows Task Scheduler mirroring via schtasks (`PRISM\<id>`, /SC DAILY|WEEKLY|ONLOGON, /D MON..SUN, /ST HH:MM), honest `dev-file` backend off-Windows, DST-aware next-run math (GetTimeZoneInformation + Hinnant civil-date algorithms), digest from the two newest snapshots (id DESC tiebreaker — found and fixed a same-ms ordering bug in list_snapshots). 4 IPC commands (`scheduler:list/upsert/delete/digest`, entitlement-gated at the dispatcher). Scope guard BY CONSTRUCTION + pinned by test: the only schedulable action is `"<runner_exe>" --background-scan "<target>"` — no scheduled cleanup can exist. Main: `--background-scan` headless boot (scan → auto-snapshot → Notification → quit; 20-min safety valve; runner_exe substituted main-side, renderer cannot point tasks at arbitrary binaries). Renderer: Settings → Scheduler section (list/add/pause/resume/remove, client-side next-run preview, locked state when unlicensed). Wire-format pin test: ScheduleTrigger tagged-kebab/camel round-trip vs generated zod (renamed_all_fields fix).
- **Criterion FIX-L suite (P1-010) — CLOSED**: `prism-benches` — synth arena builder (fanout-8/depth-6, LCG sizes, bottom-up size rollup), bench-agg (203 Melem/s @120k), bench-viz (all 6 canvas modes @20k+300k), bench-tree (children-page @300k), bench-scan (**372k files/s real pipeline, 10k on-disk files, posix backend** — 2.5× the 150k/s budget, on a CI container). Release-lease fix: bench must call `manager.complete()` (the coordinator is single-active-scan).
- **Parity QA dashboard**: `qa/parity-dashboard.mjs` parses docs/03 (71 WDS rows), merges `qa/parity-status.json` sign-offs, renders `docs/phases/parity-dashboard.md`; CI drift job. 5 rows signed off on automated evidence (CFG-01/03, PERF-01/03/04); 66 honestly pending Windows QA sessions.
- **Screenshot harness (P2-007) — CLOSED**: `qa/screenshots/harness.cjs` (Playwright _electron, real app + real engine + real scan; license-gate → trial start; UI-driven via drive-card click — Welcome/Settings/Scanning/Explore) + `compare.cjs` (pixelmatch, 0.5% regression threshold, missing = new baseline). Fixed a REAL packaging bug found by the harness: `@prism/shared` (TS sources) was externalized in the built main — a packaged app could not boot (ERR_MODULE_NOT_FOUND). Now bundled via `externalizeDepsPlugin({ exclude: ['@prism/shared'] })`.
- **CI matrix expanded**: windows-rust (fmt/clippy/tests/release build + .node smoke on the real win32 backend), windows-app (electron boot + screenshots, artifacts), screenshots (ubuntu xvfb, best-effort compare until runner font stacks stabilize), benches (informational), parity dashboard drift. Trace scanner violations fixed (6 files: WDS- ids in shipped code normalized to parity-*).

Deferred:
- R1/NTFS throughput numbers (150k/s standard, 1M/s turbo) — need the Windows CI run on real hardware (jobs pushed; results land in Actions).
- Windows screenshot baselines — first windows-app run captures them; compare is best-effort until then.
- Row-by-row parity sign-offs (66 rows) — Windows host QA sessions.

Next:
1. GitHub Actions: windows-rust + windows-app + screenshots + benches results (fix anything red).
2. Windows QA sessions: parity rows + NSIS install matrix.
3. Turbo bench on real NTFS volume.

## Session 4 — the real win32 root-enumeration bug (found, fixed, gated)

Agent: Super Z (main build agent)
Task: complete remaining implementation; run windows/ubuntu CI to green; validate everything; deliver runnable artifacts + final ZIP.

Done:

### The bug hunt (windows-rust CI, runs 5→7)
- Run 5 evidence: real-machine scan = 0 files + 1 error at root. First hypothesis (Win32 `\\?\` spelling invalid for NtOpenFile — needs NT `\??\`) produced commit 05d50eb. Did NOT fix it.
- Probe v1/v2 (tests/win32_nt_probe.rs): raw NT witness with correct nul handling SUCCEEDED while the REAL `open_dir` (via doc-hidden `__probe_open_dir_status` bridge) returned STATUS_OBJECT_NAME_INVALID on the same machine, same input, same test binary — and the build log showed `Compiling prism-core` (fresh), killing the stale-rust-cache theory.
- Root cause (the embarrassing kind): `open_dir`'s plain-drive-path branch appended ONLY the `\??\` prefix — `full.extend_from_slice(path16)` was missing from that one branch. Every scan root therefore opened the 4-character path `\??\` → NAME_INVALID. The other three branches (already-NT, `\\?\`, UNC) appended the path correctly; a witness copy of the same logic in the test file did too — which is exactly why the probe pair diverged.

### The fix (commit d2c480f)
- Path construction extracted to `scanner/nt_path.rs` — a **pure, platform-independent** module (no cfg gate) so the string-level tests run on Linux CI where regressions are caught in minutes, not Windows-runner round-trips. 10 unit tests: plain drive path, drive root, `\\?\` rewrite, `\\?\UNC\` rewrite, bare UNC → `\??\UNC\`, canonical `\??\` passthrough, `\Device\` passthrough, single-trailing-nul invariant, and an explicit `never_emits_prefix_without_path` regression guard for this exact bug.
- `win32.rs::open_dir` now delegates to `nt_path::push_nt_object_path` (+ STATUS_NAME_TOO_LONG guard for >u16-byte UNICODE_STRING lengths — caught by the zig cross-check as an i32 literal overflow, fixed with the `0xC000_010Fu32 as i32` house pattern).
- Probe upgraded: now ASSERTS the real `open_dir` returns STATUS_SUCCESS (production contract), not just the witness.
- Local verification: 81 tests green (was 71 — +10 nt_path), clippy -D warnings clean on host AND windows target, fmt clean, zig cross-check green, TS side untouched.

Next:
1. CI run 35520350144 (11 jobs) to full green — expected windows-rust PASS now.
2. Download CI artifacts (prism_core.dll windows-x64, staged electron bundle) for the user-testable ZIP.
3. Final full-workspace ZIP with builds.
