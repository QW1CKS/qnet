use criterion::{criterion_group, criterion_main, Criterion, Throughput, black_box};
use rand::{rngs::StdRng, RngCore, SeedableRng};

fn bench_aead(c: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(7);
    let mut key = [0u8; 32];
    let mut nonce = [0u8; 12];
    let mut aad = vec![0u8; 32];
    rng.fill_bytes(&mut key);
    rng.fill_bytes(&mut nonce);
    rng.fill_bytes(&mut aad);

    let mut group = c.benchmark_group("aead_chacha20poly1305");
    for size in [1024usize, 4096, 16384, 65536, 1_048_576] {
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_function(format!("seal_{}b", size), |b| {
            b.iter(|| {
                let mut pt = vec![0u8; size];
                black_box(&mut pt);
                core_crypto::aead::seal(&key, &nonce, &aad, &pt)
            })
        });
        group.bench_function(format!("open_{}b", size), |b| {
            // Precompute a ciphertext to open
            let ct = core_crypto::aead::seal(&key, &nonce, &aad, &vec![0u8; size]);
            b.iter(|| {
                let out = core_crypto::aead::open(&key, &nonce, &aad, &ct).unwrap();
                black_box(out)
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_aead);
criterion_main!(benches);
