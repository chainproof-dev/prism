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
