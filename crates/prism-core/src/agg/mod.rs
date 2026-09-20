//! # Aggregation & classification (docs/06 § 4, docs/07 § 2.1)
//!
//! Pure functions over the frozen arena → deterministic, testable. Includes
//! the fixed category table (~40 curated categories → 12 anchor hues per
//! docs/08 § 4.1) and the per-scan interned extension table.

use std::collections::BTreeMap;

use prism_types::ids::{CategoryId, NodeId};

use crate::arena::{Arena, kind};

/// Fixed category ids (stable across releases — persisted in type overrides).
pub mod categories {
    use prism_types::ids::CategoryId;
    /// Video files.
    pub const VIDEO: CategoryId = 1;
    /// Audio files.
    pub const AUDIO: CategoryId = 2;
    /// Images and photos.
    pub const IMAGES: CategoryId = 3;
    /// Text documents.
    pub const DOCUMENTS: CategoryId = 4;
    /// Spreadsheets.
    pub const SPREADSHEETS: CategoryId = 5;
    /// Presentations.
    pub const PRESENTATIONS: CategoryId = 6;
    /// E-books.
    pub const EBOOKS: CategoryId = 7;
    /// Compressed archives.
    pub const ARCHIVES: CategoryId = 8;
    /// Disk images (iso/wim).
    pub const DISK_IMAGES: CategoryId = 9;
    /// Backup files.
    pub const BACKUPS: CategoryId = 10;
    /// Installers and scripts.
    pub const INSTALLERS: CategoryId = 11;
    /// Software packages.
    pub const PACKAGES: CategoryId = 12;
    /// Source code.
    pub const SOURCE_CODE: CategoryId = 13;
    /// Compiled build outputs.
    pub const BUILD_ARTIFACTS: CategoryId = 14;
    /// Package manager metadata.
    pub const PACKAGE_MANAGERS: CategoryId = 15;
    /// Version control data.
    pub const VERSION_CONTROL: CategoryId = 16;
    /// Database files.
    pub const DATABASES: CategoryId = 17;
    /// Virtual machine disks.
    pub const VIRTUAL_MACHINES: CategoryId = 18;
    /// Container images.
    pub const CONTAINER_IMAGES: CategoryId = 19;
    /// Game executables/data.
    pub const GAMES: CategoryId = 20;
    /// Game assets and models.
    pub const GAME_ASSETS: CategoryId = 21;
    /// Shader caches.
    pub const SHADER_CACHES: CategoryId = 22;
    /// System files.
    pub const SYSTEM: CategoryId = 23;
    /// Driver files.
    pub const DRIVERS: CategoryId = 24;
    /// Fonts.
    pub const FONTS: CategoryId = 25;
    /// Certificates and keys.
    pub const CERTIFICATES: CategoryId = 26;
    /// Mail stores.
    pub const MAIL_STORES: CategoryId = 27;
    /// Caches.
    pub const CACHES: CategoryId = 28;
    /// Log files.
    pub const LOGS: CategoryId = 29;
    /// Crash dumps.
    pub const CRASH_DUMPS: CategoryId = 30;
    /// Temporary files.
    pub const TEMP: CategoryId = 31;
    /// Binary blobs.
    pub const BINARY: CategoryId = 32;
    /// Configuration files.
    pub const CONFIG: CategoryId = 33;
    /// Unclassified files.
    pub const OTHER: CategoryId = 34;
}

/// One category descriptor.
pub struct Category {
    /// Stable id (the const above).
    pub id: CategoryId,
    /// Display name.
    pub name: &'static str,
    /// Anchor hue (docs/08 § 4.1, degrees in OKLCH space).
    pub hue: f32,
    /// Extension membership (lowercase, no dot).
    pub exts: &'static [&'static str],
}

