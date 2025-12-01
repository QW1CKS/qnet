# QNet Research Requirements

> **Purpose**: This document consolidates ALL areas requiring in-depth research before implementation. Each section contains a comprehensive super-prompt for a research agent.
>
> **How to Use**: Copy the super-prompt for each topic and provide it to a research AI. Wait for findings before implementing.
>
> **AI Guardrail Compliance**: Per `memory/ai-guardrail.md` Section 7, these are consolidated super-prompts (not fragmented requests).

---

## 🔴 PRIORITY 1: Critical Research (Blocks Current Work)

---

### 1. libp2p Custom Stream Protocol Implementation

**Current Status**: The mesh streaming in `apps/stealth-browser/src/main.rs` lines 1390-1420 is a **PLACEHOLDER**:
```rust
// In a full implementation, we would:
// 1. Get a stream handle from Swarm via protocol negotiation
// 2. Use libp2p::core::upgrade to negotiate /qnet/stream/1.0.0
// 3. Copy data bidirectionally
//
// For now, simulate with a minimal placeholder that logs
```

**Impact**: Without this, mesh peer-to-peer data transfer doesn't work. The `.qnet` address routing is non-functional.

#### Super-Prompt: libp2p Custom Stream Protocols

```markdown
# Research Request: libp2p Custom Stream Protocol Implementation

## Context
QNet is implementing a P2P mesh network using libp2p in Rust. We need to create a custom 
stream protocol (`/qnet/stream/1.0.0`) that allows bidirectional data transfer between 
peers. The current implementation is a placeholder that uses mpsc channels but doesn't 
actually send data over the network.

## What I Need to Understand

### Core Protocol Mechanics
1. How do I define a custom protocol ID (e.g., `/qnet/stream/1.0.0`) in libp2p-rust?
2. What is the relationship between `NetworkBehaviour`, `ConnectionHandler`, and streams?
3. How does protocol negotiation work (multistream-select)?
4. What is the difference between `request_response` protocol vs raw stream protocols?
5. When should I use `libp2p::request_response` vs implementing a custom `NetworkBehaviour`?

### Stream Lifecycle
6. How do I open an outbound stream to a specific peer?
7. How do I accept inbound streams from other peers?
8. How do I read and write data bidirectionally on an established stream?
9. How do I handle stream closure (graceful vs abrupt)?
10. How do I detect when the remote peer closes their end?

### Event Handling
11. What events does the Swarm emit for stream-related activity?
12. How do I match incoming streams to pending requests?
13. How do I handle multiple concurrent streams to the same peer?
14. What is the correct way to propagate errors from streams to the application?

### Integration Patterns
15. How do I bridge a libp2p stream with a Tokio TcpStream (for SOCKS5 proxying)?
16. How do I handle backpressure between the mesh stream and local connections?
17. What is the pattern for request-response with streaming (not just single messages)?
18. How do working projects (IPFS, Substrate) implement custom stream protocols?

### Cross-Runtime Concerns
19. QNet uses Tokio for SOCKS5 but libp2p uses async-std. How do I bridge streams across runtimes?
20. What is the correct pattern for mpsc channel communication between runtimes?

## Research Scope

### 1. Official Documentation
- Read: https://docs.rs/libp2p/latest/libp2p/
- Read: https://docs.rs/libp2p-swarm/latest/libp2p_swarm/
- Read: https://docs.rs/libp2p-request-response/latest/libp2p_request_response/
- Extract: Protocol definition patterns, ConnectionHandler implementation, stream handling

### 2. Working Implementations to Study
- **rust-libp2p examples**: https://github.com/libp2p/rust-libp2p/tree/master/examples
  - Focus on: `file-sharing`, `chat`, `ping` examples
- **Substrate networking**: https://github.com/paritytech/polkadot-sdk/tree/master/substrate/client/network
  - Focus on: Custom protocol implementation patterns
- **Forest (Filecoin)**: https://github.com/ChainSafe/forest
  - Focus on: How they implement block/message exchange protocols
- **iroh (IPFS rewrite)**: https://github.com/n0-computer/iroh
  - Focus on: Modern stream handling patterns

### 3. Code Examples to Find
- Minimal custom `NetworkBehaviour` with stream support
- Outbound stream opening with timeout
- Inbound stream acceptance and handling
- Bidirectional copy between libp2p stream and other async streams

## Required Output Format

Create markdown files in `/research/libp2p-streams/`:

### 1. `libp2p-streams-mechanics.md`
- Step-by-step: How streams are established in libp2p
- Protocol negotiation flow diagram
- Key types and their relationships (Swarm, Behaviour, Handler, Stream)
- Common misconceptions and pitfalls

### 2. `libp2p-streams-implementation.md`
- Code pattern: Defining a custom protocol
- Code pattern: Implementing NetworkBehaviour for streams
- Code pattern: Opening outbound streams
- Code pattern: Handling inbound streams
- Code pattern: Bidirectional data transfer
- Anti-patterns to avoid

### 3. `libp2p-streams-qnet-gaps.md`
- What QNet's placeholder is missing
- Specific API calls needed to replace the placeholder
- How to bridge with Tokio TcpStream
- Test validation approach (how to verify it works)

## Success Criteria
After reading this research, a developer should be able to:
- Replace the placeholder in `main.rs` with working stream code
- Handle both outbound (.qnet destination) and inbound (peer requests) streams
- Bridge libp2p streams to SOCKS5 client connections
- Write integration tests that verify stream data transfer
```

