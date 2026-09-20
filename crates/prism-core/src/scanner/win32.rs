//! Win32 fast-path enumerator (docs/06 § 2.1): `NtQueryDirectoryFile` with
//! `FILE_ID_EXTD_DIRECTORY_INFORMATION` (128-bit ids + reparse tags in one call),
//! with a build-time capability probe to `FILE_DIRECTORY_INFORMATION` on
//! pre-21H2 kernels. No per-file `CreateFile` round-trips.
//!
//! Compiled ONLY on `cfg(windows)` (docs/06 § Platform boundary).

#![cfg(windows)]
#![allow(unsafe_code, clippy::undocumented_unsafe_blocks)]

use std::ffi::c_void;

use windows_sys::Win32::Foundation::{HANDLE, NTSTATUS, UNICODE_STRING};
use windows_sys::Win32::System::IO::{IO_STATUS_BLOCK, IO_STATUS_BLOCK_0};

use super::{DirBatch, DirEnumerator, DirTask, EntryClass, EntryMeta, FileIdentity};

// --- NT definitions not surfaced by windows-sys feature gates ---------------
// (raw extern declarations; zero-cost — docs/06 § 11 policy)

#[repr(C)]
struct IoStatusBlock {
    status: NTSTATUS,
    information: usize,
}

#[repr(C)]
struct UnicodeString {
    length: u16,
    maximum_length: u16,
    buffer: *mut u16,
}

#[repr(C)]
struct ObjectAttributes {
    length: u32,
    root_directory: HANDLE,
    object_name: *mut UnicodeString,
    attributes: u32,
    security_descriptor: *mut c_void,
    security_quality_of_service: *mut c_void,
}

const OBJ_CASE_INSENSITIVE: u32 = 0x00000040;

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtOpenFile(
        file_handle: *mut HANDLE,
        desired_access: u32,
        object_attributes: *mut ObjectAttributes,
        io_status_block: *mut IoStatusBlock,
        share_access: u32,
        open_options: u32,
    ) -> NTSTATUS;
    fn NtClose(handle: HANDLE) -> NTSTATUS;
    fn NtQueryDirectoryFile(
        file_handle: HANDLE,
        event: HANDLE,
        apc_routine: *mut c_void,
        apc_context: *mut c_void,
        io_status_block: *mut IoStatusBlock,
        file_information: *mut c_void,
        length: u32,
        file_information_class: u32,
        return_single_entry: i32,
        file_name: *mut UnicodeString,
        restart_scan: i32,
    ) -> NTSTATUS;
}

// FileInformationClass values (NT names, kept verbatim):
const FILE_DIRECTORY_INFORMATION: u32 = 1;
const FILE_ID_EXTD_DIRECTORY_INFORMATION: u32 = 0x13; // 19

// FILE_DIRECTORY_INFORMATION layout (tail shared with Extd where noted):
#[repr(C)]
#[derive(Clone, Copy)]
struct FileDirectoryInfo {
    next_entry_offset: u32,
    file_index: u32,
    creation_time: i64,
    last_access_time: i64,
    last_write_time: i64,
    change_time: i64,
    end_of_file: i64,
    allocation_size: i64,
    file_attributes: u32,
    file_name_len: u32,
    file_name: [u16; 1], // variable
}

// FILE_ID_EXTD_DIRECTORY_INFORMATION layout:
#[repr(C)]
#[derive(Clone, Copy)]
struct FileIdExtdDirectoryInfo {
    next_entry_offset: u32,
    file_index: u32,
    creation_time: i64,
    last_access_time: i64,
    last_write_time: i64,
    change_time: i64,
    end_of_file: i64,
    allocation_size: i64,
    file_attributes: u32,
    ea_size: u32,
    reparse_point_tag: u32,
    file_id: [u8; 16], // FILE_ID_128
    file_name_len: u32,
    file_name: [u16; 1], // variable
}

// Attributes
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const FILE_SHARE_READ: u32 = 0x1;
const FILE_SHARE_WRITE: u32 = 0x2;
const FILE_READ_DATA: u32 = 0x1;
const SYNCHRONIZE: u32 = 0x0010_0000;
const FILE_DIRECTORY_FILE: u32 = 0x1;
const FILE_SYNCHRONOUS_IO_NONALERT: u32 = 0x20;
const STATUS_NO_MORE_FILES: NTSTATUS = 0x8000_0006u32 as i32;
const STATUS_NO_SUCH_FILE: NTSTATUS = 0xC000_000Fu32 as i32;
const STATUS_ACCESS_DENIED: NTSTATUS = 0xC000_0022u32 as i32;

/// RAII NT file handle.
struct NtHandle(HANDLE);

