# 14 — Security Model

> Owner doc: [00-INDEX.md](00-INDEX.md) · Upstream: [04-SYSTEM-ARCHITECTURE](04-SYSTEM-ARCHITECTURE.md), [13-LICENSING-SYSTEM](13-LICENSING-SYSTEM.md) · Downstream: [15-BUILD-PACKAGING](15-BUILD-PACKAGING.md), [17-QUALITY-ENGINEERING](17-QUALITY-ENGINEERING.md).
> Requirement IDs: `PRISM-SEC-*`.

---

## 1. Guiding rule

Assume the user's machine is honest but the network is hostile, and assume a determined attacker can fully inspect/patch the client. Therefore: **nothing that matters may be decided by the client alone** (license truth lives server-side + in signatures), and **nothing user-private may leave the machine**. Anti-tamper on the client is friction — layered but never trusted as a boundary ([13 § 8](13-LICENSING-SYSTEM.md#8-anti-abuse-measured-layered--threat-model-in-14--5)).

## 2. Assets & adversaries

| Asset | Adversary | Primary protection |
|---|---|---|
| User's file metadata (paths, sizes, names) | network observer, malware, us-the-vendor | no exfiltration channels at all ([04 § 8](04-SYSTEM-ARCHITECTURE.md#8-what-this-architecture-explicitly-does-not-do)); logs redact paths ([06 § 10](06-RUST-CORE.md#10-error-strategy-engine-wide)) |
| License truth | casual sharer, cracker, refund abuser | Ed25519 tokens + device binding + server authority ([13](13-LICENSING-SYSTEM.md)) |
| Update channel | supply-chain attacker (the highest-value target) | signed manifests + hash + version policy + staged rollback ([15 § 6](15-BUILD-PACKAGING.md#6-auto-update-security)) |
| Signed binary integrity | tamper distribution | Authenticode on every artifact; updater refuses unsigned |
| Elevated capability (turbo scan handle broker) | local privilege escalation | narrow helper, one consent, handle-only interface ([06 § 7](06-RUST-CORE.md#7-elevation-strategy)) |
| License server | key thieves, webhook forgers | rate limits, webhook signatures, no secrets in client, DPAPI at rest |

## 3. Electron hardening (release config is law — CI asserts every line of this table)

| Control | Setting | ID |
|---|---|---|
| Process isolation | `contextIsolation: true`, `nodeIntegration: false`, `sandbox: true`, `webSecurity: true` | `PRISM-SEC-001` |
| Navigation lock | `will-navigate` denied except `app://-` origin; `setWindowOpenHandler` → `deny` (all external opens go through explicit `shell.openExternal` with an https-only allowlist + user gesture) | `PRISM-SEC-002` |
| CSP | `default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self'` (no `unsafe-inline` — styles are built CSS; canvas has no CSP needs) | `PRISM-SEC-003` |
| No remote content | no `webview`, no iframes, no remote scripts — the renderer is a fully offline bundle | `PRISM-SEC-004` |
| Preload surface | only the typed `window.prism` bridge ([05 § 1](05-IPC-PROTOCOL.md#1-surfaces)); no `ipcRenderer` leak; channel allowlist in main | `PRISM-SEC-005` |
| IPC validation | zod schema per command at the main boundary; payload size caps; path args length-checked | `PRISM-SEC-006` ([05 § 7](05-IPC-PROTOCOL.md#7-validation-rules-at-boundaries)) |
| Protocol handler | custom `app://` protocol serves only bundled assets (no filesystem reads outside `resources/`) | `PRISM-SEC-007` |
| Devtools | disabled in production builds (`PRISM-SEC-008`); a debug build flag exists for support sessions, never shipped |
| Dependency supply chain | pnpm lockfile frozen; `pnpm audit --prod` gate; provenance checks on CI; SBOM generated per release ([15 § 8](15-BUILD-PACKAGING.md#8-supply-chain)) | `PRISM-SEC-009` |

## 4. Data handling

- **Telemetry:** opt-in, crash reports + anonymous feature-usage counters only. No file paths/names/sizes/volume labels in any payload — enforced by a serializer allowlist + CI payload snapshot tests (`PRISM-SEC-020`).
- **Logs:** on-disk logs at `%LOCALAPPDATA%` with path redaction by default ([06 § 10](06-RUST-CORE.md#10-error-strategy-engine-wide)); auto-purge > 14 days; user-clearable from Settings.
- **Persistence:** SQLite only ([07 § 4](07-DATA-MODEL.md#4-persistence-schema-sqlite-appdb)); snapshots are user-triggered and deletable; the disclosures in-product match this doc exactly (privacy text is generated from a single source string file used both in-app and on the site — no drift).
- **Clipboard:** app *writes* paths on explicit copy; never reads the clipboard without a paste gesture.

## 5. Licensing abuse — see [13 § 8](13-LICENSING-SYSTEM.md#8-anti-abuse-measured-layered--threat-model-in-14--5)

## 6. Hardening choices (client friction tier)

| Measure | Decision | Rationale |
|---|---|---|
| Symbol stripping | **Yes** (release profile `strip=true`, `panic=abort` per [06 § 10](06-RUST-CORE.md#10-error-strategy-engine-wide)) | free, no UX cost |
| String obfuscation of license constants | **Yes, light** (compile-time XOR of embedded public key + feature names) | raises grep-ability; zero runtime cost |
| Control-flow obfuscation | **No** | real perf cost on scan hot loops; weak return for a disk tool |
| Anti-debug | **No** | false positives hurt paying users; punishes the wrong people |
| Integrity self-check | **Yes, advisory** (engine verifies its own embedded public key hash + reports mismatches to telemetry; never bricks) | detect tamper distribution; don't insult customers |
| VM/sandbox detection | **No** | analysts + legit VM users exist; we don't play that game |

## 7. Elevation helper security (turbo scan)

- Helper is a signed, version-locked exe installed only with the app; host verifies helper version + signature (WinVerifyTrust) before speaking to it.
- Handshake: host generates a 256-bit nonce, launches helper with `runas`; helper opens the volume handle and duplicates it to the host over a named pipe (`\\.\pipe\prism-turbo-<nonce>`); helper exits immediately after one exchange. No command surface, no arguments beyond the pipe name, no persistence, no service install. AOD (audited) in [17 § Persona Reviews](17-QUALITY-ENGINEERING.md#6-persona-reviews-prism-qa-020) security pass.

## 8. Secrets inventory (what exists, where, who can read it)

| Secret | Location | Exposure |
|---|---|---|
| Ed25519 license **private** key | server secret store only | never on client, never in CI client artifacts; CI pair-check test ([13 § 7](13-LICENSING-SYSTEM.md#7-server-implementation-appslicense-server)) |
| License **public** key | compiled into engine | public by design |
| License key + token (user's) | Credential Manager + DPAPI blob | user scope; no plaintext files |
| Code-signing cert | CI protected env (Hardware token / Trusted Signing) | never on dev machines ([15 § 5](15-BUILD-PACKAGING.md#5-code-signing)) |
| Update signing key | electron-builder signing identity ([15 § 6](15-BUILD-PACKAGING.md#6-auto-update-security)) | CI protected env |
| Admin token (server) | server env | bearer on admin routes only |

## 9. Threat-model reviews

Formal mini-threat-model per phase gate (STRIDE-light) recorded in `docs/threat-models/phase-<n>.md`; the security persona review ([17 § Persona Reviews](17-QUALITY-ENGINEERING.md#6-persona-reviews-prism-qa-020)) signs off each. Any new network listener, IPC channel, file write, or registry key requires a threat-model diff in the PR.
