//! Cleanup engine surface: preset rule table (docs/12 § 2) + protected-path
//! blocklist (parity-DEL-05) + staging queue. Deletion execution itself is
//! platform IFileOperation-class work (Windows, Phase 5) — staging,
//! matching and safety classification are pure and shipped now.

use prism_types::types_list::CleanupItem;

use crate::arena::{Arena, NodeId, kind};

/// One cleanup preset rule.
pub struct Preset {
    /// Stable preset id.
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// Safety class: `regenerable` | `user-content` | `warning`.
    pub safety: &'static str,
    /// Explanation (renderer-localizes by id).
    pub explanation: &'static str,
    /// Glob patterns matched against root-relative paths (own matcher).
    pub patterns: &'static [&'static str],
}

/// The shipped preset table (docs/12 § 2 — Windows-first taxonomy).
pub const PRESETS: &[Preset] = &[
    Preset {
        id: "node-modules",
        name: "JavaScript node_modules",
        safety: "regenerable",
        explanation: "Package manager dependencies — restored with an install command.",
        patterns: &["**/node_modules/**"],
    },
    Preset {
        id: "rust-target",
        name: "Rust build artifacts",
        safety: "regenerable",
        explanation: "cargo target directories — rebuilt on next build.",
        patterns: &["**/target/debug/**", "**/target/release/**"],
    },
    Preset {
        id: "dotnet-build",
        name: ".NET build outputs",
        safety: "regenerable",
        explanation: "bin/obj folders — rebuilt on next build.",
        patterns: &["**/bin/**", "**/obj/**"],
    },
    Preset {
        id: "python-caches",
        name: "Python caches",
        safety: "regenerable",
        explanation: "__pycache__ and friends — regenerated on demand.",
        patterns: &[
            "**/__pycache__/**",
            "**/pytest_cache/**",
            "**/.mypy_cache/**",
            "**/.ruff_cache/**",
            "**/*.pyc",
        ],
    },
    Preset {
        id: "gradle",
        name: "Gradle / Android caches",
        safety: "regenerable",
        explanation: "Gradle cache directories — re-downloaded when needed.",
        patterns: &["**/.gradle/caches/**", "**/.gradle/**"],
    },
    Preset {
        id: "xcode-on-windows",
        name: "iOS build folders (cross builds)",
        safety: "regenerable",
        explanation: "Pods/DerivedData from cross-platform builds.",
        patterns: &["**/ios/Pods/**", "**/ios/DerivedData/**"],
    },
    Preset {
        id: "package-caches",
        name: "Package manager caches",
        safety: "regenerable",
        explanation: "npm/pip/cargo/NuGet caches.",
        patterns: &[
            "**/.npm/**",
            "**/.cache/pip/**",
            "**/.cargo/registry/cache/**",
            "**/.nuget/packages/**",
        ],
    },
    Preset {
        id: "browser-caches",
        name: "Browser caches",
        safety: "regenerable",
        explanation: "Chromium/Edge/Firefox cache subpaths (never cookies or logins).",
        patterns: &[
            "**/Cache/Cache_Data/**",
            "**/Code Cache/**",
            "**/GPUCache/**",
        ],
    },
    Preset {
        id: "windows-temp",
        name: "Windows temp files",
        safety: "regenerable",
        explanation: "User and system temp directories.",
        patterns: &["Temp/**"],
    },
    Preset {
        id: "crash-dumps",
        name: "Crash dumps and logs",
        safety: "regenerable",
        explanation: "Dump files and dev-tool logs.",
        patterns: &["**/*.dmp", "**/LocalCrashDumps/**"],
    },
    Preset {
        id: "downloads-installers",
        name: "Installers in Downloads",
        safety: "user-content",
        explanation: "Old installers and archives — review before removal.",
        patterns: &[
            "Downloads/**/*.exe",
            "Downloads/**/*.msi",
            "Downloads/**/*.zip",
            "Downloads/**/*.iso",
        ],
    },
    Preset {
        id: "vm-disks",
        name: "Virtual machine disks",
        safety: "warning",
        explanation: "VM disk images — may contain irreplaceable data.",
        patterns: &[
            "**/*.vhd",
            "**/*.vhdx",
            "**/*.vmdk",
            "**/*.vdi",
            "**/*.qcow2",
        ],
    },
];

/// Hard blocklist roots (parity-DEL-05): never stageable, even with
/// acknowledgement — deleting these breaks Windows itself.
pub const BLOCKED_ROOTS: &[&str] = &[
    "C:\\Windows",
    "C:\\Program Files",
    "C:\\Program Files (x86)",
    "C:\\ProgramData",
    "C:\\System Volume Information",
    "C:\\WinSxS",
    "C:\\",
];

