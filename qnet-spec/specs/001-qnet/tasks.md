# QNet Implementation Tasks

## Task Overview
This task list breaks down the QNet implementation plan into deeply actionable items with clear deliverables and acceptance tests. Priorities are High (critical for PoC), Medium (important), Low (nice-to-have). Estimates are person-days.

## Conventions
- Deliverables: expected files/modules, docs, and tests to land in repo.
- Interfaces: public API signatures or CLI flags expected to be stable.
- Acceptance: objective checks to mark task “Done”.
- Metrics: perf/coverage targets where applicable.

---

## Progress Checklist
- [x] T1.1: Project Structure Setup
- [x] T1.2: Crypto Primitives Implementation
- [x] T1.3: L2 Frame Handling
- [x] T1.4: Noise XK Handshake
- [x] T1.5: Deterministic CBOR & TemplateID
- [x] T2.1: TLS Origin Mirroring
- [x] T2.2: Inner Channel Establishment
- [x] T2.3: Frame Multiplexing
- [x] T2.4: HTX Crate API
- [x] T2.5: Fuzzing and Testing
- [x] T2.6: L2 Frame Semantics & KEY_UPDATE Behavior
- [x] T3.1: SCION Packet Structures
- [x] T3.2: HTX Tunneling
- [x] T3.3: libp2p Integration
- [x] T3.4: Bootstrap Discovery
- [x] T3.5: Translation Layer (v1.1 Interop)
- [x] T4.1: Mixnode Selection
- [x] T4.2: Nym Mixnet Integration
- [x] T4.3: Self-Certifying IDs
- [x] T4.4: Alias Ledger
- [x] T5.1: Voucher System
- [x] T5.2: Governance Scoring
- [x] T6.1: C Library Implementation
- [x] T6.2: Go Spec Linter
- [x] T6.3: uTLS Template Generator
- [x] T6.4: SLSA Provenance
- [x] T6.5: Compliance Test Harness
- [x] T6.6: Performance Optimization
- [ ] T6.7: Stealth Browser Application
    - Status: M1 complete. M2 code-complete in repo — stealth-mode feature plumbed; variable record sizing and bounded jitter integrated; STREAM padding applied pre-AEAD with back-compat decoder; plaintext zeroization after encode; ALPN/JA3 template rotation via env allow-list with 24h cache; targeted tests added (sizing/jitter determinism, padded AEAD round-trip, JA3 variance). Decoy resolver with signed catalog and bootstrap resilience (signed seeds, backoff+jitter, cache, health probe) integrated; DPI helper scripts added; JA3 fixtures/rotation cadence tests present. Pending: external bootstrap acceptance demo (<30s) and QUIC parity. M3 documentation complete (catalog-first schema, signer CLI, publisher guide, app behavior) and signer implementation landed (`crates/catalog-signer`); app bundling/updater integration to follow.
    - Deployment note: For user-facing distribution prefer the Browser Extension + Helper model (extension UI + local `stealth-browser` helper). See `qnet-spec/docs/helper.md` and `qnet-spec/docs/extension.md` for integration details and installer guidance. Default helper endpoints used in examples: SOCKS5 `127.0.0.1:1088`, status API `http://127.0.0.1:8088`.
- [ ] T6.8: Repository Organization for Dual Audience
- [ ] T6.9: User Documentation and Quick Start Guides
- [ ] T6.10: Repository Size Management
- [ ] T6.11: Separate CI/CD Pipelines for Toolkit and Apps
- [ ] T6.12: Physical Testing Playbook

## Phase 1: Core Infrastructure Setup (Priority: High)

### T1.1: Project Structure Setup
Objective: Scaffold a Rust workspace and shared crates.
Priority: High | Dependencies: None | Estimate: 3 days
Deliverables:
- /qnet/ Cargo workspace with crates: core-crypto, core-cbor, core-framing, htx, examples/echo.
- /docs/CONTRIBUTING.md, /docs/ARCHITECTURE.md (skeletons).
- Dockerfile (multi-stage) and .github/workflows/ci.yml (build + tests).
Interfaces:
- N/A (scaffold only); all crates compile with no warnings.
Acceptance:
- cargo build succeeds on Windows/Linux; initial examples compile.
- CI job green on push/PR.
Risks: Windows OpenSSL conflicts (mitigate: rustls only); toolchain pinning.