---

### 2. Onion Routing / Circuit Cryptography

**Current Status**: `crates/core-mesh/src/circuit.rs` has message structures but **NO actual onion encryption**:
```rust
// Traffic is onion-routed through each hop, with each
// peer only able to decrypt one layer to reveal the next hop.
```

**Impact**: Multi-hop circuits don't provide privacy. Each relay can see the full path.

#### Super-Prompt: Onion Routing Cryptography

```markdown
# Research Request: Onion Routing Cryptography for Multi-Hop Circuits

## Context
QNet implements multi-hop circuits for privacy (similar to Tor). The circuit module 
(`crates/core-mesh/src/circuit.rs`) has message structures (CircuitRequest, CircuitReady, 
CircuitClose) but lacks the actual cryptographic onion layering. Currently, relays could 
see the entire path, defeating the privacy purpose.

## What I Need to Understand

### Onion Encryption Fundamentals
1. What is the exact packet format for onion-encrypted data?
2. How are encryption keys derived for each hop?
3. What is the difference between header encryption and payload encryption?
4. How does the initiator construct a packet that each hop can partially decrypt?
5. How does each relay know it's the final hop vs an intermediate hop?

### Key Derivation & Establishment
6. How are per-hop symmetric keys established?
7. What key derivation function (KDF) is used (HKDF, SHAKE)?
8. Is there a Diffie-Hellman exchange with each hop, or is it derived from a shared secret?
9. How is forward secrecy maintained at each hop?
10. What is the "circuit extension" pattern (Tor's CREATE/EXTEND)?

### Sphinx Packet Format
11. What is the Sphinx packet format and why is it used?
12. How does Sphinx achieve bitwise unlinkability?
13. What are the header, payload, and MAC components?
14. How is the routing information encrypted in the header?
15. What is SURB (Single-Use Reply Block) and when is it needed?

### Replay Protection
16. How do relays detect and reject replayed packets?
17. What state do relays need to maintain for replay detection?
18. How long should replay detection state be retained?
19. What is the trade-off between replay protection and memory usage?

### Implementation Patterns
20. How does Tor implement onion encryption (reference: tor-spec)?
21. How does Nym implement Sphinx packets (reference: nym-sphinx)?
22. What cryptographic primitives are required (AES-CTR, ChaCha20, HMAC)?
23. How do you handle variable-length payloads in fixed-size packets?

### Error Handling & Resilience
24. What happens if a relay in the middle fails?
25. How do you detect circuit failure vs network partition?
26. How do you securely report errors back to the initiator?

## Research Scope

### 1. Official Specifications
- Read: Tor Protocol Specification (https://spec.torproject.org/tor-spec)
  - Focus on: CREATE, EXTEND, RELAY cell formats
- Read: Sphinx Paper (https://cypherpunks.ca/~iang/pubs/Sphinx_Oakland09.pdf)
  - Focus on: Header processing, SURB construction
- Read: Nym Sphinx Implementation (https://github.com/nymtech/nym/tree/develop/common/nymsphinx)
  - Focus on: Practical Rust implementation

### 2. Working Implementations to Study
- **Tor (C)**: https://gitlab.torproject.org/tpo/core/tor
  - Focus on: `src/core/or/relay_crypto.c`, cell processing
- **Arti (Rust Tor)**: https://gitlab.torproject.org/tpo/core/arti
  - Focus on: `crates/tor-cell`, `crates/tor-proto` - modern Rust patterns
- **Nym Sphinx (Rust)**: https://github.com/nymtech/nym/tree/develop/common/nymsphinx
  - Focus on: Packet construction, header processing, payload encryption
- **Lightning Network Onion (Rust)**: https://github.com/lightningdevkit/rust-lightning
  - Focus on: `lightning/src/onion_message` - simpler onion for messages

### 3. Cryptographic Primitives Needed
- ChaCha20-Poly1305 or AES-GCM for symmetric encryption
- X25519 for per-hop DH key exchange
- HKDF-SHA256 for key derivation
- HMAC-SHA256 for integrity tags

## Required Output Format

Create markdown files in `/research/onion-routing/`:

### 1. `onion-routing-mechanics.md`
- Step-by-step: How a packet traverses a 3-hop circuit
- Diagram: Packet encryption layers (like peeling an onion)
- Key derivation chain from initiator to each hop
- How replies travel back (reverse path encryption)

### 2. `onion-routing-implementation.md`
- Code pattern: Packet construction (initiator side)
- Code pattern: Layer decryption (relay side)
- Code pattern: Final hop detection and payload extraction
- Code pattern: Reply packet construction
- Recommended packet sizes and padding strategy

### 3. `onion-routing-qnet-gaps.md`
- What `circuit.rs` is missing (specific functions to add)
- Key derivation integration with existing `core-crypto`
- How to integrate with existing CircuitRequest/CircuitReady messages
- Test vectors for validation

## Success Criteria
After reading this research, a developer should be able to:
- Implement `encrypt_for_circuit(data, circuit) -> OnionPacket`
- Implement `decrypt_layer(packet, my_keys) -> (next_hop, remaining_packet)`
- Implement `decrypt_final(packet, my_keys) -> plaintext`
- Write tests that verify each relay only sees its adjacent hops
```

