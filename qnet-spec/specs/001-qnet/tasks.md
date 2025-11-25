# QNet Implementation Tasks - Detailed Checklist

> **How to use this**: Check off each item as you complete it. Work from top to bottom. Each item is designed to be small enough to complete in one focused session.

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

### 2.1 Peer Discovery (`core-mesh`)
**Goal**: Allow Helpers to find each other automatically.

#### 2.1.1 Setup Discovery Module
- [x] Create file: `crates/core-mesh/src/discovery.rs`
- [x] Add module declaration in `crates/core-mesh/src/lib.rs`
- [x] Import libp2p Kademlia DHT dependencies in `Cargo.toml`

#### 2.1.2 Implement Bootstrap Logic
- [x] Define bootstrap node struct `BootstrapNode { peer_id, multiaddr }`
- [x] Create function: `load_bootstrap_nodes() -> Vec<BootstrapNode>`
- [x] Load bootstrap nodes from catalog
- [x] Add fallback to hardcoded seeds if catalog fails

#### 2.1.3 Implement Kademlia DHT
- [x] Create Kademlia behavior: `let kademlia = Kademlia::new(peer_id, store)`
- [x] Add bootstrap peers to Kademlia routing table
- [x] Implement periodic DHT refresh (every 5 minutes)
- [x] Add logging for DHT events (peer discovered, peer lost)

#### 2.1.4 Implement mDNS Local Discovery
- [x] Create mDNS behavior: `let mdns = Mdns::new(MdnsConfig::default())`
- [x] Listen for mDNS events (peer discovered on LAN)
- [x] Add discovered peers to Kademlia
- [x] Add logging for local peer discovery

#### 2.1.5 Combine Discovery Mechanisms
- [x] Create `DiscoveryBehavior` struct combining Kademlia + mDNS
- [x] Implement `NetworkBehaviour` trait for `DiscoveryBehavior`
- [x] Add method: `pub async fn discover_peers(&mut self) -> Result<Vec<PeerId>>`
- [x] Add method: `pub fn peer_count(&self) -> usize`

#### 2.1.6 Integration with Helper
- [x] Add `DiscoveryBehavior` to Helper's network stack
- [x] Start discovery on Helper startup
- [x] Log discovered peers to console
- [x] Expose peer count via Status API (`/status` endpoint)

#### 2.1.7 Testing
- [x] Create file: `tests/integration/mesh_discovery.rs`
- [x] Test: Start 3 Helpers, verify they discover each other (mDNS)
- [x] Test: Start Helper with bootstrap nodes, verify DHT discovery
- [x] Test: Verify peer count increases as nodes join
- [x] Run test: `cargo test --test mesh_discovery`

#### 2.1.8 Documentation
- [x] Add doc comment to `discovery.rs` module
- [x] Document `discover_peers()` function
- [x] Update `qnet-spec/docs/ARCHITECTURE.md` with discovery flow
- [x] Add example to `examples/mesh_discovery.rs`

---

### 2.2 Relay Logic (`core-mesh`)
**Goal**: Make Helpers forward packets for other peers.

#### 2.2.1 Setup Relay Module
- [ ] Create file: `crates/core-mesh/src/relay.rs`
- [ ] Add module declaration in `crates/core-mesh/src/lib.rs`
- [ ] Import libp2p relay dependencies in `Cargo.toml`

#### 2.2.2 Define Packet Structure
- [ ] Create struct: `Packet { src: PeerId, dst: PeerId, data: Vec<u8> }`
- [ ] Implement `encode()` method to serialize packet to bytes
- [ ] Implement `decode()` method to deserialize from bytes
- [ ] Add unit test for encode/decode round-trip

#### 2.2.3 Implement Relay Behavior
- [ ] Create struct: `RelayBehavior { peer_id: PeerId, routes: HashMap<PeerId, PeerId> }`
- [ ] Implement method: `fn should_relay(&self, packet: &Packet) -> bool`
- [ ] Implement method: `fn forward_packet(&mut self, packet: Packet) -> Result<()>`
- [ ] Add logging for relayed packets

#### 2.2.4 Implement Routing Table
- [ ] Create struct: `RoutingTable { routes: HashMap<PeerId, Vec<PeerId>> }`
- [ ] Implement method: `fn add_route(&mut self, dst: PeerId, via: PeerId)`
- [ ] Implement method: `fn find_route(&self, dst: PeerId) -> Option<PeerId>`
- [ ] Implement method: `fn remove_route(&mut self, dst: PeerId)`

#### 2.2.5 Integrate Relay with Discovery
- [ ] Update `DiscoveryBehavior` to populate `RoutingTable`
- [ ] When peer discovered, add route to routing table
- [ ] When peer lost, remove route from routing table
- [ ] Add method: `pub fn get_routing_table(&self) -> &RoutingTable`

#### 2.2.6 Implement Packet Handler
- [ ] Create function: `async fn handle_incoming_packet(packet: Packet, relay: &mut RelayBehavior)`
- [ ] If `packet.dst == self.peer_id`, deliver to local handler
- [ ] Else, call `relay.forward_packet(packet)`
- [ ] Add error handling for failed relays

#### 2.2.7 Integration with Helper
- [ ] Add `RelayBehavior` to Helper's network stack
- [ ] Connect relayed packets to SOCKS5 handler (if dst is this peer)
- [ ] Connect outgoing SOCKS5 traffic to relay (if dst is remote peer)
- [ ] Add relay statistics to Status API (packets_relayed count)

