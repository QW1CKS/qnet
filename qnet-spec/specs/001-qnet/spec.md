# QNet Specification

## Overview
QNet is a decentralized, censorship-resistant network designed to replace the public Internet. It provides strong user privacy, decentralized operation, and resistance to censorship through layered architecture and advanced cryptography.

## User Stories
**As an end user**, I want a simple browser extension that, together with a small local Helper, mimics normal HTTPS traffic so that I can browse anonymously without ISP tracking or technical setup.
### Core Network Functionality
- **As a developer**, I want to build applications on QNet using self-certifying identities so that users can verify service authenticity without external authorities.
- **As a privacy-conscious user**, I want my traffic to be routed through mixnodes so that observers cannot correlate my communications.
- **As a developer**, I want to integrate QNet's protocol stack into my apps via modular crates (e.g., HTX for tunneling) so that I can build custom privacy tools without reinventing cryptography.
- **As an end user**, I want a ready-to-use stealth browser that mimics normal HTTPS traffic so that I can browse anonymously without ISP tracking or technical setup.

### Layer Architecture
- **L0 Access Media**: Support any IP bearer (fibre, 5G, satellite, LoRa, etc.)
- **L1 Path Selection & Routing**: Use SCION + HTX-tunnelled transitions for secure routing
- **L2 Cover Transport**: HTX over TCP-443/QUIC-443 with origin-mirrored TLS
- **L3 Overlay Mesh**: libp2p-v2 object relay for peer-to-peer connections
- **L4 Privacy Hops**: Nym mixnet for optional anonymity
- **L5 Naming & Trust**: Self-certifying IDs + 3-chain alias ledger
- **L6 Payments**: Federated Cashu + Lightning for micro-payments
- **L7 Applications**: Web-layer replacement services

### Cryptography & Security
- **As a security engineer**, I want all communications encrypted with ChaCha20-Poly1305, Ed25519 signatures, X25519 DH, and HKDF-SHA256 so that data is protected against eavesdropping and tampering.
- **As a future-proof system**, I want post-quantum hybrid X25519-Kyber768 from 2027 so that the network remains secure against quantum attacks.

### Key Features
- Origin-mirrored TLS handshake with auto-calibration
- Noise XK inner handshake for mutual authentication
- SCION packet headers for path validation
- Mixnode selection using BeaconSet randomness
- Alias ledger with 2-of-3 finality
- Voucher-based payments
- Anti-correlation fallback with cover connections

### Improvements Over Betanet
- Enhanced anti-correlation measures
- Adaptive Proof-of-Work for bootstrap
- Better scalability and user adoption incentives
- Reproducible builds with SLSA provenance

## Functional Requirements

### Networking
1. Clients MUST mirror front origin TLS fingerprints exactly
2. Inner channel MUST use Noise XK with PQ hybrid from 2027
3. Paths MUST be validated using SCION signatures
4. Bootstrap MUST use rotating DHT with adaptive PoW
5. Mixnodes MUST be selected deterministically with diversity

### Privacy & Security
1. Traffic MUST be indistinguishable from normal HTTPS
2. All frames MUST be AEAD-protected
3. Replay protection MUST use per-direction counters
4. Congestion feedback MUST influence path selection
5. Emergency Advance MUST be available for naming liveness

### Payments & Governance
1. Vouchers MUST be 128-byte with aggregated signatures
2. Voting power MUST cap per-AS and per-org
3. Quorum MUST require 2/3 diversity
4. Upgrades MUST wait 30 days after threshold

### Compliance
1. Implementations MUST pass all 13 compliance checks
2. Builds MUST be reproducible
3. Binaries MUST have SLSA 3 provenance

## Bounties
- HTX client/server crate
- High-performance Nym mixnode
- C library implementation
- Spec-compliance linter
- Chrome-Stable uTLS generator

