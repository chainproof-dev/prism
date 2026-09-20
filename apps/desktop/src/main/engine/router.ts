// Engine bridge: T1 command router (docs/05). Validates with generated zod
// schemas, routes to engine functions, normalizes errors into the
// discriminated IpcError envelope (PRISM-IPC-003/050).

import { dialog } from 'electron';
import type { EngineModule } from './loader';
import type { IpcResponse } from '@prism/shared/client';
import * as Schemas from '@prism/shared/generated';

// T1 request validation with the GENERATED zod schemas (PRISM-IPC-050).
const REQUEST_VALIDATORS: Record<string, (v: unknown) => { ok: true } | { ok: false; detail: string }> = {
  'scan:start': (v) => validate(Schemas.ScanStartQuerySchema, v),
  'tree:children': (v) => validate(Schemas.ChildrenQuerySchema, v),
  'viz:layout': (v) => validate(Schemas.VizLayoutQuerySchema, v),
  'sys:preflight': (v) => validate(Schemas.PreflightQuerySchema, v),
  'node:resolve-path': (v) => validate(Schemas.ResolvePathQuerySchema, v),
  'duplicates:run': (v) => validate(Schemas.DupesRunQuerySchema, v),
  'cleanup:stage': (v) => validate(Schemas.StageQuerySchema, v),
  'cleanup:unstage': (v) => validate(Schemas.UnstageQuerySchema, v),
  'cleanup:execute': (v) => validate(Schemas.ExecuteQuerySchema, v),
  'apps:list': (v) => validate(Schemas.AppsQuerySchema, v),
  'apps:footprint': (v) => validate(Schemas.AppFootprintQuerySchema, v),
  'snapshots:save': (v) => validate(Schemas.SnapshotSaveQuerySchema, v),
  'snapshots:diff': (v) => validate(Schemas.SnapshotDiffQuerySchema, v),
  'monitor:start': (v) => validate(Schemas.MonitorQuerySchema, v),
  'scheduler:upsert': (v) => validate(Schemas.SchedulerUpsertQuerySchema, v),
  'scheduler:delete': (v) => validate(Schemas.SchedulerDeleteQuerySchema, v),
  'scheduler:digest': (v) => validate(Schemas.SchedulerDigestQuerySchema, v),
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

type Handler = (payload: unknown, deps: CommandRouterDeps, win: Electron.BrowserWindow | null) => unknown | Promise<unknown>;

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

const sid = (p: unknown): number => (p as { scanId: number }).scanId;

/** The command table: cmd string → handler (mirrors the generated CommandsMap). */
export const handlers: Record<string, Handler> = {
  // 3.1 lifecycle & system
  'sys:hello': (_p, { engine }) => engine.sysHello(),
  'sys:volumes': (_p, { engine }) => engine.sysVolumes(),
  'sys:preflight': (p, { engine }) => engine.sysPreflight(p),
  // 3.2 scanning
  'scan:start': (p, { engine }) => engine.scanStart(p),
  'scan:pause': (p, { engine }) => engine.scanPause(sid(p)),
  'scan:resume': (p, { engine }) => engine.scanResume(sid(p)),
  'scan:cancel': (p, { engine }) => engine.scanCancel(sid(p)),
  'scan:summary': (p, { engine }) => engine.scanSummary(sid(p)),
  'scan:rescan-subtree': (p, { engine }) => engine.scanRescanSubtree(p),
  'scan:reattach': (p, { engine }) => engine.scanReattach(sid(p)),
  // 3.3 tree & nodes
  'tree:children': (p, { engine }) => engine.treeChildren(p),
  'tree:expand-stats': (p, { engine }) => engine.treeExpandStats(sid(p), (p as { nodeId: number }).nodeId),
  'node:detail': (p, { engine }) => engine.nodeDetail(sid(p), (p as { nodeId: number }).nodeId),
  'node:resolve-path': (p, { engine }) => engine.nodeResolvePath(p),
  // 3.4 viz + types
  'viz:layout': (p, { engine }) => engine.vizLayout(p),
  'viz:color-mapping': (p, { engine }) => engine.vizColorMapping(p),
  'types:list': (p, { engine }) => engine.typesList(sid(p), (p as { sort: string }).sort, (p as { dir: string }).dir),
  'types:set-color': (p, { engine }) => engine.typesSetColor(p),
  'filter:apply': (p, { engine }) => engine.filterApply(sid(p), (p as { name: string }).name, (p as { kind: string }).kind ?? 'both'),
  'filter:clear': () => null, // filter state is renderer-owned (docs/05 § 3.5)
  // 3.6 duplicates (premium — engine gates at the boundary)
  'duplicates:run': (p, { engine }) => engine.duplicatesRun(p),
  'duplicates:cancel': (p, { engine }) => engine.duplicatesCancel(sid(p)),
  'duplicates:groups': (p, { engine }) => engine.duplicatesGroups(p),
  // 3.7 cleanup ledger
  'cleanup:presets-scan': (p, { engine }) => engine.cleanupPresetsScan(sid(p)),
  'cleanup:stage': (p, { engine }) => engine.cleanupStage(p),
  'cleanup:unstage': (p, { engine }) => engine.cleanupUnstage(p),
  'cleanup:queue': (_p, { engine }) => engine.cleanupQueue(),
  'cleanup:execute': (p, { engine }) => engine.cleanupExecute(p),
  // applications
  'apps:list': (p, { engine }) => engine.appsList(p),
  'apps:footprint': (p, { engine }) => engine.appsFootprint(p),
  'apps:leftovers': (p, { engine }) => engine.appsLeftovers(p),
  // snapshots
  'snapshots:save': (p, { engine }) => engine.snapshotsSave(p),
  'snapshots:list': (p, { engine }) => engine.snapshotsList(p),
  'snapshots:diff': (p, { engine }) => engine.snapshotsDiff(p),
  // monitor
  'monitor:start': (p, { engine }) => engine.monitorStart(p),
  'monitor:stop': (_p, { engine }) => engine.monitorStop(),
  // scheduler (PRISM-HG-080). The runner exe is substituted main-side —
  // the renderer cannot point the task at an arbitrary binary.
  'scheduler:list': (_p, { engine }) => engine.schedulerList({}),
  'scheduler:upsert': (p, { engine }) => {
    const q = p as { spec: unknown; runnerExe?: string };
    return engine.schedulerUpsert({ spec: q.spec, runnerExe: process.execPath });
  },
  'scheduler:delete': (p, { engine }) => engine.schedulerDelete(p),
  'scheduler:digest': (p, { engine }) => engine.schedulerDigest(p),
  // export (save dialog resolved main-side; the wire payload's dest field is
  // ignored — the renderer passes a hint, we substitute the chosen path)
  'export:scan': async (p, { engine }, win) => {
    const q = p as { format: string; scope: string; dest?: string; scanId: number };
    const ext = q.format === 'ndjson' ? 'ndjson' : 'csv';
    const defaultName = `prism-scan-${new Date().toISOString().slice(0, 10)}.${ext}`;
    if (!win) {
      return fail('invalid-args', { detail: 'no window for the save dialog' });
    }
    const chosen = await dialog.showSaveDialog(win, {
      defaultPath: q.dest ?? defaultName,
      filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
    });
    if (chosen.canceled || !chosen.filePath) {
      return fail('cancelled', {});
    }
    return engine.exportScan({ ...q, dest: chosen.filePath });
  },
  // engine-internal
  'engine:recent-scans': (p, { engine }) => engine.engineRecentScans(((p as { limit: number }).limit ?? 6) as number),
  // licensing (main-only bridge, never exposed to the renderer directly)
  'lic:verify-token': (p, { engine, license }, _win) => {
    const { token: tok, feature } = p as { token: string; feature: string };
    void license; // the engine verifies; the router deps' client supplied it
    return engine.licVerifyToken(tok, feature);
  },
};

/** Dispatch one command; always resolves to the response envelope. */
export async function dispatch(
  cmd: string,
  payload: unknown,
  deps: CommandRouterDeps,
  win: Electron.BrowserWindow | null = null,
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
    return ok(await handler(payload, deps, win));
  } catch (e) {
    return parseEngineError(e);
  }
}

