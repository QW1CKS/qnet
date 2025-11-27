# Running a QNet Exit Node

Guide for deploying and operating QNet exit nodes to support the network.

## 📋 Table of Contents

- [Overview](#overview)
- [Requirements](#requirements)
- [Deployment](#deployment)
- [Configuration](#configuration)
- [Monitoring](#monitoring)
- [Maintenance](#maintenance)
- [Legal Considerations](#legal-considerations)

---

## 🌐 Overview

### What is an Exit Node?

Exit nodes are QNet peers that:
- **Forward traffic** to the public internet for other users
- **Maintain high uptime** to ensure network reliability
- **Provide bandwidth** for censorship circumvention
- **Operate transparently** following network policies

### Three-Tier Exit Architecture

```
┌─────────────────────────────────────────────────────┐
│ Tier 1: User Helpers (99% of network)              │
│ - Relay-only mode (no exit traffic)                │
│ - Default for all users                             │
│ - Safe for residential connections                  │
└─────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────┐
│ Tier 2: Operator Exits (Primary)                   │
│ - VPS/dedicated servers                             │
│ - Official QNet infrastructure                      │
│ - Bandwidth-optimized                               │
└─────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────┐
│ Tier 3: Volunteer Exits (Community)                │
│ - Opt-in advanced users                             │
│ - Community-run infrastructure                      │
│ - Accepts exit traffic                              │
└─────────────────────────────────────────────────────┘
```

**Who should run exit nodes:**
- Tier 2: QNet maintainers, organizations
- Tier 3: Advanced users with legal/bandwidth capacity

**Who should NOT run exit nodes:**
- Home users (ISP may block traffic)
- Residential connections (legal liability)
- Users without legal understanding

---

## 🔧 Requirements

### Hardware Requirements

**Minimum:**
- CPU: 2 cores
- RAM: 2 GB
- Storage: 20 GB SSD
- Network: 100 Mbps symmetric

**Recommended:**
- CPU: 4 cores
- RAM: 4 GB
- Storage: 50 GB SSD
- Network: 1 Gbps symmetric

### VPS Providers

**Recommended providers:**
- **DigitalOcean**: Reliable, affordable, good network
- **Hetzner**: Excellent bandwidth, EU-friendly
- **Vultr**: Global presence, flexible
- **Linode**: Stable, good support

**Provider requirements:**
- Allow outbound traffic on all ports
- Permit Tor/VPN-like services (check ToS)
- Provide static IP address
- Allow reverse DNS setup

### Operating System

**Supported:**
- Ubuntu 22.04 LTS (recommended)
- Debian 12
- Fedora 38+
- Any Linux with systemd

**Not recommended:**
- Windows Server (performance issues)
- macOS (licensing restrictions)

---

## 🚀 Deployment

### Quick Deploy (Ubuntu 22.04)

```bash
#!/bin/bash
# QNet Exit Node Deploy Script

# Update system
sudo apt-get update && sudo apt-get upgrade -y

# Install dependencies
sudo apt-get install -y build-essential pkg-config libssl-dev curl

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"

# Create qnet user
sudo useradd -r -m -s /bin/bash qnet

# Clone QNet
cd /opt
sudo git clone https://github.com/QW1CKS/qnet.git
sudo chown -R qnet:qnet qnet

# Build release
cd qnet
sudo -u qnet cargo build -p stealth-browser --release

# Install binary
sudo cp target/release/stealth-browser /usr/local/bin/
sudo chown qnet:qnet /usr/local/bin/stealth-browser
sudo chmod +x /usr/local/bin/stealth-browser

# Create config directory
sudo mkdir -p /etc/qnet
sudo chown qnet:qnet /etc/qnet

echo "QNet exit node binary installed!"
echo "Next: Configure systemd service and firewall"
```

### systemd Service Setup

Create `/etc/systemd/system/qnet-exit.service`:

```ini
[Unit]
Description=QNet Exit Node
Documentation=https://github.com/QW1CKS/qnet
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=qnet
Group=qnet
WorkingDirectory=/opt/qnet

# Environment configuration
Environment="RUST_LOG=info"
Environment="QNET_MODE=exit"
Environment="QNET_STATUS_BIND=0.0.0.0:8088"
Environment="QNET_SOCKS_PORT=1088"

# Resource limits
LimitNOFILE=65536
LimitNPROC=4096

# Execution
ExecStart=/usr/local/bin/stealth-browser
Restart=on-failure
RestartSec=10s

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/log/qnet

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=qnet-exit

[Install]
WantedBy=multi-user.target
```

**Enable and start:**
```bash
sudo systemctl daemon-reload
sudo systemctl enable qnet-exit
sudo systemctl start qnet-exit
sudo systemctl status qnet-exit
```

### Firewall Configuration

```bash
# UFW (Ubuntu/Debian)
sudo ufw allow 8088/tcp comment 'QNet Status API'
sudo ufw allow 1088/tcp comment 'QNet SOCKS5'
sudo ufw allow proto tcp from any to any port 1024:65535 comment 'QNet P2P'
sudo ufw allow proto udp from any to any port 1024:65535 comment 'QNet P2P UDP'
sudo ufw enable

# firewalld (Fedora/RHEL)
sudo firewall-cmd --permanent --add-port=8088/tcp
sudo firewall-cmd --permanent --add-port=1088/tcp
sudo firewall-cmd --permanent --add-port=1024-65535/tcp
sudo firewall-cmd --permanent --add-port=1024-65535/udp
sudo firewall-cmd --reload
```

---

## ⚙️ Configuration

### Exit Node Policy

Create `/etc/qnet/exit-policy.conf`:

```bash
# Exit Node Policy Configuration

# Bandwidth limits (bytes/sec)
MAX_UPLOAD_RATE=10485760    # 10 MB/s
MAX_DOWNLOAD_RATE=10485760  # 10 MB/s

# Connection limits
MAX_CONNECTIONS=1000
MAX_CONNECTIONS_PER_IP=10

# Blocked ports (commonly abused)
BLOCKED_PORTS="25,465,587"  # SMTP

# Allow/deny exit to specific destinations
# Format: CIDR notation
ALLOWED_DESTINATIONS="0.0.0.0/0"  # All destinations
BLOCKED_DESTINATIONS="10.0.0.0/8,172.16.0.0/12,192.168.0.0/16"  # Private IPs

# Exit policy mode
# - open: Allow all traffic (default)
# - restricted: Block high-risk protocols
# - custom: Use BLOCKED_PORTS and BLOCKED_DESTINATIONS
EXIT_POLICY_MODE="restricted"

# Logging
LOG_EXIT_CONNECTIONS=true
LOG_LEVEL="info"
```

### Network Optimization

**TCP tuning (`/etc/sysctl.d/99-qnet.conf`):**
```bash
# Increase TCP buffer sizes
net.core.rmem_max = 134217728
net.core.wmem_max = 134217728
net.ipv4.tcp_rmem = 4096 87380 67108864
net.ipv4.tcp_wmem = 4096 65536 67108864

# Enable TCP BBR congestion control
net.core.default_qdisc = fq
net.ipv4.tcp_congestion_control = bbr

# Increase connection tracking table size
net.netfilter.nf_conntrack_max = 262144

# Apply settings
sudo sysctl -p /etc/sysctl.d/99-qnet.conf
```

### Reverse DNS Setup

Set reverse DNS (PTR record) to indicate exit node:

```
exit.qnet.yourdomain.com
```

This helps with abuse reports and transparency.

---

## 📊 Monitoring

### Health Checks

**Status API:**
```bash
curl http://localhost:8088/status | jq
```

**Expected response:**
```json
{
  "mode": "exit",
  "state": "running",
  "peers_online": 15,
  "active_circuits": 5,
  "relay_packets_relayed": 10234,
  "uptime_seconds": 86400
}
```

### Metrics Collection

**Prometheus exporter (future feature):**
```bash
# Metrics endpoint
curl http://localhost:8088/metrics
```

**Log monitoring:**
```bash
# Real-time logs
sudo journalctl -u qnet-exit -f

# Recent errors
sudo journalctl -u qnet-exit --since "1 hour ago" -p err
```

### Bandwidth Monitoring

```bash
# Install vnstat
sudo apt-get install vnstat

# Monitor interface
sudo vnstat -i eth0 -l

# Monthly bandwidth
vnstat -m
```

### Alerting

**Simple uptime monitoring:**
```bash
#!/bin/bash
# /usr/local/bin/qnet-healthcheck.sh

STATUS=$(curl -s http://localhost:8088/status | jq -r '.state')

if [ "$STATUS" != "running" ]; then
    echo "QNet exit node is down!" | mail -s "QNet Alert" admin@example.com
    systemctl restart qnet-exit
fi
```

**Cron job:**
```bash
# Add to crontab
*/5 * * * * /usr/local/bin/qnet-healthcheck.sh
```

---

## 🔧 Maintenance

### Updates

```bash
# Pull latest code
cd /opt/qnet
sudo -u qnet git pull

# Rebuild
sudo -u qnet cargo build -p stealth-browser --release

# Update binary
sudo cp target/release/stealth-browser /usr/local/bin/

# Restart service
sudo systemctl restart qnet-exit
```

### Log Rotation

Create `/etc/logrotate.d/qnet`:

```
/var/log/qnet/*.log {
    daily
    rotate 7
    compress
    delaycompress
    missingok
    notifempty
    create 0640 qnet qnet
    sharedscripts
    postrotate
        systemctl reload qnet-exit > /dev/null 2>&1 || true
    endscript
}
```

### Backup Configuration

```bash
# Backup script
#!/bin/bash
BACKUP_DIR="/backups/qnet-$(date +%Y%m%d)"
mkdir -p "$BACKUP_DIR"

# Backup configs
cp -r /etc/qnet "$BACKUP_DIR/"
cp /etc/systemd/system/qnet-exit.service "$BACKUP_DIR/"

# Compress
tar -czf "$BACKUP_DIR.tar.gz" "$BACKUP_DIR"
rm -rf "$BACKUP_DIR"

echo "Backup saved: $BACKUP_DIR.tar.gz"
```

---

## ⚖️ Legal Considerations

### Legal Framework

**Before running an exit node:**
1. **Understand your jurisdiction's laws** on internet services
2. **Consult with legal counsel** if operating commercially
3. **Review ISP/VPS Terms of Service** for exit traffic policies
4. **Set up abuse handling procedures**

### Abuse Handling

**Contact information:**
- Set up abuse@yourdomain.com email
- Add contact to reverse DNS
- Respond to abuse reports within 24 hours

**Sample abuse response:**
```
Hello,

Thank you for your report. We operate a QNet exit node, which
is part of a decentralized censorship-resistance network.

The IP address you reported is an exit node, not the origin of
the traffic. The actual user's IP is hidden by design.

We have the following logs for [timestamp]:
[Sanitized connection info]

We take abuse seriously. If you provide a court order, we will
cooperate with legitimate investigations.

For more information: https://github.com/QW1CKS/qnet

Best regards,
[Your name]
```

### Logging Requirements

**What to log (for abuse investigation):**
- Connection timestamps
- Source port (user-side)
- Destination IP/port
- Traffic volume

**What NOT to log:**
- User IP addresses (defeats anonymity)
- Packet contents
- Decrypted traffic

**Log retention:**
- 7 days minimum (abuse investigation)
- 30 days recommended
- Auto-delete after retention period

### DMCA / Copyright

**QNet stance:**
- Exit nodes are **mere conduits** under DMCA safe harbor
- Operators not liable for user content
- Respond to valid DMCA takedowns

**Sample DMCA response:**
```
We operate a QNet exit node (similar to Tor). We do not
host, store, or cache any content. We are a mere conduit.

Under 17 USC 512(a), we qualify for DMCA safe harbor.

If you believe infringement occurred, please contact the
website hosting the content, not the network infrastructure.
```

---

## 📞 Support

**Exit operator resources:**
- GitHub Discussions: https://github.com/QW1CKS/qnet/discussions
- Operator mailing list: operators@qnet.network (coming soon)
- Security contact: security@qnet.network

**Thank you for supporting internet freedom!** 🌍