## Acceptance Criteria
- All user stories implemented
- Functional requirements met
- Compliance tests pass
- Bounties deliverable
- Better than Betanet in key areas

## Stealth-mode summary (T6.7, M2)
- Feature flag across `htx` and `core-framing` enabling:
	- TLS-like record sizing profiles (small/webby/bursty) with deterministic seeds
	- Bounded timing jitter in mux write path; backpressure-aware
	- STREAM right-padding (id||len||data||pad) applied pre-AEAD; backward-compatible decoder
	- Plaintext buffer zeroization on encode
	- ALPN/JA3 template rotation with 24h cache; TemplateID via deterministic CBOR
- Decoy routing: signed catalog (Ed25519 over DET-CBOR), weighted selection, optional ALPN override
- Bootstrap: signed seeds, exponential backoff (0.5s→8s, ±10% jitter), 24h cache, HTTPS /health probe, env gate before dial
- Env knobs: STEALTH_SIZING_PROFILE/SEED, STEALTH_JITTER_PROFILE/SEED, STEALTH_TPL_ALLOWLIST, STEALTH_DECOY_* and STEALTH_BOOTSTRAP_*; reserved PREFER_QUIC
	- HTX scheduler: HTX_SCHEDULER_PROFILE (http), HTX_INITIAL_WINDOW, HTX_CHUNK for flow-control defaults
- Validation: unit tests for distributions, bounds, AEAD with padding, template rotation; example DPI scripts under `qnet-spec/templates/`

## Catalog pipeline summary (T6.7, M3)
M3 makes the signed catalog the primary configuration path, with seeds as fallback. See `docs/catalog-schema.md` and `specs/001-qnet/T6.7-playbook.md`.

- Format: JSON public form with detached Ed25519 signature over DET-CBOR bytes of the inner object; pinned publisher keys in client.
- Bundling: Ship `assets/catalog-default.json` (+ `.sig`) with the app; verify at load time.
- Cache + TTL: Store verified catalogs under OS-appropriate app data; enforce `expires_at` with grace + warnings; keep last-known-good for rollback.
- Updater: Fetch from `update_urls` mirrors (GitHub Raw/Pages/CDN), verify signature and freshness, atomically replace cache, and surface status.
- Status API: Expose `source` (bundled/cached/remote), `version`, `expires_at`, and `publisher_id` to the UI.
- Security: Mirrors are untrusted; integrity comes from signatures; support 1–3 pinned keys for rotation; publish fingerprints in docs/UI.

Acceptance (M3):
- Loader verifies signatures and TTL; falls back correctly; atomic persist + rollback implemented.
- Updater pulls a newer catalog from mirrors; rejects tamper; fails over mirrors; retains last-known-good on errors.
- UI (or dev panel) shows catalog status fields for manual validation.

Docs status: M3 documentation is complete (schema, signer CLI, publisher guide, app behavior). See:
- `qnet-spec/docs/catalog-schema.md`
- `qnet-spec/docs/catalog-signer.md`
- `qnet-spec/docs/catalog-publisher.md`
- `qnet-spec/docs/apps/stealth-browser.md`

## Performance Targets and Methodology (for T6.6)
- Micro-benchmarks:
	- AEAD (ChaCha20-Poly1305) ≥2 GB/s per core for ≥16KiB payloads (x86_64 AVX2 reference).
	- L2 frame encode/decode shows no more than 1 allocation per frame; padding overhead within 5%.
	- HTX handshake median latency <50ms on loopback; CPU time reduced vs baseline.
- Transport comparison:
	- QUIC path p50 latency improves by ≥50ms vs TCP under 20ms RTT/1% loss simulation.
- Mixnet latency:
	- With latency-mode=low and 3 hops, added p95 latency <100ms vs direct connection in a local testbed.
- Methodology:
	- Use Criterion benches, fixed hardware profile, and CI trend tracking with 10–15% regression thresholds.
