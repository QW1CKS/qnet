# QNet Home Super Peer Testing Guide

This guide provides step-by-step instructions for running a QNet super peer on a home laptop/desktop behind CGNAT.

---

## Tunneling Strategy

Different QNet services require different tunnel types:

| Port | Service | Protocol | Tunnel Tool | Why |
|------|---------|----------|-------------|-----|
| 8088 | Status API | HTTP | InstaTunnel | HTTP-only tool, perfect for web APIs |
| 1088 | SOCKS5 Proxy | Raw TCP | Bore | Raw TCP tunneling required |
| 4001 | libp2p Mesh | Raw TCP | Bore | Raw TCP tunneling required |

**Why two tools?**
- **InstaTunnel** only supports HTTP/HTTPS - it cannot forward raw TCP protocols like SOCKS5 or libp2p
- **Bore** is a minimal Rust-based TCP tunnel that forwards arbitrary byte streams without protocol inspection

---

## Prerequisites

### Hardware Requirements
- Windows 10/11 laptop or desktop
- Minimum 8GB RAM (16GB recommended)
- 10GB free disk space
- Internet connection (WiFi or Ethernet)

### Software Requirements
- Rust toolchain (`rustup` with stable channel)
- Node.js and npm (for InstaTunnel)
- PowerShell 7+ or Windows Terminal
- Git for Windows
- QNet source code cloned locally

---

## Part 1: Verify CGNAT Status

### 1.1 Check Your Network Setup

**Step 1: Get your router's WAN IP**
1. Log into your router admin panel (usually `192.168.1.1` or `192.168.0.1`)
2. Find the WAN/Internet status page
3. Note the "WAN IP" address

**Step 2: Get your actual public IP**
```powershell
(Invoke-WebRequest -Uri "https://ifconfig.me/ip" -UseBasicParsing).Content.Trim()
```

**Step 3: Compare the IPs**
- ✅ **IPs match** → You have a real public IP (port forwarding may work)
- ❌ **IPs don't match** → You're behind CGNAT (tunneling required)

**Common CGNAT indicators:**
- Router WAN IP starts with `100.64.x.x` to `100.127.x.x` (CGNAT range)
- Router WAN IP is private (`10.x.x.x`, `172.16-31.x.x`, `192.168.x.x`)

---

## Part 2: Install Tunneling Tools

### 2.1 Install Bore (for raw TCP)

**Option A: Cargo (recommended)**
```powershell
cargo install bore-cli
```

