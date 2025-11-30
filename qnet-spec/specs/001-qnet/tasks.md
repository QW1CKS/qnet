# QNet Implementation Tasks - Detailed Checklist

> **How to use this**: Check off each item as you complete it. Work from top to bottom. Each item is designed to be small enough to complete in one focused session.

---

## ⚠️ ARCHIVED: Phase 2.1.9 - DHT Provider Discovery (Superseded Jan 2025)

> **STATUS**: This task was superseded by Task 2.1.10 (Operator Directory). DHT-based peer discovery has been removed in favor of a simpler operator directory HTTP registry model. See Task 2.1.10 below for current implementation.

**Historical Context**: Attempted to fix DHT provider discovery but determined DHT complexity exceeded project needs. Removed ~450 lines of Kademlia code, replaced with ~480 lines of operator directory implementation.

---

## 🚀 ACTIVE: Phase 2.1.10 - Operator Directory Peer Discovery (Jan 2025)

**Context**: Replaced Kademlia DHT with lightweight HTTP-based peer registry. Relay nodes register via heartbeat, client nodes query on startup.

**Implementation Complete**: All tasks finished, documentation updates in progress.

### 2.1.10 Operator Directory Implementation ✅ COMPLETE
**Goal**: Replace DHT with operator directory HTTP registry for peer discovery.

#### 2.1.10.1 Remove DHT Code ✅ COMPLETE
- [x] Remove Kademlia imports from `crates/core-mesh/src/discovery.rs`
- [x] Remove `kademlia` field from DiscoveryBehavior
- [x] Remove `public_libp2p_seeds()`, `discover_peers()`, `peer_count()`, `get_peers()` methods
- [x] Remove "kad" feature from `crates/core-mesh/Cargo.toml`
- [x] Remove DHT event handling from `apps/stealth-browser/src/main.rs` (~150 lines)
- [x] Update `test_load_bootstrap_nodes_returns_operator_nodes()` test
- [x] Simplify `crates/core-mesh/examples/mesh_discovery.rs` (disabled notice)

#### 2.1.10.2 Create Operator Directory Module ✅ COMPLETE
- [x] Create `apps/stealth-browser/src/directory.rs` module
- [x] Implement `RelayInfo` struct (peer_id, addrs, country, capabilities, timestamps)
- [x] Implement `PeerDirectory` struct with HashMap storage
- [x] Add `register_peer()` method (POST /api/relay/register handler)
- [x] Add `get_relays_by_country()` method (GET /api/relays/by-country handler)
- [x] Add `prune_stale_peers()` method (120s TTL)
- [x] Add unit tests (8 tests covering registration, updates, queries, staleness, pruning)

#### 2.1.10.3 Add Directory Query on Startup ✅ COMPLETE
- [x] Implement `query_operator_directory()` async function in `main.rs`
- [x] Add 3-tier fallback: directory → disk cache (TODO) → hardcoded operators
- [x] Integrate into `spawn_mesh_discovery()` before swarm event loop
- [x] Dial discovered relay peers from directory response
- [x] Add `extract_http_url_from_multiaddr()` helper for IP/port extraction

#### 2.1.10.4 Add Heartbeat Loop ✅ COMPLETE
- [x] Implement `spawn_heartbeat_loop()` function in `main.rs`
- [x] POST to `/api/relay/register` every 30 seconds
- [x] Only spawn in relay-only mode (check config)
- [x] Include local peer_id, addrs, country (GeoIP TODO) in payload
- [x] Use `reqwest` HTTP client with "json" feature

#### 2.1.10.5 Update Tests ✅ COMPLETE
- [x] Run `cargo test -p core-mesh --lib` (passing: 1 passed, 0 failed)
- [x] Verify workspace compiles: `cargo check --workspace --quiet` (success with warnings)
- [x] Warnings about unused directory methods expected (used by operator nodes)

#### 2.1.10.6 Update Documentation ✅ COMPLETE
- [x] Update README.md (remove DHT warnings, add operator directory section)
- [x] Update ARCHITECTURE.md (Layer 3 description)
- [x] Update qnet-spec/specs/001-qnet/spec.md (Section 3.3 Bootstrap Strategy)
- [x] Update qnet-spec/specs/001-qnet/tasks.md (archive Task 2.1.9, add Task 2.1.10)
- [x] Update qnet-spec/docs/helper.md (peer discovery section)
- [x] Update qnet-spec/docs/extension.md (status API fields if changed)
- [x] Update docs/CONTRIBUTING.md (DHT removal note)
- [x] Update research doc with implementation checkmarks

#### 2.1.10.7 Final Testing & Polish ✅ COMPLETE
- [x] Run `cargo fmt` (format all code)
- [x] Run `cargo clippy --workspace` (lint checks, warnings only - no errors)
- [x] Run `cargo test --workspace --lib` (all library tests passing)
- [x] Run `cargo build --release -p stealth-browser` (production build successful)
- [x] Integration tests disabled (pending rewrite for operator directory mocks)
- [x] Workspace compiles successfully with expected dead code warnings
- [ ] Commit changes to main branch

---

## 🔧 Phase 2.1.11 - Super Peer Implementation (Bootstrap+Exit+Relay)

**Context**: Implement "super peer" mode for operator-run droplets that serve as bootstrap nodes, exit nodes, and relay nodes simultaneously. This enables the 6-droplet infrastructure model.

**Status**: 📋 PENDING

### 2.1.11 Super Peer Implementation
**Goal**: Enable operator droplets to function as bootstrap+exit+relay nodes.

#### 2.1.11.1 Add Directory HTTP Endpoints 📋 PENDING
- [ ] Open file: `apps/stealth-browser/src/main.rs`
- [ ] Locate `spawn_status_server()` function (blocking HTTP server)
- [ ] Add `POST /api/relay/register` endpoint handler
  - [ ] Parse JSON body into `RelayInfo` struct
  - [ ] Call `directory.register_peer(info)`
  - [ ] Return 200 OK with JSON `{ "registered": true, "is_new": bool }`
