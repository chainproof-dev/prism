//! NTFS attribute headers: common + resident/non-resident specifics, and typed
//! value parsers for the attributes the turbo scan consumes
//! ($STANDARD_INFORMATION, $FILE_NAME, $DATA, $OBJECT_ID).

use crate::error::{Error, Result};

/// Well-known attribute types we parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeType {
    /// 0x10 — standard information.
    StandardInformation,
    /// 0x30 — file name.
    FileName,
    /// 0x40 — object id / volume-unique id.
    ObjectId,
    /// 0x80 — data stream.
    Data,
    /// 0x90 — index root.
    IndexRoot,
    /// 0xA0 — index allocation.
    IndexAllocation,
    /// Anything else (skipped by the walker).
    Other(u32),
}

impl AttributeType {
    /// Map the raw u32.
    pub fn from_raw(ty: u32) -> Self {
        match ty {
            0x10 => Self::StandardInformation,
            0x30 => Self::FileName,
            0x40 => Self::ObjectId,
            0x80 => Self::Data,
            0x90 => Self::IndexRoot,
            0xA0 => Self::IndexAllocation,
            other => Self::Other(other),
        }
    }
}

/// Resident (inline) attribute header.
#[derive(Debug, Clone, Copy)]
pub struct ResidentHeader {
    /// Value length in bytes.
    pub value_len: u32,
    /// Value offset from the attribute start.
    pub value_offset: u16,
}

/// Non-resident attribute header.
#[derive(Debug, Clone, Copy)]
pub struct NonResidentHeader {
    /// Starting VCN.
    pub start_vcn: u64,
    /// Last VCN (inclusive).
    pub last_vcn: u64,
    /// Run list offset from attribute start.
    pub runs_offset: u16,
    /// Allocated size (cluster-rounded).
    pub alloc_size: u64,
    /// Real (valid data) size.
    pub real_size: u64,
    /// Initialized size.
    pub init_size: u64,
}

/// Resident vs non-resident discriminator.
#[derive(Debug, Clone, Copy)]
pub enum AttrSpecific {
    /// Inline value.
    Resident(ResidentHeader),
    /// Data runs.
    NonResident(NonResidentHeader),
}

/// Parsed common attribute header at a known offset.
#[derive(Debug, Clone, Copy)]
pub struct AttrHeader {
    /// Offset of the attribute within the record buffer.
    pub offset: usize,
    /// Total attribute length.
    pub len: u32,
    /// Name length in UTF-16 units (named streams).
    pub name_len: u8,
    /// Name offset.
    pub name_offset: u16,
    /// Specific (resident/non-resident) part.
    pub specific: AttrSpecific,
}

impl AttrHeader {
    /// Parse the attribute header at `offset` in `data`.
    pub fn parse(data: &[u8], offset: usize) -> Result<Self> {
        let need = 16;
        if offset + need > data.len() {
            return Err(Error::truncated(
                offset,
                need,
                data.len().saturating_sub(offset),
            ));
        }
        let len = u32::from_le_bytes(data[offset + 4..offset + 8].try_into().expect("4"));
        let non_resident = data[offset + 8];
        let name_len = data[offset + 9];
        let name_offset = u16::from_le_bytes([data[offset + 10], data[offset + 11]]);
        if len < 24 || offset + len as usize > data.len() {
            return Err(Error::BadAttribute {
                ty: 0,
                offset,
                reason: format!("length {len} out of bounds"),
            });
        }
        let specific = if non_resident == 0 {
            let value_len =
                u32::from_le_bytes(data[offset + 16..offset + 20].try_into().expect("4"));
            let value_offset = u16::from_le_bytes([data[offset + 20], data[offset + 21]]);
            AttrSpecific::Resident(ResidentHeader {
                value_len,
                value_offset,
            })
        } else {
            if offset + 64 > data.len() {
                return Err(Error::truncated(
                    offset,
                    64,
                    data.len().saturating_sub(offset),
                ));
            }
            let start_vcn =
                u64::from_le_bytes(data[offset + 16..offset + 24].try_into().expect("8"));
            let last_vcn =
                u64::from_le_bytes(data[offset + 24..offset + 32].try_into().expect("8"));
            let runs_offset = u16::from_le_bytes([data[offset + 32], data[offset + 33]]);
            let alloc_size =
                u64::from_le_bytes(data[offset + 40..offset + 48].try_into().expect("8"));
            let real_size =
                u64::from_le_bytes(data[offset + 48..offset + 56].try_into().expect("8"));
            let init_size =
                u64::from_le_bytes(data[offset + 56..offset + 64].try_into().expect("8"));
            AttrSpecific::NonResident(NonResidentHeader {
                start_vcn,
                last_vcn,
                runs_offset,
                alloc_size,
                real_size,
                init_size,
            })
        };
        Ok(Self {
            offset,
            len,
            name_len,
            name_offset,
            specific,
        })
    }
}