/// The fixed shipped category table. "Adjacent categories never share a hue"
/// is satisfied by grouping (docs/08 § 4.1 table).
pub const CATEGORY_TABLE: &[Category] = &[
    Category {
        id: categories::VIDEO,
        name: "Video",
        hue: 40.0,
        exts: &[
            "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "3gp", "ts",
        ],
    },
    Category {
        id: categories::AUDIO,
        name: "Audio",
        hue: 350.0,
        exts: &[
            "mp3", "flac", "aac", "ogg", "opus", "wav", "wma", "aiff", "m4a", "mid",
        ],
    },
    Category {
        id: categories::IMAGES,
        name: "Images",
        hue: 15.0,
        exts: &[
            "jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "tif", "heic", "avif", "svg",
            "ico", "psd", "ai", "raw", "cr2", "nef", "arw", "dng",
        ],
    },
    Category {
        id: categories::DOCUMENTS,
        name: "Documents",
        hue: 265.0,
        exts: &["doc", "docx", "odt", "rtf", "txt", "md", "pdf"],
    },
    Category {
        id: categories::SPREADSHEETS,
        name: "Spreadsheets",
        hue: 265.0,
        exts: &["xls", "xlsx", "ods", "csv", "tsv"],
    },
    Category {
        id: categories::PRESENTATIONS,
        name: "Presentations",
        hue: 265.0,
        exts: &["ppt", "pptx", "odp", "key"],
    },
    Category {
        id: categories::EBOOKS,
        name: "Ebooks",
        hue: 265.0,
        exts: &["epub", "mobi", "azw3", "fb2"],
    },
    Category {
        id: categories::ARCHIVES,
        name: "Archives",
        hue: 75.0,
        exts: &[
            "zip", "rar", "7z", "tar", "gz", "bz2", "xz", "zst", "lz4", "cab",
        ],
    },
    Category {
        id: categories::DISK_IMAGES,
        name: "Disk images",
        hue: 75.0,
        exts: &["iso", "img", "wim", "vhd", "vhdx", "dmg"],
    },
    Category {
        id: categories::BACKUPS,
        name: "Backups",
        hue: 75.0,
        exts: &["bak", "old", "orig", "tmp-backup", "abf", "bk"],
    },
    Category {
        id: categories::INSTALLERS,
        name: "Installers",
        hue: 40.0,
        exts: &[
            "exe",
            "msi",
            "msix",
            "appx",
            "appxbundle",
            "msixbundle",
            "bat",
            "cmd",
            "ps1",
        ],
    },
    Category {
        id: categories::PACKAGES,
        name: "Packages",
        hue: 40.0,
        exts: &[
            "deb", "rpm", "apk", "appimage", "nupkg", "whl", "gem", "crate",
        ],
    },
    Category {
        id: categories::SOURCE_CODE,
        name: "Source code",
        hue: 250.0,
        exts: &[
            "rs", "ts", "tsx", "js", "jsx", "py", "go", "c", "h", "cpp", "hpp", "cc", "cs", "java",
            "kt", "swift", "rb", "php", "lua", "vim", "el", "clj", "ex", "exs", "zig", "v",
            "scala", "hs", "ml", "fs", "fsx", "vb", "asm", "s", "sql",
        ],
    },
    Category {
        id: categories::BUILD_ARTIFACTS,
        name: "Build artifacts",
        hue: 250.0,
        exts: &[
            "o", "obj", "a", "lib", "so", "dll", "dylib", "class", "jar", "war", "pyc", "pyo",
            "elc", "wasm", "pdb",
        ],
    },
    Category {
        id: categories::PACKAGE_MANAGERS,
        name: "Package managers",
        hue: 250.0,
        exts: &["lock", "sum", "toml-cargo", "npmrc-cache"],
    },
    Category {
        id: categories::VERSION_CONTROL,
        name: "Version control",
        hue: 250.0,
        exts: &[
            "git",
            "gitattributes",
            "gitignore",
            "gitmodules",
            "pack-idx",
        ],
    },
    Category {
        id: categories::DATABASES,
        name: "Databases",
        hue: 250.0,
        exts: &[
            "db",
            "sqlite",
            "sqlite3",
            "mdb",
            "accdb",
            "dbf",
            "ibd",
            "frm",
            "mongodump",
            "bak-sql",
        ],
    },
    Category {
        id: categories::VIRTUAL_MACHINES,
        name: "Virtual machines",
        hue: 300.0,
        exts: &["vmdk", "vdi", "qcow2", "vmwarevm", "vpc", "hdd"],
    },
    Category {
        id: categories::CONTAINER_IMAGES,
        name: "Container images",
        hue: 300.0,
        exts: &["dockerfile-layer", "oci", "containerarchive"],
    },
    Category {
        id: categories::GAMES,
        name: "Games",
        hue: 135.0,
        exts: &["unity3d", "uasset", "pak", "bsp", "vpk", "wad"],
    },
    Category {
        id: categories::GAME_ASSETS,
        name: "Game assets",
        hue: 135.0,
        exts: &["fbx", "glb", "gltf", "obj3d", "dds", "hdr"],
    },
    Category {
        id: categories::SHADER_CACHES,
        name: "Shader caches",
        hue: 135.0,
        exts: &["bin-shadercache", "pipelinecache"],
    },
    Category {
        id: categories::SYSTEM,
        name: "System",
        hue: 265.0,
        exts: &["sys", "cat", "inf", "reg", "msc", "dllcache", "muicache"],
    },
    Category {
        id: categories::DRIVERS,
        name: "Drivers",
        hue: 265.0,
        exts: &["dmp-driver", "drv"],
    },
    Category {
        id: categories::FONTS,
        name: "Fonts",
        hue: 265.0,
        exts: &["ttf", "otf", "woff", "woff2", "eot"],
    },
    Category {
        id: categories::CERTIFICATES,
        name: "Certificates",
        hue: 265.0,
        exts: &["cer", "crt", "pem", "pfx", "p12", "key"],
    },
    Category {
        id: categories::MAIL_STORES,
        name: "Mail stores",
        hue: 185.0,
        exts: &["pst", "ost", "eml", "msg", "mbox"],
    },
    Category {
        id: categories::CACHES,
        name: "Caches",
        hue: 320.0,
        exts: &["cache", "cache2", "bfcache"],
    },
    Category {
        id: categories::LOGS,
        name: "Logs",
        hue: 320.0,
        exts: &["log", "log1", "log2", "out", "err", "nlog"],
    },
    Category {
        id: categories::CRASH_DUMPS,
        name: "Crash dumps",
        hue: 320.0,
        exts: &["dmp", "mdmp", "core", "hserr"],
    },
    Category {
        id: categories::TEMP,
        name: "Temporary",
        hue: 320.0,
        exts: &[
            "tmp",
            "temp",
            "~",
            "swp",
            "swo",
            "part",
            "crdownload",
            "partial",
        ],
    },
    Category {
        id: categories::BINARY,
        name: "Binaries",
        hue: 250.0,
        exts: &["bin", "dat", "image", "rom", "firmware", "efi"],
    },
    Category {
        id: categories::CONFIG,
        name: "Configuration",
        hue: 265.0,
        exts: &[
            "ini",
            "cfg",
            "conf",
            "yaml",
            "yml",
            "json",
            "xml",
            "toml",
            "plist",
            "properties",
            "env",
        ],
    },
    Category {
        id: categories::OTHER,
        name: "Other",
        hue: 265.0,
        exts: &[],
    },
];