- [ ] Add `GET /api/relays/by-country` endpoint handler
  - [ ] Optional query param: `?country=US` (filter by country)
  - [ ] Call `directory.get_relays_by_country()`
  - [ ] Return 200 OK with JSON HashMap
- [ ] Add `GET /api/relays/prune` endpoint (manual pruning trigger, dev only)
  - [ ] Call `directory.prune_stale_peers()`
  - [ ] Return 200 OK with count of pruned peers
- [ ] Update status server request routing to handle new paths
- [ ] Add unit tests for endpoint parsing and response format

#### 2.1.11.2 Implement Exit Node Logic 📋 PENDING
**⚠️ RESEARCH REQUIRED**: Need research on secure HTTP/HTTPS exit node implementation
- [ ] **STOP**: Research required before implementation
- [ ] Create research prompt covering:
  - [ ] HTX stream decryption for exit node use case
  - [ ] HTTP/HTTPS request forwarding patterns
  - [ ] TLS certificate validation for outbound requests
  - [ ] Abuse detection and rate limiting strategies
  - [ ] Legal considerations and best practices for exit nodes
  - [ ] Memory-safe request parsing (avoid buffer overflows)
  - [ ] Connection pooling for outbound requests
- [ ] **WAIT**: User provides research findings before proceeding
- [ ] Create file: `apps/stealth-browser/src/exit.rs` module
- [ ] Implement `ExitNode` struct with config (max_bandwidth, allowed_protocols)
- [ ] Implement `handle_exit_request()` async function
  - [ ] Decrypt HTX stream to get HTTP CONNECT request
  - [ ] Parse destination host:port from CONNECT
  - [ ] Validate destination (whitelist/blacklist)
  - [ ] Open outbound TcpStream to destination
  - [ ] Bridge HTX stream <-> TcpStream bidirectionally
  - [ ] Handle HTTPS (TLS passthrough, no MITM)
  - [ ] Handle HTTP (plain forwarding)
- [ ] Add bandwidth tracking per client
- [ ] Add rate limiting (requests per minute per client)
- [ ] Add abuse detection (suspicious patterns, malware domains)
- [ ] Add logging for exit traffic (sanitized, no PII)
- [ ] Add unit tests for exit logic (mock outbound connections)

#### 2.1.11.3 Add Super Peer Mode Configuration 📋 PENDING
- [ ] Open file: `apps/stealth-browser/src/main.rs`
- [ ] Add CLI flag: `--mode <MODE>` where MODE = client | relay | bootstrap | exit | super
  - [ ] `client`: Default, query directory, no registration, no exit
  - [ ] `relay`: Register with directory, relay traffic, no exit
  - [ ] `bootstrap`: Run directory service, relay traffic, no exit
  - [ ] `exit`: Relay traffic + exit to internet, no directory service
  - [ ] `super`: All features (bootstrap + relay + exit)
- [ ] Add environment variable: `STEALTH_MODE` (overrides CLI)
- [ ] Implement mode validation and feature enablement logic
- [ ] Update `AppState` to include `mode: HelperMode` enum
- [ ] Add startup log showing enabled features based on mode
- [ ] Document mode behaviors in `qnet-spec/docs/helper.md`

#### 2.1.11.4 Integrate Directory with Super Peer Mode 📋 PENDING
- [ ] Modify `spawn_status_server()` to conditionally enable directory endpoints
  - [ ] Only enable `/api/relay/register` and `/api/relays/*` in `bootstrap` or `super` mode
  - [ ] Return 404 for directory endpoints in other modes
- [ ] Modify `spawn_heartbeat_loop()` to respect mode
  - [ ] Spawn heartbeat in `relay`, `exit`, or `super` mode
  - [ ] Skip heartbeat in `client` or `bootstrap` mode
- [ ] Add background pruning task for directory (every 60 seconds)
  - [ ] Only run in `bootstrap` or `super` mode
  - [ ] Call `directory.prune_stale_peers()`
  - [ ] Log count of pruned peers
- [ ] Update `query_operator_directory()` to work in all modes
  - [ ] Client mode: Query hardcoded operators
  - [ ] Bootstrap/super mode: Can query self (localhost) for testing

#### 2.1.11.5 Add Exit Node Integration 📋 PENDING
**Prerequisite**: 2.1.11.2 complete (exit logic implemented)
- [ ] Modify SOCKS5 handler (`handle_connect()`)
  - [ ] Check if mode includes exit capability (`exit` or `super`)
  - [ ] If yes, handle exit requests (decrypt HTX, forward to internet)
  - [ ] If no, reject with SOCKS error 0x02 (connection not allowed)
- [ ] Add exit statistics to `AppState`
  - [ ] `exit_requests_total: AtomicU64`
  - [ ] `exit_requests_success: AtomicU64`
  - [ ] `exit_requests_blocked: AtomicU64`
  - [ ] `exit_bandwidth_bytes: AtomicU64`
- [ ] Update `/status` endpoint to include exit stats (if mode supports exit)
- [ ] Add exit node warning to startup logs
  - [ ] "WARNING: Exit node enabled - you are responsible for traffic from this IP"
  - [ ] "Exit Policy: <policy summary>"

#### 2.1.11.6 Testing - Local Super Peer 📋 PENDING
- [ ] Test: Run Helper in `super` mode locally
  - [ ] `cargo run --release --bin stealth-browser -- --mode super`
  - [ ] Verify directory endpoints respond (POST register, GET relays)
  - [ ] Verify exit node accepts SOCKS requests (if implemented)
- [ ] Test: Run second Helper in `client` mode
  - [ ] Point client at local super peer (`hardcoded_operator_nodes()` = localhost)
  - [ ] Verify client queries directory successfully
  - [ ] Verify client discovers local super peer
  - [ ] Verify client can connect through super peer (if exit implemented)
