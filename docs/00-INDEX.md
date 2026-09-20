# PRISM — Build Plan Architecture · Master Index

> **Codename:** `Prism` (internal). Final product name is decided by the owner and MUST NOT echo "WinDirStat", "DiskBuddy", "KDirStat", " QDirStat", "WizTree", "TreeSize", or any competitor name — in code, UI copy, telemetry, domains, bundle IDs, or marketing. See [01-PRODUCT-VISION § Naming Rules](01-PRODUCT-VISION.md#9-naming-rules).
>
> **Status:** v1.0 — frozen baseline. Changes require a written amendment in `docs/amendments/` and a re-issue of this index.
>
> **Audience:** AI build agents and human reviewers operating in sessions. Every agent reads [19-AGENT-PROTOCOL.md](19-AGENT-PROTOCOL.md) **before writing any code**, and the phase entry for the current phase in [18-PHASE-PLAN.md](18-PHASE-PLAN.md) before accepting any task.

---

## 1. What this document set is

This is the complete, authoritative build plan for a **commercial, paid Windows desktop disk analyzer** — a ground-up product built on **Electron + React + TypeScript + Vite for the UI, and Rust for the core engine**, connected via an in-process N-API bridge. The product's functional baseline is a **100% coverage port** of the reference open-source tool (tracked exhaustively in [03-PORTING-MATRIX-WINDIRSTAT.md](03-PORTING-MATRIX-WINDIRSTAT.md)), extended with a large set of homegrown, monetizable features (tracked in [12-HOMEGROWN-FEATURES.md](12-HOMEGROWN-FEATURES.md)), a licensing system ([13-LICENSING-SYSTEM.md](13-LICENSING-SYSTEM.md)), and a premium UI/UX layer ([08](08-UI-DESIGN-SYSTEM.md), [09](09-UI-COMPONENTS.md), [10](10-SCREENS-AND-FLOWS.md), [11](11-VISUALIZATION-ENGINE.md)).

Two reference products were studied during research. Their names appear **only inside this documentation set** (for engineering traceability) and are **forbidden vocabulary in all shipped artifacts**:

| Reference | Role in this plan | Where it is analyzed |
|---|---|---|
| WinDirStat (open source, GPL-2.0, v2.6.2) | **Functional parity baseline.** 100% of its observable behavior must be covered — re-implemented, never copied. | [03-PORTING-MATRIX-WINDIRSTAT.md](03-PORTING-MATRIX-WINDIRSTAT.md) |
| DiskBuddy (commercial macOS app, v1.0.0, arm64) | **Competitive intelligence.** Binary was reverse-engineered (DMG extracted, HFS+ parsed, 10,014 Swift symbols cataloged, UI screenshots analyzed by vision model). We learn its product decisions; we do not copy its assets, code, names, or exact visual identity. | [02-INTEL-DISKBUDDY.md](02-INTEL-DISKBUDDY.md) |

**Legal posture:** WinDirStat is GPL-2.0. We are building a *clean-room behavioral re-implementation*: no source code, no assets, no strings, no resource files from WinDirStat are ever copied into this project. Behaviors (treemap visualization, dual-pane sync, extension statistics) are functional ideas, not copyrightable expression, and are re-implemented from scratch in Rust/TypeScript. Every contributor must follow the clean-room protocol in [03-PORTING-MATRIX-WINDIRSTAT.md § Clean-Room Protocol](03-PORTING-MATRIX-WINDIRSTAT.md#0-clean-room-protocol). DiskBuddy is closed commercial software; its extracted binary and screenshots live only in the research workspace and never enter the product repository.

---

## 2. Document map & reading order

Agents follow this reading order on session start (see [19-AGENT-PROTOCOL § Session Bootstrap](19-AGENT-PROTOCOL.md#session-bootstrap-mandatory-every-session)):

| # | Document | Purpose | Required for |
|---|---|---|---|
| 00 | This index | Navigation, traceability, conventions | Everyone, always |
| 01 | [PRODUCT-VISION](01-PRODUCT-VISION.md) | What we are building, for whom, why anyone pays | Everyone |
| 02 | [INTEL-DISKBUDDY](02-INTEL-DISKBUDDY.md) | Reverse-engineering findings & product lessons | Feature design, UI/UX, licensing |
| 03 | [PORTING-MATRIX-WINDIRSTAT](03-PORTING-MATRIX-WINDIRSTAT.md) | The 100% parity contract (item-by-item) | Engine, UI, QA |
| 04 | [SYSTEM-ARCHITECTURE](04-SYSTEM-ARCHITECTURE.md) | Process model, layers, dataflow, failure domains | Everyone |
| 05 | [IPC-PROTOCOL](05-IPC-PROTOCOL.md) | N-API bridge, event streaming, type contracts | Rust + main-process TS |
| 06 | [RUST-CORE](06-RUST-CORE.md) | Scanner, aggregation, Windows integration, error strategy | Rust engineers |
| 07 | [DATA-MODEL](07-DATA-MODEL.md) | Arena, columnar store, snapshots, persistence schema | Rust + renderer |
| 08 | [UI-DESIGN-SYSTEM](08-UI-DESIGN-SYSTEM.md) | Tokens, themes, typography, color, motion | All UI work |
| 09 | [UI-COMPONENTS](09-UI-COMPONENTS.md) | Component library spec (every component, props, states) | All UI work |
| 10 | [SCREENS-AND-FLOWS](10-SCREENS-AND-FLOWS.md) | Every screen, every state, every flow, keyboard map | All UI work |
| 11 | [VISUALIZATION-ENGINE](11-VISUALIZATION-ENGINE.md) | All 9 visualization modes: algorithms + rendering + perf | Viz engineers |
| 12 | [HOMEGROWN-FEATURES](12-HOMEGROWN-FEATURES.md) | Differentiators beyond parity (spec per feature) | Features, licensing tiers |
| 13 | [LICENSING-SYSTEM](13-LICENSING-SYSTEM.md) | License server, client enforcement, demo key, activation UX | Licensing + security |
| 14 | [SECURITY-MODEL](14-SECURITY-MODEL.md) | Threat model, signing, updater, hardening | Security, release |
| 15 | [BUILD-PACKAGING](15-BUILD-PACKAGING.md) | Build pipeline, installer, auto-update, channels | Release engineering |
| 16 | [PERFORMANCE-BUDGETS](16-PERFORMANCE-BUDGETS.md) | Hard numbers & how they are enforced | Everyone |
| 17 | [QUALITY-ENGINEERING](17-QUALITY-ENGINEERING.md) | Test strategy, CI gates, review standards, personas | Everyone |
| 18 | [PHASE-PLAN](18-PHASE-PLAN.md) | Strict phases, entry/exit gates (no calendar dates — sessions only) | Task assignment |
| 19 | [AGENT-PROTOCOL](19-AGENT-PROTOCOL.md) | Rules of engagement for build agents | Everyone, always |
| 20 | [DEPENDENCY-MANIFEST](20-DEPENDENCY-MANIFEST.md) | Pinned versions & selection rationale | Setup, upgrades |
| 21 | [GLOSSARY](21-GLOSSARY.md) | Shared vocabulary | Reference |

---

## 3. Requirement traceability

Every hard requirement from the owner maps to a document section. **If a row here cannot be shown a home, the plan is incomplete — add one before proceeding.**

| # | Owner requirement | Where it is satisfied |
|---|---|---|
| R1 | Windows disk analyzer, paid product | [01-PRODUCT-VISION](01-PRODUCT-VISION.md), [13-LICENSING-SYSTEM](13-LICENSING-SYSTEM.md) |
| R2 | Base functional parity with the GPL reference tool, 100% coverage, no gaps | [03-PORTING-MATRIX-WINDIRSTAT.md](03-PORTING-MATRIX-WINDIRSTAT.md) (the authoritative contract; QA sign-off is per-row) |
| R3 | Core engine rewritten in Rust (memory + performance advantage) | [06-RUST-CORE.md](06-RUST-CORE.md), [07-DATA-MODEL.md](07-DATA-MODEL.md) |
| R4 | UI/UX stack: Electron + React + TypeScript + Vite (replacing the earlier Slint direction) | [04-SYSTEM-ARCHITECTURE.md](04-SYSTEM-ARCHITECTURE.md), [20-DEPENDENCY-MANIFEST.md](20-DEPENDENCY-MANIFEST.md) |
| R5 | Component system: shadcn/ui foundation (~70–80%), Radix primitives, Lucide icons, Tailwind; selective premium/animated components (~10–20%); bespoke viz/UI remainder | [09-UI-COMPONENTS.md § Sourcing Ratios](09-UI-COMPONENTS.md#1-sourcing-ratios-owner-mandated-r5), [20-DEPENDENCY-MANIFEST.md](20-DEPENDENCY-MANIFEST.md) |
| R6 | TanStack Table for large file lists; Recharts for statistics charts | [09-UI-COMPONENTS.md](09-UI-COMPONENTS.md) (FileTable, StatCards), [20-DEPENDENCY-MANIFEST.md](20-DEPENDENCY-MANIFEST.md) |
| R7 | Learn from DiskBuddy via reverse engineering; borrow ideas (not code/names/visual identity) | [02-INTEL-DISKBUDDY.md](02-INTEL-DISKBUDDY.md) + per-feature borrowings in [12-HOMEGROWN-FEATURES.md](12-HOMEGROWN-FEATURES.md) |
| R8 | Zero-trace naming: no reference to either product anywhere in shipped artifacts | [01-PRODUCT-VISION § Naming Rules](01-PRODUCT-VISION.md#9-naming-rules), [17-QUALITY-ENGINEERING § Trace Scanner](17-QUALITY-ENGINEERING.md#4-trace-scanner-prism-qa-010) |
| R9 | Unique value beyond the free alternative (why pay?) | [01-PRODUCT-VISION § Value Proposition](01-PRODUCT-VISION.md#4-value-proposition--why-pay-when-the-baseline-tool-is-free), [12-HOMEGROWN-FEATURES.md](12-HOMEGROWN-FEATURES.md) |
| R10 | Licensing: yearly license + lifetime license — exactly two SKUs | [13-LICENSING-SYSTEM.md](13-LICENSING-SYSTEM.md) |
| R11 | First build runs the license server on `localhost:8080`, one-command start (`npm run dev`), one demo license key for testing; same URL/port wired into client config | [13-LICENSING-SYSTEM.md § Local Development](13-LICENSING-SYSTEM.md#10-local-development--demo-key-owner-requirement-r11) |
| R12 | Phased execution, **no calendar time units anywhere** (no weeks/months/dates) — sessions only | [18-PHASE-PLAN.md](18-PHASE-PLAN.md) (convention enforced doc-wide) |
| R13 | Strict agent instructions: no fallbacks, latest dependencies, production-grade quality, senior-Rust patterns, zero-lag responsive UI | [19-AGENT-PROTOCOL.md](19-AGENT-PROTOCOL.md) (rules A1–A7), [16-PERFORMANCE-BUDGETS.md](16-PERFORMANCE-BUDGETS.md), [20-DEPENDENCY-MANIFEST.md](20-DEPENDENCY-MANIFEST.md) |
| R14 | 100% parity **plus** homegrown features from competitive intel + real user needs | [03](03-PORTING-MATRIX-WINDIRSTAT.md) + [12](12-HOMEGROWN-FEATURES.md) |
| R15 | Technical, deep, non-generic documentation; no filler | Style enforced by [17-QUALITY-ENGINEERING § Documentation Standard](17-QUALITY-ENGINEERING.md#8-documentation-standard-this-repo) |
| R16 | Peak UI/UX attention: premium, polished, distinctive themes/colors (no generic gradients/palettes), senior-designer-level detail | [08-UI-DESIGN-SYSTEM.md](08-UI-DESIGN-SYSTEM.md), [09](09-UI-COMPONENTS.md), [10](10-SCREENS-AND-FLOWS.md), [11](11-VISUALIZATION-ENGINE.md) |
| R17 | Multi-POV quality reviews (senior dev, security researcher, UI/UX designer, Windows dev, Rust dev — 25+ years each) run periodically | [17-QUALITY-ENGINEERING § Persona Reviews](17-QUALITY-ENGINEERING.md#6-persona-reviews-prism-qa-020) |
| R18 | Worklog maintained across sessions; everything saved to avoid progress loss | [19-AGENT-PROTOCOL § Worklog](19-AGENT-PROTOCOL.md#worklog), [18-PHASE-PLAN § Session Records](18-PHASE-PLAN.md#session-records) |
| R19 | Full production architecture: updater, licensing, packaging, signing, CI | [14](14-SECURITY-MODEL.md), [15](15-BUILD-PACKAGING.md) |
| R20 | Monorepo covering app + license server, fully documented, zipped deliverable | [04 § Repository Layout](04-SYSTEM-ARCHITECTURE.md#4-repository-layout), [15](15-BUILD-PACKAGING.md) |

---

## 4. Conventions used across all documents

### 4.1 Normative language

- **MUST / MUST NOT** — non-negotiable. A PR violating these is rejected without discussion.
- **SHOULD** — required unless a documented, approved reason exists in the PR description.
- **MAY** — genuinely optional.
- Requirement IDs: `PRISM-<AREA>-<n>` (e.g. `PRISM-IPC-014`) are stable and cited in code review, tests, and commit messages.

### 4.2 Technical conventions

| Topic | Convention |
|---|---|
| Language of docs | English, technical, imperative. No filler adjectives ("robust", "blazing") without a number attached. |
| Units of time in plan docs | **Phases and sessions only.** The strings `week`, `month`, `day`, `quarter`, and any dates are forbidden in [18-PHASE-PLAN.md](18-PHASE-PLAN.md) and in worklog estimates. Work is measured in *sessions* and *gates*. |
| Rust | Edition 2024, toolchain pinned in `rust-toolchain.toml` (see [20-DEPENDENCY-MANIFEST](20-DEPENDENCY-MANIFEST.md)). `#![deny(clippy::unwrap_used)]` in `src/` (integration tests may unwrap). |
| TypeScript | `strict: true`, `noUncheckedIndexedAccess: true`, `exactOptionalPropertyTypes: true`. `any` is forbidden; `unknown` + narrowing is the pattern. |
| Sizes | All byte counts are `u64`. Sizes are formatted per [09-UI-COMPONENTS § ByteFormat](09-UI-COMPONENTS.md#byteformat) (SI decimals with binary toggle). |
| IDs | Node IDs are `u32` indices into the arena ([07-DATA-MODEL](07-DATA-MODEL.md)). They are only valid within one `ScanId`. |
| IPC names | `domain:action` kebab-free style, e.g. `scan:start`, `tree:children`, `viz:layout` ([05-IPC-PROTOCOL](05-IPC-PROTOCOL.md)). |
| Errors | Rust: `thiserror` enums per crate module; `anyhow` only at CLI/bin edges. TS: discriminated unions mirroring Rust variants — never `string` errors. |
| Testing | Every doc's requirements are testable; tests cite requirement IDs in comments (`// PRISM-PERF-003`). |
| Cross-links | Relative markdown links only (`04-SYSTEM-ARCHITECTURE.md#anchors`). Keep them valid — CI link-checks (see [17-QUALITY-ENGINEERING](17-QUALITY-ENGINEERING.md)). |

### 4.3 Forbidden vocabulary in shipped artifacts

The strings below (case-insensitive, including near-miss spellings and leetspeak) MUST NOT appear in any repository file outside `docs/` research documents, and never in any compiled binary, resource, installer, domain, or UI string. CI enforces with a trace scanner ([17-QUALITY-ENGINEERING § Trace Scanner](17-QUALITY-ENGINEERING.md#4-trace-scanner-prism-qa-010)):

```
windirstat, wds, kdirstat, qdirstat, filelight, diskbuddy, disk buddy,
wiztree, treesize, spacesniffer, dirstat
```

Internal codename `Prism` / `prism-core` is used in repos and docs. The **shipping name** is a separate owner decision recorded later in `docs/amendments/naming.md`.

---

## 5. How to use this plan in a build session

1. **Session bootstrap** (mandatory, ~10 minutes): read [19-AGENT-PROTOCOL.md](19-AGENT-PROTOCOL.md) → read the current phase's entry/exit criteria in [18-PHASE-PLAN.md](18-PHASE-PLAN.md) → read `worklog.md` tail (last 3 sessions) → read the specific docs your task touches (map above).
2. **Claim a task** only from the current phase's task list. Tasks have stable IDs (`P3-017`). One task = one branch = one PR.
3. **Implement** following the relevant spec docs. When two docs conflict, the more specific one wins; if still ambiguous, stop and file a `docs/amendments/` question — do **not** guess.
4. **Verify** against the task's definition-of-done checklist ([19-AGENT-PROTOCOL § Definition of Done](19-AGENT-PROTOCOL.md#a8--verification-before-hand-off-definition-of-done)), including the multi-persona review when the phase gate requires it.
5. **Record** the session in `worklog.md` and update the phase checklist in `18-PHASE-PLAN.md` if you completed a gate item.

---

## 6. Document status board

| Doc | Status | Last reviewed |
|---|---|---|
| 00–21 | Frozen v1.0 baseline | Initial issue |

Amendments live in `docs/amendments/AMM-<n>-<slug>.md` and update the status board above in the same PR.
