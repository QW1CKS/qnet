use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use htx::api::dial_inproc_secure;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

fn bench_stream(c: &mut Criterion) {
    let mut group = c.benchmark_group("htx_stream");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_millis(1500));
    for size in [1024usize, 4096, 16384, 65536] {
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_function(format!("write_read_{}b", size), |b| {
            // Build an in-process secure connection and spawn echo server once
            let (client, server) = dial_inproc_secure();
            let running = Arc::new(AtomicBool::new(true));
            let rflag = running.clone();
            let echo = std::thread::spawn(move || {
                // Accept and service new streams continuously while running
                'outer: loop {
                    if !rflag.load(Ordering::Relaxed) {
                        break;
                    }
                    if let Some(s) = server.accept_stream(1_000) {
                        loop {
                            if let Some(buf) = s.read() {
                                s.write(&buf);
                            } else if !rflag.load(Ordering::Relaxed) {
                                break 'outer;
                            } else {
                                // read timeout; assume stream ended or no data forthcoming
                                break;
                            }
                        }
                    }
                }
            });
            let buf = vec![0u8; size];
            b.iter(|| {
                // open a fresh stream per iteration to avoid flow-control accumulation
                let st = client.open_stream();
                // Write in chunks to avoid flow-control stalls
                let chunk = 1024.min(size);
                let mut off = 0usize;
                while off < size {
                    let end = (off + chunk).min(size);
                    st.write(&buf[off..end]);
                    off = end;
                }
                // Read until the entire payload is echoed back
                let mut got = 0usize;
                let mut tmp_bytes = 0usize;
                let start = std::time::Instant::now();
                while got < size {
                    if let Some(chunk) = st.read() {
                        tmp_bytes = chunk.len();
                        got += chunk.len();
                    } else if start.elapsed() > std::time::Duration::from_secs(10) {
                        // Safety guard: avoid indefinite stalls in warm-up
                        break;
                    }
                }
                // ensure at least one chunk arrived
                assert!(tmp_bytes > 0);
                black_box(got);
            });
            // signal echo thread to stop and join
            running.store(false, Ordering::Relaxed);
            let _ = echo.join();
        });
    }
    group.finish();
}

criterion_group!(benches, bench_stream);
criterion_main!(benches);
