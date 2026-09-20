# 01 — Product Vision

> Owner doc: [00-INDEX.md](00-INDEX.md) · Upstream: none · Downstream: all documents.
> Requirement IDs defined here: `PRISM-VIS-*`.

---

## 1. One-paragraph definition

**Prism** (codename) is a **paid, native-feeling Windows disk space analyzer**: a Rust engine that scans drives at speeds the open-source alternatives cannot reach, wrapped in an Electron + React interface that feels like a premium consumer product — multiple best-in-class visualizations, guided cleanup, duplicate detection, snapshots, and an integrated license system with yearly and lifetime tiers. Everything runs locally; nothing about a user's files ever leaves the machine.

## 2. The problem, precisely

Windows users discover "C: is full" in one of three moments, all of them bad:

1. **The upgrade wall** — a 200 GB SSD is full; Windows shows a red bar and a "Storage settings" screen that lists top-level folders by *logical size only*, with no drill-down beyond two levels, no allocated-size accounting, no awareness of hard links, and no cleanup beyond "empty recycle bin".
2. **The developer workstation** — `node_modules`, `target\`, `.gradle`, `Packages`, `obj\bin`, container images, VM disks, and simulator images silently consume hundreds of GB. Each is safe to delete and expensive to find manually.
3. **The creeping mystery** — storage decays over months; nobody knows which 40 GB appeared since last quarter, or which files haven't been touched in two years.

The free reference tool solves the *measurement* half of this (treemap + tree + extension list) but: it is slow on modern NVMe-scale volumes (single-threaded enumeration with a dated MFC UI), it has no notion of *what to do next* (no cleanup guidance, no duplicates, no history), it miscounts hard links, and its visual design reads as 2005 shareware. Users who live in this problem weekly will pay for: speed (10× faster scans), guidance (a ranked, explainable list of what to reclaim), history (what changed since last time), and a UI they don't have to apologize for.

## 3. Target users (personas used throughout the docs)

| Persona | Profile | Primary jobs-to-be-done | Will pay because |
|---|---|---|---|
| **Dev Priya** | Senior developer, 2 TB NVMe, 30+ projects, Docker, WSL2 | Reclaim space fast during work; find `node_modules`/`target`/container bloat; compare before/after big builds | Time. A 4-minute scan that finds 180 GB of regenerable build artifacts pays for itself the first session. |
| **Creator Marco** | Video editor / photographer, multi-TB spinning disk + SSD | Find stale project media, duplicates from imports, old cache folders | Safety + guidance. Wants ranked, explainable reclaim candidates, not raw trees. |
| **IT Sam** | Manages 10–50 Windows machines (family, small office) | Periodic audits; uninstall apps cleanly with leftover removal; export reports | Accountability: CSV/JSON exports, snapshots, repeatable cleanup. |
| **Power Nina** | The WinDirStat power user | Everything the old tool did — dual-pane sync, extension stats, deep treemap | 100% parity **plus** the extras; she defects only if parity is genuinely complete. |

**Non-users (explicit):** enterprise fleet management with central reporting (out of scope for v1 — no server-side aggregation of user data, ever); macOS/Linux clients (Windows only for v1; the architecture keeps a port path open but does not spend on it — see [06-RUST-CORE § Platform Boundary](06-RUST-CORE.md#platform-boundary)).

## 4. Value proposition — why pay when the baseline tool is free?

The free tool measures. **Prism decides.** Concretely, the paid value stack:

1. **Speed (`PRISM-VIS-001`)** — Rust parallel scanner with raw-NTFS turbo mode: a full 2 TB / 8-million-file volume scanned in seconds-to-tens-of-seconds, not minutes. Budget: [16-PERFORMANCE-BUDGETS § Scan Budgets](16-PERFORMANCE-BUDGETS.md#2-scan-budgets). The free tool is single-threaded directory enumeration.
2. **Truth (`PRISM-VIS-002`)** — allocated (on-disk) size, logical size, hard-link-aware unique bytes, sparse-file awareness, per-volume cluster-size compensation. The free tool double-counts hard links and confuses sparse logical size with disk usage.
3. **Guidance (`PRISM-VIS-003`)** — a *Cleanup* system: Quick Wins ranking, rule-based presets (developer folders, caches, installers, logs), stale-file rules, and an app uninstaller with leftover detection — each action previewed and reversible (recycle bin by default). The free tool offers only raw delete.
4. **Duplicates (`PRISM-VIS-004`)** — byte-identical file detection with the size → partial-fingerprint → full-hash pipeline ([12-HOMEGROWN-FEATURES § Duplicates](12-HOMEGROWN-FEATURES.md#4-duplicates)), staging *n−1* copies per group for cleanup.
5. **History (`PRISM-VIS-005`)** — snapshots: capture a scan's shape, diff any two ("what grew by 18 GB since last month"), with a 10 MB significance threshold on delta rows.
6. **Nine ways to read the same bytes (`PRISM-VIS-006`)** — treemap, sunburst, icicle, circle pack, radial mind-map, folder cards, ranked table, bars, age timeline — one interaction grammar across all ([11-VISUALIZATION-ENGINE](11-VISUALIZATION-ENGINE.md)).
7. **A product, not a utility (`PRISM-VIS-007`)** — premium visual system with six hand-tuned themes ([08-UI-DESIGN-SYSTEM](08-UI-DESIGN-SYSTEM.md)), 60–120 fps interaction, full keyboard control, and empty states that teach.
8. **Privacy as a feature (`PRISM-VIS-008`)** — all analysis local; the only network calls the app ever makes are license activation/validation and signed update checks ([13](13-LICENSING-SYSTEM.md), [14-SECURITY-MODEL](14-SECURITY-MODEL.md)). This is stated in-product, not just in a policy.

## 5. Product principles (decision tie-breakers)

When any doc, PR, or agent is ambiguous, resolve with these, in order:

1. **Local-first.** If a feature needs user file data to leave the machine, it does not ship. No telemetry on file paths, names, or sizes — ever. Crash reports and anonymous feature flags only, opt-in.
2. **Fast is a feature.** Interactions respond in < 100 ms; visualizations hold frame budget ([16](16-PERFORMANCE-BUDGETS.md)). If a feature cannot be made fast, it is redesigned or cut — never shipped laggy.
3. **Never lie about bytes.** Every displayed number has a defined provenance (logical / allocated / unique) and the inspector always shows all three. Silent miscounts are release blockers.
4. **Destructive actions are always previewed, reversible-by-default, and never batched without a confirmation ledger.** See [10-SCREENS-AND-FLOWS § Cleanup Safety](10-SCREENS-AND-FLOWS.md#cleanup-safety-the-deletion-contract).
5. **Parity is a floor, not a ceiling.** The porting matrix ([03](03-PORTING-MATRIX-WINDIRSTAT.md)) must reach 100% before GA, and no homegrown feature may degrade any parity row.
6. **One code path per feature.** No fallback implementations, no degraded modes, no feature flags hiding half-built features ([19-AGENT-PROTOCOL A2](19-AGENT-PROTOCOL.md#a2--no-fallbacks-the-rule-the-owner-cares-about-most)).
7. **Own the visual identity.** Components are hand-tuned on top of owned shadcn/ui sources; no default-palette look, no stock marketing-site effects ([09 § Sourcing Ratios](09-UI-COMPONENTS.md#1-sourcing-ratios-owner-mandated-r5)).

## 6. Licensing & commercial shape

- **Two SKUs, exactly** (`PRISM-VIS-010`): **Yearly** (12-month license, renews manually) and **Lifetime** (one-time purchase). No tiers named Free/Pro/Team in v1 — the *app itself* is the product; a time-limited full-featured trial (14 days, full features, no watermarks) converts to one of the two SKUs.
- Activation is **per-machine with device binding**; deactivate on this machine releases the key for another (both SKUs — mirrors the competitive model validated by our intel, [02-INTEL-DISKBUDDY § Licensing Model](02-INTEL-DISKBUDDY.md#35-licensing-model-fully-reconstructed)).
- Offline grace: signed entitlement cached locally, 30-day revalidation window, clear in-UI countdown ([13-LICENSING-SYSTEM § Grace](13-LICENSING-SYSTEM.md#53-offline-grace)).
- Payments via a merchant-of-record provider (provider-agnostic adapter; **Dodo Payments** is the default integration target because our intel confirms the model works end-to-end for this exact product category — see [13 § Payment Provider](13-LICENSING-SYSTEM.md#payment-provider)).
- Server for license operations ships in the monorepo; **first build runs on `http://localhost:8080` with a demo key** ([13 § Local Development](13-LICENSING-SYSTEM.md#10-local-development--demo-key-owner-requirement-r11)).

