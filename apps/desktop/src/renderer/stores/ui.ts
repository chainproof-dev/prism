// UI + license + cleanup stores (docs/10 § 15). No derived data (PRISM-FLOW-001).
import { create } from 'zustand';
import { t, setLocale, type Locale } from '../lib/i18n';

// ---------------------------------------------------------------- license
export interface LicenseMirrorState {
  phase:
    | 'unlicensed'
    | 'trial'
    | 'licensed'
    | 'grace'
    | 'expired';
  daysLeft: number | null;
  sku: string | null;
  exp: number | null;
  activateError: string | null;
  activating: boolean;
  refresh: () => Promise<void>;
  activate: (key: string) => Promise<boolean>;
  deactivate: () => Promise<void>;
  startTrial: () => Promise<void>;
}

interface LicenseMirror {
  state: { phase: string; exp?: number; sku?: string; endsAt?: number };
  daysLeft: number | null;
}

export const useLicenseStore = create<LicenseMirrorState>((set, get) => ({
  phase: 'unlicensed',
  daysLeft: null,
  sku: null,
  exp: null,
  activateError: null,
  activating: false,
  refresh: async () => {
    const api = window.prism?.license;
    const mirror = (await api?.('state')) as LicenseMirror | undefined;
    if (mirror) {
      set({
        phase: mirror.state.phase as LicenseMirrorState['phase'],
        daysLeft: mirror.daysLeft,
        sku: mirror.state.sku ?? null,
        exp: mirror.state.exp ?? mirror.state.endsAt ?? null,
      });
    }
  },
  activate: async (key: string) => {
    set({ activating: true, activateError: null });
    const apiA = window.prism?.license;
    const res = (await apiA?.('activate', key)) as
      | { ok: true }
      | { ok: false; code: string };
    set({ activating: false });
    if (res && res.ok) {
      await get().refresh();
      return true;
    }
    const code = res && !res.ok ? res.code : 'network';
    const msg =
      code === 'key_active_elsewhere'
        ? t('license.activeElsewhere', { n: 3 })
        : code === 'key_expired'
          ? t('license.expired')
          : code === 'key_revoked'
            ? t('license.revoked')
            : code === 'key_not_found'
              ? t('license.enterKey')
              : `${code}`;
    set({ activateError: msg });
    return false;
  },
  deactivate: async () => {
    const apiD = window.prism?.license;
    await apiD?.('deactivate');
    await get().refresh();
  },
  startTrial: async () => {
    const apiT = window.prism?.license;
    await apiT?.('start-trial');
    await get().refresh();
  },
}));

// ---------------------------------------------------------------- cleanup
export interface StagedItem {
  nodeId: number;
  path: string;
  bytes: number;
  source: string;
}

export interface CleanupState {
  staged: StagedItem[];
  ledgerOpen: boolean;
  executing: boolean;
  lastResult: { reclaimed: number; failed: number } | null;
  refreshQueue: () => Promise<void>;
  stage: (
    scanId: number,
    nodeIds: number[],
    source: string,
  ) => Promise<{ totals: { count: number; bytes: number }; rejected: { nodeId: number; reason: string }[] }>;
  unstage: (nodeIds: number[]) => Promise<void>;
  openLedger: () => void;
  closeLedger: () => void;
  execute: (scanId: number, toRecycleBin: boolean) => Promise<void>;
}

export const useCleanupStore = create<CleanupState>((set, get) => ({
  staged: [],
  ledgerOpen: false,
  executing: false,
  lastResult: null,
  refreshQueue: async () => {
    const page = (await window.prism?.invoke('cleanup:queue', null)) as { items: StagedItem[] } | undefined;
    if (page) set({ staged: page.items });
  },
  stage: async (scanId, nodeIds, source) => {
    const res = (await window.prism?.invoke('cleanup:stage', {
      scanId,
      nodeIds,
      source,
    })) as { totals: { count: number; bytes: number }; rejected: { nodeId: number; reason: string }[] };
    await get().refreshQueue();
    return res;
  },
  unstage: async (nodeIds) => {
    await window.prism?.invoke('cleanup:unstage', { nodeIds });
    await get().refreshQueue();
  },
  openLedger: () => set({ ledgerOpen: true }),
  closeLedger: () => set({ ledgerOpen: false }),
  execute: async (scanId, toRecycleBin) => {
    set({ executing: true });
    try {
      const res = (await window.prism?.invoke('cleanup:execute', {
        scanId,
        toRecycleBin,
        acknowledgedBlocks: 0,
      })) as { reclaimed: number; failed: number };
      set({ lastResult: res, ledgerOpen: res.failed > 0 });
    } finally {
      set({ executing: false });
      await get().refreshQueue();
    }
  },
}));

// ---------------------------------------------------------------- UI chrome
export type WorkspaceTab = 'explore' | 'duplicates' | 'applications' | 'monitor' | 'snapshots' | 'leftovers';
export type VizModeId = 'treemap' | 'sunburst' | 'icicle' | 'pack' | 'mindmap' | 'age-timeline' | 'folders' | 'table' | 'bars';
export type ColorModeId = 'type' | 'branch' | 'age';
export type ScopeId = 'folder' | 'files-anywhere' | 'folders-anywhere';

export interface UiState {
  tab: WorkspaceTab;
  vizMode: VizModeId;
  colorMode: ColorModeId;
  scope: ScopeId;
  paletteOpen: boolean;
  settingsOpen: boolean;
  errorsDrawerOpen: boolean;
  filter: string;
  filterCount: number | null;
  theme: string;
  locale: Locale;
  setTab: (tab: WorkspaceTab) => void;
  setVizMode: (mode: VizModeId) => void;
  setColorMode: (mode: ColorModeId) => void;
  setScope: (scope: ScopeId) => void;
  openPalette: () => void;
  closePalette: () => void;
  setSettingsOpen: (open: boolean) => void;
  setErrorsDrawerOpen: (open: boolean) => void;
  setFilter: (filter: string) => void;
  setFilterCount: (n: number | null) => void;
  setTheme: (theme: string) => void;
  setLocale: (l: Locale) => void;
}

export const useUiStore = create<UiState>((set) => ({
  tab: 'explore',
  vizMode: 'treemap',
  colorMode: 'type',
  scope: 'folder',
  paletteOpen: false,
  settingsOpen: false,
  errorsDrawerOpen: false,
  filter: '',
  filterCount: null,
  theme: 'nocturne',
  locale: 'en-US',
  setTab: (tab) => set({ tab }),
  setVizMode: (vizMode) => set({ vizMode }),
  setColorMode: (colorMode) => set({ colorMode }),
  setScope: (scope) => set({ scope }),
  openPalette: () => set({ paletteOpen: true }),
  closePalette: () => set({ paletteOpen: false }),
  setSettingsOpen: (settingsOpen) => set({ settingsOpen }),
  setErrorsDrawerOpen: (errorsDrawerOpen) => set({ errorsDrawerOpen }),
  setFilter: (filter) => set({ filter }),
  setFilterCount: (filterCount) => set({ filterCount }),
  setTheme: (theme) => {
    set({ theme });
    document.documentElement.dataset.theme = theme;
    const api = window.prism?.settings;
    if (api) void api('set', { general: { theme } } as never);
  },
  setLocale: (l) => {
    setLocale(l);
    set({ locale: l });
    const api = window.prism?.settings;
    if (api) void api('set', { general: { language: l } } as never);
  },
}));
