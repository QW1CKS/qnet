use criterion::{black_box, criterion_group, criterion_main, Criterion};
use curve25519_dalek::scalar::Scalar;
use htx::Handshake;

fn bench_handshake(c: &mut Criterion) {
    c.bench_function("htx_noise_xk_loopback", |b| {
        b.iter(|| {
            // Deterministic static keys
            let si = Scalar::from_bytes_mod_order([1u8; 32]);
            let sr = Scalar::from_bytes_mod_order([2u8; 32]);
            let rs = (sr * curve25519_dalek::constants::X25519_BASEPOINT).to_bytes();

            let mut init = Handshake::init_initiator(si, rs);
            let mut resp = Handshake::init_responder(sr);

            let m1 = init.next(None).unwrap().unwrap();
            let m2 = resp.next(Some(&m1)).unwrap().unwrap();
            let m3 = init.next(Some(&m2)).unwrap().unwrap();
            let _ = resp.next(Some(&m3)).unwrap();

            assert!(init.is_done() && resp.is_done());
            black_box(init.transport_keys());
        })
    });
}

criterion_group!(benches, bench_handshake);
criterion_main!(benches);
