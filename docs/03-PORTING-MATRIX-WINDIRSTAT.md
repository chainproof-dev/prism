# 03 — Porting Matrix: 100% Coverage Contract (Reference Tool Parity)

> Owner doc: [00-INDEX.md](00-INDEX.md) · Upstream: [01-PRODUCT-VISION.md](01-PRODUCT-VISION.md) · Downstream: [06-RUST-CORE](06-RUST-CORE.md), [07-DATA-MODEL](07-DATA-MODEL.md), [09-UI-COMPONENTS](09-UI-COMPONENTS.md), [10-SCREENS-AND-FLOWS](10-SCREENS-AND-FLOWS.md), [11-VISUALIZATION-ENGINE](11-VISUALIZATION-ENGINE.md), [17-QUALITY-ENGINEERING](17-QUALITY-ENGINEERING.md).
>
> **This document is the parity contract.** "100% coverage port" means: every row below reaches status **P** (parity) or **P+** (parity plus intentional improvement, which must cite the improvement doc). A row may only be marked **X — dropped by owner decision** with a written amendment; v1 GA cannot ship with open rows. Row IDs are stable (`WDS-<area>-<n>`) and are cited in tests and PRs.
>
> Reference behavior was cataloged from the public application (v2.6.2 line, all user-visible behavior and options), its published changelog, and documented file-format/config semantics. **No source code was consulted or copied** — see § Clean-Room Protocol.

---

## 0. Clean-Room Protocol

