// Main process entry (docs/04, docs/14 § 3 hardening table).
import { app, BrowserWindow, ipcMain } from 'electron';
import { join } from 'node:path';
import { loadEngine, type EngineModule } from './engine/loader';
import { dispatch, type CommandRouterDeps } from './engine/router';
import type { EngineEvent } from '@prism/shared/generated';
import { randomUUID } from 'node:crypto';

// --- licensing client (main-side; renderer never sees tokens, PRISM-IPC-054)
const instanceId = randomUUID();
let cachedEntitlement: { token: string; exp: number } | null = null;

const license = {
  instanceId: () => instanceId,
  verifyEntitlement: async (feature: string): Promise<{ token: string } | null> => {
    if (!cachedEntitlement) {
      return null;
    }
    // engine-boundary verification (the engine is the gate, not this check)
    try {
      deps.engine.licVerifyToken(cachedEntitlement.token, feature);
      return { token: cachedEntitlement.token };
    } catch {
      return null;
    }
  },
};

let engine: EngineModule;
let deps: CommandRouterDeps;

function createWindow(): void {
  const win = new BrowserWindow({
    width: 1440,
    height: 900,
    minWidth: 1024,
    minHeight: 640,
    show: false,
    backgroundColor: '#17181f', // Nocturne surface-app (no white flash)
    titleBarStyle: process.platform === 'darwin' ? 'hiddenInset' : 'default',
    webPreferences: {
      preload: join(__dirname, '../preload/index.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
      webSecurity: true,
    },
  });
  win.once('ready-to-show', () => win.show());
  // dev server URL comes from electron-vite; in prod load the built index
  if (process.env.ELECTRON_RENDERER_URL) {
    void win.loadURL(process.env.ELECTRON_RENDERER_URL);
  } else {
    void win.loadFile(join(__dirname, '../renderer/index.html'));
  }
  // navigation lock (docs/14 § 3)
  win.webContents.on('will-navigate', (e, url) => {
    if (!url.startsWith('http://localhost') && !url.startsWith('file://')) {
      e.preventDefault();
    }
  });
}

app.whenReady().then(() => {
  try {
    engine = loadEngine();
  } catch (e) {
    // Startup diagnostic (fail-loud, actionable — never a white screen)
    const msg = e instanceof Error ? e.message : String(e);
    const win = new BrowserWindow({ width: 640, height: 320, backgroundColor: '#17181f' });
    void win.loadURL(`data:text/html,<body style="background:#17181f;color:#f2f3f7;font:14px ui-monospace;padding:32px"><h2 style="color:#e5533d">Engine failed to load</h2><pre style="white-space:pre-wrap">${msg.replaceAll('<', '&lt;')}</pre></body>`);
    return;
  }
  engine.engineSetInstanceId(instanceId);
  deps = { engine, license };

  // T1 command surface
  ipcMain.handle('prism:invoke', (_ev, cmd: string, payload: unknown) => dispatch(cmd, payload, deps));

  // T2 event pump → renderer fan-out
  engine.engineAttachEventSink((batchJson: string) => {
    const batch = JSON.parse(batchJson) as EngineEvent[];
    for (const win of BrowserWindow.getAllWindows()) {
      if (!win.isDestroyed()) {
        win.webContents.send('prism:events', batch);
      }
    }
  });

  createWindow();
  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit();
  }
});
