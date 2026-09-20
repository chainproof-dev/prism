# 21 — Glossary

> Owner doc: [00-INDEX.md](00-INDEX.md) · Shared vocabulary for all agents. Terms are normative: use these words in code, docs, PRs, and QA scripts so cross-referencing stays mechanical.

| Term | Definition | Home doc |
|---|---|---|
| **Arena** | The engine's columnar, index-addressed node store for one scan. `NodeId = u32` index; SoA arrays; single-writer during build, frozen after. | [07](07-DATA-MODEL.md#2-the-arena-engine-side) |
| **Allocated size** | Bytes on disk a file/folder occupies (cluster-aligned, sparse-aware). Default display mode. | [06 § 2.3](06-RUST-CORE.md#23-correctness-rules-each--unit-test-module) |
| **Logical size** | EndOfFile byte count (what Explorer's Properties "Size" shows). | [06 § 2.3](06-RUST-CORE.md#23-correctness-rules-each--unit-test-module) |
| **Unique size** | Bytes counted once per hard-link group (first file id owns the bytes). | WDS-SCN-07 |
| **ScanId** | Engine lease id for one scan session; NodeIds are valid only within it. | [05 § 3.2](05-IPC-PROTOCOL.md#32-scanning) |
| **Standard scan** | Parallel directory-enumeration strategy; no elevation; default. | [ADR-06](04-SYSTEM-ARCHITECTURE.md#adr-06--two-first-class-scan-strategies-not-a-fallback) |
| **Turbo scan** | Opt-in elevated raw-NTFS (MFT) strategy. | [06 § 3](06-RUST-CORE.md#3-turbo-scan--raw-ntfs-opt-in-elevated) |
| **Elevation helper** | Signed `prism-elev.exe` that brokers one volume handle via named pipe, then exits. Never scans. | [06 § 7](06-RUST-CORE.md#7-elevation-strategy) |
| **Entitlement token** | Short-lived Ed25519-signed CBOR claims object granting premium features to one device. | [13 § 4](13-LICENSING-SYSTEM.md#4-entitlement-token-the-contract) |
| **instance_id** | Per-install UUID used as the device binding identity. | [13 § 4](13-LICENSING-SYSTEM.md#4-entitlement-token-the-contract) |
| **Grace window** | Offline validity of the last entitlement token (≤ 30 days), with in-UI countdown. | [13 § 5.3](13-LICENSING-SYSTEM.md#53-offline-grace) |
| **Cleanup ledger** | The reviewed, reversible-by-default staging list through which ALL deletions flow. | [10 § Cleanup Safety](10-SCREENS-AND-FLOWS.md#cleanup-safety-the-deletion-contract) |
| **Stage / Unstage** | Add/remove an item to/from the cleanup queue (never deletes by itself). | [09 CleanupBar](09-UI-COMPONENTS.md#cleanupbar-ledger--10--cleanup-safety) |
| **Preset** | A named cleanup rule (glob match + safety class + explanation). | [12 § 2](12-HOMEGROWN-FEATURES.md#2-cleanup-presets-rule-engine) |
| **Preset hit** | One concrete match of a preset rule in the current scan. | [12 § 2](12-HOMEGROWN-FEATURES.md#2-cleanup-presets-rule-engine) |
| **Stale** | Item meeting the age+size rule (default ≥ 40 MB ∧ ≥ 1 year). | [12 § 3](12-HOMEGROWN-FEATURES.md#3-stale-rules) |
| **Quick Wins** | Ranked, explainable reclaim candidates (bytes × confidence). | [12 § 1](12-HOMEGROWN-FEATURES.md#1-quick-wins) |
| **Snapshot** | Depth-capped persistent capture of a scan's shape (PSNP1 format) for diffing. | [07 § 4](07-DATA-MODEL.md#4-persistence-schema-sqlite-appdb) |
| **SnapshotDelta** | Diff between two snapshots with a 10 MB significance floor. | [12 § 6](12-HOMEGROWN-FEATURES.md#6-snapshots--diff) |
| **Leftovers** | Data of uninstalled apps detected by the token matcher. | [12 § 5](12-HOMEGROWN-FEATURES.md#5-application-uninstaller--leftovers) |
| **Category** | Curated classification of files (~40) driving the data palette and type list. | [08 § 4.1](08-UI-DESIGN-SYSTEM.md#41-the-12-hue-categorical-spectrum-anchor-hues-nocturne-rendering) |
| **DataPalette** | Theme-relative color mapping (ARGB array) for canvas viz; single source of truth for legend. | [08 § 4](08-UI-DESIGN-SYSTEM.md#4-data-colors-the-spectrum) |
| **Color mode** | Viz encoding: by type / by branch / by age. | [08 § 4.2](08-UI-DESIGN-SYSTEM.md#42-color-modes-viz-rail--concept-parity-with-02--32) |
| **VizFrame** | Binary payload (PVF1) of a culled, laid-out visualization ready to paint. | [05 § 5](05-IPC-PROTOCOL.md#5-binary-payloads-vizframe) |
| **LOD params** | Culling limits (maxTiles, maxArcs, minTilePx, minShare, drawnDepth…). | [11 § 3](11-VISUALIZATION-ENGINE.md#3-level-of-detail-lod--the-scale-rules) |
| **Composite tile** | Aggregated `<small items>` node produced by LOD culling; real, selectable. | [11 § 3](11-VISUALIZATION-ENGINE.md#3-level-of-detail-lod--the-scale-rules) |
| **HoverReadout** | The pointer-anchored floating detail pill. | [09](09-UI-COMPONENTS.md#hoverreadout) |
| **Cushion treemap** | Squarified treemap with height-field shading (van Wijk & van de Wetering paper), our clean-room implementation. | [11 § 4.1](11-VISUALIZATION-ENGINE.md#41-treemap-parity-contract-wds-tmp-0111) |
| **Event pump** | SPSC ring → drain thread → TSFN batched event pipeline. | [05 § 4](05-IPC-PROTOCOL.md#4-event-pump-architecture) |
| **Free-space node** | Synthetic node showing volume free bytes. | WDS-SCN-04 |
| **Unknown node** | Synthetic node aggregating unreadable paths (access denied etc.). | WDS-SCN-05 |
| **Errors drawer** | UI surface listing per-path scan errors with retry. | [10 § 11](10-SCREENS-AND-FLOWS.md#11-errors-drawer) |
| **Gate (phase)** | The verified checklist that ends a phase; only persona/owner reviews may sign boxes. | [18](18-PHASE-PLAN.md) |
| **Session** | One agent work block; recorded in the worklog. | [18 § Session Records](18-PHASE-PLAN.md#session-records) |
| **Persona review** | Structured review pass wearing one of the five senior hats. | [17 § 6](17-QUALITY-ENGINEERING.md#6-persona-reviews-prism-qa-020) |
| **Trace scanner** | CI job hunting forbidden vocabulary in shipped artifacts. | [17 § 4](17-QUALITY-ENGINEERING.md#4-trace-scanner-prism-qa-010) |
| **Requirement ID** | Stable identifier (`PRISM-<AREA>-<n>`, `WDS-<area>-<n>`) linking spec→tests→QA. | [00 § 4](00-INDEX.md#4-conventions-used-across-all-documents) |
| **Amendment** | The only legal way to change a frozen doc: `docs/amendments/AMM-<n>-<slug>.md` + status board update. | [00 § 6](00-INDEX.md#6-document-status-board) |
| **No-fallback rule** | The prohibition on silent degraded paths; failures must be loud, visible, or blocking. | [19 A2](19-AGENT-PROTOCOL.md#a2--no-fallbacks-the-rule-the-owner-cares-about-most) |
| **Fail loud** | Typed error + log + actionable user message. The required failure style. | [19 A2](19-AGENT-PROTOCOL.md#a2--no-fallbacks-the-rule-the-owner-cares-about-most) |
| **Locked-tab preview** | Honest gated state for premium tabs (masked real affordances + upgrade card). | [10 § 12](10-SCREENS-AND-FLOWS.md#12-locked-tab-preview-conversion-surface) |
| **R1/R2/R3** | The three reference machines for budgets. | [16 § 1](16-PERFORMANCE-BUDGETS.md#1-reference-machines) |
| **FIX-S/M/L** | Deterministic fixture volumes for tests and benches. | [16 § 1](16-PERFORMANCE-BUDGETS.md#1-reference-machines) |
| **Prism** | Internal codename of this product (shipping name TBD; zero-trace rule applies). | [01 § 9](01-PRODUCT-VISION.md#9-naming-rules) |
| **Vendor** | Placeholder for the company/publisher identity in configs, signing CN, bundle IDs. | [01 § 9](01-PRODUCT-VISION.md#9-naming-rules) |
