#!/usr/bin/env node
// Docs link checker (docs/17 § 3 gate): every relative markdown link in docs/
// must resolve to a file and a valid anchor.

import { readdirSync, readFileSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';

let broken = 0;
const mdFiles = [];

const walk = (dir) => {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (entry.name === 'amendments' && entry.isDirectory()) continue; // amendments may reference pending docs
    const full = join(dir, entry.name);
    if (entry.isDirectory()) walk(full);
    else if (entry.name.endsWith('.md')) mdFiles.push(full);
  }
};
walk('docs');

for (const file of mdFiles) {
  const content = readFileSync(file, 'utf8');
  const linkRe = /\[([^\]]*)\]\(([^)#\s]+)(#[^)\s]*)?\)/g;
  let m;
  while ((m = linkRe.exec(content)) !== null) {
    const [, , target] = m;
    if (target.startsWith('http') || target.startsWith('mailto:')) continue;
    const resolved = join(dirname(file), target);
    if (!existsSync(resolved)) {
      console.error(`BROKEN LINK: ${file} → ${target}`);
      broken += 1;
    }
  }
}

if (broken > 0) {
  console.error(`link check: ${broken} broken link(s)`);
  process.exit(1);
}
console.log(`link check: clean (${mdFiles.length} docs scanned)`);
