# QNet Implementation Tasks

## Task Overview
This task list breaks down the QNet implementation plan into deeply actionable items with clear deliverables and acceptance tests. Priorities are High (critical for PoC), Medium (important), Low (nice-to-have). Estimates are person-days.

## Conventions
- Deliverables: expected files/modules, docs, and tests to land in repo.
- Interfaces: public API signatures or CLI flags expected to be stable.
- Acceptance: objective checks to mark task “Done”.
- Metrics: perf/coverage targets where applicable.

---

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
Acceptance:
- CI gate: coverage ≥80% on core-framing + noise; no crashes in 30m fuzz run.

### T2.6: L2 Frame Semantics & KEY_UPDATE Behavior
Objective: Implement KEY_UPDATE concurrency with 3-frame overlap; strict nonce lifecycle.
Priority: High | Dependencies: T1.3, T1.4 | Estimate: 3 days
Deliverables:
- mux key-rotation logic; tests for simultaneous updates.
Acceptance:
- Tests show acceptance of new+old keys up to 3 frames; old frames rejected after switch.

---

## Phase 3: Routing & Mesh (Priority: Medium)

### T3.1: SCION Packet Structures
Objective: Define data structures and signature checks (no external routing yet).
Priority: Medium | Dependencies: T1.1 | Estimate: 6 days
Deliverables:
- core-routing structs; sign/verify over segment; unit tests.
Acceptance:
- Valid segments verify; tampered ones fail; timestamp/expiry bounds enforced.

### T3.2: HTX Tunneling
Objective: Control stream for transition across non-SCION links; replay-bound tuple cache.
Priority: Medium | Dependencies: T2.4, T3.1 | Estimate: 4 days
Deliverables:
- htx/src/transition.rs: control stream CBOR map {prevAS,nextAS,TS,FLOW,NONCE,SIG}.
Acceptance:
- Duplicate (FLOW,TS) rejected; ±300s TS enforced; rekey closes control stream.

### T3.3: libp2p Integration
Objective: Minimal mesh for seeds, capability exchange, and stream negotiation.
Priority: Medium | Dependencies: T2.4 | Estimate: 5 days
Deliverables:
- core-mesh using libp2p; capability messages; basic gossip timer.
Acceptance:
- Node connects to 2 seeds; exchanges caps; opens stream 1 for CapMsg.

### T3.4: Bootstrap Discovery
Objective: Rotating rendezvous/mDNS/LE prototype + adaptive PoW verification.
Priority: Medium | Dependencies: T3.3 | Estimate: 4 days
Deliverables:
- discovery module; PoW verify constant-time; rate limits per prefix.
Acceptance:
- Join obtains ≥5 peers within 30s in local testnet.

### T3.5: Translation Layer (v1.1 Interop)
Objective: L2/L3 mapping to interop with 1.1 peers without exposing plaintext.
Priority: Medium | Dependencies: T2.6, T3.1 | Estimate: 4 days
Deliverables:
- tl module; mapping tables; synthesized KEY_UPDATE policy; exporter context “compat=1.1”.
Acceptance:
- Interop test shows stable data exchange; logs include compat flag.

---

## Phase 4: Privacy & Naming (Priority: Medium)

### T4.1: Mixnode Selection
Objective: BeaconSet + per-stream entropy; diversity constraints.
Priority: Medium | Dependencies: T1.2 | Estimate: 5 days
Deliverables:
- mix/select.rs with VRF selection; tests across epochs; diversity tracker.
Acceptance:
- Within (src,dst,epoch) reuses avoided until ≥8 sets; cross-AS hop present.

### T4.2: Nym Mixnet Integration
Objective: Sphinx processing + cover traffic padding; 25k pkt/s target on 4-core VPS.
Priority: Medium | Dependencies: T4.1 | Estimate: 7 days
Deliverables:
- mixnode binary; rate-limiters; perf benchmark harness.
Acceptance:
- Sustained 25k pkt/s in bench; latency budget adhered (≤900ms total added).

