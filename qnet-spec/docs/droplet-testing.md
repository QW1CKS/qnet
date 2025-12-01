# QNet Droplet Deployment Testing

This document provides step-by-step procedures for deploying and testing a QNet super peer on a DigitalOcean droplet. It covers provisioning, deployment, verification, and integration testing with local clients.

---

## Prerequisites

### DigitalOcean Requirements
- Active DigitalOcean account
- SSH key uploaded to DigitalOcean (recommended)
- Budget: ~$6/month for basic droplet

### Local Requirements
- SSH client (Windows Terminal, PuTTY, or WSL)
- `curl` for API testing
- QNet source code cloned locally
- PowerShell 7+ (for Windows testing)

---

## Part 1: Droplet Provisioning

**Purpose**: Create a DigitalOcean droplet suitable for running a QNet super peer.

**Duration**: ~5 minutes

### Steps

1. **Log into DigitalOcean** at https://cloud.digitalocean.com

2. **Create a new Droplet**:
   - Click "Create" → "Droplets"
   - **Region**: Choose closest to your location (e.g., NYC1, SFO3, LON1)
   - **Image**: Ubuntu 22.04 LTS x64
   - **Size**: Basic → Regular → $6/mo (1 vCPU, 1 GB RAM, 25 GB SSD)
   - **Authentication**: SSH Key (recommended) or Password
   - **Hostname**: `qnet-super-1` (or similar)
   - Click "Create Droplet"

3. **Note the droplet's public IP** (e.g., `138.197.176.64`)

4. **Wait for droplet status**: "Active" (usually 30-60 seconds)

### Pass Criteria
- [ ] Droplet created successfully
- [ ] Status shows "Active"
- [ ] Public IP assigned
- [ ] SSH access works: `ssh root@<DROPLET_IP>`

**Record droplet IP**: ________________

---

## Part 2: Automated Deployment

**Purpose**: Deploy QNet super peer using the automated deployment script.

**Duration**: ~10-15 minutes (mostly build time)

### Steps

1. **SSH into the droplet**:
   ```bash
   ssh root@<DROPLET_IP>
   ```

2. **Run the deployment script** (single command):
   ```bash
   curl -sSL https://raw.githubusercontent.com/QW1CKS/qnet/main/scripts/deploy-super-peer.sh | bash
   ```

   **Alternative** (if repo not public or testing local changes):
   ```bash
   # Download script manually
   wget https://raw.githubusercontent.com/QW1CKS/qnet/main/scripts/deploy-super-peer.sh
   chmod +x deploy-super-peer.sh
   ./deploy-super-peer.sh
   ```

3. **Watch the deployment progress**. You should see:
   ```
   [INFO] Step 1/7: Updating system and installing dependencies...
   [SUCCESS] System dependencies installed
   [INFO] Step 2/7: Installing Rust toolchain...
   [SUCCESS] Rust installed: rustc 1.xx.x
   [INFO] Step 3/7: Creating qnet system user...
   [SUCCESS] User 'qnet' created
   [INFO] Step 4/7: Cloning QNet repository...
   [SUCCESS] Repository cloned to /opt/qnet
   [INFO] Step 5/7: Building QNet binary (this may take 5-10 minutes)...
   [SUCCESS] Binary built: /opt/qnet/target/release/stealth-browser
   [INFO] Step 6/7: Creating systemd service...
   [SUCCESS] Systemd service created: qnet-super.service
   [INFO] Step 7/7: Configuring firewall...
   [SUCCESS] Firewall configured
   ```

4. **Note the peer ID** from the summary output or logs:
   ```bash
   journalctl -u qnet-super | grep 'local_peer_id'
   ```

   Example output:
   ```
   local_peer_id=12D3KooWNcvVLoDYXo6oFfJ4ENCcVGE61SPpsXxSc18N165oQqbw
   ```

### Pass Criteria
- [ ] Script completes without errors
- [ ] Service status shows "active (running)"
- [ ] Peer ID is logged
- [ ] Firewall rules configured

**Record peer ID**: ________________

---

## Part 3: Service Verification

