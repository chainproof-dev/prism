//! System-level DTOs: engine hello, volumes, preflight (docs/05 § 3.1).

use serde::{Deserialize, Serialize};

use crate::ids::UnixMs;

/// What the compiled engine can do (reported honestly, never guessed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EngineFeature {
    /// Win32 NT fast-path enumerator (NtQueryDirectoryFile, Extd class).
    ScannerWin32Nt,
    /// Development-only POSIX enumerator (non-Windows builds; visible label,
    /// never a silent substitute — docs/amendments/AMM-002-dev-platform.md).
    ScannerPosixDev,
    /// Raw NTFS MFT turbo scan (prism-ntfs).
    TurboNtfs,
    /// Icon extraction via shell image list.
    Icons,
    /// Recycle-bin operations.
    RecycleBin,
    /// Process monitor sampling.
    Monitor,
}

/// One enumerated volume/drive (docs/06 § 5).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeInfo {
    /// Win32 root path, e.g. `C:\`.
    pub path: String,
    /// Volume label ("" when none).
    pub label: String,
    /// Filesystem name, e.g. `NTFS`.
    pub fs: String,
    /// Drive kind for glyph selection.
    pub kind: VolumeKind,
    /// Total bytes.
    pub total: u64,
    /// Free bytes.
    pub free: u64,
    /// Volume serial number (hard-link identity domain).
    pub serial: u32,
    /// Media present (false → drive shown disabled with reason).
    pub has_media: bool,
}

/// Drive classification (glyph + latency honesty on Welcome).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VolumeKind {
    /// Internal fixed disk.
    Fixed,
    /// Removable / USB.
    Removable,
    /// Network share (latency warning chip).
    Network,
    /// Optical.
    Optical,
    /// RAM disk / virtual.
    Ram,
    /// Unknown.
    Unknown,
}

/// Engine identity block returned by `sys:hello`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloInfo {
    /// Semver of the engine crate.
    pub engine_version: String,
    /// rustc version string.
    pub rustc: String,
    /// Honestly-reported compiled capabilities.
    pub features: Vec<EngineFeature>,
    /// Volumes visible right now.
    pub volumes: Vec<VolumeInfo>,
    /// Process is elevated (admin).
    pub is_admin: bool,
    /// Platform the engine was built for.
    pub platform: String,
}

/// `sys:preflight` result (docs/05 § 3.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightInfo {
    /// Target path as given.
    pub target: String,
    /// Readable without elevation.
    pub readable: bool,
    /// Reading everything requires elevation (UAC-class roots).
    pub requires_elevation: bool,
    /// Free bytes on the containing volume.
    pub free_bytes: u64,
    /// Total bytes on the containing volume.
    pub total_bytes: u64,
    /// Preflight time (unix ms).
    pub checked_at: UnixMs,
}