1. The reference tool is GPL-2.0. Its **code, resources, translations, icons, bitmaps, dialogs, and config file formats are never opened, copied, translated, or ported** into this repository.
2. What we port is **observable functional behavior**: what the tool does for a user, itemized below. Functionality is re-expressed in our own architecture (Rust engine + React UI) with our own data structures, algorithms where not algorithmically forced (treemap layout algorithms are published academic work — squarified treemaps: Bruls, Huizing & van Wijk; cushion treemaps: van Wijk & van de Wetering — and are implemented from the papers).
3. Any contributor who has read the reference tool's source may **only** contribute behavior descriptions to this matrix, never code. Code contributors work from this matrix.
4. The trace scanner ([17-QUALITY-ENGINEERING § Trace Scanner](17-QUALITY-ENGINEERING.md#4-trace-scanner-prism-qa-010)) blocks any artifact containing reference-tool identifiers.

## Legend

- **Status:** `P` parity · `P+` parity with documented improvement · `X` dropped (needs amendment)
- **Spec:** where our implementation is specified
- **Verify:** how QA proves the row (manual script `QA-<n>`, automated test ID, or bench)

---

## 1. Launch & scan target selection

| ID | Reference behavior | Our spec | Status | Verify |
|---|---|---|---|---|
| WDS-SEL-01 | On launch: dialog listing **all drives** with volume label, type icon, total/used/free, and a graphical usage bar; plus "Folder…" button to pick any folder | **Welcome screen** with drive cards (same data) + folder picker + recent targets + quick targets (Home subfolders) — richer, not poorer ([10 § Welcome](10-SCREENS-AND-FLOWS.md#3-welcome-drivefolder-selection--wds-sel-0102040506)) | P+ | QA-01 |
| WDS-SEL-02 | Selection dialog reachable from toolbar at any time ("Select drive/folder") | Toolbar → "New scan" button + `Ctrl+N`; identical options | P | QA-01 |
| WDS-SEL-03 | Removable/network drives appear with distinct icons; drives with no media are excluded | Same via `GetLogicalDrives`/`GetDriveTypeW`; no-media excluded; network drives carry a latency warning chip (improvement: honest about slow sources) | P+ | QA-02 |
| WDS-SEL-04 | Can scan "All local drives" as one multi-root scan | Supported: multi-root scan; results grouped by root in the tree with a synthetic "This PC" root | P | QA-03 |
| WDS-SEL-05 | Drag a folder onto the main window to scan it | Supported (drop target on empty/loading states) | P | QA-04 |
| WDS-SEL-06 | Re-scan remembered paths offered (recent list) | Recent targets persisted (10 entries) on Welcome + toolbar dropdown | P | QA-05 |

## 2. Scanning behavior

| ID | Reference behavior | Our spec | Status | Verify |
|---|---|---|---|---|
| WDS-SCN-01 | Scans show per-item progress: files read, elapsed time, **animated progress feedback**, and remain responsive/cancellable | Scan overlay: streaming counts (files, bytes, dirs), current path, elapsed, per-phase progress, cancel (`Esc`) — 60 fps animation, no main-thread stalls ([10 § Scanning](10-SCREENS-AND-FLOWS.md#4-scanning-overlay-over-target-screen)) | P+ | QA-10, bench-scan |
| WDS-SCN-02 | Directory sizes aggregated bottom-up during scan; tree shows partial (growing) sizes while scanning | Live aggregation: partial subtree sums stream into the tree at ≤ 4 Hz coalesced updates; totals finalize with a "finished in X" toast | P+ | QA-11 |
| WDS-SCN-03 | **Pac-Man animation** on the treemap during scan (progress motif) | Replaced by our own scan motif: treemap tiles **fade+scale in** as directories finalize, with a sweeping luminance pass (distinct mechanism, same reassurance purpose) — owner-approved substitution | P+ | QA-12 |
| WDS-SCN-04 | Free space on scanned volume displayed as a pseudo-item ("<Free space>") sized to actual free bytes | `free-space` pseudo-node under volume root; excluded from cleanup actions; shown in all views with dedicated neutral styling | P | QA-13 |
| WDS-SCN-05 | Items whose size could not be read (access denied, errors) surface as a pseudo-item ("unknown") and per-path error list | `unknown` pseudo-node + **Errors drawer**: path, reason (translated), retry-scan-this-path action (improvement) | P+ | QA-14 |
| WDS-SCN-06 | Junctions/symlinks: user option whether reparse points are followed (default: not followed, shown with distinct badge) | Settings → Scanner: "Follow junctions/symlinks" (default **off**), loop protection via per-scan `(dev,inode)` visited-set regardless; junction nodes carry badge + jump-to-target action ([06 § Reparse](06-RUST-CORE.md#23-correctness-rules-each--unit-test-module)) | P+ | unit REPARSE-* |
| WDS-SCN-07 | Hard links: reference tool counts each link instance at full size (known double-count) | **P+ (intentional divergence):** hard-link groups counted once in "unique bytes" accounting; tree shows `n×links` badge; inspector lists all link paths; size-mode switch Logical/Allocated/Unique (see also WDS-DSP-04) | P+ | unit HARDLINK-* |
| WDS-SCN-08 | Scan can be paused? — reference: no pause, only cancel | We add pause/resume on the standard scanner (improvement; turbo scan cancel-only) | P+ | QA-15 |
| WDS-SCN-09 | Rescan selected subtree only (refresh of one folder) | Context menu "Rescan this folder" — engine re-walks one subtree in place, preserving node identity for non-changed paths | P+ | QA-16 |
| WDS-SCN-10 | Full rescan of the same target (re-read everything) | `Ctrl+R` / toolbar refresh; with snapshot diff offered after (homegrown, [12 § Snapshots](12-HOMEGROWN-FEATURES.md#6-snapshots--diff)) | P | QA-17 |
| WDS-SCN-11 | Scans continue when UAC-protected paths deny access (deny → unknown, not fatal) | Same; plus "Scan again as administrator" affordance when unknown > threshold (improvement, [06 § Elevation](06-RUST-CORE.md#7-elevation-strategy)) | P+ | QA-18 |
| WDS-SCN-12 | Long paths (> 260 chars) handled | `\\?\`-prefixed wide-path I/O everywhere in the engine; UI displays trimmed paths with full path in tooltip/inspector | P | unit LONGPATH-* |
| WDS-SCN-13 | Scan multiple roots (mix of folders/drives) in one session via AFS dialog | Multi-root supported from Welcome (multi-select) — matches WDS-SEL-04 | P | QA-03 |

## 3. Main window — directory tree pane

| ID | Reference behavior | Our spec | Status | Verify |
|---|---|---|---|---|
| WDS-TRE-01 | Tree columns: name (with icon), **subtree percentage bar**, size (formatted), allocated?, items (file count), folders? — with per-column sorting | Tree table: Name+icon, % bar, Logical size, Allocated size, Files, Folders, % of parent; click-header sort + sort direction indicator; column chooser persists | P+ | QA-20 |
| WDS-TRE-02 | Percentage bar colored by extension category of the node | Bar tinted with the node's category color (our category system, [08 § Data Colors](08-UI-DESIGN-SYSTEM.md#4-data-colors-the-spectrum)) | P | QA-20 |
| WDS-TRE-03 | Expand/collapse with lazy loading of children | Virtualized tree (TanStack Virtual); children fetched from arena on expand ([09 § FileTree](09-UI-COMPONENTS.md#filetree-directory-tree--wds-tre-)) | P | bench-tree |
| WDS-TRE-04 | Selection syncs treemap + extension list + detail views | Selection is a single store slice; all panes subscribe — selection change highlights in every view incl. all 9 viz modes | P | QA-21 |
| WDS-TRE-05 | Keyboard navigation: arrows up/down (siblings), left/right (collapse/expand/into), Home/End, page keys, type-ahead search | Full keyboard map incl. type-ahead with debounce; see [10 § Keyboard Map](10-SCREENS-AND-FLOWS.md#14-keyboard-map-complete--parity-wds-tre-05chrome-04--ours) | P | QA-22 |
| WDS-TRE-06 | Double-click folder: rescan-on-double-click option in reference tool ("rescan on double-click") — default opens (expands) | Option "Double-click rescans folder" (default off; double-click = expand/zoom-to) | P | QA-23 |
| WDS-TRE-07 | File rows show file icon + name + size; files are leaves | Same; icons via Shell image list (SHIL) extraction, cached ([06 § Icons](06-RUST-CORE.md#6-icon-extraction)) | P | QA-24 |
| WDS-TRE-08 | Sorting: by size (desc default), name, files count, folders count, percent | Same + allocated/unique columns sortable | P+ | QA-20 |
| WDS-TRE-09 | Tree shows "<Root>" synthetic node for folder scans | Folder scan shows a root node named after the folder; drive scans show volume root `C:\` etc. | P | QA-25 |

## 4. Main window — treemap pane

| ID | Reference behavior | Our spec | Status | Verify |
|---|---|---|---|---|
| WDS-TMP-01 | Cushion treemap: squarified layout + cushion shading (3D-look height field), files as leaf tiles | Squarified layout (Rust) + **cushion shading** implemented as our own height-field renderer on Canvas2D/WebGL ([11 § Treemap](11-VISUALIZATION-ENGINE.md#41-treemap-parity-contract-wds-tmp-0111)) — the signature look is preserved conceptually with our own palette/shading curve | P+ | QA-30 |
| WDS-TMP-02 | Tiles colored by file extension/category; directory tiles are transparent composites (children painted over) | Same semantic: leaf color = extension category; directory = composite of children; our category palette ([08](08-UI-DESIGN-SYSTEM.md#4-data-colors-the-spectrum)) | P | QA-30 |
| WDS-TMP-03 | Hover highlights tile + shows tooltip (name, size, percentage) | Hover = tile outline + hover readout (name, full path, size, % of parent, % of scan, mtime); 60 fps hit-testing via spatial index | P+ | QA-31 |
| WDS-TMP-04 | Click selects (syncs all panes); selection outline persists | Same; selection ring animated (150 ms) once, no continuous animation cost | P | QA-21 |
| WDS-TMP-05 | **Zoom**: double-click a directory tile → zoom into it; "zoom out to parent" control; breadcrumb of zoom path; Ctrl+wheel zoom | All present: dbl-click zoom-in, toolbar/`Alt+Up` zoom-out, zoom breadcrumb, Ctrl+wheel smooth zoom with 200 ms eased re-layout; keyboard `+`/`-` | P+ | QA-32 |
| WDS-TMP-06 | Zoomed-out context: parent shown as surrounding frame ("zoomed treemap shows parent as frame") | Parent frame with "back to parent" affordance — same model | P | QA-32 |
| WDS-TMP-07 | Treemap options: grid on/off (borders between tiles), cushion brightness, highlight factor, scale factor | Settings → Treemap: tile gap (0–3 px), cushion elevation, cushion brightness/highlight, label density — mapped to our renderer's params; defaults chosen for our palette | P | QA-33 |
| WDS-TMP-08 | Right-click on tile opens context menu | Same menu set as tree rows (WDS-CTX-01) | P | QA-34 |
| WDS-TMP-09 | Files below a size threshold are aggregated visually but still selectable via zoom | Sub-pixel tiles are culled from painting; selection of aggregates via the table/tree; zooming reveals them — matches reference behavior | P | QA-35 |
| WDS-TMP-10 | Treemap redraws correctly on window resize with continuous relayout | ResizeObserver-driven relayout, rAF-coalesced; resize stays interactive (> 30 fps during drag on 100k-tile view) | P | bench-treemap |
| WDS-TMP-11 | "Zoomed treemap" is the primary visualization and survives all operations (scan, delete, refresh) | Treemap is the default viz; zoom path and selection preserved across rescan of visible subtree where identity survives | P+ | QA-36 |

## 5. Main window — extension/type list pane

| ID | Reference behavior | Our spec | Status | Verify |
|---|---|---|---|---|
| WDS-EXT-01 | Third pane: every file extension present, with color swatch, description text, count of files, total size, % bar; sortable | Types panel (left sidebar section, collapsible): swatch, ext name, description (Shell type description, fallback our own table), files count, total size, % of scan; sortable; multi-select by category | P+ | QA-40 |
| WDS-EXT-02 | Selecting an extension highlights all matching files in treemap (and filters tree in reference via highlight) | Selecting type(s) → **filter dimming** across all views (non-matching tiles dim to 8% opacity); tree filters optionally; count badge in header | P+ | QA-41 |
| WDS-EXT-03 | Extension colors user-editable per extension; "reset to defaults" | Settings → Type colors: per-extension override, per-category override, reset; persisted; applies live | P | QA-42 |
| WDS-EXT-04 | Extension descriptions user-editable | Same (editable description, persisted) | P | QA-42 |
| WDS-EXT-05 | Legend of colors in treemap = this pane (single source of truth) | Same: one category system feeds pane, treemap, bars, donut, mindmap | P | QA-40 |

## 6. Selection, clipboard, integration

| ID | Reference behavior | Our spec | Status | Verify |
|---|---|---|---|---|
| WDS-CTX-01 | Context menu: **Open** (ShellExecute), **Explore** (opens Explorer at folder / selects file), **Command Prompt here** (opens terminal at path), **Copy Path**, **Delete to Recycle Bin**, **Delete Permanently**, **Properties** (Shell properties dialog) | All present (Windows Terminal/PowerShell/Cmd auto-detect for terminal item), plus our additions: Preview, Copy all paths of selection, Open with…, "Find in all views" | P+ | QA-50 |
| WDS-CTX-02 | Multi-select in tree (Ctrl/Shift) with aggregate size in status bar; context actions apply to selection | Multi-select in tree + table (marquee in table); aggregate readout in status bar; batch actions guarded by the cleanup ledger ([10 § Cleanup Safety](10-SCREENS-AND-FLOWS.md#cleanup-safety-the-deletion-contract)) | P+ | QA-51 |
| WDS-CTX-03 | Cleanup menu with: **Empty recycle bin**, **Delete temporary files** (system temp walk), **Delete to recycle bin/permanently** on selection | Cleanup tab subsumes these: Empty Recycle Bin action, "System temp files" preset, and per-selection delete with choice of bin/permanent + confirmation ledger | P+ | QA-52 |
| WDS-CTX-04 | Explorer integration none (reference is standalone) | **P+:** optional "Analyze with <product>" shell context-menu registration (per-machine install, opt-in checkbox in installer, removable from Settings) | P+ | QA-53 |
| WDS-CTX-05 | Drag rows out to Explorer (copy) / drag files in to locate | Drag-out (file copy via `startDrag` with file URLs), drag-in from Explorer = "reveal in current scan" (locate file, focus node) — improvement on both sides | P+ | QA-54 |
| WDS-CTX-06 | Copy path puts plain path on clipboard; Ctrl+C on selection | Same + "Copy as JSON/CSV" for multi-selection | P+ | QA-55 |
| WDS-CTX-07 | "Send mail to bug report" style integration — reference: report bug menu | In-app feedback (opens mail client with diagnostics bundle: version, OS build, GPU string, crash ID — no file data) | P+ | QA-56 |

## 7. Deletion semantics

| ID | Reference behavior | Our spec | Status | Verify |
|---|---|---|---|---|
| WDS-DEL-01 | Delete to recycle bin (default path); failure → error listing | Recycle-bin delete via `IFileOperation` with `FOF_ALLOWUNDO` + `FOF_NO_CONNECTED_ELEMENTS`, per-item error report, node removed from tree on success (sizes recomputed upward) | P | QA-60 |
| WDS-DEL-02 | Delete permanently (shift-delete semantics) with confirmation dialog stating count + total size | Same; confirmation ledger lists paths grouped by folder, shows total reclaimed estimate; "don't ask again for < 10 items < 100 MB" user option (default off) | P+ | QA-61 |
| WDS-DEL-03 | After delete: treemap + tree update immediately; free space node grows | Immediate arena removal + upward re-aggregation; free-space node updated from fresh volume query | P | QA-62 |
| WDS-DEL-04 | Deleting from recycle-bin context (empty bin) updates sizes | Empty-bin action refreshes `$Recycle.Bin` subtree + free space | P | QA-63 |
| WDS-DEL-05 | Reference warns when deleting inside system directories? — it does not (relies on UAC failing) | **P+:** hard block-list (Windows, Program Files, ProgramData roots, System Volume Information, `C:\` root itself, WinSxS) unless user types the folder name (mirrors our [10 § Cleanup Safety](10-SCREENS-AND-FLOWS.md#cleanup-safety-the-deletion-contract)) | P+ | QA-64 |

## 8. Progress, status, chrome

| ID | Reference behavior | Our spec | Status | Verify |
|---|---|---|---|---|
| WDS-CHROME-01 | Toolbar: scan controls (select drive/folder, refresh), configure, help/about | Our TopBar: New scan, Refresh, viz rail, search, theme, settings, account/license state; tooltips on all | P+ | QA-70 |
| WDS-CHROME-02 | Status bar: total scanned size, items, elapsed, current job | Status strip: selected node summary (name, logical, allocated, files, folders), scan totals, engine state (idle/scanning/paused), last action result toast anchor | P+ | QA-71 |
| WDS-CHROME-03 | Window layout (splitters, column widths, window size/pos) persisted | Persisted via electron-window-state + our layout store; restored on launch | P | QA-72 |
| WDS-CHROME-04 | Menu bar (File/Edit/View/Help equivalents in modern ribbon-less UI) | Full menu model preserved under: global menu (Alt), command palette (`Ctrl+K` — improvement), context menus; all reference functions reachable | P+ | QA-73 |
| WDS-CHROME-05 | About dialog: version, credits, license of components | About screen: version, channel, engine build hash, OSS attributions (auto-generated from SBOM), EULA link, privacy statement | P | QA-74 |
| WDS-CHROME-06 | Update check ("check for updates") with manual trigger | Auto + manual update via signed manifests ([15 § Auto-Update](15-BUILD-PACKAGING.md#6-auto-update-security)); channel-aware (stable/beta) | P+ | QA-75 |
| WDS-CHROME-07 | UI language switchable (reference ships ~40 translations) | **P+ scoped:** v1 ships `en-US` full + i18n architecture (ICU message format, RTL-safe layouts, pseudo-loc QA harness) so additional languages are additive; parity with *switching mechanism* demonstrated with one extra language pack (`de-DE`) at GA | P+ | QA-76 |
| WDS-CHROME-08 | High-DPI and multi-monitor correct rendering | Per-monitor DPI aware (Electron `zoomFactor` sync), mixed-DPI tested, dark/light title bar follows theme | P | QA-77 |

## 9. Configuration & persistence

| ID | Reference behavior | Our spec | Status | Verify |
|---|---|---|---|---|
| WDS-CFG-01 | All options persisted across sessions (scanner options, treemap options, colors, window layout) | Settings store: JSON under `%APPDATA%`, schema-versioned, migrations; every setting in this matrix has a persisted key | P | unit CFG-* |
| WDS-CFG-02 | "Show percentage bars in tree", "show grid/tile borders" style toggles | All reference-visible toggles exist in Settings with same semantics | P | QA-33 |
| WDS-CFG-03 | Settings reset to defaults | Settings → Reset (per-section and global) | P | QA-78 |

## 10. Performance & scale behavior (parity = never worse on these user-visible metrics)

| ID | Reference behavior | Our spec | Status | Verify |
|---|---|---|---|---|
| WDS-PERF-01 | Scans millions of files without crashing (reference handles ~millions slowly) | Hard budget: 8M files, ≤ 4 GB RAM, no crash; scan speed budget in [16](16-PERFORMANCE-BUDGETS.md#2-scan-budgets) | P+ | bench-scan |
| WDS-PERF-02 | UI stays responsive during scan | Renderer never blocks > 16 ms on scan events; all engine work off-thread ([05](05-IPC-PROTOCOL.md)) | P | bench-ui |
| WDS-PERF-03 | Scrolling large trees stays usable | Virtualized; scroll budget in [16](16-PERFORMANCE-BUDGETS.md#4-ui-budgets-renderer-r2-fix-l-active-scan) | P+ | bench-tree |
| WDS-PERF-04 | Treemap interaction responsive at 1M+ files | Hit-test + paint budget in [16](16-PERFORMANCE-BUDGETS.md#5-viz-budgets-vizcanvas-r2); culling + LOD by default | P+ | bench-treemap |

## 11. Deliberate divergences (owner-approved; each cites its rationale)

| ID | Divergence | Rationale |
|---|---|---|
| WDS-DIV-01 | Pac-Man scan motif → fade-in + luminance sweep | Own visual identity (R8); equal reassurance function |
| WDS-DIV-02 | Hard links double-counted → unique-bytes accounting with explicit mode switch | Correctness principle ([01 § 5.3](01-PRODUCT-VISION.md#5-product-principles-decision-tie-breakers)); a paid product must not miscount |
| WDS-DIV-03 | MFC dialogs → React overlays | Stack decision (R4) |
| WDS-DIV-04 | INI-based settings → JSON with schema migrations | Maintainability; no format debt |
| WDS-DIV-05 | Extension "descriptions" registry table → Shell type description with curated fallback | Better Windows-native data source, same user value |
| WDS-DIV-06 | Single treemap view → 9 visualization modes | Flagship differentiator ([11](11-VISUALIZATION-ENGINE.md), [12](12-HOMEGROWN-FEATURES.md)) |

## 12. Verification & sign-off protocol

1. Each QA-<n> script lives in `qa/scripts/QA-<n>.md` with exact steps, fixtures (the `fixtures/volumes/` synthetic trees, generated deterministically by `qa/mkfixtures.ps1`), and expected results.
2. A row moves to `P` only when: its automated tests (if any) pass on CI **and** a QA session records the manual script result in the phase checklist ([18-PHASE-PLAN](18-PHASE-PLAN.md)).
3. The parity dashboard (`npm run qa:parity`) renders this matrix with live statuses; GA gate requires 0 open rows ([18 Phase GA](18-PHASE-PLAN.md#phase-10--general-availability)).
