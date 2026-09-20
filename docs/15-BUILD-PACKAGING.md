# 15 — Build, Packaging & Distribution

> Owner doc: [00-INDEX.md](00-INDEX.md) · Upstream: [04-SYSTEM-ARCHITECTURE](04-SYSTEM-ARCHITECTURE.md), [14-SECURITY-MODEL](14-SECURITY-MODEL.md) · Downstream: [17-QUALITY-ENGINEERING](17-QUALITY-ENGINEERING.md), [18-PHASE-PLAN](18-PHASE-PLAN.md), [20-DEPENDENCY-MANIFEST](20-DEPENDENCY-MANIFEST.md).
> Requirement IDs: `PRISM-PKG-*`.

---

## 1. Toolchain summary (pinned versions in [20](20-DEPENDENCY-MANIFEST.md))

| Stage | Tool |
|---|---|
| Package manager | pnpm (workspaces, frozen lockfile in CI) |
| Node | 24 LTS (main/preload + license server; `.nvmrc` + `engines` strict) |
| Bundler (renderer/main/preload) | Vite 8 via `electron-vite` pipeline (Rolldown core) |
| Rust | stable 1.98+ pinned (`rust-toolchain.toml`), cargo workspace, `cargo-deny` (licenses/advisories), `cargo-audit` in CI |
| N-API build | napi-rs v3 CLI (`napi build --platform --release` → `prism-core.node` per-target) |
| Packaging | **electron-builder 26** — NSIS installer (primary) + portable `.exe` (secondary); MSIX **deferred** post-GA (re-evaluate; the classic installer covers shell integration + elevation helper cleanly) |
| Signing | Windows code signing — **Azure Trusted Signing** (or OV/EV cert via hardware token) for all artifacts; electron-builder `signtoolOptions` |
| CI | GitHub Actions: `ci.yml` (PRs), `release.yml` (tags), Windows runners `windows-latest` + self-hosted perf runner for benches |
| Targets | `x64` + `arm64` (separate installers, correct per-arch `.node` binaries; no fat bundles) |

## 2. Build pipeline

```
pnpm install --frozen-lockfile
cargo deny check                       # licenses + advisories gate
pnpm codegen                           # Rust types → TS (drift = fail)  [05 § 6]
pnpm --filter @prism/ui build          # tokens pipeline + component lib
napi build -p crates/prism-core --release --target x86_64-pc-windows-msvc   → apps/desktop/native/prism-core.x64.node
electron-vite build                    # renderer (Vite 8) + main + preload
electron-builder --win nsis portable   → signed artifacts + SBOM
```

