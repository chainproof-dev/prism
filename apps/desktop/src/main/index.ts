// Main process entry (docs/04, docs/14 § 3 hardening table).
import { app, BrowserWindow, ipcMain, Menu, dialog, Notification } from 'electron';
import { join } from 'node:path';
import { loadEngine, type EngineModule } from './engine/loader';
import { dispatch, type CommandRouterDeps } from './engine/router';
import type { EngineEvent, ScanSummary } from '@prism/shared/generated';
import * as licensing from './licensing/client';
import { loadSettings, saveSettings, resetSection, loadWindowState, saveWindowState, type PrismSettings } from './settings';

let engine: EngineModule;
let deps: CommandRouterDeps;
let mainWin: BrowserWindow | null = null;
let settings: PrismSettings;

// --- background-scan boot mode (PRISM-HG-080) ------------------------------
// Task Scheduler launches the exe with `--background-scan <path>`: run a
// headless STANDARD scan, auto-capture a snapshot, toast the result, quit.
// No window is ever created; failures land in the notification + log only.
const bgArgIdx = process.argv.indexOf('--background-scan');
const bgTargetArg = bgArgIdx >= 0 ? process.argv[bgArgIdx + 1] : undefined;
const backgroundTarget: string | null = typeof bgTargetArg === 'string' && bgTargetArg.length > 0 ? bgTargetArg : null;

