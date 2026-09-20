# 18 — Phase Plan (Strict Phases · Sessions Only · No Calendar)

> Owner doc: [00-INDEX.md](00-INDEX.md) · Upstream: all spec docs · Downstream: [19-AGENT-PROTOCOL](19-AGENT-PROTOCOL.md).
>
> **Convention (enforced):** this plan contains **no calendar units** — no dates, days, weeks, months, quarters. Work proceeds in **sessions** (one agent work block) and **phases** (a bundle of sessions ending in a gate). A phase is complete when its **gate** passes — never before, never "mostly". Gates are checklists; every box is verifiable by a command, a test, or a recorded QA script.

---

## 0. Phase overview

| Phase | Name | Gate summary |
|---|---|---|
| 0 | Foundations | G0 — repo/toolchain/CI greenfield green |
| 1 | Rust engine core | G1 — scanner+arena+ipc parity of data truth on fixtures |
| 2 | App shell & design system | G2 — shell renders themed screens with tokens |
| 3 | Parity spine | G3 — tree + treemap + types + selection sync (the core WDS experience) |
| 4 | Parity completion | G4 — porting matrix 100% (P/P+) |
| 5 | Homegrown features | G5 — all [12] features shipped behind license gates |
| 6 | Licensing system | G6 — full license lifecycle E2E incl. demo key + grace |
| 7 | Performance & polish | G7 — every budget row green |
| 8 | Security & signing | G8 — security review sign-offs + signed builds + updater chain |
| 9 | Packaging & release channels | G9 — installers, update feeds, runbooks |
| 10 | General availability | G10 — GA release |