---

## 🔴 PRIORITY 2: Needed for Phase 2 Completion

---

### 3. NAT Traversal with libp2p (AutoNAT, Circuit Relay, Hole Punching)

**Current Status**: libp2p is used but NAT traversal mechanics are not fully understood or configured.

**Impact**: Users behind NAT cannot participate as relay nodes or receive inbound connections.

#### Super-Prompt: NAT Traversal with libp2p

```markdown
# Research Request: NAT Traversal with libp2p (AutoNAT, Circuit Relay, Hole Punching)

## Context
QNet uses libp2p for P2P networking. Most users are behind NAT (home routers, corporate 
firewalls). For the mesh to work, nodes need to:
1. Detect their NAT status
2. Advertise reachable addresses
3. Fall back to relayed connections when direct fails
4. Attempt hole punching for better performance

## What I Need to Understand

### AutoNAT (NAT Detection)
1. What is AutoNAT and how does it determine NAT status?
2. What are the possible NAT statuses (Public, Private, Unknown)?
3. How do I configure AutoNAT in libp2p-rust?
4. How often does AutoNAT re-check reachability?
5. What events does AutoNAT emit (NatStatusChanged)?
6. How do I handle the case where AutoNAT can't determine status?

### Circuit Relay v2
7. What is the difference between Circuit Relay v1 and v2?
8. How do I enable Circuit Relay in libp2p-rust?
9. When should a node act as a relay server vs relay client?
10. How are relayed addresses constructed and advertised?
11. What is the reservation system in Circuit Relay v2?
12. How do I limit relay resource usage (bandwidth, connections)?
13. How do I discover available relay nodes?

### Hole Punching (DCUtR)
14. What is DCUtR (Direct Connection Upgrade through Relay)?
15. How does hole punching work with libp2p?
16. What NAT types support hole punching (cone vs symmetric)?
17. What is the success rate for hole punching in practice?
18. How do I enable and configure DCUtR in libp2p-rust?
19. What events indicate hole punching success/failure?

### Address Handling
20. How do I get my external (public) address from AutoNAT results?
21. How do I advertise both direct and relayed addresses?
22. How do I prioritize direct connections over relayed ones?
23. How do I handle address changes (dynamic IP, network switch)?

### Integration with Identify Protocol
24. How does the Identify protocol relate to NAT traversal?
25. What information does Identify exchange that helps with connectivity?
26. How do I configure Identify alongside AutoNAT and Relay?

## Research Scope

### 1. Official Documentation
- Read: https://docs.rs/libp2p-autonat/latest/libp2p_autonat/
- Read: https://docs.rs/libp2p-relay/latest/libp2p_relay/
- Read: https://docs.rs/libp2p-dcutr/latest/libp2p_dcutr/
- Read: Circuit Relay v2 spec: https://github.com/libp2p/specs/blob/master/relay/circuit-v2.md
- Read: DCUtR spec: https://github.com/libp2p/specs/blob/master/relay/DCUtR.md

### 2. Working Implementations
- **rust-libp2p examples**: https://github.com/libp2p/rust-libp2p/tree/master/examples
  - Focus on: `dcutr` example, `relay-server` example
- **IPFS Kubo**: How IPFS handles NAT traversal
- **iroh**: https://github.com/n0-computer/iroh - Modern approach to connectivity

### 3. Real-World Considerations
- NAT type distribution in the wild (% cone vs symmetric)
- Typical hole punching success rates
- Relay bandwidth/connection limits best practices
- Mobile network NAT challenges

## Required Output Format

Create markdown files in `/research/nat-traversal/`:

### 1. `nat-traversal-mechanics.md`
- NAT types and their implications for P2P
- AutoNAT detection flow diagram
- Circuit Relay v2 reservation and connection flow
- Hole punching (DCUtR) sequence diagram

### 2. `nat-traversal-implementation.md`
- Code pattern: Configuring AutoNAT in libp2p
- Code pattern: Setting up as relay client
- Code pattern: Setting up as relay server
- Code pattern: Enabling DCUtR for hole punching
- Code pattern: Handling NAT status change events
- Code pattern: Advertising relayed addresses

### 3. `nat-traversal-qnet-gaps.md`
- Current libp2p configuration in QNet
- What's missing for NAT-friendly operation
- Recommended configuration for different node roles (client, relay, super)
- Test scenarios for NAT traversal validation

## Success Criteria
After reading this research, a developer should be able to:
- Configure libp2p so nodes behind NAT can participate
- Set up super peers as relay servers
- Enable hole punching for direct connections when possible
- Handle all NAT-related events appropriately
```

---

### 4. HTX Noise XK Handshake Verification

**Current Status**: `crates/htx/src/inner.rs` claims Noise XK but the full handshake state machine needs audit.

**Impact**: If handshake is incorrect, security properties (authentication, forward secrecy) may be compromised.

#### Super-Prompt: Noise XK Protocol Verification