function fmtBytes(n: number): string {
  if (n >= 1024 ** 4) return `${(n / 1024 ** 4).toFixed(1)} TB`;
  if (n >= 1024 ** 3) return `${(n / 1024 ** 3).toFixed(1)} GB`;
  if (n >= 1024 ** 2) return `${(n / 1024 ** 2).toFixed(1)} MB`;
  if (n >= 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${n} B`;
}

async function runBackgroundScan(target: string): Promise<void> {
  const started = engine.scanStart({
    target: { kind: 'folder', paths: [target] },
    strategy: 'standard',
    options: { followReparse: false, sizeMode: 'allocated', treatPackagesAsNodes: true, excludePatterns: [] },
  }) as { scanId: number };
  const scanId = started.scanId;
  await new Promise<void>((resolve) => {
    const finish = (delayMs: number): void => {
      setTimeout(() => resolve(), delayMs);
    };
    engine.engineAttachEventSink((batchJson: string) => {
      const batch = JSON.parse(batchJson) as EngineEvent[];
      for (const ev of batch) {
        if (ev.ev === 'scan-done' && ev.done) {
          const summary = (ev.done as { summary?: ScanSummary }).summary;
          if (summary) {
            try {
              // Auto-capture (docs/12 § 9): scheduled scan → snapshot.
              engine.snapshotsSave({ scanId, depth: 6 });
              const title = 'Prism scheduled scan complete';
              const body =
                `${target} — ${summary.files.toLocaleString()} files, ` +
                `${fmtBytes(Number(summary.allocated))}. Snapshot saved.`;
              if (Notification.isSupported()) {
                new Notification({ title, body }).show();
              }
            } catch {
              // quiet failure to notifications (docs/12 § 9) — never loud UI
            }
          }
          finish(1500); // let the toast surface before quit
          return;
        }
      }
    });
    // Safety valve: never hang a scheduled task (20 min cap).
    finish(20 * 60_000);
  });
  app.quit();
}

const license: CommandRouterDeps['license'] = {
  instanceId: () => licensing.instance(),
  verifyEntitlement: async (feature: string): Promise<{ token: string } | null> => {
    const tok = licensing.token();
    if (!tok) return null;
    // engine-boundary verification (the engine is the gate, not this check)
    try {
      engine.licVerifyToken(tok, feature);
      return { token: tok };
    } catch {
      return null;
    }
  },
};

function createWindow(): void {
  const ws = loadWindowState();
  const opts: Electron.BrowserWindowConstructorOptions = {
    width: ws.width,
    height: ws.height,
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
  };
  if (ws.x !== undefined && ws.y !== undefined) {
    opts.x = ws.x;
    opts.y = ws.y;
  }
  const win = new BrowserWindow(opts);
  mainWin = win;
  win.once('ready-to-show', () => {
    if (ws.maximized) win.maximize();
    win.show();
  });
  // window-state persistence (bounds on move/resize end + close)
  const persist = (): void => {
    if (win.isDestroyed()) return;
    const b = win.getBounds();
    saveWindowState({ x: b.x, y: b.y, width: b.width, height: b.height, maximized: win.isMaximized() });
  };
  win.on('resized', persist);
  win.on('moved', persist);
  win.on('close', persist);
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

function broadcastLicense(): void {
  for (const win of BrowserWindow.getAllWindows()) {
    if (!win.isDestroyed()) {
      win.webContents.send('prism:license', {
        state: licensing.current(),
        daysLeft: licensing.daysLeft(),
      });
    }
  }
}

function buildMenu(): Menu {
  const send = (cmd: string): void => mainWin?.webContents.send('prism:menu', cmd);
  const template: Electron.MenuItemConstructorOptions[] = [
    {
      label: 'File',
      submenu: [
        { label: 'New scan…', accelerator: 'CmdOrCtrl+N', click: () => send('new-scan') },
        { type: 'separator' },
        { label: 'Settings…', accelerator: 'CmdOrCtrl+,', click: () => send('open-settings') },
        { type: 'separator' },
        { role: 'quit' },
      ],
    },
    {
      label: 'View',
      submenu: [
        { label: 'Command palette', accelerator: 'CmdOrCtrl+K', click: () => send('command-palette') },
        { label: 'Search', accelerator: 'CmdOrCtrl+F', click: () => send('focus-search') },
        { label: 'Toggle inspector', accelerator: 'CmdOrCtrl+I', click: () => send('toggle-inspector') },
        { type: 'separator' },
        { role: 'toggleDevTools' },
      ],
    },
    {
      label: 'Help',
      submenu: [
        {
          label: 'About Prism',
          click: () =>
            void dialog.showMessageBox({
              type: 'info',
              title: 'About Prism',
              message: `Prism — every byte, accounted for.\n\nEngine ${engine.engineVersion()}\nApp ${app.getVersion()}\nPlatform ${process.platform} ${process.getSystemVersion()}\n\nLocal-only posture: nothing about your files leaves this computer.`,
              buttons: ['OK'],
            }),
        },
      ],
    },
  ];
  return Menu.buildFromTemplate(template);
}

app.whenReady().then(async () => {
  try {
    engine = loadEngine();
  } catch (e) {
    // Startup diagnostic (fail-loud, actionable — never a white screen)
    const msg = e instanceof Error ? e.message : String(e);
    const win = new BrowserWindow({ width: 640, height: 320, backgroundColor: '#17181f' });
    void win.loadURL(`data:text/html,<body style="background:#17181f;color:#f2f3f7;font:14px ui-monospace;padding:32px"><h2 style="color:#e5533d">Engine failed to load</h2><pre style="white-space:pre-wrap">${msg.replaceAll('<', '&lt;')}</pre></body>`);
    return;
  }

  settings = loadSettings();
  licensing.initLicensing();
  engine.engineSetInstanceId(licensing.instance());
  engine.engineOpenDb(join(app.getPath('userData'), 'app.db'));
  deps = { engine, license };

  // T1 command surface (window-aware: export needs the save dialog)
  ipcMain.handle('prism:invoke', (ev, cmd: string, payload: unknown) =>
    dispatch(cmd, payload, deps, BrowserWindow.fromWebContents(ev.sender)));

  // T2 event pump → renderer fan-out
  engine.engineAttachEventSink((batchJson: string) => {
    const batch = JSON.parse(batchJson) as EngineEvent[];
    for (const win of BrowserWindow.getAllWindows()) {
      if (!win.isDestroyed()) {
        win.webContents.send('prism:events', batch);
      }
    }
  });

  // Licensing surface (renderer → main only; state flows back via events)
  ipcMain.handle('prism:license', (_ev, op: string, arg: unknown) => {
    switch (op) {
      case 'state':
        return { state: licensing.current(), daysLeft: licensing.daysLeft() };
      case 'activate':
        return licensing.activate(String(arg));
      case 'deactivate':
        return licensing.deactivate();
      case 'start-trial':
        licensing.startTrial();
        return { ok: true };
      default:
        return { ok: false, code: 'unknown-op' };
    }
  });
  // notify on every state change (heartbeat heal, activation, expiry)
  const origHeartbeatTimer = setInterval(broadcastLicense, 60_000);
  app.on('before-quit', () => clearInterval(origHeartbeatTimer));

  // Settings surface (parity-CFG-01/03)
  ipcMain.handle('prism:settings', (_ev, op: string, arg: unknown) => {
    switch (op) {
      case 'get':
        return settings;
      case 'set': {
        settings = { ...settings, ...(arg as Partial<PrismSettings>) };
        saveSettings(settings);
        return settings;
      }
      case 'reset': {
        settings = resetSection(settings, arg as keyof PrismSettings);
        return settings;
      }
      default:
        return settings;
    }
  });

  Menu.setApplicationMenu(buildMenu());
  if (backgroundTarget) {
    // Headless boot: no window, no menu interaction — scan, snapshot, quit.
    await runBackgroundScan(backgroundTarget);
    return;
  }
  createWindow();
  broadcastLicense();
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
