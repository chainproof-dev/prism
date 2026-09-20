//! Development-only POSIX enumerator (cfg(unix)). Labeled honestly as
//! `scanner-posix-dev` in `sys:hello` — it exists so the entire engine and UI
//! stack can be developed and tested on non-Windows workstations; Windows
//! release builds do not compile this file at all (docs/amendments/
//! AMM-002-dev-platform.md). Uses std::fs (metadata in the same syscall
//! family as the Win32 one-batch design via `dirent` + `statx`).

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::MetadataExt;

use super::{DirBatch, DirEnumerator, DirTask, EntryClass, EntryMeta, FileIdentity};

const IO_REPARSE_LIKE_SYMLINK: u32 = 1; // synthetic tag for symlinks

/// POSIX dev backend.
pub struct PosixDevEnumerator {
    volume_serial: u32,
}

impl PosixDevEnumerator {
    /// New enumerator with the device serial for identity.
    pub fn new(volume_serial: u32) -> Self {
        Self { volume_serial }
    }
}

impl DirEnumerator for PosixDevEnumerator {
    // Integer division is exact: ns → 100ns ticks.
    #[allow(clippy::integer_division)]
    fn enumerate(&mut self, task: &DirTask) -> DirBatch {
        let mut batch = DirBatch {
            dir: task.node,
            count: 0,
            names: Vec::with_capacity(64),
            name_off: Vec::with_capacity(64),
            metas: Vec::with_capacity(64),
            error: None,
        };
        let path = String::from_utf16_lossy(&task.path);
        let entries = match fs::read_dir(&path) {
            Ok(e) => e,
            Err(e) => {
                let code = e.raw_os_error().unwrap_or(0);
                let msg = if code == 13 {
                    "access denied"
                } else {
                    "read_dir failed"
                };
                return DirBatch::error_batch(task.node, code, msg.to_string());
            }
        };
        for entry in entries.flatten() {
            let name_os = entry.file_name();
            let name_lossy = name_os.to_string_lossy().into_owned();
            let name16: Vec<u16> = name_lossy.encode_utf16().collect();
            if name16.len() <= 2
                && name16.first() == Some(&(b'.' as u16))
                && (name16.len() == 1 || name16[1] == b'.' as u16)
            {
                continue; // . and ..
            }
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => {
                    // Stat failure on one entry does not abort the directory
                    // (parity-SCN-05 class): push an unknown-size entry.
                    batch.name_off.push(batch.names.len() as u32);
                    batch.names.extend_from_slice(&name16);
                    batch.names.push(0);
                    batch.metas.push(EntryMeta {
                        size: 0,
                        alloc: 0,
                        mtime: 0,
                        attrs: 0,
                        reparse: 0,
                        file_id: FileIdentity {
                            volume_serial: 0,
                            file_id: 0,
                        },
                        kind: EntryClass::File,
                    });
                    continue;
                }
            };
            let ft = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok());
            let mtime_ticks = ft
                .map(|d| (d.as_nanos() / 100) as i64 + super::EPOCH_OFFSET_TICKS)
                .unwrap_or(0);
            let is_symlink = entry.file_type().map(|t| t.is_symlink()).unwrap_or(false);
            let kind = if is_symlink {
                EntryClass::Reparse(IO_REPARSE_LIKE_SYMLINK)
            } else if meta.is_dir() {
                EntryClass::Dir
            } else {
                EntryClass::File
            };
            let alloc = meta.blocks().saturating_mul(512); // st_blocks × 512B
            batch.name_off.push(batch.names.len() as u32);
            batch.names.extend_from_slice(&name16);
            batch.names.push(0);
            batch.metas.push(EntryMeta {
                size: meta.len(),
                alloc,
                mtime: mtime_ticks,
                attrs: if meta.is_dir() { 0x10 } else { 0x80 },
                reparse: u32::from(is_symlink),
                file_id: FileIdentity {
                    volume_serial: self.volume_serial,
                    file_id: meta.ino(),
                },
                kind,
            });
        }
        batch.count = batch.metas.len();
        batch
    }
}
