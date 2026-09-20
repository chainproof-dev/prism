# 20 — Dependency Manifest

> Owner doc: [00-INDEX.md](00-INDEX.md) · Upstream: [04-SYSTEM-ARCHITECTURE](04-SYSTEM-ARCHITECTURE.md), [08-UI-DESIGN-SYSTEM](08-UI-DESIGN-SYSTEM.md) · Downstream: [15-BUILD-PACKAGING](15-BUILD-PACKAGING.md), [17-QUALITY-ENGINEERING](17-QUALITY-ENGINEERING.md).
> Requirement IDs: `PRISM-DEP-*`.
>
> Versions below were **verified against registries at plan-issue time** (September 2026). Rule A1 ([19](19-AGENT-PROTOCOL.md#a1--latest-dependencies-pinned)): when a task introduces a dependency, it uses the latest stable *at that time* and updates this manifest in the same PR. This file is the single pin list — `package.json`/`Cargo.toml` values must match it (CI cross-checks).

---

## 1. Toolchains

| Tool | Pin | Notes |
|---|---|---|
| Rust | stable **1.98.x** (`rust-toolchain.toml`) | MSRV for crates documented = 1.88 (napi floor) |
| Node | **24 LTS** (`.nvmrc`, `engines: >=24 <25`) | main, preload, license server |
| pnpm | **10.x** | workspaces, frozen lockfile |
| Electron | **44.x** (44.4 line) | Chromium 148, Node 24; updated within major only via amendment |
| Vite | **8.3.x** (via `electron-vite` 4.x line) | Rolldown core |
| TypeScript | **5.9.x** | strict preset from [19 A3](19-AGENT-PROTOCOL.md#a3--production-grade-quality-baseline) |
| Tailwind CSS | **4.1.x** | CSS-first config, `@theme` tokens |
| electron-builder | **26.15.x** | NSIS + portable, signing, differential updates |

## 2. Renderer / main (npm)

| Package | Version | Role | License |
|---|---|---|---|
| `react` / `react-dom` | 19.x | UI runtime | MIT |
| `zustand` | 5.x | stores ([10 § 15](10-SCREENS-AND-FLOWS.md#15-stores-state-architecture)) | MIT |
| `@tanstack/react-table` | 9.2.x | tables ([09 FileTable](09-UI-COMPONENTS.md#filetable-tanstack-table-9--rankedbig-lists)) | MIT |
| `@tanstack/react-virtual` | 3.x | list virtualization | MIT |
| `recharts` | 3.10.x | auxiliary charts only (monitor sparkline/area, gauge atoms where SVG suffices) | MIT |
| `radix-ui` (unified package) | latest 1.x | primitives behind shadcn sources | MIT |
| shadcn/ui | **vendored sources** (owned, in `packages/ui`) — pulled initially via CLI at latest, then hand-maintained | MIT |
| `lucide-react` | latest (0.5xx line) | icons | ISC |
| `motion` (Framer) | 12.x | premium moments only ([09 § 2.3](09-UI-COMPONENTS.md#23-premiumanimated-moments-the-1020)) | MIT |
| `cmdk` | 1.x | command palette | MIT |
| `sonner` | 2.x | toasts | MIT |
| `zod` | 4.x | T1 validation schemas (generated) | MIT |
| `electron-updater` | 6.x (builder-matched) | signed updates ([15 § 6](15-BUILD-PACKAGING.md#6-auto-update-security)) | MIT |
| `electron-window-state` | 5.x | window persistence (WDS-CHROME-03) | MIT |
| `@napi-rs/cli` | 3.x | build `prism-core.node` | MIT |
| `@fontsource` or bundled files | — | **Instrument Sans + JetBrains Mono bundled as files** (no network font loads, [08 § 2](08-UI-DESIGN-SYSTEM.md#2-typography)) | OFL 1.1 |
| (dev) `electron-vite`, `vite`, `vitest`, `@playwright/test`, `eslint` (flat config), `eslint-plugin-*`, `typescript-eslint`, `husky`, `lint-staged`, `knip` | latest stable | tooling | MIT |

**Aceternity-style premium components:** adopted **selectively** by copying audited source (not as a dependency) where a [09 § 2.3](09-UI-COMPONENTS.md#23-premiumanimated-moments-the-1020) moment warrants it; each adoption is a manifest row here with license + diff review. Default: we build the moment ourselves on `motion` — fewer external moving parts.

## 3. License server (npm)

| Package | Version | Role |
|---|---|---|
| `fastify` | 5.x | HTTP |
| `better-sqlite3` | 12.x | keys/devices/events store |
| `@noble/ed25519` + `@noble/hashes` | latest 2.x/2.x | token signing (parity with Rust verify, property-tested) |
| `cbor-x` | latest | CBOR encode/decode (schema shared with Rust `serde_cbor`-class output; cross-tested) |
| `zod` | 4.x | request validation |
| `pino` | 9.x | structured logs (ip-hashed) |
| `execa` | 9.x | admin CLI |
| (dev) `tsx`, `vitest`, `supertest` | latest | tooling |

## 4. Rust crates

| Crate | Version | Role | License |
|---|---|---|---|
| `napi` / `napi-derive` / `@napi-rs/cli` | 3.x | FFI bridge | MIT |
| `windows-sys` | 0.62.x | raw Win32 (hot paths) | MIT/Apache |
| `windows` | 0.62.x | COM surfaces (IFileOperation, shell) | MIT/Apache |
| `crossbeam-channel` / `crossbeam-deque` | 0.8.x | SPSC ring, work-stealing | MIT/Apache |
| `rayon` | 1.10.x | aggregation pools | MIT/Apache |
| `serde` / `serde_json` | 1.x | DTOs | MIT/Apache |
| `ciborium` | 0.2.x | CBOR for entitlement tokens | MIT/Apache |
| `ed25519-dalek` | 2.x | token verify (engine) | MIT/Apache |
| `blake3` | 1.x | duplicate full hash | MIT/Apache |
| `xxhash-rust` (xxh3/xxh128) | 0.8.x | fingerprints, arena hashing | MIT/Apache |
| `rusqlite` (bundled) | 0.37.x | persistence | MIT |
| `zstd` | 0.13.x | snapshot payloads | MIT/Apache |
| `thiserror` / `tracing` / `tracing-subscriber` | latest 2.x / 0.1.x / 0.3.x | errors, logs | MIT/Apache |
| `widestring` | 1.x | UTF-16 handling | MIT/Apache |
| `globset`-class matching | — | **NO** — own matcher per [06 § 2.4](06-RUST-CORE.md#24-exclusion-matcher) (zero-dep glob subset, tested) | — |
| (dev) `criterion`, `proptest`, `cargo-fuzz`, `cargo-deny`, `cargo-audit` | latest | quality tooling | MIT/Apache |

**Explicit non-dependencies (decisions, not omissions):** `tokio`/`async-std` (engine is thread-based — syscall-bound work gains nothing from async; [06 § 13](06-RUST-CORE.md#13-crate-dependency-policy-full-pins-in-20-dependency-manifest)) · `reqwest` in engine (license client lives in main-process TS) · `d3-*` in renderer (viz is bespoke, [11]) · any CSS-in-JS (Tailwind 4 tokens) · any state library beyond zustand.

## 5. License audit (rule: SPDX field required in every manifest row)

| Family | Verdict |
|---|---|
| MIT / Apache-2.0 / ISC / BSD-2/3 / Zlib / Unicode-3.0 / MPL-2.0 (with review) | allowed |
| OFL 1.1 (fonts) | allowed, license file bundled + in About credits |
| GPL / AGPL / LGPL / SSPL | **denied** in shipped artifacts (keeps the proprietary binary clean — and reinforces the clean-room posture of [03 § 0](03-PORTING-MATRIX-WINDIRSTAT.md#0-clean-room-protocol)) |
| Unlicense / WTFPL / custom-EULA deps | denied without owner amendment |
| Radix/shadcn/Lucide/Tailwind | MIT-family ✅ (shadcn sources are ours once vendored) |
| `motion` | MIT ✅ |
| `better-sqlite` / `rusqlite(bundled SQLite)` | MIT + SQLite public domain ✅ |
| Fonts: Instrument Sans, JetBrains Mono | OFL ✅ — bundling + attribution per [08 § 2](08-UI-DESIGN-SYSTEM.md#2-typography) |

`cargo-deny` + `pnpm license-check` enforce in CI ([15 § 8](15-BUILD-PACKAGING.md#8-supply-chain)).

## 6. Upgrade policy

- Renovate-class automation opens upgrade PRs on a regular automated cadence; humans merge only with green CI + perf diff for hot-path deps (`prism-core`, `napi`, `electron`, `vite`, `react`, `windows-sys`).
- Electron major upgrades = amendment-level events (they change Chromium + Node; the [14 § 3](14-SECURITY-MODEL.md#3-electron-hardening-release-config-is-law--ci-asserts-every-line-of-this-table) hardening table is re-asserted + release channel smoke).
- Security advisories (high/critical, prod deps): upgrade PR is **priority-ordered above all feature work** until merged ([17 § 7](17-QUALITY-ENGINEERING.md#7-bug-taxonomy--severity-drives-fix-ordering) S1).
- Every upgrade PR updates this file's version column — the file is the changelog of pins.