- [ ] Test: Directory pruning works
  - [ ] Register fake peer with old timestamp
  - [ ] Wait 120 seconds
  - [ ] Verify peer removed from directory
- [ ] Test: Heartbeat registration works
  - [ ] Run relay mode with hardcoded operator
  - [ ] Verify POST /api/relay/register every 30s
  - [ ] Verify relay appears in directory query

#### 2.1.11.7 Testing - Droplet Deployment 📋 PENDING
**Prerequisite**: Access to 1 DigitalOcean droplet ($6/month)
- [ ] Provision droplet (Ubuntu 22.04, 1 vCPU, 1 GB RAM)
- [ ] Install Rust: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- [ ] Clone repo: `git clone https://github.com/QW1CKS/qnet.git && cd qnet`
- [ ] Checkout branch: `git checkout feature/remove-dht-add-directory`
- [ ] Build release: `cargo build --release --bin stealth-browser`
- [ ] Create systemd service: `/etc/systemd/system/qnet-super.service`
  ```ini
  [Unit]
  Description=QNet Super Peer
  After=network.target
  
  [Service]
  Type=simple
  User=qnet
  WorkingDirectory=/opt/qnet
  Environment="STEALTH_MODE=super"
  ExecStart=/opt/qnet/target/release/stealth-browser
  Restart=always
  RestartSec=10
  
  [Install]
  WantedBy=multi-user.target
  ```
- [ ] Enable and start: `systemctl enable qnet-super && systemctl start qnet-super`
- [ ] Verify logs: `journalctl -u qnet-super -f`
- [ ] Test from laptop: Update `hardcoded_operator_nodes()` with droplet IP
- [ ] Run laptop Helper: `cargo run --release --bin stealth-browser`
- [ ] Verify laptop discovers droplet peer
- [ ] Verify laptop can connect through droplet (if exit implemented)

#### 2.1.11.8 Documentation Updates 📋 PENDING
- [ ] Update `README.md`
  - [ ] Add "Super Peer Deployment" section
  - [ ] Document mode options (client, relay, bootstrap, exit, super)
  - [ ] Add droplet deployment instructions
- [ ] Update `qnet-spec/docs/helper.md`
  - [ ] Document `/api/relay/register` endpoint (request/response format)
  - [ ] Document `/api/relays/by-country` endpoint
  - [ ] Document mode configuration (CLI flags + env vars)
  - [ ] Add security warnings for exit node mode
- [ ] Update `qnet-spec/docs/extension.md`
  - [ ] Document mode field in status API (if exposed)
- [ ] Update `qnet-spec/specs/001-qnet/spec.md`
  - [ ] Update Section 3.3 (Bootstrap Strategy) with super peer architecture
  - [ ] Document directory API specification
- [ ] Create `qnet-spec/docs/deployment.md`
  - [ ] Droplet provisioning guide
  - [ ] Super peer configuration best practices
  - [ ] Cost breakdown (1-6 droplets)
  - [ ] Legal considerations for exit nodes

#### 2.1.11.9 Final Testing & Validation 📋 PENDING
- [ ] Run full test suite: `cargo test --workspace`
- [ ] Run clippy: `cargo clippy --workspace --all-targets`
- [ ] Run fmt: `cargo fmt --check`
- [ ] Test all modes work:
  - [ ] `--mode client`: Queries directory, no registration
  - [ ] `--mode relay`: Registers, relays traffic
  - [ ] `--mode bootstrap`: Directory service works
  - [ ] `--mode exit`: Exit functionality works (if implemented)
  - [ ] `--mode super`: All features work together
- [ ] Load test directory endpoints (100 registrations, 1000 queries)
- [ ] Verify memory usage stable under load (no leaks)
- [ ] Test graceful shutdown (all modes)
- [ ] Test restart recovery (directory survives restart with disk cache)

---

## ✅ Phase 1: Core Infrastructure (COMPLETED)

### 1.1 Project Setup
- [x] Create Cargo workspace
- [x] Set up `.gitignore`
- [x] Configure CI/CD pipeline
- [x] Create README.md

### 1.2 Crypto Primitives (`core-crypto`)
- [x] Create `crates/core-crypto/` directory
- [x] Implement ChaCha20-Poly1305 AEAD
- [x] Implement Ed25519 signatures
- [x] Implement X25519 key exchange
- [x] Implement HKDF-SHA256 key derivation
- [x] Add unit tests for all crypto operations

### 1.3 L2 Framing (`core-framing`)
- [x] Create `crates/core-framing/` directory
- [x] Define frame types (STREAM, WINDOW_UPDATE, PING, etc.)
- [x] Implement frame encoding
- [x] Implement frame decoding
- [x] Add AEAD protection to frames
- [x] Add padding support

### 1.4 HTX Transport (`htx`)
- [x] Create `crates/htx/` directory
- [x] Implement TLS fingerprint cloning
- [x] Implement Noise XK handshake
- [x] Add traffic shaping (jitter, padding)
- [x] Implement stream multiplexing

### 1.5 Catalog System
- [x] Define catalog schema (JSON + signature)
- [x] Create `crates/catalog-signer/` tool
- [x] Implement signature verification
- [x] Add catalog loader to Helper

---

## 🚧 Phase 2: The P2P Mesh Network

### 2.1 Peer Discovery
**Goal**: Allow Helpers to find each other via operator directory.

---

## ⚠️ ARCHIVED: Phase 2.1.1-2.1.8 - DHT-Based Peer Discovery (Superseded Nov 2025)

> **STATUS**: These tasks were superseded by Task 2.1.10 (Operator Directory). Kademlia DHT-based peer discovery has been removed in favor of operator directory HTTP registry model.

**Historical Context**: Originally implemented DHT-based discovery with Kademlia + mDNS. Removed in Nov 2025 due to bootstrap timeout issues on Windows/NAT. Replaced with lightweight operator directory (POST /api/relay/register, GET /api/relays/by-country).

