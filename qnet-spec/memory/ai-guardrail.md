# AI Agent Guardrail & Project Context

## 🎯 Project Mission
**QNet is a decentralized overlay network designed to provide censorship resistance by disguising all traffic as legitimate HTTPS connections to popular domains.**

**The Big Idea**: Make it impossible for ISPs and censors to block the network without breaking the entire internet. We do this by mimicking traffic to sites like Microsoft, Google, and Cloudflare.

## 🏗️ Architecture Philosophy

### The "Helper Node" Model
**Core Concept**: Every user's Helper Service is simultaneously:
1. **A Client**: Accepts SOCKS5 from the user's browser
2. **A Peer**: Routes traffic through the P2P mesh
3. **An Exit**: Can fetch content for other peers

**Why This Matters**: This is NOT a VPN. There's no central server. Every user strengthens the network for everyone else.

### The 7-Layer Stack
```
L7 (Application)  → Browser Extension (JS)
L6 (Incentive)    → Payments (Future: Vouchers/Cashu)
L5 (Naming)       → DNS Replacement (Future: Petnames)
L4 (Privacy)      → Mixnet (Optional: Nym integration)
L3 (Mesh)         → P2P Routing (libp2p)
L2 (Transport)    → HTX (THIS IS THE MAGIC - TLS Fingerprint Cloning)
L1 (Path)         → SCION / IP routing
L0 (Physical)     → TCP / UDP / QUIC
```

**The Critical Layer: L2 (HTX)**
- **Goal**: Make QNet traffic indistinguishable from normal HTTPS to a decoy site (e.g., microsoft.com).
- **How**:
  1. **TLS Fingerprint Cloning**: Mimic the exact ClientHello of the decoy (JA3, ALPN, Extension Order).
  2. **Inner Handshake**: Establish a Noise XK secure channel INSIDE the TLS stream.
  3. **Traffic Shaping**: Add padding and timing jitter to match the decoy's behavior.
- **Result**: ISP sees `HTTPS -> microsoft.com`, Reality is `Encrypted QNet traffic`.

## 🧠 Design Decisions (Why Things Are The Way They Are)

### 1. Why Rust?
- **Memory Safety**: No buffer overflows or use-after-free in the core networking stack.
- **Performance**: Zero-cost abstractions, native performance.
- **Ecosystem**: Tokio (async), libp2p (P2P), rustls (TLS).

### 2. Why Browser Extension + Helper (Instead of Full Desktop App)?
- **User Friction**: Installing a browser extension is easier than installing system-level software.
- **Security**: The extension doesn't need elevated privileges; the Helper does the heavy lifting.
- **Cross-Platform**: WebExtensions work on Chrome/Edge/Firefox; the Helper is just a binary.

### 3. Why P2P (Instead of Centralized Servers)?
- **Censorship Resistance**: No single server to block or seize.
- **Scalability**: The network grows as users join.
- **Trust**: No need to trust a single operator.

### 4. Why Signed Catalogs (Instead of Hardcoded Config)?
- **Updates**: We can push new decoy nodes without shipping new binaries.
- **Security**: Ed25519 signatures prevent tampering.
- **Resilience**: Multiple mirrors (GitHub, CDN) for redundancy.

## 📝 Coding Standards

### Language-Specific Idioms

#### Rust
- **Use `Result<T, E>` for errors**, not panics in library code.
- **Prefer `&str` over `String` in function arguments** (borrow when possible).
- **Use `#[derive(Debug)]` liberally** for debuggability.
- **Avoid `.unwrap()` in production code** - use `?` or handle explicitly.
- **Name things clearly**: `establish_htx_connection`, not `do_conn`.

#### JavaScript (Browser Extension)
- **Use `const` by default**, `let` when reassignment needed.
- **Prefer async/await** over raw Promises.
- **Use WebExtensions APIs** (not browser-specific hacks).
- **Error handling**: Always `.catch()` promises or use try/catch with async.

### Project-Specific Patterns

#### 1. Catalog-First Design
**Rule**: All configuration (decoys, seeds, update URLs) comes from the signed catalog.
```rust
// ✅ GOOD: Load from catalog
let decoys = CatalogLoader::load()?.decoys;

// ❌ BAD: Hardcoded fallback without catalog check
let decoys = vec!["microsoft.com", "google.com"];
```