```markdown
# Research Request: Noise XK Protocol Verification & Implementation Audit

## Context
QNet's HTX protocol uses a Noise XK handshake to establish secure channels inside TLS 
connections. The implementation is in `crates/htx/src/inner.rs`. We need to verify the 
handshake is correctly implemented per the Noise Protocol Framework specification.

## What I Need to Understand

### Noise Protocol Fundamentals
1. What is the Noise Protocol Framework?
2. What does "XK" mean in Noise pattern naming (X=static, K=known)?
3. What is the exact message pattern for Noise XK?
4. What security properties does XK provide vs XX, IK, KK?
5. Why was XK chosen for HTX (vs other patterns)?

### Noise XK Message Flow
6. What is sent in the first message (initiator -> responder)?
7. What is sent in the second message (responder -> initiator)?
8. What DH operations happen at each step?
9. How is the transcript hash computed?
10. When are the transport keys derived?

### Key Management
11. How are ephemeral keypairs generated?
12. How are static keypairs managed (storage, rotation)?
13. What is the key schedule (CipherState, SymmetricState)?
14. How does HKDF fit into key derivation?
15. What happens if rekey is needed mid-session?

### Channel Binding (HTX-specific)
16. How does HTX bind the Noise keys to the outer TLS session?
17. What is the TLS exporter and how is it used?
18. What is the "exporter context" and why does it include TemplateID?
19. How does this binding prevent MITM attacks?

### Implementation Verification
20. How do I verify the implementation against test vectors?
21. Where can I find Noise XK test vectors?
22. What are common implementation mistakes in Noise?
23. How do I test forward secrecy properties?

## Research Scope

### 1. Official Specifications
- Read: Noise Protocol Framework: https://noiseprotocol.org/noise.html
  - Focus on: Section 7 (Handshake Patterns), Section 9 (XK pattern)
- Read: Noise Explorer: https://noiseexplorer.com/patterns/XK/
  - Focus on: Security analysis, message flow visualization

### 2. Reference Implementations
- **snow (Rust)**: https://github.com/mcginty/snow
  - Focus on: How they implement XK, test vectors
- **noise-protocol (Rust)**: https://docs.rs/noise-protocol/
  - Focus on: State machine implementation
- **WireGuard**: Uses Noise IK, but similar patterns

### 3. Security Analysis
- Noise Protocol security proofs
- Common implementation vulnerabilities
- Timing attack considerations

## Required Output Format

Create markdown files in `/research/noise-xk/`:

### 1. `noise-xk-mechanics.md`
- Complete message flow for XK pattern with all DH operations
- Transcript hash computation at each step
- Key derivation schedule diagram
- Security properties proven by XK

### 2. `noise-xk-implementation-audit.md`
- Checklist for verifying Noise XK implementation
- Test vectors for validation
- Common implementation mistakes to check for
- How to verify forward secrecy

### 3. `noise-xk-qnet-gaps.md`
- Audit of current `inner.rs` implementation
- Any deviations from spec (justified or not)
- Missing tests or validations
- Recommendations for hardening

## Success Criteria
After reading this research, a developer should be able to:
- Verify the HTX handshake follows Noise XK correctly
- Add test vectors that validate the implementation
- Identify any security issues in the current code
- Understand the channel binding with TLS exporter
```

---

## 🟡 PRIORITY 3: Phase 4 Preparation (Future Work)

---

### 5. Traffic Shaping & ML-Based Fingerprinting Resistance

**Current Status**: No traffic shaping implementation exists. ML classifiers can fingerprint traffic patterns.

**Impact**: Without traffic shaping, ML classifiers can detect QNet traffic despite TLS fingerprint resistance.

#### Super-Prompt: Traffic Shaping & Anti-Fingerprinting

```markdown
# Research Request: Traffic Shaping & ML-Based Fingerprinting Resistance

## Context
QNet aims to make traffic indistinguishable from normal HTTPS. While TLS fingerprinting 
(JA3/JA4) is addressed, traffic analysis attacks remain. An adversary with ML can 
potentially identify QNet traffic by timing, packet sizes, and flow patterns.

## What I Need to Understand

### Traffic Analysis Attacks
1. What is website fingerprinting (WF)?
2. How do ML classifiers identify encrypted traffic?
3. What features do classifiers use (timing, size, direction, bursts)?
4. What accuracy do state-of-the-art WF attacks achieve against Tor?
5. What is traffic correlation and how does it deanonymize users?

### Defense Techniques
6. What is constant-rate padding and how effective is it?
7. What is adaptive padding (WTF-PAD, Walkie-Talkie)?
8. What is traffic morphing (making one site look like another)?
9. What is NetShaper and how does it use differential privacy?
10. What is the latency/bandwidth cost of these defenses?

### Implementation Considerations
11. How do you implement padding without destroying latency?
12. How do you generate realistic dummy traffic patterns?
13. How do you handle bidirectional traffic (upload vs download)?
14. What is the right granularity for padding (packet, burst, flow)?
15. How do you tune parameters for different threat models?

### Evaluation Methods
16. How do you measure fingerprinting resistance?
17. What datasets exist for testing (Tor traces)?
18. What ML models should defenses be tested against?
19. What is an acceptable accuracy reduction (>90% → <70%)?

### Traffic Obfuscation (QNet-specific)
20. How do you generate realistic HTTPS-like traffic patterns?
21. How do you vary timing to avoid statistical fingerprinting?
22. What entropy sources are needed for convincing randomization?
23. How do you handle interactive content (WebSockets, streaming)?

## Research Scope

### 1. Academic Papers
- Read: "Website Fingerprinting Defenses at the Application Layer" (WTF-PAD)
- Read: "NetShaper: A Differentially Private Network Side-Channel Mitigation System"
- Read: "Deep Fingerprinting" (state-of-art attack)
- Read: "Walkie-Talkie" traffic shaping defense

### 2. Implementations
- **Tor's circuit padding framework**: https://gitlab.torproject.org/tpo/core/tor
  - Focus on: `src/core/or/circuitpadding.c`
- **WTF-PAD implementations**: Academic repos
- **Traffic morphing tools**: Research prototypes

### 3. Datasets & Tools
- Tor website fingerprinting datasets
- Deep learning models for traffic analysis
- Timing analysis tools

## Required Output Format

Create markdown files in `/research/traffic-shaping/`:

### 1. `traffic-shaping-mechanics.md`
- Taxonomy of traffic analysis attacks
- Taxonomy of defense techniques
- Trade-offs: privacy vs performance vs complexity

### 2. `traffic-shaping-implementation.md`
- Code pattern: Constant-rate padding
- Code pattern: Adaptive padding (WTF-PAD style)
- Code pattern: Traffic obfuscation for ML resistance
- Integration points in QNet architecture

### 3. `traffic-shaping-qnet-gaps.md`
- What QNet needs to implement
- Recommended defense strategy for QNet's threat model
- Test methodology for validation
- Performance budget considerations

## Success Criteria
After reading this research, a developer should be able to:
- Implement a basic padding defense
- Understand the trade-offs of different approaches
- Design a test to measure fingerprinting resistance
- Choose appropriate parameters for QNet's use case
```

