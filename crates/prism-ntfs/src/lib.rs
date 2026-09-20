//! # prism-ntfs — raw NTFS parsing (turbo scan)
//!
//! MFT record parsing, run-list decoding, attribute walking, USN journal
//! enumeration. Pure parsing — no volume handles here (the engine's sys layer
//! owns I/O; docs/06 § 3). Parser functions are fuzz targets (docs/06 § 12).
//!
//! v1 scope note (docs/18 Phase 5): the on-disk format readers ship in this
//! crate; the turbo scan pipeline wiring lands in Phase 5 with the elevation
//! helper. The parsers are complete and tested against synthetic records now.

#![deny(missing_docs)]

pub mod attrs;
pub mod error;
pub mod mft;
pub mod runs;

pub use error::Error;