- Status: Complete with caveats (VM results on DigitalOcean droplet documented; bare metal optional for future).

## Review & Acceptance Checklist
- [x] Spec covers all layers L0-L7
- [x] Cryptography requirements specified
- [x] Privacy features detailed
- [x] Governance and payments included
- [x] Improvements over Betanet identified
- [x] Bounties clearly defined
- [x] Compliance points enumerated

## Deployment recommendation (users)

The recommended user-facing deployment is the Browser Extension + Helper model. The extension provides the UI and communicates with a local Helper service (the `stealth-browser` binary) which exposes a SOCKS5 proxy and a local status API. See `qnet-spec/docs/helper.md` and `qnet-spec/docs/extension.md` for installation and integration details. Default helper endpoints used in examples throughout this repo:

- SOCKS5: `127.0.0.1:1088`
- Status API: `http://127.0.0.1:8088`

This approach keeps the user installation lightweight (small extension + helper service) and simplifies packaging for cross-platform distribution.

## QNet Browser Extension: Complete User Experience

This section defines the expected user experience for the Browser Extension + Helper deployment model.

### Installation Process

1. Initial Download
	- User installs the QNet browser extension from the Chrome/Edge/Firefox store.
	- Additionally, user downloads a small "QNet Helper" installer (~10–20 MB) that contains the Rust binaries (Helper runs the `stealth-browser` and can launch `edge-gateway` when needed). Browser extensions cannot run binaries directly, so the Helper is required.

2. First-Run Setup
	- Extension checks if the Helper is installed; if missing, it guides the user to install it.
	- Helper installs as a background service with minimal/no UI.
	- Extension connects to the Helper via Native Messaging (preferred) or a localhost dev API.
	- Helper handles any one-time privilege prompts needed for system integration (e.g., hosts file editing if enabled by the user).

### Using QNet

1. Starting Protection
	- User clicks the QNet extension icon and toggles "Connect".
	- Extension signals the Helper to start the SOCKS proxy (`stealth-browser`).
	- Extension configures browser proxy to use `127.0.0.1:1088` (SOCKS5).
	- Extension icon turns green to indicate protection is active.

2. Normal Browsing
	- User browses normally.
	- Browser sends requests through the local SOCKS proxy. The Helper selects a decoy from the signed catalog, and connections appear as if going to the decoy site; actual content is fetched from the real site via the HTX path.

3. Status & Control
	- Extension popup shows: protection status (active/inactive), current catalog version, toggle to enable/disable, and a settings page for advanced options.

4. Disabling Protection
	- User toggles off in the extension.
	- Extension restores browser proxy to direct connection.
	- Helper places the proxy in standby.

### Behind the Scenes

- Helper service:
  - Runs `stealth-browser` (SOCKS5) and can launch `edge-gateway` when masked mode is required.
  - Manages catalog updates (download + signature verification) and persistence.
  - Handles system integration (optional hosts file changes when explicitly enabled, startup registration).
  - Starts automatically with the system (optional).

- Extension:
  - Provides UI controls and manages the browser proxy settings.
  - Communicates with the Helper via Native Messaging (production) or localhost HTTP/WebSocket (dev).

### Technical Requirements

1. Helper App
	- ~10–20 MB download size (Rust binaries + certs when applicable).
	- Admin rights for initial install only (service registration or native messaging manifest on some platforms).
	- Small CPU/RAM footprint when running.

2. Data Usage
	- Minimal overhead beyond normal browsing.
	- Small signed catalog updates (KBs) periodically.

### Development Path

1. Helper app
	- Package the Rust components from this repo.
	- Provide a stable local API for the extension (Native Messaging; dev HTTP endpoints behind a flag).
	- Handle system integration tasks (proxy lifecycle, optional hosts edits with consent).

2. Browser extension
	- Provide simple UI controls and status.
	- Manage browser proxy configuration while active.
	- Communicate with the Helper for start/stop/status/update.