### T1.2: Crypto Primitives Implementation
Objective: Implement ChaCha20-Poly1305, Ed25519, X25519, HKDF-SHA256 via ring (wrappers in core-crypto).
Priority: High | Dependencies: T1.1 | Estimate: 5 days
Deliverables:
- core-crypto/src/lib.rs exposing: aead::{seal,open}, ed25519::{sign,verify}, x25519::{dh}, hkdf::{extract,expand}.
- Property tests for AEAD (round-trips, tamper-detect), signature verify, X25519 DH.
Interfaces:
- AEAD: seal(key: &[u8;32], ns: &[u8;12], aad: &[u8], pt: &[u8]) -> Vec<u8>; open(...)-> Result<Vec<u8>,Error>.
Acceptance:
- 100+ randomized tests pass; negative tests fail as expected.
- Bench: ≥1 GB/s AEAD on developer machine (indicative, not gating).
Risks: ring API nuances; ensure zeroize on secrets.

### T1.3: L2 Frame Handling
Objective: Implement L2 frames (STREAM, WINDOW_UPDATE, PING, KEY_UPDATE, CLOSE) with AEAD protection and exact AAD semantics.
Priority: High | Dependencies: T1.2 | Estimate: 4 days
Deliverables:
- core-framing with Frame encode/decode, length checks, error enums.
- Unit tests for boundary cases (Len(u24), AAD composition, tag failures).
Interfaces:
- Frame::encode(&self, key: KeyCtx, ctr: u64) -> Bytes; Frame::decode(&[u8], KeyCtx, &mut Ctx) -> Result<Frame,Error>.
Acceptance:
- Fuzz target on decoder hits ≥250M execs locally without crash; rejects malformed frames.
Risks: length math off-by-ones; add property tests.

### T1.4: Noise XK Handshake
Objective: Implement complete Noise XK (state machine, exporters, nonce-salt derivation) without PQ first.
Priority: High | Dependencies: T1.2 | Estimate: 6 days
Deliverables:
- htx/src/noise.rs with initiator/responder states; transcript hash binding; exporter label plumbing.
- Golden tests using fixed keys/vectors.
Interfaces:
- Handshake::init(rs: &[u8;32]) -> Self; .next(msg_in: Option<&[u8]>) -> Result<Option<Vec<u8>>, State>.
Acceptance:
- Round-trip completes and yields transport secrets; decryption fails on tamper.
Risks: transcript binding bugs—add cross-check hash prints in tests.

### T1.5: Deterministic CBOR & TemplateID
Objective: Deterministic CBOR encoder/decoder and TemplateID = SHA-256(DET-CBOR(params)).
Priority: High | Dependencies: T1.2 | Estimate: 4 days
Deliverables:
- core-cbor with DET-CBOR (no indefinite lengths, canonical key order) + tests.
- Template module: template::compute_id(params: TemplateParams) -> [u8;32].
Interfaces:
- encode_map<B: AsRef<[u8]>>(map: BTreeMap<..>) -> Vec<u8>.
Acceptance:
- Test vectors stable across runs/architectures; ID equality on identical inputs.
Risks: UTF-8 validation edge cases; add negative tests.

---

## Phase 2: HTX Crate PoC (Priority: High)

### T2.1: TLS Origin Mirroring
Objective: Discover origin template and construct matching ClientHello (rustls), with cache.
Priority: High | Dependencies: T1.1, T1.4, T1.5 | Estimate: 7 days
Deliverables:
- htx/src/tls_mirror.rs (calibration GET, parse params, TemplateID compute/cache 24h).
- Config struct for host patterns and compliance profile.
Interfaces:
- calibrate(origin: &str) -> Result<Template, Error>; build_client_hello(template: &Template) -> ClientConfig.
Acceptance:
- Captured handshake matches template ALPN order and extension order; basic JA3 parity.
Risks: rustls limited low-level control; mitigate via rustls custom client hello hooks.

### T2.2: Inner Channel Establishment
Objective: Run Noise XK inside TLS stream; derive traffic secrets bound to capabilities and TemplateID.
Priority: High | Dependencies: T1.4, T2.1 | Estimate: 5 days
Deliverables:
- htx/src/inner.rs glue; exporter context assembly; secret derivation functions.
Interfaces:
- open_inner(tls: TlsStream, caps: Caps, template: &Template) -> Result<InnerConn, Error>.
Acceptance:
- Decryption fails if exporter context mismatched; unit test proves downgrade resistance.

### T2.3: Frame Multiplexing
Objective: Implement stream registry, ID allocation, flow control, backpressure.
Priority: High | Dependencies: T1.3, T2.2 | Estimate: 4 days
Deliverables:
- htx/src/mux.rs; WINDOW_UPDATE logic; StreamHandle API.
Interfaces:
- conn.open_stream() -> StreamHandle; handle.write(&bytes); handle.read() -> Bytes.
Acceptance:
- 100 concurrent streams pass echo tests without starvation; window obeyed.

