# QNet Exit Node Deployment Guide

## Overview
Deploy and configure QNet operator exit nodes on DigitalOcean droplets. These nodes serve triple duty as:
1. **Primary bootstrap** nodes for peer discovery
2. **Relay nodes** for mesh networking
3. **Exit nodes** for actual web requests

## Prerequisites
- DigitalOcean account (or Linode/Vultr)
- SSH access configured
- Basic Linux command line knowledge

---

## Step 1: Create Droplets

### Recommended Configuration
- **Provider**: DigitalOcean
- **Size**: Basic Droplet ($4-6/month)
  - 512 MB RAM
  - 1 vCPU
  - 10 GB SSD
- **OS**: Ubuntu 22.04 LTS (x64)

### Geographic Distribution
Deploy 6 droplets across regions for optimal global coverage:
1. **NYC** (New York - Americas)
2. **AMS** (Amsterdam - Europe)
3. **SIN** (Singapore - Asia-Pacific)
4. **FRA** (Frankfurt - Europe)
5. **TOR** (Toronto - Americas)
6. **SYD** (Sydney - Oceania)

### Create Via DigitalOcean Dashboard
1. Click "Create" → "Droplets"
2. Select Ubuntu 22.04 LTS
3. Choose Basic plan ($4-6/month)
4. Select region
5. Add SSH key
6. Create droplet
7. Note the public IPv4 address
8. Repeat for each region

---

## Step 2: Install QNet on Each Droplet

### SSH Into Droplet
```bash
ssh root@<DROPLET_IP>
```

### Install Rust
```bash
# Install rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Load Rust environment
source "$HOME/.cargo/env"

# Verify installation
rustc --version
```

### Clone and Build QNet
```bash
# Install git if not present
apt-get update && apt-get install -y git

# Clone repository
git clone https://github.com/QW1CKS/qnet.git
cd qnet

# Build stealth-browser
cargo build --release -p stealth-browser

# Verify build
./target/release/stealth-browser --help
```

---

## Step 3: Configure as Exit + Bootstrap Node

### Set Environment Variables
Create `/etc/qnet/env` file:
```bash
# Create config directory
mkdir -p /etc/qnet

# Create environment file
cat > /etc/qnet/env << 'EOF'
# QNet Exit Node Configuration
QNET_MODE=bootstrap
STEALTH_SOCKS_PORT=1088
STEALTH_STATUS_PORT=8088
QNET_STATUS_BIND=0.0.0.0:8088
STEALTH_DISABLE_BOOTSTRAP=0
QNET_MESH_ENABLED=1
EOF
```

**Configuration Explained**:
- `QNET_MODE=bootstrap`: Enables exit + bootstrap + relay mode
- `QNET_STATUS_BIND=0.0.0.0:8088`: Allow remote status monitoring
- `STEALTH_DISABLE_BOOTSTRAP=0`: Enable bootstrap discovery
- `QNET_MESH_ENABLED=1`: Enable mesh networking

---

## Step 4: Firewall Setup

### UFW Configuration
```bash
# Enable UFW
ufw --force enable

# Allow SSH
ufw allow 22/tcp

# Allow libp2p (peer discovery)
ufw allow 4001/tcp

# Allow status API (for monitoring)
ufw allow 8088/tcp

# Verify rules
ufw status
```

### Expected Output:
```
Status: active

To                         Action      From
--                         ------      ----
22/tcp                     ALLOW       Anywhere
4001/tcp                   ALLOW       Anywhere
8088/tcp                   ALLOW       Anywhere
```

---

## Step 5: Systemd Service Setup

### Create Service File
```bash
cat > /etc/systemd/system/qnet-exit.service << 'EOF'
[Unit]
Description=QNet Exit + Bootstrap Node
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=/root/qnet
EnvironmentFile=/etc/qnet/env
ExecStart=/root/qnet/target/release/stealth-browser --bootstrap --exit-node
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF
```

### Enable and Start Service
```bash
# Reload systemctl daemon
systemctl daemon-reload

# Enable service (start on boot)
systemctl enable qnet-exit

# Start service
systemctl start qnet-exit

# Check status
systemctl status qnet-exit
```

### View Logs
```bash
# Real-time logs
journalctl -u qnet-exit -f

# Last 100 lines
journalctl -u qnet-exit -n 100
```

---

