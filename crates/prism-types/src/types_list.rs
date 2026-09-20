//! Extension/type statistics (parity-EXT-01) + cleanup/duplicates DTOs.

use serde::{Deserialize, Serialize};

use crate::ids::{CategoryId, ExtId};

/// One row of the Types panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeRow {
    /// Extension key (lowercase, no dot; "" = no extension; "." = dotfile).
    pub key: String,
    /// Interned extension id.
    pub ext_id: ExtId,
    /// Category id.
    pub category: CategoryId,
    /// Files with this extension.
    pub files: u64,
    /// Logical total.
    pub logical: u64,
    /// Allocated total.
    pub allocated: u64,
    /// Share of scan (0–1).
    pub share: f32,
    /// User-overridden color (`#RRGGBB`) or null.
    pub user_color: Option<String>,
}

/// Types list response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypesPage {
    /// Rows sorted by the requested spec.
    pub items: Vec<TypeRow>,
    /// Sum of all shares (sanity for % bars).
    pub total_share: f32,
}

/// One staged cleanup item (ledger row).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupItem {
    /// Arena node id (0 = non-scan path item).
    pub node_id: u32,
    /// Absolute path.
    pub path: String,
    /// Reclaimable bytes.
    pub bytes: u64,
    /// Source of staging.
    pub source: CleanupSource,
}

/// Where a staged item came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CleanupSource {
    /// Manual selection.
    Manual,
    /// Preset rule hit.
    Preset,
    /// Duplicate extras.
    Duplicate,
    /// Stale rule.
    Stale,
    /// Uninstaller leftovers.
    Leftover,
}

/// One duplicate group member.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateMember {
    /// Arena node id.
    pub node_id: u32,
    /// Full path.
    pub path: String,
    /// File size.
    pub bytes: u64,
    /// Mtime unix ms.
    pub mtime: Option<i64>,
    /// First-seen member (protected, "Kept" badge).
    pub kept: bool,
    /// Partial fingerprint matched (XXH128 of head+tail).
    pub partial_hashed: bool,
    /// Full BLAKE3 hashed (final arbiter).
    pub full_hashed: bool,
    /// Hard-link sibling of the kept instance.
    pub hardlink_of_kept: bool,
}

/// One duplicate group.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    /// Group id.
    pub group_id: u32,
    /// Members (first = kept).
    pub members: Vec<DuplicateMember>,
    /// (n−1) × size.
    pub reclaimable: u64,
}

/// Duplicates run progress payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DupesPhase {
    /// Grouping by size.
    GroupingSizes,
    /// Grouping by size + extension.
    GroupingExt,
    /// Partial fingerprinting (head+tail reads).
    Fingerprinting,
    /// Full BLAKE3 hashing.
    FullHashing,
    /// Collapsing hard-link groups.
    HardlinkCollapse,
    /// Done.
    Done,
}