**Option B: Prebuilt binary**
1. Download from: https://github.com/ekzhang/bore/releases
2. Extract `bore.exe` to a folder in your PATH (e.g., `C:\Tools\bore\`)
3. Add to PATH if needed

**Verify installation:**
```powershell
bore --version
```

### 2.2 Install InstaTunnel (for HTTP)

```powershell
npm install -g instatunnel
instatunnel --version
```

### 2.3 Configure Credentials (DO NOT commit to repo!)

**For InstaTunnel API key:**
```powershell
# Create config file (this is in your HOME, not the repo)
@"
api_key: "YOUR_API_KEY_HERE"
"@ | Out-File -FilePath "$HOME\.instatunnel.yaml" -Encoding utf8
```

**For Bore secret (if self-hosting):**
```powershell
# Set as environment variable (not in repo)
$env:BORE_SECRET = "YOUR_SECRET_HERE"
```

> ⚠️ **NEVER commit API keys, secrets, or passwords to the repository!**

---

## Part 3: Build QNet Super Peer

### 3.1 Build the Binary

```powershell
cd P:\GITHUB\qnet

# Build release version
cargo build --release -p stealth-browser
```

### 3.2 Generate Persistent Keypair

```powershell
# Create data directory
New-Item -ItemType Directory -Path "P:\GITHUB\qnet\data" -Force

# Generate keypair
cargo run -p stealth-browser -- --generate-keypair P:\GITHUB\qnet\data\keypair.pb
```

Note the peer ID from the output (e.g., `12D3KooWABC123...`).

---

## Part 4: Start QNet Super Peer

### 4.1 Launch Super Peer

```powershell
cd P:\GITHUB\qnet

$env:RUST_LOG = "info"
$env:QNET_KEYPAIR_PATH = "P:\GITHUB\qnet\data\keypair.pb"

cargo run --release -p stealth-browser -- --helper-mode super
```

**Expected output:**
```
[INFO] stealth-browser starting
[INFO] config loaded port=1088 status_port=8088 mode=Direct helper_mode=Super
[INFO] Loaded persistent keypair peer_id=12D3KooW...
[INFO] status server listening status_addr=0.0.0.0:8088
[INFO] starting SOCKS5 server addr=0.0.0.0:1088 mode=Direct
[INFO] mesh: Listening on /ip4/0.0.0.0/tcp/4001
```

**Keep this terminal running!**

### 4.2 Verify Local Access

```powershell
Invoke-RestMethod http://127.0.0.1:8088/status | ConvertTo-Json
Invoke-RestMethod http://127.0.0.1:8088/ping
```

---

## Part 5: Create Tunnels

### 5.1 Status API Tunnel (InstaTunnel - HTTP)

Open **Terminal 2**:
```powershell
# Basic tunnel
instatunnel 8088 -s qnet-status

# With password protection (recommended)
instatunnel 8088 -s qnet-status --password "YOUR_PASSWORD"
```

**Output:**
```
✅ Tunnel created: https://qnet-status.instatunnel.my
```

**Record this URL:** `https://qnet-status.instatunnel.my`

### 5.2 SOCKS5 Proxy Tunnel (Bore - TCP)

Open **Terminal 3**:
```powershell
# Using public bore.pub server (no setup required)
bore local 1088 --to bore.pub

# Or with self-hosted server + authentication
bore local 1088 --to your-vps.example.com --secret $env:BORE_SECRET
```

**Output:**
```
2024-01-15T10:30:00.000Z INFO bore::client > listening at bore.pub:43210
```

**Record this address:** `bore.pub:43210` (port will vary each time)

### 5.3 libp2p Mesh Tunnel (Bore - TCP)

Open **Terminal 4**:
```powershell
# Using public bore.pub server
bore local 4001 --to bore.pub

# Or with self-hosted server
bore local 4001 --to your-vps.example.com --secret $env:BORE_SECRET
```

**Output:**
```
2024-01-15T10:31:00.000Z INFO bore::client > listening at bore.pub:43211
```

**Record this address:** `bore.pub:43211` (port will vary each time)

### 5.4 Summary of Your Tunnels

| Service | Local Port | Public Address |
|---------|------------|----------------|
| Status API | 8088 | `https://qnet-status.instatunnel.my` |
| SOCKS5 | 1088 | `bore.pub:<PORT>` (from output) |
| libp2p | 4001 | `bore.pub:<PORT>` (from output) |

---

## Part 6: Update Hardcoded Operator Nodes

For clients to discover your home super peer, update the bootstrap configuration.

### 6.1 Edit discovery.rs

Open `crates/core-mesh/src/discovery.rs` and find `hardcoded_operator_nodes()`:

```rust
pub fn hardcoded_operator_nodes() -> Vec<OperatorNode> {
    vec![
        OperatorNode {
            peer_id: "YOUR_PEER_ID_HERE".to_string(),
            multiaddr: "/ip4/BORE_SERVER_IP/tcp/BORE_PORT".to_string(),
        },
    ]
}
```

**Replace:**
- `YOUR_PEER_ID_HERE` → Your peer ID from keypair generation
- `BORE_SERVER_IP` → IP of bore server (e.g., `bore.pub` resolves to an IP)
- `BORE_PORT` → The port from your libp2p bore tunnel output

**Example with bore.pub:**
```rust
multiaddr: "/dns4/bore.pub/tcp/43211".to_string(),
```

### 6.2 Rebuild After Changes

```powershell
cargo build --release -p stealth-browser
```

---

## Part 7: Test External Access

### 7.1 Test Status API

From your phone (on mobile data) or any external device:

```bash
# If password protected
curl https://qnet-status.instatunnel.my/ping

# Expected: {"ok":true,"ts":1234567890}
```

### 7.2 Test SOCKS5 Proxy

```bash
# Replace with your actual bore port
curl --socks5-hostname bore.pub:43210 https://httpbin.org/ip

# Expected: {"origin": "YOUR_IP"}
```

### 7.3 Test libp2p Connection

Run a client on another machine pointing to your bore tunnel address.

---

## Part 8: Self-Hosting Bore Server (Optional)

For stable, predictable ports, run your own bore server on a VPS.

### 8.1 On Your VPS

```bash
# Install bore
cargo install bore-cli

# Run server with authentication and restricted port range
bore server \
  --min-port 40000 \
  --max-port 41000 \
  --secret "YOUR_BORE_SECRET"
```

**Firewall rules needed:**
- TCP 7835 (bore control port)
- TCP 40000-41000 (tunnel ports)

### 8.2 On Your Home Machine

```powershell
$env:BORE_SECRET = "YOUR_BORE_SECRET"

# Fixed ports for predictable addresses
bore local 1088 --to your-vps.example.com --port 40001 --secret $env:BORE_SECRET
bore local 4001 --to your-vps.example.com --port 40002 --secret $env:BORE_SECRET
```

Now your addresses are always:
- SOCKS5: `your-vps.example.com:40001`
- libp2p: `your-vps.example.com:40002`

---

## Part 9: Convenience Scripts

### 9.1 Start All Tunnels

Create `scripts/start-tunnels.ps1`:

```powershell
# QNet Tunnel Startup Script
# NOTE: Set your secrets as environment variables before running!
# $env:BORE_SECRET = "..." (if using self-hosted bore)

param(
    [string]$BoreServer = "bore.pub",
    [string]$StatusSubdomain = "qnet-status"
)

Write-Host "Starting QNet tunnels..." -ForegroundColor Cyan

# Start InstaTunnel for Status API (HTTP)
Start-Process pwsh -ArgumentList "-NoExit", "-Command", "instatunnel 8088 -s $StatusSubdomain"

# Start Bore for SOCKS5 (TCP)
Start-Process pwsh -ArgumentList "-NoExit", "-Command", "bore local 1088 --to $BoreServer"

# Start Bore for libp2p (TCP)
Start-Process pwsh -ArgumentList "-NoExit", "-Command", "bore local 4001 --to $BoreServer"

Write-Host "Tunnels starting in separate windows..." -ForegroundColor Green
Write-Host "Check each window for the assigned public addresses!" -ForegroundColor Yellow
```

### 9.2 Full Startup Script

Create `scripts/start-home-superpeer.ps1`:

```powershell
# QNet Home Super Peer Full Startup
param(
    [string]$BoreServer = "bore.pub"
)

$ErrorActionPreference = "Stop"

Write-Host @"
╔═══════════════════════════════════════════════════════════════╗
║           QNet Home Super Peer                                ║
║           Bore (TCP) + InstaTunnel (HTTP)                     ║
╚═══════════════════════════════════════════════════════════════╝
"@ -ForegroundColor Cyan

# Check prerequisites
if (-not (Get-Command bore -ErrorAction SilentlyContinue)) {
    Write-Host "ERROR: Bore not installed. Run: cargo install bore-cli" -ForegroundColor Red
    exit 1
}

if (-not (Get-Command instatunnel -ErrorAction SilentlyContinue)) {
    Write-Host "ERROR: InstaTunnel not installed. Run: npm install -g instatunnel" -ForegroundColor Red
    exit 1
}

# Set environment
$env:RUST_LOG = "info"
$env:QNET_KEYPAIR_PATH = "P:\GITHUB\qnet\data\keypair.pb"

# Generate keypair if needed
if (-not (Test-Path $env:QNET_KEYPAIR_PATH)) {
    Write-Host "Generating persistent keypair..." -ForegroundColor Yellow
    New-Item -ItemType Directory -Path "P:\GITHUB\qnet\data" -Force | Out-Null
    cargo run -p stealth-browser -- --generate-keypair $env:QNET_KEYPAIR_PATH
}

Write-Host "`nStarting tunnels..." -ForegroundColor Cyan

# Start tunnels in new windows
Start-Process pwsh -ArgumentList "-NoExit", "-Command", "instatunnel 8088 -s qnet-status"
Start-Process pwsh -ArgumentList "-NoExit", "-Command", "bore local 1088 --to $BoreServer"
Start-Process pwsh -ArgumentList "-NoExit", "-Command", "bore local 4001 --to $BoreServer"

Write-Host "Waiting for tunnels to establish..." -ForegroundColor Yellow
Start-Sleep -Seconds 5

Write-Host "`nStarting QNet Super Peer..." -ForegroundColor Cyan
Write-Host "Check the tunnel windows for your public addresses!" -ForegroundColor Green

# Start super peer (this blocks)
cargo run --release -p stealth-browser -- --helper-mode super
```

---

## Part 10: Monitoring & Debugging

### 10.1 Check Tunnel Status

**Bore tunnels:** Each bore terminal shows connection activity

**InstaTunnel:** 
```powershell
instatunnel --list
instatunnel --logs
```

### 10.2 Monitor Super Peer

```powershell
Invoke-RestMethod http://127.0.0.1:8088/status | ConvertTo-Json -Depth 4
```

---

## Part 11: Troubleshooting

### Bore Issues

| Problem | Cause | Solution |
|---------|-------|----------|
| "Connection refused" to bore.pub | Firewall blocking outbound 7835 | Check firewall rules |
| Port already in use | Previous tunnel still running | Kill old bore processes |
| "authentication failed" | Wrong secret | Check BORE_SECRET matches server |

### InstaTunnel Issues

| Problem | Cause | Solution |
|---------|-------|----------|
| "Subdomain taken" | Name already in use | Choose different subdomain |
| Tunnel disconnects | 24-hour session limit | Restart tunnel |

### Common Fixes

```powershell
# Kill all tunnel processes
Get-Process bore -ErrorAction SilentlyContinue | Stop-Process
Get-Process node -ErrorAction SilentlyContinue | Stop-Process

# Check what's using ports
netstat -an | findstr "1088 8088 4001 7835"
```

---

## Part 12: Security Considerations

### 12.1 Development/Testing Only

⚠️ **This setup is for development/testing only, NOT production!**

- **InstaTunnel**: Third-party can see HTTP traffic patterns
- **bore.pub**: Public server, anyone can use it
- **No encryption**: Bore forwards raw TCP without encryption

### 12.2 For Production

Use a proper VPS with:
- Real public IP
- Self-hosted bore server with authentication
- Or no tunneling at all

See `droplet-testing.md` for cloud deployment.

---

## Part 13: Session Management

### Bore Sessions

Bore tunnels persist until:
- You press Ctrl+C
- Network disconnection
- bore.pub server restarts

**Ports are dynamic** - you get a new port each time you start a tunnel (unless using `--port` with self-hosted server).

### InstaTunnel Sessions

- 24-hour limit on free tier
- Restart tunnel daily

---

## Quick Reference

| Task | Command |
|------|---------|
| Install bore | `cargo install bore-cli` |
| Install instatunnel | `npm install -g instatunnel` |
| Start super peer | `cargo run --release -p stealth-browser -- --helper-mode super` |
| Tunnel Status API | `instatunnel 8088 -s qnet-status` |
| Tunnel SOCKS5 | `bore local 1088 --to bore.pub` |
| Tunnel libp2p | `bore local 4001 --to bore.pub` |
| Check local status | `Invoke-RestMethod http://127.0.0.1:8088/status` |
| Generate keypair | `cargo run -p stealth-browser -- --generate-keypair data/keypair.pb` |

---

## Next Steps

After successful home testing:

1. **Validate stability**: Run for several hours, monitor reconnections
2. **Test client connections**: Connect from another machine via bore tunnels
3. **Deploy to VPS**: Follow `droplet-testing.md` for production deployment

---

## Appendix A: Tool Comparison

| Feature | Bore | InstaTunnel | ngrok |
|---------|------|-------------|-------|
| **Protocol** | Raw TCP only | HTTP/HTTPS only | HTTP + TCP |
| **Self-hostable** | ✅ Yes | ❌ No | ✅ Yes (paid) |
| **Free tier** | Unlimited (bore.pub) | 24h sessions, 3 tunnels | 2h sessions, 1 tunnel |
| **Fixed ports** | ✅ With self-hosted | ❌ Random subdomains | ✅ Paid only |
| **Authentication** | HMAC shared secret | API key | Auth token |
| **Encryption** | ❌ None (app layer) | ✅ TLS | ✅ TLS |
| **Best for** | SOCKS5, libp2p, SSH | Web APIs, webhooks | General purpose |
