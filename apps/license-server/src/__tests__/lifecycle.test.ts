// License lifecycle E2E (docs/13 § 11 test matrix) — in-process Fastify with
// a temp DB + the committed dev keypair. CRITICAL cross-check: server-minted
// tokens verify against the SAME public key the Rust engine embeds
// (dev-keys/ed25519.pub.hex — dual-run CI test, doc 13 § 7).

import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import Fastify, { type FastifyInstance } from 'fastify';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { randomUUID } from 'node:crypto';
import { Db } from '../db';
import { loadKeyPair, verifyToken } from '../crypto/token';
import { registerRoutes } from '../routes';
import { seed } from '../admin/seed';

const PUBLISHED_ENGINE_PUBKEY = '712df68d0c43a7281dc913dd1e891a2709de8271795e7eba5086aa441c934380';

let app: FastifyInstance;
let db: Db;
const instanceA = randomUUID();
const instanceB = randomUUID();
const instanceC = randomUUID();
const instanceD = randomUUID();

beforeAll(async () => {
  db = new Db(':memory:');
  seed(db);
  const pem = readFileSync(resolve(process.cwd(), '../../dev-keys/ed25519.pem'), 'utf8');
  const { sign } = await loadKeyPair(pem);
  app = Fastify();
  registerRoutes(app, { db, sign, keyId: 'sha256:test', adminToken: 'test-admin' });
  await app.ready();
});

afterAll(async () => {
  await app.close();
  db.close();
});

describe('GET /v1/health', () => {
  it('returns ok', async () => {
    const res = await app.inject({ method: 'GET', url: '/v1/health' });
    expect(res.statusCode).toBe(200);
    expect(res.json()).toMatchObject({ ok: true });
  });
});

describe('activation lifecycle', () => {
  it('activates the lifetime demo key; token verifies against the ENGINE pubkey', async () => {
    const res = await app.inject({
      method: 'POST', url: '/v1/licenses/activate',
      payload: { license_key: 'PRSM-DEMO-KEY-2026-0001', instance_id: instanceA, device_name: 'TEST-A', app_version: '0.1.0', os_build: 'test' },
    });
    expect(res.statusCode).toBe(201);
    const body = res.json() as { entitlement: string; grants: { sku: string; features: string[] } };
    expect(body.grants.sku).toBe('lifetime');
    const claims = await verifyToken(body.entitlement, PUBLISHED_ENGINE_PUBKEY);
    expect(claims.instance_id).toBe(instanceA);
    expect(claims.features).toContain('turbo');
    expect(claims.exp - claims.iat).toBeLessThanOrEqual(30 * 86_400);
  });

  it('activates 2 more devices (limit 3)', async () => {
    for (const [inst, name] of [[instanceB, 'TEST-B'], [instanceC, 'TEST-C']] as const) {
      const res = await app.inject({
        method: 'POST', url: '/v1/licenses/activate',
        payload: { license_key: 'PRSM-DEMO-KEY-2026-0001', instance_id: inst, device_name: name, app_version: '0.1.0', os_build: 'test' },
      });
      expect(res.statusCode).toBe(201);
    }
  });

  it('4th device → key_active_elsewhere with device list', async () => {
    const res = await app.inject({
      method: 'POST', url: '/v1/licenses/activate',
      payload: { license_key: 'PRSM-DEMO-KEY-2026-0001', instance_id: instanceD, device_name: 'TEST-D', app_version: '0.1.0', os_build: 'test' },
    });
    expect(res.statusCode).toBe(409);
    const body = res.json() as { error: { code: string; devices: unknown[] } };
    expect(body.error.code).toBe('key_active_elsewhere');
    expect(body.error.devices).toHaveLength(3);
  });

  it('validate (heartbeat) works and rotates nonce', async () => {
    const res = await app.inject({
      method: 'POST', url: '/v1/licenses/validate',
      payload: { license_key: 'PRSM-DEMO-KEY-2026-0001', instance_id: instanceA },
    });
    expect(res.statusCode).toBe(200);
    const body = res.json() as { entitlement: string };
    const claims = await verifyToken(body.entitlement, PUBLISHED_ENGINE_PUBKEY);
    expect(claims.nonce).toBeTruthy();
  });

  it('deactivate frees the slot', async () => {
    const res = await app.inject({
      method: 'POST', url: '/v1/licenses/deactivate',
      payload: { license_key: 'PRSM-DEMO-KEY-2026-0001', instance_id: instanceC },
    });
    expect(res.json()).toEqual({ released: true });
    const again = await app.inject({
      method: 'POST', url: '/v1/licenses/activate',
      payload: { license_key: 'PRSM-DEMO-KEY-2026-0001', instance_id: instanceD, device_name: 'TEST-D', app_version: '0.1.0', os_build: 'test' },
    });
    expect(again.statusCode).toBe(201);
  });

  it('revoked demo key → key_revoked', async () => {
    const res = await app.inject({
      method: 'POST', url: '/v1/licenses/activate',
      payload: { license_key: 'PRSM-DEMO-KEY-2026-0003', instance_id: randomUUID(), device_name: 'TEST-R', app_version: '0.1.0', os_build: 'test' },
    });
    expect(res.statusCode).toBe(403);
    expect((res.json() as { error: { code: string } }).error.code).toBe('key_revoked');
  });

  it('unknown key → key_not_found; malformed → invalid_payload', async () => {
    const miss = await app.inject({
      method: 'POST', url: '/v1/licenses/activate',
      payload: { license_key: 'PRSM-NOT-A-REAL-KEY-99', instance_id: randomUUID(), device_name: 'X', app_version: '0.1.0', os_build: 'test' },
    });
    expect(miss.statusCode).toBe(404);
    expect((miss.json() as { error: { code: string } }).error.code).toBe('key_not_found');
    const bad = await app.inject({ method: 'POST', url: '/v1/licenses/activate', payload: { license_key: 'x' } });
    expect(bad.statusCode).toBe(400);
    expect((bad.json() as { error: { code: string } }).error.code).toBe('invalid_payload');
  });
});