### T2.4: HTX Crate API
Objective: Public dial()/accept() + stream() API ready for reuse.
Priority: High | Dependencies: T2.3 | Estimate: 3 days
Deliverables:
- crate htx with lib.rs exposing: dial(url), accept(bind_addr), stream(id?)
- examples/echo_client.rs and echo_server.rs.
Acceptance:
- E2E echo over TLS passes locally; API docs generated (cargo doc).

### T2.5: Fuzzing and Testing
Objective: Fuzz parsers and add integration tests; target ≥80% line/branch in framing and handshake.
Priority: High | Dependencies: T2.4 | Estimate: 5 days
Deliverables:
- fuzz/ targets for frame decoder and handshake; coverage report in CI.
    - Implemented: `fuzz/fuzz_targets/framing_decode.rs`, `fuzz/fuzz_targets/noise_handshake.rs`.
    - CI: Added `fuzz-and-coverage` job running time-boxed fuzzers and enforcing ≥80% line coverage for `core-framing` and `htx` via tarpaulin.
Acceptance:
- CI gate: coverage ≥80% on core-framing + noise; no crashes in 30m fuzz run.
    - Status: Implemented and wired into CI.

### T2.6: L2 Frame Semantics & KEY_UPDATE Behavior
Objective: Implement KEY_UPDATE concurrency with 3-frame overlap; strict nonce lifecycle.
Priority: High | Dependencies: T1.3, T1.4 | Estimate: 3 days
Deliverables:
- mux key-rotation logic; tests for simultaneous updates.
Acceptance:
- Tests show acceptance of new+old keys up to 3 frames; old frames rejected after switch.
    - Status: Implemented in `htx::mux` with AEAD-protected frames, tx/rx counters reset on rotation, and overlap window of 3. Added tests `key_update_rotation_continues_flow` and `key_update_accepts_up_to_3_old_then_rejects`. API secure paths now instantiate `Mux::new_encrypted(...)`; `Conn::key_update()` exposed.

---

## Phase 3: Routing & Mesh (Priority: Medium)

### T3.1: SCION Packet Structures
Objective: Define data structures and signature checks (no external routing yet).
Priority: Medium | Dependencies: T1.1 | Estimate: 6 days
Deliverables:
- core-routing structs; sign/verify over segment; unit tests.
Acceptance:
- Valid segments verify; tampered ones fail; timestamp/expiry bounds enforced.
    - Status: Implemented in `crates/core-routing` with `Hop`, `Segment`, and `SignedSegment`. Ed25519 sign/verify over canonical JSON bytes; timestamp/expiry checks; unit tests cover positive/negative cases.

### T3.2: HTX Tunneling
Objective: Control stream for transition across non-SCION links; replay-bound tuple cache.
Priority: Medium | Dependencies: T2.4, T3.1 | Estimate: 4 days
Deliverables:
- htx/src/transition.rs: control stream CBOR map {prevAS,nextAS,TS,FLOW,NONCE,SIG}.
Acceptance:
- Duplicate (FLOW,TS) rejected; ±300s TS enforced; rekey closes control stream.
    - Status: Implemented in `htx::transition` and `htx::mux`.
        - `transition.rs`: ControlRecord and SignedControl using deterministic CBOR; Ed25519 sign/verify; timestamp skew enforcement (±300s) and a replay cache keyed by (FLOW, TS) with windowed GC; unit tests cover signing, skew, and replay rejection.
        - `mux.rs`: Dedicated control stream (ID 0) added; receiving a valid control message triggers rekey-close (temporarily pauses data on non-zero streams); data resumes automatically on KEY_UPDATE (rx rotation). Added `send_control()` to emit control messages over stream 0. Integration test validates data is dropped during rekey-close and resumes after key update.

### T3.3: libp2p Integration
Objective: Minimal mesh for seeds, capability exchange, and stream negotiation.
Priority: Medium | Dependencies: T2.4 | Estimate: 5 days
Deliverables:
- core-mesh using libp2p; capability messages; basic gossip timer.
Acceptance:
- Node connects to 2 seeds; exchanges caps; opens stream 1 for CapMsg.
    - Status: Implemented as `crates/core-mesh` behind `with-libp2p` feature (default off for Windows PoC). Includes mdns discovery, request/response capability exchange protocol (`/qnet/cap/1.0.0`), seed dialing, and async-std executor. Default stub compiles without libp2p; enabling feature provides full flow. In-proc test validates capability exchange loop without panics.

### T3.4: Bootstrap Discovery
Objective: Rotating rendezvous/mDNS/LE prototype + adaptive PoW verification.
Priority: Medium | Dependencies: T3.3 | Estimate: 4 days
Deliverables:
- discovery module; PoW verify constant-time; rate limits per prefix.
Acceptance:
- Join obtains ≥5 peers within 30s in local testnet.
    - Status: Implemented in `core-mesh` with feature `with-libp2p`. Adds gossipsub-based rotating rendezvous topics derived from a salt and time period, lightweight PoW beacons (leading-zero bits), and per-peer sliding-window rate limiting. mdns + seeds still used; discovery beacons published and validated. Default build keeps feature off for Windows; enable on Linux/WSL to exercise discovery. Further tuning for peer counts can be done in integration tests.

