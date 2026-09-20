//! Identity types and shared aliases (docs/07-DATA-MODEL.md).

use serde::{Deserialize, Serialize};

/// Node handle. Index into the arena; only valid within one `ScanId`
/// (docs/00-INDEX.md § 4.2 conventions).
pub type NodeId = u32;

/// Scan lease id, monotonic per engine process.
pub type ScanId = u32;

/// Extension-table index (`u32::MAX` = not a file / no extension).
pub type ExtId = u32;

/// Category-table index. Fixed shipped table + user overrides
/// (docs/07-DATA-MODEL.md § 2.1).
pub type CategoryId = u16;

/// Special pseudo-category: free space tile (never data-colored).
pub const CATEGORY_FREE_SPACE: CategoryId = 0xFFFE;
/// Special pseudo-category: unknown accounting (warning-hatched).
pub const CATEGORY_UNKNOWN: CategoryId = 0xFFFD;
/// Special pseudo-category: synthetic scan root / "This PC" node.
pub const CATEGORY_ROOT: CategoryId = 0xFFFC;

/// Volume-unique file identifier (NTFS file reference number). `0` = unknown.
pub type FileId = u64;

/// Unix milliseconds.
pub type UnixMs = i64;

/// FILETIME as raw 100-ns ticks (engine-internal; converted at the DTO edge).
pub type FileTimeTicks = i64;

/// What kind of entry a node is (wire form; arena stores `u8`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EntryKind {
    /// Regular file.
    File,
    /// Directory.
    Dir,
    /// Junction / symlink (reparse point not followed by default).
    Reparse,
    /// Mounted volume (never auto-descended).
    Mount,
    /// Secondary hard-link instance (bytes owned by the first-seen node).
    Link,
    /// Synthetic free-space pseudo-node.
    FreeSpace,
    /// Synthetic unknown-accounting pseudo-node.
    Unknown,
    /// Synthetic root for multi-root scans.
    Root,
}

/// Compact node badge set (bitflags on the wire).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeBadge {
    /// Reparse point (junction or symlink).
    Junction,
    /// Sparse file.
    Sparse,
    /// Compressed (NTFS compression / LZX).
    Compressed,
    /// Has additional hard links.
    Hardlinked,
    /// Offloaded to cloud (FILE_ATTRIBUTE_OFFLINE / recall-on-data-access).
    Offline,
    /// Package container (msix/appx/vhdx/iso…) collapsed per option.
    Package,
    /// System Volume Information / WinSxS-class path.
    System,
    /// Access denied during scan; size unknown.
    Denied,
}

/// Sort specification for engine-side columnar sorts (docs/05 § 3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SortKey {
    /// Node display name (case-folded).
    Name,
    /// Logical size (EndOfFile).
    Logical,
    /// Allocated size (cluster-rounded).
    Allocated,
    /// Subtree file count.
    Files,
    /// Subtree folder count.
    Folders,
    /// Percentage of parent.
    Percent,
    /// Last modified (subtree max).
    Mtime,
    /// Extension/category.
    Category,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SortDir {
    /// Ascending.
    Asc,
    /// Descending (default for sizes).
    Desc,
}

/// A single sort term; multiple terms apply in order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SortSpec {
    /// Primary key.
    pub key: SortKey,
    /// Direction.
    pub dir: SortDir,
}
