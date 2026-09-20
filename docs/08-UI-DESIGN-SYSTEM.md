# 08 — UI Design System

> Owner doc: [00-INDEX.md](00-INDEX.md) · Upstream: [01-PRODUCT-VISION](01-PRODUCT-VISION.md), [02-INTEL-DISKBUDDY](02-INTEL-DISKBUDDY.md) · Downstream: [09-UI-COMPONENTS](09-UI-COMPONENTS.md), [10-SCREENS-AND-FLOWS](10-SCREENS-AND-FLOWS.md), [11-VISUALIZATION-ENGINE](11-VISUALIZATION-ENGINE.md).
> Requirement IDs: `PRISM-DS-*`.
>
> **Identity thesis — "precision instrument."** The product behaves like a laboratory instrument that happens to be beautiful: dark-first surfaces like anodized metal and smoked glass, hairline bezels instead of drop shadows, tabular numerals like a multimeter, a jewel-tone data spectrum (it *is* called Prism) instead of pastels, and motion that feels like damped mechanical assemblies — fast, quiet, no bounce. This is deliberately **not** the warm-minimalist pastel direction of the competitive reference ([02 § 3.4](02-INTEL-DISKBUDDY.md#34-themes--visual-language)); our divergence is the brand.

---

## 1. Principles

1. **Content is chrome.** The user's data (tiles, bars, numbers) is the brightest thing on screen; UI chrome recedes. Chrome never uses saturated color; data always does.
2. **One accent.** Each theme has exactly one accent hue used for *interactive intent* (focus rings, primary buttons, active tabs, selection). Data colors are separate and never double as UI accents — this single rule keeps dense screens calm.
3. **Hairline over shadow.** Depth comes from 1px borders + surface luminance steps, not blur shadows (with three exceptions: menus/popovers, modals, toasts — see § 7 Elevation).
4. **Numbers align.** Every quantity renders in JetBrains Mono with `font-variant-numeric: tabular-nums`; all columns of figures right-aligned; units always present; every size in every context also shows its % of parent where ≥ 0.1%.
5. **Motion is information.** Animation communicates state change (layout ownership, hierarchy, staging into cleanup) or it is deleted. Global durations are short (§ 8); nothing loops unless it conveys progress.
6. **Dense by default, never cramped.** Two densities (§ 5); information density is a feature for our personas, but touch targets never drop below 28px height in lists and 32px for controls.
7. **Accessible contrast is non-negotiable.** AA minimum for all text (4.5:1), AAA (7:1) for body in all six themes, verified in CI with an automated contrast harness (§ 12).

## 2. Typography

| Role | Family | Notes |
|---|---|---|
| UI text | **Instrument Sans** (variable `wght 400–700`, + italic) | SIL OFL 1.1, bundled (`resources/fonts/`), registered per-document via `@font-face` with `font-display: block` (no FOUT on a desktop app). Distinctive grotesque with angular terminals — reads "instrument", not "startup template". |
| Data / code | **JetBrains Mono** (`wght 400–700`) | All numerals, all byte sizes, paths, extension chips, table sort keys. `tabular-nums` enforced. |
| (Reference note) | The competitor bundles Inter at 9 weights ([02 § 2](02-INTEL-DISKBUDDY.md#2-confirmed-technology-stack-and-what-it-implies)) — we deliberately do not ship Inter. | `PRISM-DS-002` |

### 2.1 Type scale (px, at 100% zoom / 96 dpi; scales with `zoomFactor` on DPI change — WDS-CHROME-08)

| Token | Size/Line | Weight | Tracking | Family | Usage |
|---|---|---|---|---|---|
| `text-2xs` | 10/14 | 500 | +0.06em | Instrument | axis ticks, badges |
| `text-xs` | 11/16 | 400–500 | +0.01em | Instrument | table metadata, hints |
| `text-sm` | 12.5/18 | 400 | 0 | Instrument | table body (comfortable), tree rows |
| `text-sm` mono | 12.5/18 | 400 | 0 | JetBrains | all numerals in tables |
| `text-base` | 13.5/20 | 400 | 0 | Instrument | default body |
| `text-md` | 15/22 | 400 | 0 | Instrument | inspector body, dialogs |
| `text-lg` | 18/26 | 550 | −0.01em | Instrument | section titles, dialog titles |
| `text-xl` | 22/30 | 600 | −0.015em | Instrument | screen titles (Explore header) |
| `text-2xl` | 28/36 | 650 | −0.02em | Instrument | hero metrics (welcome cards, gauge center) |
| `text-num-hero` | 34/40 | 500 | 0 | JetBrains | big byte readouts (gauge center, reclaim preview) — always with `text-2xs` unit suffix in muted color |

Rules: no font sizes outside tokens; headings never skip two levels; ALL-CAPS only for `text-2xs`/`text-xs` section labels with `letter-spacing +0.08em` and `color: text-muted` (e.g., `QUICK WINS`, `LARGEST INSIDE` — the micro-label pattern); ellipsis for overflow is `…` with full value in tooltip.

## 3. Themes

Six themes, **four dark-first** + **two light**. Names are our own; palettes defined in OKLCH (device-independent, perceptually tuned) with hex fallbacks rendered by the token pipeline. A theme = one JSON file (`packages/ui/src/themes/<name>.tokens.json`); adding a theme must not require component edits.

| Theme | Mode | Character | Accent |
|---|---|---|---|
| **Nocturne** *(default)* | dark | Deep indigo-charcoal "smoked glass"; the flagship look | Glacial cyan |
| **Graphite** | dark | Neutral near-black "anodized"; executive, monochrome-plus | Warm ivory |
| **Verdigris** | dark | Green-tinted slate "oxidized metal" | Patina copper |
| **Ember** | dark | Warm charcoal "forge" | Ember orange |
| **Alabaster** | light | Cool porcelain "gallery wall" | Deep teal |
| **Terracotta** | light | Warm clay paper "atelier" | Burnt sienna |

### 3.1 Core token table (Nocturne shown in full; other themes ship the same key set)

```
surface-app        oklch(0.145 0.012 264)   #17181f   app background
surface-panel      oklch(0.185 0.014 264)   #1f2029   sidebars, inspector
surface-raised     oklch(0.225 0.016 264)   #282a35   cards, table headers
surface-inset      oklch(0.120 0.010 264)   #131419   wells: code blocks, gauges track, treemap canvas bg
surface-overlay    oklch(0.250 0.018 264)   #2e313d   popovers/menus (over surface-*)
surface-hover      rgba(255 255 255 / 0.05)
surface-active     rgba(255 255 255 / 0.08)
border-hairline    oklch(0.320 0.014 264)   #3a3d4d   1px chrome borders
border-strong      oklch(0.420 0.016 264)   #565a6e   inputs, focus pre-ring
border-data        rgba(7 9 14 / 0.85)      viz tile gaps (drawn on canvas)
text-primary       oklch(0.955 0.004 264)   #f2f3f7
text-secondary     oklch(0.760 0.008 264)   #b4b7c4
text-muted         oklch(0.590 0.010 264)   #858a9c
text-faint         oklch(0.450 0.010 264)   #63677a   placeholders, axis ticks
accent             oklch(0.845 0.135 205)   #53d7f0   glacial cyan
accent-hover       oklch(0.885 0.120 205)   #7ae2f6
accent-pressed     oklch(0.800 0.130 205)   #3fc9e4
accent-contrast    oklch(0.145 0.012 264)             text on accent (dark)
accent-subtle      oklch(0.845 0.135 205 / 0.14)       selected-row tint, chips
focus-ring         accent at 60% + 2px offset ring
signal-success     oklch(0.780 0.150 155)   #4ad07f
signal-warning     oklch(0.830 0.145 85)    #f0b23e
signal-danger      oklch(0.680 0.190 25)    #e5533d   (also "staged for deletion" tint)
signal-info        accent
```

Full palettes for the other five themes live in `packages/ui/src/themes/*.tokens.json` (authoritative); their design constraints (not free variables):

- Dark themes: `surface-app` L ∈ [0.13, 0.17]; `text-primary` L ≥ 0.94; data palette (§ 4) L compressed into [0.55, 0.85] so tiles glow against chrome without glare.
- Light themes: `surface-app` L ∈ [0.955, 0.975] (paper-warm, never pure white; pure white reserved for raised cards); borders at L ≈ 0.88; data palette L ∈ [0.45, 0.72].
- Accent in every theme must pass 4.5:1 against its `surface-raised` for text usage — CI contrast harness enforces (§ 12).

`PRISM-DS-010` — Theme switching is instant (CSS custom properties swap; no reload), applies to canvas visualizations via the same tokens (the token provider pushes a `DataPalette` struct to the viz layer, [11 § Palettes](11-VISUALIZATION-ENGINE.md#5-palettes)), and persists (WDS-CFG-01). The title bar and native frame follow the theme (`titleBarOverlay` colors on Windows).

### 3.2 Divergence from references (compliance with [02 `PRISM-INTEL-012`](02-INTEL-DISKBUDDY.md#4-what-we-take-what-we-change-summary-contract))

No Inter, no pastel warm-cream base, no material-name theme set (`paper/linen/sage/mist/slate`), no macOS-gray secondary ramp. Our theme names, hue anchors, ramps, and component styling are original. The only structural borrowings are the *concepts*: multi-theme support, type/branch/age color-mode rail, and a legend as single source of truth.

## 4. Data colors (the spectrum)

Data colors are **theme-relative ramps over fixed hue anchors** — the same category keeps its hue in every theme; only lightness/chroma adapt. Defined in OKLCH; the build renders per-theme hex into `DataPalette`.

### 4.1 The 12-hue categorical spectrum (anchor hues, Nocturne rendering)

```
hue anchors (° in OKLCH space) and Nocturne chroma/L ranges:
cyan 200 · azure 250 · violet 300 · magenta 350 · rose 15 · coral 40
amber 75 · lime-green 135 · green 155 · teal 185 · slate-blue 265 · mauve 320
```

Category assignment (curated table, ~40 categories → 12 anchor hues by semantic grouping, adjacent categories never share a hue):

| Group | Categories (→ anchor hue) |
|---|---|
| Video (coral) | video, animation, screen-recordings |
| Audio (magenta) | audio, music-projects, podcasts |
| Images (rose) | images, raw-photos, design-assets |
| Documents (slate-blue) | documents, spreadsheets, presentations, pdf, ebooks |
| Archives (amber) | archives, disk-images, backups |
| Installers (orange/coral-40) | installers, packages, update-caches |
| Developer (azure) | source-code, build-artifacts, package-managers, version-control, databases |
| Virtualization (violet) | virtual-machines, container-images, emulator-images |
| Games (lime) | games, game-assets, shader-caches |
| System (slate-blue-265 desat.) | system, drivers, fonts, certificates |
| Mail (teal) | mail-stores, attachments |
| Caches & logs (mauve) | caches, logs, crash-dumps, temp |
| Free space (special) | neutral: theme `border-hairline` at 40% — always recessive |
| Unknown (special) | `signal-warning` at 60% with diagonal hatch pattern on canvas |

`PRISM-DS-020` — Every categorical rendering (treemap, sunburst, mindmap, donut, type chips, table swatches, folder-card tints) draws from this **single** `DataPalette` — one legend for the whole app (WDS-EXT-05). Per-extension overrides (WDS-EXT-03) write through to the palette layer, versioning it (`viz:color-mapping` version bump).

### 4.2 Color modes (viz rail — concept parity with [02 § 3.2](02-INTEL-DISKBUDDY.md#32-the-nine-visualizations-vizmode-cases))

| Mode | Encoding |
|---|---|
| **By type** | § 4.1 spectrum by category |
| **By branch** | depth-stable hue hashing of top-level branches: hash(branchPath) → 12 anchors; descendants ramp toward neutral with depth (L +0.03/level, C ×0.85/level) — trees stay visually grouped |
| **By age** | § 4.3 ramp on mtime |

### 4.3 Age ramp (mtime → color, shared by age viz, age color mode, stale badges)

```
0–7d      oklch(0.80 0.15 155)  #46d17e  fresh green
7–30d     oklch(0.78 0.13 120)  #96c94e  young lime
30–90d    oklch(0.80 0.13 85)   #e8b83f  maturing amber
90d–1y    oklch(0.76 0.11 60)   #d9963a  aging ochre
1–2y      oklch(0.66 0.08 40)   #b0713a  old copper
>2y       oklch(0.52 0.03 60)   #776a55  ancient umber (near-neutral)
unknown   text-faint
```

Design intent (deliberate divergence from the reference's green→brown→grey): our ramp ends in *umber, not grey* — old data stays visible as "sediment" while receding; thresholds are tokenized (`age-bucket-edges`) and reused by stale rules ([12 § Stale Rules](12-HOMEGROWN-FEATURES.md#3-stale-rules)) so color and policy can never disagree.

### 4.4 Sequential scales (charts)

Single-hue lightness ramps derived from the theme accent (7 steps, L 0.35→0.90) for bar charts of a single measure; never rainbow. Diverging (delta bars): teal↔accent for ±, clamped saturation; zero = border color.

## 5. Spacing, sizing, density

- **Base grid 4px.** Spacing tokens `sp-1..12` = 4,8,12,16,20,24,32,40,48,64,80,96. No off-grid values in layouts (icons at odd sizes are centered on grid).
- **Density modes** (setting, persisted): `comfortable` (row 36px, control 32px, table cell padding-y 8px) and `compact` (row 28px, control 28px, padding-y 5px). Default follows first-run screen resolution (≤ 1080p height → compact).
- **Layout constants:** sidebar 264px (collapsible to 56px icon rail), inspector 300px (320px on ≥ 1600px viewport), top bar 48px, status strip 28px, gutter between panes 1px hairline (not floating panels — the app is one machined surface, sections divided by hairlines).

## 6. Radius

| Token | px | Use |
|---|---|---|
| `r-xs` | 3 | checkboxes, tiny chips, canvas tile corners (canvas uses 2px) |
| `r-sm` | 5 | buttons, inputs, selects, extension chips |
| `r-md` | 8 | cards, table containers, inspector sections |
| `r-lg` | 12 | dialogs, sheets, welcome hero cards |
| `r-full` | 999 | pills, toggle tracks |

The sharp-ish scale is identity (§ 0 "instrument"); circular oversized radii are banned on data UI.

## 7. Elevation (three exceptions to hairline rule)

| Level | Shadow | Used by |
|---|---|---|
| `e-1` | `0 2px 8px rgba(0,0,0,.28), 0 0 1px rgba(0,0,0,.35)` | menus, popovers, tooltips |
| `e-2` | `0 8px 28px rgba(0,0,0,.34), 0 0 1px rgba(0,0,0,.4)` | modals, command palette |
| `e-3` | `0 16px 48px rgba(0,0,0,.4)` | toasts (rare) |

Light themes use the same shadows at 60% opacity + a hairline outline. Everything else (cards, panels) uses borders and surface steps only.

## 8. Motion

| Token | Duration | Curve | Use |
|---|---|---|---|
| `t-instant` | 90ms | `cubic-bezier(.2,0,0,1)` | hover states, swatch tints |
| `t-fast` | 140ms | `cubic-bezier(.2,0,0,1)` | toggles, chips, small reveals |
| `t-base` | 200ms | `cubic-bezier(.05,.7,.1,1)` | section transitions, dialogs, sheet |
| `t-slow` | 280ms | `cubic-bezier(.05,.7,.1,1)` | screen-level transitions (scan → explore), hero number count-ups |
| `t-viz` | 200ms | `cubic-bezier(.2,0,0,1)` | viz zoom/relayout eased in **Rust-computed target frames** (layout tweened on canvas, [11 § Zoom](11-VISUALIZATION-ENGINE.md#43-zoom-interaction-all-geometric-modes)) |

Rules: max **one** `t-slow` animation visible at a time; entrance staggers ≤ 24ms per item, cap 6 items; no spring bounce anywhere; `prefers-reduced-motion` collapses all durations to 0 except opacity fades (functional feedback preserved); scanning progress motif ([WDS-SCN-03 substitution](03-PORTING-MATRIX-WINDIRSTAT.md#2-scanning-behavior)) is a luminance sweep — implemented on canvas, GPU-composited, zero layout cost.

## 9. Iconography

- **Lucide** only (consistency of stroke system), sizes 14/16/20, stroke 1.5px (1.75px at 20 for optical balance), `currentColor`.
- Custom glyphs (14): the six viz-mode glyphs, category glyphs for the type list, drive states — drawn by us on the 24-grid, 1.5px stroke, same optical family; SVGs live in `packages/ui/src/icons/`.
- Badges (junction/sparse/hardlink/package/system): 12px square outline glyphs, `text-muted`, tooltip explains.

## 10. Copy & voice (summary — full in [10 § Voice](10-SCREENS-AND-FLOWS.md#voice))

Sentence case everywhere (no Title Case buttons); verbs first on actions; every destructive confirm states count + bytes; empty states = 1 line of empathy + 1 line of action; humor dry, never cute; units per § 11.

## 11. Numbers & formatting (normative — used by [ByteFormat](09-UI-COMPONENTS.md#byteformat))

- SI decimal by default: `kB, MB, GB, TB` (1 kB = 1000 B) with `0–1 fraction digits` by magnitude (< 10 → 1 digit; ≥ 100 → 0 digits; e.g. `4.2 MB`, `118 GB`, `1.9 TB`); binary toggle (`KiB…`) in Settings, applies app-wide.
- Counts: thousands separators; exact counts in tooltips (`1,284,396 files`).
- Percentages: 1 decimal under 10%, 0 above (`0.4%`, `12%`); `< 0.1%` shows `< 0.1%`.
- Durations: `12s`, `4m 03s`, `1h 22m`; rates: `1.24M files/s`.

## 12. Accessibility & theming enforcement (CI)

`PRISM-DS-030` — The token pipeline (`script/tokens.ts`) runs in CI and fails on: (a) any text/background pair below AA (AAA for body) per theme; (b) focus-visible ring contrast < 3:1 anywhere; (c) a component consuming a raw hex instead of a token (lint rule `no-raw-colors`); (d) a theme missing any required key. The palette export for canvas is generated in the same pipeline (single source of truth for DOM + canvas + title bar).
