//! NT object-manager path construction (pure UTF-16 string logic).
//!
//! `NtOpenFile`/`NtQueryDirectoryFile` bypass the Win32 path layer, so a
//! Win32 path must be rewritten into the object manager's canonical form
//! before the call:
//!
//! | input (Win32-ish)            | NT object form               |
//! |------------------------------|------------------------------|
//! | `C:\Users\x`                 | `\??\C:\Users\x`             |
//! | `\\?\C:\Users\x`             | `\??\C:\Users\x`             |
//! | `\\?\UNC\srv\share`          | `\??\UNC\srv\share`          |
//! | `\\srv\share`                | `\??\UNC\srv\share`          |
//! | `\??\C:\x` (already NT)      | unchanged                    |
//! | `\Device\...` (object-native)| unchanged                    |
//!
//! The `\\?\` spelling is translated **only** by `CreateFileW`; passing it
//! straight to an NT API yields `STATUS_OBJECT_NAME_INVALID` (the doubled
//! leading backslash reads as an empty path component). Conversely, a bare
//! drive path like `C:\x` is meaningless to the object manager — it needs
//! the `\??\` DOS-device link directory.
//!
//! This module is plain string logic with **no platform gate**: the tests
//! below run on every CI platform, so path-construction regressions (e.g.
//! emitting a prefix with no path attached) fail on Linux before they can
//! ever reach a Windows runner.

/// Append the NT object-manager form of `path16` to `out`, then a single
/// NUL terminator. `path16` must not itself be NUL-terminated.
pub fn push_nt_object_path(path16: &[u16], out: &mut Vec<u16>) {
    const NT_PREFIX: &[u16] = &[0x5C, 0x3F, 0x3F, 0x5C]; // \??\
    const NT_UNC_PREFIX: &[u16] = &[0x5C, 0x3F, 0x3F, 0x5C, 0x55, 0x4E, 0x43, 0x5C]; // \??\UNC\
    const WIN32_PREFIX: &[u16] = &[0x5C, 0x5C, 0x3F, 0x5C]; // \\?\

    let starts_backslash = path16.first() == Some(&0x5C);
    let is_unc = path16.starts_with(&[0x5C, 0x5C]);

    if path16.starts_with(NT_PREFIX) || (starts_backslash && !is_unc) {
        // Already an object-manager path: canonical `\??\` form, or
        // object-native (`\Device\`, `\DosDevices\`, ...). Pass through.
        out.extend_from_slice(path16);
    } else if path16.starts_with(WIN32_PREFIX) {
        // `\\?\C:\x` → `\??\C:\x`; `\\?\UNC\srv\share` → `\??\UNC\srv\share`.
        out.extend_from_slice(NT_PREFIX);
        out.extend_from_slice(&path16[WIN32_PREFIX.len()..]);
    } else if is_unc {
        // `\\srv\share` → `\??\UNC\srv\share` (drop the leading `\\`).
        out.extend_from_slice(NT_UNC_PREFIX);
        out.extend_from_slice(&path16[2..]);
    } else {
        // Drive-absolute Win32 path (`C:\...`): prefix the DOS-device link
        // directory. **Both** the prefix and the path go into `out` — the
        // original bug shipped only the prefix, so every scan root opened
        // the 4-char path `\??\` and failed with NAME_INVALID.
        out.extend_from_slice(NT_PREFIX);
        out.extend_from_slice(path16);
    }
    out.push(0);
}

/// UTF-16 → lossy Rust string (test diagnostics).
#[cfg(test)]
fn s(v: &[u16]) -> String {
    String::from_utf16_lossy(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    /// Build the NT form of `path` and compare against `want` + NUL.
    fn check(path: &str, want: &str) {
        let mut out = Vec::new();
        push_nt_object_path(&u(path), &mut out);
        let mut want16 = u(want);
        want16.push(0);
        assert_eq!(out, want16, "input {path:?} produced {}", s(&out));
    }

    #[test]
    fn plain_drive_path_gets_prefix_and_keeps_path() {
        // Regression guard for the shipped Windows bug: the plain-path
        // branch once emitted ONLY the `\??\` prefix (path never appended),
        // so every scan root opened the 4-char path `\??\` and failed.
        check(
            r"C:\Users\runner\Temp\.tmpXYZ",
            r"\??\C:\Users\runner\Temp\.tmpXYZ",
        );
    }

    #[test]
    fn drive_root_only() {
        check(r"C:\", r"\??\C:\");
    }

    #[test]
    fn win32_extended_prefix_is_rewritten() {
        check(r"\\?\C:\very\long\path", r"\??\C:\very\long\path");
    }

    #[test]
    fn win32_unc_prefix_is_rewritten() {
        check(r"\\?\UNC\server\share\dir", r"\??\UNC\server\share\dir");
    }

    #[test]
    fn bare_unc_maps_to_unc_link() {
        check(r"\\server\share\dir", r"\??\UNC\server\share\dir");
    }

    #[test]
    fn canonical_nt_form_passes_through() {
        check(r"\??\C:\Windows", r"\??\C:\Windows");
    }

    #[test]
    fn object_native_path_passes_through() {
        check(
            r"\Device\HarddiskVolume3\Users",
            r"\Device\HarddiskVolume3\Users",
        );
    }

    fn build(path16: &[u16]) -> Vec<u16> {
        let mut out = Vec::new();
        push_nt_object_path(path16, &mut out);
        out
    }

    #[test]
    fn output_always_ends_in_exactly_one_nul() {
        for case in [
            r"C:\x",
            r"\\?\C:\x",
            r"\\srv\s",
            r"\??\C:\x",
            r"\Device\D\p",
        ] {
            let got = build(&u(case));
            assert_eq!(got.last(), Some(&0), "trailing nul for {case}");
            assert_eq!(
                got.iter().filter(|&&c| c == 0).count(),
                1,
                "exactly one nul for {case}"
            );
        }
    }

    #[test]
    fn never_emits_prefix_without_path() {
        // Regression guard for the shipped bug: the built string must be
        // strictly longer than the 4-char `\??\` prefix alone.
        let got = build(&u(r"C:\Users\x"));
        assert!(got.len() > 5, "built path is just a prefix: {:?}", s(&got));
    }
}
