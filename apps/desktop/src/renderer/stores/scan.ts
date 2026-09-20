// Scan store (zustand; docs/10 § 15): phases, progress, deltas applied to a
// renderer-side incremental tree for live building (parity-SCN-02), then the
// completed scan's query surface.

import { create } from 'zustand';
import type { EngineEvent, NodeDelta, ScanProgress, ScanSummary, VolumeInfo } from '@prism/shared/generated';
import * as api from '../lib/prism';

export type Screen = 'welcome' | 'scanning' | 'explore';

/** Live tree node (deltas stream BigInt sizes — docs/05 § 3.3). */
interface LiveNode {
  id: number;
  parent: number;
  name: string;
  logical: bigint;
  allocated: bigint;
  kind: number;
}

interface ScanState {
  screen: Screen;
  scanId: number | null;
  volumes: VolumeInfo[];
  platform: string;
  progress: ScanProgress | null;
  phase: string;
  liveTree: Map<number, LiveNode>;
  summary: ScanSummary | null;
  selectedNode: number | null;
  hoveredNode: number | null;
  // actions
  loadVolumes: () => Promise<void>;
  beginScan: (target: { kind: 'volume'; path: string } | { kind: 'folder'; paths: string[] }) => Promise<void>;
  handleEvent: (ev: EngineEvent) => void;
  select: (node: number | null) => void;
  hover: (node: number | null) => void;
}

export const useScanStore = create<ScanState>((set, get) => ({
  screen: 'welcome',
  scanId: null,
  volumes: [],
  platform: 'unknown',
  progress: null,
  phase: 'idle',
  liveTree: new Map(),
  summary: null,
  selectedNode: null,
  hoveredNode: null,

  loadVolumes: async () => {
    const hello = await api.hello();
    set({ volumes: hello.volumes, platform: hello.platform });
  },

  beginScan: async (target) => {
    const scanId = await api.startScan(target);
    set({ scanId, screen: 'scanning', phase: 'walking', liveTree: new Map(), progress: null, summary: null, selectedNode: null });
  },

  handleEvent: (ev) => {
    const state = get();
    switch (ev.ev) {
      case 'scan-progress':
        set({ progress: ev.progress });
        break;
      case 'scan-nodes': {
        // rAF-batched delta application (PRISM-IPC-010)
        const tree = state.liveTree;
        for (const d of ev.nodes.deltas) {
          applyDelta(tree, d);
        }
        set({ liveTree: tree });
        break;
      }
      case 'scan-phase':
        set({ phase: ev.phase.phase });
        break;
      case 'scan-done':
        set({ summary: ev.done.summary, screen: 'explore', phase: 'done' });
        break;
      case 'scan-error-batch':
        // errors drawer data lands with the drawer UI (P3-008)
        break;
      default:
        break;
    }
  },

  select: (node) => set({ selectedNode: node }),
  hover: (node) => set({ hoveredNode: node }),
}));

function applyDelta(tree: Map<number, LiveNode>, d: NodeDelta): void {
  const existing = tree.get(d.id);
  if (existing) {
    tree.set(d.id, { ...existing, logical: BigInt(d.logical), allocated: BigInt(d.allocated) });
  } else {
    tree.set(d.id, { id: d.id, parent: d.parent, name: d.name, logical: BigInt(d.logical), allocated: BigInt(d.allocated), kind: d.kind });
  }
}
