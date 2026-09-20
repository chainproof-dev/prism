//! # Arena — the columnar node store (docs/07-DATA-MODEL.md § 2)
//!
//! Structural SoA arrays, interned UTF-16 names, CSR children. Single-writer
//! (the scan coordinator); `NodeId` = row index. ~84 B/node + names.
//! Invariants (PRISM-DM-030) are property-tested in `tests/arena_props.rs`.

use std::fmt::Write as _;

pub use prism_types::ids::NodeId;
use prism_types::ids::{CategoryId, ExtId};

/// Internal entry kinds (packed `u8` in the arena; wire enum in prism-types).
pub mod kind {
    /// Regular file.
    pub const FILE: u8 = 0;
    /// Directory.
    pub const DIR: u8 = 1;
    /// Reparse point (junction/symlink) — not descended by default.
    pub const REPARSE: u8 = 2;
    /// Mounted volume.
    pub const MOUNT: u8 = 3;
    /// Secondary hard link.
    pub const LINK: u8 = 4;
    /// Synthetic free-space node.
    pub const FREE_SPACE: u8 = 5;
    /// Synthetic unknown node.
    pub const UNKNOWN: u8 = 6;
    /// Synthetic root.
    pub const ROOT: u8 = 7;
}

/// FILE_ATTRIBUTE_* + internal flag bits (sparse/compressed/offline…).
pub mod flags {
    /// FILE_ATTRIBUTE_READONLY
    pub const READONLY: u32 = 0x0000_0001;
    /// FILE_ATTRIBUTE_HIDDEN
    pub const HIDDEN: u32 = 0x0000_0002;
    /// FILE_ATTRIBUTE_SYSTEM
    pub const SYSTEM: u32 = 0x0000_0004;
    /// FILE_ATTRIBUTE_DIRECTORY
    pub const DIRECTORY: u32 = 0x0000_0010;
    /// FILE_ATTRIBUTE_ARCHIVE
    pub const ARCHIVE: u32 = 0x0000_0020;
    /// FILE_ATTRIBUTE_DEVICE
    pub const DEVICE: u32 = 0x0000_0040;
    /// FILE_ATTRIBUTE_NORMAL
    pub const NORMAL: u32 = 0x0000_0080;
    /// FILE_ATTRIBUTE_TEMPORARY
    pub const TEMPORARY: u32 = 0x0000_0100;
    /// FILE_ATTRIBUTE_SPARSE_FILE
    pub const SPARSE: u32 = 0x0000_0200;
    /// FILE_ATTRIBUTE_REPARSE_POINT
    pub const REPARSE: u32 = 0x0000_0400;
    /// FILE_ATTRIBUTE_COMPRESSED
    pub const COMPRESSED: u32 = 0x0000_0800;
    /// FILE_ATTRIBUTE_OFFLINE
    pub const OFFLINE: u32 = 0x0000_1000;
    /// FILE_ATTRIBUTE_NOT_CONTENT_INDEXED
    pub const NOT_CONTENT_INDEXED: u32 = 0x0000_2000;
    /// FILE_ATTRIBUTE_ENCRYPTED
    pub const ENCRYPTED: u32 = 0x0000_4000;
    /// Internal: package container collapsed (treatPackagesAsNodes).
    pub const INTERNAL_PACKAGE: u32 = 0x4000_0000;
    /// Internal: hard-link secondary (bytes owned elsewhere).
    pub const INTERNAL_LINK2: u32 = 0x2000_0000;
}

/// Hard ceiling protecting the 4 GB RSS budget (docs/07 § 1, docs/16 § 2).
pub const ARENA_NODE_CEILING: usize = 8 * 1024 * 1024;

