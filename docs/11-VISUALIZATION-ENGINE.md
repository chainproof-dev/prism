# 11 — Visualization Engine

> Owner doc: [00-INDEX.md](00-INDEX.md) · Upstream: [08-UI-DESIGN-SYSTEM](08-UI-DESIGN-SYSTEM.md), [09-UI-COMPONENTS](09-UI-COMPONENTS.md#vizcanvas-host-for-11) · Downstream: [16-PERFORMANCE-BUDGETS](16-PERFORMANCE-BUDGETS.md), [20-DEPENDENCY-MANIFEST](20-DEPENDENCY-MANIFEST.md).
> Requirement IDs: `PRISM-VIZ-*`.
>
> Nine modes, one interaction grammar (hover → readout; click → select; dbl-click → zoom; wheel → zoom; right-click → node menu). All layouts computed **in Rust** (deterministic, testable, off the UI thread); all painting in the renderer on canvas. The DOM never renders data viz; Recharts is used only for small auxiliary charts (monitor sparklines/areas, welcome gauges are SVG atoms) — never for treemap/sunburst/icicle/pack/mindmap ([ADR-07](04-SYSTEM-ARCHITECTURE.md#adr-07--renderer-tech-pinned)).

---

## 1. Mode catalogue

| # | Mode | What it answers | Layout algorithm (Rust) | Renderer |
|---|---|---|---|---|
| 1 | **Treemap** *(default)* | "What's big, right here, in place" (parity WDS-TMP-*) | Squarified + cushion height-field | Canvas2D ≤ 50k visible tiles → WebGL2 above (auto, measured) |
| 2 | **Sunburst** | "How is space split by ring/level" | Radial icicle (angle ∝ size, rings = depth) | Canvas2D |
| 3 | **Icicle** | "Structure top-down, size left-to-right" | Width ∝ size, rows = depth | Canvas2D |
| 4 | **Circle pack** | "Nested proportions, softer read" | Aurenhammerer-style pack: sibling circle pack via deterministic smallest-enclosing placement | Canvas2D |
| 5 | **Mindmap** | "Branches from root, weighted" | Radial tree: angular allocation ∝ subtree size, radial spacing by depth band | Canvas2D |
| 6 | **Folders** | "One level at a time, cards" | Grid cards (no layout math; engine supplies children + sizes) | DOM (cards — small counts only) |
| 7 | **Table** | "The biggest items, ranked" | Top-N sort per scope | DOM (FileTable) |
| 8 | **Bars** | "Ranked bars, quick scan" | Top-N bar rows | DOM |
| 9 | **Age timeline** | "Where bytes sit in time" | Byte-weighted histogram by mtime bucket (§ 5) | Canvas2D |

Every mode ships: HoverReadout, selection highlight overlay, dim-by-filter, keyboard focus ring equivalent (selected node outline + breadcrumb announce for screen readers), zoom/pan model (where meaningful), and legend sync from `DataPalette` (WDS-EXT-05).

## 2. Renderer architecture

### 2.1 VizCanvas pipeline

```
viz:layout(scanId, mode, root, viewport, options)
  → Rust: cull (LOD § 3) → layout → VizFrame binary [05 § 5]
  → VizCanvas: decode to typed arrays → dirty flag → rAF paint → cached until (root|viewport|palette|filter|selection-external) changes
```

- **Frame cache:** LRU 4 frames per mode; zoom animation interpolates between two computed frames (§ 4.3).
- **Repaint triggers:** hover (overlay layer only — never relayouts), selection, palette version bump, filter version, resize (ResizeObserver → relayout request, coalesced to rAF).
- **Paint layers (3 stacked canvases):** L0 data (tiles/arcs/nodes), L1 overlay (hover/selection/filter-dim — redrawn on pointer move, ≤ 2 ms budget), L2 labels (text; repainted only when L0 or zoom changes). Layers keep interaction 60 fps even when a full data repaint costs 8–12 ms.
- **DPR:** canvas backing store = CSS px × `devicePixelRatio` (capped 2.0 for battery; setting to exceed); all coordinates in device px internally.
- **Text:** labels painted with `letter-spacing` emulation via measured runs; JetBrains Mono for sizes inside tiles, Instrument Sans for names; label collision solver (§ 3.3).

### 2.2 WebGL2 backend (treemap only, ≥ 50k visible tiles)

Instanced quads (1 draw call per frame): per-instance `vec4 rect`, `u32 colorARGB`, `u32 meta`; cushion shading in fragment shader (height-field normal from rect + elevation uniform, [§ 4.1](#41-treemap-parity-contract-wds-tmp-0111)); labels still Canvas2D on L2. Selection: instance highlight via meta bit. Hit-testing stays CPU-side (same spatial index — GPU picking rejected: readback stalls).

Activation rule is **measured**, not configured: the first frames of a treemap view are painted Canvas2D while a background probe measures paint time; if rolling p95 > 6 ms at ≥ 50k tiles, VizCanvas switches to WebGL2 for that frame size class (result cached per machine). The switch is logged (diagnostics) and identical output is verified by screenshot parity test in CI on both backends.

## 3. Level of detail (LOD) — the scale rules

`PRISM-VIZ-010` — Engine culling per frame (values are defaults; Settings → Treemap/viz can raise them at user's own perf cost):

| Param | Default | Effect |
|---|---|---|
| `maxTiles` (treemap/icicle) | 60,000 | smaller siblings aggregate into `<small items>` composites per parent |
| `maxArcs` (sunburst) | 12,000 | outer rings aggregate |
| `maxCircles` (pack) | 8,000 | |
| `maxGraphNodes` (mindmap) | 6,000 | |
| `minTilePx` | 3.0 | below → aggregated into parent composite |
| `minShare` | 0.0004 (0.04%) | below → composite |
| `drawnDepth` | 6 (slider 2–12) | depth cutoff per mode |
| label rules | § 3.3 | |

Composites are **real nodes** (selectable, inspectable, expandable by zoom) — never invisible data. Aggregation is deterministic (size-desc order), so frames are reproducible in tests (golden-frame hashes per fixture).

### 3.3 Label painter rules

Show a label when: tile ≥ 28px tall ∧ ≥ 42px wide (mode-adjusted) ∧ not occluded. Priority order: selected > hovered > larger size. Collision: scanline sweep over placed rects; loser drops. Content: name (truncated to 60% width, middle-ellipsis) + size when ≥ 56px height. Fade labels in at zoom-end (t-viz), never during motion.

## 4. Mode specifications

### 4.1 Treemap (parity contract WDS-TMP-01..11)

- **Layout:** squarified (Bruls–Huizing–van Wijk) over `allocated` (or active size mode) with aspect-ratio target 1:1, worst-case 4:1 guard; children sorted size-desc; padding: 1px inner gap default (0–3 setting, WDS-TMP-07); composites per § 3.
- **Cushion:** height-field per tile: `z(x,y) = e·(1−x̂)(1−ŷ)` ridge per ancestor level (elevation e scales by depth: `e_d = E·f^d`, defaults E=0.55, f=0.66 — brightness/highlight settings map to E and the light model's specular gain). Rendered Canvas2D as per-pixel lambert on an offscreen ImageData pass for ≤ 8k tiles, else precomputed per-tile radial gradient (visually equivalent, O(tiles) not O(px)); WebGL path uses the true height field. The cushion is **our own implementation from the published paper** (van Wijk & van de Wetering, "Cushion Treemaps") — clean-room per [03 § 0](03-PORTING-MATRIX-WINDIRSTAT.md#0-clean-room-protocol).
- **Zoom:** dbl-click directory → new frame rooted there with parent frame ring at 12% luminance offset ("bezel" — parity WDS-TMP-06 with our styling); `Alt+↑`/breadcrumb out; Ctrl+wheel continuous zoom crossing depth boundaries by auto-selecting the hovered chain (target-frame interpolation § 4.3).
- **Free space / unknown:** neutral recessive tile with hatch pattern (unknown) and plain recessive fill (free) — never category colors (they aren't data).
- **Hover:** 1.5px accent outline + HoverReadout; hit-test = uniform grid index (64×48 cells) built at frame decode, O(k) candidates.

### 4.2 Sunburst
Angle ∝ share; ring thickness constant per level (28px) down to viewport min; inner rings clip before outer; arcs ≥ 1.2° get stroke hairline `border-data`; labels: radial for arcs ≥ 14° and ≥ 44px length, horizontal at ring 0. Center disc: current root name + size (click center = zoom out). Rotation: none by default (stability > cute), optional 15° step rotate via drag with Alt.

### 4.3 Zoom interaction (all geometric modes)
Two-frame tween: engine returns frame A (current) and frame B (target) plus a **node→both-rects mapping**; renderer interpolates rects with `t-viz` (200 ms, instrument curve) on the overlay timeline; interrupted zoom re-targets from current interpolation state (no restart glitch). This is why layout stays in Rust: the mapping is cheap there, impossible to reconstruct reliably in JS for 60k tiles.

### 4.4 Icicle
Rows = depth (row height 26px, auto-grows to fill height for shallow trees), width ∝ size; same LOD/cushion-free flat fills; depth ruler on the left (micro labels); flame-gradient **forbidden** — flat category colors only (identity: we are not a profiler flame graph, and heat colors would fight the age ramp).

### 4.5 Circle pack
Deterministic packing: children circles packed by size-desc into parent circle via forward-greedy smallest-enclosing-circle placement (Welzl base); gap 2px; labels at ≥ 40px diameter; deepest visible circles get 1px `border-data` stroke. Same hit-test grid over circle bounds.

### 4.6 Mindmap
Radial tree: root center; level-1 branches placed at angles ∝ subtree share (clockwise, stable order = size-desc); each level allocates angular spans recursively; radial bands: `r_d = r_1 + d·band` (band 46px, `maxNodeRadius` 120px cap); edges = cubic beziers from parent rim to child rim (stroke 1.25px, category color at 70%); node discs sized `∝ sqrt(share)` capped; labels outside discs (right-side nodes) / flipped (left-side) for readability; depth ≤ `drawnDepth`.

### 4.7 Age timeline
X = time (auto extent: oldest mtime → now, ticks at adaptive granularity, locale dates); Y = bytes; stacked areas by age bucket (§ [08 § 4.3](08-UI-DESIGN-SYSTEM.md#43-age-ramp-mtime--color-shared-by-age-viz-age-color-mode-stale-badges)) — each file contributes its bytes to its mtime column (64 columns default, pinch/zoom to 256); hover column = readout with bucket breakdown; click column = filter to that age bucket; selected file highlighted as tick. Stale-rule threshold line (1y) drawn as vertical hairline with micro label.

### 4.8 Folders / Table / Bars (DOM modes)
Specified in [09](09-UI-COMPONENTS.md) — they consume the same `viz:layout` frames (mode-specific payloads) so filtering, selection, and color modes behave identically.

## 5. Palettes

`DataPalette` (per theme, from [08 § 4](08-UI-DESIGN-SYSTEM.md#4-data-colors-the-spectrum)) reaches the renderer as a typed array (ARGB u32 × categories + ramps) attached to each `viz:layout` response — the canvas never parses CSS. Palette version bumps on user color edits → frames re-tint without relayout (fast path: recolor-only frame update, ≤ 4 ms for 60k tiles on Canvas2D via cached tile arrays; WebGL: uniform swap, ~0 cost).

## 6. Interaction contracts

| Contract | Rule |
|---|---|
| Hover latency | ≤ 8 ms from pointermove to overlay update (hit-test budget 2 ms; [16](16-PERFORMANCE-BUDGETS.md#5-viz-budgets-vizcanvas-r2)) |
| Selection | ≤ 16 ms visual confirmation; syncs tree/table/inspector same frame |
| Zoom | layout request → frame ≤ 30 ms engine (100k tiles); interpolation 200 ms; user input never blocked during layout (frames arrive async; zoom targets queue) |
| Filter dim | overlay-layer multiply at 8% opacity for non-matching, 0 relayout |
| Keyboard | every mode: Tab traverses legend → canvas; arrows move selection by neighbor graph (mode-aware adjacency map computed with frame) |
| Screen readers | each mode exposes a live region summary ("Selected node Users, 118 GB, 34% of scan") + a "data table view of this visualization" link switching to Table scoped to same root (real a11y, not checkbox a11y) |

## 7. Testing

- **Golden frames:** every mode × 3 fixtures (small 5k, deep 120k, wide 1M-nodes) → VizFrame binary hash in CI (layout determinism).
- **Golden images:** headless screenshot suite (`--screenshot=viz:<mode>` harness, technique adopted per [02 § 2](02-INTEL-DISKBUDDY.md#2-confirmed-technology-stack-and-what-it-implies)) at 1x/2x DPR × Nocturne/Alabaster; pixel-diff tolerance 0.1% area.
- **Interaction tests:** Playwright-driven pointer sequences (hover paths, dbl-click zoom chains, wheel sequences) asserting frame requests + overlay updates.
- **Perf gates:** budgets in [16 § Viz Budgets](16-PERFORMANCE-BUDGETS.md#5-viz-budgets-vizcanvas-r2) run on CI hardware class + reference machines.
