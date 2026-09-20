//! FIX-L bench suite (docs/16 § 2/§ 4/§ 5): agg, viz layout frames, and
//! tree paging over a deterministic synthetic arena. Budgets are R1 targets;
//! CI runs are informational (documented in docs/phases/phase-status.md).

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use prism_core::viz;
use prism_types::viz::{VizMode, VizOptions};

fn bench_agg(c: &mut Criterion) {
    let (arena, _root, ext) = prism_benches::synth_arena(120_000);
    let mut group = c.benchmark_group("agg");
    group.throughput(Throughput::Elements(arena.len() as u64));
    group.bench_function("compute/120k-nodes", |b| {
        b.iter(|| {
            let a = prism_core::agg::compute(&arena, &ext);
            std::hint::black_box(a.total_files);
        })
    });
    group.finish();
}

fn bench_viz(c: &mut Criterion) {
    for size in [20_000u32, 300_000] {
        let (arena, root, _ext) = prism_benches::synth_arena(size as usize);
        let opts = VizOptions::default();
        let mut group = c.benchmark_group("viz");
        group.throughput(Throughput::Elements(size as u64));
        for (mode, name) in [
            (VizMode::Treemap, "treemap"),
            (VizMode::Sunburst, "sunburst"),
            (VizMode::Icicle, "icicle"),
            (VizMode::Pack, "pack"),
            (VizMode::Mindmap, "mindmap"),
            (VizMode::AgeTimeline, "age-timeline"),
        ] {
            group.bench_function(format!("{name}/{size}"), |b| {
                b.iter(|| {
                    let frame = viz::layout(&arena, 1, root, mode, (1920.0, 1080.0, 2.0), &opts)
                        .unwrap_or_default();
                    std::hint::black_box(frame.len());
                })
            });
        }
        group.finish();
    }
}

fn bench_tree(c: &mut Criterion) {
    let (arena, root, _ext) = prism_benches::synth_arena(300_000);
    let mut group = c.benchmark_group("tree");
    // Children page: sort by allocated desc + page slice (the tree:children
    // hot path shape — the ipc layer pages ≤ 1000 rows, PRISM-IPC-020).
    group.throughput(Throughput::Elements(300_000));
    group.bench_function("children-page/300k-arena", |b| {
        b.iter(|| {
            let n = arena.len();
            let mut rows: Vec<(u32, u64)> =
                (0..n).map(|i| (i as u32, arena.allocated(i))).collect();
            rows.sort_by_key(|r| std::cmp::Reverse(r.1));
            let page: Vec<(u32, u64)> = rows.iter().take(500).cloned().collect();
            std::hint::black_box(page.len());
            std::hint::black_box(root);
        })
    });
    group.finish();
}

criterion_group!(benches, bench_agg, bench_viz, bench_tree);
criterion_main!(benches);