/// The columnar arena. All Vecs share the row index (`NodeId`).
///
/// Build protocol (single writer):
/// 1. **Walking** — `push` + `attach` append-only (flat `(parent, child)`
///    edges in discovery order; O(1) each). `children()` is unavailable until
///    finalize — the renderer builds its live view from `NodeDelta` batches
///    instead (ADR-03 windowing), so this is by design, not a limitation.
/// 2. **Finalize** — `finalize_children()` counting-sorts edges into the CSR
///    arrays (O(n)) and sorts each row deterministically (size-desc, name).
/// 3. **Queries** — `children()`/`remove_node()` operate on the CSR.
#[derive(Default)]
pub struct Arena {
    n: u32,
    parent: Vec<NodeId>,
    // flat edge list during walk; CSR after finalize
    edges: Vec<(NodeId, NodeId)>,
    finalized: bool,
    // CSR children
    child_index: Vec<NodeId>,
    child_offset: Vec<u32>, // len n+1
    // interned names (UTF-16 packed; offset table)
    names: Vec<u16>,
    name_off: Vec<u32>, // len n+1 (terminal sentinel)
    // metrics
    logical: Vec<u64>,
    allocated: Vec<u64>,
    files: Vec<u32>,
    folders: Vec<u32>,
    mtime: Vec<i64>, // FILETIME ticks; subtree max; 0 = unknown
    // classification
    kind: Vec<u8>,
    category: Vec<CategoryId>,
    ext_id: Vec<ExtId>,
    attr_flags: Vec<u32>,
    // hard links / errors
    link_to: Vec<u64>, // FileId of owner; 0 = none
    err_code: Vec<u16>,
}

impl Arena {
    /// New arena pre-sized for `capacity` nodes (avoids realloc storms).
    pub fn with_capacity(capacity: usize) -> Self {
        let cap = capacity.min(ARENA_NODE_CEILING);
        Self {
            n: 0,
            parent: Vec::with_capacity(cap),
            edges: Vec::with_capacity(cap),
            finalized: false,
            child_index: Vec::new(),
            child_offset: vec![0],
            names: Vec::new(),
            name_off: Vec::new(),
            logical: Vec::with_capacity(cap),
            allocated: Vec::with_capacity(cap),
            files: Vec::with_capacity(cap),
            folders: Vec::with_capacity(cap),
            mtime: Vec::with_capacity(cap),
            kind: Vec::with_capacity(cap),
            category: Vec::with_capacity(cap),
            ext_id: Vec::with_capacity(cap),
            attr_flags: Vec::with_capacity(cap),
            link_to: Vec::with_capacity(cap),
            err_code: Vec::with_capacity(cap),
        }
    }

    /// Node count.
    pub fn len(&self) -> usize {
        self.n as usize
    }

    /// Empty?
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// Append a node; returns its id. Children attach later via `attach`.
    pub fn push(&mut self, node: NodeInput<'_>) -> NodeId {
        debug_assert!(self.kind_is_valid(node.kind));
        let id = self.n;
        let start = self.names.len() as u32;
        self.names.extend_from_slice(node.name_utf16);
        self.name_off.push(start);
        self.parent.push(node.parent);
        self.logical.push(node.logical);
        self.allocated.push(node.allocated);
        self.files.push(node.files);
        self.folders.push(node.folders);
        self.mtime.push(node.mtime);
        self.kind.push(node.kind);
        self.category.push(node.category);
        self.ext_id.push(node.ext_id);
        self.attr_flags.push(node.attr_flags);
        self.link_to.push(node.link_to);
        self.err_code.push(node.err_code);
        self.n += 1;
        id
    }

    fn kind_is_valid(&self, k: u8) -> bool {
        matches!(
            k,
            kind::FILE
                | kind::DIR
                | kind::REPARSE
                | kind::MOUNT
                | kind::LINK
                | kind::FREE_SPACE
                | kind::UNKNOWN
                | kind::ROOT
        )
    }

    /// Attach `child` under `parent` (append-only during walk; O(1)).
    pub fn attach(&mut self, parent: NodeId, child: NodeId) {
        self.edges.push((parent, child));
    }

    /// Build the CSR arrays from the flat edge list (counting sort, O(n))
    /// then sort each row deterministically (size-desc, name-ordinal — A4).
    /// After this call, `children()` is available and `edges` is freed.
    pub fn finalize_children(&mut self) {
        if self.finalized {
            return;
        }
        // counting pass
        let n = self.n as usize;
        let mut counts = vec![0u32; n + 1];
        for &(p, _) in &self.edges {
            counts[p as usize + 1] += 1;
        }
        for i in 1..=n {
            counts[i] += counts[i - 1];
        }
        self.child_offset = counts;
        self.child_index = vec![NodeId::MAX; self.edges.len()];
        let mut cursor = self.child_offset[..n].to_vec();
        for &(p, c) in &self.edges {
            let row = p as usize;
            self.child_index[cursor[row] as usize] = c;
            cursor[row] += 1;
        }
        self.edges = Vec::new(); // free the walk-time edge list
        self.finalized = true;
        // deterministic row sort
        for p in 0..n {
            let start = self.child_offset[p] as usize;
            let end = self.child_offset[p + 1] as usize;
            if end - start > 1 {
                let mut row: Vec<NodeId> = self.child_index[start..end].to_vec();
                row.sort_by(|&x, &y| self.compare_nodes(x, y));
                self.child_index[start..end].copy_from_slice(&row);
            }
        }
    }

