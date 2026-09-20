//! End-to-end scan tests: build a synthetic tree on disk, scan it through the
//! real coordinator (dev backend), and verify the invariants (PRISM-DM-030):
//! Σ children == parent, exclusions prune, cancellation is fast, viz frames lay out.

use std::sync::Arc;

use prism_core::ipc::EngineEventSink;
use prism_core::scanner::coordinator::{CompletedScan, CompletionCb, ScanManager};
use prism_types::scan::{ScanOptions, ScanTarget};
use prism_types::viz::{VizMode, VizOptions};

fn setup_tree() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    std::fs::write(root.join("a.txt"), vec![1u8; 10_000]).unwrap();
    std::fs::write(root.join("b.log"), vec![2u8; 5_000]).unwrap();
    std::fs::create_dir_all(root.join("sub/node_modules/pkg")).unwrap();
    std::fs::write(root.join("sub/node_modules/pkg/x.js"), vec![3u8; 8_000]).unwrap();
    std::fs::write(root.join("sub/big.bin"), vec![4u8; 50_000]).unwrap();
    std::fs::create_dir_all(root.join("empty")).unwrap();
    dir
}

fn run_scan(target: String, options: ScanOptions) -> Arc<CompletedScan> {
    let sink = Arc::new(EngineEventSink::default());
    let manager = ScanManager::new(sink);
    let (done_tx, done_rx) = std::sync::mpsc::channel::<Option<Arc<CompletedScan>>>();
    let tx = done_tx;
    let on_complete: CompletionCb = Arc::new(move |_id, c| {
        tx.send(c).expect("completion send");
    });
    manager
        .start(
            ScanTarget::Folder {
                paths: vec![target],
            },
            prism_types::scan::ScanStrategy::Standard,
            options,
            on_complete,
        )
        .expect("scan start");
    done_rx
        .recv_timeout(std::time::Duration::from_secs(60))
        .expect("scan completion within 60 s")
        .expect("scan succeeded")
}

#[test]
fn scan_completes_with_correct_totals() {
    let dir = setup_tree();
    let target_path = dir.path().to_string_lossy().into_owned();
    let completed = run_scan(target_path, ScanOptions::default());

    // Summary sanity: 5 files, >=3 dirs, 73_000 logical bytes
    assert_eq!(completed.summary.files, 4, "files: {:?}", completed.summary);
    assert!(
        completed.summary.folders >= 3,
        "folders: {:?}",
        completed.summary
    );
    assert_eq!(
        completed.summary.logical, 73_000,
        "logical: {:?}",
        completed.summary
    );

    // Arena invariant (PRISM-DM-030.1): Σ child.allocated == parent.allocated
    let arena = &completed.arena;
    for node in 0..arena.len() as u32 {
        let children = arena.children(node);
        if children.is_empty() {
            continue;
        }
        let kind = arena.kind(node);
        if kind == prism_core::arena::kind::FREE_SPACE || kind == prism_core::arena::kind::UNKNOWN {
            continue; // pseudo nodes carry their own semantics
        }
        let sum: u64 = children
            .iter()
            .map(|&c| arena.allocated(c as usize))
            .zip(children.iter().map(|&c| arena.kind(c)))
            .filter(|(_, k)| {
                *k != prism_core::arena::kind::FREE_SPACE && *k != prism_core::arena::kind::UNKNOWN
            })
            .map(|(v, _)| v)
            .sum();
        assert_eq!(
            sum,
            arena.allocated(node as usize),
            "node {node} ({}): children sum {sum} != own {}",
            arena.name_str(node),
            arena.allocated(node as usize)
        );
    }
    let root = arena.roots().first().copied().expect("root exists");
    assert!(arena.allocated(root as usize) >= 73_000);
}

#[test]
fn exclusion_prunes_subtree() {
    let dir = setup_tree();
    let target_path = dir.path().to_string_lossy().into_owned();
    let options = ScanOptions {
        exclude_patterns: vec!["**/node_modules/**".to_string()],
        ..ScanOptions::default()
    };
    let completed = run_scan(target_path, options);
    // x.js (8_000 bytes) pruned → 65_000
    assert_eq!(
        completed.summary.logical, 65_000,
        "node_modules pruned: {:?}",
        completed.summary
    );
    let arena = &completed.arena;
    assert!(
        (0..arena.len() as u32).all(|n| arena.name_str(n) != "node_modules"),
        "node_modules must not appear in the arena"
    );
}

#[test]
fn scan_is_deterministic() {
    // Two scans of the same tree produce the identical canonical hash (PRISM-DM-030.5).
    let dir = setup_tree();
    let target = dir.path().to_string_lossy().into_owned();
    let a = run_scan(target.clone(), ScanOptions::default());
    let b = run_scan(target, ScanOptions::default());
    assert_eq!(
        a.arena.canonical_hash(),
        b.arena.canonical_hash(),
        "arena hash must be deterministic"
    );
}

#[test]
fn cancellation_is_fast() {
    let dir = setup_tree();
    let target_path = dir.path().to_string_lossy().into_owned();
    let sink = Arc::new(EngineEventSink::default());
    let manager = ScanManager::new(sink);
    let (done_tx, done_rx) = std::sync::mpsc::channel::<Option<Arc<CompletedScan>>>();
    let tx = done_tx;
    let on_complete: CompletionCb = Arc::new(move |_id, c| {
        tx.send(c).ok();
    });
    let id = manager
        .start(
            ScanTarget::Folder {
                paths: vec![target_path],
            },
            prism_types::scan::ScanStrategy::Standard,
            ScanOptions::default(),
            on_complete,
        )
        .expect("start");
    let t0 = std::time::Instant::now();
    manager.cancel(id).expect("cancel ack");
    let _ = done_rx.recv_timeout(std::time::Duration::from_secs(10));
    assert!(
        t0.elapsed() < std::time::Duration::from_secs(10),
        "cancel must not hang"
    );
}

#[test]
fn viz_frames_lay_out_completed_scan() {
    let dir = setup_tree();
    let target_path = dir.path().to_string_lossy().into_owned();
    let completed = run_scan(target_path, ScanOptions::default());
    let arena = &completed.arena;
    let root = arena.roots().first().copied().expect("root");
    for mode in [
        VizMode::Treemap,
        VizMode::Sunburst,
        VizMode::Icicle,
        VizMode::Pack,
        VizMode::Mindmap,
        VizMode::AgeTimeline,
    ] {
        let frame = prism_core::viz::layout(
            arena,
            1,
            root,
            mode,
            (800.0, 600.0, 2.0),
            &VizOptions::default(),
        )
        .unwrap_or_else(|e| panic!("{mode:?}: {e}"));
        let decoded =
            prism_core::viz::frame::decode(&frame).unwrap_or_else(|e| panic!("{mode:?}: {e}"));
        assert!(!decoded.tiles.is_empty(), "{mode:?} produced no tiles");
    }
}
