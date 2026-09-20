# 02 — Competitive Intel: DiskBuddy Reverse-Engineering Report

> Owner doc: [00-INDEX.md](00-INDEX.md) · Upstream: [01-PRODUCT-VISION.md](01-PRODUCT-VISION.md) · Downstream: [08-UI-DESIGN-SYSTEM](08-UI-DESIGN-SYSTEM.md), [10-SCREENS-AND-FLOWS](10-SCREENS-AND-FLOWS.md), [11-VISUALIZATION-ENGINE](11-VISUALIZATION-ENGINE.md), [12-HOMEGROWN-FEATURES](12-HOMEGROWN-FEATURES.md), [13-LICENSING-SYSTEM](13-LICENSING-SYSTEM.md).
>
> **Handling rule:** the analyzed binary, extracted files, and screenshots live only in the research workspace (`research/diskbuddy/`), outside the product repository. Nothing from it is ever copied into the product — no fonts, no icon assets, no strings, no code, no color-for-color themes. This document extracts **decisions and lessons**, not expression. `PRISM-INTEL-*` requirements below define what we *take* and what we *deliberately change*.

---

## 1. Method & provenance

Full static analysis was performed on the shipping macOS distribution (`DiskBuddy-1.0.0.dmg`, v1.0.0):

1. **DMG container** — UDIF `koly` trailer parsed; mish-block chunk table decoded (19 chunks; the image mislabels zlib blocks as ADC — handled by magic sniffing); raw HFS+ volume reassembled (34 MiB).
2. **HFS+ filesystem** — volume header, catalog B-tree (node size 4096, 52 leaf records), and extent records parsed with a custom reader; full bundle extracted byte-exact. File inventory: 17 files, 8 folders.
3. **Mach-O analysis** — arm64, LC_BUILD_VERSION minOS 14.0 / SDK 26.5, **38,778 exported symbols (binary shipped unstripped)** of which **10,014 belong to the `DiskBuddy` Swift module**. Linked frameworks: SwiftUI, AppKit, Combine, CoreGraphics, CoreText, CryptoKit, Security, CFNetwork, QuickLookUI, Foundation, libswift*.
4. **String/symbol mining** — full symbol table demangled by pattern; string tables filtered for UI copy, endpoints, enums, format strings.
5. **Visual analysis** — 8 unique UI screenshots (treemap, sunburst, icicle/flame, circle pack/bubbles, mind-map, folders, age map, top sizes) analyzed by a vision model with a design-system-extraction prompt; palette estimates below are vision-model approximations, refined in [08-UI-DESIGN-SYSTEM](08-UI-DESIGN-SYSTEM.md).

**Legality note:** analysis was of a legitimately obtained copy, for interoperability/competitive understanding. Findings here are functional facts (feature lists, architecture patterns, copy tone), which are not copyrightable. We re-implement ideas; we do not decompile-to-copy.

## 2. Confirmed technology stack (and what it implies)

