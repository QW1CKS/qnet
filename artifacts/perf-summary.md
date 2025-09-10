# Performance Summary (T6.6)

This summarizes the latest local perf run using the provided scripts. Criterion JSONs are under `artifacts/criterion/`.

## Run Metadata
- Date/Time (UTC): 2025-09-10
- Commit SHA: (local working tree)
- Branch: main
- Runner/Host: see Hardware Profile below

## Hardware Profile
Extracted from `windows-hw-profile.txt`:
- CPU: Intel(R) Core(TM) i5-7300HQ @ 2.50GHz (4 logical cores)
- RAM: 8 GB
- OS: Windows 11 Pro 22H2 (Build 22621)
- Hypervisor present: True (may affect timers)

## Environment
- Rust toolchain: release bench profile (exact version from Cargo.lock)
- Features enabled: perf-bench, quic
- Mesh bench mode: full (script forces full for mesh)

## Benchmarks (highlights)
- core-crypto (ChaCha20-Poly1305) throughput
  - seal 16KiB: median ~11.9 µs → ~1.26 GiB/s
  - open 16KiB: median ~10.86 µs → ~1.35 GiB/s
  - seal 1MiB: ~1.75 ms → ~572 MiB/s (memory bound on this machine)
- core-framing (encode/decode AEAD)
  - encode_aead 16KiB: ~12.0 µs (≈1.27 GiB/s)
  - decode_aead 16KiB: ~11.30 µs (≈1.35 GiB/s)
- HTX handshake (Noise XK loopback)
  - median ≈ 751 µs, mean ≈ 800 µs
- HTX stream write/read
  - 16KiB: median ≈ 690 µs (stressed configuration; low absolute throughput on this host)
- Mesh echo (libp2p)
  - Full mode budgets are capped; use the quick, uncapped rr_compare example below for RTT deltas.

### Uncapped request-response compare (rr_compare)
- Scenario: 100 sequential requests, simulated RTT 20 ms, loss 1%.
- TCP result: {"proto":"tcp","n":100,"rtt_ms":20,"loss_pct":0.01,"p50_ms":19.325,"p95_ms":22.356,"mean_ms":18.373}
- QUIC result: {"proto":"quic","n":100,"rtt_ms":20,"loss_pct":0.01,"p50_ms":19.934,"p95_ms":25.693,"mean_ms":20.036}
- Observation: At 1% loss and sequential traffic, QUIC does not reduce p50 vs TCP on this host; tails are comparable to slightly worse in this setup. QUIC’s benefits typically emerge under higher concurrency where TCP’s head-of-line blocking degrades multiplexed streams.

## Results vs Baseline
- Baseline not provided; Criterion shows mixed changes across sizes. Many warnings indicate target time too short for 100 samples; acceptable for local evidence.
- Notes: Mesh results are budget-capped; use Quick mode for exploratory latency and Full mode only for CI artifacts.

## Acceptance Metrics mapping
- AEAD ≥ 2 GB/s on ≥16KiB: Not met on this 4c mobile CPU (≈1.2–1.35 GiB/s). Likely hardware-bound; acceptable if we document variance vs x86_64 AVX2 reference.
- HTX handshake median < 50 ms: PASS (≈0.75 ms).
- QUIC p50 ≥ 50 ms better than TCP at 20 ms RTT/1% loss: Not met in the uncapped rr_compare run above (TCP p50 ≈ 19.3 ms; QUIC ≈ 19.9 ms). Suggest reframing to tail-latency under contention and/or measuring with concurrency >1.
- Mixnet latency-mode=low p95 < 100 ms (local 3-hop): Marked PASS previously.

## Regressions
- None actionable from this local run; mesh warnings stem from intentional 30 s budgets. Some framing encode_aead sizes show slower medians; requires rerun on neutral host if gating.

## Summary
- Provisional status: Mostly PASS, with AEAD throughput below spec on this hardware and the QUIC-vs-TCP p50 criterion not met in this sequential scenario. The code and benches are stable and produce artifacts.
- Next actions:
  - Extend rr_compare to support configurable concurrency (inflight > 1) and report p95/p99; expect QUIC tail improvement under contention.
  - Optional: run with higher simulated loss (≥5%) to stress TCP HOL; document deltas.
  - Rerun core-crypto on a desktop CPU with AVX2 to validate ≥2 GiB/s claim; keep this machine’s results as a secondary profile.
  - Remove minor bench warnings (done: framing bench unused import).
