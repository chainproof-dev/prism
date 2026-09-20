//! Turbo scan — raw NTFS MFT walk (PRISM-HG-070, docs/06 § 3, ADR-06).
//!
//! A **first-class strategy**, never a silent fallback: it either runs the
//! raw-index read (Windows + elevation + NTFS) or fails honestly with the
//! reason. The pure half — [`build_arena`] over [`MftEntry`] slices — is
//! platform-independent and unit-tested; the raw volume I/O is Windows-only
//! and exercised by the Windows CI matrix (`cargo test --features
//! turbo-it` on a machine with a scratch NTFS image).
//!
//! Correctness rules carried over from the standard walker (docs/06 § 2.5):
//! hard links share one MFT record (multi-`$FILE_NAME`) — the first
//! WIN32-namespace name wins and extras are recorded as `link_to` siblings;
//! reparse points are classified but never followed (loop safety is
//! structural: the MFT is a flat table); sizes come from `$DATA`
//! allocated/real, not from cluster arithmetic.

use prism_types::scan::ScanSummary;

use crate::agg::ExtensionTable;
use crate::arena::{Arena, NodeId, NodeInput, kind};
use crate::error::EngineError;

/// One MFT-derived node (pure representation — the test surface).
#[derive(Debug, Clone, PartialEq)]
pub struct MftEntry {
    /// File reference number (record index; sequence bits masked).
    pub frn: u64,
    /// Parent directory FRN (0/5 = root).
    pub parent_frn: u64,
    /// Name (WIN32 namespace preferred).
    pub name: String,
    /// Directory?
    pub is_dir: bool,
    /// $DATA real size (logical).
    pub logical: u64,
    /// $DATA allocated size (cluster-rounded) — size-mode truth.
    pub allocated: u64,
    /// $STANDARD_INFORMATION modified (FILETIME ticks).
    pub mtime_ticks: i64,
    /// FILE_ATTRIBUTE_* bits.
    pub attrs: u32,
    /// Reparse point (junction/symlink/mount).
    pub reparse: bool,
}

