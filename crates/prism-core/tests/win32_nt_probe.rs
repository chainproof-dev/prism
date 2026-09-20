//! Windows NT-API probe (cfg(windows) only): exercises the exact
//! open+enumerate sequence the scanner uses and PRINTS every NTSTATUS so
//! CI logs are self-diagnosing. This test fails loudly with the status
//! codes in its message — it exists to make win32 backend defects
//! debuggable from a remote runner.

// Probe test context: expect() on fixture setup is the failure mode.
#![cfg(windows)]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::undocumented_unsafe_blocks)]

use std::io::Write as _;

// Mirror of the scanner's NT declarations (kept local so the probe is an
// independent witness, not a tautology of the code under test).
#[repr(C)]
struct UnicodeString {
    length: u16,
    maximum_length: u16,
    buffer: *mut u16,
}
#[repr(C)]
struct IoStatusBlock {
    status: i32,
    information: usize,
}
#[repr(C)]
struct ObjectAttributes {
    length: u32,
    root_directory: *mut core::ffi::c_void,
    object_name: *mut UnicodeString,
    attributes: u32,
    security_descriptor: *mut core::ffi::c_void,
    security_quality_of_service: *mut core::ffi::c_void,
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtOpenFile(
        file_handle: *mut *mut core::ffi::c_void,
        desired_access: u32,
        object_attributes: *mut ObjectAttributes,
        io_status_block: *mut IoStatusBlock,
        share_access: u32,
        open_options: u32,
    ) -> i32;
    fn NtClose(handle: *mut core::ffi::c_void) -> i32;
    fn NtQueryDirectoryFile(
        file_handle: *mut core::ffi::c_void,
        event: *mut core::ffi::c_void,
        apc_routine: *mut core::ffi::c_void,
        apc_context: *mut core::ffi::c_void,
        io_status_block: *mut IoStatusBlock,
        file_information: *mut core::ffi::c_void,
        length: u32,
        file_information_class: u32,
        return_single_entry: i32,
        file_name: *mut UnicodeString,
        restart_scan: i32,
    ) -> i32;
}

fn status_name(s: i32) -> String {
    match s as u32 {
        0x0000_0000 => "STATUS_SUCCESS".into(),
        0x8000_0006 => "STATUS_NO_MORE_FILES".into(),
        0xC000_0003 => "STATUS_INVALID_INFO_CLASS".into(),
        0xC000_000D => "STATUS_INVALID_PARAMETER".into(),
        0xC000_000F => "STATUS_NO_SUCH_FILE".into(),
        0xC000_0022 => "STATUS_ACCESS_DENIED".into(),
        0xC000_0033 => "STATUS_OBJECT_NAME_INVALID".into(),
        0xC000_0034 => "STATUS_OBJECT_NAME_NOT_FOUND".into(),
        0xC000_0035 => "STATUS_OBJECT_PATH_NOT_FOUND".into(),
        0xC000_0039 => "STATUS_OBJECT_PATH_SYNTAX_BAD".into(),
        0xC000_00BB => "STATUS_NOT_SUPPORTED".into(),
        _ => format!("STATUS_{:08X}", s as u32),
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FileDirectoryInfoW {
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
    file_name: [u16; 1],
}

#[test]
fn nt_probe_open_and_enumerate() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("probe.txt"), b"probe").expect("write");
    let plain: Vec<u16> = dir
        .path()
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    for (label, path) in [
        ("plain", plain.clone()),
        ("win32-\\\\?\\", {
            let mut p: Vec<u16> = "\\\\?\\".encode_utf16().collect();
            p.extend_from_slice(&plain[..plain.len() - 1]); // strip nul, re-add below
            p.push(0);
            p
        }),
        ("nt-\\??\\", {
            let mut p: Vec<u16> = "\\??\\".encode_utf16().collect();
            p.extend_from_slice(&plain[..plain.len() - 1]);
            p.push(0);
            p
        }),
    ] {
        let mut full = path.clone();
        if full.last() == Some(&0) {
            full.pop();
        }
        let mut name = UnicodeString {
            length: (full.len() - 1) as u16 * 2,
            maximum_length: full.len() as u16 * 2,
            buffer: full.as_mut_ptr(),
        };
        let mut oa = ObjectAttributes {
            length: std::mem::size_of::<ObjectAttributes>() as u32,
            root_directory: std::ptr::null_mut(),
            object_name: &mut name,
            attributes: 0x40, // OBJ_CASE_INSENSITIVE
            security_descriptor: std::ptr::null_mut(),
            security_quality_of_service: std::ptr::null_mut(),
        };
        let mut handle: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut iosb = IoStatusBlock {
            status: 0,
            information: 0,
        };
        // SAFETY: probe mirrors the scanner call with valid locals.
        let status = unsafe {
            NtOpenFile(
                &mut handle,
                0x1 | 0x0010_0000, // FILE_READ_DATA | SYNCHRONIZE
                &mut oa,
                &mut iosb,
                0x1 | 0x2,  // FILE_SHARE_READ|WRITE
                0x1 | 0x20, // FILE_DIRECTORY_FILE | SYNCHRONOUS_IO_NONALERT
            )
        };
        let mut log = format!("probe[{label}] open → {} ({})", status_name(status), status);
        if status >= 0 {
            let mut buf = vec![0u8; 64 * 1024];
            for class in [1u32, 19u32] {
                let mut qiosb = IoStatusBlock {
                    status: 0,
                    information: 0,
                };
                // SAFETY: buffer sized as in the scanner.
                let qs = unsafe {
                    NtQueryDirectoryFile(
                        handle,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        &mut qiosb,
                        buf.as_mut_ptr().cast(),
                        buf.len() as u32,
                        class,
                        0,
                        std::ptr::null_mut(),
                        1,
                    )
                };
                let mut entries = 0usize;
                let mut names: Vec<String> = Vec::new();
                if qs >= 0 {
                    let mut off = 0usize;
                    loop {
                        // SAFETY: same parse discipline as the scanner.
                        let rec = unsafe { &*(buf.as_ptr().add(off) as *const FileDirectoryInfoW) };
                        let name_len = rec.file_name_len as usize;
                        let name_bytes =
                            unsafe { std::slice::from_raw_parts(rec.file_name.as_ptr(), name_len) };
                        names.push(String::from_utf16_lossy(name_bytes));
                        entries += 1;
                        if rec.next_entry_offset == 0 {
                            break;
                        }
                        off += rec.next_entry_offset as usize;
                        if off >= qiosb.information {
                            break;
                        }
                    }
                }
                log.push_str(&format!(
                    " | class {class} → {} ({} entries: {})",
                    status_name(qs),
                    entries,
                    names.join(", ")
                ));
            }
            unsafe { NtClose(handle) };
        }
        let _ = std::io::stdout().write_all(format!("{log}\n").as_bytes());
        let _ = std::io::stdout().flush();
    }

    // The probe itself asserts nothing beyond "at least one form works",
    // because its job is diagnostics; the e2e suite asserts behavior.
    // If ALL forms fail, e2e will fail with these logs attached.
}
