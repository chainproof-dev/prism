//! Export (PRISM-HG-090, docs/07 § 7): CSV (RFC 4180, UTF-8 BOM) and NDJSON.
//!
//! Streamed writes (bounded row buffer → write flush) so a 10M-file scan
//! never buffers the whole export in memory. Scope: full scan / selection /
//! current filter. Runs on a worker thread; the napi layer awaits join.

use std::io::Write;
use std::path::Path;

use prism_types::commands::{ExportFormat, ExportResult, ExportScope};

use crate::arena::{Arena, NodeId, kind};
use crate::error::EngineError;
use crate::ipc::node_path;

/// Rows buffered before each flush.
const FLUSH_ROWS: usize = 4_096;

/// Run an export over a completed scan's arena.
#[allow(clippy::too_many_arguments)] // arg list mirrors the wire DTO fields
pub fn export(
    arena: &Arena,
    ext_table: &crate::agg::ExtensionTable,
    root_path: &str,
    selected: &[NodeId],
    filter_matched: Option<&[NodeId]>,
    format: ExportFormat,
    scope: ExportScope,
    dest: &Path,
) -> Result<ExportResult, EngineError> {
    let file = std::fs::File::create(dest).map_err(|e| EngineError::Io {
        path: dest.to_string_lossy().into_owned(),
        source: e,
    })?;
    let mut w = std::io::BufWriter::with_capacity(256 * 1024, file);

    let mut rows: u64 = 0;
    let mut bytes_written: u64 = 0;

    let emit = |row: String,
                w: &mut dyn Write,
                rows: &mut u64,
                bytes: &mut u64|
     -> Result<(), EngineError> {
        *rows += 1;
        *bytes += row.len() as u64 + 1;
        w.write_all(row.as_bytes())
            .and_then(|_| w.write_all(b"\n"))
            .map_err(|e| EngineError::Io {
                path: dest.to_string_lossy().into_owned(),
                source: e,
            })
    };

    match format {
        ExportFormat::Csv => {
            // UTF-8 BOM (Excel opens UTF-8 CSV correctly only with it).
            w.write_all(&[0xEF, 0xBB, 0xBF]).map_err(io(dest))?;
            let header =
                "path,kind,logical_bytes,allocated_bytes,files,folders,extension,modified_ms";
            w.write_all(header.as_bytes())
                .and_then(|_| w.write_all(b"\n"))
                .map_err(io(dest))?;
            bytes_written += header.len() as u64 + 4; // header + BOM + newline
        }
        ExportFormat::Ndjson => {}
    }

    match scope {
        ExportScope::Full => {
            for id in 0..arena.len() as NodeId {
                let k = arena.kind(id);
                if k == kind::FREE_SPACE || k == kind::UNKNOWN {
                    continue;
                }
                let s = row_for(arena, ext_table, root_path, id, format);
                emit(s, &mut w, &mut rows, &mut bytes_written)?;
            }
        }
        ExportScope::Selection => {
            for &n in selected {
                let s = row_for(arena, ext_table, root_path, n, format);
                emit(s, &mut w, &mut rows, &mut bytes_written)?;
            }
        }
        ExportScope::Filtered => {
            let matched = filter_matched.unwrap_or(&[]);
            for &n in matched {
                let s = row_for(arena, ext_table, root_path, n, format);
                emit(s, &mut w, &mut rows, &mut bytes_written)?;
            }
        }
    }

    w.flush().map_err(io(dest))?;
    if rows.is_multiple_of(FLUSH_ROWS as u64) {
        // final flush already done above; sync only the file handle
        w.get_ref().sync_all().ok();
    }
    Ok(ExportResult {
        bytes_written,
        rows,
    })
}

fn io(dest: &Path) -> impl Fn(std::io::Error) -> EngineError + '_ {
    move |e| EngineError::Io {
        path: dest.to_string_lossy().into_owned(),
        source: e,
    }
}

