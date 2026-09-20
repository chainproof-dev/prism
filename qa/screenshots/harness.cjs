#!/usr/bin/env node
// P2-007 screenshot harness: boots the REAL app (real engine, real scan)
// under Playwright and captures deterministic baselines for visual
// regression. States: welcome, settings, scanning (mid-scan), explore
// (post-scan). Run: node qa/screenshots/harness.cjs [--out DIR]
// Requires: built app (pnpm --filter desktop build) + engine .node copied.
// Linux/CI: wrap with xvfb-run.

const { _electron } = require('playwright');
const { mkdirSync, writeFileSync, mkdtempSync } = require('node:fs');
const { join } = require('node:path');
const { tmpdir } = require('node:os');

const ROOT = join(__dirname, '..', '..');
const OUT = process.argv.includes('--out')
  ? join(process.cwd(), process.argv[process.argv.indexOf('--out') + 1])
  : join(__dirname, 'shots');

async function main() {
  mkdirSync(OUT, { recursive: true });
  // Deterministic scan target: fixed tree in a temp dir.
  const target = mkdtempSync(join(tmpdir(), 'prism-shot-'));
  const sub = join(target, 'proj');
  mkdirSync(sub, { recursive: true });
  for (let i = 0; i < 40; i += 1) {
    writeFileSync(join(sub, `f${String(i).padStart(3, '0')}.bin`), Buffer.alloc(1024 * (i + 1), i));
  }
  mkdirSync(join(target, 'media'), { recursive: true });
  writeFileSync(join(target, 'media', 'big.mp4'), Buffer.alloc(4 * 1024 * 1024, 7));

  // electron binary lives in the desktop workspace; playwright at root.
  const electron = require(join(ROOT, 'apps', 'desktop', 'node_modules', 'electron'));
  console.log('launching app…');
  const app = await _electron.launch({
    args: [
      join(ROOT, 'apps', 'desktop', 'out', 'main', 'index.js'),
      '--disable-gpu',
      '--no-sandbox',
    ],
  });
  let win = app.windows()[0];
  if (!win) {
    // Electron 44 sometimes registers the window a tick late.
    win = await app.waitForEvent('window', { timeout: 10_000 });
  }
  await win.waitForLoadState('domcontentloaded');
  await win.setViewportSize({ width: 1440, height: 900 });
  const shot = async (name) => {
    await win.screenshot({ path: join(OUT, `${name}.png`) });
    console.log(`captured ${name}.png`);
  };

  // 0. license gate → start trial (deterministic dev flow, docs/13 § 5.1).
  await win.waitForTimeout(800);
  const gated = await win.evaluate(() => document.body.innerText.includes('ENTER LICENSE KEY'));
  if (gated) {
    await win.evaluate(() => window.prism?.license('start-trial'));
    await win.waitForTimeout(800);
    console.log('trial started (gate dismissed)');
  }

  // 1. welcome — the first stable screen
  await win.waitForTimeout(1200);
  await shot('01-welcome');

  // 2. settings sheet
  await win.keyboard.press('Control+Comma');
  await win.waitForTimeout(600);
  await shot('02-settings');
  // close via the backdrop (Escape is not bound to this sheet)
  await win.locator('div.fixed.inset-0.bg-black\\/40').click({ position: { x: 20, y: 450 } });
  await win.waitForTimeout(500);

  // 3. drive the UI like a user: click the first drive card (Welcome →
  // Scanning → Explore is the store's state machine, not raw invokes).
  const cards = win.locator('button.group');
  const n = await cards.count();
  console.log('drive cards:', n);
  if (n === 0) throw new Error('no drive cards rendered');
  await cards.first().click();
  await win.waitForTimeout(350);
  await shot('03-scanning');

  // 4. explore — wait for the scan-done event then capture
  await win.evaluate(
    () =>
      new Promise((resolve) => {
        const off = window.prism.onEvents((batch) => {
          if (batch.some((e) => e.ev === 'scan-done')) {
            off();
            resolve();
          }
        });
        setTimeout(resolve, 15_000); // safety valve
      }),
  );
  await win.waitForTimeout(1200);
  await shot('04-explore');

  await app.close();
  console.log(`done → ${OUT}`);
}

main().catch((e) => {
  console.error('screenshot harness failed:', e);
  process.exit(1);
});
