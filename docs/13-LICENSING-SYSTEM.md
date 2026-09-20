# 13 — Licensing System

> Owner doc: [00-INDEX.md](00-INDEX.md) · Upstream: [01-PRODUCT-VISION](01-PRODUCT-VISION.md), [02-INTEL-DISKBUDDY](02-INTEL-DISKBUDDY.md#35-licensing-model-fully-reconstructed), [04-SYSTEM-ARCHITECTURE](04-SYSTEM-ARCHITECTURE.md) · Downstream: [06-RUST-CORE § 8](06-RUST-CORE.md#8-licensing-verification-inside-the-engine), [14-SECURITY-MODEL](14-SECURITY-MODEL.md), [15-BUILD-PACKAGING](15-BUILD-PACKAGING.md).
> Requirement IDs: `PRISM-LIC-*`.
>
> Taxonomy and UX patterns validated by competitive intel ([02 § 3.5](02-INTEL-DISKBUDDY.md#35-licensing-model-fully-reconstructed)); implementation entirely our own, and **stronger**: enforcement at the engine boundary, not only the UI ([02 `PRISM-INTEL-003`](02-INTEL-DISKBUDDY.md#35-licensing-model-fully-reconstructed)).

---

## 1. SKUs and feature matrix

Exactly two SKUs (R10). A license is bound to a **license key**; the key carries a product tier:

| Capability | Trial (14 days, full) | Yearly | Lifetime |
|---|---|---|---|
| All parity (Explore) features (scan standard, tree, treemap, all 9 viz modes, type colors, cleanup of manual selection) | ✅ | ✅ | ✅ |
| Turbo scan, duplicates, cleanup presets/ledger execute, uninstaller+leftovers, snapshots, monitor, scheduler, exports | ✅ | ✅ | ✅ |
| Duration | 14 days from first run, no card | 12 months from activation | perpetual for current major version line + all minors |
| Devices per key | 1 (the trial machine) | 3 (default, server-configurable) | 3 (default, server-configurable) |
| Support | community | email | email, priority |

`PRISM-LIC-001` — "Lifetime" is defined in the EULA as: the current major version at purchase + all updates within it; majors may be paid upgrades (at most one paid major per 24 months, stated publicly). This must be the exact copy shown at checkout and in the About screen — no ambiguity is allowed near money.

### Device policy

Default **3 devices per key** (both SKUs), server-configurable per key (`max_devices`) and adjustable by support via the admin API. Self-release: a user may deactivate *their own* device from any machine where the key is active; releasing *another* device requires support (admin `release-device`) — the activate-conflict dialog states this plainly. Device identity is the per-install `instance_id` (see § 4), not hardware fingerprints (MAC spoofing made those worthless — [02 § 3.5](02-INTEL-DISKBUDDY.md#35-licensing-model-fully-reconstructed)).

### Payment provider

The provider layer is an interface (`PaymentProvider`): `verifyWebhook`, `issueOrExtendKey`, `checkoutUrl(sku)`. **Dodo Payments is the default adapter** (merchant-of-record handles VAT/sales tax + email delivery; buy-links opened externally per [04 § 8](04-SYSTEM-ARCHITECTURE.md#8-what-this-architecture-explicitly-does-not-do)); a `ManualProvider` covers dev/QA. Adding a second provider (e.g., Stripe) must not touch client code — only server config + adapter.

## 2. Architecture

```
Desktop (main process)
 ├─ LicenseClient (TS)  ── HTTPS ──▶ License Server (apps/license-server)
 │    activate / validate / deactivate                │  keys, devices, entitlements
 ├─ SecureStore (Credential Manager + DPAPI blob)      │  Ed25519 signing (server-side)
 └─ Engine verifyEntitlement (Rust, [06 § 8])          │  payment webhooks (Dodo adapter)
        ▲ public key only                              ▼
        └──── signed entitlement tokens (CBOR+Ed25519) ─┘
```

**Trust split:** the server holds the private Ed25519 key; clients hold only the public key (compile-time injected, [06 § 8](06-RUST-CORE.md#8-licensing-verification-inside-the-engine)). Tokens are short-lived (grace window) signed CBOR claims. The client can *verify* everything offline; it can *obtain/extend* only via the server. Revocation/expiry take effect at next validation or token expiry — by design, not by accident.

## 3. API surface (license server)

Base URL: config `LICENSE_SERVER_URL`. **First build: `http://localhost:8080`** ([§ 10](#10-local-development--demo-key-owner-requirement-r11)); production URL injected at build from CI secrets (never committed). All requests/responses JSON; TLS in production (HSTS, no redirects). Rate limits per IP + per key (see § 8).

| Endpoint | Request | Response (success) |
|---|---|---|
| `POST /v1/licenses/activate` | `{ license_key, instance_id, device_name, app_version, os_build }` | `{ entitlement: TokenB64, grants: {...}, expires_at }` |
| `POST /v1/licenses/validate` | `{ license_key, instance_id }` | same as activate (also serves as heartbeat/refresh) |
| `POST /v1/licenses/deactivate` | `{ license_key, instance_id }` | `{ released: true }` |
| `GET  /v1/health` | — | `{ ok, version, time }` |

Error envelope (all endpoints): `{ error: { code, message, retry_after? } }` with codes exactly matching client UX states: `key_not_found` · `key_active_elsewhere` (with `{ devices: [{device_name, activated_at}] }` detail) · `key_expired` · `key_revoked` (refund/release) · `rate_limited` · `invalid_payload` · `server_error`. Code→UX copy mapping is 1:1 ([10 § 2](10-SCREENS-AND-FLOWS.md#2-licensegate--activation)); no generic "something went wrong" is ever shown for a known code — parity of honesty with the intel model ([02 § 3.5](02-INTEL-DISKBUDDY.md#35-licensing-model-fully-reconstructed)).

Admin (separate, bearer-auth `ADMIN_TOKEN`): `POST /v1/admin/keys` (issue key: sku, note) · `GET /v1/admin/keys?…` (list/search) · `POST /v1/admin/keys/:key/revoke` · `.../release-device` · `.../extend` · `GET /v1/admin/stats`. CLI wrapper: `pnpm lic:admin ...` (see § 10).

## 4. Entitlement token (the contract)

CBOR map, Ed25519 (`ed25519-dalek` server-side / Rust client; server runtime is Node — `@noble/ed25519` — sharing the exact CBOR schema with a cross-implementation property test):

```jsonc
{
  "v": 1,                          // token schema version
  "key_id": "sha256:…",            // key fingerprint (not the key itself)
  "sku": "yearly" | "lifetime",
  "features": ["turbo","dupes","cleanup","apps","snapshots","monitor","scheduler","export"],
  "instance_id": "uuid-v4-per-install",
  "device": { "name": "DESKTOP-AB12", "os": "build 26100", "app": "1.4.2" },
  "iat": 1758000000,               // issued-at (unix s)
  "exp": 1758345600,               // min(now+30d, sku_end) — drives offline grace
  "nonce": "random-16B-b64"
}
```

Rules: `exp − iat ≤ 30 days` (grace ceiling); engine rejects `iat` older than 7 days (replay bound, [06 § 8](06-RUST-CORE.md#8-licensing-verification-inside-the-engine)); `instance_id` mismatch ⇒ `entitlement.device-mismatch`. `PRISM-LIC-010`.

## 5. Client flows

### 5.1 Activation
1. User pastes key (`XXXX-XXXX-XXXX-XXXX[-XXXX]` auto-grouped, case-insensitive, whitespace-tolerant).
2. Main: derive/ensure `instance_id` (generated at first run, stored with the DPAPI blob), read device info, call `activate`.
3. Success → entitlement stored (Credential Manager: the license key + server URL; DPAPI-encrypted SQLite row: last entitlement + metadata), token passed to engine, UI unlocks, success checkmark moment ([09 § 2.3](09-UI-COMPONENTS.md#23-premiumanimated-moments-the-1020)).
4. `key_active_elsewhere` → dialog lists *other* devices with "Deactivate this device instead?" (self-release allowed only for the **calling** machine's own device — releasing others requires the admin/API — copy states this plainly).

### 5.2 Heartbeat & refresh
- On app start and every 24 h while running: `validate`.
- Sliding refresh: any validate response replaces the stored token.
- Clock skew: ±90 s tolerance; skew beyond ⇒ treated as needing online validation (no silent expiry).

### 5.3 Offline grace
- App functions fully while `now < exp`.
- At T−7d: Banner (countdown, "open app online before X to keep everything working").
- At expiry with no server contact: premium engine commands return `entitlement:expired` → **Explore (parity) stays functional**; premium tabs show the locked preview state ([10 § 12](10-SCREENS-AND-FLOWS.md#12-locked-tab-preview-conversion-surface)); reconnection auto-heals within one validate. This is deliberate product policy (paying users are never bricked by a vacation) — and it is *not* a security hole: no premium compute happens without a valid token.

### 5.4 Deactivation
Settings → Account → "Deactivate this machine" → confirm copy explains the release semantics (parity tone with intel model, our words) → `deactivate` → local state cleared → gate screen. Server frees the slot. Offline deactivate impossible (honest error with the reason; anti-abuse by design).

### 5.5 Trial
First run: gate offers "Start 14-day full trial" or "Enter license key". Trial state = local entitlement signed by... **no** — a trial is *not* a self-signed token (forgeable); trial is an **unsigned local counter** that unlocks parity + premium **engine** features only while a locally-stored, DPAPI-protected trial record says `active` and `now < start+14d`, **and** trial expiry does not need server truth (it's generous by design). Trial record: start timestamp, machine id, tamper-evident (DPAPI + HMAC with a per-install secret in the same blob). No network required to trial. `PRISM-LIC-020`.

## 6. Client storage

| Secret | Where | Notes |
|---|---|---|
| License key | Windows Credential Manager (`com.<vendor>.prism.license`) | user-scope; not in files |
| Entitlement token + iat/exp + sku | DPAPI-encrypted row in `app.db` (`CryptProtectData`, user scope) | [07 § 4](07-DATA-MODEL.md#4-persistence-schema-sqlite-appdb) |
| `instance_id` + per-install HMAC secret | same DPAPI blob | regenerated only by explicit "Reset install identity" (support tooling) |

No plaintext license material on disk, ever (`PRISM-LIC-030`). Diagnostics bundle shows only: sku, expiry date, last-validation result, instance id prefix.

## 7. Server implementation (`apps/license-server`)

- **Stack:** TypeScript, Fastify (v5), better-sqlite3 (WAL) for keys/devices/events; zod schemas shared with client via `packages/shared` (server subset). Single process, stateless beyond SQLite — trivially deployable (a $5 VM / Fly.io / Railway-class host suffices; capacity: licensing traffic is tiny by design).
- **Schema:**
```sql
CREATE TABLE keys  (key TEXT PRIMARY KEY, sku TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'active',
                    note TEXT, created_at INTEGER NOT NULL, expires_at INTEGER,          -- yearly end
                    max_devices INTEGER NOT NULL DEFAULT 3);
CREATE TABLE devices (key TEXT REFERENCES keys(key), instance_id TEXT, device_name TEXT,
                    activated_at INTEGER NOT NULL, last_seen INTEGER, PRIMARY KEY (key, instance_id));
CREATE TABLE events (id INTEGER PRIMARY KEY, ts INTEGER NOT NULL, kind TEXT NOT NULL,
                    key TEXT, instance_id TEXT, ip_hash TEXT, detail TEXT);   -- audit trail
CREATE INDEX idx_events_key ON events(key, ts);
```
- **Key format:** `PRSM-XXXXX-XXXXX-XXXXX-XXXXX` (Crockford base32, 80 bits entropy, check chars — human-proof). Demo key (dev seed): **`PRSM-DEMO-KEY-2026-0001`** with sku `lifetime` (see § 10). Generated keys: `pnpm lic:admin issue --sku yearly|lifetime --note "…"`.
- **Signer:** server holds `ED25519_PRIVATE_KEY` (env/secret store); public half committed to the client repo (`crates/prism-core/src/licensing/pubkey.rs` via build injection — CI verifies pair match in a dual-run test).
- **Payments:** pluggable `PaymentProvider` interface; **Dodo Payments adapter** default (webhook: `payment.succeeded` → issue/extend key → fulfillment email via provider's email or our SMTP), plus a `ManualProvider` for dev (issues the demo key). Webhook signature verification (per-provider scheme), idempotency by event id, replay window 15 min. Buy links: `checkout.<provider>/buy/<product>/<variant>` opened via `shell.openExternal` — never an in-app webview ([04 § 8](04-SYSTEM-ARCHITECTURE.md#8-what-this-architecture-explicitly-does-not-do)).
- **Security headers & transport:** HTTPS-only in prod (reverse proxy or Fastify TLS), strict CSP, no CORS (native clients), request body ≤ 4 KB, JSON-only, request logging with ip hashed (salted) — **no PII beyond device_name (user-editable, defaulted to hostname prefix)**.

## 8. Anti-abuse (measured, layered — threat model in [14 § 5](14-SECURITY-MODEL.md#5-licensing-abuse--see-13--8))

| Layer | Control |
|---|---|
| Server | rate limits: activate 5/min/IP, 10/h/key; validate 30/h/key; global health monitors; audit events; anomaly alerts (many activations across IPs) |
| Tokens | 30-day exp ceiling, 7-day iat window, nonce uniqueness enforced at validation (last 32 nonces per key) |
| Client | entitlement verified in engine per premium command (typestate, [06 § 8](06-RUST-CORE.md#8-licensing-verification-inside-the-engine)); UI gate is cosmetic only |
| Binary | code signing + obfuscation tier (strings/flow) per [14 § 6](14-SECURITY-MODEL.md#6-hardening-choices-client-friction-tier) — friction, not the boundary |

**Honest threat statement (product-internal):** a determined attacker with a debugger can always patch a local binary; the architecture ensures that (a) casual key-sharing dies on device binding, (b) forged entitlements are cryptographically impossible without the server key, (c) nothing server-side trusts client claims, and (d) the *cost* of cracking must exceed the product price — which for a disk tool it always will. This is the correct economic bar; we do not ship invasive anti-tamper (no kernel drivers, no online-only mode) because it punishes paying users (per [01 § 5](01-PRODUCT-VISION.md#5-product-principles-decision-tie-breakers) and the intel postmortem in [02](02-INTEL-DISKBUDDY.md#35-licensing-model-fully-reconstructed)).

## 9. Enforcement depth (engine boundary)

Premium commands list + gating mechanics: [05 § 3.7](05-IPC-PROTOCOL.md#37-premium-operations-entitlement-gated-13) and [06 § 8](06-RUST-CORE.md#8-licensing-verification-inside-the-engine). `PRISM-LIC-040`: the dispatcher makes it a **compile error** to register a premium handler without the `Verified<Ent>` parameter — enforced by the `premium_command!` macro; CI greps for handler registrations bypassing the macro.

## 10. Local development & demo key (owner requirement R11)

`PRISM-LIC-050` — the monorepo's root `package.json` wires:

- `npm run dev` (alias of `pnpm dev`) starts **both**: the license server on **`http://localhost:8080`** and the Electron app in dev mode, with `LICENSE_SERVER_URL=http://localhost:8080` injected into the desktop app config (same code path as production — only the URL differs, from `app.config.json` resolved at build time per [15 § 3](15-BUILD-PACKAGING.md#3-configuration-strategy)).
- The dev server seeds (idempotently): demo key `PRSM-DEMO-KEY-2026-0001` (sku lifetime, note `demo`), a yearly demo key `PRSM-DEMO-KEY-2026-0002` (expires far-future in dev), and a revoked key `PRSM-DEMO-KEY-2026-0003` (to test the revocation UX), plus a deterministic dev Ed25519 keypair (`dev-keys/ed25519.{pub,pem}`, gitignored in prod builds, committed for dev convenience — the **client in dev builds embeds the dev public key**; production builds embed the prod key and CI fails if dev keys leak into a release config).
- E2E tests run the same server binary on a random port; tests assert the full lifecycle: activate → validate → premium op OK → deactivate → premium op blocked → reactivate; offline grace: stop server → premium ops keep working until token `exp` (test uses a short-expiry dev token) → expiry flips tabs to locked preview → server back → auto-heal.
- Admin CLI in dev: `pnpm lic:admin issue --sku lifetime --note "qa"` prints a fresh key.

## 11. Test matrix (summary; full suite in [17](17-QUALITY-ENGINEERING.md))

| Case | Asserts |
|---|---|
| activate ok | 201, token verifies against public key, devices row created |
| activate second device (≤ max) | allowed; both listed |
| activate beyond max | `key_active_elsewhere` with device list |
| validate rotates nonce; replayed token rejected | engine `entitlement` error |
| deactivate frees slot; key works on next machine | flow |
| expired yearly → validate returns `key_expired`; lifetime never | logic |
| webhook idempotent (same event id twice → one extension) | payments |
| token forgery (bit flips in sig/claims) | engine verify fails, premium op blocked |
| clock skew ±90s ok, +10min rejected | engine |
| trial: no network at any point | local unlock works; expiry locks premium, parity stays |