    /// Has `finalize_children` run? (children queries legal)
    pub fn is_finalized(&self) -> bool {
        self.finalized
    }

    /// Deterministic node comparison: size desc, then depth-stable name asc.
    pub fn compare_nodes(&self, a: NodeId, b: NodeId) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        let (sa, sb) = (self.allocated[a as usize], self.allocated[b as usize]);
        let ord = sb.cmp(&sa);
        if ord != Ordering::Equal {
            return ord;
        }
        let na = self.name_str(a);
        let nb = self.name_str(b);
        na.cmp(&nb)
    }

    /// Children ids of `parent` (CSR slice). Only valid after
    /// `finalize_children()` (see build protocol above).
    pub fn children(&self, parent: NodeId) -> &[NodeId] {
        let p = parent as usize;
        let start = self.child_offset[p] as usize;
        let end = self.child_offset[p + 1] as usize;
        &self.child_index[start..end]
    }

    /// Child count (valid after finalize; during walk returns the live edge
    /// count for the parent — cheap enough for status displays only).
    pub fn child_count(&self, parent: NodeId) -> u32 {
        if self.finalized {
            let p = parent as usize;
            self.child_offset[p + 1] - self.child_offset[p]
        } else {
            self.edges.iter().filter(|&&(p, _)| p == parent).count() as u32
        }
    }

    /// Parent of `node` (root's parent is itself).
    pub fn parent(&self, node: NodeId) -> NodeId {
        self.parent[node as usize]
    }

    /// Depth from the tree root(s).
    pub fn depth(&self, node: NodeId) -> u32 {
        let mut d = 0u32;
        let mut cur = node;
        let root_loop_guard = self.len() + 1;
        while self.parent[cur as usize] != cur {
            cur = self.parent[cur as usize];
            d += 1;
            if d > root_loop_guard as u32 {
                break; // malformed guard — property tests would fail loudly
            }
        }
        d
    }

    /// Name as UTF-16 slice.
    pub fn name_utf16(&self, node: NodeId) -> &[u16] {
        let i = node as usize;
        let start = self.name_off[i] as usize;
        let end = self
            .name_off
            .get(i + 1)
            .map(|&e| e as usize)
            .unwrap_or(self.names.len());
        &self.names[start..end]
    }

    /// Name as String (lossy at display edge only — A4 zero-copy discipline).
    pub fn name_str(&self, node: NodeId) -> String {
        String::from_utf16_lossy(self.name_utf16(node))
    }

    /// Kind byte.
    pub fn kind(&self, node: NodeId) -> u8 {
        self.kind[node as usize]
    }

    /// Logical size.
    pub fn logical(&self, node: NodeId) -> u64 {
        self.logical[node as usize]
    }

    /// Allocated size.
    pub fn allocated(&self, node: usize) -> u64 {
        self.allocated[node]
    }

    /// Subtree file count.
    pub fn files(&self, node: NodeId) -> u32 {
        self.files[node as usize]
    }

    /// Subtree folder count.
    pub fn folders(&self, node: NodeId) -> u32 {
        self.folders[node as usize]
    }

    /// mtime (FILETIME ticks; subtree max; 0 = unknown).
    pub fn mtime(&self, node: NodeId) -> i64 {
        self.mtime[node as usize]
    }

    /// Attribute flags.
    pub fn attr_flags(&self, node: NodeId) -> u32 {
        self.attr_flags[node as usize]
    }

    /// Category id.
    pub fn category(&self, node: NodeId) -> CategoryId {
        self.category[node as usize]
    }

    /// Extension id.
    pub fn ext_id(&self, node: NodeId) -> ExtId {
        self.ext_id[node as usize]
    }

    /// Hard-link owner (0 = none).
    pub fn link_to(&self, node: NodeId) -> u64 {
        self.link_to[node as usize]
    }

    /// Error code (0 = ok).
    pub fn err_code(&self, node: NodeId) -> u16 {
        self.err_code[node as usize]
    }

    /// Add subtree contributions into `node` (used by the completion cascade
    /// and file pushes). `files`/`folders` are deltas to the subtree counters.
    pub fn add_metrics(
        &mut self,
        node: NodeId,
        logical: u64,
        allocated: u64,
        files: u32,
        folders: u32,
    ) {
        let i = node as usize;
        self.logical[i] = self.logical[i].saturating_add(logical);
        self.allocated[i] = self.allocated[i].saturating_add(allocated);
        self.files[i] = self.files[i].saturating_add(files);
        self.folders[i] = self.folders[i].saturating_add(folders);
    }

    /// Set a node's subtree counters directly (aggregation finalize).
    pub fn set_subtree(&mut self, node: NodeId, files: u32, folders: u32) {
        let i = node as usize;
        self.files[i] = files;
        self.folders[i] = folders;
    }

    /// Bump mtime to subtree max.
    pub fn bump_mtime(&mut self, node: NodeId, mtime: i64) {
        let i = node as usize;
        if mtime > self.mtime[i] {
            self.mtime[i] = mtime;
        }
    }

    /// Set category for a node.
    pub fn set_category(&mut self, node: NodeId, cat: CategoryId) {
        self.category[node as usize] = cat;
    }

    /// Mark a node errored.
    pub fn set_err(&mut self, node: NodeId, code: u16) {
        self.err_code[node as usize] = code;
    }

    /// Remove a deleted node's bytes up the chain (WDS-DEL-03) and drop the
    /// CSR row. Node ids stay stable (hole-punched); children list of parent
    /// rebuilds. Returns reclaimed allocated bytes.
    pub fn remove_node(&mut self, node: NodeId) -> u64 {
        let reclaimed = self.allocated[node as usize];
        let mut cur = node;
        let mut guard = 0usize;
        let n = self.len();
        // Walk up subtracting.
        loop {
            let i = cur as usize;
            self.logical[i] = self.logical[i].saturating_sub(reclaimed);
            self.allocated[i] = self.allocated[i].saturating_sub(reclaimed);
            let parent = self.parent[i];
            if parent == cur {
                break;
            }
            cur = parent;
            guard += 1;
            if guard > n {
                break;
            }
        }
        // Detach from parent's CSR row.
        let parent = self.parent[node as usize];
        if parent != node {
            let p = parent as usize;
            let start = self.child_offset[p] as usize;
            let end = self.child_offset[p + 1] as usize;
            if let Some(pos) = self.child_index[start..end].iter().position(|&c| c == node) {
                let abs = start + pos;
                self.child_index.remove(abs);
                for off in self.child_offset.iter_mut().skip(p + 1) {
                    *off -= 1;
                }
            }
        }
        reclaimed
    }

    /// Root nodes (parent == self). Usually exactly one (synthetic root).
    pub fn roots(&self) -> Vec<NodeId> {
        (0..self.n)
            .filter(|&i| self.parent[i as usize] == i)
            .collect()
    }

    /// Deterministic canonical hash for the frozen arena (docs/06 § 12):
    /// XXH3 over an **order-independent** canonical serialization — rows are
    /// (name-path, kind, sizes) sorted lexicographically, so worker arrival
    /// order cannot change the hash. Internal table indices (ext_id) are
    /// excluded: they are arrival-order-dependent; the name carries the same
    /// information deterministically.
    pub fn canonical_hash(&self) -> u64 {
        use std::collections::BTreeSet;
        use xxhash_rust::xxh3::Xxh3;
        let mut rows: BTreeSet<String> = BTreeSet::new();
        for i in 0..self.n as usize {
            // pseudo nodes carry live environmental state (volume free bytes),
            // not fixture data — excluded by definition
            if self.kind[i] == kind::FREE_SPACE || self.kind[i] == kind::UNKNOWN {
                continue;
            }
            let mut row = String::with_capacity(64);
            let _ = write!(
                row,
                "{}|{}|{}|{}|{}|{}|{}",
                self.path_string(i as u32),
                self.kind[i],
                self.logical[i],
                self.allocated[i],
                self.files[i],
                self.folders[i],
                self.category[i]
            );
            rows.insert(row);
        }
        let mut h = Xxh3::new();
        for r in &rows {
            h.update(r.as_bytes());
            h.update(&[0xFF]);
        }
        h.digest()
    }

    /// Name-path of a node (root-to-node, `/`-joined; order-free identity).
    pub fn path_string(&self, node: NodeId) -> String {
        let mut parts: Vec<String> = Vec::new();
        let mut cur = node;
        let mut guard = 0usize;
        while guard <= self.n as usize {
            parts.push(self.name_str(cur));
            let p = self.parent[cur as usize];
            if p == cur {
                break;
            }
            cur = p;
            guard += 1;
        }
        parts.reverse();
        parts.join("/")
    }
}