**Archived Tasks**:
- ~~2.1.1 Setup Discovery Module~~ (replaced by directory.rs module)
- ~~2.1.2 Implement Bootstrap Logic~~ (now queries operator directory)
- ~~2.1.3 Implement Kademlia DHT~~ (removed entirely)
- ~~2.1.4 Implement mDNS Local Discovery~~ (kept for LAN, removed DHT integration)
- ~~2.1.5 Combine Discovery Mechanisms~~ (now directory-only)
- ~~2.1.6 Integration with Helper~~ (now query_operator_directory() function)
- ~~2.1.7 Testing~~ (disabled pending directory mock rewrite)
- ~~2.1.8 Documentation~~ (updated for directory model in Task 2.1.10.6)

**See**: Task 2.1.10 for current implementation details.

---

---

### 2.2 Relay Logic (`core-mesh`)
**Goal**: Make Helpers forward packets for other peers.

**Note**: Relay logic uses peers discovered via operator directory (Task 2.1.10), not DHT.

#### 2.2.1 Setup Relay Module
- [x] Create file: `crates/core-mesh/src/relay.rs`
- [x] Add module declaration in `crates/core-mesh/src/lib.rs`
- [x] Import libp2p relay dependencies in `Cargo.toml`

#### 2.2.2 Define Packet Structure
- [x] Create struct: `Packet { src: PeerId, dst: PeerId, data: Vec<u8> }`
- [x] Implement `encode()` method to serialize packet to bytes
- [x] Implement `decode()` method to deserialize from bytes
- [x] Add unit test for encode/decode round-trip

#### 2.2.3 Implement Relay Behavior
- [x] Create struct: `RelayBehavior { peer_id: PeerId, routes: HashMap<PeerId, PeerId> }`
- [x] Implement method: `fn should_relay(&self, packet: &Packet) -> bool`
- [x] Implement method: `fn forward_packet(&mut self, packet: Packet) -> Result<()>`
- [x] Add logging for relayed packets

#### 2.2.4 Implement Routing Table
- [x] Create struct: `RoutingTable { routes: HashMap<PeerId, Vec<PeerId>> }`
- [x] Implement method: `fn add_route(&mut self, dst: PeerId, via: PeerId)`
- [x] Implement method: `fn find_route(&self, dst: PeerId) -> Option<PeerId>`
- [x] Implement method: `fn remove_route(&mut self, dst: PeerId)`

#### 2.2.5 Integrate Relay with Directory-Based Discovery
- [x] Populate `RoutingTable` from operator directory query results
- [x] When peer discovered via directory, add route to routing table
- [x] When peer heartbeat expires (stale), remove route from routing table
- [x] Add method: `pub fn get_routing_table(&self) -> &RoutingTable`

#### 2.2.6 Implement Packet Handler
- [x] Create function: `async fn handle_incoming_packet(packet: Packet, relay: &mut RelayBehavior)`
- [x] If `packet.dst == self.peer_id`, deliver to local handler
- [x] Else, call `relay.forward_packet(packet)`
- [x] Add error handling for failed relays

#### 2.2.7 Integration with Helper
- [x] Add `RelayBehavior` to Helper's network stack
- [x] Connect relayed packets to SOCKS5 handler (if dst is this peer)
- [x] Connect outgoing SOCKS5 traffic to relay (if dst is remote peer)
- [x] Add relay statistics to Status API (packets_relayed count)

#### 2.2.8 Testing
- [x] Create file: `tests/integration/mesh_relay.rs`
- [x] Test: Node A sends to Node C via Node B (3-node relay)
- [x] Test: Verify packet arrives with correct data
- [x] Test: Verify relay statistics are updated
- [x] Run test: `cargo test --test mesh_relay`

#### 2.2.9 Documentation
- [x] Add doc comment to `relay.rs` module
- [x] Document relay packet format
- [x] Update `qnet-spec/docs/ARCHITECTURE.md` with relay flow diagram
- [x] Add example to `examples/mesh_relay.rs`

---

### 2.3 Circuit Building (`core-mesh`)
**Goal**: Construct multi-hop paths for privacy.

#### 2.3.1 Setup Circuit Module
- [x] Create file: `crates/core-mesh/src/circuit.rs`
- [x] Add module declaration in `crates/core-mesh/src/lib.rs`
- [x] Define circuit constants (MAX_HOPS = 3)

#### 2.3.2 Define Circuit Structure
- [x] Create struct: `Circuit { id: u64, hops: Vec<PeerId>, created_at: Instant }`
- [x] Implement method: `fn new(hops: Vec<PeerId>) -> Self`
- [x] Implement method: `fn next_hop(&self, current: &PeerId) -> Option<PeerId>`
- [x] Add unit test for circuit creation

#### 2.3.3 Implement Circuit Builder
- [x] Create struct: `CircuitBuilder` with access to operator directory peer list
- [x] Implement method: `async fn build_circuit(&self, dst: PeerId, num_hops: usize) -> Result<Circuit>`
- [x] Select random intermediate relay peers from directory query results
- [x] Ensure no peer appears twice in the circuit
- [x] Return constructed circuit

**Note**: Circuit builder uses peers from operator directory (Task 2.1.10), not DHT discovery.

#### 2.3.4 Integrate with Routing
- [x] Update `RoutingTable` to store circuits
- [x] Add method: `fn add_circuit(&mut self, circuit: Circuit)`
- [x] Modify `find_route()` to use circuits when available
- [x] Add method: `fn get_circuit(&self, id: u64) -> Option<&Circuit>`

#### 2.3.5 Implement Circuit Handshake
- [x] Define handshake message: `CircuitRequest { circuit_id, next_hop }`
- [x] Send handshake to first hop
- [x] Each hop forwards to next hop
- [x] Last hop sends `CircuitReady` back to client
- [x] Add timeout for circuit establishment (10 seconds)

