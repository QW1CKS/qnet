# QNet Home Super Peer Testing Guide (InstaTunnel)

This guide provides step-by-step instructions for running a QNet super peer on a home laptop/desktop using **InstaTunnel** to bypass CGNAT and expose your local services to the internet.

---

## Why InstaTunnel?

Most home networks are behind CGNAT (Carrier-Grade NAT), making traditional port forwarding impossible. InstaTunnel solves this by creating secure tunnels from your laptop to the public internet.

| Feature | InstaTunnel | ngrok | Cloudflare Tunnel |
|---------|-------------|-------|-------------------|
| **Free Session Duration** | 24 hours | 2 hours | Unlimited |
| **Free Concurrent Tunnels** | 3 | 1 | Unlimited |
| **Custom Subdomains (Free)** | ✅ Yes | ❌ No | ✅ Yes |
| **Setup Time** | < 30 seconds | ~5 minutes | ~10 minutes |
| **Account Required** | Optional | Required | Required |
| **TCP Tunnels** | ✅ Yes | ✅ Yes | ✅ Yes |
| **Request Inspection** | ✅ Dashboard | ✅ Dashboard | ❌ No |
| **Cost (Pro tier)** | $5/mo | $10/mo | Free |

**InstaTunnel is ideal for QNet because:**
- **24-hour sessions** vs ngrok's 2-hour limit
- **3 free tunnels** (enough for Status API + SOCKS5 + libp2p)
- **Custom subdomains** for consistent, memorable URLs
- **Zero configuration** - works immediately
- **TCP tunnel support** - essential for libp2p mesh connections

---

## Prerequisites

### Hardware Requirements
- Windows 10/11 laptop or desktop
- Minimum 8GB RAM (16GB recommended)
- 10GB free disk space
- Internet connection (WiFi or Ethernet)

### Software Requirements
- Rust toolchain installed (`rustup` with stable channel)
- Node.js and npm (for InstaTunnel installation)
- PowerShell 7+ or Windows Terminal
- Git for Windows
- QNet source code cloned locally

---

## Part 1: Verify CGNAT Status

Before proceeding, confirm you need InstaTunnel:

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
- ✅ **IPs match** → You have a real public IP (port forwarding may work, but InstaTunnel is still easier)
- ❌ **IPs don't match** → You're behind CGNAT (InstaTunnel required)

**Common CGNAT indicators:**
- Router WAN IP starts with `100.64.x.x` to `100.127.x.x` (CGNAT range)
- Router WAN IP is private (`10.x.x.x`, `172.16-31.x.x`, `192.168.x.x`)

---

## Part 2: Install InstaTunnel

### 2.1 Install via NPM (Recommended)

```powershell
# Install globally
npm install -g instatunnel

# Verify installation
instatunnel --version
```

### 2.2 Alternative: Direct Download

1. Visit: https://www.instatunnel.my/downloads
2. Download Windows installer
3. Run installer
4. Add to PATH if not automatic

### 2.3 Test Installation

```powershell
# Quick test - expose a simple HTTP server
# First, start a test server
npx http-server -p 8888

# In another terminal, create tunnel
instatunnel http 8888 --name test-tunnel

# You should see:
# ✓ Tunnel created
# ✓ URL: https://test-tunnel.instatunnel.my
```

---

## Part 3: Build QNet Super Peer

### 3.1 Build the Binary

```powershell
cd P:\GITHUB\qnet

# Build release version (faster runtime)
cargo build --release -p stealth-browser

# Or debug version (faster compilation)
cargo build -p stealth-browser
```

### 3.2 Generate Persistent Keypair

For a stable peer ID across restarts:

```powershell
# Create data directory
New-Item -ItemType Directory -Path "P:\GITHUB\qnet\data" -Force

# Generate keypair
cargo run -p stealth-browser -- --generate-keypair P:\GITHUB\qnet\data\keypair.pb
```

Note the peer ID from the output (e.g., `12D3KooWABC123...`). You'll need this later.

---

## Part 4: Start QNet Super Peer

### 4.1 Launch Super Peer

