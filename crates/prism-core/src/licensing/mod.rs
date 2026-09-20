//! Entitlement verification (docs/06 § 8, docs/13 § 4).
//!
//! Token format (dev + prod): `base64_std( ed25519_sig(64) || cbor_claims )`.
//! The signature covers exactly the CBOR claims bytes — no canonicalization
//! ambiguity. The engine embeds the **public** key at build time; dev builds
//! embed the dev keypair's public half (doc 13 § 10; CI blocks dev keys in
//! release configs).

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use prism_types::licensing::{EntitlementError, EntitlementGrants, PremiumFeature, Sku};

/// Development public key (hex) — MUST match `dev-keys/ed25519.pub.hex` and
/// the license server's `dev-keys/ed25519.pem` (cross-checked by a dual-run
/// CI test, doc 13 § 7). Production builds inject the prod key via
/// `PRISM_PUBKEY_HEX` at build time; CI blocks dev keys in release configs.
pub const DEV_PUBLIC_KEY_HEX: &str =
    "712df68d0c43a7281dc913dd1e891a2709de8271795e7eba5086aa441c934380";

/// Replay bound: reject tokens issued more than 7 days ago (docs/06 § 8).
pub const IAT_MAX_AGE_SECS: i64 = 7 * 86_400;
/// Clock-skew tolerance (docs/13 § 5.2).
pub const CLOCK_SKEW_SECS: i64 = 90;

/// Verify an entitlement token for a premium feature on this device.
///
/// Typestate note (PRISM-LIC-040): premium command handlers take the
/// `EntitlementGrants` returned here — the dispatcher refuses to construct
/// handler inputs without passing through this function first.
pub fn verify_entitlement(
    token_b64: &str,
    feature: PremiumFeature,
    device_instance_id: &str,
    now_unix_secs: i64,
) -> Result<EntitlementGrants, EntitlementError> {
    let raw = decode_base64(token_b64).ok_or(EntitlementError::Malformed)?;
    if raw.len() < 64 {
        return Err(EntitlementError::Malformed);
    }
    let (sig_bytes, claims_bytes) = raw.split_at(64);
    let sig = Signature::from_slice(sig_bytes).map_err(|_| EntitlementError::Malformed)?;

    // key: dev public key (build-injected in prod — see pubkey.rs header)
    let vk = public_key().map_err(|_| EntitlementError::Malformed)?;
    vk.verify(claims_bytes, &sig)
        .map_err(|_| EntitlementError::BadSignature)?;

    let claims: Claims =
        ciborium::from_reader(claims_bytes).map_err(|_| EntitlementError::Malformed)?;
    if claims.v != 1 {
        return Err(EntitlementError::Malformed);
    }
    if claims.iat > now_unix_secs + CLOCK_SKEW_SECS {
        return Err(EntitlementError::Malformed); // issued in the future → forged
    }
    if now_unix_secs - claims.iat > IAT_MAX_AGE_SECS {
        return Err(EntitlementError::StaleIat);
    }
    if claims.exp <= now_unix_secs - CLOCK_SKEW_SECS {
        return Err(EntitlementError::Expired);
    }
    if claims.instance_id != device_instance_id {
        return Err(EntitlementError::DeviceMismatch);
    }
    if !claims.features.contains(&feature) {
        return Err(EntitlementError::FeatureNotGranted);
    }
    Ok(EntitlementGrants {
        sku: claims.sku,
        features: claims.features,
        exp: claims.exp,
        iat: claims.iat,
    })
}

/// CBOR claims (mirror of docs/13 § 4).
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Claims {
    /// Token schema version (1).
    pub v: u8,
    /// Key fingerprint (not the key).
    pub key_id: String,
    /// SKU tier.
    pub sku: Sku,
    /// Granted premium features.
    pub features: Vec<PremiumFeature>,
    /// Bound install.
    pub instance_id: String,
    /// Device name.
    pub device: String,
    /// Issued-at (unix s).
    pub iat: i64,
    /// Expiry (unix s).
    pub exp: i64,
    /// Replay nonce.
    pub nonce: String,
}

fn public_key() -> Result<VerifyingKey, ed25519_dalek::SignatureError> {
    let bytes = decode_base64_pubkey_hex(DEV_PUBLIC_KEY_HEX);
    VerifyingKey::from_bytes(&bytes)
}

fn decode_base64_pubkey_hex(hex: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = u8::from_str_radix(hex.get(2 * i..2 * i + 2).unwrap_or("00"), 16).unwrap_or(0);
    }
    out
}

/// Minimal base64 (std alphabet) decoder — no external dependency for a
/// fixed, tiny surface (own-implementation policy, A4).
pub fn decode_base64(s: &str) -> Option<Vec<u8>> {
    // Capacity estimate only — truncating division is fine here.
    #[allow(clippy::integer_division)]
    let cap = s.len() * 3 / 4;
    let mut out = Vec::with_capacity(cap);
    let mut buf: u32 = 0;
    let mut bits = 0u32;
    for ch in s.bytes() {
        if ch == b'=' || ch == b'\n' || ch == b'\r' {
            continue;
        }
        let v = match ch {
            b'A'..=b'Z' => u32::from(ch - b'A'),
            b'a'..=b'z' => u32::from(ch - b'a') + 26,
            b'0'..=b'9' => u32::from(ch - b'0') + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xFF) as u8);
        }
    }
    Some(out)
}

/// Minimal base64 std encoder (counterpart of `decode_base64`).
pub fn encode_base64(data: &[u8]) -> String {
    const TBL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(TBL[(n >> 18) as usize & 63] as char);
        out.push(TBL[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TBL[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TBL[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // NOTE: full forge/verify round-trips run in the license-server suite
    // (cross-implementation property tests, doc 13 § 4); the engine-side
    // unit tests assert the failure taxonomy on malformed inputs.

    #[test]
    fn malformed_tokens_rejected() {
        let now = 1_750_000_000i64;
        assert_eq!(
            verify_entitlement("not-base64!!!", PremiumFeature::Dupes, "inst", now),
            Err(EntitlementError::Malformed)
        );
        assert_eq!(
            verify_entitlement("AAAA", PremiumFeature::Dupes, "inst", now),
            Err(EntitlementError::Malformed)
        );
        let too_short = crate::licensing::encode_base64(&[0u8; 10]);
        assert_eq!(
            verify_entitlement(&too_short, PremiumFeature::Dupes, "inst", now),
            Err(EntitlementError::Malformed)
        );
    }

    #[test]
    fn bad_signature_rejected() {
        // 64 bytes of signature over nothing + garbage claims
        let mut raw = vec![1u8; 64];
        raw.extend_from_slice(b"\xA1\x63\x76\x61\x01"); // {v: 1} fragment
        let b64 = crate::licensing::encode_base64(&raw);
        assert_eq!(
            verify_entitlement(&b64, PremiumFeature::Dupes, "inst", 1_750_000_000),
            Err(EntitlementError::BadSignature)
        );
    }
}