---

### 6. QUIC Transport & Encrypted Client Hello (ECH)

**Current Status**: QNet uses TCP only. QUIC and ECH are not implemented.

**Impact**: Missing modern transport options and SNI encryption.

#### Super-Prompt: QUIC & ECH Implementation

```markdown
# Research Request: QUIC Transport & Encrypted Client Hello (ECH)

## Context
QNet currently uses TCP with TLS 1.3. Modern censorship systems can see the SNI in 
the TLS handshake. QUIC + ECH would provide:
1. Better performance (0-RTT, multiplexing)
2. SNI encryption (ECH hides destination hostname)
3. Harder to block (looks like generic UDP)

## What I Need to Understand

### QUIC Fundamentals
1. What is QUIC and how does it differ from TCP+TLS?
2. What are QUIC's multiplexing capabilities?
3. How does QUIC handle connection migration?
4. What is 0-RTT and what are its security implications?
5. How does QUIC interact with middleboxes (firewalls, NAT)?

### QUIC in Rust
6. What Rust crates implement QUIC (quinn, quiche, s2n-quic)?
7. How do I establish a QUIC connection with quinn?
8. How do I create bidirectional streams in QUIC?
9. How do I handle connection errors and retries?
10. What configuration options are available?

### Encrypted Client Hello (ECH)
11. What is ECH and how does it hide SNI?
12. How does ECH work with DNS (HTTPS records)?
13. What is the ECH "outer" vs "inner" ClientHello?
14. How do I implement ECH in Rust (rustls support)?
15. What happens if ECH fails (fallback behavior)?

### Integration with QNet
16. How would QUIC replace TCP in HTX?
17. Can QUIC and TCP coexist (transport selection)?
18. How does QUIC affect TLS fingerprinting (different fingerprints)?
19. How does ECH interact with HTX's traffic obfuscation?
20. What are the performance differences (latency, throughput)?

### Censorship Resistance
21. How do censors detect QUIC traffic?
22. What fraction of internet traffic is QUIC (plausible deniability)?
23. Are there QUIC blocking techniques in the wild?
24. How does ECH compare to ESNI (predecessor)?

## Research Scope

### 1. Official Specifications
- Read: RFC 9000 (QUIC)
- Read: RFC 9001 (QUIC-TLS)
- Read: ECH draft specification
- Read: HTTPS DNS records (for ECH config distribution)

### 2. Rust Implementations
- **quinn**: https://github.com/quinn-rs/quinn
  - Focus on: Client/server setup, stream handling
- **rustls ECH**: https://github.com/rustls/rustls
  - Focus on: ECH support status and API
- **quiche**: https://github.com/cloudflare/quiche
  - Focus on: Alternative implementation, comparison

### 3. Real-World Deployment
- Cloudflare's QUIC/ECH deployment
- Browser support status (Chrome, Firefox)
- CDN support for ECH

## Required Output Format

Create markdown files in `/research/quic-ech/`:

### 1. `quic-ech-mechanics.md`
- QUIC connection establishment flow
- ECH ClientHello encryption process
- Comparison: TCP+TLS vs QUIC vs QUIC+ECH

### 2. `quic-ech-implementation.md`
- Code pattern: Basic QUIC client with quinn
- Code pattern: QUIC server setup
- Code pattern: Bidirectional streams
- Code pattern: ECH configuration (when available)
- Integration with existing codebase

### 3. `quic-ech-qnet-gaps.md`
- What changes to HTX for QUIC support
- Transport selection strategy (TCP, QUIC, both)
- ECH configuration distribution (how to get ECH keys)
- Test plan for QUIC deployment

## Success Criteria
After reading this research, a developer should be able to:
- Add QUIC as an alternative transport in HTX
- Understand when to use QUIC vs TCP
- Plan for ECH integration when rustls support matures
- Evaluate censorship resistance of QUIC
```

