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

// FileInformationClass values (NT names, kept verbatim). The numeric
// values are the FILE_INFORMATION_CLASS enum ordinals — cross-checked
// against the winternl/ntifs enumeration:
//   1  = FileDirectoryInformation
//   60 = FileIdExtdDirectoryInformation  (NOT 19 — 19 is FileAllocation-
//        Information territory; passing it to NtQueryDirectoryFile returns
//        STATUS_INVALID_INFO_CLASS, exactly what the windows CI caught)
const FILE_DIRECTORY_INFORMATION: u32 = 1;
const FILE_ID_EXTD_DIRECTORY_INFORMATION: u32 = 60;

// FILE_DIRECTORY_INFORMATION layout. FileNameLength is in BYTES and the
// name is NOT NUL-terminated for this class.
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
    file_name: [u16; 1], // variable, length from file_name_len
}

// FILE_ID_128 as the kernel's ntifs.h defines it: a UNION containing
// ULONGLONG members → 8-byte alignment. This is the subtle part: the
// user-mode SDK spelling looks align-1 (UCHAR[16]), but the FS driver lays
// the record out with the union alignment, pushing FileId to offset 72
// and FileName to 88. MEASURED on a real windows-latest runner (probe v3
// ground truth): single-entry information=90 = 88 + name(2); 3-entry
// 298 = 96+96+106; NextEntryOffset=96; MFT-ref bytes at FileId[0..8]
// (72..80) decode as a valid record/sequence pair.
#[repr(C, align(8))]
#[derive(Clone, Copy)]
struct NtFileId128 {
    id: [u8; 16],
}

// FILE_ID_EXTD_DIRECTORY_INFORMATION layout. This class has NO length
// field — the name is NUL-terminated (the FINAL record may omit the NUL
// and simply run to the end of the returned data). Name offset via
// offset_of! — never `size_of - 2` (struct tail pads to 8).
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
    file_id: NtFileId128, // 8-aligned → offset 72..88
    file_name: [u16; 1],  // offset 88, variable, NUL-terminated
}