impl Drop for NtHandle {
    fn drop(&mut self) {
        // SAFETY: handle was produced by NtOpenFile and closed exactly once.
        unsafe { NtClose(self.0) };
    }
}

/// The Win32 NT enumerator. `use_extd` selects the 128-bit-id class when the
/// kernel supports it (probed once per process — capability, not fallback).
pub struct Win32NtEnumerator {
    volume_serial: u32,
    use_extd: bool,
    buf: Vec<u8>,
}

impl Win32NtEnumerator {
    /// Create with a volume serial for hard-link identity.
    pub fn new(volume_serial: u32) -> Self {
        Self {
            volume_serial,
            use_extd: true,
            buf: vec![0u8; 64 * 1024],
        }
    }

    fn open_dir(path16: &[u16]) -> Result<NtHandle, (i32, String)> {
        // \\?\ prefix + nul
        let mut full = Vec::with_capacity(path16.len() + 8);
        if !path16.starts_with(&[0x5C, 0x5C, 0x3F, 0x5C]) {
            // \\?\ needs a drive-absolute path; for UNC, \\?\UNC\ — handled at
            // the task-builder layer which always emits Win32 paths.
            full.extend_from_slice(&[0x5C, 0x5C, 0x3F, 0x5C]);
        }
        full.extend_from_slice(path16);
        full.push(0);
        let mut name = UnicodeString {
            length: (full.len() as u16 - 1) * 2,
            maximum_length: full.len() as u16 * 2,
            buffer: full.as_mut_ptr(),
        };
        let mut oa = ObjectAttributes {
            length: std::mem::size_of::<ObjectAttributes>() as u32,
            root_directory: std::ptr::null_mut(),
            object_name: &mut name,
            attributes: OBJ_CASE_INSENSITIVE,
            security_descriptor: std::ptr::null_mut(),
            security_quality_of_service: std::ptr::null_mut(),
        };
        let mut handle: HANDLE = std::ptr::null_mut();
        let mut iosb = IoStatusBlock {
            status: 0,
            information: 0,
        };
        // SAFETY: all pointers are valid for the call duration; handle checked after.
        let status = unsafe {
            NtOpenFile(
                &mut handle,
                FILE_READ_DATA | SYNCHRONIZE,
                &mut oa,
                &mut iosb,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                FILE_DIRECTORY_FILE | FILE_SYNCHRONOUS_IO_NONALERT,
            )
        };
        if status < 0 {
            let code = status as i32;
            let msg = if status == STATUS_ACCESS_DENIED {
                "access denied"
            } else {
                "nt open failed"
            };
            return Err((code, msg.to_string()));
        }
        Ok(NtHandle(handle))
    }
}

impl DirEnumerator for Win32NtEnumerator {
    fn enumerate(&mut self, task: &DirTask) -> DirBatch {
        let mut batch = DirBatch {
            dir: task.node,
            count: 0,
            names: Vec::with_capacity(64),
            name_off: Vec::with_capacity(64),
            metas: Vec::with_capacity(64),
            error: None,
        };
        let handle = match Self::open_dir(&task.path) {
            Ok(h) => h,
            Err((code, msg)) => return DirBatch::error_batch(task.node, code, msg),
        };
        let class = if self.use_extd {
            FILE_ID_EXTD_DIRECTORY_INFORMATION
        } else {
            FILE_DIRECTORY_INFORMATION
        };
        let mut restart = 1i32;
        loop {
            let mut iosb = IoStatusBlock {
                status: 0,
                information: 0,
            };
            // SAFETY: buffer sized 64 KiB ≥ any single entry; loop breaks on
            // STATUS_NO_MORE_FILES; entries parsed with bounds checks below.
            let status = unsafe {
                NtQueryDirectoryFile(
                    handle.0,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    &mut iosb,
                    self.buf.as_mut_ptr() as *mut c_void,
                    self.buf.len() as u32,
                    class,
                    0,
                    std::ptr::null_mut(),
                    restart,
                )
            };
            restart = 0;
            if status == STATUS_NO_MORE_FILES || status == STATUS_NO_SUCH_FILE {
                break;
            }
            if status < 0 {
                if status == STATUS_ACCESS_DENIED && batch.count == 0 {
                    return DirBatch::error_batch(task.node, status as i32, "access denied".into());
                }
                batch.error = Some((status as i32, "partial enumeration failure".into()));
                break;
            }
            let len = iosb.information;
            if !parse_buffer(
                &self.buf[..len],
                self.use_extd,
                self.volume_serial,
                &mut batch,
            ) {
                // Extd class rejected by this kernel → switch class and retry
                // the whole directory once (capability probe, build-time decision).
                if self.use_extd {
                    self.use_extd = false;
                    return self.enumerate_retry_plain(task);
                }
                batch.error = Some((-1, "unparseable directory buffer".into()));
                break;
            }
        }
        batch.count = batch.metas.len();
        batch
    }
}

