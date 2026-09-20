//! Windows NT-API probe (cfg(windows) only): exercises the REAL scanner
//! open path (via the `__probe_open_dir_status` bridge) plus a raw-NT
//! witness, and PRINTS every NTSTATUS so CI logs are self-diagnosing.

// Probe test context: expect() on fixture setup is the failure mode.
#![cfg(windows)]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::undocumented_unsafe_blocks
)]

use std::io::Write as _;

fn status_name(s: i32) -> String {
    match s as u32 {
        0 => "STATUS_SUCCESS".into(),
        0x8000_0006 => "STATUS_NO_MORE_FILES".into(),
        0xC000_0003 => "STATUS_INVALID_INFO_CLASS".into(),
        0xC000_000D => "STATUS_INVALID_PARAMETER".into(),
        0xC000_000F => "STATUS_NO_SUCH_FILE".into(),
        0xC000_0022 => "STATUS_ACCESS_DENIED".into(),
        0xC000_0033 => "STATUS_OBJECT_NAME_INVALID".into(),
        0xC000_0034 => "STATUS_OBJECT_NAME_NOT_FOUND".into(),
        0xC000_0035 => "STATUS_OBJECT_PATH_NOT_FOUND".into(),
        0xC000_0039 => "STATUS_OBJECT_PATH_SYNTAX_BAD".into(),
        0xC000_003A => "STATUS_OBJECT_PATH_INVALID".into(),
        0xC000_003B => "STATUS_OBJECT_PATH_SYNTAX_BAD_B".into(),
        0xC000_00BB => "STATUS_NOT_SUPPORTED".into(),
        _ => format!("STATUS_{:08X}", s as u32),
    }
}

#[test]
fn nt_probe_real_open_dir() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("probe.txt"), b"probe").expect("write");
    let plain: Vec<u16> = dir.path().to_string_lossy().encode_utf16().collect();

    // 1. The REAL scanner open path on the plain target string (what the
    //    coordinator feeds it in production).
    let real = prism_core::scanner::win32::__probe_open_dir_status(&plain);
    let _ = std::io::stdout()
        .write_all(format!("probe[real-open_dir] {} ({})\n", status_name(real), real).as_bytes());

    // 2. Raw witness: canonical \\??\ form, nul-terminated, Length excludes nul.
    let mut full: Vec<u16> = "\\??\\".encode_utf16().collect();
    full.extend_from_slice(&plain);
    full.push(0);

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
        attributes: 0x40,
        security_descriptor: std::ptr::null_mut(),
        security_quality_of_service: std::ptr::null_mut(),
    };
    let mut handle: *mut core::ffi::c_void = std::ptr::null_mut();
    let mut iosb = IoStatusBlock {
        status: 0,
        information: 0,
    };
    let witness = unsafe {
        NtOpenFile(
            &mut handle,
            0x1 | 0x0010_0000,
            &mut oa,
            &mut iosb,
            0x1 | 0x2,
            0x1 | 0x20,
        )
    };
    let _ = std::io::stdout().write_all(
        format!(
            "probe[raw-nt-witness] {} ({})\n",
            status_name(witness),
            witness
        )
        .as_bytes(),
    );
    let _ = std::io::stdout().flush();

    // The raw canonical form MUST succeed — that is the contract the scanner
    // relies on. If the real open differs, the diff is visible in the log.
    assert_eq!(
        witness, 0,
        "raw \\??\\ open failed ({witness}) — NT contract broken"
    );
}
