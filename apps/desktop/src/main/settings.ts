// Settings service (parity-CFG-01): JSON in userData, main-owned, mirrored to
// the renderer over the bridge. Reset-per-section (parity-CFG-03) = spread the
// DEFAULTS over the section.

import { app } from 'electron';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

export interface PrismSettings {
  general: {
    theme: 'nocturne' | 'graphite' | 'verdigris' | 'ember' | 'alabaster' | 'terracotta';
    density: 'comfortable' | 'compact';
    sizeDisplay: 'allocated' | 'logical';
    language: string; // en-US | de-DE
    autostart: boolean;
    shellIntegration: boolean;
  };
  scanner: {
    strategyDefault: 'standard' | 'turbo';
    followReparse: boolean;
    packagesAsNodes: boolean;
    exclusions: string[];
  };
  viz: {
    defaultMode: string;
    labels: boolean;
    gap: number;
    cushionBrightness: number;
  };
  cleanup: {
    defaultMode: 'recycle' | 'permanent';
    confirmThresholdMb: number;
    presetsEnabled: Record<string, boolean>;
  };
  updates: {
    channel: 'stable' | 'beta';
  };
}

export const DEFAULT_SETTINGS: PrismSettings = {
  general: { theme: 'nocturne', density: 'comfortable', sizeDisplay: 'allocated', language: 'en-US', autostart: false, shellIntegration: false },
  scanner: { strategyDefault: 'standard', followReparse: false, packagesAsNodes: false, exclusions: [] },
  viz: { defaultMode: 'treemap', labels: true, gap: 2, cushionBrightness: 0.28 },
  cleanup: { defaultMode: 'recycle', confirmThresholdMb: 1024, presetsEnabled: {} },
  updates: { channel: 'stable' },
};

const SECTION_KEYS = ['general', 'scanner', 'viz', 'cleanup', 'updates'] as const;

function path(): string {
  return join(app.getPath('userData'), 'settings.json');
}

export function loadSettings(): PrismSettings {
  try {
    if (!existsSync(path())) return structuredClone(DEFAULT_SETTINGS);
    const parsed = JSON.parse(readFileSync(path(), 'utf8')) as Partial<PrismSettings>;
    const merged = structuredClone(DEFAULT_SETTINGS);
    for (const key of SECTION_KEYS) {
      if (parsed[key]) {
        (merged as unknown as Record<string, unknown>)[key] = {
          ...(DEFAULT_SETTINGS[key] as Record<string, unknown>),
          ...(parsed[key] as Record<string, unknown>),
        };
      }
    }
    return merged;
  } catch {
    return structuredClone(DEFAULT_SETTINGS); // corrupt settings = defaults, fail honest
  }
}

export function saveSettings(s: PrismSettings): void {
  writeFileSync(path(), JSON.stringify(s, null, 2), { mode: 0o600 });
}

export function resetSection(s: PrismSettings, section: keyof PrismSettings): PrismSettings {
  const next = structuredClone(s);
  (next as unknown as Record<string, unknown>)[section] = structuredClone(
      DEFAULT_SETTINGS[section],
    );
  saveSettings(next);
  return next;
}

// -- window state persistence (deferred from session 1) -----------------------

interface WindowState {
  x?: number;
  y?: number;
  width: number;
  height: number;
  maximized: boolean;
}

const DEFAULT_WIN: WindowState = { width: 1440, height: 900, maximized: false };

export function loadWindowState(): WindowState {
  try {
    if (!existsSync(join(app.getPath('userData'), 'window-state.json'))) return DEFAULT_WIN;
    return { ...DEFAULT_WIN, ...(JSON.parse(readFileSync(join(app.getPath('userData'), 'window-state.json'), 'utf8')) as Partial<WindowState>) };
  } catch {
    return DEFAULT_WIN;
  }
}

export function saveWindowState(ws: WindowState): void {
  writeFileSync(join(app.getPath('userData'), 'window-state.json'), JSON.stringify(ws));
}
