use criterion::{criterion_group, criterion_main, Criterion, black_box};
use htx::api::dial_inproc_secure;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::Duration;

fn bench_mixed(c: &mut Criterion) {
    let mut g = c.benchmark_group("htx_stream_mixed");
    g.sample_size(10);
    g.warm_up_time(Duration::from_millis(300));
    g.measurement_time(Duration::from_millis(1200));

    g.bench_function("small_vs_large_concurrent", |b| {
        // RR scheduler can be forced via env for this bench
        std::env::set_var("HTX_SCHEDULER_RR", "1");
        std::env::set_var("HTX_SCHEDULER_PROFILE", "http");

        let (client, server) = dial_inproc_secure();
        let running = Arc::new(AtomicBool::new(true));
        let rflag = running.clone();
        let echo = std::thread::spawn(move || {
            while rflag.load(Ordering::Relaxed) {
                if let Some(s) = server.accept_stream(5) {
                    std::thread::spawn(move || {
                        while let Some(buf) = s.read() { s.write(&buf); }
                    });
                }
            }
        });
        b.iter(|| {
            let small = vec![3u8; 8 * 1024];
            let large = vec![4u8; 512 * 1024];
            let c1 = client.clone();
            let h_small = std::thread::spawn(move || {
                let sh = c1.open_stream();
                sh.write(&small);
                let mut got = 0usize;
                while let Some(buf) = sh.read() { got += buf.len(); if got >= small.len() { break; } }
                got
            });
            let c2 = client.clone();
            let h_large = std::thread::spawn(move || {
                let sh = c2.open_stream();
                sh.write(&large);
                let mut got = 0usize;
                let start = std::time::Instant::now();
                while let Some(buf) = sh.read() { got += buf.len(); if got >= large.len() || start.elapsed() > Duration::from_secs(3) { break; } }
                got
            });
            let small_done = h_small.join().unwrap();
            let large_done = h_large.join().unwrap();
            assert_eq!(small_done, 8 * 1024);
            black_box(large_done);
        });
        running.store(false, Ordering::Relaxed);
        let _ = echo.join();
    });
    g.finish();
}

criterion_group!(benches, bench_mixed);
criterion_main!(benches);
