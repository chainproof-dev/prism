//! MFT record parsing: file record segments with fixup (update sequence)
//! application and attribute iteration (docs/06 § 3).

use crate::attrs::{AttrHeader, AttributeType, NonResidentHeader, ResidentHeader};
use crate::error::{Error, Result};

/// Multi-sector header (shared by file records and attribute lists).
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MultiSectorHeader {
    /// Magic, e.g. `FILE` for MFT records.
    pub magic: [u8; 4],
    /// Update sequence array offset from record start.
    pub usa_offset: u16,
    /// Update sequence array size in u16 units (incl. the USN itself).
    pub usa_count: u16,
}

/// Parsed MFT file record header (after fixup application).
#[derive(Debug, Clone, Copy)]
pub struct FileRecordHeader {
    /// Sequence number (record reuse counter).
    pub sequence: u16,
    /// Hard link count.
    pub link_count: u16,
    /// Offset of the first attribute.
    pub attrs_offset: u16,
    /// Record flags (in-use 0x01, directory 0x02).
    pub flags: u16,
    /// Used size of the record.
    pub used_size: u32,
    /// Allocated size of the record.
    pub alloc_size: u32,
    /// Base record file reference (0 = base record).
    pub base_record: u64,
    /// Next attribute id.
    pub next_attr_id: u16,
}

/// A parsed, fixup-applied MFT record view over a copied buffer.
pub struct MftRecord {
    /// Fixup-applied record bytes.
    pub data: Vec<u8>,
    /// Parsed header.
    pub header: FileRecordHeader,
}

/// Sector size floor (512) used by fixup validation.
const SECTOR_MIN: usize = 512;

impl MftRecord {
    /// Parse and apply the update sequence (fixup) array.
    ///
    /// # Errors
    /// Typed parse errors — never panics, suitable for fuzzing (docs/06 § 12).
    pub fn parse(raw: &[u8]) -> Result<Self> {
        if raw.len() < 48 {
            return Err(Error::truncated(0, 48, raw.len()));
        }
        if &raw[0..4] != b"FILE" {
            return Err(Error::BadMagic {
                expected: "FILE",
                offset: 0,
            });
        }
        let usa_offset = u16::from_le_bytes([raw[4], raw[5]]) as usize;
        let usa_count = u16::from_le_bytes([raw[6], raw[7]]) as usize;
        if usa_count == 0 || usa_offset + usa_count * 2 > raw.len() {
            return Err(Error::BadAttribute {
                ty: 0,
                offset: 4,
                reason: format!("usa out of bounds: off={usa_offset} count={usa_count}"),
            });
        }
        // Apply fixups: last 2 bytes of every sector-sized block are replaced.
        let mut data = raw.to_vec();
        let stride = if raw.len() >= SECTOR_MIN * 2 {
            SECTOR_MIN
        } else {
            raw.len().max(1)
        };
        // USN check: every sector tail must equal usa[0].
        for i in 1..usa_count {
            let tail = i * stride - 2;
            if tail + 2 > data.len() {
                return Err(Error::truncated(tail, 2, data.len() - tail.min(data.len())));
            }
            let usn = u16::from_le_bytes([raw[usa_offset], raw[usa_offset + 1]]);
            let found = u16::from_le_bytes([data[tail], data[tail + 1]]);
            if found != usn {
                return Err(Error::FixupMismatch { index: i });
            }
            let fix = u16::from_le_bytes([raw[usa_offset + 2 * i], raw[usa_offset + 2 * i + 1]]);
            data[tail] = fix.to_le_bytes()[0];
            data[tail + 1] = fix.to_le_bytes()[1];
        }
        let header = FileRecordHeader {
            sequence: u16::from_le_bytes([data[16], data[17]]),
            link_count: u16::from_le_bytes([data[18], data[19]]),
            attrs_offset: u16::from_le_bytes([data[20], data[21]]),
            flags: u16::from_le_bytes([data[22], data[23]]),
            used_size: u32::from_le_bytes([data[24], data[25], data[26], data[27]]),
            alloc_size: u32::from_le_bytes([data[28], data[29], data[30], data[31]]),
            base_record: u64::from_le_bytes(
                data[32..40]
                    .try_into()
                    .map_err(|_| Error::truncated(32, 8, data.len().saturating_sub(32)))?,
            ),
            next_attr_id: u16::from_le_bytes([data[40], data[41]]),
        };
        if header.attrs_offset as usize >= data.len() {
            return Err(Error::OutOfRange {
                what: "attrs_offset",
                value: header.attrs_offset as u64,
            });
        }
        Ok(Self { data, header })
    }