#### 2.2.8 Testing
- [ ] Create file: `tests/integration/mesh_relay.rs`
- [ ] Test: Node A sends to Node C via Node B (3-node relay)
- [ ] Test: Verify packet arrives with correct data
- [ ] Test: Verify relay statistics are updated
- [ ] Run test: `cargo test --test mesh_relay`

#### 2.2.9 Documentation
- [ ] Add doc comment to `relay.rs` module
- [ ] Document relay packet format
- [ ] Update `qnet-spec/docs/ARCHITECTURE.md` with relay flow diagram
- [ ] Add example to `examples/mesh_relay.rs`

---

### 2.3 Circuit Building (`core-mesh`)
**Goal**: Construct multi-hop paths for privacy.

#### 2.3.1 Setup Circuit Module
- [ ] Create file: `crates/core-mesh/src/circuit.rs`
- [ ] Add module declaration in `crates/core-mesh/src/lib.rs`
- [ ] Define circuit constants (MAX_HOPS = 3)

#### 2.3.2 Define Circuit Structure
- [ ] Create struct: `Circuit { id: u64, hops: Vec<PeerId>, created_at: Instant }`
- [ ] Implement method: `fn new(hops: Vec<PeerId>) -> Self`
- [ ] Implement method: `fn next_hop(&self, current: &PeerId) -> Option<PeerId>`
- [ ] Add unit test for circuit creation

#### 2.3.3 Implement Circuit Builder
- [ ] Create struct: `CircuitBuilder { discovery: Arc<DiscoveryBehavior> }`
- [ ] Implement method: `async fn build_circuit(&self, dst: PeerId, num_hops: usize) -> Result<Circuit>`
- [ ] Select random intermediate peers from discovered peers
- [ ] Ensure no peer appears twice in the circuit
- [ ] Return constructed circuit

#### 2.3.4 Integrate with Routing
- [ ] Update `RoutingTable` to store circuits
- [ ] Add method: `fn add_circuit(&mut self, circuit: Circuit)`
- [ ] Modify `find_route()` to use circuits when available
- [ ] Add method: `fn get_circuit(&self, id: u64) -> Option<&Circuit>`

#### 2.3.5 Implement Circuit Handshake
- [ ] Define handshake message: `CircuitRequest { circuit_id, next_hop }`
- [ ] Send handshake to first hop
- [ ] Each hop forwards to next hop
- [ ] Last hop sends `CircuitReady` back to client
- [ ] Add timeout for circuit establishment (10 seconds)

#### 2.3.6 Implement Circuit Teardown
- [ ] Define teardown message: `CircuitClose { circuit_id }`
- [ ] Send teardown when circuit no longer needed
- [ ] Each hop removes circuit from local table
- [ ] Add automatic teardown after 5 minutes of inactivity

#### 2.3.7 Integration with Helper
- [ ] Add `CircuitBuilder` to Helper
- [ ] When SOCKS5 request arrives, build circuit to destination
- [ ] Route traffic through the circuit (not direct)
- [ ] Add circuit info to Status API (active_circuits count)

#### 2.3.8 Testing
- [ ] Create file: `tests/integration/mesh_circuit.rs`
- [ ] Test: Build 1-hop circuit, verify traffic flows
- [ ] Test: Build 3-hop circuit, verify traffic flows
- [ ] Test: Verify circuit teardown works
- [ ] Run test: `cargo test --test mesh_circuit`

#### 2.3.9 Documentation
- [ ] Add doc comment to `circuit.rs` module
- [ ] Document circuit handshake protocol
- [ ] Update `qnet-spec/docs/ARCHITECTURE.md` with circuit flow
- [ ] Add example to `examples/mesh_circuit.rs`

---

### 2.4 Helper Service Integration
**Goal**: Connect the mesh to the SOCKS5 proxy.

#### 2.4.1 Refactor Helper Startup
- [ ] Open file: `apps/stealth-browser/src/main.rs`
- [ ] Add mesh initialization: `let mesh = MeshNetwork::new(peer_id).await?`
- [ ] Start discovery: `mesh.start_discovery().await?`
- [ ] Log mesh status: `info!("Mesh started, peer_id: {}", peer_id)`

#### 2.4.2 Connect SOCKS5 to Mesh
- [ ] Open file: `apps/stealth-browser/src/socks5.rs`
- [ ] Modify `handle_connect()` to check if destination is a peer
- [ ] If destination is a QNet peer, route via mesh
- [ ] If destination is regular internet, use HTX (existing logic)

#### 2.4.3 Add Mesh Status Endpoint
- [ ] Open file: `apps/stealth-browser/src/api.rs`
- [ ] Add field to `StatusResponse`: `mesh_peer_count: usize`
- [ ] Add field to `StatusResponse`: `active_circuits: usize`
- [ ] Populate fields from mesh state

#### 2.4.4 Add Configuration
- [ ] Create file: `apps/stealth-browser/config.toml`
- [ ] Add section: `[mesh]`
- [ ] Add config: `enabled = true`
- [ ] Add config: `max_circuits = 10`
- [ ] Load config on startup

#### 2.4.5 Testing
- [ ] Update smoke test: `scripts/test-masked-connect.ps1`
- [ ] Add check: Verify mesh_peer_count > 0 after startup
- [ ] Add check: Verify circuit works for peer destination
- [ ] Run test: `pwsh scripts/test-masked-connect.ps1`

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
- Phase 2: P2P Mesh Network → **0% Complete** 🚧
- Phase 3: Browser Extension → **0% Complete** 🚧
- Phase 4: Advanced Privacy → **0% Complete** 🔮

**Next Task**: Start Phase 2.1.1 (Create discovery.rs file)
