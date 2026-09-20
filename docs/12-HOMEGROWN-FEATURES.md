# 12 — Homegrown Features (Beyond Parity)

> Owner doc: [00-INDEX.md](00-INDEX.md) · Upstream: [01-PRODUCT-VISION](01-PRODUCT-VISION.md), [02-INTEL-DISKBUDDY](02-INTEL-DISKBUDDY.md), [03-PORTING-MATRIX-WINDIRSTAT](03-PORTING-MATRIX-WINDIRSTAT.md) · Downstream: [06-RUST-CORE](06-RUST-CORE.md), [09-UI-COMPONENTS](09-UI-COMPONENTS.md), [10-SCREENS-AND-FLOWS](10-SCREENS-AND-FLOWS.md), [13-LICENSING-SYSTEM](13-LICENSING-SYSTEM.md).
> Requirement IDs: `PRISM-HG-*`. Every feature here is premium-tier unless marked `(free)` — tier matrix in [13 § SKUs](13-LICENSING-SYSTEM.md#1-skus-and-feature-matrix).

---

## 1. Quick Wins

Sidebar-ranked reclaim candidates with **explainability** as the design constraint: each row = name, bytes, source chip, one-line reason. Ranking: `score = bytes × confidence` where confidence comes from source (preset rule 1.0, stale 0.7, duplicate 0.9, large-cache heuristics 0.6); ties broken by recency of rule match. Rows deep-link to the evidence (preset → preset-hit list; stale → the file rows; duplicate → group). Cap 6 rows + "see all" popover. `PRISM-HG-001`. Engine surface: computed in aggregation ([06 § 4](06-RUST-CORE.md#4-aggregation--statistics)); no separate scan.

## 2. Cleanup presets (rule engine)

**Windows-first rule table** (our own taxonomy — competitive intel validated the *categories*, we ship Windows targets; concept credit noted in [02 § 3.3](02-INTEL-DISKBUDDY.md#33-feature-inventory-from-symbols--strings--exhaustive)):

| Preset | Match rules (glob, roots) | Safety class |
|---|---|---|
| JavaScript node_modules | `**/node_modules/**` | regenerable — safe |
| Rust build artifacts | `**/target/{debug,release}/**` | regenerable |
| .NET build | `**/{bin,obj}/**` | regenerable |
| Python caches | `**/{__pycache__,pytest_cache,.mypy_cache,.ruff_cache}/**`, `**/*.pyc` | regenerable |
| Gradle / Android | `~/.gradle/caches/**`, `**/.gradle/**`, `**/build/**` (gradle projects only), AVD images path | regenerable + user-content (AVD listed separately) |
| Xcode-on-Windows (React Native etc.) | `**/{ios/Pods,ios/DerivedData}/**` | regenerable |
| Container images | Docker Desktop WSL vhdx path, `**/*docker*disk*.vhdx` | **user-visible warning** (compact via provider tool preferred) |
| VM disks | `**/*.{vhd,vhdx,vmdk,vdi,qcow2,vmwarevm/**}` | user-content — confirm extra |
| Emulator images | Android emulator/system images paths | regenerable |
| Package-manager caches | npm/pnpm/yarn cache dirs, `~/.cargo/registry/cache`, pip cache, NuGet cache, Go mod cache | regenerable |
| Browser caches | Chromium/Edge/Firefox `Cache` profile subpaths (careful path rules; never cookies/login data) | regenerable |
| Windows temp | `%TEMP%`, `C:\Windows\Temp` (admin needed on some) | regenerable |
| Recycle bin | bin contents | parity WDS-CTX-03 |
| Crash dumps & logs | `**/*.dmp`, `LocalCrashDumps`, `**/Logs/**` (dev-tool paths only) | regenerable |
| Installers & archives in Downloads | `Downloads/**/*.{exe,msi,zip,7z,iso}` | user-content — confirm extra |
| Old downloads (stale) | Downloads + stale rule | user-content — confirm extra |

Each rule ships: matcher ([06 § 2.4](06-RUST-CORE.md#24-exclusion-matcher)), safety class, explanation string (localized), enable/ disable + custom rules editor (Settings → Cleanup; export/import JSON). Preset scan runs against the current arena (no re-walk). `PRISM-HG-010`.

## 3. Stale rules

Default: **size ≥ 40 MB ∧ mtime ≥ 1 year** (thresholds configurable; the 40 MB/1y defaults honor the validated heuristic from [02 § 3.3](02-INTEL-DISKBUDDY.md#33-feature-inventory-from-symbols--strings--exhaustive)). Age buckets shared with the age ramp ([08 § 4.3](08-UI-DESIGN-SYSTEM.md#43-age-ramp-mtime--color-shared-by-age-viz-age-color-mode-stale-badges)) — color and policy can never disagree. View: stale table with bucket filter chips; staged via the standard cleanup ledger. `PRISM-HG-020`.

## 4. Duplicates

Pipeline (engine, cancellable, progress events per [05 § 4](05-IPC-PROTOCOL.md#4-event-pump-architecture)):

```
1. group by (size)                      — arena query, O(n) over files
2. group by (size, ext)                 — cheap filter
3. partial fingerprint: first+last 64 KiB + length → XXH128   — reads only candidates
4. full hash: BLAKE3 (fast, SIMD)       — only for surviving candidates
5. hardlink collapse: same file_id ⇒ same bytes — auto-resolved group marked "hard links", excluded from waste
```

Guarantee (user-facing, parity of promise with [02 § 3.3](02-INTEL-DISKBUDDY.md#33-feature-inventory-from-symbols--strings--exhaustive)): byte-identical only — full hash is the final arbiter; partial fingerprints only *prune*. I/O budget: user-configurable MB/s cap + pause (disk-honesty); SSD detection (trim/rotational flag) adjusts defaults. UI per [10 § 6](10-SCREENS-AND-FLOWS.md#6-duplicates-premium-tab). Exclusions honored; protected paths unstaggable. `PRISM-HG-030`.

## 5. Application uninstaller + leftovers

Inventory: registry uninstall keys (HKLM/HKCU, both `.../Uninstall` trees, MSI product detection via `MsiEnumProducts`), Store apps (non-executable-class filtered), Steam games (libraryfolders.vdf). Footprint scan per app: token-based matching (normalized publisher+product name) across `%LOCALAPPDATA%`, `%APPDATA%`, `%PROGRAMDATA%`, `Documents`, common registry hives; explicit **evidence list** (every matched path with size) — no opaque totals ([01 § 5.4](01-PRODUCT-VISION.md#5-product-principles-decision-tie-breakers)). Uninstall flow per [10 § 7](10-SCREENS-AND-FLOWS.md#7-applications-premium-tab). Leftovers: same matcher run against apps **not present** in inventory. All deletions via the cleanup ledger (never direct). `PRISM-HG-040`.

## 6. Snapshots + diff

Per [07 § 4](07-DATA-MODEL.md#4-persistence-schema-sqlite-appdb): depth-capped shape capture; diff engine produces grew/shrank/added/removed with 10 MB floor; UI per [10 § 9](10-SCREENS-AND-FLOWS.md#9-snapshots-premium-tab). Auto-capture option: after every completed scan of a previously-snapshotted root (opt-in, disclosed). `PRISM-HG-050`.

## 7. Monitor

1 Hz `NtQuerySystemInformation` sampling ([06 § 5](06-RUST-CORE.md#5-volume-enumeration--system-info)): top processes by CPU / DiskIO / WorkingSet; system CPU/mem/disk-activity sparklines (60 s window). No actions (v1). UI per [10 § 8](10-SCREENS-AND-FLOWS.md#8-monitor-premium-tab). `PRISM-HG-060`.

## 8. Turbo scan (raw NTFS)

First-class strategy per [ADR-06](04-SYSTEM-ARCHITECTURE.md#adr-06--two-first-class-scan-strategies-not-a-fallback), mechanics in [06 § 3](06-RUST-CORE.md#3-turbo-scan--raw-ntfs-opt-in-elevated), consent UX in [10 § 3](10-SCREENS-AND-FLOWS.md#3-welcome-drivefolder-selection--wds-sel-0102040506). `PRISM-HG-070`.

## 9. Scheduled scans

In-app scheduler (Windows Task Scheduler registration behind the scenes — survives app closure): triggers (daily/weekly/at-logon), action = background **standard** scan (no elevation, quiet failure to notifications), result = notification + snapshot auto-capture + "what changed" digest toast on next launch. Settings → Scheduler. `PRISM-HG-080`. (Scope guard: **no scheduled cleanup** — destructive automation is out of scope v1, [01 § 8](01-PRODUCT-VISION.md#8-scope-boundaries-v1).)

## 10. Exports

CSV/NDJSON per [07 § 7](07-DATA-MODEL.md#7-export-formats-premium-exportscan) + **snapshot share file** (PSNP1 payload, contains no absolute paths — path-key hashes only, privacy-safe for support forums). `PRISM-HG-090`.

## 11. Preview pane (free)

Shell preview: Windows Explorer preview handlers (`IPreviewHandler` host) for selected file (images, PDFs, text, media) in a dockable right pane; toggled `Ctrl+P`. Falls back — **no**: if no handler is registered for a type, the pane shows file facts (sizes, hashes of nothing — just metadata + icon) and states "No preview available for this type" (honest state, [19 A2](19-AGENT-PROTOCOL.md#a2--no-fallbacks-the-rule-the-owner-cares-about-most)). `PRISM-HG-100`.

## 12. Command palette (free)

Per [09 § CommandPalette](09-UI-COMPONENTS.md#commandpalette-ctrlk). Includes recent paths, preset toggles, theme switching, "jump to biggest file", "reveal current root". `PRISM-HG-110`.

## 13. Multi-language architecture (free mechanism)

ICU messageformat, pseudo-loc QA harness, RTL-safe layouts; `en-US` complete + `de-DE` demonstrating the mechanism at GA (parity WDS-CHROME-07). String extraction CI (no hardcoded UI strings — lint `no-literal-strings` in components). `PRISM-HG-120`.

## 14. Traceability to value propositions

| Feature | Value prop ([01 § 4](01-PRODUCT-VISION.md#4-value-proposition--why-pay-when-the-baseline-tool-is-free)) |
|---|---|
| Turbo scan | Speed |
| Unique-bytes accounting | Truth |
| Quick wins, presets, stale, uninstaller, leftovers | Guidance |
| Duplicates | Duplicates |
| Snapshots/diff, scheduler digests | History |
| 9 viz modes | Ways to read |
| Themes, density, a11y, palette | Product polish |
| Exports, preview, palette | Product polish / IT Sam |
| Local-only posture (all features) | Privacy |
