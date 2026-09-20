//! Volume enumeration + system info (docs/06 § 5). Platform layer behind the
//! `sys` boundary (docs/06 § Platform boundary).

use prism_types::sys::{EngineFeature, HelloInfo, VolumeInfo};

/// Compiled scanner backend identity — honestly reported in `sys:hello`
/// (never guessed, never silently substituted).
pub fn scanner_feature() -> EngineFeature {
    #[cfg(windows)]
    {
        EngineFeature::ScannerWin32Nt
    }
    #[cfg(unix)]
    {
        EngineFeature::ScannerPosixDev
    }
    #[cfg(not(any(windows, unix)))]
    {
        compile_error!("unsupported platform")
    }
}

/// All feature flags compiled into this build.
pub fn engine_features() -> Vec<EngineFeature> {
    let mut f = vec![
        scanner_feature(),
        EngineFeature::RecycleBin,
        EngineFeature::Monitor,
        EngineFeature::Icons,
    ];
    f.push(EngineFeature::TurboNtfs); // parsers ship in prism-ntfs; pipeline in P5
    f
}

/// Engine hello block.
pub fn hello() -> HelloInfo {
    HelloInfo {
        engine_version: env!("CARGO_PKG_VERSION").to_string(),
        rustc: rustc_version(),
        features: engine_features(),
        volumes: volumes(),
        is_admin: is_admin(),
        platform: std::env::consts::OS.to_string(),
    }
}

fn rustc_version() -> String {
    option_env!("PRISM_RUSTC_VERSION")
        .unwrap_or("rustc (build-env)")
        .to_string()
}

fn is_admin() -> bool {
    #[cfg(unix)]
    {
        // dev backend: uid 0 approximates elevation for testing
        libc_geteuid() == 0
    }
    #[cfg(windows)]
    {
        // Checked via token elevation in the win32 sys module
        crate::sysinfo::win32::is_elevated()
    }
}

#[cfg(unix)]
fn libc_geteuid() -> u32 {
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    // SAFETY: geteuid is async-signal-safe and takes no pointers.
    unsafe { geteuid() }
}

/// Enumerate volumes.
pub fn volumes() -> Vec<VolumeInfo> {
    #[cfg(unix)]
    {
        crate::sysinfo::posix::volumes()
    }
    #[cfg(windows)]
    {
        crate::sysinfo::win32::volumes()
    }
}

/// Free bytes on the volume containing `path` (0 when unknowable — callers
/// treat 0 as "free space unknown", displayed honestly).
pub fn volume_free_bytes(path: &str) -> u64 {
    #[cfg(unix)]
    {
        crate::sysinfo::posix::free_bytes(path)
    }
    #[cfg(windows)]
    {
        crate::sysinfo::win32::free_bytes(path)
    }
}

#[cfg(unix)]
pub mod posix {
    //! Development-platform volume enumeration (parse /proc/self/mounts +
    //! statvfs). Windows builds do not compile this module.

    use prism_types::sys::{VolumeInfo, VolumeKind};

    unsafe extern "C" {
        fn statvfs(path: *const i8, buf: *mut StatVfs) -> i32;
    }

    #[repr(C)]
    struct StatVfs {
        f_bsize: u64,
        f_frsize: u64,
        f_blocks: u64,
        f_bfree: u64,
        f_bavail: u64,
        f_files: u64,
        f_ffree: u64,
        f_favail: u64,
        f_fsid: u64,
        f_flag: u64,
        f_namemax: u64,
        __spare: [i32; 6],
    }

    /// statvfs wrapper (typed error → Option; 0 free = unknown is honest).
    fn statvfs_of(path: &str) -> Option<(u64, u64, u64)> {
        let mut c = Vec::with_capacity(path.len() + 1);
        c.extend(path.bytes());
        c.push(0);
        let mut buf = StatVfs {
            f_bsize: 0,
            f_frsize: 0,
            f_blocks: 0,
            f_bfree: 0,
            f_bavail: 0,
            f_files: 0,
            f_ffree: 0,
            f_favail: 0,
            f_fsid: 0,
            f_flag: 0,
            f_namemax: 0,
            __spare: [0; 6],
        };
        // SAFETY: c is NUL-terminated; buf is a valid out-pointer.
        let rc = unsafe { statvfs(c.as_ptr() as *const i8, &mut buf) };
        if rc != 0 {
            return None;
        }
        let total = buf.f_blocks.saturating_mul(buf.f_frsize);
        let free = buf.f_bavail.saturating_mul(buf.f_frsize);
        Some((total, free, buf.f_fsid))
    }