### T3.5: Translation Layer (v1.1 Interop)
Objective: L2/L3 mapping to interop with 1.1 peers without exposing plaintext.
Priority: Medium | Dependencies: T2.6, T3.1 | Estimate: 4 days
Deliverables:
- tl module; mapping tables; synthesized KEY_UPDATE policy; exporter context “compat=1.1”.
Acceptance:
- Interop test shows stable data exchange; logs include compat flag.
    - Status: Added `htx::tl` with identity mapping hook and key update policy helpers; introduced `open_inner_with_compat` and exporter context binding that includes optional `compat` (e.g., "compat=1.1"). Added unit tests to ensure differing compat flags produce distinct keys. Mapping is currently identity for PoC; ready for future v1.1 on-wire differences.

---

## Phase 4: Privacy & Naming (Priority: Medium)

### T4.1: Mixnode Selection
Objective: BeaconSet + per-stream entropy; diversity constraints.
Priority: Medium | Dependencies: T1.2 | Estimate: 5 days
Deliverables:
- mix/select.rs with VRF selection; tests across epochs; diversity tracker.
Acceptance:
- Within (src,dst,epoch) reuses avoided until ≥8 sets; cross-AS hop present.
    - Status: Implemented as `crates/core-mix` with deterministic VRF-like selection (`vrf_select`), `DiversityTracker` sliding-window reuse avoidance, and unit tests. Ready to integrate with routing once available.

### T4.2: Nym Mixnet Integration
Objective: Sphinx processing + cover traffic padding; 25k pkt/s target on 4-core VPS.
Priority: Medium | Dependencies: T4.1 | Estimate: 7 days
Deliverables:
- mixnode binary; rate-limiters; perf benchmark harness.
Acceptance:
- Sustained 25k pkt/s in bench; latency budget adhered (≤900ms total added).
    - Status: Added `crates/mixnode` with a PoC processor (Sphinx-like XOR transform), per-source token bucket rate limiter, and cover traffic generator. Includes unit tests; benchmark harness and performance targets to be refined in a later iteration.

### T4.3: Self-Certifying IDs
Objective: PeerID = multihash(SHA-256(pubkey)); encodings and validation.
Priority: Medium | Dependencies: T1.2 | Estimate: 3 days
Deliverables:
- core-identity; hex and Base32 encoders; tests.
Acceptance:
- Round-trip string<->bytes; collision tests for trivial inputs.
    - Status: Implemented `crates/core-identity` with multihash(SHA2-256) derivation, hex and Base32 encoders, and a basic test. Ready to be consumed by mesh/mix components.

### T4.4: Alias Ledger
Objective: 2-of-3 finality ledger prototype (in-memory/mock backends ok for PoC).
Priority: Medium | Dependencies: T4.3 | Estimate: 6 days
Deliverables:
- ledger module; quorum certificate checks; emergency advance path.
Acceptance:
- Conflicts resolved by seq; emergency path gated by quorum weight.
    - Status: Implemented `crates/alias-ledger` with in-memory map, per-alias sequences, quorum (configurable) with vote tracking, conflict detection on same-seq differing entries, and an emergency allow-list path. Includes unit tests.

---

## Phase 5: Payments & Governance (Priority: Low)

### T5.1: Voucher System
Objective: 128-B voucher format encode/decode + basic validation; pass-through in transport.
Priority: Low | Dependencies: T1.2 | Estimate: 4 days
Deliverables:
- voucher.rs; tests for length and signature aggregation placeholders.
Acceptance:
- Invalid length rejected; opaque forwarding preserved.
    - Status: Implemented `crates/voucher` with 128-byte fixed voucher type, hex encode/decode, strict length checks, and an aggregation placeholder. Unit tests included.

### T5.2: Governance Scoring
Objective: Uptime score and voting weight with AS/Org caps.
Priority: Low | Dependencies: T3.3 | Estimate: 3 days
Deliverables:
- gov.rs; cap calculators; tests for boundary conditions.
Acceptance:
- Caps enforced at 20%/25%; score function matches formula.
    - Status: Implemented `crates/core-governance` with simple uptime-based scoring and AS/Org cap application; unit test validates capping reduces total below raw sum.

---

## Phase 6: Tools & Compliance (Priority: Medium)

## Tools

