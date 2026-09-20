//! Errors for prism-ntfs.

/// Parser error taxonomy. All parsers return this — no panics (docs/06 § 10).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Buffer too short for the structure being parsed.
    #[error("truncated: need {need} bytes at offset {offset:#x}, have {have}")]
    Truncated {
        /// Bytes needed.
        need: usize,
        /// Absolute offset in the input buffer.
        offset: usize,
        /// Bytes available.
        have: usize,
    },
    /// Magic/signature mismatch.
    #[error("bad magic {expected} at offset {offset:#x}")]
    BadMagic {
        /// Expected magic string.
        expected: &'static str,
        /// Absolute offset.
        offset: usize,
    },
    /// Malformed run list.
    #[error("malformed run list at offset {offset:#x}: {reason}")]
    BadRunList {
        /// Absolute offset.
        offset: usize,
        /// Why.
        reason: &'static str,
    },
    /// Attribute type/length impossible.
    #[error("malformed attribute {ty:#x} at offset {offset:#x}: {reason}")]
    BadAttribute {
        /// Attribute type.
        ty: u32,
        /// Absolute offset.
        offset: usize,
        /// Why.
        reason: String,
    },
    /// Fixup (update sequence) validation failed.
    #[error("fixup mismatch at index {index}")]
    FixupMismatch {
        /// Fixup slot index.
        index: usize,
    },
    /// Value out of the valid range.
    #[error("{what} out of range: {value}")]
    OutOfRange {
        /// What.
        what: &'static str,
        /// Value.
        value: u64,
    },
}

impl Error {
    /// Construct a truncation error with context.
    pub fn truncated(offset: usize, need: usize, have: usize) -> Self {
        Self::Truncated { need, offset, have }
    }
}

/// Result alias.
pub type Result<T> = std::result::Result<T, Error>;
