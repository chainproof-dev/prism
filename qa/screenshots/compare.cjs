#!/usr/bin/env node
// P2-007 visual regression: compare qa/screenshots/shots/*.png against
// committed baselines in qa/screenshots/baselines/ with pixelmatch.
// Exit 1 on any diff above tolerance. Missing baselines are captured as
// new baselines (first run), reported as "new".

const { readFileSync, existsSync, mkdirSync, copyFileSync } = require('node:fs');
const { join } = require('node:path');

const DIR = __dirname;
const SHOTS = join(DIR, 'shots');
const BASE = join(DIR, 'baselines');

let pixelmatch;
let PNG;
try {
  ({ pixelmatch } = require('pixelmatch'));
  ({ PNG } = require('pngjs'));
} catch {
  console.error('pixelmatch/pngjs not installed — run: pnpm add -D pixelmatch pngjs');
  process.exit(2);
}

const { readdirSync } = require('node:fs');
const files = existsSync(SHOTS) ? readdirSync(SHOTS).filter((f) => f.endsWith('.png')) : [];
if (files.length === 0) {
  console.error('no shots found — run harness.cjs first');
  process.exit(2);
}

mkdirSync(BASE, { recursive: true });
let fail = 0;
let fresh = 0;
for (const f of files) {
  const shot = join(SHOTS, f);
  const base = join(BASE, f);
  if (!existsSync(base)) {
    copyFileSync(shot, base);
    console.log(`NEW  ${f} (baseline captured)`);
    fresh += 1;
    continue;
  }
  const a = PNG.sync.read(readFileSync(base));
  const b = PNG.sync.read(readFileSync(shot));
  if (a.width !== b.width || a.height !== b.height) {
    console.error(`FAIL ${f} — size mismatch ${a.width}x${a.height} vs ${b.width}x${b.height}`);
    fail += 1;
    continue;
  }
  const diff = new PNG({ width: a.width, height: a.height });
  const changed = pixelmatch(a.data, b.data, diff.data, a.width, a.height, {
    threshold: 0.12, // tolerant of AA/subpixel differences across GPUs
  });
  const total = a.width * a.height;
  const pct = (changed / total) * 100;
  if (pct > 0.5) {
    // >0.5% pixels changed = regression
    require('node:fs').writeFileSync(join(SHOTS, f.replace('.png', '.diff.png')), PNG.sync.write(diff));
    console.error(`FAIL ${f} — ${changed} px (${pct.toFixed(2)}%) changed`);
    fail += 1;
  } else {
    console.log(`OK   ${f} (${pct.toFixed(3)}% changed)`);
  }
}
console.log(`\n${files.length - fresh - fail} ok · ${fresh} new baselines · ${fail} regressions`);
process.exit(fail > 0 ? 1 : 0);