```powershell
cd P:\GITHUB\qnet

# Set environment variables
$env:RUST_LOG = "info"
$env:QNET_KEYPAIR_PATH = "P:\GITHUB\qnet\data\keypair.pb"

# Start super peer
cargo run --release -p stealth-browser -- --helper-mode super
```

**Expected output:**
```
[INFO] stealth-browser starting
[INFO] config loaded port=1088 status_port=8088 mode=Direct helper_mode=Super
[INFO] helper mode features helper_mode=Super features="all features"
[INFO] Loaded persistent keypair peer_id=12D3KooW...
[INFO] status server listening status_addr=0.0.0.0:8088
[INFO] starting SOCKS5 server addr=0.0.0.0:1088 mode=Direct
[INFO] mesh: Listening on /ip4/0.0.0.0/tcp/4001
[INFO] mesh: local_peer_id=12D3KooW...
```

**Keep this terminal running!**

### 4.2 Verify Local Access

Open a new terminal:
```powershell
# Test status endpoint
Invoke-RestMethod http://127.0.0.1:8088/status | ConvertTo-Json

# Test ping
Invoke-RestMethod http://127.0.0.1:8088/ping
```

---

## Part 5: Create InstaTunnel Tunnels

You need **3 tunnels** for full super peer functionality:

| Port | Service | Tunnel Type | Purpose |
|------|---------|-------------|---------|
| 8088 | Status API | HTTP | Web status page, directory endpoints |
| 1088 | SOCKS5 Proxy | TCP | Client proxy connections |
| 4001 | libp2p | TCP | Mesh peer connections |

### 5.1 Create Status API Tunnel (HTTP)

Open **Terminal 2**:
```powershell
# Create HTTP tunnel with password protection
instatunnel http 8088 --name qnet-status --password "YourSecurePassword123"
```

**Output:**
```
✓ Tunnel created
✓ URL: https://qnet-status.instatunnel.my
✓ Password protected
```

**Record this URL:** `https://qnet-status.instatunnel.my`

### 5.2 Create SOCKS5 Proxy Tunnel (TCP)

Open **Terminal 3**:
```powershell
# Create TCP tunnel for SOCKS5 proxy
instatunnel tcp 1088 --name qnet-socks
```

**Output:**
```
✓ TCP Tunnel created  
✓ Address: tcp://qnet-socks.instatunnel.my:12345
```

**Record this address:** `qnet-socks.instatunnel.my:12345` (port will vary)

### 5.3 Create libp2p Mesh Tunnel (TCP)

Open **Terminal 4**:
```powershell
# Create TCP tunnel for libp2p mesh
instatunnel tcp 4001 --name qnet-mesh
```

**Output:**
```
✓ TCP Tunnel created
✓ Address: tcp://qnet-mesh.instatunnel.my:23456
```

**Record this address:** `qnet-mesh.instatunnel.my:23456` (port will vary)

### 5.4 Summary of Your Tunnels

| Service | Local Port | InstaTunnel URL |
|---------|------------|-----------------|
| Status API | 8088 | `https://qnet-status.instatunnel.my` |
| SOCKS5 | 1088 | `tcp://qnet-socks.instatunnel.my:<PORT>` |
| libp2p | 4001 | `tcp://qnet-mesh.instatunnel.my:<PORT>` |

---

## Part 6: Update Hardcoded Operator Nodes

For clients to discover your home super peer, update the bootstrap configuration.

### 6.1 Edit discovery.rs

Open `crates/core-mesh/src/discovery.rs` and find `hardcoded_operator_nodes()`:

```rust
pub fn hardcoded_operator_nodes() -> Vec<OperatorNode> {
    vec![
        OperatorNode {
            peer_id: "12D3KooWYourPeerIdFromStep3".to_string(),
            multiaddr: "/dns4/qnet-mesh.instatunnel.my/tcp/23456".to_string(),
        },
    ]
}
```

**Replace:**
- `12D3KooWYourPeerIdFromStep3` → Your actual peer ID from keypair generation
- `23456` → The actual port from your libp2p tunnel output

### 6.2 Rebuild After Changes

```powershell
cargo build --release -p stealth-browser
```

---

## Part 7: Test External Access

### 7.1 Test Status API

From your phone (on mobile data, NOT WiFi) or any external device:

