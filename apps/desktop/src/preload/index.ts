// Preload: the ONLY bridge surface (contextBridge, docs/04 ADR-02).
import { contextBridge, ipcRenderer } from 'electron';
import type { EngineEvent } from '@prism/shared/generated';

contextBridge.exposeInMainWorld('prism', {
  invoke: (cmd: string, payload: unknown): Promise<unknown> => ipcRenderer.invoke('prism:invoke', cmd, payload),
  onEvents: (handler: (batch: EngineEvent[]) => void): (() => void) => {
    const listener = (_e: Electron.IpcRendererEvent, batch: EngineEvent[]): void => handler(batch);
    ipcRenderer.on('prism:events', listener);
    return () => ipcRenderer.removeListener('prism:events', listener);
  },
  platform: process.platform,
  engineVersion: 'bundled',
});