    /// Is the record in use?
    pub fn in_use(&self) -> bool {
        self.header.flags & 0x0001 != 0
    }

    /// Is this record a directory?
    pub fn is_dir(&self) -> bool {
        self.header.flags & 0x0002 != 0
    }

    /// Iterate attribute headers in this record.
    pub fn attributes(&self) -> AttributeIter<'_> {
        AttributeIter {
            data: &self.data,
            pos: self.header.attrs_offset as usize,
        }
    }
}

/// Iterator over the attributes of an MFT record.
pub struct AttributeIter<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Iterator for AttributeIter<'a> {
    type Item = Result<AttrView<'a>>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.pos + 4 > self.data.len() {
            return None;
        }
        let ty = u32::from_le_bytes(self.data[self.pos..self.pos + 4].try_into().ok()?);
        if ty == 0xFFFFFFFF {
            return None; // end marker
        }
        match AttrHeader::parse(self.data, self.pos) {
            Ok(h) => {
                let pos_u32 = self.pos as u64;
                if pos_u32 > u32::MAX as u64 {
                    return Some(Err(Error::OutOfRange {
                        what: "attribute offset",
                        value: pos_u32,
                    }));
                }
                let pos_u32 = pos_u32 as u32;
                self.pos = match h.len.checked_add(pos_u32) {
                    Some(next) if next as usize > self.pos && next as usize <= self.data.len() => {
                        next as usize
                    }
                    _ => {
                        return Some(Err(Error::BadAttribute {
                            ty,
                            offset: self.pos,
                            reason: "attribute length out of bounds".into(),
                        }));
                    }
                };
                Some(Ok(AttrView {
                    data: self.data,
                    header: h,
                    ty: AttributeType::from_raw(ty),
                }))
            }
            Err(e) => Some(Err(e)),
        }
    }
}

/// A typed view over one attribute instance.
pub struct AttrView<'a> {
    data: &'a [u8],
    /// Parsed common header.
    pub header: AttrHeader,
    /// Attribute type enum.
    pub ty: AttributeType,
}

