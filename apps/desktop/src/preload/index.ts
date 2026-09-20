// Preload: the ONLY bridge surface (contextBridge, docs/04 ADR-02).
import { contextBridge, ipcRenderer } from 'electron';
import type { EngineEvent } from '@prism/shared/generated';

export interface LicenseMirror {
  state:
    | { phase: 'unlicensed' }
    | { phase: 'trial'; startedAt: number; endsAt: number }
    | { phase: 'licensed'; sku: string; exp: number; lastValidatedAt: number }
    | { phase: 'grace'; sku: string; exp: number }
    | { phase: 'expired' };
  daysLeft: number | null;
}

contextBridge.exposeInMainWorld('prism', {
  invoke: (cmd: string, payload: unknown): Promise<unknown> => ipcRenderer.invoke('prism:invoke', cmd, payload),
  onEvents: (handler: (batch: EngineEvent[]) => void): (() => void) => {
    const listener = (_e: Electron.IpcRendererEvent, batch: EngineEvent[]): void => handler(batch);
    ipcRenderer.on('prism:events', listener);
    return () => ipcRenderer.removeListener('prism:events', listener);
  },
  // licensing (state mirrors only — tokens never cross, PRISM-IPC-054)
  license: (op: string, arg?: unknown): Promise<unknown> => ipcRenderer.invoke('prism:license', op, arg),
  onLicense: (handler: (mirror: LicenseMirror) => void): (() => void) => {
    const listener = (_e: Electron.IpcRendererEvent, mirror: LicenseMirror): void => handler(mirror);
    ipcRenderer.on('prism:license', listener);
    return () => ipcRenderer.removeListener('prism:license', listener);
  },
  // settings service (WDS-CFG-01)
  settings: (op: string, arg?: unknown): Promise<unknown> => ipcRenderer.invoke('prism:settings', op, arg),
  // app menu → renderer commands (docs/10 § 14)
  onMenu: (handler: (cmd: string) => void): (() => void) => {
    const listener = (_e: Electron.IpcRendererEvent, cmd: string): void => handler(cmd);
    ipcRenderer.on('prism:menu', listener);
    return () => ipcRenderer.removeListener('prism:menu', listener);
  },
  platform: process.platform,
  engineVersion: 'bundled',
});