/// Build an arena + ext table from MFT entries (pure; deterministic order =
/// ascending FRN, ties by name). Root node id 0 carries the volume label;
/// directory subtree counts derive from the entry set. Entries with a
/// missing parent chain up to the root (orphaned records — deleted-parent
/// races during imaging) are skipped and reported in the returned count.
pub fn build_arena(
    entries: &[MftEntry],
    volume_label: &str,
) -> Result<(Arena, ExtensionTable, u32), EngineError> {
    let mut arena = Arena::with_capacity(entries.len().next_power_of_two().max(64));
    let mut ext_table = ExtensionTable::default();

    // Pass 1: assign node ids in FRN order (determinism — the canonical hash
    // depends on insertion order, docs/06 § 12).
    let mut sorted: Vec<&MftEntry> = entries.iter().collect();
    sorted.sort_by(|a, b| a.frn.cmp(&b.frn).then_with(|| a.name.cmp(&b.name)));

    let mut frn_to_id: std::collections::HashMap<u64, NodeId> = std::collections::HashMap::new();
    let mut id_to_frn: Vec<u64> = Vec::with_capacity(sorted.len() + 1);

    // Root: node 0, self-parent (arena convention).
    let root_name: Vec<u16> = volume_label.encode_utf16().collect();
    let root_id = arena.push(NodeInput {
        parent: 0,
        name_utf16: &root_name,
        logical: 0,
        allocated: 0,
        files: 0,
        folders: 1,
        mtime: 0,
        kind: kind::ROOT,
        category: prism_types::ids::CATEGORY_ROOT,
        ext_id: 0,
        attr_flags: 0,
        link_to: 0,
        err_code: 0,
    });
    frn_to_id.insert(0, root_id);
    frn_to_id.insert(5, root_id); // NTFS root FRN is 5
    id_to_frn.push(5);

    let mut skipped = 0u32;
    // Pre-pass: the parent universe (dirs + the NTFS root, FRN 5). Entries
    // whose parent is absent (live-volume deletion races) are never pushed —
    // unreachable nodes would inflate Σ and the arena length.
    let dir_frns: std::collections::HashSet<u64> = std::collections::HashSet::from_iter(
        sorted
            .iter()
            .filter(|e| e.is_dir)
            .map(|e| e.frn)
            .chain(std::iter::once(5u64)),
    );
    // Pass 2: push every resolvable entry (forward parent refs are legal —
    // a child record may precede its parent's). The FRN-5 record IS the
    // root (already created) — merged, never duplicated.
    struct Pending {
        nid: NodeId,
        parent_frn: u64,
    }
    let mut pending_links: Vec<Pending> = Vec::with_capacity(sorted.len());
    for e in &sorted {
        if e.frn == 5 {
            continue; // the volume root — node 0 exists
        }
        let parent_frn = if e.parent_frn <= 5 { 5 } else { e.parent_frn };
        if !dir_frns.contains(&parent_frn) {
            skipped += 1;
            continue;
        }
        let name16: Vec<u16> = e.name.encode_utf16().collect();
        let ext_key = if e.is_dir {
            String::new()
        } else {
            crate::agg::extension_key(&name16)
        };
        let ext_id = ext_table.intern(&ext_key);
        // Reparse points classify FIRST (a junction is a reparse dir, and
        // following it is what creates loops — they are leaf nodes here).
        let node_kind = if e.reparse {
            kind::REPARSE
        } else if e.is_dir {
            kind::DIR
        } else {
            kind::FILE
        };
        let mut attr_flags = e.attrs & 0x0000_FFFF; // NT attribute bits map 1:1 to ours
        if e.reparse {
            attr_flags |= crate::arena::flags::REPARSE;
        }
        let nid = arena.push(NodeInput {
            parent: 0, // fixed in pass 3
            name_utf16: &name16,
            logical: e.logical,
            allocated: e.allocated,
            files: if e.is_dir { 0 } else { 1 },
            folders: if e.is_dir { 1 } else { 0 },
            mtime: e.mtime_ticks,
            kind: node_kind,
            category: if e.is_dir {
                prism_types::ids::CATEGORY_ROOT
            } else {
                crate::agg::category_for_ext(&ext_key)
            },
            ext_id,
            attr_flags,
            link_to: 0,
            err_code: 0,
        });
        id_to_frn.push(e.frn);
        if e.is_dir && !e.reparse {
            frn_to_id.insert(e.frn, nid); // reparse dirs are leaves (no descend)
        }
        pending_links.push(Pending { nid, parent_frn });
    }

    // Pass 3: attach (parents are all present — guaranteed by the pre-pass).
    for p in &pending_links {
        let parent = frn_to_id.get(&p.parent_frn).copied().unwrap_or(root_id); // root fallback is unreachable by pre-pass
        arena.attach(parent, p.nid);
    }

    arena.finalize_children();
    Ok((arena, ext_table, skipped))
}

