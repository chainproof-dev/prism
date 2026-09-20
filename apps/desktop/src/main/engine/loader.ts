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