**Browser test:**
1. Open: `https://qnet-status.instatunnel.my`
2. Enter password when prompted: `YourSecurePassword123`
3. You should see the QNet status page

**Command line test:**
```bash
# From any external machine
curl -u ":YourSecurePassword123" https://qnet-status.instatunnel.my/ping

# Expected: {"ok":true,"ts":1234567890}
```

### 7.2 Test SOCKS5 Proxy

```bash
# Replace 12345 with your actual SOCKS tunnel port
curl --socks5-hostname qnet-socks.instatunnel.my:12345 https://httpbin.org/ip

# Expected: {"origin": "YOUR_HOME_IP"}
```

### 7.3 Test Directory API

```bash
curl -u ":YourSecurePassword123" https://qnet-status.instatunnel.my/api/relays/by-country

# Expected: {} (empty initially, or list of registered relays)
```

---

## Part 8: Test Client Connections

### 8.1 Run Client on Another Machine

On a different computer (or the same one with different ports):

1. **Update discovery.rs** with your InstaTunnel addresses (as shown in Part 6)

2. **Build and run client:**
```powershell
cd P:\GITHUB\qnet
$env:STEALTH_SINGLE_INSTANCE_OVERRIDE = "1"
cargo run -p stealth-browser -- --socks-port 1089 --status-port 8089
```

3. **Watch logs for connection:**
```
[INFO] mesh: Dialing /dns4/qnet-mesh.instatunnel.my/tcp/23456
[INFO] mesh: Connection established peer_id=12D3KooW...
```

### 8.2 Verify Mesh Connection

Check your super peer's status:
```powershell
Invoke-RestMethod http://127.0.0.1:8088/status | ConvertTo-Json
```

Look for `peers_online` > 0.

---

## Part 9: Convenience Scripts

### 9.1 Start All Tunnels Script

Create `P:\GITHUB\qnet\scripts\start-tunnels.ps1`:

```powershell
# QNet InstaTunnel Startup Script
param(
    [string]$Password = "ChangeThisPassword123"
)

Write-Host "Starting QNet InstaTunnel tunnels..." -ForegroundColor Cyan

# Start tunnels as background jobs
$statusJob = Start-Job -Name "qnet-status" -ScriptBlock {
    param($pwd)
    instatunnel http 8088 --name qnet-status --password $pwd
} -ArgumentList $Password

$socksJob = Start-Job -Name "qnet-socks" -ScriptBlock {
    instatunnel tcp 1088 --name qnet-socks
}

$meshJob = Start-Job -Name "qnet-mesh" -ScriptBlock {
    instatunnel tcp 4001 --name qnet-mesh
}

# Wait a moment for tunnels to establish
Start-Sleep -Seconds 3

# Show status
Write-Host "`nTunnel Jobs:" -ForegroundColor Green
Get-Job | Where-Object { $_.Name -like "qnet-*" } | Format-Table Name, State

Write-Host "`nTo view tunnel URLs:" -ForegroundColor Yellow
Write-Host "  Receive-Job -Name qnet-status -Keep"
Write-Host "  Receive-Job -Name qnet-socks -Keep"
Write-Host "  Receive-Job -Name qnet-mesh -Keep"

Write-Host "`nTo stop all tunnels:" -ForegroundColor Yellow
Write-Host "  Get-Job -Name 'qnet-*' | Stop-Job; Get-Job -Name 'qnet-*' | Remove-Job"
```

**Usage:**
```powershell
.\scripts\start-tunnels.ps1 -Password "MySecurePassword"
```

### 9.2 Full Startup Script

Create `P:\GITHUB\qnet\scripts\start-home-superpeer.ps1`:

```powershell
# QNet Home Super Peer Full Startup
param(
    [string]$Password = "ChangeThisPassword123"
)

$ErrorActionPreference = "Stop"

Write-Host @"
╔═══════════════════════════════════════════════════════════════╗
║           QNet Home Super Peer (InstaTunnel Mode)             ║
╚═══════════════════════════════════════════════════════════════╝
"@ -ForegroundColor Cyan

# Check prerequisites
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

Write-Host "`nStarting InstaTunnel tunnels..." -ForegroundColor Cyan

