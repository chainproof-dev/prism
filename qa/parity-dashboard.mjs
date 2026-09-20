#!/usr/bin/env node
// qa:parity — render the docs/03 porting matrix as a live status dashboard
// (docs/03 § 12.3). Rows come from docs/03-PORTING-MATRIX-WINDIRSTAT.md;
// sign-offs from qa/parity-status.json (QA sessions record results there).
// Output: docs/phases/parity-dashboard.md (checked in; CI drift-checks it).

import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const matrixPath = join(root, 'docs', '03-PORTING-MATRIX-WINDIRSTAT.md');
const statusPath = join(root, 'qa', 'parity-status.json');
const outPath = join(root, 'docs', 'phases', 'parity-dashboard.md');

const matrix = readFileSync(matrixPath, 'utf8');
const signoffs = existsSync(statusPath) ? JSON.parse(readFileSync(statusPath, 'utf8')) : {};

// Parse table rows: | ID | ... | Status | Verify |
const rows = [];
let section = 'unknown';
for (const line of matrix.split('\n')) {
  const secMatch = line.match(/^## \d+\. (.+)$/);
  if (secMatch) {
    section = secMatch[1].trim();
    continue;
  }
  if (!line.startsWith('|')) continue;
  const cells = line.split('|').map((c) => c.trim());
  // ['' , ID, ref, spec, status, verify, ''] — drop the empty bookends.
  if (cells.length < 6 || cells[1].startsWith('---') || cells[1] === 'ID') continue;
  const id = cells[1];
  if (!/^WDS-/.test(id)) continue;
  const status = cells[cells.length - 3];
  const verify = cells[cells.length - 2];
  if (!status || !verify) continue;
  rows.push({ id, section, status, verify, spec: cells[3] ?? '' });
}

const statusOf = (r) => {
  if (r.status === 'X') return 'diverged';
  const so = signoffs[r.id];
  if (so === 'pass') return 'signed-off';
  if (so === 'blocked') return 'blocked';
  if (so && typeof so === 'object' && so.status === 'pass') return 'signed-off';
  return 'pending';
};

const counts = { 'signed-off': 0, pending: 0, diverged: 0, blocked: 0 };
for (const r of rows) counts[statusOf(r)] += 1;

const ICONS = { 'signed-off': '✅', pending: '⏳', diverged: '➖', blocked: '🚫' };

let md = `# Parity dashboard (generated — do not edit by hand)

> Source of truth: docs/03-PORTING-MATRIX-WINDIRSTAT.md · sign-offs: qa/parity-status.json
> Regenerate with \`npm run qa:parity\`. GA gate: 0 pending rows (docs/18 Phase 10).

**Summary:** ${counts['signed-off']} signed-off · ${counts.pending} pending QA · ${counts.blocked} blocked · ${counts.diverged} deliberate divergence — ${rows.length} rows total.
`;

const sections = [...new Set(rows.map((r) => r.section))];
for (const sec of sections) {
  const secRows = rows.filter((r) => r.section === sec);
  md += `\n## ${sec}\n\n| ID | Status | QA | Verify |\n|---|---|---|---|\n`;
  for (const r of secRows) {
    const st = statusOf(r);
    const note = signoffs[r.id] && typeof signoffs[r.id] === 'object' ? (signoffs[r.id].note ?? '') : '';
    md += `| ${r.id} | ${ICONS[st]} ${st} | ${note} | ${r.verify} |\n`;
  }
}

md += `\n> Sign-off protocol (docs/03 § 12.2): a row moves to signed-off only\n> when its automated tests pass on CI AND a QA session records the manual\n> script result in qa/parity-status.json. Rows requiring a physical Windows\n> host stay pending until that session happens — they are never faked.\n`;

writeFileSync(outPath, md);
console.log(
  `parity dashboard: ${rows.length} rows → docs/phases/parity-dashboard.md ` +
    `(✅ ${counts['signed-off']} · ⏳ ${counts.pending} · 🚫 ${counts.blocked} · ➖ ${counts.diverged})`,
);