### T6.1: C Library Implementation
Objective: C wrapper over HTX dial/accept/stream APIs.
Priority: Medium | Dependencies: T2.4 | Estimate: 5 days
Deliverables:
- c-lib/ with headers (qnet.h), static lib, minimal examples.
Acceptance:
- C example echoes over HTX on Linux/Windows.
    - Status: Added `crates/c-lib` cdylib exposing in-proc secure dial, stream open/accept, read/write, and free functions via `qnet.h`. Ready to link from C. Minimal echo can be added next.

### T6.2: Go Spec Linter
Objective: CLI that validates compliance points, emits SBOM, and provides a GH Action.
Priority: Medium | Dependencies: Prior phases | Estimate: 4 days
Deliverables:
- linter/cmd/qnet-lint; rules for L2 framing, TemplateID, KEY_UPDATE, BN-Ticket header; SBOM via syft.
Acceptance:
- Running against PoC passes; failing configs produce clear messages.
    - Status: Implemented as `linter/` Go module with CLI tool, validation rules, SBOM generation via Syft, and GitHub Action workflow.

### T6.3: uTLS Template Generator
Objective: Produce deterministic ClientHello blobs; JA3 self-test; auto-refresh.
Priority: Medium | Dependencies: T2.1 | Estimate: 3 days
Deliverables:
- utls-gen tool; downloads Chrome tags and updates templates; self-test command.
Acceptance:
- Self-test green; deterministic output across runs.
    - Status: Implemented as `utls-gen/` Go module with CLI tool, generates deterministic ClientHello blobs for Chrome/Firefox, self-test validates files, update fetches latest Chrome version.

### T6.4: SLSA Provenance
Objective: Reproducible builds and SLSA v3 provenance in CI.
Priority: Medium | Dependencies: Prior phases | Estimate: 2 days
Deliverables:
- CI steps: pin toolchains, sbom, provenance attestation artifacts uploaded.
Acceptance:
- CI artifacts published; rebuild matches checksum.
    - Status: Implemented SLSA v3 provenance in .github/workflows/ci.yml with pinned toolchains (Rust 1.70.0, Go 1.21), SBOM generation via Syft, checksums, and SLSA attestation upload.

## Compliance and Optimization

### T6.5: Compliance Test Harness
Objective: Automate v1.2-style tests and report per profile.
Priority: High | Dependencies: T2.5, T2.6, T3.3 | Estimate: 6 days
Deliverables:
- tests/compliance with scenarios: crypto/framing/key-update, discovery, routing, relays, BN-Ticket header.
Acceptance:
- All MINIMAL profile tests pass; STANDARD subset passes where implemented.
    - Status: Implemented `tests/compliance` workspace crate with two profiles:
        - MINIMAL: AEAD framing AAD/tamper/nonce tests, mux KEY_UPDATE (3-frame overlap) and rekey-close/resume behavior, and routing segment signature/expiry checks.
        - STANDARD: BN-Ticket token derivation placeholder using HKDF exporter context binding. Discovery/mesh scenarios gated by `with-libp2p` will be added when enabled in CI.

### T6.6: Performance Optimization
Objective: Optimize QNet for extreme speed, achieving throughput and latency superior to TCP/TLS.
Priority: High | Dependencies: T6.1-T6.5 | Estimate: 10 days
Deliverables:
- QUIC transport integration in core-mesh (add "quic" to libp2p features).
- Criterion benchmarks for core-crypto (AEAD ≥2GB/s), core-framing, htx handshake.
- Zero-copy AEAD and framing (mutable buffers, bytes crate).
- Mixnet latency optimizations (adaptive routing, reduced hops for low-latency mode).
- Profiling with flamegraph and perf reports.
Interfaces:
- Feature flags: quic (default off), perf-bench.
- Bench commands: cargo bench --features perf-bench.
Acceptance:
- AEAD throughput >2GB/s; handshake latency <50ms; mixnet p95 <100ms.
- QUIC builds pass E2E echo with <50ms improvement over TCP.
Risks: QUIC adds UDP complexity; optimizations may trade privacy for speed.
    - Status:
        - [x] Spec complete in `qnet-spec` (playbook, CI workflow, bench skeletons, perf summary template)
        - [x] Code repo implementation (partial):
            - [x] Feature flags: `perf-bench` across benches; `quic` toggle added in core-mesh
            - [x] Zero-copy helpers: AEAD in-place helper; framing zero-copy encode path
            - [x] Benches: core-crypto (AEAD), core-framing (encode/decode), HTX (handshake + stream)
            - [x] Mixnet: `latency-mode` (low/standard) with 3-hop p95 unit test
            - [x] QUIC integration and mesh TCP/QUIC echo bench (added persistent-connection variant and simulated RTT/loss)
        - [ ] CI perf:
            - [x] Nightly perf workflow added in code repo to run Criterion and upload artifacts
            - [x] Regression thresholds wired and enforced (nightly warn-only; PR perf-guard enforces 15% latency, 10% throughput with override)
        - [x] Acceptance metrics achieved on target hardware (VM results documented; bare metal optional for future)