Phases 1–2 and some 5-track work may interleave (the gate order is what is strict, not the wall-clock sequence — but a gate may not be signed before its dependencies' gates).

---

## Phase 0 — Foundations

**Scope:** monorepo skeleton ([04 § 4](04-SYSTEM-ARCHITECTURE.md#4-repository-layout)), toolchain pins ([20](20-DEPENDENCY-MANIFEST.md)), CI skeleton with fmt/clippy/lint/typecheck/test/trace-scan stubs, license-server greenfield with `/v1/health`, `npm run dev` one-command startup (R11), codegen pipeline skeleton, docs worklog initialized.

**Entry:** n/a (bootstrap).

**Tasks (selection; full task list is the phase's live checklist in `docs/phases/phase-0.md`):**
- `P0-001` monorepo init: pnpm workspaces, cargo workspace, `.cargo/config`, `rust-toolchain.toml`
- `P0-002` electron-vite app boots to placeholder screen (CSP locked, sandbox on — the hardening table asserted in CI from the first commit, [14 § 3](14-SECURITY-MODEL.md#3-electron-hardening-release-config-is-law--ci-asserts-every-line-of-this-table))
- `P0-003` `prism-core` crate compiles a `hello` napi command; codegen emits its TS types; drift check in CI
- `P0-004` license-server: Fastify + SQLite + `/v1/health` + dev keypair + seed script + `npm run dev` wiring (localhost:8080)
- `P0-005` CI: `ci.yml` full gate chain running on PRs; trace scanner v1 (wordlist from [00 § 4.3](00-INDEX.md#43-forbidden-vocabulary-in-shipped-artifacts))
- `P0-006` fixture generator v1 (`FIX-S`, `FIX-M`)

**Gate G0 (all must pass):**
- [ ] `npm run dev` starts server + app with one command; app hello-world talks to engine and server health
- [ ] CI green on a dummy PR touching all layers; every gate job present (fmt, clippy, test, lint, typecheck, vitest, codegen, trace, docs-links)
- [ ] `pnpm lic:admin issue` produces a key; server persists to SQLite; seeded demo keys present
- [ ] Fixtures reproduce byte-identically on two CI runs
- [ ] Persona pass: security hat signs the baseline (hardening table + secrets layout)

---

## Phase 1 — Rust engine core

**Scope:** standard scanner ([06 § 2](06-RUST-CORE.md#2-scanner--standard-strategy)), arena ([07 § 2](07-DATA-MODEL.md#2-the-arena-engine-side)), aggregation, IPC command/event surface ([05](05-IPC-PROTOCOL.md)), persistence schema, icon extraction, volume enumeration. **No UI beyond a debug console.**

**Entry:** G0 passed.

**Tasks (selection):** `P1-001` NtQueryDirectoryFile enumerator (+Extd class) with reusable buffers · `P1-002` worker topology + coordinator + cancellation · `P1-003` arena SoA + interning + CSR children + remap tables · `P1-004` correctness rules REPARSE/HARDLINK/SPARSE/PACKAGE/LONGPATH/DENIAL (each its own test module) · `P1-005` aggregation pass + leaderboards + age histogram · `P1-006` SQLite migrations + scans/settings tables · `P1-007` IPC: commands § 3.1–3.6 + event pump (SPSC → TSFN batcher) · `P1-008` icons via SHIL + cache · `P1-009` exclusion matcher · `P1-010` debug CLI (`cargo run -p prism-core --bin scan -- D:\` style harness) + criterion suites + FIX-L synthetic arena.

**Gate G1:**
- [ ] Scan FIX-M: all correctness tests green; **arena hash deterministic across 3 runs** ([06 § 12](06-RUST-CORE.md#12-testing-requirements-summary--full-strategy-in-17))
- [ ] Throughput ≥ 150k files/s on R1 (standard) — `bench-scan` gate green
- [ ] RSS ≤ 4 GB on FIX-L; `tree:children`/`node:detail`/`viz:layout` engine budgets met ([16 § 3](16-PERFORMANCE-BUDGETS.md#3-ipc--engine-query-budgets))
- [ ] Property tests: Σ-children invariants in all 3 size modes; event replay == final snapshot
- [ ] Fuzz corpus green for 24h-equivalent cycles on ntfs-free parsers (ipc decode, exclusion matcher)
- [ ] Persona passes: Rust hat (code review of hot loops + unsafe audit) + systems hat (lifetimes, cancellation, backpressure)
- [ ] Documentation: module docs complete; `docs/adr/` updated with any deviations (deviations require amendments per [00 § 6](00-INDEX.md#6-document-status-board))

---

## Phase 2 — App shell & design system

**Scope:** token pipeline + all six themes ([08](08-UI-DESIGN-SYSTEM.md)), owned shadcn base + primitives ([09 § 2.1](09-UI-COMPONENTS.md#21-primitives-owned-shadcnradix--customized)), AppShell/TopBar/Sidebar/Inspector chrome, Zustand stores skeleton, command palette, router-less screen switching, screenshot harness mode.

**Entry:** G0 passed (parallel with Phase 1 allowed; merge order still gated).

**Tasks (selection):** `P2-001` tokens pipeline (OKLCH → CSS vars + canvas palette + title bar) with CI checks ([08 § 12](08-UI-DESIGN-SYSTEM.md#12-accessibility--theming-enforcement-ci)) · `P2-002` theme JSONs ×6 · `P2-003` vendored shadcn primitives customized to spec · `P2-004` AppShell + TopBar + Sidebar + Inspector statics with all states · `P2-005` ByteFormat + micro-label + type chips atoms · `P2-006` command palette v1 (static commands) · `P2-007` screenshot harness `--screenshot=` + storybook + visual-regression CI job · `P2-008` density modes + keyboard scope manager.

**Gate G2:**
- [ ] Contrast/a11y/no-raw-colors gates green for all 6 themes
- [ ] Visual regression baselines locked for primitives + shell in Nocturne/Alabaster × 1x/2x
- [ ] Keyboard-only walkthrough of shell passes (focus visible everywhere, tab order documented)
- [ ] Renderer bundle guard green (≤ 8 MB gz)
- [ ] Persona pass: UI/UX hat pixel-reviews shell against [08]/[09]; findings triaged
- [ ] Screenshot harness produces every screen state deterministically on CI

---

## Phase 3 — Parity spine (the core experience)

**Scope:** Welcome → Scanning → Explore with FileTree + VizCanvas(treemap) + TypeList + selection sync + inspector + context actions + status strip + deletion semantics — the rows WDS-SEL/SCN/TRE/TMP/EXT/CTX/DEL.

**Entry:** G1 **and** G2 passed.

**Tasks (selection):** `P3-001` Welcome (drive cards, preflight, recents, folder picker) · `P3-002` scanning overlay (live tiles, counters, pause/resume/cancel) · `P3-003` FileTree virtualized + sort + multi-select + type-ahead · `P3-004` VizCanvas treemap (Canvas2D + cushion + LOD + hit grid + zoom frames) · `P3-005` WebGL2 treemap path + auto-switch probe · `P3-006` TypeList + color mapping + dim filtering · `P3-007` Inspector + Largest Inside + actions · `P3-008` context menus (WDS-CTX-01 set) + delete flows incl. ledger v1 (recycle/permanent, per-item errors) · `P3-009` status strip + settings persistence (WDS-CFG rows) · `P3-010` rescan subtree + full refresh + free-space/unknown nodes.

**Gate G3:**
- [ ] All WDS-SEL/SCN/TRE/TMP/EXT/CTX/DEL rows implemented and individually verified per [03 § 12](03-PORTING-MATRIX-WINDIRSTAT.md#12-verification--sign-off-protocol) (QA scripts recorded; statuses flipped in the matrix)
- [ ] Interaction budgets: hover ≤ 8 ms, selection sync ≤ 16 ms, 60 fps on R2 with FIX-L treemap ([16 § 5](16-PERFORMANCE-BUDGETS.md#5-viz-budgets-vizcanvas-r2))
- [ ] E2E: full user journey (pick drive → scan → explore → find biggest file → reveal → delete → tree updates) green on packaged build
- [ ] Visual: treemap golden images × backends × themes green
- [ ] Persona passes: UI/UX + Windows hats (DPI, localized OS, shell dialogs)
- [ ] No console errors/warnings in a scripted 30-minute soak session (Playwright)

---

## Phase 4 — Parity completion

**Scope:** remaining parity rows — the other DOM viz (table/bars/folders) + zoom/context/treemap options + extension editing + cleanup menu rows (empty bin, system temp) + i18n mechanism + drag in/out + errors drawer + about/updates UI + everything else in [03] not covered by G3.

**Entry:** G3 passed.

**Gate G4:**
- [ ] **Porting matrix at 100%** — every row `P` or `P+`; `npm run qa:parity` reports zero open; QA sign-off rows recorded in `docs/phases/phase-4-parity-signoffs.md`
- [ ] All X-divergences carry amendment links ([03 § 11](03-PORTING-MATRIX-WINDIRSTAT.md#11-deliberate-divergences-owner-approved-each-cites-its-rationale))
- [ ] i18n mechanism proven with `de-DE` (WDS-CHROME-07): pseudo-loc harness green, zero hardcoded strings
- [ ] Soak: FIX-M rescan loops × 10 + delete/undo cycles — no leaks (heap + handles audit)

---

## Phase 5 — Homegrown features

**Scope:** everything in [12], each behind engine entitlement gates, in this order (dependency-driven): Quick Wins → Cleanup presets + ledger full → Stale → Duplicates → Snapshots + diff → Applications + Leftovers → Monitor → Turbo scan (+elevation helper) → Scheduler → Exports → Preview pane → viz modes sunburst/icicle/pack/mindmap/age timeline (if not already in P5 track via parallelism) → palette + moments polish.

**Entry:** G4 passed (features may be built behind flags in parallel sessions once G3 passes, but **ship** only after G4).

**Gate G5:**
- [ ] Feature-per-feature verification tables complete (each [12] section's requirement IDs mapped to tests + QA scripts + screenshots)
- [ ] Premium commands all typestate-gated ([13 § 9](13-LICENSING-SYSTEM.md#9-enforcement-depth-engine-boundary)); unlicensed tab previews render per [10 § 12](10-SCREENS-AND-FLOWS.md#12-locked-tab-preview-conversion-surface)
- [ ] Turbo scan: ≥ 1M files/s on R1; elevation helper security review passed ([14 § 7](14-SECURITY-MODEL.md#7-elevation-helper-security-turbo-scan)); decline path = honest no-scan (no silent standard substitution, [ADR-06](04-SYSTEM-ARCHITECTURE.md#adr-06--two-first-class-scan-strategies-not-a-fallback))
- [ ] All 9 viz modes: golden frames + budgets + a11y summaries + interaction contracts verified
- [ ] Persona passes: all five hats over the feature set (esp. duplicates safety, uninstaller evidence, scheduler scope guard)

---

## Phase 6 — Licensing system

**Scope:** server endpoints + admin CLI + payments adapter (Dodo) + webhooks; client activation/heartbeat/grace/deactivation; engine token verification; trial; locked previews; E2E suites ([13](13-LICENSING-SYSTEM.md)).

**Entry:** G2 passed (can run parallel to 4/5; **gate order**: G6 signs after G5 so the tier matrix is verifiable against real features).

**Gate G6:**
- [ ] Full matrix of [13 § 11](13-LICENSING-SYSTEM.md#11-test-matrix-summary-full-suite-in-17) green against the real dev server
- [ ] Forge tests: bit-flipped tokens rejected by engine; replay rejected; clock-skew bounds honored
- [ ] Offline grace UX verified end-to-end (short-expiry dev tokens): countdown banner → locked tabs → heal
- [ ] Webhook idempotency + replay window verified; manual provider flow issues demo keys
- [ ] Checkout round trip in sandbox (or provider dry-run) produces a working key; email fulfillment path exercised or explicitly mocked at the boundary with a documented contract test
- [ ] Persona passes: security hat (full licensing threat model sign-off) + systems hat (server runbook)

---

## Phase 7 — Performance & polish

**Scope:** close every gap in [16]; cold-start work; memory sweeps; motion/interaction polish pass over every screen; empty-state/copy pass with the voice guide; reduced-motion + a11y sweep; crash-reporting pipeline (opt-in) verified.

**Entry:** G5 + G6 passed.

**Gate G7:**
- [ ] Every row of [16](16-PERFORMANCE-BUDGETS.md) green on R1/R2/R3 as applicable; `perf/budgets.json` gate job green on release branch
- [ ] Soak sessions (scripted, on FIX-L + real-machine drive of the QA host): 60-minute interaction soak — zero long tasks > 50 ms, zero renderer crashes, heap stable ±10%
- [ ] UI/UX hat final polish review with findings at S3-only severity remaining (S0–S2 = 0 open)
- [ ] Reduced-motion + keyboard-only + screen-reader summary flows verified per viz mode

---

## Phase 8 — Security & signing

**Scope:** [14] hardening sign-offs, code signing pipeline, updater chain (staged + rollback), threat-model updates, SBOM/provenance.

**Entry:** G7 passed.

**Gate G8:**
- [ ] All [14 § 3](14-SECURITY-MODEL.md#3-electron-hardening-release-config-is-law--ci-asserts-every-line-of-this-table) assertions verified on the **packaged** app (not dev)
- [ ] Signed builds: every artifact Authenticode-verified; unsigned-artifact CI step fails correctly on a deliberately unsigned test build
- [ ] Updater: update → verify → staged → health-check → rollback drill executed on CI + manually once on R2/R3; downgrade attack rejected test
- [ ] Security persona final report with zero open S0/S1 findings; S2 findings scheduled or waived with owner sign-off
- [ ] Trace scanner green on final artifacts; SBOM + provenance published to the release draft

---

## Phase 9 — Packaging & release channels

**Scope:** NSIS + portable finalization, delta updates, channel feeds (stable/beta), website download integration, runbooks (rollback, incident, support).

**Entry:** G8 passed.

**Gate G9:**
- [ ] Per-user + per-machine installs clean on: Win10 21H2, Win11 24H2, x64 + arm64 (fresh + upgrade-from-previous + uninstall-keep-data + uninstall-purge)
- [ ] Beta channel opt-in round trip; delta update applied and verified on both arches
- [ ] Shell integration (context menu) install/uninstall verified per mode ([WDS-CTX-04](03-PORTING-MATRIX-WINDIRSTAT.md#6-selection-clipboard-integration))
- [ ] Runbooks documented + rehearsed once (rollback drill recorded)

---

## Phase 10 — General Availability

**Entry:** G9 passed.

**Gate G10 (release):**
- [ ] [01 § 7](01-PRODUCT-VISION.md#7-success-criteria-v1-ga-definition) success criteria S1–S7 all evidenced (parity dashboard, perf dashboard, E2E, crash pipeline live, usability script results, licensing E2E, trace scan)
- [ ] Release notes + About + EULA + privacy text final; owner sign-off recorded in `docs/amendments/ga-signoff.md`
- [ ] Update feed serves the GA build; website links live; support inbox + admin runbook ready

---

## Session records

Every working session appends to `docs/worklog.md` (append-only, [19 § Worklog](19-AGENT-PROTOCOL.md#worklog)):

```markdown
## Session <n> — <phase> — <date-optional>
Agent: <id/human>
Tasks: P<n>-<nnn> (+ unplanned work flagged)
Done: <bulleted evidence: tests, screenshots, commands run>
Deferred: <items moved, with reason and new home>
Risks/Notes: <open questions, doc amendments needed>
Next: <suggested next task IDs>
```

Phase checklists live in `docs/phases/phase-<n>.md` and are updated by session authors as boxes complete; **only gate reviews (persona passes + owner) may check gate boxes** — implementers check task boxes only.

## Change control

Mid-phase scope additions require: an amendment doc (`docs/amendments/`), an updated task list, and (if they touch contracts) updated specs in the same change. "Scope creep by PR description" is prohibited and reverted on review ([19 A6](19-AGENT-PROTOCOL.md#a6--scope-discipline)).
