# Testing Rules & Philosophy

## 🎯 Testing Philosophy

**Rule #1: Tests are documentation.**
Your tests should explain HOW the system works and WHY certain behaviors exist.

**Rule #2: Tests catch regressions.**
If a bug was fixed, there should be a test that would fail if the bug comes back.

**Rule #3: Tests enable refactoring.**
Good tests allow you to change the implementation without changing the tests (they test behavior, not internals).

## 📋 Required Tests by Component

### 🔐 Cryptography (`core-crypto`)
**Why this matters**: Any bug here is a security vulnerability.

**Required Tests**:
- [ ] **Happy Path**: Encrypt -> Decrypt -> Get original plaintext
- [ ] **Tamper Detection**: Modify ciphertext by 1 bit -> Decryption fails
- [ ] **IV Reuse**: Same nonce twice -> Should panic or error
- [ ] **Boundary Cases**: Empty message, 1-byte message, 16MiB message

**Example**:
```rust
#[test]
fn aead_detects_tampered_ciphertext() {
    let key = [0u8; 32];
    let nonce = [0u8; 12];
    let plaintext = b"secret message";
    
    let mut ciphertext = seal(&key, &nonce, &[], plaintext).unwrap();
    
    // Tamper: flip one bit
    ciphertext[5] ^= 0x01;
    
    // Should fail to decrypt
    assert!(open(&key, &nonce, &[], &ciphertext).is_err());
}
```

### 🚀 HTX Transport (`htx`)
**Why this matters**: This is the core of our censorship resistance.

**Required Tests**:
- [ ] **TLS Fingerprint Matching**: Generated ClientHello matches the decoy's
- [ ] **Inner Handshake**: Noise XK completes correctly
- [ ] **Traffic Shaping**: Padding and timing are applied
- [ ] **Catalog Integration**: Decoy selection works with signed catalogs

**Example**:
```rust
#[tokio::test]
async fn htx_clones_decoy_tls_fingerprint() {
    let decoy = "microsoft.com";
    
    // Calibrate against real Microsoft
    let expected = calibrate_tls(decoy).await.unwrap();
    
    // Generate our ClientHello
    let generated = build_client_hello(&expected).unwrap();
    
    // Should match exactly
    assert_eq!(generated.ja3_hash(), expected.ja3_hash());
    assert_eq!(generated.alpn_list(), expected.alpn_list());
}
```

### 🕸️ Mesh Routing (`core-mesh`)
**Why this matters**: This is how we avoid centralization.

**Required Tests**:
- [ ] **Peer Discovery**: Can find other nodes via DHT
- [ ] **Relay Logic**: Can forward packets for other peers
- [ ] **Circuit Building**: Can construct multi-hop paths
- [ ] **Failure Handling**: Handles peer disconnections gracefully

**Example**:
```rust
#[tokio::test]
async fn mesh_forwards_packets_for_other_peers() {
    let node_a = TestNode::new("A").await;
    let node_b = TestNode::new("B").await;
    let node_c = TestNode::new("C").await;
    
    // A wants to reach C, but routes through B
    let packet = Packet::new(node_a.id(), node_c.id(), b"hello");
    
    node_a.send(packet.clone()).await.unwrap();
    
    // B should receive and forward it
    assert!(node_b.received_and_forwarded(&packet).await);
    
    // C should receive the original packet
    let received = node_c.receive().await.unwrap();
    assert_eq!(received.data, b"hello");
}
```

### 🌐 Browser Extension (`apps/extension`)
**Why this matters**: This is the user-facing component.

**Required Tests**:
- [ ] **Proxy Toggle**: Clicking "Connect" enables proxy, "Disconnect" disables it
- [ ] **Native Messaging**: Extension can communicate with the Helper
- [ ] **Status Display**: Shows correct connection state

**Example** (using WebExtensions test framework):
```javascript
test('toggleConnection enables SOCKS proxy', async () => {
  const extension = await loadExtension();
  
  // Initially disconnected
  expect(await getProxySettings()).toBeNull();
  
  // Click "Connect"
  await extension.click('#connect-button');
  
  // Proxy should be enabled
  const proxy = await getProxySettings();
  expect(proxy.type).toBe('socks');
  expect(proxy.host).toBe('127.0.0.1');
  expect(proxy.port).toBe(1088);
});
```

### 🔧 Helper Service (`apps/stealth-browser`)
**Why this matters**: This is the core runtime binary.

**Required Tests**:
- [ ] **SOCKS5 Server**: Accepts connections on `127.0.0.1:1088`
- [ ] **Status API**: Returns JSON at `http://127.0.0.1:8088/status`
- [ ] **Catalog Loading**: Loads and verifies signed catalogs
- [ ] **Mesh Integration**: Joins the P2P network on startup

**Example**:
```rust
#[tokio::test]
async fn helper_serves_status_api() {
    let helper = HelperService::start_test().await.unwrap();
    
    // Query the status endpoint
    let response = reqwest::get("http://127.0.0.1:8088/status")
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
    
    let status: StatusResponse = response.json().await.unwrap();
    assert_eq!(status.proxy_state, "listening");
    assert!(status.decoy_count > 0);
}
```

## 🧪 Test Categories

### 1. Unit Tests
**Purpose**: Test individual functions in isolation.
**Location**: Same file as the code (`#[cfg(test)] mod tests { ... }`)
**Run**: `cargo test --lib`

**Good Example**:
```rust
#[test]
fn parse_socks5_request_success() {
    let bytes = [0x05, 0x01, 0x00, 0x01, ...];
    let req = parse_socks5_request(&bytes).unwrap();
    assert_eq!(req.command, Command::Connect);
    assert_eq!(req.address, "example.com:80");
}
```

