//! Licensing DTOs (docs/13). Tokens are CBOR+Ed25519; these DTOs describe the
//! *verified* view plus activation-facing enums shared with the TS client.

use serde::{Deserialize, Serialize};

use crate::ids::UnixMs;

/// Exactly two SKUs (R10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Sku {
    /// 12 months from activation.
    Yearly,
    /// Current major line, perpetual.
    Lifetime,
}

/// Premium feature identifiers (token `features` array, engine gates).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PremiumFeature {
    /// Turbo scan (raw NTFS).
    Turbo,
    /// Duplicates pipeline.
    Dupes,
    /// Cleanup presets + ledger execute.
    Cleanup,
    /// Uninstaller + leftovers.
    Apps,
    /// Snapshots + diff.
    Snapshots,
    /// Process/system monitor.
    Monitor,
    /// Scheduled scans.
    Scheduler,
    /// CSV/NDJSON exports.
    Export,
}

/// Engine-visible device identity (used for `device_id` binding checks).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceIdentity {
    /// Per-install instance id (uuid v4).
    pub instance_id: String,
    /// Human device name (hostname prefix, user-editable).
    pub device_name: String,
    /// OS build string.
    pub os_build: String,
    /// App version.
    pub app_version: String,
}

/// Verified entitlement grants (engine typestate payload, docs/06 § 8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntitlementGrants {
    /// SKU tier.
    pub sku: Sku,
    /// Granted premium features.
    pub features: Vec<PremiumFeature>,
    /// Token expiry (unix ms).
    pub exp: UnixMs,
    /// Token issued-at (unix ms).
    pub iat: UnixMs,
}

/// Entitlement failure taxonomy (engine boundary, mirrors TS `IpcError.entitlement`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EntitlementError {
    /// Signature invalid (forged or corrupted token).
    #[error("signature verification failed")]
    BadSignature,
    /// Token expired (offline grace exhausted).
    #[error("entitlement expired")]
    Expired,
    /// Token issued too long ago (replay bound, 7 days).
    #[error("token stale beyond replay window")]
    StaleIat,
    /// Bound to a different install.
    #[error("device mismatch")]
    DeviceMismatch,
    /// Feature not in the SKU matrix.
    #[error("feature not granted for this SKU")]
    FeatureNotGranted,
    /// Malformed token (CBOR/schema).
    #[error("malformed token")]
    Malformed,
    /// Key revoked server-side.
    #[error("license revoked")]
    Revoked,
}