impl AttrView<'_> {
    /// Resident header + value bytes (files with inline data).
    pub fn resident(&self) -> Result<Option<(ResidentHeader, &[u8])>> {
        match &self.header.specific {
            crate::attrs::AttrSpecific::Resident(r) => {
                let start = self.header.offset + r.value_offset as usize;
                let end = start
                    .checked_add(r.value_len as usize)
                    .unwrap_or(usize::MAX);
                if end > self.data.len() {
                    return Err(Error::truncated(
                        start,
                        r.value_len as usize,
                        self.data.len().saturating_sub(start),
                    ));
                }
                Ok(Some((*r, &self.data[start..end])))
            }
            _ => Ok(None),
        }
    }

    /// Non-resident header (data runs live elsewhere).
    pub fn non_resident(&self) -> Result<Option<NonResidentHeader>> {
        match &self.header.specific {
            crate::attrs::AttrSpecific::NonResident(nr) => Ok(Some(*nr)),
            _ => Ok(None),
        }
    }

    /// Decoded run list bytes for non-resident attributes.
    pub fn runs(&self) -> Result<Option<Vec<crate::runs::Run>>> {
        match self.non_resident()? {
            Some(nr) => {
                let start = self.header.offset + nr.runs_offset as usize;
                if start > self.data.len() {
                    return Err(Error::truncated(start, 1, self.data.len()));
                }
                // Runs extend to the attribute end (header.len) at most.
                let end = (self.header.offset + self.header.len as usize).min(self.data.len());
                if start > end {
                    return Err(Error::BadRunList {
                        offset: start,
                        reason: "runs start past attribute end",
                    });
                }
                Ok(Some(crate::runs::decode_runs(&self.data[start..end])?))
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a synthetic FILE record with one resident $FILE_NAME-ish attribute.
    fn synth_record() -> Vec<u8> {
        const SZ: usize = 1024;
        let mut b = vec![0u8; SZ];
        b[0..4].copy_from_slice(b"FILE");
        // usa: offset 48, count 3 (one USN + 2 fixups at 512-byte boundaries… but
        // our record is 1024 bytes → stride 512, fixups at 510 and 1022)
        b[4..6].copy_from_slice(&(48u16).to_le_bytes());
        b[6..8].copy_from_slice(&(3u16).to_le_bytes());
        // USN value 0xABCD stored at 48; tails must match.
        b[48..50].copy_from_slice(&0xABCDu16.to_le_bytes());
        b[510..512].copy_from_slice(&0xABCDu16.to_le_bytes());
        b[1022..1024].copy_from_slice(&0xABCDu16.to_le_bytes());
        // header fields
        b[16..18].copy_from_slice(&1u16.to_le_bytes()); // sequence
        b[18..20].copy_from_slice(&1u16.to_le_bytes()); // link count
        b[20..22].copy_from_slice(&56u16.to_le_bytes()); // attrs offset
        b[22..24].copy_from_slice(&0x0001u16.to_le_bytes()); // in use
        b[24..28].copy_from_slice(&(SZ as u32).to_le_bytes()); // used
        b[28..32].copy_from_slice(&(SZ as u32).to_le_bytes()); // alloc
        // attribute at 56: type 0x30 ($FILE_NAME), len 72, resident
        b[56..60].copy_from_slice(&0x30u32.to_le_bytes());
        b[60..64].copy_from_slice(&72u32.to_le_bytes());
        b[64] = 0; // non-resident flag = resident
        b[65] = 0; // name len
        b[66..68].copy_from_slice(&0u16.to_le_bytes()); // name offset
        b[68..70].copy_from_slice(&0u16.to_le_bytes()); // flags
        b[70..72].copy_from_slice(&0u16.to_le_bytes()); // instance
        b[72..76].copy_from_slice(&24u32.to_le_bytes()); // value len (u32)
        b[76..78].copy_from_slice(&40u16.to_le_bytes()); // value offset → 56+40=96
        // 24 bytes of "value"
        b[96..120].copy_from_slice(&[0xAAu8; 24]);
        // end marker at 56+72=128
        b[128..132].copy_from_slice(&0xFFFFFFFFu32.to_le_bytes());
        b
    }

    #[test]
    fn parses_synthetic_record() {
        let rec = MftRecord::parse(&synth_record()).unwrap_or_else(|e| panic!("{e}"));
        assert!(rec.in_use());
        assert!(!rec.is_dir());
        assert_eq!(rec.header.sequence, 1);
        let attrs: Vec<_> = rec.attributes().collect();
        assert_eq!(attrs.len(), 1);
        let view = attrs
            .into_iter()
            .next()
            .unwrap()
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(view.ty, AttributeType::FileName);
        let (_, value) = view.resident().unwrap().expect("resident");
        assert_eq!(value.len(), 24);
        assert_eq!(value[0], 0xAA);
    }

    #[test]
    fn fixup_mismatch_is_detected() {
        let mut b = synth_record();
        b[510] ^= 0xFF; // corrupt a sector tail
        assert!(matches!(
            MftRecord::parse(&b),
            Err(Error::FixupMismatch { .. })
        ));
    }
}
