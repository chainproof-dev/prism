// One-command dev startup (R11 / PRISM-LIC-050): license server on :8080 +
// Electron app, with LICENSE_SERVER_URL wired into the client config.
import { spawn } from 'node:child_process';

const children = [];
function run(name, cmd, args, cwd) {
  const child = spawn(cmd, args, { cwd, stdio: 'inherit', shell: process.platform === 'win32' });
  children.push(child);
  console.log(`[dev] ${name} started (pid ${child.pid})`);
  return child;
}

function shutdown() {
  for (const c of children) {
    try { c.kill('SIGTERM'); } catch { /* already gone */ }
  }
  process.exit(0);
}
process.on('SIGINT', shutdown);
process.on('SIGTERM', shutdown);

const root = new URL('..', import.meta.url).pathname;

// 1) license server (localhost:8080, dev keys, seeded)
run('license-server', 'pnpm', ['--filter', 'license-server', 'run', 'seed'], root);
run('license-server', 'pnpm', ['--filter', 'license-server', 'run', 'dev'], root);

// 2) desktop app (env: same code path as prod, only URL differs)
const electronEnv = { ...process.env, LICENSE_SERVER_URL: 'http://localhost:8080' };
const desk = spawn('pnpm', ['--filter', 'desktop', 'run', 'dev'], {
  cwd: root, stdio: 'inherit', shell: process.platform === 'win32', env: electronEnv,
});
children.push(desk);
console.log('[dev] desktop started (LICENSE_SERVER_URL=http://localhost:8080)');

desk.on('exit', shutdown);