/// Input for `Arena::push` (borrowed; zero per-node heap allocs beyond rows).
pub struct NodeInput<'a> {
    /// Parent id (self for root).
    pub parent: NodeId,
    /// UTF-16 name slice.
    pub name_utf16: &'a [u16],
    /// Own logical size (files) or 0 (dirs aggregate later).
    pub logical: u64,
    /// Own allocated size.
    pub allocated: u64,
    /// Subtree file count (own for files = 1).
    pub files: u32,
    /// Subtree folder count.
    pub folders: u32,
    /// mtime FILETIME ticks (0 = unknown).
    pub mtime: i64,
    /// Kind byte.
    pub kind: u8,
    /// Category.
    pub category: CategoryId,
    /// Extension id.
    pub ext_id: ExtId,
    /// Attribute flags.
    pub attr_flags: u32,
    /// Hard-link owner file id (0 = none).
    pub link_to: u64,
    /// Error code.
    pub err_code: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Owned-name input builder (tests are exempt from the zero-copy rule).
    fn input_owned(name: &str, parent: NodeId, allocated: u64, kind: u8) -> NodeInput<'static> {
        let utf16: Vec<u16> = name.encode_utf16().collect();
        NodeInput {
            parent,
            name_utf16: Box::leak(utf16.into_boxed_slice()),
            logical: allocated,
            allocated,
            files: u32::from(kind == kind::FILE),
            folders: u32::from(kind == kind::DIR),
            mtime: 0,
            kind,
            category: 1,
            ext_id: 0,
            attr_flags: 0,
            link_to: 0,
            err_code: 0,
        }
    }

    #[test]
    fn push_attach_children() {
        let mut a = Arena::with_capacity(16);
        let root = a.push(input_owned("root", 0, 0, kind::ROOT));
        let c1 = a.push(input_owned("b.bin", root, 100, kind::FILE));
        let c2 = a.push(input_owned("a.bin", root, 50, kind::FILE));
        a.attach(root, c1);
        a.attach(root, c2);
        assert_eq!(a.child_count(root), 2);
        a.finalize_children();
        // size-desc order
        assert_eq!(a.children(root), &[c1, c2]);
        assert_eq!(a.parent(c1), root);
        assert_eq!(a.name_str(c2), "a.bin");
    }

    #[test]
    fn remove_subtracts_upward() {
        let mut a = Arena::with_capacity(16);
        let root = a.push(input_owned("r", 0, 0, kind::DIR));
        let sub = a.push(input_owned("s", root, 0, kind::DIR));
        let f = a.push(input_owned("f", sub, 40, kind::FILE));
        a.attach(root, sub);
        a.attach(sub, f);
        a.add_metrics(sub, 40, 40, 1, 0);
        a.add_metrics(root, 40, 40, 1, 0);
        a.finalize_children();
        let reclaimed = a.remove_node(f);
        assert_eq!(reclaimed, 40);
        assert_eq!(a.allocated(sub as usize), 0);
        assert_eq!(a.allocated(root as usize), 0);
        assert_eq!(a.child_count(sub), 0);
    }

    #[test]
    fn canonical_hash_deterministic() {
        let mut a1 = Arena::with_capacity(8);
        let r = a1.push(input_owned("r", 0, 0, kind::DIR));
        a1.push(input_owned("x", r, 7, kind::FILE));
        let mut a2 = Arena::with_capacity(8);
        let r2 = a2.push(input_owned("r", 0, 0, kind::DIR));
        a2.push(input_owned("x", r2, 7, kind::FILE));
        assert_eq!(a1.canonical_hash(), a2.canonical_hash());
    }
}
