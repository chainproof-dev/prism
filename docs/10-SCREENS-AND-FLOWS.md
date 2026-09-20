# 10 — Screens & Flows

> Owner doc: [00-INDEX.md](00-INDEX.md) · Upstream: [08-UI-DESIGN-SYSTEM](08-UI-DESIGN-SYSTEM.md), [09-UI-COMPONENTS](09-UI-COMPONENTS.md) · Downstream: [12-HOMEGROWN-FEATURES](12-HOMEGROWN-FEATURES.md), [13-LICENSING-SYSTEM](13-LICENSING-SYSTEM.md), [17-QUALITY-ENGINEERING](17-QUALITY-ENGINEERING.md).
> Requirement IDs: `PRISM-FLOW-*`.
>
> Every screen below specifies: purpose, layout, states (loading/empty/partial/error/success), entry points, and exit criteria. QA scripts reference these IDs. Parity rows from [03](03-PORTING-MATRIX-WINDIRSTAT.md) are cited inline.

---

## 1. Screen map

```
LicenseGate (pre-license)  ·  Welcome  ·  Scanning (overlay)  ·  Explore  ·  Duplicates
Applications  ·  Monitor  ·  Snapshots  ·  Leftovers  ·  Settings (sheet)  ·  About (dialog)
Errors (drawer)  ·  Ledger (sheet)  ·  Command palette (overlay)  ·  Activation (screen)
```