// ABI layout pinned to MEASURED kernel behavior (windows-latest).
// Any field addition/removal/reorder breaks the build HERE instead of
// misparsing kernel buffers at runtime.
const _: () = {
    assert!(std::mem::size_of::<FileDirectoryInfo>() == 72);
    assert!(std::mem::offset_of!(FileDirectoryInfo, file_name) == 64);
    assert!(std::mem::size_of::<FileIdExtdDirectoryInfo>() == 96);
    assert!(std::mem::offset_of!(FileIdExtdDirectoryInfo, file_name) == 88);
};

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
const STATUS_INVALID_INFO_CLASS: NTSTATUS = 0xC000_0003u32 as i32;

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
        // NT object-manager path construction lives in `nt_path` (pure,
        // platform-independent, unit-tested on every CI platform — including
        // Linux, where path-construction regressions are caught cheaply).
        // NtOpenFile bypasses the Win32 layer: the Win32 "\\?\" spelling is
        // invalid here (empty path component → STATUS_OBJECT_NAME_INVALID);
        // drive paths need the `\??\` DOS-device link directory, UNC maps to
        // `\??\UNC\server\share`.
        let mut full = Vec::with_capacity(path16.len() + 9);
        super::nt_path::push_nt_object_path(path16, &mut full);
        // UNICODE_STRING length fields are u16 BYTES; refuse over-long paths
        // loudly (STATUS_NAME_TOO_LONG) instead of truncating the field.
        if full.len() * 2 > u16::MAX as usize {
            return Err((
                0xC000_010Fu32 as i32, // STATUS_NAME_TOO_LONG
                "path too long for UNICODE_STRING".into(),
            ));
        }

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
            // Entries and STATUS_NO_MORE_FILES can arrive in the SAME call:
            // the filesystem fills the buffer and reports exhaustion in one
            // status (fastfat/NTFS set the final status after copying the
            // last fitting entry). Parse the returned data FIRST, then test
            // the status — checking NO_MORE_FILES before parsing silently
            // drops the final batch (every small directory enumerated as
            // empty: 0 files, 0 errors — the exact windows-CI signature).
            let len = iosb.information;
            if len > 0 {
                let (wellformed, any_name) = parse_buffer(
                    &self.buf[..len],
                    self.use_extd,
                    self.volume_serial,
                    &mut batch,
                );
                if !wellformed || (self.use_extd && !any_name) {
                    // Malformed buffer OR the extd layout does not match this
                    // kernel's record shape (every name scanned as empty —
                    // exactly the signature of an ABI drift). Capability
                    // downgrade to the unambiguous class 1 and one retry; a
                    // wrong guess can never silently misparse names.
                    if self.use_extd {
                        self.use_extd = false;
                        return self.enumerate_retry_plain(task);
                    }
                    batch.error = Some((-1, "unparseable directory buffer".into()));
                    break;
                }
            }
            if status == STATUS_NO_MORE_FILES || status == STATUS_NO_SUCH_FILE {
                break;
            }
            if status < 0 {
                // Extd class rejected by this kernel/filesystem (older builds,
                // exotic redirectors) → capability downgrade to class 1, one
                // retry of the whole directory (documented probe — build-time
                // decision, never a per-directory silent fallback).
                if status == STATUS_INVALID_INFO_CLASS && self.use_extd {
                    self.use_extd = false;
                    return self.enumerate_retry_plain(task);
                }
                if status == STATUS_ACCESS_DENIED && batch.count == 0 {
                    return DirBatch::error_batch(task.node, status as i32, "access denied".into());
                }
                batch.error = Some((status as i32, "partial enumeration failure".into()));
                break;
            }
            if len == 0 {
                // Defensive: a success-status call with zero bytes would
                // otherwise spin forever. Should never happen on a healthy
                // volume — fail loudly if it does.
                batch.error = Some((-1, "empty directory query response".into()));
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

/// Test/diagnostics bridge: run the REAL open_dir on a path and return the
/// raw NTSTATUS (0 on success). Used by tests/win32_nt_probe.rs so the probe
/// witnesses the production code path, not a copy of it.
#[doc(hidden)]
pub fn __probe_open_dir_status(path16: &[u16]) -> i32 {
    match Win32NtEnumerator::open_dir(path16) {
        Ok(_) => 0,
        Err((code, _)) => code,
    }
}

/// Test/diagnostics bridge: run the FULL production enumeration (open +
/// NtQueryDirectoryFile loop + parse) on a path with a throwaway enumerator.
/// Returns (entry count, error code if any, whether the Extd class survived
/// without downgrade). Used by tests/win32_nt_probe.rs.
#[doc(hidden)]
pub fn __probe_enumerate(path16: &[u16]) -> (usize, Option<i32>, bool) {
    let task = DirTask {
        node: 0,
        path: path16.to_vec(),
        depth: 0,
    };
    let mut e = Win32NtEnumerator::new(0);
    let batch = e.enumerate(&task);
    (batch.count, batch.error.map(|(c, _)| c), e.use_extd)
}

fn parse_buffer(buf: &[u8], extd: bool, serial: u32, batch: &mut DirBatch) -> (bool, bool) {
    // Returns (wellformed, any_name_parsed). any_name_parsed=false on a
    // non-empty buffer means every record's name scanned as empty — the
    // signature of an extd ABI mismatch (caller downgrades to class 1).
    let mut any_name = false;
    let mut off: usize = 0;
    while off < buf.len() {
        if extd {
            if off + std::mem::size_of::<FileIdExtdDirectoryInfo>() > buf.len() {
                return (false, any_name);
            }
            // SAFETY: the bounds check above guarantees a full record header
            // at `off`; NtQueryDirectoryFile lays records back-to-back.
            let rec: &FileIdExtdDirectoryInfo = unsafe { &*(buf.as_ptr().add(off) as *const _) };
            let next = rec.next_entry_offset as usize;
            if next != 0 && next < std::mem::size_of::<FileIdExtdDirectoryInfo>() {
                return (false, any_name); // corrupt chain (next must clear the header)
            }
            // This class has NO length field: the name is NUL-terminated and
            // bounded by the next record (or the end of the returned data —
            // the FINAL record may omit the trailing NUL entirely).
            let name_off = off + std::mem::offset_of!(FileIdExtdDirectoryInfo, file_name);
            let span_end = if next != 0 {
                (off + next).min(buf.len())
            } else {
                buf.len()
            };
            if name_off + 2 > span_end {
                return (false, any_name);
            }
            let mut name_end = name_off;
            while name_end + 2 <= span_end && (buf[name_end] | buf[name_end + 1]) != 0 {
                name_end += 2;
            }
            let name_len = name_end - name_off; // even byte count
            if name_len > 0 {
                any_name = true;
            }
            // SAFETY: name bytes are inside buf (bounded above); /2 is the
            // exact UTF-16 byte→code-unit conversion.
            #[allow(clippy::integer_division)]
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
                file_id_128(&rec.file_id.id, serial),
            ) {
                return (false, any_name);
            }
            if next == 0 {
                break;
            }
            off += next;
        } else {
            if off + std::mem::size_of::<FileDirectoryInfo>() > buf.len() {
                return (false, any_name);
            }
            // SAFETY: the bounds check above guarantees a full record header
            // at `off` (same back-to-back layout contract as above).
            let rec: &FileDirectoryInfo = unsafe { &*(buf.as_ptr().add(off) as *const _) };
            let next = rec.next_entry_offset as usize;
            if next != 0 && next < std::mem::size_of::<FileDirectoryInfo>() {
                return (false, any_name); // corrupt chain (next must clear the header)
            }
            // offset_of! — never `size_of - 2`: repr(C) pads the tail to
            // 8-byte alignment (sizeof = 72, name at 64).
            let name_off = off + std::mem::offset_of!(FileDirectoryInfo, file_name);
            let name_len = rec.file_name_len as usize; // BYTES for this class
            if name_off + name_len > buf.len() {
                return (false, any_name);
            }
            if name_len > 0 {
                any_name = true;
            }
            // SAFETY: name bytes are inside buf (checked); /2 is the exact
            // UTF-16 byte→code-unit conversion.
            #[allow(clippy::integer_division)]
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
                return (false, any_name);
            }
            if next == 0 {
                break;
            }
            off += next;
        }
    }
    (true, any_name)
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

#[allow(clippy::too_many_arguments)] // mirrors the flattened NT record fields
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
    if name.len() <= 2 && name[0] == b'.' as u16 && (name.len() == 1 || name[1] == b'.' as u16) {
        return true;
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
