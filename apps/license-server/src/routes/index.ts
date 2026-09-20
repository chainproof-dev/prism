// API routes (docs/13 § 3). Error envelope codes map 1:1 to client UX states.

import type { FastifyInstance } from 'fastify';
import { z } from 'zod';
import type { Db } from '../db';
import { logger } from '../logger';
import { type Claims, mintClaims } from '../crypto/token';

const ActivateSchema = z.object({
  license_key: z.string().min(10).max(64),
  instance_id: z.string().uuid(),
  device_name: z.string().min(1).max(64),
  app_version: z.string().min(1).max(32),
  os_build: z.string().min(1).max(64),
});
const ValidateSchema = z.object({
  license_key: z.string().min(10).max(64),
  instance_id: z.string().uuid(),
});
const AdminIssueSchema = z.object({
  sku: z.enum(['yearly', 'lifetime']),
  note: z.string().max(200).optional(),
});
const AdminDeviceSchema = z.object({ instance_id: z.string().uuid() });
const AdminExtendSchema = z.object({ seconds: z.number().int().positive().max(60 * 60 * 24 * 3650) });

export interface RouteDeps {
  db: Db;
  sign: (claims: Claims) => Promise<string>;
  keyId: string;
  adminToken: string;
}

export function registerRoutes(app: FastifyInstance, deps: RouteDeps): void {
  const { db, sign, keyId, adminToken } = deps;

  const err = (code: string, message: string, extra?: Record<string, unknown>) => {
    const reply: Record<string, unknown> = { error: { code, message } };
    if (extra) {
      Object.assign(reply.error, extra);
    }
    return reply;
  };

  // --- health ---------------------------------------------------------------
  app.get('/v1/health', async () => ({
    ok: true,
    version: process.env.npm_package_version ?? '0.1.0',
    time: Date.now(),
  }));

  // --- activation -----------------------------------------------------------
  app.post('/v1/licenses/activate', async (req, reply) => {
    const parsed = ActivateSchema.safeParse(req.body);
    if (!parsed.success) {
      reply.code(400);
      return err('invalid_payload', 'request body failed schema validation', {
        detail: parsed.error.issues.map((i) => `${i.path.join('.')}: ${i.message}`),
      });
    }
    const { license_key, instance_id, device_name, app_version, os_build } = parsed.data;
    const row = db.getKey(license_key);
    if (!row) {
      db.logEvent('activate.miss', license_key, instance_id, null, null);
      reply.code(404);
      return err('key_not_found', 'this license key does not exist');
    }
    if (row.status === 'revoked') {
      db.logEvent('activate.revoked', license_key, instance_id, null, null);
      reply.code(403);
      return err('key_revoked', 'this license key was revoked (refund or release)');
    }
    const nowSec = Date.now() / 1000;
    if (row.sku === 'yearly' && row.expires_at !== null && row.expires_at <= nowSec) {
      db.logEvent('activate.expired', license_key, instance_id, null, null);
      reply.code(403);
      return err('key_expired', 'this yearly license expired — renew to continue');
    }
    const devices = db.devicesOf(license_key);
    const known = devices.find((d) => d.instance_id === instance_id);
    if (!known && devices.length >= row.max_devices) {
      db.logEvent('activate.limit', license_key, instance_id, null, `${devices.length}/${row.max_devices}`);
      reply.code(409);
      return err('key_active_elsewhere', 'device limit reached for this key', {
        devices: devices.map((d) => ({ device_name: d.device_name, activated_at: d.activated_at })),
      });
    }
    db.addDevice({
      key: license_key,
      instance_id,
      device_name,
      activated_at: known?.activated_at ?? Date.now(),
      last_seen: Date.now(),
    });
    const claims = mintClaims({
      sku: row.sku,
      instanceId: instance_id,
      deviceName: device_name,
      keyId,
      skuEndSeconds: row.sku === 'yearly' ? row.expires_at : null,
    });
    const token = await sign(claims);
    db.logEvent('activate.ok', license_key, instance_id, null, `${app_version} / ${os_build}`);
    reply.code(201);
    return {
      entitlement: token,
      grants: { sku: claims.sku, features: claims.features },
      expires_at: claims.exp,
    };
  });

  // --- validation / heartbeat ---------------------------------------------------
  app.post('/v1/licenses/validate', async (req, reply) => {
    const parsed = ValidateSchema.safeParse(req.body);
    if (!parsed.success) {
      reply.code(400);
      return err('invalid_payload', 'request body failed schema validation');
    }
    const { license_key, instance_id } = parsed.data;
    const row = db.getKey(license_key);
    if (!row) {
      reply.code(404);
      return err('key_not_found', 'this license key does not exist');
    }
    if (row.status === 'revoked') {
      reply.code(403);
      return err('key_revoked', 'this license key was revoked');
    }
    const nowSec = Date.now() / 1000;
    if (row.sku === 'yearly' && row.expires_at !== null && row.expires_at <= nowSec) {
      reply.code(403);
      return err('key_expired', 'this yearly license expired');
    }
    const device = db.devicesOf(license_key).find((d) => d.instance_id === instance_id);
    if (!device) {
      reply.code(409);
      return err('device-mismatch', 'this device is not activated on this key');
    }
    db.addDevice({ ...device, last_seen: Date.now() });
    const claims = mintClaims({
      sku: row.sku,
      instanceId: instance_id,
      deviceName: device.device_name,
      keyId,
      skuEndSeconds: row.sku === 'yearly' ? row.expires_at : null,
    });
    db.logEvent('validate.ok', license_key, instance_id, null, null);
    return {
      entitlement: await sign(claims),
      grants: { sku: claims.sku, features: claims.features },
      expires_at: claims.exp,
    };
  });

  // --- deactivation ----------------------------------------------------------
  app.post('/v1/licenses/deactivate', async (req, reply) => {
    const parsed = ValidateSchema.safeParse(req.body);
    if (!parsed.success) {
      reply.code(400);
      return err('invalid_payload', 'request body failed schema validation');
    }
    const { license_key, instance_id } = parsed.data;
    const ok = db.removeDevice(license_key, instance_id);
    if (!ok) {
      reply.code(404);
      return err('key_not_found', 'device not activated on this key');
    }
    db.logEvent('deactivate.ok', license_key, instance_id, null, null);
    return { released: true };
  });

  // --- admin (bearer token) ----------------------------------------------------
  app.addHook('onRequest', async (req, reply) => {
    if (!req.url.startsWith('/v1/admin')) {
      return;
    }
    const auth = req.headers.authorization;
    if (auth !== `Bearer ${adminToken}`) {
      reply.code(401);
      return err('unauthorized', 'admin token required');
    }
  });

  app.post('/v1/admin/keys', async (req, reply) => {
    const parsed = AdminIssueSchema.safeParse(req.body);
    if (!parsed.success) {
      reply.code(400);
      return err('invalid_payload', 'body: { sku: yearly|lifetime, note? }');
    }
    const key = generateKey();
    const nowSec = Date.now() / 1000;
    db.upsertKey({
      key,
      sku: parsed.data.sku,
      status: 'active',
      note: parsed.data.note ?? null,
      created_at: Date.now(),
      expires_at: parsed.data.sku === 'yearly' ? nowSec + 365 * 86_400 : null,
      max_devices: 3,
    });
    db.logEvent('admin.issue', key, null, null, parsed.data.note ?? null);
    reply.code(201);
    return { key, sku: parsed.data.sku };
  });

  app.get('/v1/admin/keys', async () => ({ keys: db.listKeys() }));

  app.get('/v1/admin/stats', async () => db.stats());

  app.post('/v1/admin/keys/:key/revoke', async (req, reply) => {
    const { key } = req.params as { key: string };
    if (!db.revokeKey(key)) {
      reply.code(404);
      return err('key_not_found', 'no such key');
    }
    db.logEvent('admin.revoke', key, null, null, null);
    return { revoked: true };
  });

  app.post('/v1/admin/keys/:key/release-device', async (req, reply) => {
    const { key } = req.params as { key: string };
    const parsed = AdminDeviceSchema.safeParse(req.body);
    if (!parsed.success) {
      reply.code(400);
      return err('invalid_payload', 'body: { instance_id }');
    }
    if (!db.removeDevice(key, parsed.data.instance_id)) {
      reply.code(404);
      return err('key_not_found', 'device not found on this key');
    }
    db.logEvent('admin.release', key, parsed.data.instance_id, null, null);
    return { released: true };
  });

  app.post('/v1/admin/keys/:key/extend', async (req, reply) => {
    const { key } = req.params as { key: string };
    const parsed = AdminExtendSchema.safeParse(req.body);
    if (!parsed.success) {
      reply.code(400);
      return err('invalid_payload', 'body: { seconds }');
    }
    if (!db.extendKey(key, parsed.data.seconds)) {
      reply.code(404);
      return err('key_not_found', 'no such key');
    }
    db.logEvent('admin.extend', key, null, null, `${parsed.data.seconds}s`);
    return { extended: true };
  });
}

/** Key format: PRSM-XXXXX-XXXXX-XXXXX-XXXXX (Crockford base32, 80 bits).
 * Dev-seeded demo keys use fixed recognizable strings (seed.ts). */
const CROCKFORD = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';
export function generateKey(): string {
  const rand = (len: number): string => {
    let out = '';
    for (let i = 0; i < len; i++) {
      out += CROCKFORD[Math.floor(Math.random() * CROCKFORD.length)];
    }
    return out;
  };
  return `PRSM-${rand(5)}-${rand(5)}-${rand(5)}-${rand(5)}`;
}

export { logger };
