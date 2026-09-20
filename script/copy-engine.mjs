// Copy the built engine cdylib to the desktop resources dir with the
// platform-appropriate .node name (no fallbacks: missing file = loud failure
// at app boot with the diagnostic screen — docs/04 § 6).
import { copyFileSync, existsSync, mkdirSync, statSync } from 'node:fs';
import { resolve } from 'node:path';

const root = resolve(import.meta.dirname, '..');
const targetDir = resolve(root, 'apps/desktop/resources');
mkdirSync(targetDir, { recursive: true });

const candidates = [
  {
    src: resolve(root, 'target/release/prism_core.dll'),
    out: 'prism-core.node',
  },
  {
    src: resolve(root, 'target/release/libprism_core.so'),
    out: 'prism-core.node',
  },
  {
    src: resolve(root, 'target/release/libprism_core.dylib'),
    out: 'prism-core.node',
  },
];

for (const c of candidates) {
  if (existsSync(c.src)) {
    copyFileSync(c.src, resolve(targetDir, c.out));
    console.log(`engine → apps/desktop/resources/${c.out} (${(statSync(c.src).size / 1048576).toFixed(1)} MB)`);
    process.exit(0);
  }
}
console.error('engine cdylib not found — run `cargo build --release -p prism-core` first');
process.exit(1);