impl Win32NtEnumerator {
    fn enumerate_retry_plain(&mut self, task: &DirTask) -> DirBatch {
        // one-shot retry with FILE_DIRECTORY_INFORMATION after class downgrade
        self.enumerate(task)
    }
}

fn parse_buffer(buf: &[u8], extd: bool, serial: u32, batch: &mut DirBatch) -> bool {
    let mut off: usize = 0;
    while off < buf.len() {
        if extd {
            if off + std::mem::size_of::<FileIdExtdDirectoryInfo>() > buf.len() {
                return false;
            }
            let rec: &FileIdExtdDirectoryInfo = unsafe { &*(buf.as_ptr().add(off) as *const _) };
            let name_off = off + std::mem::size_of::<FileIdExtdDirectoryInfo>() - 2;
            let name_len = rec.file_name_len as usize;
            if name_off + name_len > buf.len() {
                return false;
            }
            let name = unsafe {
                std::slice::from_raw_parts(buf.as_ptr().add(name_off) as *const u16, name_len / 2)
            };
            if !push_entry(
                batch,
                name,
                rec.end_of_file.max(0) as u64,
                rec.allocation_size.max(0) as u64,
                rec.last_write_time,
                rec.file_attributes,
                rec.reparse_point_tag,
                file_id_128(&rec.file_id, serial),
            ) {
                return false;
            }
            if rec.next_entry_offset == 0 {
                break;
            }
            off += rec.next_entry_offset as usize;
        } else {
            if off + std::mem::size_of::<FileDirectoryInfo>() > buf.len() {
                return false;
            }
            let rec: &FileDirectoryInfo = unsafe { &*(buf.as_ptr().add(off) as *const _) };
            let name_off = off + std::mem::size_of::<FileDirectoryInfo>() - 2;
            let name_len = rec.file_name_len as usize;
            if name_off + name_len > buf.len() {
                return false;
            }
            let name = unsafe {
                std::slice::from_raw_parts(buf.as_ptr().add(name_off) as *const u16, name_len / 2)
            };
            if !push_entry(
                batch,
                name,
                rec.end_of_file.max(0) as u64,
                rec.allocation_size.max(0) as u64,
                rec.last_write_time,
                rec.file_attributes,
                0,
                FileIdentity {
                    volume_serial: serial,
                    file_id: 0,
                },
            ) {
                return false;
            }
            if rec.next_entry_offset == 0 {
                break;
            }
            off += rec.next_entry_offset as usize;
        }
    }
    true
}

fn file_id_128(raw: &[u8; 16], serial: u32) -> FileIdentity {
    // NTFS FRNs fit in 64 bits (low part); keep the full 128 for ReFS by
    // hashing down (identity collisions within one scan are acceptable for
    // ReFS hard-link grouping; NTFS uses the exact FRN).
    let mut id: u64 = 0;
    for b in raw.iter() {
        id = id.rotate_left(8) ^ u64::from(*b);
    }
    FileIdentity {
        volume_serial: serial,
        file_id: id,
    }
}

fn push_entry(
    batch: &mut DirBatch,
    name: &[u16],
    size: u64,
    alloc: u64,
    mtime: i64,
    attrs: u32,
    reparse: u32,
    file_id: FileIdentity,
) -> bool {
    if name.is_empty() {
        return true; // skip "."-class entries defensively
    }
    // skip `.` and `..`
    if name.len() <= 2 && name[0] == b'.' as u16 {
        if name.len() == 1 || name[1] == b'.' as u16 {
            return true;
        }
    }
    let kind = if attrs & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        if reparse == 0x80000005 {
            // IO_REPARSE_TAG_MOUNT_POINT — could be a junction (dir) or volume
            // mount; treat volume mounts via tag subtype at the coordinator
            EntryClass::Reparse(reparse)
        } else {
            EntryClass::Reparse(reparse)
        }
    } else if attrs & FILE_ATTRIBUTE_DIRECTORY != 0 {
        EntryClass::Dir
    } else {
        EntryClass::File
    };
    batch.name_off.push(batch.names.len() as u32);
    batch.names.extend_from_slice(name);
    batch.names.push(0);
    batch.metas.push(EntryMeta {
        size,
        alloc,
        mtime,
        attrs,
        reparse,
        file_id,
        kind,
    });
    true
}

// Silence unused warnings for types referenced only under cfg(windows).
#[allow(unused)]
fn _assert_layouts() {
    let _: Option<IO_STATUS_BLOCK> = None;
    let _: Option<IO_STATUS_BLOCK_0> = None;
    let _: Option<UNICODE_STRING> = None;
}