---

### 7. Obfs4 Pluggable Transport

**Current Status**: Not implemented. Task 4.1.5 placeholder.

**Impact**: No support for active probing resistance.

#### Super-Prompt: Obfs4 Pluggable Transport

```markdown
# Research Request: Obfs4 Pluggable Transport Implementation

## Context
Obfs4 is a pluggable transport designed to resist active probing by censors. It's used 
by Tor to bypass the Great Firewall of China and similar censorship systems. QNet should 
support obfs4 as a transport option for high-censorship environments.

## What I Need to Understand

### Obfs4 Protocol
1. What is obfs4 and how does it differ from obfs3?
2. What is the obfs4 handshake protocol?
3. How does obfs4 use X25519 and HKDF?
4. What is the IAT (inter-arrival time) mode?
5. How does obfs4 resist active probing?

### Cryptographic Details
6. What is the "node ID" and "public key" in obfs4?
7. How is the server's public key distributed (bridge line)?
8. What is the "server-timing" defense?
9. How is the shared secret derived?
10. What cipher is used for data encryption?

### Probe Resistance
11. What is active probing and how do censors use it?
12. How does obfs4's handshake resist probing?
13. What is the "HMAC-based authentication"?
14. How does obfs4 behave when probed (timing, response)?
15. What are the known weaknesses of obfs4?

### Implementation
16. What is lyrebird (the obfs4 implementation)?
17. How does the pluggable transport (PT) API work?
18. Can obfs4 be implemented in pure Rust?
19. What is the SOCKS-based PT interface?
20. How do I integrate obfs4 with an existing connection?

### QNet Integration
21. How would obfs4 wrap HTX connections?
22. Should obfs4 be inside or outside TLS?
23. How do users configure obfs4 bridges?
24. What is the performance overhead?

## Research Scope

### 1. Official Specifications
- Read: obfs4 spec: https://gitlab.torproject.org/tpo/anti-censorship/pluggable-transports/obfs4
- Read: Pluggable Transport spec v2.1
- Read: Tor's PT documentation

### 2. Implementations
- **lyrebird (Go)**: https://gitlab.torproject.org/tpo/anti-censorship/pluggable-transports/lyrebird
  - Focus on: Handshake implementation, probe resistance
- **obfs4proxy**: https://github.com/Yawning/obfs4
  - Focus on: Original implementation details
- **ptrs (Rust PT framework)**: Rust pluggable transport experiments

### 3. Censorship Research
- Academic papers on obfs4 fingerprinting
- GFW (Great Firewall) obfs4 blocking attempts
- Comparison with other PTs (meek, snowflake)

## Required Output Format

Create markdown files in `/research/obfs4/`:

### 1. `obfs4-mechanics.md`
- Obfs4 handshake flow diagram
- Cryptographic operations at each step
- IAT mode operation
- Probe resistance mechanisms

### 2. `obfs4-implementation.md`
- Code pattern: obfs4 client handshake
- Code pattern: obfs4 server handshake
- Code pattern: Data encryption/decryption
- Code pattern: IAT padding
- Integration with PT API

### 3. `obfs4-qnet-gaps.md`
- How to layer obfs4 with HTX
- Bridge distribution strategy for QNet
- Configuration options to expose
- Test methodology (including probe resistance)

## Success Criteria
After reading this research, a developer should be able to:
- Implement obfs4 handshake in Rust
- Integrate obfs4 as a transport option
- Configure IAT mode for timing obfuscation
- Test probe resistance
```

---

### 8. Shadowsocks AEAD Integration

**Current Status**: Not implemented. Task 4.1.6 placeholder.

**Impact**: Missing compatibility with popular censorship circumvention tool.

#### Super-Prompt: Shadowsocks AEAD Integration

```markdown
# Research Request: Shadowsocks AEAD Protocol Integration

## Context
Shadowsocks is widely used for censorship circumvention, especially in China. Supporting 
Shadowsocks as a transport option would allow QNet to interoperate with existing 
infrastructure and provide a familiar option for users.

## What I Need to Understand

### Shadowsocks Protocol
1. What is Shadowsocks and how does it work?
2. What is the difference between Stream and AEAD ciphers?
3. What is the current recommended cipher (ChaCha20-Poly1305-IETF)?
4. What is the wire format for AEAD mode?
5. How are encryption keys derived from password?

### AEAD Wire Format
6. What is the salt and how is it used?
7. What is the subkey derivation process?
8. What is the nonce management strategy?
9. What is the chunk format (length + tag + payload + tag)?
10. How are requests encoded (SOCKS-like address format)?

### Implementation
11. What Rust crates implement Shadowsocks (shadowsocks-rust)?
12. Should we use the crate or reimplement?
13. How does shadowsocks-rust handle async I/O?
14. What is the SIP003 plugin system?
15. How do we interoperate with existing SS servers?

### QNet Integration
16. How would Shadowsocks wrap HTX connections?
17. How does Shadowsocks compare to HTX for censorship resistance?
18. When should users choose SS vs HTX?
19. What configuration options are needed?

### Censorship Resistance
20. How do censors detect Shadowsocks?
21. What is the "replay attack" and mitigation?
22. What is "active probing" against Shadowsocks?
23. How does AEAD help vs stream ciphers?

## Research Scope

### 1. Official Specifications
- Read: Shadowsocks AEAD spec: https://shadowsocks.org/doc/aead.html
- Read: SIP003 plugin spec
- Read: Shadowsocks protocol overview

### 2. Implementations
- **shadowsocks-rust**: https://github.com/shadowsocks/shadowsocks-rust
  - Focus on: AEAD implementation, crypto handling
- **shadowsocks-libev**: C implementation (reference)
- **go-shadowsocks2**: Go implementation

### 3. Analysis
- Academic papers on Shadowsocks fingerprinting
- GFW Shadowsocks blocking research
- Comparison with other protocols

## Required Output Format

Create markdown files in `/research/shadowsocks/`:

### 1. `shadowsocks-mechanics.md`
- AEAD encryption flow
- Key derivation from password
- Chunk format diagram
- Nonce management

### 2. `shadowsocks-implementation.md`
- Code pattern: AEAD encryption/decryption
- Code pattern: Key derivation
- Code pattern: Client connection
- Vendor vs reimplement decision
- Integration approach

### 3. `shadowsocks-qnet-gaps.md`
- How to expose Shadowsocks as transport option
- Configuration (password, method)
- Interop testing with SS servers
- Performance comparison with HTX

## Success Criteria
After reading this research, a developer should be able to:
- Implement or integrate Shadowsocks AEAD
- Configure QNet to use SS servers as exits
- Understand the security trade-offs
```