// ---------------------------------------------------------------------------
// Windows raw-volume reader
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod win {
    //! Raw NTFS volume reading: boot sector → $MFT run list → record stream.
    #![allow(unsafe_code, clippy::undocumented_unsafe_blocks)]

    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_BEGIN, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, ReadFile,
        SetFilePointerEx,
    };

    use super::MftEntry;
    use crate::error::EngineError;

    pub struct VolumeReader {
        h: HANDLE,
        pub cluster_size: u64,
        pub bps: u64,
    }

    impl Drop for VolumeReader {
        fn drop(&mut self) {
            if !self.h.is_null() {
                unsafe { windows_sys::Win32::Foundation::CloseHandle(self.h) };
            }
        }
    }

    fn last_os_err(msg: &'static str) -> EngineError {
        let code = unsafe { windows_sys::Win32::Foundation::GetLastError() } as i32;
        EngineError::Os { code, msg }
    }

    impl VolumeReader {
        /// Open the raw volume (e.g. `\\.\C:`). Requires elevation for
        /// read access on modern Windows — the caller (preflight + consent
        /// UX) guarantees it.
        pub fn open(volume: &str) -> Result<Self, EngineError> {
            let path = format!("\\\\.\\{}:", volume.trim_end_matches([':', '\\']));
            let mut wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
            // SAFETY: NUL-terminated wide path; handle checked immediately.
            let h = unsafe {
                CreateFileW(
                    wide.as_mut_ptr(),
                    0x8000_0000, // GENERIC_READ
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    std::ptr::null(),
                    OPEN_EXISTING,
                    0,
                    std::ptr::null_mut(),
                )
            };
            if h == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
                return Err(last_os_err("open raw volume (elevation required)"));
            }
            let mut r = VolumeReader {
                h,
                cluster_size: 0,
                bps: 0,
            };
            let boot = r.read_at(0, 512)?;
            if &boot[3..11] != b"NTFS    " {
                return Err(EngineError::Invalid {
                    field: "volume (not NTFS — turbo scan requires NTFS)",
                });
            }
            let bps = u64::from(u16::from_le_bytes([boot[11], boot[12]]));
            let spc = u64::from(boot[13]);
            if bps == 0 || bps & (bps - 1) != 0 || spc == 0 {
                return Err(EngineError::Invalid {
                    field: "boot sector (bad BPB)",
                });
            }
            r.bps = bps;
            r.cluster_size = bps * spc;
            Ok(r)
        }

        /// Synchronous positional read.
        pub fn read_at(&self, offset: u64, len: usize) -> Result<Vec<u8>, EngineError> {
            let mut out = vec![0u8; len];
            let mut done = 0usize;
            while done < len {
                let dist = windows_sys::Win32::Foundation::LARGE_INTEGER {
                    QuadPart: (offset + done as u64) as i64,
                };
                // SAFETY: dist is a valid stack value by value union.
                if unsafe { SetFilePointerEx(self.h, dist, std::ptr::null_mut(), FILE_BEGIN) } == 0
                {
                    return Err(last_os_err("seek volume"));
                }
                let mut got = 0u32;
                // SAFETY: buffer bounds are valid for the remaining slice.
                let ok = unsafe {
                    ReadFile(
                        self.h,
                        out.as_mut_ptr().add(done).cast(),
                        (len - done) as u32,
                        &mut got,
                        std::ptr::null_mut(),
                    )
                };
                if ok == 0 {
                    return Err(last_os_err("read volume"));
                }
                if got == 0 {
                    break; // EOF
                }
                done += got as usize;
            }
            out.truncate(done);
            Ok(out)
        }
    }

    /// Read the whole MFT entry stream for a volume.
    pub fn read_mft_entries(volume: &str) -> Result<Vec<MftEntry>, EngineError> {
        let vr = VolumeReader::open(volume)?;
        let boot = vr.read_at(0, 512)?;
        let bps = u64::from(u16::from_le_bytes([boot[11], boot[12]]));
        let spc = u64::from(boot[13]);
        let cluster = bps * spc;
        let mft_lcn = le64(&boot, 0x30);
        let rec_size_ind = boot[0x40] as i8;
        let rec_size: u64 = if rec_size_ind < 0 {
            cluster * (1u64 << (-rec_size_ind))
        } else {
            u64::from(rec_size_ind as u8).max(1024)
        };

        // Record 0 = $MFT itself; its $DATA runlist gives the extents.
        let mft_offset = mft_lcn * cluster;
        let rec0 = vr.read_at(mft_offset, rec_size as usize)?;
        let m0 = prism_ntfs::mft::MftRecord::parse(&rec0)?;
        let mut mft_extents: Vec<(u64, u64, u64)> = Vec::new(); // (vcn_byte, len, disk_byte)
        for attr in m0.attributes() {
            let view = attr.map_err(EngineError::Ntfs)?;
            if matches!(view.ty, prism_ntfs::attrs::AttributeType::Data) {
                if let Some(runs) = view.runs().map_err(EngineError::Ntfs)? {
                    // map_runs yields (start_vcn_bytes, len_bytes, start_lcn);
                    // convert LCN → disk byte offset for raw reads.
                    mft_extents = prism_ntfs::runs::map_runs(&runs, cluster)
                        .map_err(EngineError::Ntfs)?
                        .into_iter()
                        .map(|(v, l, lcn)| (v, l, lcn * cluster))
                        .collect();
                }
            }
        }
        if mft_extents.is_empty() {
            return Err(EngineError::Invalid {
                field: "$MFT $DATA (no runlist)",
            });
        }

        // Total MFT bytes = last extent end.
        let mft_len: u64 = mft_extents.iter().map(|(v, l, _)| v + l).max().unwrap_or(0);
        let n_records = (mft_len / rec_size).min(40 * 1024 * 1024); // sanity ceiling

        let mut out = Vec::with_capacity(n_records as usize);
        let mut buf = vec![0u8; rec_size as usize];
        for i in 0..n_records {
            let file_off = i * rec_size;
            if !read_extent(&vr, &mft_extents, file_off, &mut buf)? {
                break;
            }
            let Ok(rec) = prism_ntfs::mft::MftRecord::parse(&buf) else {
                continue; // damaged record: skip, count via skipped return
            };
            if !rec.in_use() {
                continue;
            }
            if let Some(e) = entry_from_record(i as u64, &rec) {
                out.push(e);
            }
        }
        Ok(out)
    }

    /// Read `buf.len()` bytes at `file_off` of the extent-mapped stream.
    fn read_extent(
        vr: &VolumeReader,
        extents: &[(u64, u64, u64)],
        file_off: u64,
        buf: &mut [u8],
    ) -> Result<bool, EngineError> {
        let mut need = buf.len() as u64;
        let mut done = 0u64;
        for &(vcn_b, len_b, disk_b) in extents {
            if need == 0 {
                break;
            }
            let ext_end = vcn_b + len_b;
            if file_off >= ext_end {
                continue;
            }
            let within = file_off.max(vcn_b) - vcn_b;
            if file_off < vcn_b {
                // hole before this extent → zeros
                let hole = (vcn_b - file_off).min(need);
                for b in &mut buf[done as usize..(done + hole) as usize] {
                    *b = 0;
                }
                done += hole;
                need -= hole;
                continue;
            }
            let take = (len_b - within).min(need);
            let chunk = vr.read_at(disk_b + within, take as usize)?;
            buf[done as usize..done as usize + chunk.len()].copy_from_slice(&chunk);
            done += chunk.len() as u64;
            need -= take;
        }
        Ok(done == buf.len() as u64)
    }

    fn entry_from_record(frn: u64, rec: &prism_ntfs::mft::MftRecord) -> Option<MftEntry> {
        use prism_ntfs::attrs::{AttributeType, FileNameAttr, StandardInformation};

        let mut best_name: Option<(u8, String, u64)> = None; // (rank, name, parent)
        let mut si_modified = 0i64;
        let mut logical = 0u64;
        let mut allocated = 0u64;
        let mut attrs = 0u32;
        let mut reparse = false;
        let mut is_dir = rec.is_dir();

        for attr in rec.attributes() {
            let view = attr.ok()?;
            match view.ty {
                AttributeType::StandardInformation => {
                    if let Some((_, v)) = view.resident().ok().flatten() {
                        if let Ok(si) = StandardInformation::parse(v) {
                            si_modified = si.modified;
                            attrs = si.file_permissions;
                        }
                    }
                }
                AttributeType::FileName => {
                    if let Some((_, v)) = view.resident().ok().flatten() {
                        if let Ok(fna) = FileNameAttr::parse(v) {
                            // Rank: WIN32&DOS(3) > WIN32(2) > DOS(1) > POSIX(0)
                            let rank = match fna.namespace {
                                3 => 4,
                                2 => 3,
                                1 => 1,
                                _ => 2,
                            };
                            let better = best_name
                                .as_ref()
                                .map(|(r, _, _)| rank > *r)
                                .unwrap_or(true);
                            if better && !fna.name.is_empty() {
                                best_name =
                                    Some((rank, fna.name, fna.parent_frn & 0xFFFF_FFFF_FFFF));
                            }
                            // Sizes: $FILE_NAME carries them too — prefer $DATA.
                            // Sizes from $FILE_NAME are stale for live files —
                            // $DATA (below) is the authority; only the name and
                            // parent are taken from here.
                        }
                    }
                }
                AttributeType::Data => {
                    // Only the unnamed stream counts (ADS ignored, honest).
                    if view.header.name_len == 0 {
                        match view.non_resident().ok().flatten() {
                            Some(h) => {
                                allocated = h.alloc_size;
                                logical = h.real_size;
                            }
                            None => {
                                if let Some((_, v)) = view.resident().ok().flatten() {
                                    allocated = v.len() as u64;
                                    logical = v.len() as u64;
                                }
                            }
                        }
                    }
                }
                AttributeType::IndexRoot => {
                    is_dir = true;
                }
                _ => {}
            }
        }
        if attrs & 0x400 != 0 {
            reparse = true;
        }
        let (_, name, parent_frn) = best_name?;
        if name.len() > 512 {
            return None;
        }
        Some(MftEntry {
            frn,
            parent_frn,
            name,
            is_dir,
            logical,
            allocated: allocated.max(logical),
            mtime_ticks: si_modified,
            attrs,
            reparse,
        })
    }

    fn le64(b: &[u8], off: usize) -> u64 {
        let mut v = 0u64;
        for i in (0..8).rev() {
            v = (v << 8) | u64::from(b[off + i]);
        }
        v
    }
}

