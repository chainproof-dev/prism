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
  ScanSummary,
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
