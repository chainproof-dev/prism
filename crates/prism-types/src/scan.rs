//! Scanning DTOs: targets, options, summaries, node rows/details (docs/05 § 3.2–3.3).

use serde::{Deserialize, Serialize};

use crate::ids::{CategoryId, EntryKind, FileId, NodeBadge, NodeId, ScanId, SortSpec, UnixMs};

/// What to scan (multi-root allowed, docs/05 § 3.2 / parity-SEL-04).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ScanTarget {
    /// A whole volume, e.g. `C:\`.
    Volume {
        /// Win32 root path.
        path: String,
    },
    /// One or more folders in a single logical scan.
    Folder {
        /// Win32 folder paths.
        paths: Vec<String>,
    },
    /// The user profile root (`%USERPROFILE%`).
    Home,
}

/// Size accounting mode (parity-SCN-07 / parity-DSP-04).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SizeMode {
    /// EndOfFile sums (per-link counted).
    Logical,
    /// Cluster-allocated sums (sparse-aware, per-link counted).
    Allocated,
    /// First-seen hard-link owns the bytes.
    Unique,
}

/// Scanner strategy — two first-class paths (ADR-06), never a fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScanStrategy {
    /// Parallel directory enumeration, no elevation, any readable root.
    Standard,
    /// Raw NTFS MFT walk (elevated consent, NTFS-only).
    Turbo,
}

/// Options honored by `scan:start` (docs/05 § 3.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanOptions {
    /// Follow junctions/symlinks (default false, parity-SCN-06).
    pub follow_reparse: bool,
    /// Active size accounting mode.
    pub size_mode: SizeMode,
    /// Collapse package containers to single nodes.
    pub treat_packages_as_nodes: bool,
    /// Glob exclusions (own matcher, docs/06 § 2.4).
    pub exclude_patterns: Vec<String>,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            follow_reparse: false,
            size_mode: SizeMode::Allocated,
            treat_packages_as_nodes: false,
            exclude_patterns: Vec::new(),
        }
    }
}

/// One page of children from `tree:children`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeRow {
    /// Arena node id.
    pub id: NodeId,
    /// Display name (no path).
    pub name: String,
    /// Entry kind.
    pub kind: EntryKind,
    /// Logical (EndOfFile) size; for dirs = subtree sum.
    pub logical: u64,
    /// Allocated (cluster) size; for dirs = subtree sum.
    pub allocated: u64,
    /// Subtree file count (dirs).
    pub files: u32,
    /// Subtree folder count (dirs).
    pub folders: u32,
    /// Category for coloring.
    pub category: CategoryId,
    /// Badges (junction/sparse/…).
    pub badges: Vec<NodeBadge>,
    /// Last modified unix ms (None = unknown).
    pub mtime: Option<UnixMs>,
    /// Percent of parent size (0.0–1.0).
    pub parent_share: f32,
}

/// Paged children response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeRowsPage {
    /// Total children (for the pager).
    pub total: u32,
    /// This page's rows.
    pub items: Vec<NodeRow>,
}

/// Full detail for `node:detail` (inspector payload).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeDetail {
    /// Arena node id.
    pub id: NodeId,
    /// Full display path.
    pub path: String,
    /// Display name.
    pub name: String,
    /// Entry kind.
    pub kind: EntryKind,
    /// Category id.
    pub category: CategoryId,
    /// Category display name.
    pub category_name: String,
    /// Extension (files; "" otherwise).
    pub extension: String,
    /// Logical bytes.
    pub logical: u64,
    /// Allocated bytes.
    pub allocated: u64,
    /// Unique bytes (first-seen hard-link owner accounting).
    pub unique: u64,
    /// Badges.
    pub badges: Vec<NodeBadge>,
    /// Created (unix ms).
    pub created: Option<UnixMs>,
    /// Modified (unix ms).
    pub modified: Option<UnixMs>,
    /// Accessed (unix ms).
    pub accessed: Option<UnixMs>,
    /// Attributes (FILE_ATTRIBUTE_* raw bits).
    pub attributes: u32,
    /// Hard-link group size (1 = no links).
    pub link_count: u32,
    /// All link paths when link_count > 1.
    pub link_paths: Vec<String>,
    /// Volume-unique file id.
    pub file_id: FileId,
    /// Parent id (root = self).
    pub parent: NodeId,
    /// Depth from scan root.
    pub depth: u32,
    /// Subtree stats (dirs).
    pub files: u32,
    /// Subtree folder count (dirs).
    pub folders: u32,
}

/// Scan phase transitions (engine state machine).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScanPhase {
    /// Walking directories (enumeration active).
    Walking,
    /// Bottom-up aggregation running.
    Aggregating,
    /// Extension/category indexing.
    IndexingExt,
    /// Completed successfully.
    Done,
    /// Failed (see error batches).
    Failed,
    /// Cancelled by user.
    Cancelled,
    /// Paused (standard strategy only).
    Paused,
}

/// Per-path scan error (errors drawer row, parity-SCN-05).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathError {
    /// Path that failed.
    pub path: String,
    /// OS error code (raw).
    pub code: i32,
    /// Translated message (renderer localizes; engine provides fallback).
    pub message: String,
    /// True when the failure was access-denied class.
    pub denied: bool,
}

/// Final scan statistics (`scan:done` / `scan:summary`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    /// Scan lease id.
    pub scan_id: ScanId,
    /// Root display path.
    pub root: String,
    /// Strategy used.
    pub strategy: ScanStrategy,
    /// Active size mode.
    pub size_mode: SizeMode,
    /// Total files seen.
    pub files: u64,
    /// Total folders seen.
    pub folders: u64,
    /// Logical total.
    pub logical: u64,
    /// Allocated total.
    pub allocated: u64,
    /// Unique total (hard links counted once).
    pub unique: u64,
    /// Unknown bytes (denied/unreadable).
    pub unknown: u64,
    /// Free bytes on volume at completion.
    pub free: u64,
    /// Wall duration ms.
    pub duration_ms: u64,
    /// Error count (see error batches).
    pub errors: u32,
    /// Scan truncated by the arena ceiling.
    pub truncated: bool,
}

/// Cheap expand counters for a tree row (`tree:expand-stats`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpandStats {
    /// Subtree files.
    pub files: u64,
    /// Subtree folders.
    pub folders: u64,
    /// Logical bytes.
    pub logical: u64,
    /// Allocated bytes.
    pub allocated: u64,
    /// Unique bytes.
    pub unique: u64,
}

/// Sort request bundled with tree queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildrenQuery {
    /// Scan lease.
    pub scan_id: ScanId,
    /// Node whose children are requested.
    pub node_id: NodeId,
    /// Sort spec (engine-side columnar sort).
    pub sort: SortSpec,
    /// Page offset.
    pub offset: u32,
    /// Page size (≤ 1000, PRISM-IPC-052).
    pub limit: u32,
}
