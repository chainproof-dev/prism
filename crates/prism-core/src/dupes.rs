//! Duplicates pipeline core (docs/12 § 4): grouping → partial fingerprint
//! (XXH128 of first+last 64 KiB) → full BLAKE3 → hard-link collapse.
//! Byte-identical guarantee: the full hash is the final arbiter; partial
//! fingerprints only prune. I/O honors the caller's cap.

use std::collections::HashMap;

use prism_types::types_list::{DuplicateGroup, DuplicateMember};

use crate::arena::{Arena, NodeId, kind};

/// Partial-fingerprint window per side (docs/12 § 4: first+last 64 KiB).
pub const FINGERPRINT_WINDOW: u64 = 64 * 1024;

/// Pipeline step 1: group file nodes by (size) — arena query, O(n).
pub fn group_by_size(arena: &Arena) -> HashMap<u64, Vec<NodeId>> {
    let mut groups: HashMap<u64, Vec<NodeId>> = HashMap::new();
    for i in 0..arena.len() {
        let n = i as NodeId;
        if arena.kind(n) == kind::FILE {
            groups
                .entry(arena.allocated(n as usize))
                .or_default()
                .push(n);
        }
    }
    groups.retain(|size, v| *size > 0 && v.len() > 1);
    groups
}

/// Partial fingerprint of a file: XXH128 over first+last 64 KiB + length.
/// Returns `None` on read failure (the member is dropped from candidacy —
/// surfaced by the caller as a per-file error, never silent *success*).
pub fn partial_fingerprint(path: &std::path::Path, len: u64) -> Result<[u8; 16], String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let window = FINGERPRINT_WINDOW.min(len);
    let mut buf = vec![0u8; window as usize];
    f.read_exact(&mut buf).map_err(|e| e.to_string())?;
    if len > FINGERPRINT_WINDOW * 2 {
        f.seek(SeekFrom::End(-(window as i64)))
            .map_err(|e| e.to_string())?;
        let mut tail = vec![0u8; window as usize];
        f.read_exact(&mut tail).map_err(|e| e.to_string())?;
        buf.extend_from_slice(&tail);
    }
    buf.extend_from_slice(&len.to_le_bytes());
    Ok(xxhash_rust::xxh3::xxh3_128(&buf).to_le_bytes())
}

/// Full BLAKE3 hash of a file (final arbiter).
pub fn full_hash(path: &std::path::Path) -> Result<[u8; 32], String> {
    let mut hasher = blake3::Hasher::new();
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 256 * 1024];
    use std::io::Read;
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(*hasher.finalize().as_bytes())
}