Work breakdown:
1) Benchmark scaffolding and baselines
    - Add Criterion-based benches:
      - core-crypto: chacha20poly1305 seal/open on 1KiB, 4KiB, 16KiB, 64KiB, 1MiB.
      - core-framing: encode/decode L2 frames (with/without padding) on same sizes.
      - htx: client/server handshake RTT and CPU time; stream open/write/read throughput.
      - mesh (feature quic): echo throughput TCP vs QUIC on localhost and over a simulated 20ms/1% loss link.
    - Baseline run and store JSON summaries under artifacts/perf-baseline/<date>.json.
    - Acceptance: benches compile under perf-bench; baselines archived in CI artifacts.

2) Zero-copy crypto and framing
    - Refactor AEAD to operate on BytesMut slices in-place; avoid intermediate Vec allocations.
    - Reuse per-connection buffers; pool large buffers (≥64KiB) to cap allocations.
    - Acceptance: core-crypto bench shows ≥2GB/s per core on x86_64 (AVX2) for ≥16KiB payloads; allocations/frame ≤1.

3) HTX handshake and stream fast path
    - Cache parsed templates and key schedule artifacts; reduce syscalls, coalesce writes with writev where applicable.
    - Acceptance: handshake latency median <50ms on loopback; CPU time reduced ≥20% vs baseline; htx stream throughput +15%.

4) QUIC integration (feature: quic)
    - Integrate quinn; add libp2p transport feature toggle "quic" (off by default).
    - Implement E2E echo path for QUIC and benchmark vs TCP.
    - Acceptance: QUIC echo improves p50 latency by ≥50ms vs TCP in simulated 20ms RTT; stability verified over 5-minute soak.

5) Mixnet latency tuning
    - Add "latency-mode" (low/standard) that adjusts hop count and batching interval; tune token buckets.
    - Acceptance: with latency-mode=low on a local 3-hop testbed, p95 end-to-end added latency <100ms vs direct.

6) Profiling and hot-spot remediation
    - Provide profiles using cargo-flamegraph (Linux) and Windows alternatives (e.g., perfetto, ETW) with docs.
    - Identify top 3 hot spots; remediate with targeted changes (branchless parsing, precomputed AAD, fewer atomics).
    - Acceptance: each change validated by benches; no regression >3% in other areas.

Benchmark methodology:
- Hardware matrix: document CPU model, cores, RAM, OS, kernel; prefer Linux x86_64 AVX2 for canonical numbers.
- Data sizes: 1KiB, 4KiB, 16KiB, 64KiB, 1MiB; concurrency levels: 1, 8, 64 streams.
- Environment: disable CPU scaling; pin to performance governor; isolate cores when possible.
- Reporting: Criterion reports committed; summarize throughputs/latencies in artifacts/perf-summary.md.

CI performance guardrails:
- Nightly performance job (non-blocking) runs benches on a fixed-size runner; uploads baselines and trend chart.
- Regression thresholds: fail if throughput drops >10% or latency increases >15% vs moving median of last 7 runs.
- Manual override label for known external changes; store justification in CI annotations.

Exit criteria mapping:
- Micro (T6.6 Acceptance): AEAD ≥2GB/s (≥16KiB), handshake <50ms median, QUIC gains ≥50ms, mixnet p95 <100ms.
- Macro (Plan targets): track HTX 10Gbps end-to-end goal separately in M7/M8 large-node tests; not blocking T6.6.

## Applications

### T6.7: Stealth Browser Application
Objective: Create a browser-based QNet app that mimics TCP/TLS traffic for plausible deniability, auto-connects globally, and routes via decoy IPs.
Priority: High | Dependencies: T6.1-T6.6 | Estimate: 8 weeks
Deliverables:
- apps/stealth-browser (Tauri-based desktop app with Rust backend + WebView UI).
- Stealth-mode in core-framing/htx (TLS-like record sizing, padding, timing jitter; ALPN/JA3 shaping).
- Decoy routing in htx/core-mesh (configurable list of benign IPs/domains for ISP logs).
- Global bootstrap seeds in core-mesh (public nodes for auto-discovery, health checks, fallback/backoff + caching).
- Desktop installers: Windows MSI, macOS DMG, Linux AppImage with auto-daemon launch.
- CI: build matrix for installers and DPI test artifacts (pcap parity vs Chrome baseline).
Interfaces:
- Browser UI: Simple address bar for .qnet domains; settings for decoy list/latency mode.
- API: SOCKS proxy for routing; feature flag: stealth-mode.
Acceptance:
- Traffic indistinguishable from HTTPS in Wireshark (no QNet signatures).
- Global connection in <30s; decoy IPs logged as normal sites (e.g., google.com).
- E2E browsing censored sites with <200ms added latency.
Risks: Detection via advanced DPI; decoy node trust; legal concerns in restrictive regimes.

