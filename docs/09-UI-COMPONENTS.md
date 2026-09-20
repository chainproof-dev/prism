# 09 — UI Component Library

> Owner doc: [00-INDEX.md](00-INDEX.md) · Upstream: [08-UI-DESIGN-SYSTEM](08-UI-DESIGN-SYSTEM.md) · Downstream: [10-SCREENS-AND-FLOWS](10-SCREENS-AND-FLOWS.md), [11-VISUALIZATION-ENGINE](11-VISUALIZATION-ENGINE.md), [12-HOMEGROWN-FEATURES](12-HOMEGROWN-FEATURES.md).
> Requirement IDs: `PRISM-CMP-*`.

---

## 1. Sourcing ratios (owner-mandated, R5)

| Source | Share | Rule |
|---|---|---|
| **shadcn/ui source-owned + Radix primitives** | ~70–80% | `packages/ui` contains the vendored, **owned** shadcn sources (we edit them freely; updates are deliberate cherry-picks, never blind pulls). Radix provides a11y behavior (dialog focus trap, roving tabindex, etc.). |
| **Selective premium/animated components** | ~10–20% | Only where a "moment" is earned: welcome hero, scan completion, activation success, viz mode transitions. License-audited before adoption ([20 § License Audit](20-DEPENDENCY-MANIFEST.md#5-license-audit-rule-spdx-field-required-in-every-manifest-row)); no marketing-page effect ships into data screens (per [02 § caution](02-INTEL-DISKBUDDY.md#2-confirmed-technology-stack-and-what-it-implies)). |
| **Bespoke Prism components** | remainder | Everything data-specific: visualizations, tree, tables, gauges, inspector, cleanup ledger. Hand-built, no library substitutes. |

Forbidden: pulling a whole component *library* skin on top (MUI/Chakra/NextUI); mixing icon sets; default-palette usage (`no-raw-colors` lint, [08 § 12](08-UI-DESIGN-SYSTEM.md#12-accessibility--theming-enforcement-ci)).

## 2. Library inventory

### 2.1 Primitives (owned shadcn/Radix — customized)

`Button` · `IconButton` · `ToggleButton` · `SegmentedControl` (custom, replaces tabs for viz rail) · `Tooltip` (400ms delay, 150ms hide) · `Popover` · `ContextMenu` (Radix menu, our items API) · `Dialog` · `Sheet` (right, for settings) · `Select` · `Combobox` (cmd palette base) · `Checkbox` · `Switch` · `Slider` (treemap depth, brightness) · `Tabs` (settings) · `ScrollArea` (overlay scrollbars, 8px, fade 300ms) · `Toast` (sonner-based, our tokens) · `Kbd` · `Separator` · `Badge` · `Progress` · `Skeleton`.

Every primitive spec = one markdown card in this file with: props table, states (default/hover/active/focus/disabled/loading), density variants, a11y contract (roles, keys), and a screenshot-regression test ID (`shot-cmp-<name>`, [17 § Visual Regression](17-QUALITY-ENGINEERING.md#visual-regression)). Full prop tables live in `packages/ui/README.md` (kept in sync by CI doc-gen); this doc fixes the *design contract*.

### 2.2 Prism components (bespoke — the heart)

#### AppShell
The machined single-surface frame: `TopBar` / `Sidebar` / `Center` / `Inspector` / `StatusStrip` regions separated by hairlines ([08 § 5](08-UI-DESIGN-SYSTEM.md#5-spacing-sizing-density)). Props: `regions` config (which panes visible per screen), `density`. Owns global keyboard scope routing (see [10 § Keyboard Map](10-SCREENS-AND-FLOWS.md#14-keyboard-map-complete--parity-wds-tre-05chrome-04--ours)). Collapsible sidebar (icon rail 56px with tooltips), inspector toggle (`Ctrl+I`, WDS-CTX parity via inspector).

#### TopBar (48px)
Left: app glyph + `WorkspaceTabs` (Explore · Duplicates · Applications · Monitor · Snapshots · Leftovers — premium tabs show a lock glyph until licensed, [10 § Tabs](10-SCREENS-AND-FLOWS.md#workspace-tabs)). Center: breadcrumb (zoom path) with `‹ Back to scan root`. Right: search field (240px, grows to 360 on focus), viz-mode segmented rail (Explore only), color-mode segmented control, theme menu, account/license chip, `⋯` overflow (settings, about, feedback, update channel). All controls 32px hit area, tooltips complete.

#### Sidebar (264px)
Sections (collapsible, order persists): **Scan actions** (New scan, Refresh, recent targets dropdown) · **Disk gauge** (DonutGauge of current scan root vs volume: used/free/unknown with center hero number) · **Current view** (path, Reveal, Copy path, Include-everything-below toggle) · **Quick Wins** (top 6 reclaim candidates: name, bytes, source chip preset/stale/duplicate; click = focus + stage affordance) · **Types** (TypeList, WDS-EXT-01) · **Cleanup queue** (persistent mini-panel when non-empty: count, bytes, Review button — pulses once on add). Section headers = micro-label pattern (`text-2xs` caps). Each section independently scrollable within one Sidebar scroll (sticky headers).

#### Inspector (300/320px)
Context panel bound to selection ([10 § Inspector](10-SCREENS-AND-FLOWS.md#inspector)): header (glyph or icon, name, kind + category chip, badges), metrics grid (Logical / Allocated / Unique big numerals with % of parent), attributes list (times created/modified/accessed, attributes, extension, links count, package contents toggle), **Largest Inside** (top 10 children, mini-bar per row, click = select+scroll), actions row (Open, Reveal, Preview, Copy path, Delete). Empty state: "Select something to inspect it." + a directional hint arrow into the viz.

#### FileTree (directory tree — WDS-TRE-*)
Virtualized (TanStack Virtual) tree table. Columns: name+icon | % bar | size | allocated | files | folders. Sort by header (engine-side, `tree:children` sort spec). Rows: indent 16px/level with continuous guide hairlines at 30% opacity; disclosure chevron 16px; badges (junction/sparse/link/system); % bar = category color at 80% width-proportional, 3px tall, rounded 2px. Multi-select: Ctrl/Shift + marquee on the % bar column gutter. Keyboard: full map (WDS-TRE-05). Type-ahead search (300ms clear window) with inline highlight. Row height per density; selection = `accent-subtle` fill + 2px `accent` left inset; hover = `surface-hover`. Files leaf rows: icon + name + size mono; double-click = open (ShellExecute); folder double-click = zoom (option: rescan, WDS-TRE-06).

#### FileTable (TanStack Table 9 — ranked/big lists)
Used for: top-sizes table, duplicates groups, app uninstaller list, leftover list, snapshot diff rows, errors drawer. Contract: column defs typed per use; server-(engine-)side sorting & paging where volume > 10k rows; client-side for small sets; row virtualization always on; column widths drag-resizable with persisted state per table id; sticky header; sortable indicators (arrow + ordinal when multi-sort); zebra OFF (hairline row separators at 40%); numeric columns right-aligned mono ([08 § 1.4](08-UI-DESIGN-SYSTEM.md#1-principles)); selection column with tri-state header checkbox; group headers sticky for duplicate groups; keyboard: `↑/↓` move, `Space` toggle row, `Enter` default action, `Ctrl+A` page-select, type-ahead on name column. Duplicates-specific: group summary row (n items, total reclaimable, "stage extras" button), per-item path with jump-to-in-tree link, partial/full hash status chips.

#### ByteFormat
`<ByteFormat bytes={bigint} mode="logical|allocated|unique" variant="auto|compact|exact|bytes" />` — renders per [08 § 11](08-UI-DESIGN-SYSTEM.md#11-numbers--formatting-normative--used-by-byteformat); `exact` shows `1,234,567,890 bytes` in tooltip; `auto` = SI with unit suffix; always `tabular-nums`; accepts `percentOf` to append % of a parent. Localization-ready (unit strings from ICU bundles).

#### DonutGauge
SVG ring (not canvas) for small counts: 3 segments (used/unknown/free), 3px gap arcs, center = hero number (ByteFormat) + subtitle. Animated sweep on scan completion (t-slow, once). Used in sidebar + welcome.

#### TypeList (WDS-EXT-01)
Rows: swatch (10px, category color), extension chip (mono, `text-xs`), description (muted), file count, total size, % bar mini. Multi-select via checkboxes → filters all views (dimming, WDS-EXT-02). Sort: by size desc default / count / name. Footer link → "Edit colors & descriptions" (settings section, WDS-EXT-03/04).

#### VizCanvas (host for [11](11-VISUALIZATION-ENGINE.md))
One component, nine modes. Owns: DPR-correct canvas sizing, rAF render loop (only while dirty), pointer handling (hover/selection/zoom per mode), LOD culling params, frame decode from `VizFrame` binary ([05 § 5](05-IPC-PROTOCOL.md#5-binary-payloads-vizframe)), palette application, label painter, highlight overlay, legend coordination. **Never** holds React state per tile — mode-level state only (`hoveredId`, `selectedId`, `zoomRoot`, `viewport`).

#### HoverReadout
Floating pill (elevated `e-1`) anchored to pointer with 8px offset, content: name (medium) · path (muted, truncated middle, mono `text-xs`) · size + % of parent + % of scan · mtime relative ("2 years ago"). Never overlaps cursor; flips at edges; 120ms show delay, instant hide; updates at pointer-move without re-render of parent (direct DOM write on a portal).

#### MetricCard / StatRow / MiniBar / Sparkline
Dashboard atoms for monitor + welcome: MetricCard (label micro, value hero mono, delta chip ±%, optional sparkline), StatRow (label/value/mini-bar), MiniBar (4px, category color, width % of max sibling), Sparkline (SVG polyline, 60 samples, gradient-free stroke in accent, 1.5px).

#### CleanupBar (ledger — [10 § Cleanup Safety](10-SCREENS-AND-FLOWS.md#cleanup-safety-the-deletion-contract))
Sticky bottom bar when queue non-empty: `n items · X reclaimable` + breakdown chips by source (preset/duplicate/stale/manual) + `Review` (opens ledger sheet) + `Execute` (danger-styled, opens confirm) + `Clear`. Ledger sheet: grouped tree of staged paths with per-item remove, size totals per group, blocked-list warnings ([WDS-DEL-05](03-PORTING-MATRIX-WINDIRSTAT.md#7-deletion-semantics)), search-within-staged.

#### CommandPalette (`Ctrl+K`)
cmdk-based on our tokens: actions (all menu commands), navigation (tabs, screens), recent paths, quick settings toggles (theme, density, size mode). Fuzzy, keyboard-first, `e-2` overlay, 200ms open.

#### Banner (contextual, dismissible)
Full-width, 40px, severity-tinted left border 2px: license grace countdown, scan errors summary ("37 locations couldn't be read — Review"), update available. Never stacks more than one; lowest severity yields.

#### ActivationCard / LicenseGate
[13 § Activation UX](13-LICENSING-SYSTEM.md#51-activation): full-screen gate pre-license; centered card (max 440px): product glyph, key field (mono, paste-friendly, auto-groups XXXX-XXXX-XXXX-XXXX), activate button (accent, loading state with spinner 16px), trial-status line, "Buy" (external checkout), footer links (resend key, support). Error states map 1:1 from server responses (already-active / not-found / rate-limited / offline) with actionable next step each.

### 2.3 Premium/animated moments (the 10–20%)
Used exactly where listed — nowhere else: welcome drive-card entrance stagger (6 items max, 24ms stagger); scan completion → explore transition (one `t-slow` crossfade + gauge sweep); staging-to-cleanup micro-interaction (row flies to queue chip — 200ms, reduced-motion: none); activation success checkmark draw (240ms, once). All `motion` (Framer v12) driven, GPU transform/opacity only.

## 3. Component engineering rules

`PRISM-CMP-001` — Every component: typed props (no `any`), forwardRef on interactive primitives, `data-testid` stable ids, keyboard contract documented in-file, visual states covered by screenshot tests, and storybook stories (interactive, all states — storybook is a dev tool, not shipped).
`PRISM-CMP-002` — Lists > 200 rows must virtualize (rule of the codebase; CI perf test scans for non-virtualized long lists in e2e fixtures).
`PRISM-CMP-003` — No component fetches IPC directly; data access via store hooks only ([10 § Stores](10-SCREENS-AND-FLOWS.md#15-stores-state-architecture)).
`PRISM-CMP-004` — All interactive elements: visible focus (2px accent ring, 2px offset) — never `outline: none` without replacement.
`PRISM-CMP-005` — Text overflow: middle-truncation for paths, end-truncation for names; tooltips always carry the full string; copy actions always use the full string.
