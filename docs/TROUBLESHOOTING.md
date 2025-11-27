# QNet Troubleshooting Guide

Common issues and solutions for QNet users and developers.

## 📋 Table of Contents

- [Helper Issues](#helper-issues)
- [Connection Problems](#connection-problems)
- [Peer Discovery](#peer-discovery)
- [Performance Issues](#performance-issues)
- [Build Errors](#build-errors)
- [Platform-Specific](#platform-specific)

---

## 🔧 Helper Issues

### Helper Won't Start

**Symptom:** `stealth-browser` exits immediately or shows port binding error

**Solution 1: Port Conflict**
```powershell
# Windows - Check what's using port 1088 (SOCKS5)
netstat -ano | findstr :1088
# Kill conflicting process
taskkill /PID <process_id> /F

# Check status port 8088
netstat -ano | findstr :8088
```

```bash
# Linux/macOS
lsof -i :1088
lsof -i :8088
# Kill process
kill <PID>
```

**Solution 2: Change Ports**
```powershell
# Use different ports
$env:QNET_STATUS_BIND = "127.0.0.1:9088"
$env:QNET_SOCKS_PORT = "1089"
.\stealth-browser.exe
```

**Solution 3: Check Permissions**
```bash
# Linux - Allow binding to low ports
sudo setcap 'cap_net_bind_service=+ep' ./stealth-browser

# Or run on high port
export QNET_SOCKS_PORT=11088
./stealth-browser
```

### Helper Crashes on Startup

**Check logs:**
```powershell
# Windows - Run with debug logging
$env:RUST_LOG = "debug"
.\stealth-browser.exe 2>&1 | Tee-Object -FilePath helper.log

# Linux/macOS
RUST_LOG=debug ./stealth-browser 2>&1 | tee helper.log
```

**Common causes:**
- Missing runtime dependencies (libssl, libcrypto)
- Corrupted configuration files
- Insufficient permissions

**Solutions:**
```bash
# Linux - Install dependencies
sudo apt-get install libssl-dev pkg-config  # Debian/Ubuntu
sudo yum install openssl-devel              # RHEL/CentOS

# macOS - Install OpenSSL
brew install openssl@3
```

### Status API Not Responding

**Check if Helper is running:**
```powershell
# Windows
Get-Process stealth-browser

# Linux/macOS
ps aux | grep stealth-browser
```

**Test status endpoint:**
```powershell
# Windows
Invoke-WebRequest -Uri "http://127.0.0.1:8088/ping"

# Linux/macOS
curl http://127.0.0.1:8088/ping
```

**Check firewall:**
```powershell
# Windows - Allow localhost connections
New-NetFirewallRule -DisplayName "QNet Helper" -Direction Inbound -LocalPort 8088 -Protocol TCP -Action Allow
```

---

## 🌐 Connection Problems

### Can't Connect Through SOCKS5

**Verify Helper status:**
```bash
curl http://127.0.0.1:8088/status
```

**Test SOCKS5 directly:**
```bash
# Using netcat
nc -X 5 -x 127.0.0.1:1088 example.com 80

# Using curl
curl --socks5 127.0.0.1:1088 https://example.com -v
```

**Check browser configuration:**
- Firefox: Settings → Network → Manual Proxy
- Chrome: Use SwitchyOmega extension
- Verify: `127.0.0.1:1088` and SOCKS5 selected

### Slow Connection Speed

**Check peer count:**
```bash
curl http://127.0.0.1:8088/status | jq '.peers_online'
```

**Reasons for slowness:**
1. **Few peers discovered** - Wait 2-3 minutes for DHT bootstrap
2. **Network congestion** - Try different exit nodes
3. **Multi-hop routing** - Fast mode uses direct routing (default)

**Enable fast mode (if disabled):**
```powershell
# Fast mode is default - verify
curl http://127.0.0.1:8088/status | jq '.mode'
# Should show: "socks5" or "fast"
```

### Intermittent Disconnections

**Monitor connection stability:**
```powershell
# Windows - Continuous status check
while ($true) { 
    (Invoke-WebRequest -Uri "http://127.0.0.1:8088/status" | ConvertFrom-Json).state
    Start-Sleep -Seconds 5
}
```

```bash
# Linux/macOS
watch -n 5 'curl -s http://127.0.0.1:8088/status | jq .state'
```

**Common causes:**
- Peer churn (nodes joining/leaving)
- NAT traversal issues
- Network instability

**Solutions:**
- Wait for network to stabilize (2-3 min)
- Check internet connection
- Restart Helper if persistent

### Connection Timeouts

**Check target reachability:**
```bash
# Test without QNet
curl https://example.com -I

# Test with QNet
curl --socks5 127.0.0.1:1088 https://example.com -I --max-time 30
```

**Increase timeout:**
```powershell
# Browser extension: Set timeout to 60s in settings
# Command line:
curl --socks5 127.0.0.1:1088 https://example.com --connect-timeout 60
```

---

## 👥 Peer Discovery

### No Peers Found

**Check NAT status:**
```bash
curl http://127.0.0.1:8088/status | jq '.nat_status'
```

**Verify bootstrap nodes:**
```bash
# Check if bootstrap nodes are reachable
curl -I https://bootstrap.libp2p.io
```

**Enable debug logging:**
```bash
RUST_LOG=debug,libp2p=debug ./stealth-browser 2>&1 | grep -i "peer\|bootstrap\|kad"
```

**Firewall rules:**
```bash
# Linux - Allow libp2p ports
sudo ufw allow proto tcp from any to any port 0:65535
sudo ufw allow proto udp from any to any port 0:65535

# Windows
New-NetFirewallRule -DisplayName "QNet P2P" -Direction Inbound -Protocol TCP -Action Allow
New-NetFirewallRule -DisplayName "QNet P2P UDP" -Direction Inbound -Protocol UDP -Action Allow
```

### Behind Strict NAT

**Check NAT type:**
```bash
curl http://127.0.0.1:8088/status | jq '.nat_status'
# Possible values: "unknown", "public", "private"
```

**For "private" (behind NAT):**
- QNet uses Circuit Relay V2 for NAT traversal
- Wait 1-2 minutes for relay reservation
- Check logs for "Relay reservation accepted"

**Expected behavior:**
- Private NAT: Uses relay nodes automatically
- Public IP: Direct connections (faster)

### Peer Count Stuck at Zero

**Wait for DHT bootstrap (60-120 seconds):**
```bash
# Monitor peer discovery
watch -n 5 'curl -s http://127.0.0.1:8088/status | jq .peers_online'
```

**Check network connectivity:**
```bash
# Verify internet connection
ping 8.8.8.8

# Check DNS
nslookup bootstrap.libp2p.io
```

**Restart Helper:**
```bash
# Kill and restart
pkill stealth-browser
./stealth-browser > helper.log 2>&1 &
```

---

## ⚡ Performance Issues

### High CPU Usage

**Check what's consuming CPU:**
```powershell
# Windows
Get-Process stealth-browser | Select-Object CPU,WorkingSet

# Linux
top -p $(pgrep stealth-browser)
```

**Common causes:**
- DHT maintenance (normal, peaks every 5-10 min)
- Active connections (each connection uses CPU)
- Relay forwarding (if acting as relay)

**Reduce CPU usage:**
```bash
# Limit connection rate (future feature)
# Current: Automatic rate limiting built-in
```

### High Memory Usage

**Monitor memory:**
```bash
# Linux
ps aux | grep stealth-browser | awk '{print $6/1024 " MB"}'

# Expected: 50-150 MB normal, 200-300 MB under load
```

**If memory keeps growing:**
- Could indicate memory leak (report as bug)
- Restart Helper periodically
- Check for updates

### Network Bandwidth Issues

**Monitor bandwidth:**
```bash
# Linux - Install iftop
sudo iftop -i <interface>

# Windows - Resource Monitor → Network
```

**Typical bandwidth usage:**
- Idle: < 100 KB/s (DHT maintenance)
- Active browsing: Varies by usage
- Relay mode: Up to 1-5 MB/s

---

## 🛠️ Build Errors

### Cargo Build Fails

**Rust version too old:**
```bash
rustup update stable
cargo --version  # Should be 1.70+
```

**Missing dependencies:**
```bash
# Linux
sudo apt-get install build-essential pkg-config libssl-dev

# macOS
xcode-select --install
brew install openssl@3
```

**Linker errors:**
```bash
# Windows - Install Visual Studio Build Tools
# Linux - Install clang/gcc
sudo apt-get install clang llvm

# macOS - Update Xcode
softwareupdate --install -a
```

### Test Failures

**Run specific test:**
```bash
cargo test --package core-mesh --test mesh_discovery -- --nocapture
```

**Common test issues:**
- Timing issues: Re-run tests
- Port conflicts: Kill other processes
- Network flakiness: Check internet

**Skip flaky tests:**
```bash
cargo test --workspace -- --skip flaky_test_name
```

### Clippy Warnings

**Fix common warnings:**
```bash
cargo clippy --workspace --all-targets --fix --allow-dirty
```

**Suppress specific warnings:**
```rust
#[allow(clippy::too_many_arguments)]
fn my_function(...) { }
```

---

## 🖥️ Platform-Specific

### Windows

**Antivirus Blocking:**
- Add exclusion for `stealth-browser.exe`
- Windows Defender: Settings → Virus & threat protection → Exclusions

**PowerShell Execution Policy:**
```powershell
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
```

**Build on Windows:**
```powershell
# Use Visual Studio Developer Command Prompt
# Or install Build Tools
winget install --id Microsoft.VisualStudio.2022.BuildTools
```

### Linux

**Permission Issues:**
```bash
# Don't run as root
# Use setcap for port binding
sudo setcap 'cap_net_bind_service=+ep' ./stealth-browser
```

**systemd Service:**
```bash
# Create service file
sudo nano /etc/systemd/system/qnet-helper.service

[Unit]
Description=QNet Helper
After=network.target

[Service]
Type=simple
User=qnet
ExecStart=/usr/local/bin/stealth-browser
Restart=on-failure

[Install]
WantedBy=multi-user.target

# Enable service
sudo systemctl enable qnet-helper
sudo systemctl start qnet-helper
```

### macOS

**Gatekeeper Warning:**
```bash
# Allow unsigned binary
xattr -d com.apple.quarantine ./stealth-browser

# Or sign the binary (requires Apple Developer account)
```

**Firewall Prompt:**
- Allow incoming connections when prompted
- System Preferences → Security & Privacy → Firewall → Options

---

## 📞 Getting More Help

If your issue isn't covered here:

1. **Check logs:**
   ```bash
   RUST_LOG=debug ./stealth-browser 2>&1 | tee debug.log
   ```

2. **Search existing issues:**
   https://github.com/QW1CKS/qnet/issues

3. **Open a new issue:**
   - Include: OS, QNet version, logs, steps to reproduce
   - Use issue template

4. **Security issues:**
   See [SECURITY.md](../SECURITY.md) for responsible disclosure

5. **Community:**
   https://github.com/QW1CKS/qnet/discussions

---

**Still stuck? We're here to help!** 🚀