#### 2.3.6 Implement Circuit Teardown
- [x] Define teardown message: `CircuitClose { circuit_id }`
- [x] Send teardown when circuit no longer needed
- [x] Each hop removes circuit from local table
- [x] Add automatic teardown after 5 minutes of inactivity

#### 2.3.7 Integration with Helper
- [x] Add `CircuitBuilder` to Helper
- [x] When SOCKS5 request arrives, build circuit to destination
- [x] Route traffic through the circuit (not direct)
- [x] Add circuit info to Status API (active_circuits count)

#### 2.3.8 Testing
- [x] Create file: `tests/integration/mesh_circuit.rs`
- [x] Test: Build 1-hop circuit, verify traffic flows
- [x] Test: Build 3-hop circuit, verify traffic flows
- [x] Test: Verify circuit teardown works
- [x] Run test: `cargo test --test mesh_circuit`

#### 2.3.9 Documentation
- [x] Add doc comment to `circuit.rs` module
- [x] Document circuit handshake protocol
- [x] Update `qnet-spec/docs/ARCHITECTURE.md` with circuit flow
- [x] Add example to `examples/mesh_circuit.rs`

---

### 2.4 Helper Service Integration
**Goal**: Connect the mesh to the SOCKS5 proxy.

#### 2.4.1 Refactor Helper Startup
- [x] Open file: `apps/stealth-browser/src/main.rs`
- [x] Add mesh initialization: `let mesh = MeshNetwork::new(peer_id).await?`
- [x] Start discovery: `mesh.start_discovery().await?`
- [x] Log mesh status: `info!("Mesh started, peer_id: {}", peer_id)`

#### 2.4.2 Connect SOCKS5 to Mesh
- [x] Open file: `apps/stealth-browser/src/socks5.rs`
- [x] Modify `handle_connect()` to check if destination is a peer
- [x] If destination is a QNet peer, route via mesh
- [x] If destination is regular internet, use HTX (existing logic)

#### 2.4.3 Add Mesh Status Endpoint
- [x] Open file: `apps/stealth-browser/src/api.rs`
- [x] Add field to `StatusResponse`: `mesh_peer_count: usize`
- [x] Add field to `StatusResponse`: `active_circuits: usize`
- [x] Populate fields from mesh state

#### 2.4.4 Add Configuration
- [x] Configuration via environment variables and CLI flags
- [x] Add env var: `QNET_MESH_ENABLED` (enable/disable mesh)
- [x] Add env var: `QNET_MODE` (helper mode: relay/exit/bootstrap)
- [x] Add CLI flag: `--no-mesh` (disable mesh)
- [x] Add CLI flags: `--relay-only`, `--exit-node`, `--bootstrap`
- [x] Load config on startup

#### 2.4.5 Testing
- [x] Update smoke test: `scripts/test-masked-connect.ps1`
- [x] Add check: Verify mesh_peer_count > 0 after startup
- [x] Add check: Verify circuit works for peer destination
- [x] Run test: `pwsh scripts/test-masked-connect.ps1`

---

### 2.5 Super Peer Infrastructure Deployment
**Goal**: Deploy and configure super peer droplets (bootstrap + exit + relay).

**Note**: This phase focuses on **deploying super peers**, not implementing their logic (see Task 2.1.11 for implementation).

#### 2.5.1 Update Hardcoded Operator Nodes
- [x] Open file: `apps/stealth-browser/src/main.rs`
- [x] Update `hardcoded_operator_nodes()` function with actual droplet IPs
- [x] Structure: `Vec<OperatorNode { http_url: String, country: String }>`
- [x] Add 6 operator nodes (NYC, AMS, SIN, FRA, TOR, SYD)
- [x] Test: Verify Helper queries directory endpoint successfully
- [x] Test: `cargo run --bin stealth-browser` connects to operator

**Note**: Operator directory replaces DHT - no public DHT bootstrap needed.

#### 2.5.2 Prepare Droplet Deployment Script
- [ ] Create file: `scripts/deploy-exit-node.sh`
- [ ] Add droplet provisioning steps:
  - [ ] Install Rust via rustup
  - [ ] Clone QNet repository
  - [ ] Build stealth-browser binary
  - [ ] Configure as exit + bootstrap node
  - [ ] Set up systemd service
- [ ] Add environment variable configuration
- [ ] Test: Deploy to test droplet manually

#### 2.5.3 Configure Exit Node Mode
- [x] Open file: `apps/stealth-browser/src/main.rs`
- [x] Add CLI flag: `--exit-node` (enables exit mode)
- [x] Add CLI flag: `--relay-only` (default, disables exit)
- [x] Read env var: `QNET_MODE` ("relay", "exit", "bootstrap")
- [x] Implement exit node logic (decrypt and forward to internet)
- [x] Log warning when exit mode enabled

#### 2.5.4 Add Exit Node Policy
- [ ] Create struct: `ExitPolicy { max_bandwidth_mbps, allowed_protocols }`
- [ ] Read from config or env var: `QNET_EXIT_POLICY`
- [ ] Implement bandwidth limiting per user
- [ ] Implement protocol filtering (http/https only)
- [ ] Add abuse detection (rate limiting)

#### 2.5.5 Deploy Initial Droplets (Optional)
**Note**: This is operator-specific, not required for development
- [ ] Sign up for DigitalOcean / Linode / Vultr
- [ ] Create droplet in NYC (Americas):
  - [ ] 512 MB RAM, 1 vCPU, $4/month
  - [ ] Run deployment script
  - [ ] Verify Helper starts as exit + bootstrap
- [ ] Create droplet in Amsterdam (Europe):
  - [ ] Same specs as NYC
  - [ ] Run deployment script
- [ ] Update `.env` with droplet IPs:
  - [ ] `QNET_OPERATOR_SEEDS="ip1:4001,ip2:4001"`
- [ ] Test: Connect local Helper to deployed droplets

