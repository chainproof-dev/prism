// Typed engine access for the renderer (stores call these; components never
// touch IPC directly — PRISM-CMP-003).
import {
  PrismClient,
  bridgeOrThrow,
  type IpcError,
} from '@prism/shared/client';
import type {
  CommandName,
  CommandsMap,
  EngineEvent,
  NodeDetail,
  NodeRowsPage,
  PreflightInfo as PreflightInfoT,
  TypesPage,
  VolumeInfo,
} from '@prism/shared/generated';

export const client = new PrismClient(bridgeOrThrow());

export async function hello(): Promise<{ volumes: VolumeInfo[]; platform: string; features: string[] }> {
  return client.invoke('sys:hello', null);
}

export async function volumes(): Promise<VolumeInfo[]> {
  const page = await client.invoke('sys:volumes', { refresh: true });
  return page.volumes;
}

export async function startScan(target: { kind: 'volume'; path: string } | { kind: 'folder'; paths: string[] }): Promise<number> {
  const res = await client.invoke('scan:start', {
    target,
    strategy: 'standard',
    options: {
      followReparse: false,
      sizeMode: 'allocated',
      treatPackagesAsNodes: false,
      excludePatterns: [],
    },
  });
  return res.scanId;
}

export async function cancelScan(scanId: number): Promise<void> {
  await client.invoke('scan:cancel', { scanId });
}

export async function pauseScan(scanId: number): Promise<void> {
  await client.invoke('scan:pause', { scanId });
}

export async function resumeScan(scanId: number): Promise<void> {
  await client.invoke('scan:resume', { scanId });
}

export async function children(scanId: number, nodeId: number, offset = 0, limit = 500, sort: 'logical' | 'name' = 'logical', dir: 'asc' | 'desc' = 'desc'): Promise<NodeRowsPage> {
  return client.invoke('tree:children', { scanId, nodeId, sort: { key: sort, dir }, offset, limit });
}

export async function nodeDetail(scanId: number, nodeId: number): Promise<NodeDetail> {
  return client.invoke('node:detail', { scanId, nodeId });
}

export async function typesList(scanId: number): Promise<TypesPage> {
  return client.invoke('types:list', { scanId, sort: 'logical', dir: 'desc' });
}

export async function vizLayout(scanId: number, mode: number, root: number, w: number, h: number, dpr: number): Promise<ArrayBuffer> {
  const buf = await client.invoke('viz:layout', {
    scanId,
    mode: modeFromByte(mode),
    root,
    viewport: { w, h, dpr },
    options: {
      maxTiles: 60000,
      maxArcs: 12000,
      maxCircles: 8000,
      maxGraphNodes: 6000,
      minTilePx: 3,
      minShare: 0.0004,
      drawnDepth: 6,
      gapPx: 1,
      cushionElevation: 0.55,
      cushionFalloff: 0.66,
      labelDensity: 0.5,
      colorMode: 'type',
      sizeMode: 'allocated',
    },
  });
  // Buffer → ArrayBuffer (structured clone gives Uint8Array)
  return buf as unknown as ArrayBuffer;
}

function modeFromByte(_mode: number): 'treemap' {
  return 'treemap'; // viz frame mode list expands with the other modes' UI
}

export function onEngineEvents(handler: (ev: EngineEvent) => void): () => void {
  return client.onEvents(handler);
}

export { type IpcError, type CommandName, type CommandsMap };

// ---- extension surface (docs/05 § 3.3–3.7) --------------------------------

type GenPresetHit = import('@prism/shared/generated').PresetHit;
type GenAppRow = import('@prism/shared/generated').AppRow;
type GenSnapshotInfo = import('@prism/shared/generated').SnapshotInfo;

export async function preflight(target: string): Promise<PreflightInfoT> {
  return client.invoke('sys:preflight', { target });
}

export async function rescanSubtree(scanId: number, nodeId: number): Promise<number> {
  const res = await client.invoke('scan:rescan-subtree', {
    scanId, nodeId,
    options: { followReparse: false, sizeMode: 'allocated', treatPackagesAsNodes: false, excludePatterns: [] },
  });
  return res.scanId;
}

export async function resolvePath(scanId: number, path: string): Promise<number | null> {
  const res = await client.invoke('node:resolve-path', { scanId, path });
  return res.nodeId ?? null;
}

export async function colorMapping(
  scanId: number,
  mode: 'type' | 'branch' | 'age',
): Promise<import('@prism/shared/generated').ColorMapping> {
  return client.invoke('viz:color-mapping', { scanId, mode });
}

export async function setTypeColor(key: string, color: string): Promise<void> {
  await client.invoke('types:set-color', { key, color });
}

export type { DuplicateGroup } from '@prism/shared/generated';
export async function dupesRun(scanId: number, minSize = 1024n): Promise<number> {
  const res = await client.invoke('duplicates:run', { scanId, minSize, ioCapBps: 0n });
  return res.runId;
}
export async function dupesCancel(scanId: number): Promise<void> {
  await client.invoke('duplicates:cancel', { scanId });
}
export async function dupesGroups(
  runId: number,
  offset = 0,
  limit = 50,
): Promise<import('@prism/shared/generated').DupesGroupsPage> {
  return client.invoke('duplicates:groups', { runId, offset, limit });
}

export async function presetsScan(
  scanId: number,
): Promise<GenPresetHit[]> {
  const page = await client.invoke('cleanup:presets-scan', { scanId });
  return page.hits;
}

export async function appsList(
  includeSystem: boolean,
): Promise<GenAppRow[]> {
  const page = await client.invoke('apps:list', { includeSystem });
  return page.apps;
}
export async function appFootprint(token: string): Promise<import('@prism/shared/generated').AppFootprint> {
  return client.invoke('apps:footprint', { token });
}
export async function leftovers(
  scanId: number,
): Promise<GenAppRow[]> {
  const page = await client.invoke('apps:leftovers', { scanId });
  return page.apps;
}

export type { SnapshotInfo, SnapshotDelta } from '@prism/shared/generated';
export async function snapshotsSave(scanId: number, depth = 6): Promise<GenSnapshotInfo> {
  return client.invoke('snapshots:save', { scanId, depth });
}
export async function snapshotsList(root = ''): Promise<GenSnapshotInfo[]> {
  const page = await client.invoke('snapshots:list', { root });
  return page.snapshots;
}
export async function snapshotsDiff(
  before: number,
  after: number,
  floorBytes = 10 * 1024 * 1024,
): Promise<import('@prism/shared/generated').SnapshotDiffPage> {
  return client.invoke('snapshots:diff', {
    before,
    after,
    floorBytes: BigInt(floorBytes),
  });
}

export type { ProcessSample, MonitorSample } from '@prism/shared/generated';
export async function monitorStart(periodMs = 1000): Promise<void> {
  await client.invoke('monitor:start', { periodMs });
}
export async function monitorStop(): Promise<void> {
  await client.invoke('monitor:stop', null as never);
}

export async function exportScan(
  scanId: number,
  format: 'csv' | 'ndjson',
  scope: 'full' | 'selection' | 'filtered',
): Promise<import('@prism/shared/generated').ExportResult> {
  return client.invoke('export:scan', { scanId, format, scope, dest: '' });
}


export async function recentScans(
  limit = 6,
): Promise<import('@prism/shared/generated').ScanRecordDto[]> {
  const page = await client.invoke('engine:recent-scans', { limit });
  return page.records;
}