/// $STANDARD_INFORMATION parsed value.
#[derive(Debug, Clone, Copy)]
pub struct StandardInformation {
    /// Creation FILETIME.
    pub created: i64,
    /// Last modification FILETIME.
    pub modified: i64,
    /// MFT-record modification FILETIME.
    pub mft_modified: i64,
    /// Last access FILETIME.
    pub accessed: i64,
    /// DOS file permission bits.
    pub file_permissions: u32,
}

impl StandardInformation {
    /// Parse a 48-byte (v1+) $STANDARD_INFORMATION value.
    pub fn parse(v: &[u8]) -> Result<Self> {
        if v.len() < 48 {
            return Err(Error::truncated(0, 48, v.len()));
        }
        Ok(Self {
            created: i64::from_le_bytes(v[0..8].try_into().expect("8")),
            modified: i64::from_le_bytes(v[8..16].try_into().expect("8")),
            mft_modified: i64::from_le_bytes(v[16..24].try_into().expect("8")),
            accessed: i64::from_le_bytes(v[24..32].try_into().expect("8")),
            file_permissions: u32::from_le_bytes(v[32..36].try_into().expect("4")),
        })
    }
}

/// $FILE_NAME parsed value (namespace + parent link + name).
#[derive(Debug, Clone)]
pub struct FileNameAttr {
    /// Parent directory file reference (FRN, lower 48 bits meaningful).
    pub parent_frn: u64,
    /// Creation FILETIME.
    pub created: i64,
    /// Modification FILETIME.
    pub modified: i64,
    /// Access FILETIME.
    pub accessed: i64,
    /// Allocated size.
    pub alloc_size: u64,
    /// Real size.
    pub real_size: u64,
    /// File flags (FILE_ATTRIBUTE_* compatible).
    pub flags: u32,
    /// Name namespace (0=POSIX,1=WIN32,2=DOS,3=WIN32&DOS).
    pub namespace: u8,
    /// Name (UTF-16 decoded lossy-free; invalid surrogates are errors).
    pub name: String,
}

impl FileNameAttr {
    /// Parse a $FILE_NAME attribute value.
    pub fn parse(v: &[u8]) -> Result<Self> {
        if v.len() < 66 {
            return Err(Error::truncated(0, 66, v.len()));
        }
        let parent_frn = u64::from_le_bytes(v[0..8].try_into().expect("8"));
        let created = i64::from_le_bytes(v[8..16].try_into().expect("8"));
        let modified = i64::from_le_bytes(v[16..24].try_into().expect("8"));
        let accessed = i64::from_le_bytes(v[24..32].try_into().expect("8"));
        let alloc_size = u64::from_le_bytes(v[32..40].try_into().expect("8"));
        let real_size = u64::from_le_bytes(v[40..48].try_into().expect("8"));
        let flags = u32::from_le_bytes(v[48..52].try_into().expect("4"));
        let name_len = v[64] as usize;
        let namespace = v[65];
        let name_bytes = name_len * 2;
        if 66 + name_bytes > v.len() {
            return Err(Error::truncated(66, name_bytes, v.len().saturating_sub(66)));
        }
        let units: Vec<u16> = v[66..66 + name_bytes]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let name = String::from_utf16(&units).map_err(|_| Error::BadAttribute {
            ty: 0x30,
            offset: 0,
            reason: "invalid UTF-16 name".into(),
        })?;
        Ok(Self {
            parent_frn,
            created,
            modified,
            accessed,
            alloc_size,
            real_size,
            flags,
            namespace,
            name,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_file_name_value() {
        let mut v = vec![0u8; 72]; // 66 header bytes + 3 UTF-16 name units
        v[0..8].copy_from_slice(&0x1000u64.to_le_bytes()); // parent FRN
        v[40..48].copy_from_slice(&1234u64.to_le_bytes()); // real size
        v[48..52].copy_from_slice(&0x20u32.to_le_bytes()); // archive flag
        v[64] = 3; // name len
        v[65] = 1; // WIN32 namespace
        let name: [u16; 3] = [b'a' as u16, b'b' as u16, b'c' as u16];
        for (i, u) in name.iter().enumerate() {
            v[66 + i * 2..66 + i * 2 + 2].copy_from_slice(&u.to_le_bytes());
        }
        let f = FileNameAttr::parse(&v).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(f.name, "abc");
        assert_eq!(f.real_size, 1234);
        assert_eq!(f.parent_frn, 0x1000);
        assert_eq!(f.namespace, 1);
    }

    #[test]
    fn truncated_file_name_is_typed() {
        assert!(matches!(
            FileNameAttr::parse(&[0u8; 30]),
            Err(Error::Truncated { .. })
        ));
    }
}