### T4.3: Self-Certifying IDs
Objective: PeerID = multihash(SHA-256(pubkey)); encodings and validation.
Priority: Medium | Dependencies: T1.2 | Estimate: 3 days
Deliverables:
- core-identity; hex and Base32 encoders; tests.
Acceptance:
- Round-trip string<->bytes; collision tests for trivial inputs.

### T4.4: Alias Ledger
Objective: 2-of-3 finality ledger prototype (in-memory/mock backends ok for PoC).
Priority: Medium | Dependencies: T4.3 | Estimate: 6 days
Deliverables:
- ledger module; quorum certificate checks; emergency advance path.
Acceptance:
- Conflicts resolved by seq; emergency path gated by quorum weight.

---

## Phase 5: Payments & Governance (Priority: Low)

### T5.1: Voucher System
Objective: 128-B voucher format encode/decode + basic validation; pass-through in transport.
Priority: Low | Dependencies: T1.2 | Estimate: 4 days
Deliverables:
- voucher.rs; tests for length and signature aggregation placeholders.
Acceptance:
- Invalid length rejected; opaque forwarding preserved.

### T5.2: Governance Scoring
Objective: Uptime score and voting weight with AS/Org caps.
Priority: Low | Dependencies: T3.3 | Estimate: 3 days
Deliverables:
- gov.rs; cap calculators; tests for boundary conditions.
Acceptance:
- Caps enforced at 20%/25%; score function matches formula.

---

## Phase 6: Tools & Compliance (Priority: Medium)

### T6.1: C Library Implementation
Objective: C wrapper over HTX dial/accept/stream APIs.
Priority: Medium | Dependencies: T2.4 | Estimate: 5 days
Deliverables:
- c-lib/ with headers (qnet.h), static lib, minimal examples.
Acceptance:
- C example echoes over HTX on Linux/Windows.

### T6.2: Go Spec Linter
Objective: CLI that validates compliance points, emits SBOM, and provides a GH Action.
Priority: Medium | Dependencies: Prior phases | Estimate: 4 days
Deliverables:
- linter/cmd/qnet-lint; rules for L2 framing, TemplateID, KEY_UPDATE, BN-Ticket header; SBOM via syft.
Acceptance:
- Running against PoC passes; failing configs produce clear messages.

### T6.3: uTLS Template Generator
Objective: Produce deterministic ClientHello blobs; JA3 self-test; auto-refresh.
Priority: Medium | Dependencies: T2.1 | Estimate: 3 days
Deliverables:
- utls-gen tool; downloads Chrome tags and updates templates; self-test command.
Acceptance:
- Self-test green; deterministic output across runs.

### T6.4: SLSA Provenance
Objective: Reproducible builds and SLSA v3 provenance in CI.
Priority: Medium | Dependencies: Prior phases | Estimate: 2 days
Deliverables:
- CI steps: pin toolchains, sbom, provenance attestation artifacts uploaded.
Acceptance:
- CI artifacts published; rebuild matches checksum.

### T6.5: Compliance Test Harness
Objective: Automate v1.2-style tests and report per profile.
Priority: High | Dependencies: T2.5, T2.6, T3.3 | Estimate: 6 days
Deliverables:
- tests/compliance with scenarios: crypto/framing/key-update, discovery, routing, relays, BN-Ticket header.
Acceptance:
- All MINIMAL profile tests pass; STANDARD subset passes where implemented.

---

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

---

## Risk Mitigation
- Parallelize High-priority tasks where dependency allows; keep tight CI gates.
- Property-based tests and fuzzing for parsers/crypto to catch edge cases early.
- Keep QUIC/SCION optional for PoC to de-risk timeline.

## Success Criteria
- All High tasks delivered; E2E HTX echo demo; compliance MINIMAL profile passes.
- Modular crates ready for bounty components and future extension.