# Start tunnels in new windows
Start-Process pwsh -ArgumentList "-NoExit", "-Command", "instatunnel http 8088 --name qnet-status --password '$Password'"
Start-Process pwsh -ArgumentList "-NoExit", "-Command", "instatunnel tcp 1088 --name qnet-socks"
Start-Process pwsh -ArgumentList "-NoExit", "-Command", "instatunnel tcp 4001 --name qnet-mesh"

Write-Host "Waiting for tunnels to establish..." -ForegroundColor Yellow
Start-Sleep -Seconds 5

Write-Host "`nStarting QNet Super Peer..." -ForegroundColor Cyan
Write-Host "Check the tunnel windows for your public URLs!" -ForegroundColor Green

# Start super peer (this blocks)
cargo run --release -p stealth-browser -- --helper-mode super
```

---

## Part 10: Monitoring & Debugging

### 10.1 InstaTunnel Dashboard

InstaTunnel provides a web dashboard for request inspection:

1. Open: https://dashboard.instatunnel.my
2. View real-time request/response logs
3. Inspect headers, bodies, status codes
4. Replay failed requests for debugging

### 10.2 Check Tunnel Status

```powershell
# View all tunnel jobs
Get-Job -Name "qnet-*"

# View specific tunnel output
Receive-Job -Name qnet-status -Keep
Receive-Job -Name qnet-mesh -Keep
```

### 10.3 Monitor Super Peer Logs

The super peer terminal shows real-time connection info:
- Incoming mesh connections
- Directory registrations
- Exit node requests
- Ping/keepalive events

### 10.4 Check Mesh Health

```powershell
# Local status
Invoke-RestMethod http://127.0.0.1:8088/status | ConvertTo-Json -Depth 4

# Remote status (via tunnel)
curl -u ":YourPassword" https://qnet-status.instatunnel.my/status
```

---

## Part 11: Troubleshooting

### Tunnel Issues

| Problem | Cause | Solution |
|---------|-------|----------|
| "Tunnel name already in use" | Previous session still active | Wait 5 min or use different name |
| Tunnel disconnects | 24-hour session limit | Restart tunnel |
| "Connection refused" | Super peer not running | Start super peer first |
| Slow connections | Network latency | Normal (~45ms overhead) |

### Mesh Connection Issues

| Problem | Cause | Solution |
|---------|-------|----------|
| 0 peers connected | No clients connecting | Normal if just started |
| Dial failures | Wrong multiaddr in discovery.rs | Check hostname and port |
| Peer ID mismatch | Using wrong keypair | Regenerate or use correct path |

### Common Fixes

```powershell
# Restart all tunnels
Get-Job -Name "qnet-*" | Stop-Job
Get-Job -Name "qnet-*" | Remove-Job
.\scripts\start-tunnels.ps1

# Check if ports are in use
netstat -an | findstr "1088 8088 4001"

# Kill orphaned processes
Get-Process stealth-browser -ErrorAction SilentlyContinue | Stop-Process
```

---

## Part 12: Security Considerations

### 12.1 Development/Testing Only

⚠️ **InstaTunnel is for development/testing only, NOT production!**

Reasons:
1. **Metadata exposure**: InstaTunnel can see connection patterns and timing
2. **Centralization**: Traffic routes through InstaTunnel servers
3. **No censorship resistance**: Standard HTTPS, easily blocked
4. **Session limits**: 24-hour tunnels require daily restarts

### 12.2 Security Best Practices

**Always use password protection:**
```powershell
# Generate random password
$password = -join ((65..90) + (97..122) + (48..57) | Get-Random -Count 20 | ForEach-Object { [char]$_ })
instatunnel http 8088 --name qnet-status --password $password
Write-Host "Password: $password"
```

**Rotate subdomain names:**
```powershell
$date = Get-Date -Format "yyyyMMdd"
instatunnel http 8088 --name "qnet-status-$date"
```

**Monitor access:**
- Check InstaTunnel dashboard for unexpected requests
- Review super peer logs for suspicious activity

### 12.3 Production Deployment

For 24/7 production super peers, use a proper VPS:
- See `droplet-testing.md` for DigitalOcean deployment
- Cost: ~$6/month for basic droplet
- Benefits: Real public IP, no tunnels, full control

---

## Part 13: Session Management

### 13.1 Handling 24-Hour Limit

InstaTunnel free tier disconnects after 24 hours. Options:

**Option A: Manual restart (daily)**
```powershell
# Stop old tunnels
Get-Job -Name "qnet-*" | Stop-Job; Get-Job -Name "qnet-*" | Remove-Job

