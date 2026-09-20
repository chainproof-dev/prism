// Admin CLI (docs/13 § 10): `pnpm lic:admin issue --sku lifetime --note "qa"`
// Talks to the running server's admin API.

const BASE = process.env.LICENSE_SERVER_URL ?? 'http://localhost:8080';
const TOKEN = process.env.ADMIN_TOKEN ?? 'prism-dev-admin-token';

async function call(method: string, path: string, body?: unknown): Promise<unknown> {
  const res = await fetch(`${BASE}${path}`, {
    method,
    headers: {
      authorization: `Bearer ${TOKEN}`,
      ...(body ? { 'content-type': 'application/json' } : {}),
    },
    body: body ? JSON.stringify(body) : null,
  });
  const json = (await res.json().catch(() => ({}))) as Record<string, unknown>;
  if (!res.ok) {
    console.error(`error ${res.status}:`, JSON.stringify(json));
    process.exit(1);
  }
  return json;
}

function arg(name: string): string | undefined {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 ? process.argv[i + 1] : undefined;
}

async function main(): Promise<void> {
  const [cmd] = process.argv.slice(2);
  switch (cmd) {
    case 'issue': {
      const sku = arg('sku') ?? 'lifetime';
      const note = arg('note');
      const out = (await call('POST', '/v1/admin/keys', { sku, note })) as { key: string; sku: string };
      console.log(out.key);
      break;
    }
    case 'list': {
      const out = (await call('GET', '/v1/admin/keys')) as {
        keys: { key: string; sku: string; status: string; note: string | null }[];
      };
      for (const k of out.keys) {
        console.log(`${k.key}  ${k.sku.padEnd(9)} ${k.status.padEnd(7)} ${k.note ?? ''}`);
      }
      break;
    }
    case 'revoke': {
      const key = arg('key');
      if (!key) throw new Error('--key required');
      await call('POST', `/v1/admin/keys/${key}/revoke`);
      console.log(`revoked ${key}`);
      break;
    }
    case 'release-device': {
      const key = arg('key');
      const instanceId = arg('instance');
      if (!key || !instanceId) throw new Error('--key and --instance required');
      await call('POST', `/v1/admin/keys/${key}/release-device`, { instance_id: instanceId });
      console.log(`released ${instanceId} from ${key}`);
      break;
    }
    case 'extend': {
      const key = arg('key');
      const seconds = Number(arg('seconds') ?? 86_400);
      if (!key) throw new Error('--key required');
      await call('POST', `/v1/admin/keys/${key}/extend`, { seconds });
      console.log(`extended ${key} by ${seconds}s`);
      break;
    }
    case 'stats': {
      console.log(await call('GET', '/v1/admin/stats'));
      break;
    }
    default:
      console.log(`usage: cli <issue|list|revoke|release-device|extend|stats> [options]
  issue --sku yearly|lifetime [--note "..."]   prints a fresh key
  list                                        list keys
  revoke --key PRSM-...                       revoke a key
  release-device --key PRSM-... --instance UUID
  extend --key PRSM-... [--seconds N]
  stats`);
      process.exit(cmd ? 1 : 0);
  }
}

main().catch((e: unknown) => {
  console.error(e);
  process.exit(1);
});
