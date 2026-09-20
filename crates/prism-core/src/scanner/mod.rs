//! # Scanner (docs/06 § 2)
//!
//! Standard strategy: parallel directory enumeration. The enumeration
//! primitive is platform-selected **at build time** (never at runtime — no
//! silent fallbacks, ADR-06 spirit):
//! - `cfg(windows)` — `NtQueryDirectoryFile` with `FileIdExtdDirectoryInformation`
//!   (falls back across *builds*, not runtimes, to `FileDirectoryInformation`
//!   when the Extd class is unavailable — see `win32.rs` capability probe).
//! - `cfg(unix)` — the development backend (std::fs::read_dir + metadata),
//!   labeled `scanner-posix-dev` in `sys:hello`. Windows release builds do not
//!   compile it at all. See docs/amendments/AMM-002-dev-platform.md.
//!
//! Both backends emit `DirBatch`es into the coordinator (single arena writer).

pub mod coordinator;
pub mod exclusions;
pub mod nt_path;
pub mod packages;
pub mod posix;
pub mod turbo;
pub mod win32;

/// Volume-unique file identity for hard-link accounting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileIdentity {
    /// Volume serial (0 when the backend cannot provide one).
    pub volume_serial: u32,
    /// File id (NTFS FRN / POSIX inode).
    pub file_id: u64,
}

/// Enumeration entry metadata (32B-ish, cache friendly; doc 06 § 2.1).
#[derive(Debug, Clone, Copy)]
pub struct EntryMeta {
    /// EndOfFile.
    pub size: u64,
    /// AllocationSize (cluster-rounded, sparse-aware).
    pub alloc: u64,
    /// mtime as FILETIME ticks.
    pub mtime: i64,
    /// FILE_ATTRIBUTE_*.
    pub attrs: u32,
    /// Reparse tag or 0.
    pub reparse: u32,
    /// File identity.
    pub file_id: FileIdentity,
    /// Parsed kind.
    pub kind: EntryClass,
}

/// Parsed entry class from attributes + reparse tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryClass {
    /// Regular file.
    File,
    /// Directory.
    Dir,
    /// Junction or symlink.
    Reparse(u32),
    /// Mounted volume.
    Mount,
}

/// One directory's enumeration result (flat name buffer — zero per-entry
/// heap allocations in the hot path, A4).
pub struct DirBatch {
    /// Arena id of the enumerated directory.
    pub dir: crate::arena::NodeId,
    /// Entry count.
    pub count: usize,
    /// Packed UTF-16 names, nul-terminated.
    pub names: Vec<u16>,
    /// name i starts at `name_off[i]` (nul-terminated → length = distance).
    pub name_off: Vec<u32>,
    /// Parallel metadata.
    pub metas: Vec<EntryMeta>,
    /// Enumeration error (access denied class) if the whole dir failed.
    pub error: Option<(i32, String)>,
}

impl DirBatch {
    /// Name i as a UTF-16 slice.
    pub fn name(&self, i: usize) -> &[u16] {
        let start = self.name_off[i] as usize;
        let end = if i + 1 < self.name_off.len() {
            self.name_off[i + 1] as usize
        } else {
            self.names.len()
        };
        // strip the nul terminator
        let stop = self.names[start..end]
            .iter()
            .position(|&u| u == 0)
            .map(|p| start + p)
            .unwrap_or(end);
        &self.names[start..stop]
    }

    /// Empty batch with an error (denied directory).
    pub fn error_batch(dir: crate::arena::NodeId, code: i32, msg: String) -> Self {
        Self {
            dir,
            count: 0,
            names: Vec::new(),
            name_off: Vec::new(),
            metas: Vec::new(),
            error: Some((code, msg)),
        }
    }
}

/// A queued directory task for a worker.
pub struct DirTask {
    /// Arena id of the directory.
    pub node: crate::arena::NodeId,
    /// Absolute path (platform-native UTF-16).
    pub path: Vec<u16>,
    /// Depth from scan root (for drawn-depth limits & cycle heuristics).
    pub depth: u32,
}

/// Enumeration backend trait — both platforms implement this identically-shaped
/// surface so the coordinator is platform-agnostic.
pub trait DirEnumerator: Send {
    /// Enumerate one directory into a batch. `report_error` receives
    /// per-directory failures. Returns the batch (possibly empty with error).
    fn enumerate(&mut self, task: &DirTask) -> DirBatch;
}

/// FILETIME epoch offset: 1601-01-01 → 1970-01-01 in 100ns ticks.
pub const EPOCH_OFFSET_TICKS: i64 = 116_444_736_000_000_000;

/// Convert unix seconds → FILETIME ticks.
pub fn unix_to_filetime_ticks(unix_secs: i64) -> i64 {
    unix_secs
        .saturating_mul(10_000_000)
        .saturating_add(EPOCH_OFFSET_TICKS)
}

/// Convert FILETIME ticks → unix ms (DTO edge). Integer division is exact
/// here: FILETIME ticks are 100ns units.
#[allow(clippy::integer_division)]
pub fn filetime_ticks_to_unix_ms(ticks: i64) -> i64 {
    // Integer division is exact here: FILETIME ticks are 100ns units.
    (ticks - EPOCH_OFFSET_TICKS) / 10_000
}

/// Convert unix ms → FILETIME ticks.
pub fn unix_ms_to_filetime_ticks(ms: i64) -> i64 {
    ms.saturating_mul(10_000).saturating_add(EPOCH_OFFSET_TICKS)
}
