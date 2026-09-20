//! Arena benchmark (docs/16 § 2: arena build cost ≤ 120 ns/node average).

use criterion::{Criterion, criterion_group, criterion_main};
use prism_core::arena::{Arena, NodeInput, kind};

fn bench_arena_push(c: &mut Criterion) {
    let mut group = c.benchmark_group("arena");
    group.throughput(criterion::Throughput::Elements(1));
    group.bench_function("push+attach", |b| {
        b.iter_batched(
            || {
                let utf16: Vec<u16> = "some-file-name.bin".encode_utf16().collect();
                (Arena::with_capacity(1024), 0u32, utf16)
            },
            |(mut arena, root, name)| {
                let n = arena.push(NodeInput {
                    parent: root,
                    name_utf16: &name,
                    logical: 1234,
                    allocated: 4096,
                    files: 1,
                    folders: 0,
                    mtime: 0,
                    kind: kind::FILE,
                    category: 1,
                    ext_id: 2,
                    attr_flags: 0,
                    link_to: 0,
                    err_code: 0,
                });
                arena.attach(root, n);
            },
            criterion::BatchSize::SmallInput,
        )
    });
    group.finish();
}

criterion_group!(benches, bench_arena_push);
criterion_main!(benches);