---

### 9. Nym Mixnet Integration

**Current Status**: Not implemented. Task 4.2 placeholder.

**Impact**: No mixnet-level privacy (global adversary resistance).

#### Super-Prompt: Nym Mixnet Integration

```markdown
# Research Request: Nym Mixnet Integration

## Context
Nym is a mixnet that provides strong privacy against global adversaries through cover 
traffic and mixing. Integrating Nym would give QNet users an optional high-privacy mode 
for sensitive communications.

## What I Need to Understand

### Mixnet Fundamentals
1. What is a mixnet and how does it differ from onion routing?
2. What is cover traffic and why is it important?
3. What is the Sphinx packet format used by Nym?
4. What are mix nodes and how do they operate?
5. What is the latency vs privacy trade-off in mixnets?

### Nym Architecture
6. What is the Nym network topology?
7. What are gateways vs mix nodes?
8. How does a client send a message through Nym?
9. How does a client receive messages (SURBs)?
10. What is the Nym credential system?

### Nym SDK
11. What does the Nym Rust SDK provide?
12. How do I initialize a Nym client?
13. How do I send and receive messages?
14. How do I handle connection lifecycle?
15. What configuration is needed?

### Integration Considerations
16. How would QNet traffic be routed through Nym?
17. Can Nym carry TCP-like streams or only messages?
18. What is the latency penalty for using Nym?
19. How does cover traffic interact with bandwidth limits?
20. What are the cost/token requirements?

### Security Properties
21. What adversaries can Nym protect against?
22. What are the assumptions (honest mix nodes)?
23. How does Nym compare to Tor for privacy?
24. What are the known limitations?

## Research Scope

### 1. Official Documentation
- Read: Nym documentation: https://nymtech.net/docs/
- Read: Nym whitepaper
- Read: Sphinx paper (underlying crypto)

### 2. Implementations
- **Nym Rust SDK**: https://github.com/nymtech/nym
  - Focus on: `sdk/rust/nym-sdk`, client usage
- **Example applications**: Nym demo apps

### 3. Academic Context
- Loopix paper (Nym's theoretical foundation)
- Mixnet analysis papers

## Required Output Format

Create markdown files in `/research/nym-mixnet/`:

### 1. `nym-mixnet-mechanics.md`
- Mixnet operation flow
- Sphinx packet construction
- Cover traffic generation
- SURB (reply) mechanism

### 2. `nym-mixnet-implementation.md`
- Code pattern: Nym client initialization
- Code pattern: Sending messages
- Code pattern: Receiving messages
- SDK API overview

### 3. `nym-mixnet-qnet-gaps.md`
- Integration architecture options
- Message vs stream traffic handling
- User configuration (enable/disable mixnet)
- Performance implications

## Success Criteria
After reading this research, a developer should be able to:
- Integrate Nym SDK into QNet
- Route traffic through Nym when requested
- Understand the privacy/performance trade-offs
```

---

### 10. Native Messaging (Browser Extension ↔ Helper)

**Current Status**: Not implemented. Phase 3.3 placeholder.

**Impact**: Extension cannot communicate with Helper for control/status.

#### Super-Prompt: Native Messaging Implementation

