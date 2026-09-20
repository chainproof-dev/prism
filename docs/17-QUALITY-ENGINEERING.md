# 17 — Quality Engineering

> Owner doc: [00-INDEX.md](00-INDEX.md) · Upstream: all spec docs · Downstream: [18-PHASE-PLAN](18-PHASE-PLAN.md), [19-AGENT-PROTOCOL](19-AGENT-PROTOCOL.md).
> Requirement IDs: `PRISM-QA-*`.

---

## 1. Quality bar (non-negotiable)

A task is done when it is **specified, implemented, tested, measured, reviewed, and documented** — all six, always. "Works on my machine" is not a state that exists in this project. The bar is production software that a paying customer runs on their only machine, on their real data, and we don't get to see the failure.

## 2. Test strategy (the pyramid, concretely)

| Layer | Tooling | Required per change |
|---|---|---|
| Rust unit | `cargo test`, table-driven, fixture VHDs (junctions/hardlinks/sparse/long-path/denial ACLs scripted) | every correctness rule ([06 § 2.3](06-RUST-CORE.md#23-correctness-rules-each--unit-test-module)) touched |
| Rust property | `proptest` invariants ([07 § 6](07-DATA-MODEL.md#6-invariants-property-tested-prism-dm-030)) | arena/aggregation changes |
| Rust fuzz | `cargo-fuzz` on `prism-ntfs` parsers + IPC decode | parser changes; nightly job keeps corpora green |
| TS unit | vitest, jsdom | every component/store/service with behavior |
| Integration (engine) | Rust harness driving `ipc` facade + golden VizFrame hashes | layout/scanner/ipc changes |
| E2E | Playwright **against the packaged app** (`electron-builder --dir` artifact, not just dev server) | every user flow row in [10](10-SCREENS-AND-FLOWS.md) has at least one E2E; parity QA scripts run scripted versions |
| Visual regression | screenshot harness (`--screenshot=<screen>` app mode — the technique proven by our intel target, [02 § 2](02-INTEL-DISKBUDDY.md#2-confirmed-technology-stack-and-what-it-implies)) + Playwright snapshots | every UI component + every viz mode × 2 themes × 2 DPR |
| Performance | bench suites + CI perf gates ([16 § 7](16-PERFORMANCE-BUDGETS.md#7-measurement--enforcement)) | any change to hot paths |
| Licensing E2E | mock + real dev server lifecycle suite ([13 § 11](13-LICENSING-SYSTEM.md#11-test-matrix-summary-full-suite-in-17)) | licensing changes |
| A11y walkthroughs | manual keyboard/script walkthroughs per screen | UI changes |

**Coverage policy:** line coverage is a *diagnostic*, not a target; **requirement coverage is the target** — every requirement ID (`PRISM-*`, `WDS-*`) must map to ≥ 1 test or QA script; a coverage report of requirement IDs is generated per release (`npm run qa:coverage`) and is part of the GA gate.

### Visual regression

The visual layer runs on the app's own screenshot harness (`--screenshot=<screen>:<state>`, headless, deterministic fixtures + frozen theme + fixed DPI) — the same technique the competitive target used for its activation-screen shots ([02 § 2](02-INTEL-DISKBUDDY.md#2-confirmed-technology-stack-and-what-it-implies)) — plus Playwright snapshot assertions for component states. Baselines are committed per (component × theme-pair × DPR); pixel-diff tolerance ≤ 0.1% area; baseline updates require an intended-change note in the PR ([19 A8](19-AGENT-PROTOCOL.md#a8--verification-before-hand-off-definition-of-done)).

## 3. CI gates (PR-blocking)

1. `cargo fmt --check`, `cargo clippy -- -D warnings` (workspace, with the deny-unwrap config), `cargo test`, `cargo deny check` (licenses/advisories), fuzz smoke (60 s per target).
2. `pnpm typecheck` (strict, no `any`), `eslint` (with `no-raw-colors`, `no-literal-strings` in components, `no-restricted-imports` enforcing the source-purity rules [09 § 1](09-UI-COMPONENTS.md#1-sourcing-ratios-owner-mandated-r5)), `vitest run`, Playwright (smoke suite on PR, full on main).
3. Codegen drift check ([05 § 6](05-IPC-PROTOCOL.md#6-type-codegen)) — generated TS must match Rust.
4. Token pipeline checks ([08 § 12](08-UI-DESIGN-SYSTEM.md#12-accessibility--theming-enforcement-ci)): contrast, focus rings, no raw colors, theme key completeness.
5. **Trace scanner** (see § 4).
6. Bundle guards: renderer bundle ≤ 8 MB gzipped (warn), main process load time ≤ 400 ms, no dynamic remote imports (CSP test).
7. Perf-diff bot comment on hot-path PRs; hard-budget regressions fail ([16 § 7](16-PERFORMANCE-BUDGETS.md#7-measurement--enforcement)).
8. Docs link-check across this build plan (dead cross-links fail).

## 4. Trace scanner (`PRISM-QA-010`)

A CI job + release job scanning **every shipped artifact** (unpacked asar, native `.node`, installer contents, resources, generated config) for the forbidden vocabulary ([00 § 4.3](00-INDEX.md#43-forbidden-vocabulary-in-shipped-artifacts)) with fuzzy matching (edit distance ≤ 2 on tokens ≥ 6 chars, case-folded, leetspeak-normalized). Matches fail the build with a context dump. The wordlist lives in `script/trace-scan/wordlist.txt` and is append-only (adding words never requires history edits).

## 5. Review standards

- **Every PR:** one reviewer minimum; two for hot paths (scanner/arena/layout/ipc/licensing/security). PR template: requirement IDs touched, tests added, perf impact, screenshots for UI, threat-model diff if security-relevant ([14 § 9](14-SECURITY-MODEL.md#9-threat-model-reviews)).
- **Rust review checklist** (maintained in `.github/PULL_REQUEST_TEMPLATE`): error handling (`Result` everywhere, no unwrap), unsafe blocks have `// SAFETY:`, no allocation in hot loops without justification, channels bounded, thread lifetimes explicit, `Send`/`Sync` reasoning stated for new shared types.
- **TS/React checklist:** no `any`/`as unknown as`, stores per architecture ([10 § 15](10-SCREENS-AND-FLOWS.md#15-stores-state-architecture)), virtualization rule ([09 § 3](09-UI-COMPONENTS.md#3-component-engineering-rules)), a11y contract honored, tokens not hex.
- **Small PRs are law:** ≤ 400 changed lines of intent per PR (generated files excluded); bigger changes are split by the author *before* review.

## 6. Persona reviews (`PRISM-QA-020`)

At each phase gate (and on-demand for risky work), a dedicated review pass runs with these hats (one reviewer each, findings logged in the phase record):

| Persona | Asks |
|---|---|
| Senior systems dev (25y) | Failure modes, resource lifetimes, concurrency, error propagation, upgrade/migration paths |
| Senior security researcher (25y) | [14 § 2](14-SECURITY-MODEL.md#2-assets--adversaries) table walk, new attack surface, secret hygiene, parser safety |
| Senior UI/UX designer (25y) | Specs vs [08](08-UI-DESIGN-SYSTEM.md)/[09](09-UI-COMPONENTS.md)/[10](10-SCREENS-AND-FLOWS.md) pixel review on real screens, empty/error states, motion restraint, voice |
| Senior Windows dev (25y) | NTFS edge cases, UAC/elevation, DPI, shell integration, long paths, localized OS, upgrade paths from older Windows |
| Senior Rust dev (25y) | Idioms, unsafe policy, bounds checks in hot loops, allocator behavior, API surface, doc quality |

Findings are triaged: blockers fix before gate; non-blockers become phase tasks with IDs. No persona pass may be skipped at a gate — the gate checklist includes their sign-offs.

## 7. Bug taxonomy & severity (drives fix ordering)

| Sev | Meaning | Response |
|---|---|---|
| S0 | data loss, wrong sizes shown as truth, crash on common paths, security | hotfix channel; all hands |
| S1 | feature broken for a common persona; perf budget blown > 2× | fix before any feature work continues |
| S2 | broken edge case, budget regression ≤ 2× | scheduled in phase |
| S3 | polish, copy, minor a11y | backlog |

**Wrong numbers are S0** (never lie about bytes, [01 § 5.3](01-PRODUCT-VISION.md#5-product-principles-decision-tie-breakers)).

## 8. Documentation standard (this repo)

Every doc change: technical, specific, numbered requirements; no filler adjectives; cross-links valid (CI); diagrams as ASCII in-doc or generated to `docs/assets/`. Each phase appends its session records ([18 § Session Records](18-PHASE-PLAN.md#session-records)). The **worklog** (`docs/worklog.md`) is append-only and the first thing a new session reads ([19 § Session Bootstrap](19-AGENT-PROTOCOL.md#session-bootstrap-mandatory-every-session)).
