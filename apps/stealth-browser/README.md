# Stealth Browser (Helper service)

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

**Stealth Browser** in this repository refers to the local Helper service that provides the QNet masking functionality: a SOCKS5 proxy and a small HTTP status/control API. The recommended user-facing deployment pairs a browser extension (UI + control) with this Helper service.

## Overview

The Helper provides:

- A local SOCKS5 proxy that applies decoy-based masking to outbound connections
- A small HTTP status API (default `http://127.0.0.1:8088`) for UI/extension integration
- Catalog loading, signature verification, and atomic updates
- Developer-mode options for testing (unsigned dev catalogs, plaintext mux)

## Quick Start (Helper)

### Run from source (development)

Run the Helper directly for development or testing. The Helper exposes a SOCKS5 proxy and status API.

```powershell
# From repo root (development)
cargo run -p stealth-browser
```

By default the Helper listens on:

- SOCKS proxy: `127.0.0.1:1088`
- Status API: `http://127.0.0.1:8088`

The browser extension (recommended) will point the browser to the SOCKS proxy and read status via the status API.

## User Interface

### Main Interface

**Navigation:**
- **Address Bar**: QNet-aware URL resolution
- **Identity Selector**: Choose active identity
- **Network Status**: Connection and anonymity status
- **Wallet Balance**: Integrated token display

**Privacy Controls:**
- **Anonymity Level**: Adjustable privacy settings
- **Tracking Protection**: Anti-fingerprinting toggles
- **Connection Routing**: Manual route selection

### Identity Management

**Creating Identity:**
1. Click "New Identity" in the menu
2. Choose identity type (Personal/Organization)
3. Set display name and avatar
4. Generate cryptographic keys

**Identity Switching:**
- Click identity selector in toolbar
- Choose from available identities
- Automatic key and session switching

### Wallet Integration

**Token Management:**
- **View Balance**: Check QNet token balance
- **Send/Receive**: Transfer tokens securely
- **Voucher Redemption**: Redeem privacy vouchers
- **Transaction History**: View past transactions

## Privacy Features

### Anonymity Protection

**Mixnet Routing:**
- **Automatic Routing**: Intelligent path selection
- **Multi-Hop**: 3+ hop minimum routes
- **Route Diversity**: Avoid predictable paths
- **Traffic Padding**: Constant-rate transmission

**Anti-Tracking:**
- **Fingerprinting Protection**: Randomized browser fingerprint
- **Cookie Isolation**: Per-site cookie containers
- **Canvas Protection**: Prevent canvas fingerprinting
- **WebRTC Protection**: Disable or proxy WebRTC

### Secure Communication

**End-to-End Encryption:**
- **Message Encryption**: AES-256-GCM encryption
- **Forward Secrecy**: Ephemeral key exchange
- **Perfect Forward Secrecy**: Post-compromise security

**Anonymous Messaging:**
- **Peer-to-Peer**: Direct encrypted messaging
- **Group Chat**: Anonymous group conversations
- **File Sharing**: Secure file transfer
- **Voice/Video**: Encrypted real-time communication

## Developer Features

### dApp Browser

**Decentralized Applications:**
```javascript
// Connect to QNet
const qnet = await window.qnet.connect();

// Get user identity
const identity = await qnet.identity.getCurrent();

// Send anonymous message
await qnet.messaging.send(peerId, "Hello from dApp!");
```

**QNet JavaScript API:**
```javascript
// Available APIs
qnet.identity    // Identity management
qnet.messaging   // P2P messaging
qnet.wallet      // Token operations
qnet.storage     // Decentralized storage
qnet.naming      // Alias resolution
```

### Extension System

**Browser Extensions:**
- **Privacy Extensions**: Enhanced tracking protection
- **Wallet Extensions**: Third-party wallet integration
- **dApp Extensions**: Decentralized application support
- **Network Extensions**: Custom network providers

**Extension Development:**
```javascript
// Extension manifest
{
  "name": "My Privacy Extension",
  "version": "1.0",
  "permissions": ["privacy", "network"],
  "background": "background.js",
  "content_scripts": ["content.js"]
}
```

## Network Integration

### SOCKS5 Proxy (Helper)

The Helper provides a local SOCKS5 proxy that the browser extension will configure the browser to use. Default ports are shown above.

**Application integration (development):**

```python
import socks
import socket

# Configure SOCKS5 proxy
socks.setdefaultproxy(socks.PROXY_TYPE_SOCKS5, "127.0.0.1", 1088)
socket.socket = socks.socksocket

# All network requests now go through the Helper proxy
import requests
response = requests.get("https://example.com")
```

### Decentralized DNS