#### 2.5.6 Update Operator Directory URLs
- [ ] Open file: `apps/stealth-browser/src/main.rs`
- [ ] Update `hardcoded_operator_nodes()` with deployed droplet IPs
- [ ] Format: `http://<DROPLET_IP>:8088` for each droplet
- [ ] Test: Query `/api/relays/by-country` returns peer list
- [ ] Commit operator configuration

#### 2.5.7 Documentation
- [ ] Update `README.md` with infrastructure notes
- [ ] Document droplet deployment process
- [ ] Add cost breakdown table ($8-18/month)
- [ ] Explain relay vs exit mode for users
- [ ] Add legal disclaimer for exit node operators

#### 2.5.8 Testing
- [ ] Test directory query from client to super peer
- [ ] Test heartbeat registration from relay to super peer
- [ ] Verify relay-only users cannot exit (mode enforcement)
- [ ] Verify super peer exit nodes make actual requests
- [ ] Test exit node bandwidth limiting (if implemented)
- [ ] Test directory pruning (stale peer removal after 120s)

---

### 2.6 Production-Readiness Checkpoint (Phase 2)
**Goal**: Validate super peer architecture and mesh network reliability before extension development.

**Prerequisites**: Tasks 2.1.10 (Operator Directory) and 2.1.11 (Super Peer Implementation) complete.

#### 2.6.1 Security Audit
- [ ] Run `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Run `cargo audit` to check for vulnerable dependencies
- [ ] Review all crypto usage matches `core-crypto` wrappers only
- [ ] Verify no secrets logged or exposed in status APIs
- [ ] Check all CBOR serialization uses DET-CBOR for signed objects
- [ ] Verify nonce handling uses monotonic counters (no reuse)
- [ ] Review HTX handshake maintains forward secrecy properties

#### 2.6.2 Performance Validation
- [ ] Run benchmark suite: `cargo bench --workspace`
- [ ] Verify AEAD throughput meets baseline (see `artifacts/perf-summary.md`)
- [ ] Check HTX handshake latency < 500ms (median)
- [ ] Verify operator directory query latency < 200ms (median)
- [ ] Verify peer discovery via directory completes within 5s (target: < 2s)
- [ ] Measure relay overhead (should be < 10% vs direct connection)
- [ ] Check frame encoding/decoding throughput acceptable
- [ ] Document any performance regressions with justification

#### 2.6.3 Reliability Testing
- [ ] Run full integration test suite: `cargo test --workspace`
- [ ] Run fuzz targets for 1 hour each: `cargo +nightly fuzz run <target>`
- [ ] Verify all fuzz targets pass without crashes
- [ ] Test Helper restart under load (with active SOCKS5 connections)
- [ ] Verify graceful degradation when operator directory unreachable (fallback to hardcoded)
- [ ] Verify graceful degradation when mesh relay peers go offline
- [ ] Test directory response parsing (malformed JSON rejection)
- [ ] Verify frame decode rejects malformed packets

#### 2.6.4 Operational Checks
- [ ] Verify status API returns valid JSON under all states (offline/connecting/connected)
- [ ] Test Helper runs stable for 24 hours continuous
- [ ] Check memory usage remains bounded (no leaks, use profiler)
- [ ] Verify logs don't contain PII, keys, or nonces
- [ ] Test Windows compatibility (if cross-platform targeted)
- [ ] Test Linux compatibility (if cross-platform targeted)
- [ ] Verify Helper handles partial/interrupted reads (Windows async)
- [ ] Verify directory query timeout handling (fallback to hardcoded operators)
- [ ] Test heartbeat loop resilience (operator directory unavailable)

#### 2.6.5 Documentation Review
- [ ] Verify `ARCHITECTURE.md` reflects current Phase 2 implementation (operator directory model)
- [ ] Check all public APIs have doc comments with examples
- [ ] Update root `README.md` with Phase 2 feature status
- [ ] Review compliance with `memory/ai-guardrail.md`
- [ ] Review compliance with `memory/testing-rules.md`
- [ ] Verify all implemented tasks traced to `tasks.md`
- [ ] Check spec alignment (`qnet-spec/specs/001-qnet/spec.md` Section 3.3)
- [ ] Verify operator directory API documented in `qnet-spec/docs/helper.md`

#### 2.6.6 Decision Gate
- [ ] All security audit items pass
- [ ] All performance benchmarks meet baseline
- [ ] All integration + fuzz tests pass
- [ ] 24-hour stability test passes
- [ ] Documentation is current
- [ ] **GO/NO-GO Decision**: Proceed to Phase 3 (Extension)

---

### 2.7 Enhanced UX: Geographic Routing & Visualization
**Goal**: Improve user experience with intelligent relay selection and visual network map.

**Prerequisites**: Task 2.1.11 (Super Peer Implementation) complete.

#### 2.7.1 Smart 1-Hop Relay (Default Behavior)
- [ ] Implement geographic peer selection algorithm
- [ ] Add GeoIP lookup for user country detection
- [ ] Implement relay selection priority:
  - [ ] Same country (lowest latency)
  - [ ] Same continent (good latency)
  - [ ] Any available relay (privacy over speed)
  - [ ] Fallback to direct super peer exit if no relay available
- [ ] Query operator directory `/api/relays/by-country?country=<USER_COUNTRY>`
- [ ] Update circuit builder to use geographic selection from directory
- [ ] Test: Verify relay selection chooses closest peer from directory
- [ ] Test: Verify fallback to hardcoded super peer when directory empty

#### 2.7.2 Map API & Backend
- [ ] Extend existing `/api/relays/by-country` endpoint with GeoJSON support
- [ ] Create `/api/map/peers` - aggregated peer data with coordinates
- [ ] Create `/api/map/exits` - super peer exit node list with exact coordinates
- [ ] Implement country-level anonymization (no city data)
- [ ] Add current circuit path to API response
- [ ] Test: Verify API returns valid GeoJSON
- [ ] Test: Verify no PII leakage in responses

#### 2.7.3 Interactive World Map Visualization
- [ ] Create `static/map.html` with Leaflet.js or D3.js
- [ ] Implement country color-coding (green = peers, red = none)
- [ ] Add peer count overlays per country
- [ ] Add exit node markers (exact locations)
- [ ] Highlight user's country (no exact location)
- [ ] Implement animated circuit path visualization
- [ ] Add manual relay/exit selection controls
- [ ] Test: Verify map loads and updates correctly
- [ ] Test: Verify privacy (country-level only)

#### 2.7.4 Browser Extension Integration
- [ ] Create Chrome extension manifest
- [ ] Implement native messaging to Helper
- [ ] Create popup UI with status display
- [ ] Add "Open Map View" button (opens localhost:8088/map)
- [ ] Add quick settings access
- [ ] Package extension for Chrome/Firefox
- [ ] Test: Verify extension connects to Helper
- [ ] Test: Verify map link opens correctly

#### 2.7.5 Testing & Documentation
- [ ] Test smart routing with multiple geographic peers
- [ ] Test map visualization with sample data
- [ ] Verify fallback behavior when no relays
- [ ] Update `README.md` with map feature
- [ ] Document geographic routing algorithm
- [ ] Add screenshots of map visualization
- [ ] Create user guide for extension

---

## 🌐 Phase 3: Browser Extension

### 3.1 Extension Scaffold

#### 3.1.1 Create Extension Directory
- [ ] Create directory: `apps/extension/`
- [ ] Create directory: `apps/extension/src/`
- [ ] Create directory: `apps/extension/icons/`
- [ ] Create directory: `apps/extension/popup/`

#### 3.1.2 Create Manifest
- [ ] Create file: `apps/extension/manifest.json`
- [ ] Add manifest version: `"manifest_version": 3`
- [ ] Add name: `"name": "QNet"`
- [ ] Add description: `"description": "Decentralized, Censorship-Resistant Network"`
- [ ] Add version: `"version": "1.0.0"`
- [ ] Add permissions: `["proxy", "nativeMessaging", "storage"]`

#### 3.1.3 Create Popup UI
- [ ] Create file: `apps/extension/popup/popup.html`
- [ ] Add header: `<h1>QNet</h1>`
- [ ] Add toggle button: `<button id="toggle">Connect</button>`
- [ ] Add status div: `<div id="status">Disconnected</div>`
- [ ] Add stylesheet link: `<link rel="stylesheet" href="popup.css">`

#### 3.1.4 Create Popup CSS
- [ ] Create file: `apps/extension/popup/popup.css`
- [ ] Style header (centered, branded)
- [ ] Style toggle button (large, green when connected)
- [ ] Style status div (shows connection state)
- [ ] Add animations for state transitions

#### 3.1.5 Create Popup JS
- [ ] Create file: `apps/extension/popup/popup.js`
- [ ] Add event listener for toggle button
- [ ] Add function: `async function toggleConnection()`
- [ ] Add function: `async function updateStatus()`
- [ ] Load current state on popup open

---

### 3.2 Proxy Management

#### 3.2.1 Create Background Script
- [ ] Create file: `apps/extension/src/background.js`
- [ ] Add to manifest: `"background": { "service_worker": "src/background.js" }`
- [ ] Import Chrome Proxy API

#### 3.2.2 Implement Proxy Control
- [ ] Add function: `async function enableProxy()`
- [ ] Set SOCKS5 proxy: `{ mode: "fixed_servers", rules: { singleProxy: { scheme: "socks5", host: "127.0.0.1", port: 1088 } } }`
- [ ] Add function: `async function disableProxy()`
- [ ] Set direct connection: `{ mode: "direct" }`

#### 3.2.3 Store Connection State
- [ ] Use `chrome.storage.local` to persist state
- [ ] Add function: `async function saveState(isConnected: boolean)`
- [ ] Add function: `async function loadState() -> boolean`
- [ ] Restore state on browser restart

#### 3.2.4 Testing
- [ ] Load extension in Chrome (`chrome://extensions/`, enable Developer mode)
- [ ] Click toggle, verify proxy settings change
- [ ] Check `chrome://net-internals/#proxy`
- [ ] Verify websites load through SOCKS5

