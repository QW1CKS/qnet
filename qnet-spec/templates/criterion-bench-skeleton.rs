// Criterion bench skeleton for T6.6 (to be placed in the code repo, not here)
// cargo bench --bench aead --features perf-bench

use criterion::{criterion_group, criterion_main, Criterion, Throughput, black_box};

fn bench_aead(c: &mut Criterion) {
    let mut group = c.benchmark_group("aead_chacha20poly1305");
    for size in [1024usize, 4096, 16384, 65536, 1_048_576] { // 1KiB..1MiB
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_function(format!("seal_{}b", size), |b| {
            b.iter(|| {
                // TODO: allocate BytesMut, seal in-place
                let mut buf = vec![0u8; size];
                black_box(&mut buf);
                // seal(buf)
            })
        });
        group.bench_function(format!("open_{}b", size), |b| {
            b.iter(|| {
                let mut buf = vec![0u8; size];
                black_box(&mut buf);
                // open(buf)
            })
        });
    }
    group.finish();
}

fn bench_framing(c: &mut Criterion) {
    let mut group = c.benchmark_group("l2_framing");
    for size in [1024usize, 4096, 16384, 65536, 1_048_576] {
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_function(format!("encode_{}b", size), |b| {
            b.iter(|| {
                // TODO: encode frame with/without padding; ensure ≤1 allocation
            })
        });
        group.bench_function(format!("decode_{}b", size), |b| {
            b.iter(|| {
                // TODO: decode frame; validate AAD/tag
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_aead, bench_framing);
criterion_main!(benches);