#### 2. Helper Service API
**Rule**: The Helper exposes two interfaces:
- **SOCKS5** (`127.0.0.1:1088`) - for browser traffic
- **Status API** (`http://127.0.0.1:8088/status`) - for extension control

```rust
// ✅ GOOD: Use standard ports from the spec
let socks_addr = "127.0.0.1:1088";
let status_addr = "127.0.0.1:8088";

// ❌ BAD: Random ports or environment-only config
let socks_addr = env::var("SOCKS_PORT").unwrap();
```

#### 3. TLS Fingerprint Mirroring
**Rule**: ALWAYS clone the decoy's fingerprint exactly.
```rust
// ✅ GOOD: Calibrate before connecting
let template = calibrate_tls("microsoft.com").await?;
let client_config = build_client_hello(&template)?;

// ❌ BAD: Generic TLS config
let client_config = ClientConfig::default();
```

#### 4. Mesh Routing
**Rule**: The Helper MUST be able to forward traffic for other peers.
```rust
// ✅ GOOD: Implement relay logic
fn handle_incoming_packet(packet: Packet) {
    if packet.destination == self.peer_id {
        // This is for us
    } else {
        // Relay to the mesh
        self.mesh.forward(packet);
    }
}

// ❌ BAD: Client-only logic
fn handle_incoming_packet(packet: Packet) {
    // Only handle packets for us, ignore relay requests
}
```

## 🚨 Common Mistakes to Avoid

### 1. Don't Break Indistinguishability
```rust
// ❌ BAD: Adding a "QNet" header
let headers = [("User-Agent", "QNet/1.0")];

// ✅ GOOD: Look like a normal browser
let headers = [("User-Agent", "Mozilla/5.0...")];
```

### 2. Don't Assume Direct Connections
```rust
// ❌ BAD: Connecting directly to the destination
TcpStream::connect("amazon.com:443").await?

// ✅ GOOD: Route through the mesh
mesh.route_to_destination("amazon.com:443").await?
```

### 3. Don't Hardcode Crypto Keys
```rust
// ❌ BAD: Hardcoded public key
let pubkey = [0x12, 0x34, ...];

// ✅ GOOD: Load from catalog or environment
let pubkey_hex = env::var("QNET_CATALOG_PUBKEY")?;
let pubkey = hex::decode(pubkey_hex)?;
```

### 4. Don't Skip Signature Verification
```rust
// ❌ BAD: Trusting unsigned data
let catalog = serde_json::from_str(&response)?;

// ✅ GOOD: Verify signature first
let catalog = CatalogLoader::verify_and_load(&response, &pubkey)?;
```

## ✅ Pre-Change Checklist (MANDATORY)

Before committing code, verify:

### 1. **Architecture Alignment**
- [ ] Does this change align with the "Helper Node" model?
- [ ] Does it maintain the P2P nature of the network?
- [ ] Does it preserve traffic indistinguishability?

### 2. **Security**
- [ ] No hardcoded secrets or keys?
- [ ] All user input validated?
- [ ] Cryptographic operations use established primitives (no DIY crypto)?

### 3. **Idioms & Quality**
- [ ] Uses language-specific idioms (Rust's `Result`, JS's `const`)?
- [ ] Follows project patterns (catalog-first, standard ports)?
- [ ] No "textbook" code - reads like real production code?

### 4. **Testing**
- [ ] Unit tests for new functions?
- [ ] Integration test if it spans multiple components?
- [ ] Edge cases covered (empty input, timeout, bad data)?

### 5. **Documentation**
- [ ] Public APIs have doc comments?
- [ ] Complex logic has explanatory comments?
- [ ] README updated if user-facing change?

### 6. **Commit Message**
```
type(scope): description

AI-Guardrail: PASS
Testing-Rules: PASS
```

## 🎓 When In Doubt

### Ask These Questions:
1. **"Would this survive censorship?"** - If an ISP could detect it, redesign.
2. **"Is this decentralized?"** - If it requires a central server, rethink.
3. **"Is this simple?"** - If it's overly complex, refactor.
4. **"Is this tested?"** - If it's not, write the test first.

### Resources:
- **Protocol Spec**: `qnet-spec/specs/001-qnet/spec.md`
- **Task List**: `qnet-spec/specs/001-qnet/tasks.md`
- **Roadmap**: `qnet-spec/specs/001-qnet/plan.md`
- **Templates**: `qnet-spec/templates/`

---

**Remember**: QNet is about **freedom**. Every line of code should respect that mission.
