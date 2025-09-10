# Performance Summary Template

Use this file to record each benchmark run for T6.6. Commit alongside Criterion reports and attach to CI artifacts.

## Run Metadata
- Date/Time (UTC):
- Commit SHA:
- Branch:
- Runner/Host:

## Hardware Profile
- CPU model / cores / threads:
- RAM:
- Storage:
- NIC / Network setup:
- OS / Kernel:

## Environment
- CPU governor:
- Power settings:
- Background load:
- Rust toolchain:
- Features enabled: (perf-bench, quic, etc.)

## Benchmarks
- core-crypto (ChaCha20-Poly1305):
  - 1KiB:
  - 4KiB:
  - 16KiB:
  - 64KiB:
  - 1MiB:
- core-framing (encode/decode): allocations/frame, ns/op
- HTX handshake: median/mean, p95, CPU time
- Stream throughput: p50/p95 (TCP), p50/p95 (QUIC)
- Mixnet (latency-mode=low): added latency p50/p95

## Results vs Baseline
- Throughput delta:
- Latency delta:
- Notes:

## Regressions
- Any metric worse than threshold? (throughput >10% drop, latency >15% increase)
- Root cause hypothesis:
- Follow-up tasks:

## Summary
- Pass/Fail:
- Key takeaways:
- Next actions:
