// Engine bridge: T1 command router (docs/05). Validates with generated zod
// schemas, routes to engine functions, normalizes errors into the
// discriminated IpcError envelope (PRISM-IPC-003/050).

import type { EngineModule } from './loader';
import type { IpcResponse } from '@prism/shared/client';
import * as Schemas from '@prism/shared/generated';

// T1 request validation with the GENERATED zod schemas (PRISM-IPC-050).
// Schema names are generated from Rust — drift is a codegen break, not a
// runtime surprise.
const REQUEST_VALIDATORS: Record<string, (v: unknown) => { ok: true } | { ok: false; detail: string }> = {
  'scan:start': (v) => validate(Schemas.ScanStartQuerySchema, v),
  'tree:children': (v) => validate(Schemas.ChildrenQuerySchema, v),
  'viz:layout': (v) => validate(Schemas.VizLayoutQuerySchema, v),
};

function validate(schema: { safeParse: (v: unknown) => { success: boolean; error?: { issues: { path: (string | number | symbol)[]; message: string }[] } } }, v: unknown): { ok: true } | { ok: false; detail: string } {
  const res = schema.safeParse(v);
  if (res.success) {
    return { ok: true };
  }
  return { ok: false, detail: res.error?.issues.map((i) => `${String(i.path.join('.'))}: ${i.message}`).join('; ') ?? 'schema rejection' };
}

export interface CommandRouterDeps {
  engine: EngineModule;
  license: {
    instanceId(): string;
    verifyEntitlement(feature: string): Promise<{ token: string } | null>;
  };
}

type Handler = (payload: unknown, deps: CommandRouterDeps) => unknown | Promise<unknown>;

function ok<T>(data: T): IpcResponse<T> {
  return { ok: true, data };
}
function fail(kind: string, extra: Record<string, unknown>): { ok: false; error: unknown } {
  return { ok: false, error: { kind, ...extra } };
}

function parseEngineError(e: unknown): { ok: false; error: unknown } {
  const msg = e instanceof Error ? e.message : String(e);
  // Engine errors carry a JSON payload in the message (ipc/mod.rs ne()/…)
  try {
    const parsed = JSON.parse(msg) as Record<string, unknown>;
    if (parsed && typeof parsed.kind === 'string') {
      return { ok: false, error: parsed };
    }
  } catch {
    // not a JSON payload — wrap as engine error
  }
  return fail('engine', { msg, internal: null });
}

/** The command table: cmd string → handler (mirrors the generated CommandsMap). */
export const handlers: Record<string, Handler> = {
  'sys:hello': (_p, { engine }) => engine.sysHello(),
  'sys:volumes': (_p, { engine }) => engine.sysVolumes(),
  'scan:start': (p, { engine }) => engine.scanStart(p),
  'scan:pause': (p, { engine }) => engine.scanPause((p as { scanId: number }).scanId),
  'scan:resume': (p, { engine }) => engine.scanResume((p as { scanId: number }).scanId),
  'scan:cancel': (p, { engine }) => engine.scanCancel((p as { scanId: number }).scanId),
  'scan:summary': (p, { engine }) => engine.scanSummary((p as { scanId: number }).scanId),
  'tree:children': (p, { engine }) => engine.treeChildren(p),
  'node:detail': (p, { engine }) => engine.nodeDetail((p as { scanId: number }).scanId, (p as { nodeId: number }).nodeId),
  'tree:expand-stats': (p, { engine }) => engine.treeExpandStats((p as { scanId: number }).scanId, (p as { nodeId: number }).nodeId),
  'viz:layout': (p, { engine }) => engine.vizLayout(p),
  'types:list': (p, { engine }) => engine.typesList((p as { scanId: number }).scanId, (p as { sort: string }).sort, (p as { dir: string }).dir),
  'filter:apply': (p, { engine }) => engine.filterApply((p as { scanId: number }).scanId, (p as { name: string }).name, (p as { kind: string }).kind ?? 'both'),
};

/** Dispatch one command; always resolves to the response envelope. */
export async function dispatch(
  cmd: string,
  payload: unknown,
  deps: CommandRouterDeps,
): Promise<IpcResponse<unknown> | { ok: false; error: unknown }> {
  const handler = handlers[cmd];
  if (!handler) {
    return fail('invalid-args', { detail: `unknown command: ${cmd}` });
  }
  const validator = REQUEST_VALIDATORS[cmd];
  if (validator) {
    const v = validator(payload);
    if (!v.ok) {
      return fail('invalid-args', { detail: `payload rejected: ${v.detail}` });
    }
  }
  try {
    return ok(await handler(payload, deps));
  } catch (e) {
    return parseEngineError(e);
  }
}
