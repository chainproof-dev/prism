# PRISM — Windows Disk Analyzer · Complete Build Plan Architecture

**Stack:** Electron 44 · React 19 · TypeScript 5.9 · Vite 8 · Tailwind 4 · shadcn/ui (owned) · Radix · Lucide · TanStack Table 9 · Recharts 3 · Rust 1.98 core engine (napi-rs v3, in-process) · Fastify license server.

**What this is:** the complete, cross-linked build plan for a **paid, production-grade Windows disk analyzer** — a 100% behavioral-coverage port of the reference open-source tool, re-implemented clean-room in Rust + TypeScript, extended with homegrown premium features, a licensing system (yearly + lifetime), and a bespoke premium UI/UX design system. Written to be executed **by AI build agents working in sessions**, with strict phase gates and a hard agent protocol.

## Start here

| If you are… | Read first |
|---|---|
| A build agent starting a session | `00-INDEX.md` → `19-AGENT-PROTOCOL.md` → `18-PHASE-PLAN.md` → your task's spec docs |
| The owner / a reviewer | `00-INDEX.md` (traceability table) → `01-PRODUCT-VISION.md` → `18-PHASE-PLAN.md` |
| UI/UX | `08` → `09` → `10` → `11` |
| Engine / Rust | `06` → `07` → `05` |
| Licensing / security | `13` → `14` |

## Document map

```
00-INDEX                        master index · conventions · requirement traceability
01-PRODUCT-VISION               what & why · personas · value props · success criteria
02-INTEL-DISKBUDDY              competitive reverse-engineering report (full findings)
03-PORTING-MATRIX-WINDIRSTAT    the 100% parity contract (row-by-row, verifiable)
04-SYSTEM-ARCHITECTURE          process model · ADRs · dataflow · failure domains
05-IPC-PROTOCOL                 renderer⇄main⇄engine contract · binary viz frames · codegen
06-RUST-CORE                    scanner (standard + raw-NTFS turbo) · arena · Windows rules
07-DATA-MODEL                   columnar arena · SQLite schema · snapshots · windowing
08-UI-DESIGN-SYSTEM             precision-instrument identity · 6 themes · data spectrum
09-UI-COMPONENTS                component library · sourcing ratios · specs
10-SCREENS-AND-FLOWS            every screen/state/flow · keyboard map · cleanup safety
11-VISUALIZATION-ENGINE         9 modes · algorithms · LOD · rendering · budgets
12-HOMEGROWN-FEATURES           quick wins · presets · duplicates · snapshots · uninstaller…
13-LICENSING-SYSTEM             server + client · Ed25519 entitlements · demo key :8080
14-SECURITY-MODEL               threat model · Electron hardening · updater chain
15-BUILD-PACKAGING              build pipeline · NSIS · signing · release channels
16-PERFORMANCE-BUDGETS          hard numbers, enforced in CI
17-QUALITY-ENGINEERING          test strategy · CI gates · persona reviews · trace scanner
18-PHASE-PLAN                   strict phases 0–10 with gates (sessions only — no calendar)
19-AGENT-PROTOCOL               rules of engagement (no-fallbacks law, DoD, worklog)
20-DEPENDENCY-MANIFEST          pinned latest versions · license audit · upgrade policy
21-GLOSSARY                     shared vocabulary
```

All cross-links are audited (602 links, 0 broken). Docs are frozen as **v1.0 baseline**; changes go through `docs/amendments/`.

## Core rules (short form — full form in 19-AGENT-PROTOCOL)

1. **No fallbacks.** Fail loud, degrade visibly, or block honestly — never silently substitute.
2. **Latest pinned dependencies** only (manifest is the single pin list).
3. **Production-grade only:** no unwrap/panic, typed errors end-to-end, tests per requirement ID.
4. **60 fps floor / 120-ready UI;** budgets in `16` are CI-enforced contracts.
5. **100% parity before GA** (`03` matrix at 100%), zero-trace naming enforced by CI scanner.
6. **Sessions & gates, never calendar estimates.**
7. **Worklog is sacred** — every session appends; context survives hand-offs.