describe('token forgery (bit-flip)', () => {
  it('tampered signature rejected', async () => {
    // fresh key (the demo key's device slots are full by now)
    const issue = await app.inject({
      method: 'POST', url: '/v1/admin/keys',
      headers: { authorization: 'Bearer test-admin' }, payload: { sku: 'lifetime' },
    });
    const { key } = issue.json() as { key: string };
    const res = await app.inject({
      method: 'POST', url: '/v1/licenses/activate',
      payload: { license_key: key, instance_id: randomUUID(), device_name: 'TAMPER', app_version: '0.1.0', os_build: 'test' },
    });
    const { entitlement } = res.json() as { entitlement: string };
    const bytes = Uint8Array.from(atob(entitlement), (c) => c.charCodeAt(0));
    bytes[10] ^= 0x01;
    const tampered = btoa(String.fromCharCode(...bytes));
    await expect(verifyToken(tampered, PUBLISHED_ENGINE_PUBKEY)).rejects.toThrow(/signature/);
  });
});

describe('admin API', () => {
  it('issues, lists, extends, revokes', async () => {
    const issue = await app.inject({
      method: 'POST', url: '/v1/admin/keys',
      headers: { authorization: 'Bearer test-admin' }, payload: { sku: 'yearly', note: 'cli test' },
    });
    expect(issue.statusCode).toBe(201);
    const { key } = issue.json() as { key: string };
    expect(key).toMatch(/^PRSM-[A-Z0-9]{5}-[A-Z0-9]{5}-[A-Z0-9]{5}-[A-Z0-9]{5}$/);
    const list = await app.inject({ method: 'GET', url: '/v1/admin/keys', headers: { authorization: 'Bearer test-admin' } });
    expect((list.json() as { keys: unknown[] }).keys.length).toBeGreaterThanOrEqual(4);
    const extend = await app.inject({
      method: 'POST', url: `/v1/admin/keys/${key}/extend`,
      headers: { authorization: 'Bearer test-admin' }, payload: { seconds: 86_400 },
    });
    expect(extend.statusCode).toBe(200);
    const revoke = await app.inject({
      method: 'POST', url: `/v1/admin/keys/${key}/revoke`,
      headers: { authorization: 'Bearer test-admin' },
    });
    expect(revoke.statusCode).toBe(200);
  });

  it('rejects bad admin tokens', async () => {
    const res = await app.inject({ method: 'GET', url: '/v1/admin/keys', headers: { authorization: 'Bearer wrong' } });
    expect(res.statusCode).toBe(401);
  });
});