```markdown
# Research Request: Chrome/Firefox Native Messaging Protocol

## Context
QNet's browser extension needs to communicate with the local Helper binary. Native 
Messaging is the standard mechanism for extensions to talk to native applications.
This enables the extension to start/stop the proxy, get status, etc.

## What I Need to Understand

### Native Messaging Protocol
1. What is Native Messaging and how does it work?
2. What is the message format (length-prefixed JSON)?
3. How does the extension initiate communication?
4. What are the security restrictions?
5. What is the difference between Chrome and Firefox NM?

### Host Application
6. What is a Native Messaging host?
7. What is the host manifest format?
8. Where is the manifest registered (Windows registry, Linux path)?
9. How does the host receive messages (stdin)?
10. How does the host send messages (stdout)?

### Chrome Implementation
11. How do I use `chrome.runtime.sendNativeMessage()`?
12. How do I use `chrome.runtime.connectNative()` for persistent connections?
13. What permissions are needed in manifest.json?
14. How do I handle errors and disconnections?

### Firefox Implementation
15. How does Firefox Native Messaging differ from Chrome?
16. What is the `applications.gecko.id` requirement?
17. Where is the Firefox host manifest stored?
18. Are there WebExtension polyfills for cross-browser support?

### Rust Host Implementation
19. How do I read length-prefixed messages from stdin in Rust?
20. How do I write length-prefixed messages to stdout in Rust?
21. How do I handle the message loop (blocking vs async)?
22. How do I integrate with the existing Helper architecture?

### Security Considerations
23. What origins can connect to the host?
24. How do I validate messages from the extension?
25. What are the risks of Native Messaging?
26. How do I handle concurrent extension instances?

## Research Scope

### 1. Official Documentation
- Read: Chrome Native Messaging: https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging
- Read: Firefox Native Messaging: https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Native_messaging
- Read: WebExtensions API documentation

### 2. Implementations
- **browser-extension-native-client**: Various GitHub examples
- **pyo3 native messaging**: Python examples (for pattern)
- Existing Rust native messaging hosts

### 3. Cross-Browser
- WebExtension polyfill patterns
- Browser detection and feature handling

## Required Output Format

Create markdown files in `/research/native-messaging/`:

### 1. `native-messaging-mechanics.md`
- Protocol flow diagram (extension ↔ host)
- Message framing format (4-byte length + JSON)
- Registration process per OS/browser

### 2. `native-messaging-implementation.md`
- Code pattern: Rust stdin/stdout message loop
- Code pattern: Extension sendNativeMessage
- Code pattern: Persistent connection handling
- Host manifest examples (Windows, Linux, macOS)
- Installer integration (registry, file placement)

### 3. `native-messaging-qnet-gaps.md`
- Message protocol design for QNet (GET_STATUS, TOGGLE_PROXY, etc.)
- Integration with existing Helper status API
- Cross-browser considerations
- Error handling strategy

## Success Criteria
After reading this research, a developer should be able to:
- Implement Native Messaging host in Rust
- Create extension that communicates with Helper
- Handle registration on all platforms
- Design message protocol for QNet's needs
```

---

## 🟢 PRIORITY 4: Future Phases (Reference Only)

---

### 11. Self-Certifying IDs / Alias Ledger

**Placeholder for Task 4.3**: Petname system, 2-of-3 finality, DNS replacement.

### 12. Voucher/Cashu Payment System  

**Placeholder for Task 4.4**: Ecash tokens, blind signatures, relay incentives.

### 13. Governance & Voting

**Placeholder for Task 4.5**: Node reputation, protocol upgrades.

### 14. Refraction Networking Partnership

**Placeholder for Task 4.6**: ISP cooperation, tagging protocols.

---

## 📋 Research Tracking

### 🔴 Priority 1: Critical (Blocks Current Work)
- [x] **libp2p Custom Stream Protocol** — `/research/findings/libp2p Custom Stream Protocol Implementation/`
- [x] **Onion Routing Circuit Cryptography** — `/research/findings/Onion Routing Circuit Cryptography/`

### 🔴 Priority 2: Needed for Phase 2 Completion
- [x] **NAT Traversal (AutoNAT, Circuit Relay, Hole Punching)** — `/research/findings/NAT Traversal with libp2p (AutoNAT, Circuit Relay, Hole Punching)/`
- [x] **HTX Noise XK Handshake Verification** — `/research/findings/HTX Noise XK Handshake Verification/`

### 🟡 Priority 3: Phase 4 Preparation
- [x] **Traffic Shaping & ML Fingerprinting Resistance** — `/research/findings/Traffic Shaping & ML-Based Fingerprinting Resistance/`
- [x] **QUIC Transport & ECH** — `/research/findings/QUIC Transport & Encrypted Client Hello (ECH)/`
- [x] **Obfs4 Pluggable Transport** — `/research/findings/Obfs4 Pluggable Transport/`
- [x] **Shadowsocks AEAD Integration** — `/research/findings/Shadowsocks AEAD Integration/`
- [x] **Nym Mixnet Integration** — `/research/findings/Nym Mixnet Integration/`
- [x] **Native Messaging (Extension ↔ Helper)** — `/research/findings/Native Messaging (Browser Extension ↔ Helper)/`

### 🟢 Priority 4: Future Phases (Not Yet Researched)
- [ ] **Self-Certifying IDs / Alias Ledger** — Petname system, 2-of-3 finality, DNS replacement
- [ ] **Voucher/Cashu Payment System** — Ecash tokens, blind signatures, relay incentives
- [ ] **Governance & Voting** — Node reputation, protocol upgrades
- [ ] **Refraction Networking Partnership** — ISP cooperation, tagging protocols

---

## Usage Instructions

1. **Before implementing any feature** in the topics above, copy the relevant super-prompt
2. **Provide to a research AI** (Claude, GPT-4, etc.) with web search capability
3. **Wait for findings** to be documented in `/research/<topic>/` folder
4. **Review findings** before proceeding with implementation
5. **Update this document** with research status and findings location

---

*Last Updated: November 30, 2025*
*AI Guardrail Compliance: Section 7 (Research-First Mandate)*