| Layer | Their choice | Implication for Prism |
|---|---|---|
| UI | SwiftUI + AppKit interop | They accepted a hybrid to get NSPanel/QuickLook. We get the equivalent breadth from Electron + native modules (preview pane via Windows Shell, [12 § Preview](12-HOMEGROWN-FEATURES.md#11-preview-pane-free)). |
| Rendering | SwiftUI `Canvas` + CoreGraphics for all 9 visualizations; CoreText for text-in-canvas | Confirms the thesis: **DOM/SVG does not scale to millions of nodes — canvas-style immediate rendering is required** ([11-VISUALIZATION-ENGINE](11-VISUALIZATION-ENGINE.md)). Their canvases ship cull limits (`maxArcs`, `maxCircles`, `maxFiles`, `maxFolders`, `maxNodeRadius`, `minShare`, `minSize`, `drawnDepth`) — we adopt the same concept with our own values. |
| Concurrency | Swift async/await, `WorkQueue`, `SendableBox` wrappers | Same shape as our design: dedicated worker, no shared mutable state across threads, results marshaled to UI ([06-RUST-CORE](06-RUST-CORE.md)). |
| Fonts | Bundled **Inter** family — 9 weights (Thin→Black) + license file, loaded via `ATSApplicationFontsPath` | Validates bundling a distinctive full-weight sans family. **We must choose a different primary family** (see [08 § Typography](08-UI-DESIGN-SYSTEM.md#2-typography)) to keep visual identity distinct. |
| Licensing | CryptoKit (signature verify), Security/Keychain (storage), CFNetwork (activate/validate/deactivate over HTTPS) | Matches our [13-LICENSING-SYSTEM](13-LICENSING-SYSTEM.md): server-signed entitlements + OS keystore + three endpoints. |
| Privileges | macOS Full Disk Access prompts; no helper daemon | They scan *without* elevation and prompt for FDA. On Windows the equivalent posture is: standard scans need no admin; our turbo MFT mode is the *opt-in* elevated path ([06 § Turbo Scan](06-RUST-CORE.md#3-turbo-scan--raw-ntfs-opt-in-elevated)). |
| Dev tooling | `ScreenshotHarness` + `--shot-activation` CLI flag (headless screenshot mode for the activation screen) | We adopt this technique: a `--screenshot=<screen>` harness mode for automated visual regression and design review ([17-QUALITY-ENGINEERING](17-QUALITY-ENGINEERING.md)). |

## 3. Product structure (reconstructed)

### 3.1 Workspace model

The app is organized as **one workspace with tabs**, not multiple windows:

- Tabs (from `WorkspaceTab` cases + UI copy): **Explore** (visualizations), **Duplicates**, **Applications**, **Monitor**, **Snapshots**, **Leftovers**.
- A persistent three-column shell: left sidebar (scan actions, disk gauge, quick wins, file types) / center visualization / right inspector ([10-SCREENS-AND-FLOWS](10-SCREENS-AND-FLOWS.md) adopts this shell with our own proportions).
- Breadcrumb path with "Back to Scan Root", "Back to parent folder", and "Include everything below" (scope controls).

### 3.2 The nine visualizations (`VizMode` cases)

| Mode | Their tagline (verbatim) | Reading |
|---|---|---|
| `treemap` | "Every file as a rectangle, sized by bytes" | Nested squarified map |
| `sunburst` | "Rings radiating out from the scan root" | Radial icicle |
| `icicle` | "Depth top to bottom, size left to right" | Flame chart |
| `mindmap` | "Branches from the root, sized by weight" | Radial tree |
| `pack` | "Nested bubbles, one per folder" | Circle pack |
| `folders` | "Browse folder by folder, sized as you go" | Card grid |
| `table` | "The biggest items, ranked" | Ranked table |
| `bars` | "The biggest items, ranked" (bar rows) | Ranked bars |
| `calendar` / age | "Where your bytes sit on a timeline" | Age timeline |

Their marketing line: *"Eight ways to read the same bytes. Everything stays on this Mac."* — the privacy clause is welded to the value prop. **Lesson (`PRISM-INTEL-001`):** ship all modes at GA (this is the flagship differentiator vs. the parity baseline which has only a treemap), and weld the local/privacy claim into the product voice.

Each viz has: hover readout, selection highlight overlay (`HighlightOverlay`, `ArcHighlight`, `CircleHighlight`, `GraphHighlight`), click-to-drill, label toggling ("Draw labels on visualisations"), depth control ("How many folder levels to draw"), and a color-mode rail (by **type**, by **branch**, by **age** — `ColorMode` cases). We mirror this interaction grammar exactly in concept (values/visuals our own) — it is the right grammar.

### 3.3 Feature inventory (from symbols + strings — exhaustive)

**Scan engine**
- Scan targets: whole-volume ("Scan Full Mac" → our "Scan drive"), Home folder, arbitrary folder picker, plus per-target "Full Disk Access" guidance when protected paths are hit.
- Streaming progress: current path, scanned bytes/files, elapsed, and a completion toast "scanned %@ in %.2fs".
- Size modes: **"Measure size on disk (allocated)"** vs logical — a first-class toggle ("Showing size on disk" / "Showing logical size"), matching our truth principle ([01 § 5.3](01-PRODUCT-VISION.md#5-product-principles-decision-tie-breakers)).
- `useAllocatedSize`, `treemapDepth`, `searchText`, `filter` (name filter with scheduled + apply-now semantics — `scheduleFilter`/`applyFilterNow`, i.e. debounced filtering), `isFiltering`.
- Bundles/packages treated as single nodes (extensive package-extension table: `.xcodeproj`, `.photoslibrary`, `.logicx`, `.sparsebundle`, `.dSYM`, `dmg`/`sparseimage` detection, `diskImage`/`isPackage` flags).

**Guidance / cleanup**
- **Quick Wins**: sidebar-ranked reclaim candidates.
- **CleanupPreset** scanner with preset hits (`PresetHit`) — rule list includes (verbatim concepts): JavaScript dependency trees (`node_modules`, `next`, `nuxt`, `out`, `turbo`, `parcel`), `DerivedData`, `Archives`, `CoreSimulator` device images, `Caches`, `build`, `dist`, `target`, `__pycache__`, `pytest_cache`, `gradle`, `dart_tool`, `Pods`, `vendor`, `bundle`, Docker (`com.docker.docker`) container images, `Virtual Machines.localized`, "Installers and mountable images", "Video and audio files", "Local mail stores and attachment caches", "Application caches and log files", "Files already on their way out" (trash).
- **Cleanup queue**: stage/queue items ("click to stage for cleanup", "Stage every copy except the first", "Remove from Cleanup", "Empty Cleanup Queue"), with `reclaimPreview` (totals before executing).
- **Stale files**: `StaleRow`, `modifiedBefore` filter, buckets "Last 7 days / 3 months / 12 months"; empty state: "Nothing over 40 MB here has gone a year without being touched." → stale rule = **size ≥ 40 MB ∧ mtime ≥ 1 year**. We adopt the *concept* with configurable thresholds ([12 § Stale Rules](12-HOMEGROWN-FEATURES.md#3-stale-rules)).

**Duplicates**
- `DuplicateFinder` with its own `Progress` type; group view with "Byte-for-byte identical copies in the current scan"; "Hashes are compared in full, so a match is a real match" — **they full-hash every candidate**, no partial-fingerprint shortcut. We keep their *user promise* (byte-identical guarantee) but improve the pipeline (size → 64 KiB partial hash → full hash — the promise still holds because full hash remains the final arbiter; [12 § Duplicates](12-HOMEGROWN-FEATURES.md#4-duplicates)).
- Stage-all-extras action staged into the same cleanup queue.

**Applications (uninstaller)**
- `InstalledApp` inventory, per-app footprint ("adds up the bundle plus every cache, container…" — i.e., bundle + app data), **Leftovers** tab: "Leftovers from removed apps" (orphan detection for apps no longer installed), "Show Apple system apps" toggle (→ our "Show Windows store/system apps"), "Uninstall Completely" with confirmation, "Uninstalled with X" attribution, "No caches, containers or preferences found for this app." empty state, `AppUninstaller`, `LeftoverItem`, `Associated files` section.

**Snapshots (history)**
- `SnapshotStore` persists scan shapes; `SnapshotDelta` diff view: "Pick two snapshots to compare — Click one snapshot for the BEFORE side, then another for AFTER." Delta rows with a 10 MB significance floor ("Nothing moved by more than 10 MB"). Capture action: "Capture Current Scan".

**Monitor**
- `SystemMonitor` samples `ps`-style data (symbols: `pid`, `pcpu`, `rss`, `comm`), "Top processes by CPU", `loadAverage`, `cpuUsage` sparkline (`Sparkline`, `ProcessSample`). On Windows we do the same via PDH/perf counters — top processes by CPU + working set + disk I/O ([12 § Monitor](12-HOMEGROWN-FEATURES.md#7-monitor)).

**Inspector**
- Right panel: selection metadata ("Select something to inspect it."), size + counts + timestamps, **"Largest Inside"** top-10 children, actions: Reveal in Explorer-equivalent, Copy Path, Delete, Preview (QuickLook on their side; ours = Shell preview pane).

**Views/tables**
- `TableView` with `Column` enum (name, size, items, kind, modified), "Sortable columns, keyboard friendly", `BarsView.Scope` (In this folder / Biggest files anywhere / Biggest folders anywhere), `BarRow`, `MiniBar`, `DonutGauge` (disk usage ring), `CategoryBar`, `CompositionDots`, `Sparkline`, `MetricCard`, `StatRow`.

**Platform integration**
- QuickLook preview panel (`QuickLook`, `previewPanel`, `numberOfPreviewItems`), Finder reveal (`activateFileViewerSelectingURLs`), Trash service (`TrashService` — delete-to-trash default), CSV export (`exportCSV`), clipboard copy, drag-out (implied by `NSPasteboard` symbols).
- Settings: `PreferencesView` with theme picker, labels toggle, size-mode default, deactivate control.

### 3.4 Themes & visual language

`ThemeID` cases recovered: **`paper`, `linen`, `sage`, `mist`, `slate`** — five themes, natural-material names, warm-minimalist execution. Vision-model palette estimates (light themes ~`#F7F5F2` base, white cards; dark `slate`; categorical pastels like `#F0E6D8`, `#D8E8F0`, `#D8F0DC`, `#E8E0F0`; near-black `#1C1C1E` primary buttons; macOS system grays for secondary text `#8E8E93`). Age gradient: green (recent) → tans/browns (aging) → grey (ancient).

**Lessons (`PRISM-INTEL-002`):**
- Multi-theme at GA is table stakes for a paid utility; ship **six** themes ([08 § Themes](08-UI-DESIGN-SYSTEM.md#3-themes)) with **different names and different palettes** (ours: higher-contrast dark-first identities; theirs is light-first warm minimalism — we deliberately diverge; see [08 § Divergence](08-UI-DESIGN-SYSTEM.md#32-divergence-from-references-compliance-with-02-prism-intel-012)).
- Viz color modes (type/branch/age) as a rail control — adopt concept.
- **Inter at 9 weights as the sole family** — do *not* replicate; our primary is **Geist + Geist Mono** with JetBrains Mono for numeric runouts, giving a sharper technical identity while remaining utilitarian ([08 § Typography](08-UI-DESIGN-SYSTEM.md#2-typography)).

### 3.5 Licensing model (fully reconstructed)

| Aspect | Their implementation (recovered) | Prism decision |
|---|---|---|
| SKUs | Single one-time lifetime purchase ("One licence, one Mac, yours for good.") | **Two SKUs: Yearly + Lifetime** (owner requirement R10) — copy adapts the same lock-in relief messaging |
| Endpoints | `POST /licenses/activate`, `POST /licenses/validate`, `POST /licenses/deactivate` | Identical endpoint taxonomy ([13 § API](13-LICENSING-SYSTEM.md#3-api-surface-license-server)) |
| Payload field | `license_key_instance_id` | Device instance binding, same concept |
| Device binding | "DiskBuddy will lock on this Mac and the key becomes free to use on another one." Max 1 device; release via in-app "Deactivate This Mac" | Same UX, max devices configurable server-side (default 3 — friendlier for our personas; [13 § Device Policy](13-LICENSING-SYSTEM.md#device-policy)) |
| Conflict copy | "This key is already active on another Mac… Deactivate This Mac… or email hello@…" | Mirror messaging pattern (our support contact) |
| Offline | Grace window with escalating warnings ("has not been able to reach the licence server in X") | 30-day signed-entitlement grace with in-UI countdown ([13 § Offline Grace](13-LICENSING-SYSTEM.md#53-offline-grace)) |
| Revocation | "This licence key is no longer valid on this Mac. It may have been released from another machine, refunded, or expired." | Same semantics: server-side release/refund/expiry invalidate next validation |
| Anti-abuse | "Too many attempts. Wait a minute and try again." | Rate limiting on server, same message class |
| Storage | Keychain service `com.diskbuddy.app.license` | Windows: DPAPI-encrypted file + Credential Manager ([13 § Client Storage](13-LICENSING-SYSTEM.md#6-client-storage)) |
| Payments | Dodo Payments — `checkout.dodopayments.com/buy/…` buy-links; `live.dodopayments.com` for merchant ops | Provider-agnostic adapter with Dodo as default target ([13 § Payment Provider](13-LICENSING-SYSTEM.md#payment-provider)) |
| Trial | None observed in strings (hard activation gate `LicenseGate` → `ActivationView` or `RootView`) | We add a 14-day full-feature trial ([01 § 6](01-PRODUCT-VISION.md#6-licensing--commercial-shape)) — activation UX otherwise mirrors the gate pattern |
| Gate architecture | `LicenseGate` view: licensed ? `RootView` : `ActivationView`; license state injected via SwiftUI environment | Equivalent: React provider gate at app root; **plus** capability checks at engine boundaries (they gate only UI — we go further; [13 § Enforcement Depth](13-LICENSING-SYSTEM.md#9-enforcement-depth-engine-boundary)) |

**Critical weakness found in their design (`PRISM-INTEL-003`):** the recovered symbols show licensing state held in an observable UI object with no visible engine-side entitlement checks — the gate appears to be **UI-level only**. A patched client could flip the boolean. Our design must enforce at the Rust engine boundary (premium operations refuse without a valid entitlement token — [13 § Enforcement Depth](13-LICENSING-SYSTEM.md#9-enforcement-depth-engine-boundary)) and at the server for anything server-side. This is a *lesson from their mistake*, not a copy of their design.

### 3.6 Copywriting tone (for our voice, not their words)

Their strings show a distinctive copy culture we should match in *craft* while writing entirely our own text: short declaratives; every empty state explains what to do next; destructive actions named plainly; humor kept dry ("Files already on their way out"); the privacy promise stated in-product. Our voice guide lives in [10-SCREENS-AND-FLOWS § Voice](10-SCREENS-AND-FLOWS.md#voice) — written from scratch, never translated from their strings.

## 4. What we take, what we change (summary contract)

`PRISM-INTEL-010` — **Take (concept-level):** workspace tabs + three-column shell; nine viz modes with one interaction grammar; type/branch/age color modes; quick wins; cleanup queue with presets + reclaim preview; duplicates staged via cleanup queue; snapshots + before/after diff; app uninstaller + leftovers; monitor panel; inspector with "largest inside"; debounce-filtered search; allocated-vs-logical toggle; multi-theme; screenshot harness; licensing endpoint taxonomy + device binding + grace + deactivation UX; local-only privacy stance.

`PRISM-INTEL-011` — **Change / improve:** engine-side entitlement enforcement (their gap); duplicates pipeline with partial-hash stage (speed); 3-device default (their 1); trial period; Windows-native privilege model (turbo MFT scan vs their FDA prompts); hard-link-aware accounting (their macOS has none of Windows's hardlink/sparse/junction complexity); our own typography, palette, theme names, and copy; two SKUs instead of one.

`PRISM-INTEL-012` — **Never:** bundle their fonts/icons/images; reuse their strings verbatim; replicate their exact palettes or theme names; mention them in-product.

**Related:** [12-HOMEGROWN-FEATURES.md](12-HOMEGROWN-FEATURES.md) turns every "take" into a spec with our own parameters · [13-LICENSING-SYSTEM.md](13-LICENSING-SYSTEM.md) hardens their model · [08-UI-DESIGN-SYSTEM.md](08-UI-DESIGN-SYSTEM.md) defines our divergent visual identity.