## 7. Success criteria (v1 GA definition)

| ID | Criterion | Measured by |
|---|---|---|
| S1 | Porting matrix at 100% — every row verified | [03] checklist + QA sign-off rows |
| S2 | Scan ≥ 1M files/sec sustained on NVMe (turbo), ≥ 150k files/sec standard | `bench-scan` harness, [16] |
| S3 | Interaction p95 < 100 ms, treemap/sunburst steady 60 fps (120 fps-capable), 4 GB memory ceiling on 8M-file scans | `bench-ui` harness, [16] |
| S4 | Crash-free sessions ≥ 99.5% over beta cohort | Crash reporting (opt-in) |
| S5 | Install → first insight (a scanned drive with Quick Wins visible) in ≤ 90 seconds for a non-technical user | Usability test script, [17] |
| S6 | Licensing: activate → validate → deactivate → re-activate on second machine, full E2E including offline grace | E2E suite, [13] |
| S7 | Zero traces of forbidden vocabulary in shipped artifacts | Trace scanner, [17] |

## 8. Scope boundaries (v1)

**In:** everything in [03] (parity) + [12] (homegrown), Windows 10 21H2+ / Windows 11 x64 & arm64 (arm64: engine + app run natively; see [15-BUILD-PACKAGING](15-BUILD-PACKAGING.md)), per-user and per-machine install, auto-update with signed manifests.

