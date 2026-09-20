// Dev seed (docs/13 § 10, PRISM-LIC-050): demo keys seeded idempotently.
//   PRSM-DEMO-KEY-2026-0001  lifetime (the demo key)
//   PRSM-DEMO-KEY-2026-0002  yearly, far-future expiry
//   PRSM-DEMO-KEY-2026-0003  revoked (revocation UX testing)

import { resolve } from 'node:path';
import { Db } from '../db';

const DB_PATH = process.env.LICENSE_DB ?? resolve(process.cwd(), 'licenses.db');

export function seed(db: Db): void {
  const nowSec = Date.now() / 1000;
  db.upsertKey({
    key: 'PRSM-DEMO-KEY-2026-0001',
    sku: 'lifetime',
    status: 'active',
    note: 'demo (lifetime)',
    created_at: Date.now(),
    expires_at: null,
    max_devices: 3,
  });
  db.upsertKey({
    key: 'PRSM-DEMO-KEY-2026-0002',
    sku: 'yearly',
    status: 'active',
    note: 'demo (yearly)',
    created_at: Date.now(),
    expires_at: nowSec + 365 * 86_400 * 10, // far-future in dev
    max_devices: 3,
  });
  db.upsertKey({
    key: 'PRSM-DEMO-KEY-2026-0003',
    sku: 'lifetime',
    status: 'revoked',
    note: 'demo (revoked — UX testing)',
    created_at: Date.now(),
    expires_at: null,
    max_devices: 3,
  });
}

if (process.argv[1]?.includes('seed')) {
  const db = new Db(DB_PATH);
  seed(db);
  console.log(`seeded demo keys into ${DB_PATH}`);
  console.log('  PRSM-DEMO-KEY-2026-0001  lifetime (demo)');
  console.log('  PRSM-DEMO-KEY-2026-0002  yearly   (demo)');
  console.log('  PRSM-DEMO-KEY-2026-0003  revoked  (demo)');
  db.close();
}