## Step 6: Get Peer ID and Verify

### Extract Peer ID from Logs
```bash
# Wait ~10 seconds for startup, then grep for peer ID
journalctl -u qnet-exit | grep "Generated local peer ID"
```

**Example Output**:
```
mesh: Generated local peer ID peer_id=12D3KooWAbc123ExamplePeerId
```

Copy the peer ID (starts with `12D3KooW...`)

### Verify Status API
```bash
# Check status from droplet
curl http://localhost:8088/status | jq

# Check from remote (replace IP)
curl http://<DROPLET_IP>:8088/status | jq
```

**Expected Fields**:
- `state`: "connected"
- `mesh_peer_count`: > 0
- `mode`: "direct" or "masked"

---

## Step 7: Update discovery.rs with IPs

After deploying all 6 droplets, collect:
1. Public IPv4 address for each droplet
2. Peer ID from logs for each droplet

### Update Source Code
Edit `crates/core-mesh/src/discovery.rs`, replace placeholder with:

```rust
fn qnet_operator_seeds() -> Vec<BootstrapNode> {
    vec![
        BootstrapNode::new(
            "12D3KooW...".parse().unwrap(),  // NYC peer ID
            "/ip4/198.51.100.10/tcp/4001".parse().unwrap(),  // NYC IP
        ),
        BootstrapNode::new(
            "12D3KooW...".parse().unwrap(),  // AMS peer ID
            "/ip4/203.0.113.20/tcp/4001".parse().unwrap(),  // AMS IP
        ),
        // ... add remaining droplets
    ]
}
```

### Rebuild and Release
```bash
cd /path/to/qnet
cargo build --release -p stealth-browser
```

Distribute new binary via GitHub releases.

---

## Monitoring

### Check Node Health
```bash
# Service status
systemctl status qnet-exit

# Recent logs
journalctl -u qnet-exit -n 50

# Peer count
curl http://localhost:8088/status | jq '.mesh_peer_count'
```

### Recommended Monitoring
- **Uptime**: Monitor via status API (`GET /status`)
- **Peer count**: Should be > 0 after ~30 seconds
- **Disk space**: Monitor via `df -h`
- **Memory**: Monitor via `free -h`

---

## Troubleshooting

### Service Won't Start
```bash
# Check logs
journalctl -u qnet-exit -n 100

# Common issues:
# - Port 4001 already in use: kill other process or change port
# - Missing environment file: check /etc/qnet/env exists
# - Permission denied: ensure binary is executable (chmod +x)
```

### No Peer Connections
```bash
# Verify firewall allows 4001
ufw status | grep 4001

# Check if libp2p is listening
ss -tulpn | grep 4001

# Test public IPFS DHT connectivity
curl https://bootstrap.libp2p.io
```

### High Memory Usage
```bash
# Check current usage
free -h

# If needed, restart service
systemctl restart qnet-exit
```

---

## Cost Estimate

| Item | Cost | Quantity | Total |
|------|------|----------|-------|
| Basic Droplet | $4-6/month | 6 | $24-36/month |

**Total**: $24-36/month for global exit node infrastructure

**No longer needed**: Catalog hosting ($5/month) - REMOVED

---

## Security Best Practices

1. **SSH Keys Only**: Disable password authentication
2. **UFW Enabled**: Only expose necessary ports
3. **Regular Updates**: `apt-get update && apt-get upgrade`
4. **Monitor Logs**: Check for abuse or unexpected traffic
5. **Rate Limiting**: Consider nginx reverse proxy for status API

---

## Legal Considerations

⚠️ **EXIT NODE LIABILITY**

As an exit node operator, your IP address will be visible to destination websites. You may receive:
- DMCA notices
- Abuse complaints
- Legal inquiries

**Recommendations**:
- Use VPS provider with clear ToS supporting exit nodes
- Respond promptly to abuse complaints
- Consider liability insurance if running at scale
- Document exit node policy clearly

**Relay-only mode** (default for users) has NO legal liability - only forwards encrypted packets.

---

## Updates

### Update QNet Version
```bash
cd /root/qnet
git pull
cargo build --release -p stealth-browser
systemctl restart qnet-exit
```

### Update Hardcoded IPs
Requires new binary release:
1. Update `discovery.rs` with new IPs/peer IDs
2. Build release binary
3. Distribute via GitHub
4. Users download and replace binary