**Purpose**: Verify the QNet super peer is running correctly on the droplet.

**Duration**: ~5 minutes

### Steps

1. **Check service status** (on droplet):
   ```bash
   systemctl status qnet-super
   ```

   **Expected output**:
   ```
   ● qnet-super.service - QNet Super Peer (Bootstrap + Relay + Exit)
        Loaded: loaded (/etc/systemd/system/qnet-super.service; enabled)
        Active: active (running) since ...
   ```

2. **View live logs** (on droplet):
   ```bash
   journalctl -u qnet-super -f
   ```

   **Expected logs**:
   ```
   [INFO] stealth-browser stub starting
   [INFO] config loaded port=1088 status_port=8088 mode=Direct helper_mode=Super
   [INFO] helper mode features helper_mode=Super features="all features"
   [INFO] status server listening status_addr=0.0.0.0:8088
   [INFO] starting SOCKS5 server addr=0.0.0.0:1088 mode=Direct
   [INFO] mesh: Listening on /ip4/<PUBLIC_IP>/tcp/4001
   ```

3. **Test status endpoint locally** (on droplet):
   ```bash
   curl -s http://127.0.0.1:8088/status | jq
   ```

   **Expected response**:
   ```json
   {
     "state": "offline",
     "helper_mode": "super",
     "mode": "direct",
     "socks_addr": "0.0.0.0:1088",
     "mesh_enabled": true
   }
   ```

4. **Test ping endpoint** (on droplet):
   ```bash
   curl -s http://127.0.0.1:8088/ping | jq
   ```

   **Expected**: `{ "ok": true, "ts": <timestamp> }`

5. **Verify ports are listening** (on droplet):
   ```bash
   ss -tlnp | grep -E '(8088|1088|4001)'
   ```

   **Expected**:
   ```
   LISTEN  0  128  0.0.0.0:8088  0.0.0.0:*  users:(("stealth-browser"...))
   LISTEN  0  128  0.0.0.0:1088  0.0.0.0:*  users:(("stealth-browser"...))
   LISTEN  0  128  0.0.0.0:4001  0.0.0.0:*  users:(("stealth-browser"...))
   ```

### Pass Criteria
- [ ] Service is active and running
- [ ] `/status` returns JSON with `helper_mode: "super"`
- [ ] `/ping` returns `ok: true`
- [ ] Ports 8088, 1088, 4001 are listening
- [ ] No errors in logs

---

## Part 4: Remote API Testing

**Purpose**: Verify droplet APIs are accessible from the internet.

**Duration**: ~5 minutes

### Steps (from your LOCAL machine, not the droplet)

1. **Test status endpoint remotely** (PowerShell):
   ```powershell
   $dropletIp = "<DROPLET_IP>"  # Replace with actual IP
   Invoke-RestMethod "http://${dropletIp}:8088/status" | ConvertTo-Json -Depth 4
   ```

   **Alternative** (curl):
   ```bash
   curl -s http://<DROPLET_IP>:8088/status | jq
   ```

2. **Test ping endpoint remotely**:
   ```powershell
   Invoke-RestMethod "http://${dropletIp}:8088/ping"
   ```

3. **Test directory endpoint** (should return empty initially):
   ```powershell
   Invoke-RestMethod "http://${dropletIp}:8088/api/relays/by-country" | ConvertTo-Json -Depth 4
   ```

   **Expected**: `{}` (empty object, no peers registered yet)