/// Is `path` inside a blocked root? (Case-insensitive prefix match with a
/// separator boundary so `C:\\Program Files\\x` blocks but a hypothetical
/// `C:\\Program FilesBackup` does not.)
pub fn is_blocked_path(path: &str) -> bool {
    let p = path.replace('/', "\\").to_lowercase();
    for root in BLOCKED_ROOTS {
        let r = root.to_lowercase();
        if p == r {
            return true;
        }
        let prefix = format!("{r}\\");
        if p.starts_with(&prefix) {
            return true;
        }
    }
    false
}

/// The staging queue (ordered map by node id; ledger source of truth).
#[derive(Default)]
pub struct StagingQueue {
    items: Vec<CleanupItem>,
}

impl StagingQueue {
    /// Empty queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Stage nodes from a completed scan. Blocked paths are rejected loudly
    /// with the reason (fail-loud, A2) — the caller surfaces it in the ledger.
    pub fn stage(
        &mut self,
        arena: &Arena,
        root_path: &str,
        node_ids: &[NodeId],
        source: prism_types::types_list::CleanupSource,
    ) -> Result<(), Vec<(NodeId, String)>> {
        let mut rejected = Vec::new();
        for &n in node_ids {
            if (n as usize) >= arena.len() {
                continue;
            }
            let path = crate::ipc::node_path(arena, n, root_path);
            if is_blocked_path(&path) {
                rejected.push((n, "protected system location".to_string()));
                continue;
            }
            if self.items.iter().any(|i| i.node_id == n) {
                continue; // already staged (idempotent)
            }
            self.items.push(CleanupItem {
                node_id: n,
                path,
                bytes: arena.allocated(n as usize),
                source,
            });
        }
        if rejected.is_empty() {
            Ok(())
        } else {
            Err(rejected)
        }
    }

    /// Unstage by node ids (empty = clear all).
    pub fn unstage(&mut self, node_ids: &[NodeId]) {
        if node_ids.is_empty() {
            self.items.clear();
            return;
        }
        self.items.retain(|i| !node_ids.contains(&i.node_id));
    }

    /// Staged items.
    pub fn items(&self) -> &[CleanupItem] {
        &self.items
    }

    /// Totals.
    pub fn totals(&self) -> (u32, u64) {
        (
            self.items.len() as u32,
            self.items.iter().map(|i| i.bytes).sum(),
        )
    }
}

/// Preset scan against a frozen arena: match directories by sticky-name rule
/// (the coordinator's exclusion matcher semantics — `**/name/**`).
pub fn preset_hits(arena: &Arena, preset: &Preset) -> Vec<NodeId> {
    let mut names: Vec<String> = Vec::new();
    for p in preset.patterns {
        let normalized = p.replace('\\', "/");
        for seg in normalized.split('/') {
            if seg != "**" && !seg.is_empty() && !seg.contains(['*', '?', '[']) {
                if !names.iter().any(|n| n.eq_ignore_ascii_case(seg)) {
                    names.push(seg.to_string());
                }
            }
        }
    }
    let mut hits = Vec::new();
    for i in 0..arena.len() {
        let n = i as NodeId;
        if arena.kind(n) == kind::DIR
            && names
                .iter()
                .any(|nm| arena.name_str(n).eq_ignore_ascii_case(nm))
        {
            hits.push(n);
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_roots() {
        assert!(is_blocked_path("C:\\Windows\\System32\\drivers"));
        assert!(is_blocked_path("c:\\program files\\app"));
        assert!(is_blocked_path("C:\\"));
        assert!(!is_blocked_path("C:\\Users\\me"));
        assert!(!is_blocked_path("C:\\Program FilesBackup"));
    }

    #[test]
    fn staging_rejects_blocked() {
        let mut arena = Arena::with_capacity(8);
        let root = arena.push(crate::arena::NodeInput {
            parent: 0,
            name_utf16: &"C:".encode_utf16().collect::<Vec<_>>(),
            logical: 0,
            allocated: 0,
            files: 0,
            folders: 1,
            mtime: 0,
            kind: kind::ROOT,
            category: 0,
            ext_id: 0,
            attr_flags: 0,
            link_to: 0,
            err_code: 0,
        });
        let sys = arena.push(crate::arena::NodeInput {
            parent: root,
            name_utf16: &"Windows".encode_utf16().collect::<Vec<_>>(),
            logical: 100,
            allocated: 100,
            files: 1,
            folders: 0,
            mtime: 0,
            kind: kind::DIR,
            category: 0,
            ext_id: 0,
            attr_flags: 0,
            link_to: 0,
            err_code: 0,
        });
        arena.attach(root, sys);
        arena.finalize_children();
        assert!(is_blocked_path(&crate::ipc::node_path(&arena, sys, "C:\\")));
        let mut q = StagingQueue::new();
        // root path C:\ + name Windows → C:\Windows → blocked
        let err = q.stage(
            &arena,
            "C:\\",
            &[sys],
            prism_types::types_list::CleanupSource::Manual,
        );
        assert!(err.is_err());
        assert!(q.items().is_empty());
    }
}
