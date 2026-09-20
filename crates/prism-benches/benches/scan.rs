//! bench-scan (docs/16 § 2): the REAL scan pipeline (posix dev backend on
//! Linux CI, Win32 NT backend on Windows) over a generated on-disk tree.
//! The ≥ 150k files/s standard budget is an R1-hardware gate; CI numbers are
//! informational — the bench exists so the budget is measurable anywhere.

// Criterion-harness context: panics on fixture failure are acceptable (this
// is not the engine; a broken fixture must fail the bench loudly).
#![allow(clippy::expect_used)]

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use std::sync::Arc;

use prism_core::ipc::EngineEventSink;
use prism_core::scanner::coordinator::{CompletedScan, ScanManager};
use prism_types::scan::{ScanOptions, ScanStrategy, ScanTarget};

fn scan_once(manager: &ScanManager, target: &str) -> Arc<CompletedScan> {
    let (done_tx, done_rx) = std::sync::mpsc::channel::<Option<Arc<CompletedScan>>>();
    let tx = done_tx;
    let id = manager
        .start(
            ScanTarget::Folder {
                paths: vec![target.to_string()],
            },
            ScanStrategy::Standard,
            ScanOptions::default(),
            Arc::new(move |_id, c| {
                tx.send(c).expect("completion channel");
            }),
        )
        .expect("scan start");
    let done = done_rx
        .recv_timeout(std::time::Duration::from_secs(120))
        .expect("scan completes")
        .expect("scan ok");
    // Release the single-active-scan lease so the next iteration can start
    // (install the completed scan — `complete`, not the `completed` query).
    manager.complete(id, Some(Arc::clone(&done)));
    done
}

fn bench_scan(c: &mut Criterion) {
    // 10k on-disk files: CI-friendly, still exercises the full walk +
    // arena build + aggregation pipeline.
    const FILES: usize = 10_000;
    let dir = prism_benches::disk_tree(FILES);
    let target = dir.path().to_string_lossy().into_owned();
    let sink = Arc::new(EngineEventSink::default());
    let manager = ScanManager::new(sink);

    // Warm-up run (page cache, allocator, thread pool).
    let warm = scan_once(&manager, &target);
    assert!(warm.summary.files >= FILES as u64 - 64, "fixture intact");

    let mut group = c.benchmark_group("scan");
    group.throughput(Throughput::Elements(warm.summary.files));
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(15));
    group.bench_function("standard/10k-files", |b| {
        b.iter(|| {
            let done = scan_once(&manager, &target);
            std::hint::black_box(done.summary.allocated);
        })
    });
    group.finish();
}

criterion_group!(benches, bench_scan);
criterion_main!(benches);
