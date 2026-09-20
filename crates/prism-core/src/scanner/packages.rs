//! Package-container extension table (PRISM-ENG-014): directory-style
//! containers that can collapse to single nodes when
//! `ScanOptions::treat_packages_as_nodes` is set.

/// Extensions treated as package containers (directory-like bundles).
pub const PACKAGE_EXTS: &[&str] = &[
    "msix",
    "appx",
    "appxbundle",
    "msixbundle",
    "vhdx",
    "vhd",
    "iso",
    "img",
    "wim",
    "vmdk",
    "vdi",
    "qcow2",
    "vmwarevm",
    "appimage",
];

/// Is `ext` (lowercase, no dot) a package container?
pub fn is_package_ext(ext: &str) -> bool {
    PACKAGE_EXTS.binary_search(&ext).is_ok() || PACKAGE_EXTS.contains(&ext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_common_containers() {
        for e in ["msix", "vhdx", "iso", "vmdk"] {
            assert!(is_package_ext(e), "{e} should be a package");
        }
        assert!(!is_package_ext("txt"));
    }
}
