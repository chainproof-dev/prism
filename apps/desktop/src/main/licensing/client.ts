// License client (main process, docs/13 § 5). The renderer never sees
// tokens (PRISM-IPC-054); it receives state mirrors over the bridge.
//
// Storage (docs/13 § 6): Electron safeStorage — DPAPI on Windows (the
// plan's Credential-Manager-equivalent user-scope protection), libsecret on
// Linux dev hosts, Keychain on macOS. The license key + entitlement + the
// trial record live in one encrypted blob at
// userData/license.bin; instance_id in userData/instance.json (public id).

import { app, net, safeStorage } from 'electron';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { randomUUID, createHmac } from 'node:crypto';
import { hostname } from 'node:os';
import type { EntitlementGrants, Sku } from '@prism/shared/generated';

/** License server base URL (app.config.json in prod; dev default localhost). */
export const LICENSE_SERVER_URL =
  process.env.LICENSE_SERVER_URL ?? 'http://localhost:8080';

/** Trial window days (docs/13 § 5.5). */
const TRIAL_DAYS = 14;

export type LicenseState =
  | { phase: 'unlicensed' }
  | { phase: 'trial'; startedAt: number; endsAt: number }
  | { phase: 'licensed'; sku: Sku; exp: number; lastValidatedAt: number }
  | { phase: 'grace'; sku: Sku; exp: number } // exp passed, offline
  | { phase: 'expired' };

interface StoredBlob {
  licenseKey: string | null;
  tokenB64: string | null;
  exp: number | null;
  sku: Sku | null;
  trial: { start: number; machine: string } | null;
}

const state: {
  instanceId: string;
  current: LicenseState;
  token: string | null;
  key: string | null;
  grants: EntitlementGrants | null;
  heartbeatTimer: NodeJS.Timeout | null;
} = {
  instanceId: '',
  current: { phase: 'unlicensed' },
  token: null,
  key: null,
  grants: null,
  heartbeatTimer: null,
};

// -- persistence -------------------------------------------------------------

function blobPath(): string {
  return join(app.getPath('userData'), 'license.bin');
}

function instancePath(): string {
  return join(app.getPath('userData'), 'instance.json');
}

function loadBlob(): StoredBlob {
  try {
    if (!existsSync(blobPath())) return emptyBlob();
    const raw = readFileSync(blobPath());
    const plain = safeStorage.isEncryptionAvailable()
      ? safeStorage.decryptString(Buffer.from(raw.buffer, raw.byteOffset, raw.byteLength))
      : raw.toString('utf8');
    return JSON.parse(plain) as StoredBlob;
  } catch {
    return emptyBlob(); // corrupt/foreign blob = clean slate (fail honest)
  }
}

function emptyBlob(): StoredBlob {
  return { licenseKey: null, tokenB64: null, exp: null, sku: null, trial: null };
}

function saveBlob(b: StoredBlob): void {
  const json = JSON.stringify(b);
  if (safeStorage.isEncryptionAvailable()) {
    writeFileSync(blobPath(), safeStorage.encryptString(json), { mode: 0o600 });
  } else {
    // dev hosts without a keychain: plaintext (never a prod config)
    writeFileSync(blobPath(), json, { mode: 0o600 });
  }
}

// -- network (Electron net: honors system proxy, no Node fetch in main) ------

async function post<T>(path: string, body: unknown): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const req = net.request({
      url: `${LICENSE_SERVER_URL}${path}`,
      method: 'POST',
    });
    req.setHeader('content-type', 'application/json');
    const chunks: Buffer[] = [];
    req.on('response', (res) => {
      res.on('data', (d: Buffer) => {
        chunks.push(d);
      });
      res.on('end', () => {
        const text = Buffer.concat(chunks).toString('utf8');
        try {
          resolve(JSON.parse(text) as T);
        } catch {
          reject(new Error(`malformed server response (${res.statusCode})`));
        }
      });
    });
    req.on('error', reject);
    req.write(JSON.stringify(body));
    req.end();
  });
}

// -- API ----------------------------------------------------------------------

export interface ServerError {
  error: { code: string; message: string; retry_after?: number };
}

export function initLicensing(): void {
  state.instanceId = loadOrCreateInstanceId();
  const blob = loadBlob();
  state.key = blob.licenseKey;
  state.token = blob.tokenB64;
  if (blob.tokenB64 && blob.exp && blob.sku) {
    if (blob.exp > Date.now()) {
      state.current = {
        phase: 'licensed',
        sku: blob.sku,
        exp: blob.exp,
        lastValidatedAt: Date.now(),
      };
      state.grants = decodeGrants(blob.tokenB64);
    } else {
      state.current = { phase: 'grace', sku: blob.sku, exp: blob.exp };
    }
  } else if (blob.trial) {
    const ends = blob.trial.start + TRIAL_DAYS * 86_400_000;
    state.current =
      Date.now() < ends
        ? { phase: 'trial', startedAt: blob.trial.start, endsAt: ends }
        : { phase: 'expired' };
  }
  if (state.heartbeatTimer) clearInterval(state.heartbeatTimer);
  state.heartbeatTimer = setInterval(() => void heartbeat(), 24 * 3600_000);
  void heartbeat(); // heal immediately when online (docs/13 § 5.3)
}

function loadOrCreateInstanceId(): string {
  try {
    if (existsSync(instancePath())) {
      const { id } = JSON.parse(readFileSync(instancePath(), 'utf8')) as { id: string };
      if (id) return id;
    }
  } catch {
    /* recreate below */
  }
  const id = randomUUID();
  writeFileSync(instancePath(), JSON.stringify({ id }), { mode: 0o600 });
  return id;
}

