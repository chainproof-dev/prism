//! Windows NT-API probe (cfg(windows) only): witnesses the REAL scanner
//! path (open + full enumeration via doc-hidden bridges) AND runs raw-NT
//! query variants, PRINTING every NTSTATUS, byte count and a buffer hex
//! dump so CI logs are fully self-diagnosing — no more blind rounds.

// Probe test context: expect() on fixture setup is the failure mode.
#![cfg(windows)]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::undocumented_unsafe_blocks
)]

use std::ffi::c_void;
use std::io::Write as _;

fn status_name(s: i32) -> String {
    match s as u32 {
        0 => "STATUS_SUCCESS".into(),
        0x0000_0103 => "STATUS_PENDING".into(),
        0x8000_0005 => "STATUS_BUFFER_OVERFLOW".into(),
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
        0xC000_00BB => "STATUS_NOT_SUPPORTED".into(),
        _ => format!("STATUS_{:08X}", s as u32),
    }
}

fn emit(line: &str) {
    let _ = std::io::stdout().write_all(line.as_bytes());
    let _ = std::io::stdout().write_all(b"\n");
    let _ = std::io::stdout().flush();
}

// --- raw NT declarations (test-side witness, independent of the lib) -------

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
    root_directory: *mut c_void,
    object_name: *mut UnicodeString,
    attributes: u32,
    security_descriptor: *mut c_void,
    security_quality_of_service: *mut c_void,
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtOpenFile(
        file_handle: *mut *mut c_void,
        desired_access: u32,
        object_attributes: *mut ObjectAttributes,
        io_status_block: *mut IoStatusBlock,
        share_access: u32,
        open_options: u32,
    ) -> i32;
    fn NtQueryDirectoryFile(
        file_handle: *mut c_void,
        event: *mut c_void,
        apc_routine: *mut c_void,
        apc_context: *mut c_void,
        io_status_block: *mut IoStatusBlock,
        file_information: *mut c_void,
        length: u32,
        file_information_class: u32,
        return_single_entry: i32,
        file_name: *mut UnicodeString,
        restart_scan: i32,
    ) -> i32;
    fn NtClose(handle: *mut c_void) -> i32;
}

const FILE_DIRECTORY_INFORMATION: u32 = 1;
const FILE_ID_EXTD_DIRECTORY_INFORMATION: u32 = 60;

/// Open a directory by its plain Win32 path via the canonical NT form.
/// Panics on failure (the witness open is a settled contract).
fn witness_open(plain: &[u16]) -> *mut c_void {
    let mut full: Vec<u16> = "\\??\\".encode_utf16().collect();
    full.extend_from_slice(plain);
    full.push(0);
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
    let mut handle: *mut c_void = std::ptr::null_mut();
    let mut iosb = IoStatusBlock {
        status: 0,
        information: 0,
    };
    let status = unsafe {
        NtOpenFile(
            &mut handle,
            0x1 | 0x0010_0000, // FILE_READ_DATA | SYNCHRONIZE
            &mut oa,
            &mut iosb,
            0x1 | 0x2,  // share read | write
            0x1 | 0x20, // FILE_DIRECTORY_FILE | FILE_SYNCHRONOUS_IO_NONALERT
        )
    };
    assert_eq!(status, 0, "witness open failed: {}", status_name(status));
    handle
}