# Start fresh tunnels
.\scripts\start-tunnels.ps1
```

**Option B: Scheduled task (automatic)**
```powershell
# Create daily restart task
$action = New-ScheduledTaskAction -Execute "pwsh.exe" -Argument "-File P:\GITHUB\qnet\scripts\start-tunnels.ps1"
$trigger = New-ScheduledTaskTrigger -Daily -At "3:00AM"
Register-ScheduledTask -TaskName "QNet Tunnel Restart" -Action $action -Trigger $trigger
```

**Option C: Upgrade to Pro ($5/mo)**
- Unlimited session duration
- 10 concurrent tunnels
- Custom domains

### 13.2 Keeping Super Peer Running

The super peer itself doesn't have session limits. Only tunnels need restarting.

---

## Quick Reference

| Task | Command |
|------|---------|
| Install InstaTunnel | `npm install -g instatunnel` |
| Start super peer | `cargo run --release -p stealth-browser -- --helper-mode super` |
| Tunnel status API | `instatunnel http 8088 --name qnet-status --password "xxx"` |
| Tunnel SOCKS5 | `instatunnel tcp 1088 --name qnet-socks` |
| Tunnel libp2p | `instatunnel tcp 4001 --name qnet-mesh` |
| Check local status | `Invoke-RestMethod http://127.0.0.1:8088/status` |
| Check tunnel status | `Receive-Job -Name qnet-status -Keep` |
| Stop all tunnels | `Get-Job -Name "qnet-*" \| Stop-Job; Get-Job -Name "qnet-*" \| Remove-Job` |
| Generate keypair | `cargo run -p stealth-browser -- --generate-keypair data/keypair.pb` |

---

## Next Steps

After successful home testing with InstaTunnel:

1. **Validate stability**: Run for 24-48 hours (restart tunnels daily)
2. **Test reconnection**: Stop/start super peer, verify clients reconnect
3. **Load test**: Connect multiple clients simultaneously
4. **Deploy to droplet**: Follow `droplet-testing.md` for production deployment

---

## Appendix A: Fallback Options

### ngrok (Alternative)

If InstaTunnel is unavailable:

```powershell
# Install
choco install ngrok
# or download from https://ngrok.com/download

# Authenticate (required)
ngrok config add-authtoken YOUR_AUTH_TOKEN

# Create tunnels (one at a time on free tier)
ngrok http 8088
```

**Limitations:**
- 2-hour session limit (vs InstaTunnel's 24 hours)
- Only 1 tunnel on free tier (vs InstaTunnel's 3)
- Account required

### Cloudflare Tunnel (Alternative)

For unlimited sessions (requires Cloudflare account + DNS):

```powershell
# Install cloudflared
choco install cloudflared

# Authenticate
cloudflared tunnel login

# Create tunnel
cloudflared tunnel create qnet-home
cloudflared tunnel route dns qnet-home qnet.yourdomain.com

# Run tunnel
cloudflared tunnel run --url http://localhost:8088 qnet-home
```

**Benefits:**
- Unlimited session duration
- Free forever
- DDoS protection

**Drawbacks:**
- Requires domain with Cloudflare DNS
- More complex setup (~10 minutes)
- No request inspection dashboard

---

## Appendix B: Port Forwarding (Non-CGNAT Users)

If you have a real public IP (router WAN IP = public IP), you can use traditional port forwarding instead of InstaTunnel. This requires:

1. Router admin access
2. Static/reserved IP for your laptop
3. Windows Firewall rules
4. Three port forwarding rules (1088, 8088, 4001)

For detailed port forwarding instructions, see the archived version in git history or search online for "[Your Router Model] port forwarding guide".

InstaTunnel is still recommended even for non-CGNAT users due to simpler setup.
