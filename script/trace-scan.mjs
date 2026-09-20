#!/usr/bin/env node
// Trace scanner (PRISM-QA-010 / docs/00 § 4.3): forbidden vocabulary must
// never appear in shipped artifacts. CI gate.

import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, extname } from 'node:path';

const FORBIDDEN = [
  'windirstat', 'wds', 'kdirstat', 'qdirstat', 'filelight', 'diskbuddy',
  'disk buddy', 'wiztree', 'treesize', 'spacesniffer', 'dirstat',
];

// Docs/research paths are exempt (engineering traceability only)
const EXEMPT = [/^docs\//, /\.md$/, /node_modules/, /^target\//, /^dev-keys\//, /\.git\//, /^out\//, /^qa\//, /^script\//, /worklog/];
const TEXT_EXTS = new Set(['.ts', '.tsx', '.rs', '.js', '.mjs', '.json', '.css', '.html', '.toml', '.yaml', '.yml', '.sql', '.sh', '.py']);

let violations = 0;
const walk = (dir) => {
  for (const entry of readdirSync(dir)) {
    if (entry === '.git' || entry === 'node_modules' || entry === 'target' || entry === 'out' || entry === 'dist') {
      continue;
    }
    const full = join(dir, entry);
    const rel = full.replace(/^\.\/?/, '');
    const st = statSync(full);
    if (st.isDirectory()) {
      walk(full);
      continue;
    }
    if (EXEMPT.some((re) => re.test(rel))) {
      continue;
    }
    if (!TEXT_EXTS.has(extname(entry))) {
      continue;
    }
    const content = readFileSync(full, 'utf8').toLowerCase();
    for (const word of FORBIDDEN) {
      // word-boundary match to avoid false positives (e.g. 'wds' inside identifiers is still forbidden per docs/00 § 4.3)
      const re = new RegExp(`\\b${word.replace(/ /g, '\\s+')}\\b`, 'i');
      if (re.test(content)) {
        console.error(`TRACE VIOLATION: '${word}' in ${rel}`);
        violations += 1;
      }
    }
  }
};

walk('.');
if (violations > 0) {
  console.error(`\ntrace scanner: ${violations} violation(s) — forbidden reference-tool vocabulary in shipped artifacts`);
  process.exit(1);
}
console.log('trace scanner: clean');
