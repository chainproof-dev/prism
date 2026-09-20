//! # prism-core — the PRISM engine
//!
//! Scanner, arena, aggregation, visualization layout, licensing verification
//! and the N-API IPC surface. Windows is the release platform (docs/06 §
//! Platform boundary); unix builds carry the labeled development backend
//! (docs/amendments/AMM-002-dev-platform.md).
//!
//! Panic policy: `unwrap`/`expect`/`panic!` are denied by lints in `src/`;
//! every `#[napi]` entry contains unwinds (docs/19 A5).

#![deny(missing_docs)]

pub mod agg;
pub mod apps;
pub mod arena;
pub mod cleanup;
pub mod dupes;
pub mod error;
pub mod export;
pub mod ipc;
pub mod licensing;
pub mod monitor;
pub mod persistence;
pub mod scanner;
pub mod sysinfo;
pub mod viz;

pub use error::{EngineError, Result};

/// Crate version re-export.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
