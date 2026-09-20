//! # PRISM type source (single source of truth)
//!
//! Every struct in this crate is the authoritative definition for one side of
//! an IPC contract. The `emit-ts` binary parses *these sources* and generates
//! matching TypeScript into `packages/shared/src/generated/` — hand edits
//! there are forbidden (`PRISM-IPC-001`).
//!
//! Field-type conventions (docs/05-IPC-PROTOCOL.md):
//! - byte counts: `u64` → TS `bigint` (crosses NAPI as BigInt)
//! - timestamps: `i64` (unix ms) or FILETIME ticks where noted → TS `number`
//! - ids: `u32` node ids, `u64` scan ids → TS `number` / `bigint` respectively
//!
//! Naming: Rust identifiers are snake_case; serde renames to camelCase for the
//! wire so both ecosystems read naturally.

#![deny(missing_docs)]

pub mod commands;
pub mod events;
pub mod filter;
pub mod ids;
pub mod licensing;
pub mod scan;
pub mod sys;
pub mod types_list;
pub mod viz;

pub use commands::COMMANDS;
