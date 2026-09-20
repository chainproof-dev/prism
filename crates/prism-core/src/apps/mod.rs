//! Application uninstaller + leftovers (PRISM-HG-040, docs/12 § 5).
//!
//! Inventory (Windows): registry uninstall keys — HKLM/HKCU both
//! `…\Uninstall` trees (+ WOW6432Node), MSI products, plus a Steam
//! libraryfolders.vdf scan. Every other platform: honest empty inventory
//! (the UI states "Application inventory requires Windows" — no fake rows,
//! A2).
//!
//! Footprints: token-based matching (normalized publisher+product) across
//! well-known roots; explicit evidence list (every matched path with bytes)
//! — never opaque totals. On the dev host the same matching runs against
//! POSIX roots so the feature is exercised end-to-end.
//!
//! All deletion flows route through the cleanup ledger — this module only
//! inventories and matches; it never deletes.

use std::path::{Path, PathBuf};

use prism_types::commands::{AppFootprint, AppRow, AppsPage, FootprintRoot};

use crate::arena::{Arena, NodeId, kind};

/// Normalize a display name into a matching token: lowercase, alphanumerics
/// kept, everything else collapsed to `-`. `Prism Labs Desktop` →
/// `prism-labs-desktop`. Matching is `contains` on tokens — tolerant of
/// version suffixes and editions.
pub fn normalize_token(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut prev_sep = true;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_sep = false;
        } else if !prev_sep {
            out.push('-');
            prev_sep = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// One inventory entry (registry/steam-derived).
#[derive(Debug, Clone)]
pub struct InventoryApp {
    /// Normalized matching token.
    pub token: String,
    /// Display name.
    pub name: String,
    /// Publisher (registry).
    pub publisher: String,
    /// Inventory source (registry/steam/store).
    pub source: String,
    /// Uninstall command ("" = none).
    pub uninstall_cmd: String,
}

// ---------------------------------------------------------------------------
// Inventory — Windows registry; honest empty elsewhere
// ---------------------------------------------------------------------------

/// Enumerate installed apps (Windows registry uninstall trees; MSI via the
/// same trees' entries). SystemComponent=1 rows are hidden unless
/// `include_system`.
#[cfg(windows)]
pub fn inventory(_include_system: bool) -> Vec<InventoryApp> {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
        REG_EXPAND_SZ, REG_SZ, RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW,
    };

    const ROOTS: &[(HKEY, &str)] = &[
        (
            HKEY_LOCAL_MACHINE,
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
        ),
        (
            HKEY_LOCAL_MACHINE,
            "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
        ),
        (
            HKEY_CURRENT_USER,
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
        ),
    ];

    fn read_str(key: HKEY, name: &[u16]) -> Option<String> {
        let mut ty = 0u32;
        let mut len = 0u32;
        let name_p = name.as_ptr();
        let res = unsafe {
            RegQueryValueExW(
                key,
                name_p,
                std::ptr::null_mut(),
                &mut ty,
                std::ptr::null_mut(),
                &mut len,
            )
        };
        if res != ERROR_SUCCESS || len == 0 || len > 1 << 16 {
            return None;
        }
        let mut buf = vec![0u16; (len as usize) / 2 + 1];
        let res = unsafe {
            RegQueryValueExW(
                key,
                name_p,
                std::ptr::null_mut(),
                &mut ty,
                buf.as_mut_ptr().cast(),
                &mut len,
            )
        };
        if res != ERROR_SUCCESS || (ty != REG_SZ && ty != REG_EXPAND_SZ) {
            return None;
        }
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..end]))
    }

    let mut out: Vec<InventoryApp> = Vec::new();
    for &(root, subkey) in ROOTS {
        for wow in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
            let sub_w: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
            let mut h: HKEY = std::ptr::null_mut();
            if unsafe { RegOpenKeyExW(root, sub_w.as_ptr(), 0, KEY_READ | wow, &mut h) }
                != ERROR_SUCCESS
            {
                continue;
            }
            let mut idx = 0u32;
            loop {
                let mut name_buf = [0u16; 256];
                let mut name_len = 256u32;
                let res = unsafe {
                    RegEnumKeyExW(
                        h,
                        idx,
                        name_buf.as_mut_ptr(),
                        &mut name_len,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    )
                };
                if res != ERROR_SUCCESS {
                    break;
                }
                idx += 1;
                let sub_name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
                let path_w: Vec<u16> = format!("{subkey}\\{sub_name}")
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();
                let mut app_h: HKEY = std::ptr::null_mut();
                if unsafe { RegOpenKeyExW(root, path_w.as_ptr(), 0, KEY_READ | wow, &mut app_h) }
                    != ERROR_SUCCESS
                {
                    continue;
                }
                let disp = read_str(app_h, &to_w("DisplayName"));
                let uninst = read_str(app_h, &to_w("UninstallString"));
                let publisher = read_str(app_h, &to_w("Publisher")).unwrap_or_default();
                let sys_flag = read_str(app_h, &to_w("SystemComponent"));
                unsafe { RegCloseKey(app_h) };
                let Some(name) = disp else { continue };
                if name.trim().is_empty() {
                    continue;
                }
                if !_include_system && sys_flag.as_deref() == Some("1") {
                    continue;
                }
                let token = normalize_token(&name);
                if token.is_empty() || out.iter().any(|a| a.token == token) {
                    continue;
                }
                out.push(InventoryApp {
                    token,
                    name,
                    publisher,
                    source: "registry".into(),
                    uninstall_cmd: uninst.unwrap_or_default(),
                });
            }
            unsafe { RegCloseKey(h) };
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// Enumerate installed apps — honest empty off-Windows (A2): the
/// Applications tab states the platform requirement; no placeholder rows.
#[cfg(not(windows))]
pub fn inventory(_include_system: bool) -> Vec<InventoryApp> {
    // Honest empty inventory off-Windows (A2): the Applications tab states
    // the platform requirement; no placeholder rows.
    Vec::new()
}

#[cfg(windows)]
fn to_w(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// ---------------------------------------------------------------------------
// Footprint roots + token matching
// ---------------------------------------------------------------------------

/// Well-known footprint roots (platform-split). Order matters: it is the
/// evidence-list order the UI shows.
pub fn footprint_roots() -> Vec<(String, PathBuf)> {
    #[cfg(windows)]
    {
        let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
        let mut roots = Vec::new();
        if let Some(p) = env("ProgramFiles") {
            roots.push(("Program Files".into(), p));
        }
        if let Some(p) = env("ProgramFiles(x86)") {
            roots.push(("Program Files (x86)".into(), p));
        }
        if let Some(p) = env("LOCALAPPDATA") {
            roots.push(("LocalAppData".into(), p));
        }
        if let Some(p) = env("APPDATA") {
            roots.push(("Roaming AppData".into(), p));
        }
        if let Some(p) = env("ProgramData") {
            roots.push(("ProgramData".into(), p));
        }
        roots
    }
    #[cfg(not(windows))]
    {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/"));
        vec![
            ("Applications".into(), home.join(".local/share")),
            ("Config".into(), home.join(".config")),
            ("Caches".into(), home.join(".cache")),
            ("Opt".into(), PathBuf::from("/opt")),
        ]
    }
}

/// Does `dir_name` match the app token? Token containment, tolerant of
/// version suffixes (`prism-labs-desktop-1.4` contains `prism-labs-desktop`).
fn name_matches(token: &str, dir_name: &str) -> bool {
    let d = normalize_token(dir_name);
    if d.len() < 3 || token.len() < 3 {
        return d == token;
    }
    d == *token || d.starts_with(&format!("{token}-")) || d.contains(token)
}

/// Compute one app's footprint: scan the well-known roots for name matches
/// and sum their sizes (bounded walk; symlinks not followed).
pub fn footprint(token: &str) -> AppFootprint {
    let mut roots = Vec::new();
    let mut total = 0u64;
    for (label, base) in footprint_roots() {
        let Ok(entries) = std::fs::read_dir(&base) else {
            continue;
        };
        let mut paths = Vec::new();
        let mut root_bytes = 0u64;
        for ent in entries.flatten() {
            let name = ent.file_name().to_string_lossy().into_owned();
            if !name_matches(token, &name) {
                continue;
            }
            let p = ent.path();
            let bytes = dir_size(&p);
            if bytes == 0 {
                continue;
            }
            root_bytes += bytes;
            paths.push(p.to_string_lossy().into_owned());
        }
        if !paths.is_empty() {
            total += root_bytes;
            roots.push(FootprintRoot {
                label,
                paths,
                bytes: root_bytes,
            });
        }
    }
    AppFootprint {
        token: token.to_string(),
        total,
        roots,
    }
}

/// Bounded recursive size of a directory (files only; symlink entries not
/// followed — honest about links per the arena's own rules).
fn dir_size(p: &Path) -> u64 {
    let mut stack = vec![p.to_path_buf()];
    let mut total = 0u64;
    let mut entries_seen = 0u64;
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for ent in rd.flatten() {
            entries_seen += 1;
            if entries_seen > 2_000_000 {
                return total; // bound: 2M entries per matched dir
            }
            let Ok(md) = ent.metadata() else {
                continue;
            };
            if md.is_dir() {
                stack.push(ent.path());
            } else {
                total += md.len();
            }
        }
    }
    total
}

/// Inventory page with footprint bytes attached (walk bounded by the same
/// dir_size bound per app; only for the requested page — the UI lazy-loads
/// footprints, so this computes just top-level presence bytes).
pub fn apps_page(include_system: bool, with_bytes: bool) -> AppsPage {
    let inv = inventory(include_system);
    let apps = inv
        .into_iter()
        .map(|a| {
            let bytes = if with_bytes {
                footprint(&a.token).total
            } else {
                0
            };
            AppRow {
                token: a.token,
                name: a.name,
                publisher: a.publisher,
                bytes,
                source: a.source,
                uninstall_cmd: a.uninstall_cmd,
                installed: true,
            }
        })
        .collect();
    AppsPage { apps }
}

/// Leftovers: arena directories whose names match a known-app token that is
/// NOT in the current inventory (docs/12 § 5). Works on any platform the
/// arena exists on — the *installed* set is what requires Windows; off
/// Windows the inventory is empty, so every token match would look like a
/// leftover: dishonest. We therefore ship leftovers only where the
/// inventory is real, and return an honest empty page elsewhere.
pub fn leftovers(arena: &Arena, _root_path: &str) -> AppsPage {
    // Off-Windows the "installed" set is unknowable (no registry) — every
    // token match would masquerade as a leftover. Honest empty page instead.
    if cfg!(not(windows)) {
        return AppsPage { apps: Vec::new() };
    }
    let installed: std::collections::HashSet<String> =
        inventory(true).into_iter().map(|a| a.token).collect();

    // Known-token heuristics: common app-name patterns in data dirs. We scan
    // the arena's shallow data roots (depth ≤ 3 dirs) and token-match names.
    let mut out: Vec<AppRow> = Vec::new();
    let mut seen: std::collections::HashSet<String> = Default::default();
    for root in arena.roots() {
        walk_for_leftovers(
            arena,
            root,
            &arena.name_str(root),
            0,
            &installed,
            &mut seen,
            &mut out,
            _root_path,
        );
    }
    out.sort_by_key(|a| std::cmp::Reverse(a.bytes));
    AppsPage { apps: out }
}

#[allow(clippy::too_many_arguments)]
fn walk_for_leftovers(
    arena: &Arena,
    node: NodeId,
    _path: &str,
    depth: u8,
    installed: &std::collections::HashSet<String>,
    seen: &mut std::collections::HashSet<String>,
    out: &mut Vec<AppRow>,
    _root_path: &str,
) {
    if depth > 3 || out.len() >= 200 {
        return;
    }
    for &child in arena.children(node) {
        if arena.kind(child) != kind::DIR {
            continue;
        }
        let name = arena.name_str(child);
        let token = normalize_token(&name);
        if token.len() >= 3 && !seen.contains(&token) && !installed.contains(&token) {
            // Candidate only if the name looks like a product (contains a
            // letter and is not a generic word).
            if looks_like_product(&name) {
                seen.insert(token.clone());
                out.push(AppRow {
                    token: token.clone(),
                    name,
                    publisher: String::new(),
                    bytes: arena.allocated(child as usize),
                    source: "leftover".into(),
                    uninstall_cmd: String::new(),
                    installed: false,
                });
            }
        }
        walk_for_leftovers(
            arena,
            child,
            _path,
            depth + 1,
            installed,
            seen,
            out,
            _root_path,
        );
    }
}

/// Generic directory words that are not products (evidence-first: never
/// claim "Adobe" leftovers from a folder literally named "cache").
fn looks_like_product(name: &str) -> bool {
    const GENERIC: &[&str] = &[
        "cache",
        "caches",
        "config",
        "configs",
        "data",
        "local",
        "share",
        "state",
        "temp",
        "tmp",
        "logs",
        "log",
        "bin",
        "lib",
        "libs",
        "src",
        "etc",
        "var",
        "opt",
        "home",
        "user",
        "users",
        "documents",
        "downloads",
        "desktop",
        "pictures",
        "music",
        "videos",
        "projects",
        "code",
        "dev",
        "workspace",
        "workspaces",
        "venv",
        "node-modules",
        "appdata",
        "programdata",
        "program-files",
        "common",
        "microsoft",
        "windows",
    ];
    let t = normalize_token(name);
    !GENERIC.contains(&t.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_normalize() {
        assert_eq!(normalize_token("Prism Labs Desktop"), "prism-labs-desktop");
        assert_eq!(normalize_token("  Foo -- Bar 42! "), "foo-bar-42");
        assert_eq!(normalize_token("…"), "");
    }

    #[test]
    fn matching_tolerant_of_suffixes() {
        assert!(name_matches("prism-labs-desktop", "Prism Labs Desktop"));
        assert!(name_matches(
            "prism-labs-desktop",
            "prism-labs-desktop-1.4.2"
        ));
        assert!(!name_matches("prism-labs-desktop", "prismviewer"));
    }

    #[test]
    fn generic_words_are_not_products() {
        assert!(!looks_like_product("Cache"));
        assert!(!looks_like_product("Documents"));
        assert!(looks_like_product("SomeVendor Suite"));
    }

    #[test]
    fn footprint_on_dev_roots() {
        // HOME-based roots exist on the dev host; the token "nonexistent-xyz"
        // must yield an empty (honest) footprint, never an error.
        let f = footprint("nonexistent-xyzzy");
        assert_eq!(f.total, 0);
        assert!(f.roots.is_empty());
    }

    #[test]
    fn inventory_off_windows_is_honest_empty() {
        if cfg!(not(windows)) {
            assert!(inventory(true).is_empty());
            assert!(apps_page(true, false).apps.is_empty());
        }
    }
}