/// Lookup category by extension (lowercase). Returns OTHER for unknown.
pub fn category_for_ext(ext: &str) -> CategoryId {
    for c in CATEGORY_TABLE {
        if c.exts.binary_search(&ext).is_ok() || c.exts.contains(&ext) {
            return c.id;
        }
    }
    categories::OTHER
}

/// Display name for a category id (fallback "Other").
pub fn category_name(id: CategoryId) -> &'static str {
    CATEGORY_TABLE
        .iter()
        .find(|c| c.id == id)
        .map(|c| c.name)
        .unwrap_or("Other")
}

/// Hue anchor for a category id (docs/08 § 4.1).
pub fn category_hue(id: CategoryId) -> f32 {
    match id {
        prism_types::ids::CATEGORY_FREE_SPACE => -1.0, // neutral recessive
        prism_types::ids::CATEGORY_UNKNOWN => 85.0,    // warning hatch
        id => CATEGORY_TABLE
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.hue)
            .unwrap_or(265.0),
    }
}

/// Interned extension table (per scan).
#[derive(Default)]
pub struct ExtensionTable {
    map: BTreeMap<String, u32>, // lowercase ext → ext_id
    display: Vec<String>,
}

impl ExtensionTable {
    /// Intern an extension ("" = none; "." = dotfile). Returns ext_id.
    pub fn intern(&mut self, ext: &str) -> prism_types::ids::ExtId {
        if let Some(id) = self.map.get(ext) {
            return *id;
        }
        let id = self.display.len() as u32;
        self.display.push(ext.to_string());
        self.map.insert(ext.to_string(), id);
        id
    }

    /// Display form for id.
    pub fn display(&self, id: u32) -> &str {
        self.display
            .get(id as usize)
            .map(String::as_str)
            .unwrap_or("")
    }

    /// All keys in id order (snapshot for iteration).
    pub fn keys(&self) -> Vec<String> {
        self.display.clone()
    }

    /// Known extension count.
    pub fn len(&self) -> usize {
        self.display.len()
    }

    /// Empty?
    pub fn is_empty(&self) -> bool {
        self.display.is_empty()
    }
}

/// Extract the extension key from a UTF-16 name: lowercase, no dot.
/// Dotfiles (`.gitignore`) → `"."`; no extension → `""`.
pub fn extension_key(name_utf16: &[u16]) -> String {
    let name = String::from_utf16_lossy(name_utf16);
    let trimmed = name.trim_end_matches('.');
    match trimmed.rfind('.') {
        Some(0) => ".".to_string(),
        Some(pos) if !trimmed.is_empty() => trimmed[pos + 1..].to_lowercase(),
        _ => String::new(),
    }
}