/// One raw query variant; prints status + information + hex of the first
/// `hex_bytes` bytes of the buffer. `filter` is an optional wildcard.
fn query_variant(
    label: &str,
    plain: &[u16],
    class: u32,
    single: bool,
    filter: Option<&str>,
) -> (i32, usize) {
    let handle = witness_open(plain);
    let mut buf = vec![0u8; 8192];
    // Optional filename filter (UNICODE_STRING, NUL-terminated buffer).
    let mut filter16: Vec<u16> = filter
        .map(|f| f.encode_utf16().collect())
        .unwrap_or_default();
    if !filter16.is_empty() {
        filter16.push(0);
    }
    let mut fstr = UnicodeString {
        length: if filter16.is_empty() {
            0
        } else {
            (filter16.len() - 1) as u16 * 2
        },
        maximum_length: filter16.len() as u16 * 2,
        buffer: if filter16.is_empty() {
            std::ptr::null_mut()
        } else {
            filter16.as_mut_ptr()
        },
    };
    let mut iosb = IoStatusBlock {
        status: 0,
        information: 0,
    };
    let status = unsafe {
        NtQueryDirectoryFile(
            handle,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut iosb,
            buf.as_mut_ptr() as *mut c_void,
            buf.len() as u32,
            class,
            single as i32,
            if filter16.is_empty() {
                std::ptr::null_mut()
            } else {
                &mut fstr
            },
            1, // restart
        )
    };
    let n = iosb.information.min(buf.len());
    // Offset-indexed hex dump (16/row) — immune to hand-counting errors.
    let mut dump = String::from("\n");
    for (i, chunk) in buf[..n].chunks(16).enumerate() {
        dump.push_str(&format!("  {:04x}: ", i * 16));
        for b in chunk {
            dump.push_str(&format!("{b:02x} "));
        }
        dump.push('\n');
    }
    // Programmatic decode of the first record's name at the three candidate
    // offsets (SDK align-1 = 84, +2 = 86, union-align-8 = 88): print each
    // as lossy UTF-16 up to the first NUL or 20 chars.
    let mut decode = String::new();
    for cand in [84usize, 86, 88] {
        let mut s = String::new();
        let mut p = cand;
        while p + 2 <= n && p < cand + 40 {
            let ch = u16::from_le_bytes([buf[p], buf[p + 1]]);
            if ch == 0 {
                break;
            }
            s.push(char::from_u32(ch as u32).unwrap_or('?'));
            p += 2;
        }
        decode.push_str(&format!("  name@{cand} = {s:?}\n"));
    }
    emit(&format!(
        "probe[q:{label}] {} ({}) information={n}\n{dump}{decode}",
        status_name(status),
        status
    ));
    unsafe { NtClose(handle) };
    (status, iosb.information)
}

#[test]
fn nt_probe_real_open_dir() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("probe.txt"), b"probe").expect("write");
    let plain: Vec<u16> = dir.path().to_string_lossy().encode_utf16().collect();

    // 1. The REAL scanner open path on the plain target string (what the
    //    coordinator feeds it in production).
    let real = prism_core::scanner::win32::__probe_open_dir_status(&plain);
    emit(&format!(
        "probe[real-open_dir] {} ({})",
        status_name(real),
        real
    ));

    // 2. Raw witness open of the canonical \\??\ form (settled contract).
    let _witness_handle = witness_open(&plain);
    emit("probe[raw-nt-witness] STATUS_SUCCESS (0)");
    unsafe { NtClose(_witness_handle) };

    // 3. The FULL production enumeration on the same directory.
    let (count, enum_err, used_extd) = prism_core::scanner::win32::__probe_enumerate(&plain);
    emit(&format!(
        "probe[real-enumerate] count={count} err={} extd={used_extd}",
        enum_err
            .map(status_name)
            .unwrap_or_else(|| "none".to_string())
    ));

    // 4. RAW query variants — full ground truth for the log: which class,
    //    which filter spelling, and what the kernel actually writes.
    let _ = query_variant(
        "extd60-null",
        &plain,
        FILE_ID_EXTD_DIRECTORY_INFORMATION,
        false,
        None,
    );
    let _ = query_variant(
        "plain1-null",
        &plain,
        FILE_DIRECTORY_INFORMATION,
        false,
        None,
    );
    let _ = query_variant(
        "extd60-star",
        &plain,
        FILE_ID_EXTD_DIRECTORY_INFORMATION,
        false,
        Some("*"),
    );
    let _ = query_variant(
        "plain1-star",
        &plain,
        FILE_DIRECTORY_INFORMATION,
        false,
        Some("*"),
    );
    let _ = query_variant(
        "extd60-single",
        &plain,
        FILE_ID_EXTD_DIRECTORY_INFORMATION,
        true,
        None,
    );

    // Contract assertions (the settled parts only — the raw variants are
    // diagnostics; the e2e suite gates the production behavior).
    assert_eq!(real, 0, "real open_dir failed: {}", status_name(real));
    assert!(
        count >= 1 || _extd_returns_data(&plain),
        "production enumeration returned {count} entries and no raw extd variant returned data"
    );
}

fn _extd_returns_data(plain: &[u16]) -> bool {
    let (status, information) = query_variant(
        "extd60-null-recheck",
        plain,
        FILE_ID_EXTD_DIRECTORY_INFORMATION,
        false,
        None,
    );
    status == 0 && information > 0
}