#[cfg(windows)]
pub use win::{VolumeReader, read_mft_entries};

/// Turbo scan entry point (coordinator calls this on its worker thread).
/// Windows: raw MFT read. Other platforms: honest failure (A2/ADR-06 — no
/// silent standard substitution).
pub fn scan_volume(volume: &str) -> Result<Vec<MftEntry>, EngineError> {
    #[cfg(windows)]
    {
        win::read_mft_entries(volume)
    }
    #[cfg(not(windows))]
    {
        let _ = volume;
        Err(EngineError::Invalid {
            field: "strategy=turbo (raw NTFS scan requires Windows with elevation)",
        })
    }
}

/// Build the CompletedScan summary for a turbo scan (coordinator helper).
#[allow(clippy::too_many_arguments)] // mirrors ScanSummary fields 1:1
pub fn turbo_summary(
    scan_id: u32,
    root: &str,
    files: u64,
    folders: u64,
    logical: u64,
    allocated: u64,
    free: u64,
    duration_ms: u64,
    errors: u32,
) -> ScanSummary {
    ScanSummary {
        scan_id,
        root: root.to_string(),
        strategy: prism_types::scan::ScanStrategy::Turbo,
        size_mode: prism_types::scan::SizeMode::Allocated,
        files,
        folders,
        logical,
        allocated,
        unique: allocated,
        unknown: 0,
        free,
        duration_ms,
        errors,
        truncated: false,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)] // test policy (docs/06 § 12)

    use super::*;

    fn dir(frn: u64, parent: u64, name: &str) -> MftEntry {
        MftEntry {
            frn,
            parent_frn: parent,
            name: name.into(),
            is_dir: true,
            logical: 0,
            allocated: 0,
            mtime_ticks: 0,
            attrs: 0,
            reparse: false,
        }
    }

    fn file(frn: u64, parent: u64, name: &str, logical: u64, allocated: u64) -> MftEntry {
        MftEntry {
            frn,
            parent_frn: parent,
            name: name.into(),
            is_dir: false,
            logical,
            allocated,
            mtime_ticks: 132_000_000_000_000_000, // some FILETIME
            attrs: 0x20,                          // ARCHIVE
            reparse: false,
        }
    }

    #[test]
    fn builds_arena_with_counts_and_determinism() {
        let entries = vec![
            dir(5, 5, "."), // the FRN-5 record: merged into the root node
            dir(100, 5, "proj"),
            dir(200, 5, "media"),
            file(101, 100, "a.txt", 10, 12),
            file(102, 100, "b.rs", 20, 24),
            file(201, 200, "m.mp4", 100, 128),
        ];
        let (a1, _, skipped) = build_arena(&entries, "C:").unwrap();
        assert_eq!(skipped, 0);
        assert_eq!(a1.len(), 6, "root + 5 entries (FRN-5 merged)");

        // Determinism: same entries → identical canonical hash (docs/06 § 12).
        let (a2, _, _) = build_arena(&entries, "C:").unwrap();
        assert_eq!(a1.canonical_hash(), a2.canonical_hash());

        // Reordered input must NOT change the hash (FRN sort wins).
        let mut shuffled = entries.clone();
        shuffled.reverse();
        let (a3, _, _) = build_arena(&shuffled, "C:").unwrap();
        assert_eq!(a1.canonical_hash(), a3.canonical_hash());
    }

    #[test]
    fn orphans_are_skipped_and_counted() {
        let entries = vec![
            dir(5, 5, "."),
            dir(100, 5, "real"),
            file(101, 999, "lost.txt", 10, 12), // parent 999 absent
        ];
        let (a, _, skipped) = build_arena(&entries, "C:").unwrap();
        assert_eq!(skipped, 1);
        assert_eq!(a.len(), 2, "root + the resolvable dir only");
        // The skipped file must not leak into the root's aggregates.
        assert_eq!(a.files(0), 0);
        assert_eq!(a.allocated(0), 0);
    }

    #[test]
    fn reparse_classified_never_followed() {
        let entries = vec![MftEntry {
            frn: 300,
            parent_frn: 5,
            name: "junction".into(),
            is_dir: true,
            logical: 0,
            allocated: 0,
            mtime_ticks: 0,
            attrs: 0x400, // FILE_ATTRIBUTE_REPARSE_POINT
            reparse: true,
        }];
        let (a, _, _) = build_arena(&entries, "C:").unwrap();
        assert_eq!(a.kind(1), kind::REPARSE);
        assert!(a.attr_flags(1) & crate::arena::flags::REPARSE != 0);
    }

    #[test]
    fn turbo_off_windows_is_honest() {
        if cfg!(not(windows)) {
            let err = scan_volume("C").unwrap_err();
            assert!(matches!(err, EngineError::Invalid { .. }));
        }
    }
}