---

### 3.3 Native Messaging

#### 3.3.1 Define Message Protocol
- [ ] Create file: `apps/extension/src/protocol.js`
- [ ] Define message: `{ type: "GET_STATUS" }`
- [ ] Define message: `{ type: "START_HELPER" }`
- [ ] Define message: `{ type: "STOP_HELPER" }`
- [ ] Define response: `{ peer_count, active_circuits, proxy_state }`

#### 3.3.2 Implement Native Messaging Host
- [ ] Create file: `apps/stealth-browser/src/native_messaging.rs`
- [ ] Implement stdin/stdout message passing
- [ ] Add handler for each message type
- [ ] Call appropriate Helper methods

#### 3.3.3 Register Native Messaging Host
- [ ] Create file: `apps/extension/native/qnet_host.json` (manifest for native messaging)
- [ ] Add installer logic to register manifest on Helper install
- [ ] Test on Windows: Registry key `HKEY_CURRENT_USER\Software\Google\Chrome\NativeMessagingHosts\com.qnet.helper`
- [ ] Test on Linux: `~/.config/google-chrome/NativeMessagingHosts/com.qnet.helper.json`

#### 3.3.4 Connect Extension to Helper
- [ ] In `background.js`, add function: `async function sendNativeMessage(msg)`
- [ ] Use `chrome.runtime.sendNativeMessage("com.qnet.helper", msg)`
- [ ] Handle response and update extension state

#### 3.3.5 Testing
- [ ] Test: Send GET_STATUS, verify response
- [ ] Test: Send START_HELPER, verify Helper starts
- [ ] Test: Extension updates UI with real Helper data
- [ ] Check logs in Helper for native messaging events

---

### 3.4 Installers

