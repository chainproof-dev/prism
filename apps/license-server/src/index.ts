// Server entry (docs/13 § 7 + § 10). Default: http://localhost:8080 (R11).

import Fastify from 'fastify';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { Db } from './db';
import { loadKeyPair } from './crypto/token';
import { registerRoutes } from './routes';
import { logger } from './logger';

const PORT = Number(process.env.PORT ?? 8080);
const HOST = process.env.HOST ?? '127.0.0.1';
const DB_PATH = process.env.LICENSE_DB ?? resolve(process.cwd(), 'licenses.db');
const KEYS_PATH = process.env.DEV_KEYS ?? resolve(process.cwd(), '../../dev-keys/ed25519.pem');
const ADMIN_TOKEN = process.env.ADMIN_TOKEN ?? 'prism-dev-admin-token';

async function main(): Promise<void> {
  const db = new Db(DB_PATH);
  const pem = readFileSync(KEYS_PATH, 'utf8');
  const { publicKeyHex, sign } = await loadKeyPair(pem);
  logger.info({ publicKeyHex: publicKeyHex.slice(0, 16) + '…' }, 'signing key loaded');

  const app = Fastify({
    loggerInstance: logger as never,
    bodyLimit: 4096, // docs/13 § 7: request body ≤ 4 KiB
  });

  registerRoutes(app, { db, sign, keyId: `sha256:${publicKeyHex.slice(0, 16)}`, adminToken: ADMIN_TOKEN });

  // graceful shutdown
  for (const sig of ['SIGINT', 'SIGTERM'] as const) {
    process.on(sig, () => {
      void app.close().then(() => db.close());
      process.exit(0);
    });
  }

  await app.listen({ port: PORT, host: HOST });
  logger.info(`license server on http://${HOST}:${PORT} (health: /v1/health)`);
}

main().catch((e: unknown) => {
  logger.error(e, 'fatal');
  process.exit(1);
});