Dev: `npm run dev` = concurrently license server (:8080) + `electron-vite dev` (HMR renderer; main reload on change; Rust changes trigger `napi build --debug` watch task) — one command, per R11 ([13 § 10](13-LICENSING-SYSTEM.md#10-local-development--demo-key-owner-requirement-r11)).

### 2.1 Profile matrix (Rust)

| Profile | panic | debug | LTO | notes |
|---|---|---|---|---|
| `dev` | unwind | full | off | asserts on, path debug env honored |
| `release` (cdylib) | unwind at FFI edge → abort policy per [06 § 10](06-RUST-CORE.md#10-error-strategy-engine-wide) | none | fat, `codegen-units=1` | `strip=true` |
| `bench` | release + debug-lines | line-tables | thin | criterion |

## 3. Configuration strategy

- `app.config.json` (bundled, per-channel): `{ updateFeed, licenseServerUrl, channel, Sentry-free-crash-host (opt-in), featureFlags }`.
- Build-time injection: `LICENSE_SERVER_URL` and `UPDATE_FEED` come from CI secrets per channel — **dev default is `http://localhost:8080`** (R11); production values never appear in the repo.
- All feature flags default **off** unless the feature is complete (no dark-shipped halves, [19 A2](19-AGENT-PROTOCOL.md#a2--no-fallbacks-the-rule-the-owner-cares-about-most)); flags exist for rollout sequencing only, not for shipping stubs.

## 4. Installer (NSIS via electron-builder)

- Modes: **per-user** (default, no admin, `%LOCALAPPDATA%\Programs\…`) and **per-machine** (`Program Files`, requires admin, enables all-users shell integration by default).
- Options page: desktop shortcut (on), Start Menu (on), **"Add 'Analyze with <product>' to Explorer context menu"** (on for per-machine, opt-in for per-user via registry HKCU — WDS-CTX-04), autostart (off), beta channel (off).
- `allowToChangeInstallationDirectory: true`; assisted + one-click both supported; uninstaller removes app files + optional "keep settings/snapshots?" question (default keep); `deleteAppDataOnUninstall: false`.
- Elevation helper binary ships in `resources/`; the installer registers nothing privileged by default.
- Upgrade semantics: NSIS differential updates; settings/migrations via schema versions ([07 § 4](07-DATA-MODEL.md#4-persistence-schema-sqlite-appdb)).
- Delta packages enabled (`differentialPackage: true`) for bandwidth; full package always available as recovery.

## 5. Code signing

- Every artifact: `prism-core.*.node`, main exe, helper exe, installer, uninstaller, portable — signed (`signAndEditExecutable` + `signExe` hooks; `electron-builder` signs DLL/EXE/NSIS stub).
- `win.verifyUpdateCodeSignature = true` (updater requirement); SmartScreen reputation is a launch-ops concern: first releases may use EV/Trusted Signing to shorten warning periods — budgeted, not improvised.
- RFC 3161 timestamping mandatory; CI fails on unsigned artifacts (post-build `signtool verify /pa /all` step).

## 6. Auto-update security

electron-updater with **our own feed layout** on static storage (R2/S3-class bucket + CDN):

```
/<channel>/
  latest.yml            (signed by electron-builder identity)
  <app>-Setup-<ver>.exe / .blockmap (differential)
```

Chain enforced at update time ([04 ADR](04-SYSTEM-ARCHITECTURE.md#6-failure-domains--containment) + [14 § 2](14-SECURITY-MODEL.md#2-assets--adversaries)):

1. **TLS** to feed host (https-only, pinned provider CA set in config).
2. **Manifest signature** (electron-updater's built-in publisher verification against the signing identity compiled into the app — set at build, not runtime).
3. **Package hash** (sha512 in `latest.yml`) + Authenticode signature on the downloaded installer **before** execution.
4. **Version policy**: only forward updates within the same channel; downgrade attempts are rejected and logged (defense against rollback attacks).
5. **Staged apply + rollback**: install on quit; on next launch, a health check (engine loads, hello passes, viz frame renders headless) — failure ⇒ auto-rollback to previous version + telemetry event + banner. The updater itself is part of the trust boundary and is updated only through this same chain.

Channels: `stable`, `beta` (user-selectable, Settings → Updates). Update checks: every 24 h + manual; **never** auto-restart — user consent for the restart moment.

## 7. Release process

1. `release.yml` runs on tag `v*`: full CI matrix (see [17 § CI Gates](17-QUALITY-ENGINEERING.md#3-ci-gates-pr-blocking)) including perf benches on the self-hosted runner ([16](16-PERFORMANCE-BUDGETS.md)).
2. Artifacts built per-arch → signed → SBOM (CycloneDX) → provenance attestation (GitHub artifact attestations) → upload to feed bucket under channel path → `latest.yml` updated atomically (blue-green object swap).
3. Release notes generated from conventional commits + manual QA sign-off section (parity dashboard link, [03 § 12](03-PORTING-MATRIX-WINDIRSTAT.md#12-verification--sign-off-protocol)).
4. Website download links flip via feed metadata (no hardcoded versions in marketing pages).
5. Rollback procedure: revert bucket `latest.yml` to previous object (kept N-3), banner in app for affected version if needed — documented runbook `docs/runbooks/rollback.md`.

## 8. Supply chain

- Lockfiles frozen (`pnpm-lock.yaml`, `Cargo.lock`) and committed; renovate PRs reviewed by a human + CI perf diff before merge ([20 § Upgrade Policy](20-DEPENDENCY-MANIFEST.md#6-upgrade-policy)).
- `cargo-deny` config: deny GPL/AGPL dependencies in shipped crates (our code is proprietary; the clean-room posture of [03 § 0](03-PORTING-MATRIX-WINDIRSTAT.md#0-clean-room-protocol) applies to *our* output, and we also avoid GPL *dependencies* to keep the binary untangled), allow MIT/Apache-2.0/ISC/Unicode/BSD/Zlib, review-allow anything else explicitly with a doc note.
- npm side: only first-party + audited list ([20](20-DEPENDENCY-MANIFEST.md)); no postinstall scripts from third parties (pnpm config `side-effects-cache`, scripts allowlist).
- SBOM + attestation published with every release; `npm audit`/`cargo audit` gates fail on high advisories in prod deps.

## 9. Distribution channels

| Channel | Notes |
|---|---|
| Direct download (website) | primary; NSIS + portable |
| Beta feed | opt-in in-app |
| Microsoft Store (MSIX) | **post-GA** evaluation only — not a v1 commitment (scope [01 § 8](01-PRODUCT-VISION.md#8-scope-boundaries-v1)) |
