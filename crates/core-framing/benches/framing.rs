use core_framing as framing;
use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};

fn bench_framing(c: &mut Criterion) {
    let mut group = c.benchmark_group("l2_framing");
    for size in [1024usize, 4096, 16384, 65536, 1_048_576] {
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_function(format!("encode_{}b", size), |b| {
            b.iter(|| {
                let payload = vec![0u8; size];
                let f = framing::Frame {
                    ty: framing::FrameType::Stream,
                    payload,
                };
                let _w = framing::Frame::encode_plain(&f);
                black_box(())
            })
        });
        group.bench_function(format!("encode_aead_{}b", size), |b| {
            b.iter(|| {
                let payload = vec![0u8; size];
                let f = framing::Frame {
                    ty: framing::FrameType::Stream,
                    payload,
                };
                let key = framing::KeyCtx { key: [7u8; 32] };
                let nonce = [9u8; 12];
                let _w = framing::encode(&f, key, nonce);
                black_box(())
            })
        });
        group.bench_function(format!("encode_aead_zerocopy_{}b", size), |b| {
            b.iter(|| {
                let payload = vec![0u8; size];
                let f = framing::Frame {
                    ty: framing::FrameType::Stream,
                    payload,
                };
                let key = framing::KeyCtx { key: [7u8; 32] };
                let nonce = [9u8; 12];
                let _w = framing::encode_zerocopy(&f, key, nonce);
                black_box(())
            })
        });
        group.bench_function(format!("decode_aead_{}b", size), |b| {
            let payload = vec![0u8; size];
            let f = framing::Frame {
                ty: framing::FrameType::Stream,
                payload,
            };
            let key = framing::KeyCtx { key: [7u8; 32] };
            let nonce = [9u8; 12];
            let w = framing::encode(&f, key, nonce);
            b.iter(|| {
                let _ = framing::decode(&w, key, nonce).unwrap();
                black_box(())
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_framing);
criterion_main!(benches);