4. **Register a test peer remotely**:
   ```powershell
   $dropletIp = "<DROPLET_IP>"
   $body = @{
       peer_id = "12D3KooWTestFromLaptop123456789"
       addrs = @("/ip4/192.168.1.100/tcp/4001")
       country = "US"
       capabilities = @("relay")
       last_seen = [int][double]::Parse((Get-Date -UFormat %s))
       first_seen = [int][double]::Parse((Get-Date -UFormat %s))
   } | ConvertTo-Json

   Invoke-RestMethod -Uri "http://${dropletIp}:8088/api/relay/register" `
       -Method POST `
       -ContentType "application/json" `
       -Body $body
   ```

   **Expected**: `{ "registered": true, "is_new": true }`

5. **Verify peer was registered**:
   ```powershell
   Invoke-RestMethod "http://${dropletIp}:8088/api/relays/by-country" | ConvertTo-Json -Depth 4
   ```

   **Expected**: JSON with `US` key containing the test peer.

### Pass Criteria
- [ ] Status endpoint accessible from internet
- [ ] Ping endpoint returns `ok: true`
- [ ] Directory endpoints work (register + query)
- [ ] No firewall blocking access

---

## Part 5: Local Helper Integration

**Purpose**: Verify a local Helper can discover and connect to the droplet super peer.

**Duration**: ~10 minutes

### Prerequisites
- QNet source code on local machine
- Droplet IP and Peer ID noted from Part 2

### Steps

1. **Update hardcoded operator nodes** (local machine):
   
   Edit `apps/stealth-browser/src/main.rs`, find `hardcoded_operator_nodes()` function and update:
   ```rust
   fn hardcoded_operator_nodes() -> Vec<OperatorNode> {
       vec![
           OperatorNode {
               peer_id: "<DROPLET_PEER_ID>".to_string(),
               multiaddr: "/ip4/<DROPLET_IP>/tcp/4001".to_string(),
           },
       ]
   }
   ```

   Replace `<DROPLET_PEER_ID>` and `<DROPLET_IP>` with actual values.

2. **Build local Helper**:
   ```powershell
   cd P:\GITHUB\qnet
   cargo build -p stealth-browser
   ```

3. **Start local Helper in client mode**:
   ```powershell
   $env:STEALTH_MODE = "client"
   $env:RUST_LOG = "info"
   cargo run -p stealth-browser
   ```

4. **Watch logs for discovery**. Expected:
   ```
   [INFO] mesh: Querying operator directory for relay peers
   [INFO] directory: Querying operator node http://<DROPLET_IP>:8088/api/relays/by-country
   [INFO] mesh: Discovered relay peer <PEER_ID> from operator
   [INFO] mesh: Dialing /ip4/<DROPLET_IP>/tcp/4001
   ```

5. **Check local status for mesh connection**:
   ```powershell
   Invoke-RestMethod http://127.0.0.1:8088/status | ConvertTo-Json -Depth 4
   ```

   **Expected**: `mesh_peer_count` should be > 0 after successful connection.

6. **Verify on droplet logs** (SSH to droplet):
   ```bash
   journalctl -u qnet-super -f
   ```

   Look for incoming connection logs.

### Pass Criteria
- [ ] Local Helper queries droplet directory
- [ ] Local Helper dials droplet peer
- [ ] Mesh connection established (peer count > 0)
- [ ] Droplet logs show incoming connection

---

## Part 6: Exit Node Testing

**Purpose**: Verify the droplet exit node correctly forwards HTTPS requests.

**Duration**: ~5 minutes

### Steps (from LOCAL machine)

1. **Test SOCKS5 proxy through droplet** (requires curl):
   ```powershell
   curl.exe -v --socks5-hostname <DROPLET_IP>:1088 https://httpbin.org/ip
   ```

   **Expected output**:
   ```
   * SOCKS5 connect to httpbin.org:443 (remotely resolved)
   * SOCKS5 request granted.
   {
     "origin": "<DROPLET_IP>"
   }
   ```

   The "origin" IP should be the droplet's public IP, proving traffic exits through the droplet.

2. **Alternative: Firefox SOCKS5 test**:
   - Open Firefox Settings → Network Settings → Manual proxy configuration
   - SOCKS Host: `<DROPLET_IP>`, Port: `1088`, SOCKS v5
   - Check "Proxy DNS when using SOCKS v5"
   - Visit https://whatismyipaddress.com/
   - IP should show droplet's IP

3. **Check exit stats on droplet**:
   ```bash
   curl -s http://127.0.0.1:8088/status | jq '.exit_stats'
   ```

   **Expected**: Counters should increment after requests.

### Pass Criteria
- [ ] SOCKS5 proxy accepts external connections
- [ ] Traffic exits from droplet IP
- [ ] Exit stats track requests
- [ ] HTTPS (TLS passthrough) works correctly

---

## Part 7: Heartbeat Integration

**Purpose**: Verify local relay nodes can register with droplet directory via heartbeat.

**Duration**: ~5 minutes

### Steps

1. **Start local Helper in relay mode**:
   ```powershell
   $env:STEALTH_MODE = "relay"
   $env:RUST_LOG = "debug"
   cargo run -p stealth-browser
   ```

2. **Watch logs for heartbeat**:
   ```
   [INFO] heartbeat: Sending registration to operator nodes
   [DEBUG] heartbeat: POST http://<DROPLET_IP>:8088/api/relay/register
   [INFO] heartbeat: Registration successful
   ```

3. **Query droplet directory** (after 30+ seconds):
   ```powershell
   Invoke-RestMethod "http://${dropletIp}:8088/api/relays/by-country" | ConvertTo-Json -Depth 4
   ```

   **Expected**: Local relay peer should appear in directory.

4. **Stop local relay** and wait 2+ minutes.

5. **Query directory again** - peer should be pruned:
   ```powershell
   Invoke-RestMethod "http://${dropletIp}:8088/api/relays/by-country" | ConvertTo-Json -Depth 4
   ```

### Pass Criteria
- [ ] Relay sends heartbeat to droplet
- [ ] Droplet directory shows relay peer
- [ ] Peer is pruned after relay stops

---

## Part 8: Load & Stability Testing

**Purpose**: Verify droplet handles concurrent requests and remains stable.

**Duration**: ~10 minutes

### Steps

1. **Run concurrent registrations from local machine**:
   ```powershell
   $dropletIp = "<DROPLET_IP>"
   
   $jobs = 1..50 | ForEach-Object {
       Start-Job -ScriptBlock {
           param($id, $ip)
           $body = @{
               peer_id = "12D3KooWLoadTest$($id.ToString('D4'))"
               addrs = @("/ip4/10.0.0.$($id % 256)/tcp/4001")
               country = @("US", "FR", "DE", "JP", "AU")[$id % 5]
               capabilities = @("relay")
               last_seen = [int][double]::Parse((Get-Date -UFormat %s))
               first_seen = [int][double]::Parse((Get-Date -UFormat %s))
           } | ConvertTo-Json
           
           try {
               Invoke-RestMethod -Uri "http://${ip}:8088/api/relay/register" `
                   -Method POST -ContentType "application/json" -Body $body -TimeoutSec 10
               return "OK"
           } catch {
               return "FAIL: $_"
           }
       } -ArgumentList $_, $dropletIp
   }

   $results = $jobs | Wait-Job | Receive-Job
   $jobs | Remove-Job
   
   $success = ($results | Where-Object { $_ -eq "OK" }).Count
   Write-Host "Registrations: $success/50 succeeded"
   ```

2. **Run concurrent queries**:
   ```powershell
   $queryJobs = 1..100 | ForEach-Object {
       Start-Job -ScriptBlock {
           param($ip)
           try {
               Invoke-RestMethod "http://${ip}:8088/api/relays/by-country" -TimeoutSec 10 | Out-Null
               return "OK"
           } catch {
               return "FAIL"
           }
       } -ArgumentList $dropletIp
   }

   $queryResults = $queryJobs | Wait-Job | Receive-Job
   $queryJobs | Remove-Job
   
   $querySuccess = ($queryResults | Where-Object { $_ -eq "OK" }).Count
   Write-Host "Queries: $querySuccess/100 succeeded"
   ```

3. **Check droplet resource usage** (SSH):
   ```bash
   htop  # or: top -bn1 | head -20
   free -h
   df -h
   ```

4. **Check service still running**:
   ```bash
   systemctl status qnet-super
   ```

### Pass Criteria
- [ ] 50/50 concurrent registrations succeed (or >90%)
- [ ] 100/100 concurrent queries succeed (or >95%)
- [ ] Memory usage stable (<80% of 1GB)
- [ ] Service remains active after load test

---

## Part 9: Graceful Operations

**Purpose**: Verify service restart, stop, and recovery work correctly.

**Duration**: ~5 minutes

### Steps

1. **Stop the service** (on droplet):
   ```bash
   systemctl stop qnet-super
   ```

2. **Verify service stopped**:
   ```bash
   systemctl status qnet-super
   curl -s http://127.0.0.1:8088/status  # Should fail
   ```

3. **Start the service**:
   ```bash
   systemctl start qnet-super
   ```

4. **Verify service recovered**:
   ```bash
   systemctl status qnet-super
   curl -s http://127.0.0.1:8088/status | jq  # Should work
   ```

5. **Test restart**:
   ```bash
   systemctl restart qnet-super
   sleep 5
   curl -s http://127.0.0.1:8088/status | jq
   ```

6. **Test automatic restart** (kill process):
   ```bash
   pkill -9 stealth-browser
   sleep 15  # Wait for systemd to restart
   systemctl status qnet-super  # Should be active
   ```

### Pass Criteria
- [ ] Stop cleanly releases ports
- [ ] Start brings service back online
- [ ] Restart maintains functionality
- [ ] Auto-restart works after crash (killed process)

---

## Troubleshooting

### Service won't start
```bash
# Check logs for errors
journalctl -u qnet-super -n 50 --no-pager

# Check if ports are in use
ss -tlnp | grep -E '(8088|1088|4001)'

# Verify binary exists
ls -la /opt/qnet/target/release/stealth-browser
```

### Cannot connect from internet
```bash
# Check firewall rules
ufw status verbose

# Verify service is binding to 0.0.0.0 (not 127.0.0.1)
ss -tlnp | grep stealth
```

### Build fails
```bash
# Check Rust is installed
source ~/.cargo/env
rustc --version

# Check dependencies
apt install build-essential pkg-config libssl-dev

# Retry build with verbose output
cd /opt/qnet
cargo build --release -p stealth-browser -v
```

### Out of memory during build
```bash
# Add swap space (temporary)
fallocate -l 2G /swapfile
chmod 600 /swapfile
mkswap /swapfile
swapon /swapfile

# Retry build
cargo build --release -p stealth-browser

# Remove swap after
swapoff /swapfile
rm /swapfile
```

---

## Test Results Template

```markdown
## Droplet Deployment Test Run: [DATE]

### Environment
- Droplet Region: [NYC1/SFO3/etc.]
- Droplet Size: $6/mo (1 vCPU, 1 GB RAM)
- Droplet IP: [IP ADDRESS]
- Peer ID: [12D3KooW...]
- QNet Branch: main
- Local OS: Windows 11

### Results

| Test | Status | Notes |
|------|--------|-------|
| Part 1: Provisioning | ✅/❌ | |
| Part 2: Deployment | ✅/❌ | Build time: X min |
| Part 3: Service Verification | ✅/❌ | |
| Part 4: Remote API Testing | ✅/❌ | |
| Part 5: Local Helper Integration | ✅/❌ | |
| Part 6: Exit Node Testing | ✅/❌ | |
| Part 7: Heartbeat Integration | ✅/❌ | |
| Part 8: Load Testing | ✅/❌ | Reg: X/50, Query: X/100 |
| Part 9: Graceful Operations | ✅/❌ | |

### Performance Metrics
- Build time: ___ minutes
- Memory usage (idle): ___ MB
- Memory usage (under load): ___ MB
- Average response time: ___ ms

### Issues Found
- [Issue description]

### Sign-off
- Tester: [NAME]
- Date: [DATE]
```

---

## Cleanup

When done testing, you can:

1. **Destroy the droplet** (DigitalOcean console) - stops billing
2. **Or keep running** as your operator node

**Note**: Droplet billing is hourly, so destroying when not testing saves money.

---

## Related Documentation

- [Manual Testing (Local)](manual-testing.md) - Local testing procedures
- [Helper Documentation](helper.md) - API reference
- [Deployment Script](../../scripts/deploy-super-peer.sh) - Automated deployment
- [Architecture](../../docs/ARCHITECTURE.md) - System design
