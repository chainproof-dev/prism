// Engine module loader — fail-loud with the startup diagnostic screen when
// the native module is missing or ABI-mismatched (docs/04 § 6: NEVER a white
// screen, NEVER a silent fallback).
import { existsSync } from 'node:fs';
import { join } from 'node:path';

export interface EngineModule {
  engineInit(): void;
  engineVersion(): string;
  engineSetInstanceId(id: string): void;
  engineAttachEventSink(cb: (batchJson: string) => void): void;
  engineEventQueueDepth(): number;
  sysHello(): unknown;
  sysVolumes(): unknown;
  scanStart(payload: unknown): { scanId: number };
  scanPause(scanId: number): void;
  scanResume(scanId: number): void;
  scanCancel(scanId: number): void;
  scanSummary(scanId: number): unknown;
  scanPhase(scanId: number): string;
  treeChildren(payload: unknown): unknown;
  nodeDetail(scanId: number, nodeId: number): unknown;
  treeExpandStats(scanId: number, nodeId: number): unknown;
  vizLayout(payload: unknown): Buffer;
  typesList(scanId: number, sort: string, dir: string): unknown;
  filterApply(scanId: number, name: string, kind: string): unknown;
  licVerifyToken(tokenB64: string, feature: string): unknown;
  // -- extension surface (ipc/extend.rs) -------------------------------------
  sysPreflight(payload: unknown): unknown;
  scanRescanSubtree(payload: unknown): unknown;
  scanReattach(scanId: number): unknown;
  nodeResolvePath(payload: unknown): unknown;
  vizColorMapping(payload: unknown): unknown;
  typesSetColor(payload: unknown): void;
  duplicatesRun(payload: unknown): unknown;
  duplicatesCancel(scanId: number): void;
  duplicatesGroups(payload: unknown): unknown;
  cleanupPresetsScan(scanId: number): unknown;
  cleanupStage(payload: unknown): unknown;
  cleanupUnstage(payload: unknown): unknown;
  cleanupQueue(): unknown;
  cleanupExecute(payload: unknown): unknown;
  appsList(payload: unknown): unknown;
  appsFootprint(payload: unknown): unknown;
  appsLeftovers(payload: unknown): unknown;
  snapshotsSave(payload: unknown): unknown;
  snapshotsList(payload: unknown): unknown;
  snapshotsDiff(payload: unknown): unknown;
  monitorStart(payload: unknown): void;
  monitorStop(): void;
  schedulerList(payload: unknown): unknown;
  schedulerUpsert(payload: unknown): unknown;
  schedulerDelete(payload: unknown): void;
  schedulerDigest(payload: unknown): unknown;
  exportScan(payload: unknown): unknown;
  engineOpenDb(path: string): void;
  engineSetSetting(key: string, valueJson: string): void;
  engineGetSetting(key: string): unknown;
  engineRecentScans(limit: number): unknown;
}

const CANDIDATES = [
  join(__dirname, '../../resources/prism-core.node'),
  join(process.resourcesPath ?? '', 'prism-core.node'),
];

export function loadEngine(): EngineModule {
  for (const path of CANDIDATES) {
    if (path && existsSync(path)) {
      // eslint-disable-next-line @typescript-eslint/no-require-imports
      const mod = require(path) as EngineModule;
      mod.engineInit();
      return mod;
    }
  }
  const detail = `prism-core.node not found — looked in: ${CANDIDATES.filter(Boolean).join(', ')}. Run \`pnpm build:engine\` (cargo build -p prism-core) first.`;
  throw new Error(detail);
}
