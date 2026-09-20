// SQLite schema + migrations (docs/13 § 7). WAL mode, single writer.

import Database from 'better-sqlite3';
import { logger } from './logger';

export interface KeyRow {
  key: string;
  sku: 'yearly' | 'lifetime';
  status: 'active' | 'revoked';
  note: string | null;
  created_at: number;
  expires_at: number | null;
  max_devices: number;
}

export interface DeviceRow {
  key: string;
  instance_id: string;
  device_name: string;
  activated_at: number;
  last_seen: number;
}

export interface EventRow {
  id: number;
  ts: number;
  kind: string;
  key: string | null;
  instance_id: string | null;
  ip_hash: string | null;
  detail: string | null;
}

export class Db {
  private db: Database.Database;

  constructor(path: string) {
    this.db = new Database(path);
    this.db.pragma('journal_mode = WAL');
    this.db.pragma('synchronous = NORMAL');
    this.migrate();
  }

  private migrate(): void {
    this.db.exec(`
      CREATE TABLE IF NOT EXISTS keys (
        key TEXT PRIMARY KEY,
        sku TEXT NOT NULL CHECK (sku IN ('yearly','lifetime')),
        status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','revoked')),
        note TEXT,
        created_at INTEGER NOT NULL,
        expires_at INTEGER,
        max_devices INTEGER NOT NULL DEFAULT 3
      );
      CREATE TABLE IF NOT EXISTS devices (
        key TEXT NOT NULL REFERENCES keys(key),
        instance_id TEXT NOT NULL,
        device_name TEXT NOT NULL,
        activated_at INTEGER NOT NULL,
        last_seen INTEGER NOT NULL,
        PRIMARY KEY (key, instance_id)
      );
      CREATE TABLE IF NOT EXISTS events (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        ts INTEGER NOT NULL,
        kind TEXT NOT NULL,
        key TEXT,
        instance_id TEXT,
        ip_hash TEXT,
        detail TEXT
      );
      CREATE INDEX IF NOT EXISTS idx_events_key ON events(key, ts);
    `);
  }

  // --- keys -----------------------------------------------------------------

  upsertKey(row: KeyRow): void {
    this.db
      .prepare(
        `INSERT INTO keys (key, sku, status, note, created_at, expires_at, max_devices)
         VALUES (@key, @sku, @status, @note, @created_at, @expires_at, @max_devices)
         ON CONFLICT(key) DO UPDATE SET sku=@sku, status=@status, note=@note, expires_at=@expires_at, max_devices=@max_devices`,
      )
      .run(row);
  }

  getKey(key: string): KeyRow | undefined {
    return this.db.prepare('SELECT * FROM keys WHERE key = ?').get(key) as KeyRow | undefined;
  }

  listKeys(): KeyRow[] {
    return this.db.prepare('SELECT * FROM keys ORDER BY created_at DESC').all() as KeyRow[];
  }

  revokeKey(key: string): boolean {
    return this.db.prepare("UPDATE keys SET status = 'revoked' WHERE key = ?").run(key).changes > 0;
  }

  extendKey(key: string, addSeconds: number): boolean {
    const k = this.getKey(key);
    if (!k) return false;
    const base = k.expires_at ?? Date.now() / 1000;
    return (
      this.db
        .prepare('UPDATE keys SET expires_at = ?, status = ? WHERE key = ?')
        .run(base + addSeconds, k.status === 'revoked' ? 'revoked' : 'active', key).changes > 0
    );
  }

  // --- devices ----------------------------------------------------------------

  devicesOf(key: string): DeviceRow[] {
    return this.db.prepare('SELECT * FROM devices WHERE key = ? ORDER BY activated_at').all(key) as DeviceRow[];
  }

  addDevice(row: DeviceRow): void {
    this.db
      .prepare(
        `INSERT INTO devices (key, instance_id, device_name, activated_at, last_seen)
         VALUES (@key, @instance_id, @device_name, @activated_at, @last_seen)
         ON CONFLICT(key, instance_id) DO UPDATE SET last_seen=@last_seen, device_name=@device_name`,
      )
      .run(row);
  }

  removeDevice(key: string, instanceId: string): boolean {
    return (
      this.db
        .prepare('DELETE FROM devices WHERE key = ? AND instance_id = ?')
        .run(key, instanceId).changes > 0
    );
  }

  // --- events -------------------------------------------------------------------

  logEvent(kind: string, key: string | null, instanceId: string | null, ipHash: string | null, detail: string | null): void {
    this.db
      .prepare('INSERT INTO events (ts, kind, key, instance_id, ip_hash, detail) VALUES (?, ?, ?, ?, ?, ?)')
      .run(Date.now(), kind, key, instanceId, ipHash, detail);
  }

  stats(): { keys: number; active: number; devices: number; events: number } {
    const keys = (this.db.prepare('SELECT COUNT(*) c FROM keys').get() as { c: number }).c;
    const active = (
      this.db.prepare("SELECT COUNT(*) c FROM keys WHERE status='active'").get() as { c: number }
    ).c;
    const devices = (this.db.prepare('SELECT COUNT(*) c FROM devices').get() as { c: number }).c;
    const events = (this.db.prepare('SELECT COUNT(*) c FROM events').get() as { c: number }).c;
    return { keys, active, devices, events };
  }

  close(): void {
    this.db.close();
    logger.info('db closed');
  }
}