**Out (v1, revisit post-GA):** macOS/Linux UI (engine keeps platform boundary, [06 § Platform Boundary](06-RUST-CORE.md#platform-boundary)); central fleet reporting; cloud sync of anything; scheduled background *cleaning* (scheduled *scans* are in — see [12 § Scheduled Scans](12-HOMEGROWN-FEATURES.md#9-scheduled-scans)); file-content search (names/metadata only); third-party plugin API.

## 9. Naming rules

`PRISM-VIS-020` — Shipped artifacts (binaries, installers, UI strings, bundle IDs, file names, registry keys, domains, code-signing CN, docs inside the app repo *except* this build-plan's research docs) MUST NOT contain the forbidden vocabulary list in [00-INDEX § 4.3](00-INDEX.md#43-forbidden-vocabulary-in-shipped-artifacts), including near-homophones and wordplay. CI trace-scans every release artifact ([17-QUALITY-ENGINEERING § Trace Scanner](17-QUALITY-ENGINEERING.md#4-trace-scanner-prism-qa-010)). The internal codename `Prism`/`prism-core` is used for repos, crates, and internal docs; the shipping product name is a separate owner decision recorded in `docs/amendments/naming.md` — components must centralize display-name strings (single `productName` constant + resource files) so renaming is a one-PR change.

**Related:** competitive context in [02-INTEL-DISKBUDDY.md](02-INTEL-DISKBUDDY.md) · parity contract in [03-PORTING-MATRIX-WINDIRSTAT.md](03-PORTING-MATRIX-WINDIRSTAT.md) · feature specs in [12-HOMEGROWN-FEATURES.md](12-HOMEGROWN-FEATURES.md).