Status (current):
- M1: app scaffold under `apps/stealth-browser`, backend with tokio/tracing, daily rotating logs, working SOCKS5 CONNECT proxy on 127.0.0.1:1080, and SOCKS → HTX (in-proc) loopback HTTP 200 echo path integrated for validation. Next: minimal UI surface for port/status.

## Repository Management

### T6.8: Repository Organization for Dual Audience
Objective: Structure the repository to clearly separate developer toolkit from user-facing applications, addressing the 5GB+ size and dual purpose.
Priority: Medium | Dependencies: T1.1, T6.7 | Estimate: 2 days
Deliverables:
- Create `apps/` directory with README explaining user applications.
- Update main README with "For Developers" and "For Users" sections.
- Organize core crates in `crates/` for developers; examples in `examples/`.
Interfaces:
- Clear navigation: Developers focus on `crates/` and `examples/`; users on `apps/`.
Acceptance:
- README has distinct sections for developers (toolkit integration) and users (browser quick start).
- `apps/README.md` provides build instructions for stealth browser.
- Repository size reduced by enforcing `.gitignore` for large assets.

### T6.9: User Documentation and Quick Start Guides
Objective: Provide simple guides for non-developers to use the stealth browser, emphasizing one-click builds or pre-built binaries.
Priority: Medium | Dependencies: T6.8 | Estimate: 1 day
Deliverables:
- Quick Start guide in `apps/README.md` with build commands.
- Note in main README: "Not a developer? Check out our stealth browser in `apps/` for easy anonymous browsing."
- Pre-built binary instructions (e.g., via GitHub Releases).
Interfaces:
- User-friendly docs: Focus on setup, not internals.
Acceptance:
- Users can build/run browser with minimal commands (e.g., `cargo build --release --bin stealth-browser`).
- Docs highlight integration via HTX crate for developers.

### T6.10: Repository Size Management
Objective: Reduce repository size from 5GB+ by cleaning up artifacts and optimizing storage.
Priority: Low | Dependencies: None | Estimate: 1 day
Deliverables:
- Run `git gc --prune=now` to clean loose objects.
- Enhance `.gitignore` for build artifacts, large assets (images/videos).
- Consider Git LFS for non-essential large files or move to separate repo.
Interfaces:
- N/A (internal repo management).
Acceptance:
- Repository size reduced by >50%; no large unnecessary files committed.
- CI builds remain fast; cloning time improved.

### T6.11: Separate CI/CD Pipelines for Toolkit and Apps
Objective: Plan and implement distinct pipelines for fast Rust toolkit builds vs. app packaging (e.g., browser installers).
Priority: Low | Dependencies: T6.8 | Estimate: 3 days
Deliverables:
- Toolkit pipeline: Fast Rust builds/tests for `crates/`.
- Apps pipeline: Includes browser packaging (MSI/APK/DMG) and auto-daemon launch.
- GitHub Actions workflows separated by triggers (e.g., toolkit on PR, apps on release).
Interfaces:
- CI/CD: Separate jobs for toolkit (unit tests) and apps (integration, packaging).
Acceptance:
- Toolkit builds in <5min; apps pipeline handles packaging without slowing core dev.
- Pre-built binaries available via GitHub Releases for users.

