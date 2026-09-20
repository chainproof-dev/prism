// PrismClient — typed invoke/on wrappers over the preload bridge
// (docs/05 § 6.3). seq assignment, error normalization, event subscription.

import type { CommandName, CommandsMap, EngineEvent, EventName, EventsMap } from './generated';

/** Discriminated IPC error (PRISM-IPC-003 total mapping). */
export type IpcError =
  | { kind: 'invalid-args'; detail: string }
  | { kind: 'engine'; msg: string; internal: string | null }
  | { kind: 'not-licensed'; feature: string; sku: string | null }
  | { kind: 'busy'; operation: string }
  | { kind: 'cancelled'; scanId: number }
  | { kind: 'io'; code: number; path?: string }
  | { kind: 'entitlement'; reason: string };

/** Response envelope. */
export type IpcResponse<T> = { ok: true; data: T } | { ok: false; error: IpcError };

/** The preload-exposed surface (contextBridge, docs/04 ADR-02). */
export interface PrismBridge {
  invoke(cmd: string, payload: unknown): Promise<unknown>;
  onEvents(handler: (batch: EngineEvent[]) => void): () => void;
  platform: 'windows' | 'linux' | 'darwin';
  engineVersion: string;
}

declare global {
  interface Window {
    prism?: PrismBridge;
  }
}

/** Thrown error carrying the typed IpcError. */
export class PrismIpcError extends Error {
  constructor(public readonly ipc: IpcError) {
    super(`[${ipc.kind}] ${JSON.stringify(ipc)}`);
    this.name = 'PrismIpcError';
  }
}

/** Typed client. */
export class PrismClient {
  private seq = 0;
  constructor(private bridge: PrismBridge) {}

  /** Typed command invocation. */
  async invoke<C extends CommandName>(cmd: C, payload: CommandsMap[C]['req']): Promise<CommandsMap[C]['res']> {
    const seq = ++this.seq;
    const raw = await this.bridge.invoke(cmd, payload ?? null);
    const res = raw as IpcResponse<CommandsMap[C]['res']>;
    if (res && typeof res === 'object' && 'ok' in res) {
      if (res.ok) {
        return res.data;
      }
      throw new PrismIpcError(res.error);
    }
    throw new PrismIpcError({ kind: 'engine', msg: `malformed response for ${cmd} (seq ${seq})`, internal: null });
  }

  /** Subscribe to engine events; returns an unsubscribe fn. */
  onEvents(handler: (event: EngineEvent & { at?: number }) => void): () => void {
    return this.bridge.onEvents((batch) => {
      for (const ev of batch) {
        handler(ev);
      }
    });
  }

  /** Subscribe to one event kind. */
  on<E extends EventName>(event: E, handler: (payload: EventsMap[E]['payload']) => void): () => void {
    return this.onEvents((ev) => {
      if (ev.ev === (event as string)) {
        const payload = (ev as unknown as Record<string, unknown>)[payloadKey(event)];
        handler(payload as EventsMap[E]['payload']);
      }
    });
  }
}

/** Event payload field name per event kind (single-payload tagged union). */
function payloadKey(event: EventName): string {
  switch (event) {
    case 'scan:progress':
      return 'progress';
    case 'scan:nodes':
      return 'nodes';
    case 'scan:phase':
      return 'phase';
    case 'scan:error-batch':
      return 'errors';
    case 'scan:done':
      return 'done';
    case 'filter:updated':
      return 'filter';
    case 'dupes:progress':
      return 'dupes';
    case 'cleanup:progress':
      return 'cleanup';
    case 'monitor:sample':
      return 'sample';
    case 'license:changed':
      return 'sku';
    case 'engine:warning':
      return 'warning';
    default:
      return 'payload';
  }
}

/** Get the bridge from window (throws with an actionable message when absent). */
export function bridgeOrThrow(): PrismBridge {
  const b = globalThis.window?.prism;
  if (!b) {
    throw new Error('window.prism missing — preload bridge not exposed (dev server misconfiguration)');
  }
  return b;
}

type Bytes = number | bigint;

/** Byte formatting per docs/08 § 11 (SI decimals, binary toggle; bigint-aware). */
export function formatBytes(bytes: Bytes, opts?: { binary?: boolean; exact?: boolean }): string {
  if (opts?.exact) {
    return `${bytes.toLocaleString('en-US')} bytes`;
  }
  const unit = opts?.binary ? 1024 : 1000;
  const n = typeof bytes === 'bigint' ? Number(bytes) : bytes;
  if (n < unit) {
    return `${n} B`;
  }
  const units = opts?.binary
    ? ['KiB', 'MiB', 'GiB', 'TiB', 'PiB']
    : ['kB', 'MB', 'GB', 'TB', 'PB'];
  let v = n;
  let i = -1;
  do {
    v /= unit;
    i += 1;
  } while (v >= unit && i < units.length - 1);
  const digits = v < 10 ? 1 : 0;
  return `${v.toFixed(digits)} ${units[i]}`;
}

/** Percent format per docs/08 § 11 (bigint-safe share input). */
export function formatPercent(share: number | bigint): string {
  const pct = Number(share) * 100;
  if (pct > 0 && pct < 0.1) return '< 0.1%';
  return `${pct < 10 ? pct.toFixed(1) : Math.round(pct)}%`;
}

/** Duration format: `12s`, `4m 03s`, `1h 22m` (bigint-aware). */
export function formatDuration(ms: number | bigint): string {
  const s = Math.floor(Number(ms) / 1000);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ${String(s % 60).padStart(2, '0')}s`;
  const h = Math.floor(m / 60);
  return `${h}h ${m % 60}m`;
}

/** Rate format: `1.24M files/s` (bigint-aware). */
export function formatRate(rate: number | bigint): string {
  const rateN = Number(rate);
  if (rateN >= 1_000_000) return `${(rateN / 1_000_000).toFixed(2)}M files/s`;
  if (rateN >= 1_000) return `${(rateN / 1_000).toFixed(0)}k files/s`;
  return `${rateN.toFixed(0)} files/s`;
}
