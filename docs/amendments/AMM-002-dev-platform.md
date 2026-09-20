# AMM-002 — Development-platform enumeration backend

**Status:** Accepted · **Affects:** docs/06-RUST-CORE.md § 2, docs/04 ADR-06 · **Phase:** 1 (P1-001)

## Context

The engine is Windows-only in v1 **by decision** (docs/06 § Platform boundary).
Build agents and CI run on non-Windows workstations for the Rust/TS core
development loop. The scanner's Win32 fast path (`NtQueryDirectoryFile`)
cannot run there, which would leave the coordinator, arena, aggregation, viz
layout, and IPC layers untestable outside Windows.

## Decision

A **development-only POSIX enumeration backend** (`scanner/posix.rs`,
`cfg(unix)`) implements the same `DirEnumerator` trait with `std::fs`. It is:

1. **Build-time selected** — `cfg(unix)`; Windows release builds do not compile
   it at all. This is a capability split, not a runtime fallback (A2 applies
   to runtime substitution, not build targets).
2. **Honestly labeled** — `sys:hello` reports `scanner-posix-dev` in
   `features`; the Welcome screen shows the dev badge when present.
3. **Feature-equivalent at the contract level** — emits the same `DirBatch`
   structure (flat UTF-16 name buffer + parallel metadata), the same
   `FileIdentity` (device serial, inode), the same mtime conversion, so all
   downstream layers are exercised identically.

## Consequences

- The full E2E suite (`crates/prism-core/tests/scan_e2e.rs`) runs on Linux CI
  against the real coordinator.
- Windows-only surfaces (`IFileOperation`, SHIL icons, MFT volume handles)
  remain `cfg(windows)` and are validated by `cargo check --target
  x86_64-pc-windows-msvc` plus the Windows CI matrix.
- No product behavior differs: the shipped Windows build contains exactly one
  scanner backend.
