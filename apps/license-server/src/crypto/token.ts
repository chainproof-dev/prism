// Entitlement token minting + verification (docs/13 § 4).
// Format (shared with the Rust engine): base64_std( ed25519_sig(64) || cbor_claims ).
// The signature covers exactly the CBOR claims bytes — no canonicalization
// ambiguity. Cross-implementation property tests live in token.test.ts.

import * as ed25519 from '@noble/ed25519';
import { sha512 } from '@noble/hashes/sha2.js';
import { bytesToHex, hexToBytes, randomBytes } from '@noble/hashes/utils.js';

// v3 requires an explicit SHA-512 provider (sync paths).
ed25519.hashes.sha512 = sha512;
import { encode as cborEncode, decode as cborDecode } from 'cbor-x';

export type Sku = 'yearly' | 'lifetime';
export type PremiumFeature =
  | 'turbo'
  | 'dupes'
  | 'cleanup'
  | 'apps'
  | 'snapshots'
  | 'monitor'
  | 'scheduler'
  | 'export';

export const ALL_FEATURES: PremiumFeature[] = [
  'turbo',
  'dupes',
  'cleanup',
  'apps',
  'snapshots',
  'monitor',
  'scheduler',
  'export',
];

export interface Claims {
  v: 1;
  key_id: string;
  sku: Sku;
  features: PremiumFeature[];
  instance_id: string;
  device: string;
  iat: number;
  exp: number;
  nonce: string;
}

/** Grace ceiling: exp − iat ≤ 30 days (PRISM-LIC-010). */
export const MAX_GRACE_SECONDS = 30 * 86_400;

/** Load the signing keypair from a PEM string (dev keys are committed). */
export async function loadKeyPair(pem: string): Promise<{ publicKeyHex: string; sign: (claims: Claims) => Promise<string> }> {
  // Parse the raw private key from the PKCS#8 PEM (Ed25519: last 32 bytes of 48-byte DER).
  const der = pem
    .split('\n')
    .filter((l) => !l.startsWith('-----'))
    .join('');
  const derBytes = Uint8Array.from(atobLike(der), (c) => c.charCodeAt(0));
  const privateKey = derBytes.slice(derBytes.length - 32);
  const publicKey = await ed25519.getPublicKey(privateKey);
  const publicKeyHex = bytesToHex(publicKey);

  const sign = async (claims: Claims): Promise<string> => {
    const claimsBytes = cborEncode(claims);
    const sig = await ed25519.sign(claimsBytes, privateKey);
    const token = new Uint8Array(64 + claimsBytes.length);
    token.set(sig, 0);
    token.set(claimsBytes, 64);
    return btoaLike(token);
  };

  return { publicKeyHex, sign };
}

/** Verify a token (used by tests to cross-check the engine's verifier). */
export async function verifyToken(
  tokenB64: string,
  publicKeyHex: string,
): Promise<Claims> {
  const raw = Uint8Array.from(atobLike(tokenB64), (c) => c.charCodeAt(0));
  if (raw.length < 65) {
    throw new Error('token too short');
  }
  const sig = raw.slice(0, 64);
  const claimsBytes = raw.slice(64);
  const ok = await ed25519.verify(sig, claimsBytes, hexToBytes(publicKeyHex));
  if (!ok) {
    throw new Error('bad signature');
  }
  return cborDecode(claimsBytes) as Claims;
}

export function newNonce(): string {
  return bytesToHex(randomBytes(16));
}

export function mintClaims(opts: {
  sku: Sku;
  instanceId: string;
  deviceName: string;
  keyId: string;
  skuEndSeconds: number | null;
}): Claims {
  const now = Math.floor(Date.now() / 1000);
  const exp = Math.min(now + MAX_GRACE_SECONDS, opts.skuEndSeconds ?? now + MAX_GRACE_SECONDS);
  return {
    v: 1,
    key_id: opts.keyId,
    sku: opts.sku,
    features: [...ALL_FEATURES],
    instance_id: opts.instanceId,
    device: opts.deviceName,
    iat: now,
    exp,
    nonce: newNonce(),
  };
}

// base64 helpers without Node Buffer dependency
function btoaLike(bytes: Uint8Array): string {
  let bin = '';
  for (const b of bytes) {
    bin += String.fromCharCode(b);
  }
  return btoa(bin);
}

function atobLike(b64: string): string {
  return atob(b64.replace(/\s/g, ''));
}