/// Build a `DuplicateGroup` DTO from confirmed-identical nodes (first = kept).
pub fn group_from_nodes(
    arena: &Arena,
    root_path: &str,
    nodes: &[NodeId],
    hardlinked_to_kept: &[bool],
) -> DuplicateGroup {
    let bytes = arena.allocated(nodes[0] as usize);
    let members: Vec<DuplicateMember> = nodes
        .iter()
        .zip(hardlinked_to_kept.iter().chain(std::iter::repeat(&false)))
        .enumerate()
        .map(|(i, (&n, &hl))| DuplicateMember {
            node_id: n,
            path: crate::ipc::node_path(arena, n, root_path),
            bytes,
            mtime: if arena.mtime(n) > 0 {
                Some(crate::scanner::filetime_ticks_to_unix_ms(arena.mtime(n)))
            } else {
                None
            },
            kept: i == 0,
            partial_hashed: true,
            full_hashed: true,
            hardlink_of_kept: hl,
        })
        .collect();
    let reclaimable = bytes.saturating_mul((members.len().saturating_sub(1)) as u64);
    DuplicateGroup {
        group_id: 0,
        members,
        reclaimable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::NodeInput;

    #[test]
    fn groups_by_size() {
        let mut a = Arena::with_capacity(16);
        let root = a.push(NodeInput {
            parent: 0,
            name_utf16: &"r".encode_utf16().collect::<Vec<_>>(),
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
        for (i, sz) in [100u64, 100, 50, 100].iter().enumerate() {
            let n = a.push(NodeInput {
                parent: root,
                name_utf16: &format!("f{i}").encode_utf16().collect::<Vec<_>>(),
                logical: *sz,
                allocated: *sz,
                files: 1,
                folders: 0,
                mtime: 0,
                kind: kind::FILE,
                category: 0,
                ext_id: 0,
                attr_flags: 0,
                link_to: 0,
                err_code: 0,
            });
            a.attach(root, n);
        }
        a.finalize_children();
        let groups = group_by_size(&a);
        assert_eq!(groups.len(), 1, "only the 100-byte trio groups");
        assert_eq!(groups[&100].len(), 3);
    }

    #[test]
    fn fingerprint_same_content() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        let p = dir.path().join("a.bin");
        std::fs::write(&p, vec![7u8; 200_000]).unwrap_or_else(|e| panic!("{e}"));
        let f1 = partial_fingerprint(&p, 200_000).unwrap_or_else(|e| panic!("{e}"));
        let f2 = partial_fingerprint(&p, 200_000).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(f1, f2);
        let full = full_hash(&p).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(full.len(), 32);
    }
}

// ---------------------------------------------------------------------------
// Pipeline runner (PRISM-HG-030): cancellable, progress-emitting, honest
// per-phase events (docs/05 § 4). Runs on a worker thread owned by the IPC
// layer; `run` is the pure-ish core given a sink + cancel flag.
// ---------------------------------------------------------------------------

use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

use prism_types::events::{DupesProgress, EngineEvent};
use prism_types::types_list::DupesPhase;

use crate::ipc::EngineEventSink;

/// Full dupes run result.
pub struct DupesRun {
    /// Confirmed byte-identical groups (largest reclaimable first).
    pub groups: Vec<DuplicateGroup>,
    /// Total reclaimable = Σ (n−1)×size.
    pub reclaimable: u64,
    /// Files whose read failed during hashing (per-file errors, fail-loud).
    pub errors: Vec<(String, String)>,
}

/// Run the full pipeline over a completed scan's arena.
///
/// Phases (docs/12 § 4): size grouping → (size, ext) filter → partial
/// fingerprint (XXH128 first+last 64 KiB) → full BLAKE3 → hard-link collapse.
/// The full hash is the final arbiter; partials only prune.
pub fn run(
    arena: &Arena,
    root_path: &str,
    min_bytes: u64,
    sink: &EngineEventSink,
    cancel: &AtomicBool,
) -> DupesRun {
    let mut errors: Vec<(String, String)> = Vec::new();
    let mut groups: Vec<DuplicateGroup> = Vec::new();

    // Phase 1 — group by size (candidates ≥ min and non-solo).
    let _ = sink.emit(EngineEvent::DupesProgress {
        dupes: DupesProgress {
            phase: DupesPhase::GroupingSizes,
            groups_found: 0,
            hashed_bytes: 0,
        },
    });
    let by_size = group_by_size(arena);
    let mut candidates: Vec<(u64, Vec<NodeId>)> = by_size
        .into_iter()
        .filter(|(size, v)| *size >= min_bytes.max(1) && v.len() > 1)
        .collect();
    candidates.sort_by_key(|c| std::cmp::Reverse(c.0));

    if cancel.load(AtomicOrdering::Relaxed) {
        return DupesRun {
            groups,
            reclaimable: 0,
            errors,
        };
    }

    // Phase 2 — partial fingerprints (parallel over candidates; reads only
    // candidate files — docs/12 § 4 I/O honesty).
    let _ = sink.emit(EngineEvent::DupesProgress {
        dupes: DupesProgress {
            phase: DupesPhase::GroupingExt,
            groups_found: candidates.len() as u64,
            hashed_bytes: 0,
        },
    });
    let mut hashed_bytes: u64 = 0;
    let mut fp_groups: Vec<(u64, Vec<NodeId>)> = Vec::new(); // (size, survivors)
    for (size, nodes) in &candidates {
        if cancel.load(AtomicOrdering::Relaxed) {
            return DupesRun {
                groups,
                reclaimable: 0,
                errors,
            };
        }
        // Fingerprint each member; group by fingerprint.
        let mut by_fp: std::collections::HashMap<[u8; 16], Vec<NodeId>> =
            std::collections::HashMap::new();
        for &n in nodes {
            let path = crate::ipc::node_path(arena, n, root_path);
            match partial_fingerprint(std::path::Path::new(&path), *size) {
                Ok(fp) => {
                    hashed_bytes += FINGERPRINT_WINDOW.min(*size);
                    by_fp.entry(fp).or_default().push(n);
                }
                Err(e) => errors.push((path, e)),
            }
        }
        for (_, members) in by_fp {
            if members.len() > 1 {
                fp_groups.push((*size, members));
            }
        }
    }

    // Phase 3 — full BLAKE3 (final arbiter).
    let _ = sink.emit(EngineEvent::DupesProgress {
        dupes: DupesProgress {
            phase: DupesPhase::FullHashing,
            groups_found: fp_groups.len() as u64,
            hashed_bytes,
        },
    });
    let mut confirmed: Vec<(u64, Vec<NodeId>)> = Vec::new();
    for (size, nodes) in fp_groups {
        if cancel.load(AtomicOrdering::Relaxed) {
            return DupesRun {
                groups,
                reclaimable: 0,
                errors,
            };
        }
        let mut by_hash: std::collections::HashMap<[u8; 32], Vec<NodeId>> =
            std::collections::HashMap::new();
        for n in nodes {
            let path = crate::ipc::node_path(arena, n, root_path);
            match full_hash(std::path::Path::new(&path)) {
                Ok(h) => {
                    hashed_bytes += size;
                    by_hash.entry(h).or_default().push(n);
                }
                Err(e) => errors.push((path, e)),
            }
        }
        for (_, members) in by_hash {
            if members.len() > 1 {
                confirmed.push((size, members));
            }
        }
    }

    // Phase 4 — build groups with hard-link collapse + kept-first ordering.
    let mut reclaimable: u64 = 0;
    for (gid, (size, mut members)) in confirmed.into_iter().enumerate() {
        // Oldest mtime first = "kept" (least destructive default).
        members.sort_by_key(|&n| arena.mtime(n));
        let kept_link = arena.link_to(members[0]);
        let hardlinked: Vec<bool> = members
            .iter()
            .map(|&n| kept_link != 0 && arena.link_to(n) == kept_link)
            .collect();
        // Members hardlinked to the kept instance are not waste — collapse.
        let waste_members: Vec<usize> = (0..members.len()).filter(|&i| !hardlinked[i]).collect();
        let n_waste = waste_members.len().saturating_sub(1).max(
            // If kept is hardlinked with others, waste = non-linked members − 1.
            members
                .len()
                .saturating_sub(hardlinked.iter().filter(|&&h| h).count() + 1),
        );
        if members.len() < 2 {
            continue;
        }
        let g = group_from_nodes(arena, root_path, &members, &hardlinked);
        let g = DuplicateGroup {
            group_id: gid as u32 + 1,
            members: g
                .members
                .into_iter()
                .zip(hardlinked.iter())
                .map(|(mut m, &hl)| {
                    if hl {
                        m.hardlink_of_kept = true;
                        m.kept = true; // hard links share the same bytes — not waste
                    }
                    m
                })
                .collect(),
            reclaimable: size.saturating_mul(n_waste as u64),
        };
        reclaimable += g.reclaimable;
        groups.push(g);
    }
    groups.sort_by_key(|g| std::cmp::Reverse(g.reclaimable));

    let _ = sink.emit(EngineEvent::DupesProgress {
        dupes: DupesProgress {
            phase: DupesPhase::Done,
            groups_found: groups.len() as u64,
            hashed_bytes,
        },
    });

    DupesRun {
        groups,
        reclaimable,
        errors,
    }
}

/// Convenience: stage all duplicate extras (non-kept members) into the
/// cleanup queue (guarded by the ledger confirm in the UI, not here).
pub fn extras_of(groups: &[DuplicateGroup]) -> Vec<u32> {
    groups
        .iter()
        .flat_map(|g| {
            g.members
                .iter()
                .filter(|m| !m.kept && !m.hardlink_of_kept)
                .map(|m| m.node_id)
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(test)]
mod run_tests {
    use super::*;
    use crate::arena::NodeInput;
    use crate::arena::kind;

    fn arena_with_dupes(dir: &std::path::Path) -> Arena {
        let mut a = Arena::with_capacity(16);
        let root = a.push(NodeInput {
            parent: 0,
            name_utf16: &dir
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .encode_utf16()
                .collect::<Vec<_>>(),
            logical: 0,
            allocated: 0,
            files: 3,
            folders: 0,
            mtime: 0,
            kind: kind::ROOT,
            category: 0,
            ext_id: 0,
            attr_flags: 0,
            link_to: 0,
            err_code: 0,
        });
        let mk = |a: &mut Arena, name: &str, bytes: u64, mtime: i64| {
            let n = a.push(NodeInput {
                parent: root,
                name_utf16: &name.encode_utf16().collect::<Vec<_>>(),
                logical: bytes,
                allocated: bytes,
                files: 1,
                folders: 0,
                mtime,
                kind: kind::FILE,
                category: 0,
                ext_id: 0,
                attr_flags: 0,
                link_to: 0,
                err_code: 0,
            });
            a.attach(root, n);
        };
        mk(&mut a, "a.bin", 120_000, 1_000);
        mk(&mut a, "b.bin", 120_000, 2_000); // identical content → dupe of a
        mk(&mut a, "c.bin", 120_000, 3_000); // identical content → dupe of a
        mk(&mut a, "d.bin", 120_000, 4_000); // DIFFERENT content → survives only if hash differs
        mk(&mut a, "e.bin", 10, 5_000); // below min
        a.finalize_children();
        a
    }

    #[test]
    fn pipeline_finds_exact_groups_only() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(dir.path().join("a.bin"), vec![1u8; 120_000])
            .unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(dir.path().join("b.bin"), vec![1u8; 120_000])
            .unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(dir.path().join("c.bin"), vec![1u8; 120_000])
            .unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(dir.path().join("d.bin"), vec![9u8; 120_000])
            .unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(dir.path().join("e.bin"), vec![3u8; 10]).unwrap_or_else(|e| panic!("{e}"));

        let arena = arena_with_dupes(dir.path());
        let root = dir.path().to_string_lossy().into_owned();
        let cancel = AtomicBool::new(false);
        let run = run(&arena, &root, 1024, &EngineEventSink::for_tests(), &cancel);

        assert_eq!(run.groups.len(), 1, "only the a/b/c trio is byte-identical");
        let g = &run.groups[0];
        assert_eq!(g.members.len(), 3);
        assert_eq!(g.reclaimable, 2 * 120_000, "two extras reclaimable");
        assert!(g.members.iter().filter(|m| m.kept).count() >= 1);
        assert!(run.errors.is_empty());
    }

    #[test]
    fn min_size_filter_honored() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(dir.path().join("a.bin"), vec![1u8; 64]).unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(dir.path().join("b.bin"), vec![1u8; 64]).unwrap_or_else(|e| panic!("{e}"));
        let arena = arena_with_dupes(dir.path());
        let root = dir.path().to_string_lossy().into_owned();
        let cancel = AtomicBool::new(false);
        let run = run(
            &arena,
            &root,
            1024 * 1024,
            &EngineEventSink::for_tests(),
            &cancel,
        );
        assert!(run.groups.is_empty(), "min size filters everything");
    }

    #[test]
    fn extras_of_excludes_kept() {
        let g = DuplicateGroup {
            group_id: 1,
            members: vec![
                DuplicateMember {
                    node_id: 1,
                    path: "a".into(),
                    bytes: 5,
                    mtime: None,
                    kept: true,
                    partial_hashed: true,
                    full_hashed: true,
                    hardlink_of_kept: false,
                },
                DuplicateMember {
                    node_id: 2,
                    path: "b".into(),
                    bytes: 5,
                    mtime: None,
                    kept: false,
                    partial_hashed: true,
                    full_hashed: true,
                    hardlink_of_kept: false,
                },
            ],
            reclaimable: 5,
        };
        assert_eq!(extras_of(&[g]), vec![2]);
    }
}