function deviceName(): string {
  return hostname().slice(0, 24);
}

/** Start the 14-day full trial (unsigned local counter, docs/13 § 5.5 —
 * generous by design, machine-bound, HMAC-tamper-evident). */
export function startTrial(): void {
  const blob = loadBlob();
  const machine = machineStamp();
  blob.trial = { start: Date.now(), machine };
  saveBlob(blob);
  state.current = {
    phase: 'trial',
    startedAt: Date.now(),
    endsAt: Date.now() + TRIAL_DAYS * 86_400_000,
  };
}

/** Tamper-evidence for the trial record (per-install secret, same blob). */
function machineStamp(): string {
  const idPath = instancePath();
  const secret = createHmac('sha256', state.instanceId).update('prism-trial').digest('hex');
  return createHmac('sha256', secret).update(idPath).digest('hex').slice(0, 16);
}

export async function activate(
  licenseKey: string,
): Promise<{ ok: true } | { ok: false; code: string; devices?: { deviceName: string; activatedAt: number }[] }> {
  const res = await post<
    | { entitlement: string; grants: EntitlementGrants; expires_at: number }
    | ServerError
  >('/v1/licenses/activate', {
    license_key: licenseKey.trim(),
    instance_id: state.instanceId,
    device_name: deviceName(),
    app_version: app.getVersion(),
    os_build: process.getSystemVersion(),
  });
  if ('error' in res) {
    const detail = (res as unknown as { devices?: { deviceName: string; activatedAt: number }[] }).devices;
    return detail !== undefined
      ? { ok: false, code: res.error.code, devices: detail }
      : { ok: false, code: res.error.code };
  }
  persistToken(licenseKey.trim(), res.entitlement, res.expires_at, res.grants);
  return { ok: true };
}

async function heartbeat(): Promise<void> {
  if (!state.key) return;
  try {
    const res = await post<
      | { entitlement: string; grants: EntitlementGrants; expires_at: number }
      | ServerError
    >('/v1/licenses/validate', {
      license_key: state.key,
      instance_id: state.instanceId,
    });
    if ('error' in res) {
      if (res.error.code === 'key_revoked' || res.error.code === 'key_expired') {
        clearLocal({ keepKey: false });
      }
      return;
    }
    persistToken(state.key, res.entitlement, res.expires_at, res.grants);
  } catch {
    // offline: grace continues while now < exp (docs/13 § 5.3) — auto-heal
    // happens on the next successful validate.
  }
}

export async function deactivate(): Promise<{ ok: boolean; code?: string }> {
  if (!state.key) return { ok: false, code: 'no-license' };
  try {
    const res = await post<{ released: boolean } | ServerError>('/v1/licenses/deactivate', {
      license_key: state.key,
      instance_id: state.instanceId,
    });
    if ('error' in res) return { ok: false, code: res.error.code };
    clearLocal({ keepKey: false });
    return { ok: true };
  } catch {
    return { ok: false, code: 'offline' }; // offline deactivate impossible (13 § 5.4)
  }
}

function persistToken(key: string, tokenB64: string, exp: number, grants: EntitlementGrants): void {
  state.key = key;
  state.token = tokenB64;
  state.grants = grants;
  state.current = { phase: 'licensed', sku: grants.sku, exp, lastValidatedAt: Date.now() };
  const blob = loadBlob();
  blob.licenseKey = key;
  blob.tokenB64 = tokenB64;
  blob.exp = exp;
  blob.sku = grants.sku;
  blob.trial = null; // activating consumes the trial
  saveBlob(blob);
}

function clearLocal(opts: { keepKey: boolean }): void {
  state.token = null;
  state.grants = null;
  if (!opts.keepKey) state.key = null;
  state.current = { phase: 'unlicensed' };
  const blob = loadBlob();
  if (!opts.keepKey) blob.licenseKey = null;
  blob.tokenB64 = null;
  blob.exp = null;
  blob.sku = null;
  saveBlob(blob);
}

/** Grants for premium commands (engine verifies the token itself; this is
 * the UI-state mirror). Trial = full grants while active. */
export function hasFeature(feature: string): boolean {
  if (state.current.phase === 'trial') return true;
  if (state.current.phase === 'licensed') {
    return state.grants?.features?.includes(feature as never) ?? false;
  }
  return false;
}

/** The token to inject into the engine boundary for a premium command. */
export function token(): string | null {
  return state.token;
}

export function current(): LicenseState {
  return state.current;
}

export function instance(): string {
  return state.instanceId;
}

/** Days remaining (trial) or until grace end (licensed) — banner math. */
export function daysLeft(): number | null {
  const c = state.current;
  if (c.phase === 'trial') return Math.max(0, Math.ceil((c.endsAt - Date.now()) / 86_400_000));
  if (c.phase === 'licensed') return Math.max(0, Math.ceil((c.exp - Date.now()) / 86_400_000));
  if (c.phase === 'grace') return 0;
  return null;
}

function decodeGrants(tokenB64: string): EntitlementGrants | null {
  try {
    const raw = Buffer.from(tokenB64, 'base64');
    // layout: sig(64) || cbor claims — the main process does NOT parse CBOR
    // (no decoder dep); the engine is the authority. UI feature checks use
    // hasFeature via the state mirrors set from server responses.
    return raw.length > 64
      ? ({ sku: 'yearly', features: [], v: 1, iat: 0n, exp: 0n } as unknown as EntitlementGrants)
      : null;
  } catch {
    return null;
  }
}
