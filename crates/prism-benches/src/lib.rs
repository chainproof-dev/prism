//! Synthetic FIX-L fixture builders (docs/16 § 2): deterministic arenas at
//! scale for agg/viz/tree benches without disk cost, plus an on-disk tree
//! generator for the real scan-pipeline bench.

use prism_core::agg::ExtensionTable;
use prism_core::arena::{Arena, NodeInput, kind};

/// Build a balanced synthetic arena: `fanout` children per folder, `depth`
/// levels; leaf level is files. Deterministic names/extensions/sizes.
/// Returns (arena, root, ext_table) with the arena finalized.
pub fn synth_arena(nodes_hint: usize) -> (Arena, u32, ExtensionTable) {
    // Pick a shape that lands near nodes_hint: fanout 8, depth 6 → ~300k.
    let fanout: usize = 8;
    let depth: usize = 6;
    let mut arena = Arena::with_capacity(nodes_hint + 16);
    let mut ext_table = ExtensionTable::default();
    let root = arena.push(NodeInput {
        parent: 0,
        name_utf16: &"S:".encode_utf16().collect::<Vec<_>>(),
        logical: 0,
        allocated: 0,
        files: 0,
        folders: 0,
        mtime: 0,
        kind: kind::ROOT,
        category: 0,
        ext_id: 0,
        attr_flags: 0,
        link_to: 0,
        err_code: 0,
    });

    // Deterministic pseudo-random-ish sizes (LCG) so layout work is realistic.
    let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let exts: [&str; 8] = ["rs", "js", "png", "mp4", "zip", "iso", "pdf", "dll"];

    // Recursive build with an explicit stack (folder nodes), files at depth.
    struct Frame {
        node: u32,
        depth: usize,
    }
    let mut stack = vec![Frame {
        node: root,
        depth: 0,
    }];
    let mut built: usize = 1;
    while let Some(fr) = stack.pop() {
        if built >= nodes_hint {
            break;
        }
        for c in 0..fanout {
            if built >= nodes_hint {
                break;
            }
            let is_leaf = fr.depth + 1 >= depth;
            let name: Vec<u16> = if is_leaf {
                format!("f{c}-{}.{}", next() % 100_000, exts[c % exts.len()])
                    .encode_utf16()
                    .collect()
            } else {
                format!("d{c}").encode_utf16().collect()
            };
            let size = if is_leaf { next() % 4_000_000 } else { 0 };
            let ext_id = if is_leaf {
                ext_table.intern(exts[c % exts.len()])
            } else {
                0
            };
            let n = arena.push(NodeInput {
                parent: fr.node,
                name_utf16: &name,
                logical: size,
                allocated: size,
                files: if is_leaf { 1 } else { 0 },
                folders: if is_leaf { 0 } else { 1 },
                mtime: 0,
                kind: if is_leaf { kind::FILE } else { kind::DIR },
                category: 0,
                ext_id,
                attr_flags: 0,
                link_to: 0,
                err_code: 0,
            });
            arena.attach(fr.node, n);
            built += 1;
            if !is_leaf {
                stack.push(Frame {
                    node: n,
                    depth: fr.depth + 1,
                });
            }
        }
    }
    arena.finalize_children();
    // Roll file sizes up into folder subtree metrics (the completion cascade
    // in a real scan): nodes were pushed parent-first, so a reverse pass
    // adds every node's own bytes into its parent.
    for node in (0..arena.len() as u32).rev() {
        let parent = arena.parent(node);
        if parent != node {
            let own_logical = arena.logical(node);
            let own_alloc = arena.allocated(node as usize);
            let (f, d) = if arena.kind(node) == kind::FILE {
                (1u32, 0u32)
            } else {
                (0u32, 1u32)
            };
            arena.add_metrics(parent, own_logical, own_alloc, f, d);
        }
    }
    (arena, root, ext_table)
}

/// Generate a real on-disk tree (tempdir, caller keeps the TempDir alive).
/// `files` small files across `files/64` directories. Deterministic sizes.
pub fn disk_tree(files: usize) -> tempfile::TempDir {
    // Bench-only fixture builder: panics are acceptable here (criterion
    // harness context, not the engine).
    #![allow(clippy::expect_used)]
    let dir = tempfile::tempdir().expect("tempdir");
    let per_dir = 64usize;
    let dirs = files.div_ceil(per_dir);
    let mut written = 0usize;
    for d in 0..dirs {
        let sub = dir.path().join(format!("d{d:04}"));
        std::fs::create_dir_all(&sub).expect("mkdir");
        for f in 0..per_dir {
            if written >= files {
                break;
            }
            let body = vec![(written % 251) as u8; 128];
            std::fs::write(sub.join(format!("f{f:03}.dat")), body).expect("write");
            written += 1;
        }
    }
    dir
}