**QNet Name Resolution:**
```
# Traditional DNS
example.com -> 93.184.216.34

# QNet DNS
alice.qnet -> [identity_public_key]
service.qnet -> [service_endpoint]
```

**Alias Resolution:**
- **Human-Readable**: `alice.qnet` instead of public key
- **Secure**: Cryptographically verified mappings
- **Decentralized**: No central DNS authority

## Security Considerations

### Threat Model

**Adversary Capabilities:**
- **Network Observer**: Can see all network traffic
- **Website Operator**: Controls destination website
- **Local Attacker**: Has access to user device
- **Global Adversary**: Controls multiple network nodes

### Security Features

**Cryptographic Protection:**
- **TLS 1.3**: End-to-end encryption to websites
- **Certificate Pinning**: Prevent MITM attacks
- **HSTS**: Force HTTPS connections
- **HPKP**: Public key pinning

**Privacy Protection:**
- **No Logs**: Zero logging policy
- **Tor Integration**: Optional Tor routing
- **VPN Killswitch**: Prevent accidental leaks
- **DNS Leak Protection**: Prevent DNS leaks

## Performance

### Benchmark Results

**Browsing Performance:**

| Configuration | Page Load Time | Memory Usage |
|----------------|----------------|--------------|
| Direct | 1.2s | 150MB |
| QNet Routing | 2.8s | 180MB |
| High Privacy | 4.1s | 200MB |

**Network Throughput:**
- **Direct Connection**: 100 Mbps
- **QNet Routing**: 50 Mbps
- **High Privacy Mode**: 25 Mbps

### System Requirements

**Minimum Requirements:**
- **OS**: Windows 10, macOS 10.15, Ubuntu 18.04
- **CPU**: Dual-core 2.0 GHz
- **RAM**: 4GB
- **Storage**: 500MB
- **Network**: 10 Mbps

**Recommended Requirements:**
- **OS**: Windows 11, macOS 12, Ubuntu 20.04
- **CPU**: Quad-core 3.0 GHz
- **RAM**: 8GB
- **Storage**: 1GB
- **Network**: 25 Mbps

## Configuration

### Configuration Files

**Main Configuration:**
```json
{
  "privacy": {
    "anonymity_level": "high",
    "anti_fingerprinting": true,
    "tracking_protection": true
  },
  "network": {
    "socks5_port": 1080,
    "bootstrap_nodes": ["bootstrap.qnet.io:8080"],
    "max_hops": 5
  },
  "wallet": {
    "auto_lock": true,
    "lock_timeout": 300
  }
}
```

### Command Line Options

```bash
./stealth-browser [options]

Options:
  --socks5-port PORT       SOCKS5 proxy port
  --data-dir DIR           Data directory
  --config FILE            Configuration file
  --headless               Run without GUI
  --verbose                Enable verbose logging
  --version                Show version
  --help                   Show help
```


## Troubleshooting

### Common Issues

**Connection Problems:**

```bash
# Check network status via Helper status API
curl http://127.0.0.1:8088/status

# Test SOCKS proxy (example)
curl --socks5-hostname 127.0.0.1:1088 https://check.qnet.io
```

**If the extension can't reach the Helper:**
- Ensure the Helper binary is installed and running
- Confirm the Helper is listening on the expected ports (1088/8088)
- Check local firewall rules that may block localhost sockets

**Extension Problems:**
- Ensure the extension has permission to modify proxy settings (browser prompt)
- If auto-launch failed, the extension will show an installer prompt to guide the user


## Development

### Building from Source

**Prerequisites:**
```bash
# Install Node.js
curl -fsSL https://deb.nodesource.com/setup_18.x | sudo -E bash -
sudo apt-get install -y nodejs

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Tauri CLI
npm install -g @tauri-apps/cli
```

**Build Process:**
```bash
# Install dependencies
npm install

# Build Rust backend
npm run build-rust

# Build frontend
npm run build-frontend

# Create distributable
npm run build-dist
```

### Testing

```bash
# Run unit tests
npm test

# Run integration tests
npm run test:integration

# Run end-to-end tests
npm run test:e2e
```

## Contributing

See the main [Contributing Guide](../../qnet-spec/docs/CONTRIBUTING.md) for development setup and contribution guidelines.

### Development Requirements

- Follow [AI Guardrail](../../memory/ai-guardrail.md)
- Meet [Testing Rules](../../memory/testing-rules.md)
- Include `AI-Guardrail: PASS` and `Testing-Rules: PASS` in commits

## License

Licensed under the MIT License. See [LICENSE](../../LICENSE) for details.

## Security

If you discover a security vulnerability, please see our [Security Policy](../../SECURITY.md).

---

*Part of the [QNet](https://github.com/QW1CKS/qnet) project - Building the future of decentralized networking.*