**Bad Example** (testing internal implementation):
```rust
#[test]
fn parser_sets_state_to_reading_address() {
    // DON'T test internal state machine details
    // Test the public behavior instead
}
```

### 2. Integration Tests
**Purpose**: Test how components work together.
**Location**: `tests/` directory
**Run**: `cargo test --test integration`

**Good Example**:
```rust
#[tokio::test]
async fn end_to_end_socks5_to_htx() {
    // Start a Helper Service
    let helper = spawn_helper().await;
    
    // Connect via SOCKS5
    let mut stream = connect_socks5("127.0.0.1:1088", "example.com:80").await.unwrap();
    
    // Send HTTP request
    stream.write_all(b"GET / HTTP/1.1\r\n\r\n").await.unwrap();
    
    // Should get a response
    let response = read_response(&mut stream).await.unwrap();
    assert!(response.starts_with(b"HTTP/1.1 200"));
}
```

### 3. Property-Based Tests (Fuzzing)
**Purpose**: Find edge cases automatically.
**Tool**: `cargo-fuzz`
**Location**: `fuzz/fuzz_targets/`

**Example**:
```rust
fuzz_target!(|data: &[u8]| {
    // Should never panic, even on garbage input
    let _ = parse_htx_frame(data);
});
```

### 4. Performance Tests
**Purpose**: Ensure we meet performance targets.
**Tool**: Criterion
**Location**: `benches/`

**Example**:
```rust
fn bench_aead_throughput(c: &mut Criterion) {
    let key = [0u8; 32];
    let nonce = [0u8; 12];
    let plaintext = vec![0u8; 16*1024]; // 16 KiB
    
    c.bench_function("aead_encrypt_16kb", |b| {
        b.iter(|| seal(&key, &nonce, &[], &plaintext))
    });
}
```

## 📏 Coverage Targets

### Critical Paths (≥80% Coverage)
- `core-crypto`: All encryption/decryption/signing
- `core-framing`: Packet parsing and serialization
- `htx`: Handshake and key derivation

### Important Paths (≥60% Coverage)
- `core-mesh`: Peer discovery and routing
- `stealth-browser`: SOCKS5 handling

### Nice-to-Have (≥40% Coverage)
- `catalog-signer`: CLI tool
- Integration tests

**Check coverage**:
```bash
cargo tarpaulin --out Html --exclude-files fuzz/ tests/
```

## ⚠️ What NOT to Test

### 1. Third-Party Libraries
```rust
// ❌ DON'T: Test that rustls does TLS correctly
#[test]
fn test_rustls_handshake() {
    // rustls has its own tests
}
```

### 2. Language Features
```rust
// ❌ DON'T: Test that Vec::push works
#[test]
fn test_vec_push() {
    let mut v = vec![];
    v.push(1);
    assert_eq!(v.len(), 1); // Duh.
}
```

### 3. Trivial Getters/Setters
```rust
// ❌ DON'T: Test simple accessors
#[test]
fn test_get_name() {
    let user = User { name: "Alice" };
    assert_eq!(user.name(), "Alice"); // Obvious.
}
```

## 🚨 Common Testing Mistakes

### Mistake #1: Flaky Tests
```rust
// ❌ BAD: Relies on timing
#[tokio::test]
async fn test_async_operation() {
    start_operation();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(operation_finished()); // Might fail if slow!
}

// ✅ GOOD: Use synchronization
#[tokio::test]
async fn test_async_operation() {
    let (tx, rx) = oneshot::channel();
    start_operation(tx);
    rx.await.unwrap(); // Waits until actually done
    assert!(operation_finished());
}
```

### Mistake #2: Testing Implementation, Not Behavior
```rust
// ❌ BAD: Tests internal structure
#[test]
fn test_parser_internal_state() {
    let parser = Parser::new();
    parser.parse(b"...");
    assert_eq!(parser.state, ParserState::ReadingHeader); // Fragile!
}

// ✅ GOOD: Tests public behavior
#[test]
fn test_parser_extracts_header() {
    let parser = Parser::new();
    let result = parser.parse(b"...").unwrap();
    assert_eq!(result.header, expected_header);
}
```

### Mistake #3: Overly Specific Assertions
```rust
// ❌ BAD: Too specific
#[test]
fn test_error_message() {
    let err = do_something().unwrap_err();
    assert_eq!(err.to_string(), "Network timeout occurred at 14:23:05 UTC");
}

// ✅ GOOD: Test the important part
#[test]
fn test_error_type() {
    let err = do_something().unwrap_err();
    assert_eq!(err.kind(), ErrorKind::NetworkTimeout);
}
```

## ✅ Pre-Merge Checklist

Before opening a PR:
- [ ] All tests pass: `cargo test --workspace`
- [ ] No warnings: `cargo clippy --workspace`
- [ ] Code formatted: `cargo fmt --all`
- [ ] New code has tests (unit + integration if multi-component)
- [ ] Edge cases covered (empty, large, error conditions)
- [ ] Fuzz target added/updated if parser changed
- [ ] Performance benchmark added if claiming speed improvement
- [ ] Commit message includes `Testing-Rules: PASS`

## 🎓 When In Doubt

### Ask:
1. **"If I break this, will a test fail?"** - If no, write a test.
2. **"Is this test fragile?"** - If yes, make it more robust.
3. **"Does this test document the behavior?"** - If no, improve it.

### Resources:
- **Rust Book Testing Chapter**: https://doc.rust-lang.org/book/ch11-00-testing.html
- **Property-Based Testing**: https://github.com/rust-fuzz/cargo-fuzz
- **Criterion Benchmarking**: https://github.com/bheisler/criterion.rs

---

**Remember**: Good tests are an investment. They catch bugs early and make refactoring safe.
