// Session-2 engine smoke: full pipeline through the REAL .node module,
// including the new command surface (preflight, presets, staging, ledger
// gating, resolve, color-mapping, frames, settings, history, turbo refusal).
const e = require('/home/z/my-project/prism/apps/desktop/resources/prism-core.node');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

function die(msg) { console.error('FAIL:', msg); process.exit(1); }
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const events = [];
e.engineInit();
e.engineSetInstanceId('smoke-instance');
e.engineAttachEventSink((batchJson) => {
  for (const ev of JSON.parse(batchJson)) events.push(ev);
});
console.log('engine', e.engineVersion());


(async () => {
// db + preflight
const dbPath = path.join(os.tmpdir(), `prism-smoke-${Date.now()}.db`);
e.engineOpenDb(dbPath);
const pre = e.sysPreflight({ target: os.tmpdir() });
if (pre.readable !== true) die('preflight readable');
console.log('preflight ok (free', Number(pre.freeBytes), 'bytes)');

// scan target with reclaimable + duplicate content
const root = fs.mkdtempSync(path.join(os.tmpdir(), 'prism-smoke2-'));
fs.mkdirSync(path.join(root, 'proj', 'node_modules', 'left-pad'), { recursive: true });
fs.writeFileSync(path.join(root, 'proj', 'node_modules', 'left-pad', 'x.js'), 'x'.repeat(50_000));
fs.writeFileSync(path.join(root, 'proj', 'a.bin'), Buffer.alloc(120_000, 7));
fs.writeFileSync(path.join(root, 'b.bin'), Buffer.alloc(120_000, 7)); // dupe
fs.writeFileSync(path.join(root, 'c.bin'), Buffer.alloc(120_000, 9)); // different

const started = e.scanStart({
  target: { kind: 'folder', paths: [root] },
  strategy: 'standard',
  options: { followReparse: false, sizeMode: 'allocated', treatPackagesAsNodes: false, excludePatterns: [] },
});
const scanId = started.scanId;
console.log('scan', scanId);

for (let i = 0; i < 120; i++) {
  if (events.some((ev) => ev.ev === 'scan-done')) break;
  await sleep(250);
}
const done = events.find((ev) => ev.ev === 'scan-done');
if (!done) die('scan never finished');
console.log('scanned:', done.done.summary.files, 'files in', done.done.summary.durationMs, 'ms');

// history
const hist = e.engineRecentScans(5);
if (!Array.isArray(hist) || hist.length < 1) die('recent scans empty');
console.log('history ok:', hist.length, 'rows, target', hist[0].target);

// presets: node_modules hit
const presets = e.cleanupPresetsScan(scanId);
const nm = presets.hits.find((h) => h.presetId === 'node-modules');
if (!nm || Number(nm.bytes) < 50_000) die('node_modules preset missing');
console.log('presets ok: node-modules', Number(nm.bytes), 'bytes');

// find the node_modules dir node
const kids = e.treeChildren({ scanId, nodeId: 0, sort: { key: 'name', dir: 'asc' }, offset: 0, limit: 50 });
let nmNode = null;
for (const k of kids.items) {
  if (k.name === 'proj') {
    const sub = e.treeChildren({ scanId, nodeId: k.id, sort: { key: 'name', dir: 'asc' }, offset: 0, limit: 50 });
    for (const s of sub.items) if (s.name === 'node_modules') nmNode = s.id;
  }
}
if (nmNode === null) die('node_modules node not found');

// stage + queue
const stageRes = e.cleanupStage({ scanId, nodeIds: [nmNode], source: 'preset' });
if (Number(stageRes.totals.count) !== 1) die('staging: ' + JSON.stringify(stageRes));
const queue = e.cleanupQueue();
if (queue.items.length !== 1) die('queue len');
console.log('staging ok:', queue.items[0].path);

// engine-boundary entitlement gates (PRISM-LIC-040)
for (const [name, fn] of [
  ['cleanup:execute', () => e.cleanupExecute({ scanId, toRecycleBin: false, acknowledgedBlocks: 0 })],
  ['snapshots:save', () => e.snapshotsSave({ scanId, depth: 4 })],
  ['duplicates:run', () => e.duplicatesRun({ scanId, minSize: 1024, ioCapBps: 0 })],
  ['monitor:start', () => e.monitorStart({ periodMs: 1000 })],
  ['apps:list', () => e.appsList({ includeSystem: false })],
]) {
  let blocked = false;
  try { fn(); } catch (err) { blocked = String(err.message).includes('not-licensed'); }
  if (!blocked) die(`${name} not entitlement-gated`);
}
console.log('entitlement gates ok (cleanup/snapshots/dupes/monitor/apps all refused)');

// resolve path
const resolved = e.nodeResolvePath({ scanId, path: path.join(root, 'proj', 'a.bin') });
if (resolved.nodeId === null || resolved.nodeId === undefined) die('resolve-path');
console.log('resolve ok: a.bin →', resolved.nodeId);

// color mapping
const cm = e.vizColorMapping({ scanId, mode: 'type' });
if (!Array.isArray(cm.legend) || cm.legend.length === 0) die('color-mapping');
console.log('legend ok:', cm.legend.length, 'categories');

// all canvas modes produce PVF1
for (const mode of ['treemap', 'sunburst', 'icicle', 'pack', 'mindmap', 'age-timeline']) {
  const buf = e.vizLayout({ scanId, root: 0, mode, viewport: { w: 800, h: 600, dpr: 1 }, options: { labels: true, drawnDepth: 6, gapPx: 1, cushionElevation: 0.55, cushionFalloff: 0.66, labelDensity: 0.5, colorMode: 'type', sizeMode: 'allocated', maxTiles: 250000, maxArcs: 20000, maxCircles: 20000, maxGraphNodes: 5000, minTilePx: 4, minShare: 0.0005 } });
  if (!(buf instanceof Buffer) || buf.length < 32 || buf.subarray(0, 4).toString() !== 'PVF1') die(`frame ${mode}`);
}
console.log('viz frames ok: 6 modes');

// unstage clears
e.cleanupUnstage({ nodeIds: [nmNode] });
if (e.cleanupQueue().items.length !== 0) die('unstage');
console.log('unstage ok');

// turbo honest refusal on this platform
let turboRefused = false;
try {
  e.scanStart({ target: { kind: 'volume', path: '/' }, strategy: 'turbo', options: { followReparse: false, sizeMode: 'allocated', treatPackagesAsNodes: false, excludePatterns: [] } });
} catch { turboRefused = true; }
if (!turboRefused) console.log('NOTE: turbo accepted (entitled?) — unexpected on dev host');
else console.log('turbo honest refusal ok');

// settings round-trip
e.engineSetSetting('smoke', '"hello"');
if (e.engineGetSetting('smoke') !== 'hello') die('setting round-trip');
console.log('settings ok');

fs.rmSync(root, { recursive: true, force: true });
fs.rmSync(dbPath, { force: true });
console.log('\nSMOKE OK — session-2 surface verified through the real .node');
// The napi TSFN event pump keeps libuv alive; exit explicitly so CI pipes
// never hang after a green run.
process.exit(0);
})().catch((err) => { die(err && err.stack ? err.stack : String(err)); process.exit(1); });
