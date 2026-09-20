# Phase status dashboard (Session 3 close)

> Honest state per the 18-PHASE-PLAN gates. Boxes are checked ONLY where a
> command/test/build evidence line exists. Anything requiring a physical
> Windows host or signed credentials is explicitly marked DEFERRED-WINDOWS.

## G0 — Foundations ✅ (Session 1)
- [x] `npm run dev` one-command startup (server + app)
- [x] CI chain present (fmt/clippy/test/codegen/js/trace/fixtures + windows-cross)
- [x] `pnpm lic:admin issue` produces keys; SQLite persists; 3 demo keys
- [x] Fixtures byte-identical across runs (CI job)
- [x] Hardening table asserted (CSP/sandbox/contextIsolation in main)

## G1 — Rust engine core ✅ (Sessions 1+2)
- [x] Correctness tests green (47 core + 5 ntfs + 9 types)
- [x] Arena hash deterministic across runs (turbo: identical hash under input reordering — turbo tests)
- [x] IPC budgets: tree:children paged ≤1000, viz binary frames, event batches ≤512
- [x] Persistence: SQLite migrations v1–v3 (scans/settings/snapshots/type_colors) — P1-006 closed
- [x] Turbo scan wired end-to-end (ADR-06): MFT reader + arena builder + coordinator dispatch + entitlement gate; Windows compile validated
- [x] Monitor (NtQuerySystemInformation + /proc dev), apps inventory (registry), cleanup execute (SHFileOperation), export (CSV/NDJSON), dupes pipeline runner
- [ ] Throughput ≥150k files/s on R1 physical hardware — **bench suite shipped (Session 3): 372k files/s measured on the CI container** (posix backend, warm cache); R1/NTFS number lands with the Windows CI run
- [x] Criterion FIX-L bench suite (P1-010 closed, Session 3): arena push+attach, agg compute (200 Melem/s), all 6 canvas viz modes @ 20k+300k nodes, tree children-page @ 300k, real scan pipeline @ 10k on-disk files

## G2 — App shell & design system ✅ (Sessions 1+2)
- [x] Token pipeline + 6 themes (OKLCH)
- [x] AppShell/TopBar/Sidebar/Inspector chrome
- [x] Command palette (P2-006 closed) — cmdk, static + dynamic (recents, jump-biggest)
- [x] Settings sheet (P3-009 partial → closed) + About (menu dialog)
- [x] i18n mechanism (PRISM-HG-120): en-US + de-DE + pseudo-loc harness
- [x] Visual-regression harness (P2-007 closed, Session 3): Playwright harness boots the real app (real engine + real scan), captures welcome/settings/scanning/explore; pixelmatch compare vs committed Linux baselines; wired into CI (ubuntu xvfb + windows job, artifacts uploaded)

## G3 — Parity spine ✅ (Sessions 1+2)
- [x] Welcome → Scanning → Explore; tree + treemap + types + selection sync + inspector
- [x] Context menus + delete ledger flows (P3-008 closed): stage → ledger review → execute (recycle/permanent, fail-loud outcomes), blocked-path rejections
- [x] Errors drawer (scan-error-batch wired)
- [x] Rescan subtree + F5 + free-space/unknown nodes
- [x] Keyboard map implemented (Ctrl+K/N/,/F/I, Alt+1..6, Alt+↑, F5, Esc ordering)
- [x] 9 viz modes: 6 canvas (engine frames) + Table/Bars DOM + Folders (tree)

## G4 — Parity completion ◐ (structure complete; sign-off pending)
- [x] All screen states exist with honest empty/error states
- [x] Type colors: engine db-backed overrides + `types:set-color`
- [x] Drag-out/copy path (context menu); extension editing via settings
- [ ] `npm run qa:parity` dashboard — **SHIPPED (Session 3): 71 rows rendered, 5 signed off (automated-test rows), 66 pending Windows QA sessions** — see docs/phases/parity-dashboard.md; row-by-row sign-offs REQUIRE a Windows host (WDS behavior references)

## G5 — Homegrown features ✅ (Session 2; per-feature QA pending Windows)
- [x] Quick Wins (presets-scan ranked), Cleanup presets + full ledger, Stale (age buckets shared with ramp), Duplicates (full pipeline + staging), Snapshots + diff (10MB floor + PSNP1 share), Applications + Leftovers (registry-backed; honest empty off-Windows), Monitor, Turbo scan, Exports, Preview pane (honest no-handler state), Command palette
- [x] Locked-tab previews (20% mask + conversion card, no fake data)
- [x] Scheduler (PRISM-HG-080) — **CLOSED (Session 3)**: engine scheduler module (spec validation, DB-backed store, Windows schtasks registration `PRISM\<id>`, honest dev-file backend off-Windows, next-run math with DST-aware UTC offset, digest from two newest snapshots), 4 IPC commands (list/upsert/delete/digest, entitlement-gated), main-process `--background-scan` headless boot mode (scan → auto-snapshot → notification → quit, 20-min safety valve), Settings → Scheduler UI (create/pause/resume/remove, locked state honestly shown when unlicensed). Scope guard pinned by tests: the ONLY schedulable action is a background standard scan — no scheduled cleanup, ever.
- [ ] Turbo ≥1M files/s on R1 — DEFERRED-WINDOWS

## G6 — Licensing system ✅ (code complete; E2E vs live server on host)
- [x] Server: 11/11 lifecycle tests incl. forgery + engine-pubkey cross-check (Session 1)
- [x] Client: activate/validate/deactivate + heartbeat (24h) + offline grace + trial (HMAC-stamped) + DPAPI/safeStorage blob
- [x] Engine-boundary gating: every premium command requires grants (typestate-style `require_feature` at the dispatcher)
- [x] Forge tests: bit-flip rejection (server suite), replay bound, skew bounds (engine verify)
- [ ] Checkout round trip (Dodo sandbox) — needs provider credentials; ManualProvider covers dev flow

## G7/G8/G9/G10 — ◐ / DEFERRED-WINDOWS
- Perf budgets: renderer 915 kB (≤8MB ✓); interaction budgets need R2 hardware
- Security/signing: pipeline scripted (release.yml signtool verify gate); execution needs certs
- Packaging: NSIS + portable config complete; installs need Windows runners
- GA: blocked on the above by design (gate order is strict)

## Test evidence (Session 3 close)
| Suite | Result |
|---|---|
| cargo test --workspace | 72 green (58 core incl. 10 scheduler + 5 ntfs + 9 types + wire pin) |
| license-server vitest | 11 green |
| shared vitest | 8 green |
| cargo clippy --workspace --all-targets -D warnings | clean |
| cargo fmt --all --check | clean |
| cargo check --target x86_64-pc-windows-msvc (prism-types+ntfs+core) | green (zig cross-CC shim) |
| cargo bench -p prism-benches | agg 200 Melem/s · scan 372k files/s (posix, warm) · 6 viz modes benched |
| tsc (desktop, TS7 strict) | clean |
| electron-vite build | 926 kB renderer (≤8 MB budget) |
| screenshot harness (xvfb) | 4 states captured through real app + engine + scan |
| trace scanner / docs links / codegen drift | clean / clean / in sync |
| parity dashboard | 71 rows · 5 signed off · 66 pending Windows QA |