/// Serialize one node in the requested format.
fn row_for(
    arena: &Arena,
    ext_table: &crate::agg::ExtensionTable,
    root_path: &str,
    n: NodeId,
    format: prism_types::commands::ExportFormat,
) -> String {
    let path = node_path(arena, n, root_path);
    let kind_s = kind_label(arena.kind(n));
    let mtime = if arena.mtime(n) > 0 {
        crate::scanner::filetime_ticks_to_unix_ms(arena.mtime(n))
    } else {
        0
    };
    match format {
        prism_types::commands::ExportFormat::Csv => {
            let ext = ext_table.display(arena.ext_id(n)).to_string();
            format!(
                "{},{},{},{},{},{},{},{}",
                csv_escape(&path),
                kind_s,
                arena.logical(n),
                arena.allocated(n as usize),
                arena.files(n),
                arena.folders(n),
                csv_escape(&ext),
                mtime
            )
        }
        prism_types::commands::ExportFormat::Ndjson => {
            let ext = ext_table.display(arena.ext_id(n)).to_string();
            serde_json::json!({
                "path": path,
                "kind": kind_s,
                "logicalBytes": arena.logical(n).to_string(),
                "allocatedBytes": arena.allocated(n as usize).to_string(),
                "files": arena.files(n),
                "folders": arena.folders(n),
                "ext": ext,
                "modifiedMs": mtime.to_string(),
            })
            .to_string()
        }
    }
}

fn kind_label(k: u8) -> &'static str {
    match k {
        kind::FILE => "file",
        kind::DIR => "dir",
        kind::REPARSE => "reparse",
        kind::MOUNT => "mount",
        kind::LINK => "link",
        kind::ROOT => "root",
        _ => "node",
    }
}

/// RFC 4180: quote when the field contains comma, quote, CR or LF; double
/// embedded quotes.
fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agg::ExtensionTable;
    use crate::arena::NodeInput;

    fn ext_table() -> ExtensionTable {
        let mut t = ExtensionTable::default();
        t.intern("txt");
        t
    }

    fn arena_with_file() -> Arena {
        let mut a = Arena::with_capacity(8);
        let root = a.push(NodeInput {
            parent: 0,
            name_utf16: &"C:".encode_utf16().collect::<Vec<_>>(),
            logical: 0,
            allocated: 0,
            files: 1,
            folders: 0,
            mtime: 0,
            kind: kind::ROOT,
            category: 0,
            ext_id: 0,
            attr_flags: 0,
            link_to: 0,
            err_code: 0,
        });
        let f = a.push(NodeInput {
            parent: root,
            name_utf16: &"a, file \"quoted\".txt".encode_utf16().collect::<Vec<_>>(),
            logical: 123,
            allocated: 128,
            files: 1,
            folders: 0,
            mtime: 0,
            kind: kind::FILE,
            category: 0,
            ext_id: 0,
            attr_flags: 0,
            link_to: 0,
            err_code: 0,
        });
        a.attach(root, f);
        a.finalize_children();
        a
    }

    #[test]
    fn csv_escapes_and_boms() {
        let a = arena_with_file();
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        let dest = dir.path().join("out.csv");
        let res = export(
            &a,
            &ext_table(),
            "C:\\",
            &[],
            None,
            ExportFormat::Csv,
            ExportScope::Full,
            &dest,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(res.rows, 2, "root + file");
        let text = std::fs::read(&dest).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(&text[..3], &[0xEF, 0xBB, 0xBF], "UTF-8 BOM present");
        let s = String::from_utf8_lossy(&text);
        assert!(
            s.contains("a, file \"\"quoted\"\".txt"),
            "RFC4180 escaping: {s}"
        );
    }

    #[test]
    fn ndjson_selection_scope() {
        let a = arena_with_file();
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        let dest = dir.path().join("out.ndjson");
        export(
            &a,
            &ext_table(),
            "C:\\",
            &[1],
            None,
            ExportFormat::Ndjson,
            ExportScope::Selection,
            &dest,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let s = std::fs::read_to_string(&dest).unwrap_or_else(|e| panic!("{e}"));
        let lines: Vec<&str> = s.lines().collect();
        assert_eq!(lines.len(), 1);
        let v: serde_json::Value = serde_json::from_str(lines[0]).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(v["logicalBytes"], "123");
        assert_eq!(v["kind"], "file");
    }
}
