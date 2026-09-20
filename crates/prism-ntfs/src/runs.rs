//! MFT run-list decoding (data runs: cluster offsets/lengths in sparse
//! compressed encoding). Reference: NTFS on-disk format, $DATA run lists.

use crate::error::{Error, Result};

/// One decoded run: (logical cluster count, cluster offset delta from previous,
/// sparse flag). Absolute VCN offset = Σ previous lengths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Run {
    /// Length in clusters.
    pub len_clusters: u64,
    /// Absolute starting cluster (None = sparse run).
    pub start_cluster: Option<u64>,
}

/// Decode a run list buffer into absolute runs.
///
/// The run list is a sequence of variable-length headers: a byte whose high
/// nibble is the offset-field size and low nibble the length-field size;
/// `0x00` terminates. Offsets are signed little-endian deltas.
pub fn decode_runs(buf: &[u8]) -> Result<Vec<Run>> {
    let mut runs = Vec::new();
    let mut pos: usize = 0;
    let mut abs: u64 = 0;
    loop {
        let Some(&header) = buf.get(pos) else {
            return Err(Error::truncated(pos, 1, 0));
        };
        if header == 0 {
            return Ok(runs); // terminator
        }
        let len_size = (header & 0x0F) as usize;
        let off_size = (header >> 4) as usize;
        if len_size == 0 || len_size > 8 || off_size > 8 {
            return Err(Error::BadRunList {
                offset: pos,
                reason: "impossible field size in run header",
            });
        }
        let len_off = pos + 1;
        let off_off = len_off + len_size;
        let next = off_off + off_size;
        if buf.len() < next {
            return Err(Error::truncated(pos, next - pos, buf.len() - pos));
        }
        let len_clusters = read_uint(&buf[len_off..off_off]);
        if len_clusters == 0 {
            return Err(Error::BadRunList {
                offset: pos,
                reason: "zero-length run",
            });
        }
        let start = if off_size == 0 {
            None // sparse
        } else {
            let delta = read_int(&buf[off_off..next]);
            // Overflow-safe accumulation (checked ops on user-derived data, A3).
            abs = abs.checked_add_signed(delta).ok_or(Error::OutOfRange {
                what: "cluster offset accumulation",
                value: abs,
            })?;
            Some(abs)
        };
        runs.push(Run {
            len_clusters,
            start_cluster: start,
        });
        pos = next;
    }
}

/// Byte-range mapping of a decoded run list: contiguous (file-offset-range,
/// cluster) pairs for reading file data by VCN. Sparse ranges are omitted.
pub fn map_runs(runs: &[Run], cluster_size: u64) -> Result<Vec<(u64, u64, u64)>> {
    // returns Vec<(start_vcn_byte, len_bytes, start_cluster_byte)>
    let mut out = Vec::with_capacity(runs.len());
    let mut vcn: u64 = 0;
    for r in runs {
        let bytes = r
            .len_clusters
            .checked_mul(cluster_size)
            .ok_or(Error::OutOfRange {
                what: "run byte length",
                value: r.len_clusters,
            })?;
        if let Some(start) = r.start_cluster {
            let abs_byte = start.checked_mul(cluster_size).ok_or(Error::OutOfRange {
                what: "cluster byte offset",
                value: start,
            })?;
            out.push((vcn, bytes, abs_byte));
        }
        vcn = vcn.checked_add(bytes).ok_or(Error::OutOfRange {
            what: "vcn accumulation",
            value: vcn,
        })?;
    }
    Ok(out)
}

fn read_uint(b: &[u8]) -> u64 {
    let mut v: u64 = 0;
    for (i, byte) in b.iter().enumerate() {
        v |= u64::from(*byte) << (8 * i);
    }
    v
}

fn read_int(b: &[u8]) -> i64 {
    // sign-extend the last byte
    let mut v: u64 = 0;
    for (i, byte) in b.iter().enumerate() {
        v |= u64::from(*byte) << (8 * i);
    }
    let bits = b.len() * 8;
    if bits == 0 || bits >= 64 {
        return v as i64;
    }
    let sign_bit = 1u64 << (bits - 1);
    if v & sign_bit != 0 {
        v |= !0u64 << bits; // extend
    }
    v as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_single_run() {
        // 1-byte len field, 1-byte offset field; len=0x20, delta=0x10
        let buf = [0x11, 0x20, 0x10, 0x00];
        let runs = decode_runs(&buf).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            runs,
            vec![Run {
                len_clusters: 0x20,
                start_cluster: Some(0x10)
            }]
        );
    }

    #[test]
    fn decodes_sparse_run() {
        // len field 1 byte, no offset field → sparse
        let buf = [0x01, 0x40, 0x11, 0x10, 0x01, 0x00];
        let runs = decode_runs(&buf).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].start_cluster, None);
        assert_eq!(runs[1].start_cluster, Some(0x01));
    }

    #[test]
    fn negative_delta_walks_backwards() {
        // run1: len=0x10 at +0x10 → 0x10; run2: len=0x10 at −0x08 → 0x08
        let buf = [
            0x12, 0x10, 0x00, 0x10, 0x11, 0x10, 0xF8, /* -0x08 */
            0x00,
        ];
        let runs = decode_runs(&buf).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(runs[1].start_cluster, Some(0x08));
    }

    #[test]
    fn rejects_impossible_header() {
        assert!(decode_runs(&[0xF1, 0x01, 0x00]).is_err());
    }

    #[test]
    fn truncation_is_typed() {
        assert!(matches!(
            decode_runs(&[0x11, 0x05]),
            Err(Error::Truncated { .. })
        ));
    }
}