/// Age buckets (docs/08 § 4.3; thresholds shared with stale rules, PRISM-HG-020).
pub const AGE_BUCKET_EDGES_DAYS: [i64; 6] = [7, 30, 90, 365, 730, i64::MAX];

/// Bucket index for an mtime unix-ms (0..5). Unknown (0 mtime) → bucket 5
/// ("ancient") but flagged by caller when mtime == 0.
pub fn age_bucket(mtime_unix_ms: i64, now_ms: i64) -> usize {
    if mtime_unix_ms <= 0 {
        return 5;
    }
    // Intentional integer division: whole days is exactly the bucket grain.
    #[allow(clippy::integer_division)]
    let age_days = (now_ms - mtime_unix_ms).max(0) / 86_400_000;
    for (i, edge) in AGE_BUCKET_EDGES_DAYS.iter().enumerate() {
        if age_days < *edge {
            return i;
        }
    }
    5
}

/// Per-scan aggregate tables (computed once, post-walk; doc 06 § 4).
pub struct Aggregates {
    /// Per-extension: (files, logical, allocated).
    pub ext_stats: Vec<(u32, u64, u64)>, // indexed by ext_id
    /// Byte-weighted age histogram (6 buckets).
    pub age_histogram: [u64; 6],
    /// Top-N children per directory (leaderboards) — computed lazily by
    /// queries instead (arena children are already sorted size-desc — the
    /// "Largest Inside" list is `children(parent)[..10]`, O(1)).
    pub total_files: u64,
    /// Total folders.
    pub total_folders: u64,
    /// Total logical.
    pub total_logical: u64,
    /// Total allocated.
    pub total_allocated: u64,
}

/// Compute the aggregate tables over a frozen arena (pure, deterministic).
pub fn compute(arena: &Arena, ext_table: &ExtensionTable) -> Aggregates {
    let n = arena.len();
    let mut ext_stats = vec![(0u32, 0u64, 0u64); ext_table.len().max(1)];
    let mut age_histogram = [0u64; 6];
    let now_ms = now_unix_ms();
    let mut total_files = 0u64;
    let mut total_folders = 0u64;
    let mut total_logical = 0u64;
    let mut total_allocated = 0u64;
    for i in 0..n {
        let node = i as NodeId;
        if arena.kind(node) == kind::FILE {
            total_files += 1;
            total_logical += arena.logical(node);
            total_allocated += arena.allocated(i);
            let ext_id = arena.ext_id(node) as usize;
            if let Some(slot) = ext_stats.get_mut(ext_id) {
                slot.0 += 1;
                slot.1 += arena.logical(node);
                slot.2 += arena.allocated(i);
            }
            let mtime_ms = crate::scanner::filetime_ticks_to_unix_ms(arena.mtime(node));
            let bucket = age_bucket(mtime_ms, now_ms);
            age_histogram[bucket] += arena.allocated(i);
        } else if arena.kind(node) == kind::DIR || arena.kind(node) == kind::ROOT {
            total_folders += 1;
        }
    }
    Aggregates {
        ext_stats,
        age_histogram,
        total_files,
        total_folders,
        total_logical,
        total_allocated,
    }
}

/// Current unix ms (single call site keeps time deterministic in tests via
/// injection where needed).
pub fn now_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_keys() {
        assert_eq!(
            extension_key(&"video.mp4".encode_utf16().collect::<Vec<_>>()),
            "mp4"
        );
        assert_eq!(
            extension_key(&".gitignore".encode_utf16().collect::<Vec<_>>()),
            "."
        );
        assert_eq!(
            extension_key(&"Makefile".encode_utf16().collect::<Vec<_>>()),
            ""
        );
        assert_eq!(
            extension_key(&"ARCHIVE.ZIP".encode_utf16().collect::<Vec<_>>()),
            "zip"
        );
    }

    #[test]
    fn category_mapping() {
        assert_eq!(category_for_ext("mp4"), categories::VIDEO);
        assert_eq!(category_for_ext("zip"), categories::ARCHIVES);
        assert_eq!(category_for_ext("totally-unknown"), categories::OTHER);
    }

    #[test]
    fn age_buckets() {
        let now = 1_700_000_000_000;
        assert_eq!(age_bucket(now - 86_400_000, now), 0); // 1 day
        assert_eq!(age_bucket(now - 30 * 86_400_000, now), 2); // 30d → 30-90
        assert_eq!(age_bucket(now - 400 * 86_400_000, now), 4); // > 1y
        assert_eq!(age_bucket(0, now), 5); // unknown
    }
}