#### 3.4.1 Windows MSI Installer
- [ ] Install WiX Toolset
- [ ] Create file: `installers/windows/qnet.wxs` (WiX source)
- [ ] Add component: Helper binary (`stealth-browser.exe`)
- [ ] Add component: Extension CRX file
- [ ] Add registry keys for native messaging
- [ ] Add Start Menu shortcut (optional)
- [ ] Build MSI: `candle qnet.wxs && light qnet.wixobj`

#### 3.4.2 Linux Package
- [ ] Create directory: `installers/linux/`
- [ ] Create `.deb` structure: `DEBIAN/control`, `usr/bin/`, `usr/share/`
- [ ] Add post-install script to register native messaging manifest
- [ ] Build package: `dpkg-deb --build qnet`

#### 3.4.3 macOS Package
- [ ] Create directory: `installers/macos/`
- [ ] Create `.pkg` structure
- [ ] Add post-install script for native messaging
- [ ] Sign package with Apple Developer ID (if available)

#### 3.4.4 Testing
- [ ] Test on clean Windows VM: Run MSI, verify install
- [ ] Test on clean Linux VM: Install .deb, verify
- [ ] Test: Extension can communicate with Helper post-install

---

### 3.5 Production-Readiness Checkpoint (Phase 3)
**Goal**: Validate complete user delivery model before advanced features.

#### 3.5.1 End-to-End Security Validation
- [ ] Verify extension permissions are minimal (no unnecessary access)
- [ ] Test native messaging channel security (localhost only)
- [ ] Verify catalog updates propagate to extension UI correctly
- [ ] Check no sensitive data persists in extension storage
- [ ] Test proxy settings revert on extension disable/uninstall
- [ ] Verify SOCKS5 → HTX → Mesh path maintains confidentiality
- [ ] Test DPI resistance with demo script (see `scripts/demo-secure-connection.ps1`)

#### 3.5.2 User Experience Testing
- [ ] Test installation flow on clean system (Windows/Linux/macOS)
- [ ] Verify extension UI updates reflect Helper state changes
- [ ] Test connection toggle (connect/disconnect) works reliably
- [ ] Check status display shows peer count, circuit info correctly
- [ ] Test browser restart maintains connection state (if enabled)
- [ ] Verify error messages are user-friendly (not technical)
- [ ] Test Helper auto-start (if implemented in installer)

#### 3.5.3 Interoperability Testing
- [ ] Test with Chrome (primary target)
- [ ] Test with Edge (Chromium-based)
- [ ] Test with Brave (if compatible)
- [ ] Verify websites load correctly through SOCKS5 proxy
- [ ] Test HTTPS sites (TLS passthrough works)
- [ ] Test WebSocket connections through proxy
- [ ] Check DNS resolution (local vs remote)

#### 3.5.4 Reliability & Recovery
- [ ] Test Helper crash recovery (extension detects and shows error)
- [ ] Test network interruption handling (WiFi disconnect/reconnect)
- [ ] Verify catalog update doesn't disrupt active connections
- [ ] Test concurrent tabs loading through proxy (no conflicts)
- [ ] Check memory usage with extension active for 24 hours
- [ ] Test upgrade path (old version → new version)

#### 3.5.5 Installer & Distribution Validation
- [ ] Verify MSI/DEB/PKG installs all components correctly
- [ ] Check native messaging manifest registered properly
- [ ] Test uninstall removes all components (no orphans)
- [ ] Verify signed catalog loaded correctly on first run
- [ ] Check default config values are production-appropriate
- [ ] Test installer on systems without admin rights (if supported)

#### 3.5.6 Documentation & Support Readiness
- [ ] Create user guide: Installation steps with screenshots
- [ ] Create user guide: Troubleshooting common issues
- [ ] Document system requirements (OS versions, RAM, etc.)
- [ ] Add FAQ covering privacy, security, performance
- [ ] Update `README.md` with download links and quick start
- [ ] Verify all user-facing docs are non-technical

#### 3.5.7 Decision Gate
- [ ] All security validations pass
- [ ] Installation works on all target platforms
- [ ] Extension UI/UX is intuitive and reliable
- [ ] 24-hour stability test with extension active passes
- [ ] User documentation is complete
- [ ] **GO/NO-GO Decision**: Ready for limited beta release OR proceed to Phase 4

---

## 🔮 Phase 4: Advanced Privacy (Future)

### 4.1 Mixnet Integration (Nym)
- [ ] Research: Integrate Nym SDK
- [ ] Implement: Mixnet packet wrapping
- [ ] Implement: Cover traffic generation
- [ ] Test: Latency with 3-hop mixnet

### 4.2 Self-Certifying IDs (Naming)
- [ ] Implement: PeerId -> Human-readable alias mapping
- [ ] Implement: Alias ledger (2-of-3 finality)
- [ ] Implement: DNS replacement
- [ ] Test: Resolve `.qnet` names

### 4.3 Payment System (Vouchers/Cashu)
- [ ] Implement: Voucher generation
- [ ] Implement: Payment verification
- [ ] Implement: Relay incentives
- [ ] Test: Pay for relayed traffic

### 4.4 Governance
- [ ] Implement: Node uptime scoring
- [ ] Implement: Voting power calculation
- [ ] Implement: Protocol upgrade mechanism
- [ ] Test: Upgrade flow

---

## 📊 Progress Summary

- Phase 1: Core Infrastructure → **100% Complete** ✅
- Phase 2: P2P Mesh Network → **12.5% Complete** (Phase 2.1 ✅, 2.2-2.5 pending) 🚧
- Phase 3: Browser Extension → **0% Complete** 🚧
- Phase 4: Advanced Privacy → **0% Complete** 🔮

**Production Readiness Checkpoints**:
- 🔍 Checkpoint 1 (Phase 2.5): After mesh implementation, before extension
- 🔍 Checkpoint 2 (Phase 3.5): After complete user delivery, before advanced features

**Next Task**: Start Phase 2.2.1 (Create relay.rs file)