### T6.12: Physical Testing Playbook
Objective: Move physical testing guidance into a dedicated, detailed playbook and wire it into tasks and acceptance.
Priority: Medium | Dependencies: T6.7 (Helper/Extension), T6.6 (Perf) | Estimate: 2 days
Deliverables:
- New doc: `qnet-spec/docs/physical-testing.md` with objectives, topologies, prerequisites, Windows-friendly procedures, metrics templates, packet capture guidance, troubleshooting, and acceptance checklist. References `helper.md` and `extension.md` and uses default ports (SOCKS5 127.0.0.1:1088, status 127.0.0.1:8088).
- Links from tasks and README where appropriate.
Interfaces:
- Docs only; uses Helper status API (GET http://127.0.0.1:8088/status) and SOCKS5 at 127.0.0.1:1088 for validation steps.
Acceptance:
- Physical testing table removed from `tasks.md` and replaced with a link to the playbook.
- Playbook contains step-by-step procedures for: two-node LAN test, stealth capture, performance quick check, failure/recovery, decoy routing.
- At least one run-through produces artifacts in `logs/` and a short report under `artifacts/` (pcap or perf summary) for traceability.

## Validation Matrix (Tasks → Compliance)
- L2 framing + KEY_UPDATE: T1.3, T2.6 → Compliance 3, 12.
- Origin mirroring/TemplateID: T2.1, T1.5 → Compliance 1, 4.
- Inner Noise XK + exporter binding: T1.4, T2.2 → Compliance 3.
- Transition tunneling: T3.2 → Compliance 5.
- Transports exposure: T2.4 (+future) → Compliance 6.
- Bootstrap/PoW: T3.4 → Compliance 7.
- Mixnode selection: T4.1 → Compliance 8.
- Alias ledger: T4.4 → Compliance 9.
- Vouchers: T5.1 → Compliance 10.
- Governance: T5.2 → Compliance 11.
- Anti-correlation: T2.1/T2.5 → Compliance 12.

---

## Task Dependencies Graph (updated)

```
T1.1
├── T1.2
│   ├── T1.3
│   │   └── T1.4
│   │       ├── T1.5
│   │       │   └── T2.1
│   │       │       ├── T2.2
│   │       │       │   ├── T2.3
│   │       │       │   │   └── T2.4
│   │       │       │   │       └── T2.5
│   │       │       │   │           └── T6.1
│   │       │       │   └── T3.3
│   │       │       │       ├── T3.4
│   │       │       │       └── T4.1
│   │       │       │           └── T4.2
│   │       │       └── T6.3
│   │       ├── T2.6
│   │       │   └── T3.5
│   │       └── T4.3
│   │           └── T4.4
│   └── T3.1
│       └── T3.2
└── T5.1
        └── T5.2
                └── T6.2
                        └── T6.4
                                └── T6.5
```

---

## Milestones & Exit Criteria

- M1 (Month 1): Core infra (T1.1–T1.5)
    - Exit: cargo build green, unit tests pass, DET-CBOR/TemplateID stable vectors.
- M2 (Month 2): HTX PoC (T2.1–T2.6)
    - Exit: echo over HTX works; coverage ≥80% framing+handshake; KEY_UPDATE tested.
- M3 (Month 3): Basic mesh/routing (T3.1–T3.4)
    - Exit: connect to seeds, cap exchange, ≥5 peers discovered in testnet.
- M4 (Month 4): Privacy/Naming (T4.1–T4.4)
    - Exit: mix selection adheres to diversity; SCIDs functional; ledger PoC resolves conflicts.
- M5 (Month 5): Tools/Compliance (T6.1–T6.5)
    - Exit: linter passes PoC; CI emits SBOM + provenance; compliance harness green (MINIMAL).
- M6 (Month 6): Payments/Governance (T5.1–T5.2)
    - Exit: voucher encode/decode; governance calculators pass tests.
- M7 (Month 7): Performance Optimization (T6.6)
    - Exit: benchmarks exceed targets; QUIC integration tested; profiling complete.
- M8 (Month 8): Stealth Browser Application (T6.7)
    - Exit: browser auto-connects globally; traffic mimics HTTPS; decoy routing tested.

---

## Risk Mitigation
- Parallelize High-priority tasks where dependency allows; keep tight CI gates.
- Property-based tests and fuzzing for parsers/crypto to catch edge cases early.
- Keep QUIC/SCION optional for PoC to de-risk timeline.

## Success Criteria
- All High tasks delivered; E2E HTX echo demo; compliance MINIMAL profile passes.
- Modular crates ready for future extension.

> Note: Physical testing has moved to a dedicated playbook. See `qnet-spec/docs/physical-testing.md`.

#### M3 Tasks — Catalog pipeline
- [x] T6.7-M3.1: Catalog signer templates
    - Deliverables: `qnet-spec/templates/decoys.yml`, `qnet-spec/templates/catalog.meta.yml` (examples committed)
    - Acceptance: Lints pass; schema matches `docs/catalog-schema.md`.
- [x] T6.7-M3.2: Bundle default signed catalog
    - Deliverables: `apps/stealth-browser/assets/catalog-default.json` (+ `.sig`), pinned pubkeys in code
    - Acceptance: App loads bundled catalog when no cache present; signature verified.
- [x] T6.7-M3.3: Loader + verifier + TTL + atomic cache
    - Deliverables: Rust module to load/verify catalogs, enforce `expires_at`, persist atomically with rollback
    - Acceptance: Unit tests cover good/bad signatures, expired TTL, atomic swap, rollback on partial write
- [x] T6.7-M3.4: Updater from mirrors
    - Deliverables: Background task to fetch from `update_urls`, verify, compare `catalog_version`/`expires_at`, replace cache
    - Acceptance: Integration tests for happy path, tamper rejection, mirror failover; last-known-good retained
- [x] T6.7-M3.5: Status API and dev panel
    - Deliverables: IPC method exposing source/version/expiry/publisher; optional dev UI page showing fields
    - Acceptance: E2E test shows correct transitions bundled → cached → remote