One window, one workspace at a time (single-window app; parity tool is single-window too — WDS-CHROME class). Screens other than Explore are premium-gated tabs ([13 § SKUs](13-LICENSING-SYSTEM.md#1-skus-and-feature-matrix)) except a locked-tab preview state (see § 12).

### Workspace tabs

The TopBar hosts the workspace tabs, in fixed order: **Explore · Duplicates · Applications · Monitor · Snapshots · Leftovers**. Tabs are workspace-level, not window-level; premium tabs render the locked preview until licensed ([§ 12](#12-locked-tab-preview-conversion-surface)). Keyboard: `Alt+1..6`. Each tab owns its screen state but shares the active scan, selection, and cleanup queue.

### Inspector

The right-hand context panel (300px / 320px ≥ 1600px viewport) is present on Explore and Duplicates; its full contract is specified in [09 § Inspector](09-UI-COMPONENTS.md#inspector-300320px). Toggle: `Ctrl+I`.

## 2. LicenseGate / Activation

See [13-LICENSING-SYSTEM § Activation UX](13-LICENSING-SYSTEM.md#51-activation) for the full contract (gate condition, trial countdown, key input rules, error mapping, checkout handoff, deactivate-in-settings flow).

## 3. Welcome (drive/folder selection — WDS-SEL-01/02/04/05/06)

**Layout:** centered hero column (max 880px): product wordmark + one-line value statement ("Every byte, accounted for." — final copy from voice pass); **DriveCard grid** (responsive 2–4 columns, premium entrance stagger); below: `Scan folder…` + `Scan home` + recent targets list (6); footer microcopy: privacy line ("Nothing about your files leaves this computer.") + version/channel.

**DriveCard:** 120px tall; volume glyph (SSD/HDD/USB/Network by type); letter + label; used/free DonutGauge (56px); `total`; **Turbo badge** if NTFS + explained on hover ("Raw index read — needs admin approval"); click = preflight (`sys:preflight`) → scan; disabled state for no-media drives with reason.

**States:** loading (skeletons ≤ 300ms then cache), no-drives (server/virtual machine edge case: guidance card), network drive warning chip (latency honesty), elevation-needed preview (preflight returns `requiresElevation` → inline explainer before consent dialog).

**Exits:** any scan target → Scanning; `Ctrl+,` → Settings; account chip → Activation/licence state.

## 4. Scanning (overlay over target screen)

**Layout:** full-area canvas showing the treemap **building live** (tiles fade+scale in as directories finalize; luminance sweep motif — WDS-SCN-02/03 substitution); bottom strip: phase label (`Walking directories` → `Aggregating` → `Indexing types`), files/sec (mono, live), bytes seen, elapsed, current path (middle-truncated, mono), Cancel (`Esc`) / Pause / Resume; right: mini DonutGauge filling.

**Rules:** partial tree already browsable during scan (tree/viz interactive at ≥ 40% opacity interaction target); progress counters update ≤ 4 Hz ([05 § 4](05-IPC-PROTOCOL.md#4-event-pump-architecture)); cancel ≤ 50 ms ack; resume continues without data loss (standard strategy only — turbo shows "cancel" only, honest per [ADR-06](04-SYSTEM-ARCHITECTURE.md#adr-06--two-first-class-scan-strategies-not-a-fallback)).

**Exits:** done → Explore with toast "Scanned 1.24M files in 9.8s"; failed → Explore + Banner with errors drawer link; cancelled → return to previous state.

## 5. Explore (the core screen — WDS-TRE/TMP/EXT rows)

**Layout:** AppShell: Sidebar (§ below) | Center: viz area with sub-header | Inspector.

**Center sub-header:** breadcrumb zoom path (click any crumb = zoom), viz rail (9 modes, [11](11-VISUALIZATION-ENGINE.md)), color-mode rail (Type/Branch/Age), scope segmented (`In this folder` / `Biggest files anywhere` / `Biggest folders anywhere` — Bars/Table scopes), labels toggle, depth slider (treemap/icicle/mindmap), size-mode toggle (Allocated/Logical — WDS-DSP parity), search field (global focus `Ctrl+F`).

**Viz area:** VizCanvas host; HoverReadout on hover; selection syncs everywhere (WDS-TRE-04); context menu per node (WDS-CTX-01 full set + premium items); drag-out (WDS-CTX-05); drop-in = locate file.

**Sidebar sections:** per [09 § Sidebar](09-UI-COMPONENTS.md#sidebar-264px) — Scan actions, Disk gauge, Current view, Quick Wins, Types, Cleanup queue mini-panel.

**States:** pre-scan (Welcome handles), scanning (§ 4), done (default), rescan-subtree (subtle shimmer on the affected branch only), truncated (arena ceiling hit — Banner explaining + guidance), zero-file scan ("This folder is empty" centered state with rescan/back actions — parity empty copy pattern).

## 6. Duplicates (premium tab)

**Entry:** "Find duplicates" CTA on empty state (explains: works on the current/last scan; byte-identical guarantee; full-hash final arbiter — concept parity [02 § 3.3](02-INTEL-DISKBUDDY.md#33-feature-inventory-from-symbols--strings--exhaustive)). Run button shows pipeline phases as they execute (grouping by size → fingerprinting → full hashing with bytes-hashed progress).

**Layout:** group list (FileTable variant): group header row (n files · total waste = (n−1)×size · Stage extras) + member rows (path mono, jump-to-tree icon, hash-status chip, first-instance protected badge "Kept"). Left rail: summary card (groups, reclaimable bytes, largest group), filters (min size, category filter, exclude patterns), `Stage all extras` (guarded by ledger confirm).

**States:** no-scan (empty: "Run a scan first" + CTA to Welcome), running (progress by phase, cancellable), none-found ("No duplicates found" + last-run metadata + rescan offer), found (default).

**Safety:** staging never auto-deletes; ledger shows all; protected paths (system list) cannot be staged ([10 § Cleanup Safety](#cleanup-safety-the-deletion-contract)).

## 7. Applications (premium tab)

**Layout:** master list (left, 320px): installed apps (icon, name, size est., store/desktop chip, "Show system/Store apps" toggle — parity concept WDS/APPS from intel); detail (right): footprint breakdown (Program files, AppData ×3, ProgramData, registry keys count, other roots) as stacked CategoryBar + per-root rows with Reveal; **Uninstall completely** flow: standard uninstaller runs → leftover re-scan → leftover list with checkboxes → **Clean leftovers** → ledger → execute.

**Leftovers tab** (child of Applications): apps no longer installed but with detected remains; group by app token; same clean flow. Empty state: "No leftovers from removed apps." (parity copy pattern, own words).

## 8. Monitor (premium tab)

**Layout:** top: system strip (CPU total sparkline 60s, memory pressure, disk activity); main: ProcessSample table (top 20 by CPU toggle: by Disk IO / by Memory): name, PID, CPU%, IO bytes/s, working set, threads; 1 Hz sample cadence, smooth value transitions (140ms number crossfades, never jumpy). Note: monitor is informational (no kill/affinity actions in v1 — deliberate: no destructive surface on a diagnostics pane).

## 9. Snapshots (premium tab)

**Layout:** snapshot list (rows: root path, captured-at, files, bytes, depth); actions: **Capture current scan**, delete, export (JSON). **Diff view:** BEFORE/AFTER pickers (two clicks, guided: "Click one snapshot for the BEFORE side, then another for AFTER." — concept parity, own copy); delta table (grew/shrank/added/removed, bytes delta with diverging colors, path, depth); significance floor 10 MB with "show smaller changes" toggle; summary cards (net change, top grower, top shrinker).

## 10. Settings (sheet, right, 420px — `Ctrl+,`)

Sections: **General** (theme ×6, density, size display default, language, autostart, shell integration toggle WDS-CTX-04) · **Scanner** (strategy default, follow reparse WDS-SCN-06, packages-as-nodes, exclusions editor) · **Treemap & viz** (WDS-TMP-07 set: gap, cushion elevation/brightness/highlight, label density + per-mode defaults) · **Cleanup** (recycle-vs-permanent default, confirm thresholds, preset enable/edits, block-list review) · **Type colors** (WDS-EXT-03/04 editor) · **Account** (license state, deactivate this machine — [13](13-LICENSING-SYSTEM.md#54-deactivation)), export diagnostics · **Updates** (channel stable/beta, check now, view manifest) · **About** (versions, OSS licenses, EULA, privacy).

Every setting persists (WDS-CFG-01) with reset-per-section (WDS-CFG-03).

## 11. Errors drawer

Bottom drawer (40% height, resizable): table of PathError rows (path mono, reason translated, retry action per row, copy-all). Opens from Banner or status strip error count. Empty: closes itself.

## 12. Locked-tab preview (conversion surface)

Premium tabs when unlicensed (trial expired): tab content renders **real data shape** with 20% opacity mask + center card (feature one-liner, byte-specific value hook e.g. "1,842 MB of exact duplicates found in scans like this", Activate/Buy buttons). No fake data — the mask explains honestly ([19 A2](19-AGENT-PROTOCOL.md#a2--no-fallbacks-the-rule-the-owner-cares-about-most) spirit: no deceptive states).

## 13. Cross-cutting flows

### Search & filter
`Ctrl+F` focuses search; engine debounce contract: type → 150ms idle → `filter:apply`; results dim non-matching tiles (8% opacity) and filter tree/table with "filtered" chip + count; `Esc` clears; regex auto-detected (invalid regex → literal glob fallback **shown as such** in the chip — labeled, not silent); scope: current view or everywhere (segmented). Age/size/category facets live in a filter popover on the search field.

### Cleanup Safety (the deletion contract)
1. **Stage** (any source: manual select, preset, duplicates extras, stale, leftovers) → enters queue; row shows danger-tinted staged badge; sidebar queue panel updates; toast optional (first time only).
2. **Review** → ledger sheet: grouped tree, per-item sizes, protected-path warnings block execution until acknowledged per group (type-the-folder-name for block-list roots, WDS-DEL-05), totals with reclaim preview vs free space.
3. **Execute** → confirm dialog (danger-styled primary): "Delete 38 items · 42.1 GB to Recycle Bin" (or Permanently — destructive adjective explicit); progress events per item; failures stay in ledger with reasons (fail-loud, [19 A2](19-AGENT-PROTOCOL.md#a2--no-fallbacks-the-rule-the-owner-cares-about-most)); success → tree updates immediately (WDS-DEL-03), toast with reclaimed total + **Undo** (where bin used and within session — `IFileOperation` undo record).
4. **Hard blocks** (never staged): Windows/system roots, current scan root itself, anything on a network share (bin semantics differ — permanent-only with explicit warning instead of silent difference).

### Exports
`export:scan` → save dialog (CSV/JSON choice, scope: full/selection/filtered) → streamed write with progress → toast with open-folder affordance. Premium-gated.

### First-run
LicenseGate → (trial start consent, single line, no dark patterns — explicit "Start 14-day full trial" vs "Enter key") → Welcome. No onboarding wizard beyond a 3-card contextual hint set that appears once each (viz rail, quick wins, command palette) with permanent dismiss.

### Voice
Sentence case; verbs first; every number with its unit; every empty state ends with an action; never blame the user ("Couldn't read 37 folders" not "You don't have permission"); dry wit allowed once per screen max ("Files already on their way out" class of line — our own lines, [02 `PRISM-INTEL-012`](02-INTEL-DISKBUDDY.md#4-what-we-take-what-we-change-summary-contract)); no exclamation marks; no "Oops".

## 14. Keyboard map (complete — parity WDS-TRE-05/CHROME-04 + ours)

| Context | Key | Action |
|---|---|---|
| Global | `Ctrl+K` | Command palette |
| Global | `Ctrl+N` | New scan (Welcome) |
| Global | `Ctrl+,` | Settings |
| Global | `Ctrl+F` | Search/filter |
| Global | `Ctrl+I` | Toggle inspector |
| Global | `Ctrl+1..9` | Viz mode select |
| Global | `Alt+1..6` | Workspace tab switch |
| Global | `Alt+←/→` | History back/forward (zoom path) |
| Global | `Alt+↑` | Zoom out to parent (WDS-TMP-05) |
| Global | `+` / `-` / `Ctrl+wheel` | Viz zoom in/out |
| Global | `F5` | Rescan (WDS-SCN-10) |
| Global | `Esc` | Cancel scan / clear filter / close overlay (context-ordered) |
| Tree/Table | `↑↓` `←→` `Home/End` `PgUp/PgDn` | Navigation (WDS-TRE-05) |
| Tree/Table | `Space` | Toggle row selection |
| Tree/Table | `Enter` | Default action (open/zoom) |
| Tree/Table | `Ctrl+A` | Select page |
| Tree/Table | typing | Type-ahead (300ms window) |
| Viz | hover | HoverReadout (no IPC, [11](11-VISUALIZATION-ENGINE.md#7-testing)) |
| Viz | click / dblclick | Select / zoom (WDS-TMP-04/05) |
| Viz | `Shift+click` | Multi-select (add to selection) |
| Anywhere | `Ctrl+C` | Copy path(s) of selection (WDS-CTX-06) |
| Anywhere | `Delete` | Stage selection for cleanup |
| Anywhere | `Shift+Delete` | Delete permanently (ledger confirm) |
| Anywhere | `F2`? — **no** renaming (read-only product) | — |
| Menus | `Alt` | Focus menu bar |
| Dialogs | `Tab`/`Enter`/`Esc` | Radix contracts |

## 15. Stores (state architecture)

- `useScanStore` — active scan ids, phase, progress counters, summary (zustand; selectors slice per pane).
- `useSelectionStore` — selection set, hovered (transient, non-reactive via ref for hover), zoom path per viz mode, active filter.
- `useVizStore` — mode, colorMode, LOD settings, frame cache refs (non-serializable, rAF-synchronized).
- `useCleanupStore` — staged items (ordered map), ledger state, execute progress.
- `useLicenseStore` — state mirrored from main via `lic:status` events (never computed in renderer).
- Persistence: UI-only prefs via electron-store equivalent (main-side settings service); no component-local persistence.

`PRISM-FLOW-001` — No store may hold derived data recomputable from engine queries (anti-cache-bloat rule; derived values memoized at component edge).
`PRISM-FLOW-002` — Every screen has a `.states.md` spec table (loading/empty/partial/error/success) reviewed by the UI/UX persona ([17 § Persona Reviews](17-QUALITY-ENGINEERING.md#6-persona-reviews-prism-qa-020)) before merge.