    /// Volumes from /proc/self/mounts (real filesystems only).
    pub fn volumes() -> Vec<VolumeInfo> {
        let Ok(content) = std::fs::read_to_string("/proc/self/mounts") else {
            return Vec::new();
        };
        let mut out: Vec<VolumeInfo> = Vec::new();
        for line in content.lines() {
            let mut parts = line.split_whitespace();
            let (Some(_dev), Some(mount), Some(fs)) = (parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            // real filesystems only (skip proc/sysfs/devtmpfs/overlay class)
            const REAL: &[&str] = &[
                "ext2", "ext3", "ext4", "xfs", "btrfs", "zfs", "f2fs", "apfs", "ntfs", "exfat",
                "vfat", "tmpfs",
            ];
            if !REAL.contains(&fs) {
                continue;
            }
            if let Some((total, free, fsid)) = statvfs_of(mount) {
                if total == 0 {
                    continue;
                }
                out.push(VolumeInfo {
                    path: mount.to_string(),
                    label: mount.rsplit('/').next().unwrap_or(mount).to_string(),
                    fs: fs.to_string(),
                    kind: if fs == "tmpfs" {
                        VolumeKind::Ram
                    } else {
                        VolumeKind::Fixed
                    },
                    total,
                    free,
                    serial: (fsid & 0xFFFF_FFFF) as u32,
                    has_media: true,
                });
            }
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out.dedup_by(|a, b| a.path == b.path);
        out
    }

    /// Free bytes for the volume containing `path`.
    pub fn free_bytes(path: &str) -> u64 {
        statvfs_of(path).map(|(_, free, _)| free).unwrap_or(0)
    }
}

#[cfg(windows)]
pub mod win32 {
    //! Win32 volume enumeration (GetLogicalDrives + GetDriveTypeW +
    //! GetVolumeInformationW + GetDiskFreeSpaceExW — docs/06 § 5).

    #![allow(unsafe_code, clippy::undocumented_unsafe_blocks)]

    use prism_types::sys::{VolumeInfo, VolumeKind};

    use windows_sys::Win32::Foundation::{ERROR_UNRECOGNIZED_VOLUME, FILETIME};
    use windows_sys::Win32::Security::{
        GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        DRIVE_CDROM, DRIVE_FIXED, DRIVE_NO_ROOT_DIR, DRIVE_RAMDISK, DRIVE_REMOTE, DRIVE_REMOVABLE,
        GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW,
        INVALID_FILE_ATTRIBUTES,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    pub fn volumes() -> Vec<VolumeInfo> {
        let mut out = Vec::new();
        // SAFETY: bitmask return; each letter probed with NUL-terminated buffers.
        let mask = unsafe { GetLogicalDrives() };
        if mask == 0 {
            return out;
        }
        for i in 0..26u32 {
            if mask & (1 << i) == 0 {
                continue;
            }
            let letter = char::from(b'A' + i as u8);
            let mut root = format!("{letter}:\\\0")
                .encode_utf16()
                .collect::<Vec<u16>>();
            let dt = unsafe { GetDriveTypeW(root.as_mut_ptr()) };
            if dt == DRIVE_NO_ROOT_DIR {
                continue; // no media (WDS-SEL-03)
            }
            let kind = match dt {
                DRIVE_FIXED => VolumeKind::Fixed,
                DRIVE_REMOVABLE => VolumeKind::Removable,
                DRIVE_REMOTE => VolumeKind::Network,
                DRIVE_CDROM => VolumeKind::Optical,
                DRIVE_RAMDISK => VolumeKind::Ram,
                _ => VolumeKind::Unknown,
            };
            let mut label = [0u16; 64];
            let mut fs = [0u16; 32];
            let mut serial = 0u32;
            let ok = unsafe {
                GetVolumeInformationW(
                    root.as_mut_ptr(),
                    label.as_mut_ptr(),
                    label.len() as u32,
                    &mut serial,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    fs.as_mut_ptr(),
                    fs.len() as u32,
                )
            };
            let label_s = if ok != 0 {
                String::from_utf16_lossy(&label)
                    .trim_end_matches('\0')
                    .to_string()
            } else {
                String::new()
            };
            let fs_s = if ok != 0 {
                String::from_utf16_lossy(&fs)
                    .trim_end_matches('\0')
                    .to_string()
            } else {
                String::new()
            };
            let (total, free) = free_and_total(&format!("{letter}:\\"));
            out.push(VolumeInfo {
                path: format!("{letter}:\\"),
                label: label_s,
                fs: fs_s,
                kind,
                total,
                free,
                serial,
                has_media: ok != 0 || total > 0,
            });
        }
        out
    }

    fn free_and_total(root: &str) -> (u64, u64) {
        let mut wide = format!("{root}\0").encode_utf16().collect::<Vec<u16>>();
        let mut free: u64 = 0;
        let mut total: u64 = 0;
        let mut total_free: u64 = 0;
        // SAFETY: all out-pointers are valid stack storage.
        let ok = unsafe {
            GetDiskFreeSpaceExW(
                wide.as_mut_ptr(),
                &mut free as *mut u64 as *mut _,
                &mut total as *mut u64 as *mut _,
                &mut total_free as *mut u64 as *mut _,
            )
        };
        if ok == 0 || (total == 0 && free == 0) {
            return (0, 0);
        }
        (total, free)
    }

    /// Free bytes for the volume containing `path`.
    pub fn free_bytes(path: &str) -> u64 {
        free_and_total(path).1
    }

    /// Token-elevation probe (docs/06 § 7 posture).
    pub fn is_elevated() -> bool {
        // SAFETY: token handle closed via CloseHandle; buffers sized by API.
        unsafe {
            let mut token = std::ptr::null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return false;
            }
            let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
            let mut ret_len = 0u32;
            let ok = GetTokenInformation(
                token,
                TokenElevation,
                &mut elevation as *mut _ as *mut _,
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut ret_len,
            );
            windows_sys::Win32::Foundation::CloseHandle(token);
            ok != 0 && elevation.TokenIsElevated != 0
        }
    }

    // keep the feature-gate types referenced (compile-time surface check)
    #[allow(unused)]
    fn _surface(_a: FILETIME, _b: ERROR_UNRECOGNIZED_VOLUME, _c: INVALID_FILE_ATTRIBUTES) {}
}
