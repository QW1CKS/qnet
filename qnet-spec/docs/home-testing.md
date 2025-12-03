# QNet Home Super Peer Testing Guide

This guide provides step-by-step instructions for running a QNet super peer on a home laptop/desktop before deploying to a cloud droplet. Testing locally helps validate the setup without incurring cloud costs.

---

## Prerequisites

### Hardware Requirements
- Windows 10/11 laptop or desktop
- Minimum 8GB RAM (16GB recommended for comfortable development)
- 10GB free disk space
- Wired Ethernet connection recommended (more stable than WiFi)

### Software Requirements
- Rust toolchain installed (`rustup` with stable channel)
- PowerShell 7+ or Windows Terminal
- Git for Windows
- QNet source code cloned locally

### Network Requirements
- Home router with admin access
- Public IP address (not behind CGNAT - see verification below)
- Ability to configure port forwarding on router

---

## Part 1: Network Environment Check

### 1.1 Check for CGNAT (Carrier-Grade NAT)

CGNAT means your ISP shares one public IP among multiple customers. If you're behind CGNAT, port forwarding won't work.

**Step 1: Find your router's WAN IP**
1. Log into your router admin panel (usually `192.168.1.1` or `192.168.0.1`)
2. Find the WAN/Internet status page
3. Note the "WAN IP" or "External IP" address

**Step 2: Compare with your actual public IP**
```powershell
# Get your public IP
Invoke-RestMethod https://ifconfig.me
```

**Result interpretation:**
- ✅ **IPs match** → You have a real public IP, proceed with guide
- ❌ **IPs don't match** → You're behind CGNAT, options:
  - Contact ISP to request a real public IP (may cost extra)
  - Use a VPS/droplet instead (skip to `droplet-testing.md`)
  - Use a tunneling service like ngrok or Cloudflare Tunnel (advanced)

### 1.2 Check for Double NAT

Double NAT occurs when you have two routers (e.g., ISP modem-router + your own router).

**Symptoms of Double NAT:**
- Your router's WAN IP starts with `10.x.x.x`, `172.16-31.x.x`, or `192.168.x.x`
- Port forwarding doesn't work even though configured correctly

**Solutions:**
1. **Bridge mode**: Put ISP modem into bridge/passthrough mode
2. **DMZ**: Set your router as DMZ host on ISP modem
3. **Single router**: Use only one router (ISP modem in bridge mode)

### 1.3 Note Your Network Information

Record these values (you'll need them later):

| Item | Value |
|------|-------|
| Router admin URL | `http://192.168.___.___ ` |
| Router admin username | ________________ |
| Laptop local IP | ________________ |
| Router WAN/Public IP | ________________ |
| Public IP (ifconfig.me) | ________________ |

To find your laptop's local IP:
```powershell
# Get local IP address
Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.InterfaceAlias -notlike "*Loopback*" -and $_.PrefixOrigin -eq "Dhcp" } | Select-Object IPAddress, InterfaceAlias
```

---

## Part 2: Router Port Forwarding Configuration

### 2.1 Required Ports

QNet super peer requires three ports to be forwarded:

| Port | Protocol | Service | Description |
|------|----------|---------|-------------|
| 1088 | TCP | SOCKS5 Proxy | Client proxy connections |
| 8088 | TCP | HTTP Status API | Status page, directory API |
| 4001 | TCP | libp2p | Peer-to-peer mesh connections |

### 2.2 Assign Static/Reserved IP to Laptop

Port forwarding requires a stable local IP. Configure DHCP reservation:

1. Log into router admin panel
2. Find **DHCP** or **LAN Settings** section
3. Find **DHCP Reservation** or **Address Reservation**
4. Add reservation:
   - MAC Address: Your laptop's MAC (find with `Get-NetAdapter | Select-Object Name, MacAddress`)
   - IP Address: Choose an IP outside DHCP pool (e.g., `192.168.1.100`)
5. Save and reboot laptop to get the reserved IP

### 2.3 Configure Port Forwarding Rules

Access your router's **Port Forwarding**, **Virtual Server**, or **NAT** section.

Add three rules:

**Rule 1: SOCKS5 Proxy**
| Field | Value |
|-------|-------|
| Name/Description | QNet SOCKS5 |
| External Port | 1088 |
| Internal Port | 1088 |
| Internal IP | Your laptop's reserved IP |
| Protocol | TCP |

**Rule 2: Status API**
| Field | Value |
|-------|-------|
| Name/Description | QNet Status |
| External Port | 8088 |
| Internal Port | 8088 |
| Internal IP | Your laptop's reserved IP |
| Protocol | TCP |

**Rule 3: libp2p Mesh**
| Field | Value |
|-------|-------|
| Name/Description | QNet P2P |
| External Port | 4001 |
| Internal Port | 4001 |
| Internal IP | Your laptop's reserved IP |
| Protocol | TCP |

Save all rules and apply changes.

### 2.4 Router-Specific Instructions

<details>
<summary><b>ASUS Routers</b></summary>

1. Go to **WAN** → **Virtual Server / Port Forwarding**
2. Enable **Port Forwarding**
3. Add each rule with:
   - Service Name: `QNet-SOCKS5`, `QNet-Status`, `QNet-P2P`
   - Port Range: `1088`, `8088`, `4001`
   - Local IP: Your laptop IP
   - Protocol: `TCP`
4. Click **Apply**
</details>

<details>
<summary><b>TP-Link Routers</b></summary>

1. Go to **Advanced** → **NAT Forwarding** → **Virtual Servers**
2. Click **Add**
3. For each port:
   - Service Type: Custom
   - External Port: 1088/8088/4001
   - Internal IP: Your laptop IP
   - Internal Port: Same as external
   - Protocol: TCP
4. Save each rule
</details>

<details>
<summary><b>Netgear Routers</b></summary>

1. Go to **Advanced** → **Advanced Setup** → **Port Forwarding / Port Triggering**
2. Select **Port Forwarding**
3. Click **Add Custom Service**
4. For each port:
   - Service Name: QNet-SOCKS5/Status/P2P
   - Protocol: TCP
   - External Port Range: 1088/8088/4001
   - Internal IP: Your laptop IP
   - Internal Port Range: Same
5. Apply changes
</details>

<details>
<summary><b>Linksys Routers</b></summary>

1. Go to **Apps and Gaming** → **Single Port Forwarding**
2. Add entries:
   - Application Name: QNet-SOCKS5/Status/P2P
   - External Port: 1088/8088/4001
   - Internal Port: 1088/8088/4001
   - Protocol: TCP
   - Device IP: Your laptop IP
   - Enabled: ✓
3. Save Settings
</details>

<details>
<summary><b>ISP-Provided Routers (Generic)</b></summary>

Look for these menu items:
- **Port Forwarding**
- **Virtual Server**
- **NAT/Gaming**
- **Applications & Gaming**
- **Firewall** → **Port Forwarding**

If you can't find it, search "[Your ISP name] [Router model] port forwarding" online.
</details>

---

## Part 3: Windows Firewall Configuration

### 3.1 Create Firewall Rules

Run PowerShell as Administrator:

```powershell
# Create inbound rules for QNet
New-NetFirewallRule -DisplayName "QNet SOCKS5" -Direction Inbound -Protocol TCP -LocalPort 1088 -Action Allow
New-NetFirewallRule -DisplayName "QNet Status API" -Direction Inbound -Protocol TCP -LocalPort 8088 -Action Allow
New-NetFirewallRule -DisplayName "QNet libp2p" -Direction Inbound -Protocol TCP -LocalPort 4001 -Action Allow

# Verify rules were created
Get-NetFirewallRule -DisplayName "QNet*" | Format-Table DisplayName, Enabled, Direction, Action
```

### 3.2 Allow stealth-browser Through Firewall

When you first run stealth-browser, Windows may prompt to allow network access. Click **Allow** for both private and public networks.

If you missed the prompt:
```powershell
# Allow the debug binary
New-NetFirewallRule -DisplayName "QNet stealth-browser (Debug)" -Direction Inbound -Program "P:\GITHUB\qnet\target\debug\stealth-browser.exe" -Action Allow

# Allow the release binary (when built)
New-NetFirewallRule -DisplayName "QNet stealth-browser (Release)" -Direction Inbound -Program "P:\GITHUB\qnet\target\release\stealth-browser.exe" -Action Allow
```

---

## Part 4: Verify Port Forwarding

### 4.1 Start Super Peer Temporarily

```powershell
cd P:\GITHUB\qnet
$env:RUST_LOG = "info"
cargo run -p stealth-browser -- --helper-mode super
```

Wait for startup messages:
```
[INFO] status server listening status_addr=0.0.0.0:8088
[INFO] starting SOCKS5 server addr=0.0.0.0:1088 mode=Direct
[INFO] mesh: Listening on /ip4/0.0.0.0/tcp/4001
```

### 4.2 Test from External Source

**Option A: Use a port checking website**
1. Open browser on your phone (using mobile data, NOT WiFi)
2. Visit: `https://www.yougetsignal.com/tools/open-ports/`
3. Enter your public IP and port 8088
4. Click "Check" - should show **Open**

**Option B: Test from another network**
```bash
# From a friend's computer, VPS, or mobile hotspot:
curl http://<YOUR_PUBLIC_IP>:8088/ping
```

Expected response: `{"ok":true,"ts":...}`

**Option C: Use online curl service**
Visit: `https://reqbin.com/curl`
Enter: `curl http://<YOUR_PUBLIC_IP>:8088/status`

### 4.3 Troubleshooting Port Forwarding

| Symptom | Possible Cause | Solution |
|---------|---------------|----------|
| Port shows "Closed" | Firewall blocking | Check Windows Firewall rules |
| Port shows "Closed" | Wrong internal IP | Verify DHCP reservation |
| Port shows "Closed" | Router not applying | Reboot router |
| Port shows "Filtered" | ISP blocking port | Try alternate ports (e.g., 8080, 8443) |
| Connection refused | Service not running | Start stealth-browser first |
| Timeout | CGNAT | Contact ISP or use VPS |

---

## Part 5: Update Hardcoded Operator Nodes

For other clients to connect to your home super peer, update the bootstrap nodes:

### 5.1 Edit discovery.rs

Open `crates/core-mesh/src/discovery.rs` and find `hardcoded_operator_nodes()`:

```rust
pub fn hardcoded_operator_nodes() -> Vec<OperatorNode> {
    vec![
        OperatorNode {
            peer_id: "<YOUR_PEER_ID>".to_string(),
            multiaddr: "/ip4/<YOUR_PUBLIC_IP>/tcp/4001".to_string(),
        },
    ]
}
```

Replace:
- `<YOUR_PEER_ID>`: Found in startup logs (`local_peer_id=12D3KooW...`)
- `<YOUR_PUBLIC_IP>`: Your public IP from `Invoke-RestMethod https://ifconfig.me`

### 5.2 Generate Persistent Keypair (Optional)

For a stable peer ID across restarts:

```powershell
# Generate keypair file
cargo run -p stealth-browser -- --generate-keypair P:\GITHUB\qnet\data\keypair.pb

# Run with persistent identity
$env:QNET_KEYPAIR_PATH = "P:\GITHUB\qnet\data\keypair.pb"
cargo run -p stealth-browser -- --helper-mode super
```

---

## Part 6: Running the Home Super Peer

### 6.1 Start Super Peer

```powershell
cd P:\GITHUB\qnet

# Set environment
$env:RUST_LOG = "info"
$env:QNET_KEYPAIR_PATH = "P:\GITHUB\qnet\data\keypair.pb"  # Optional: persistent ID

# Run super peer
cargo run -p stealth-browser -- --helper-mode super
```

### 6.2 Verify Status Page

Open browser: `http://127.0.0.1:8088/`

You should see:
- Mode: **Super**
- State: **Connecting...** (yellow) → **Connected** (green)
- Mesh enabled: **Yes**

### 6.3 Monitor Logs

Watch for:
```
[INFO] helper mode features helper_mode=Super features="all features"
[INFO] mesh: Listening on /ip4/<YOUR_IP>/tcp/4001
[INFO] mesh: local_peer_id=12D3KooW...
```

---

## Part 7: Testing Client Connections

### 7.1 Test from Second Device on Same Network

On another computer (same WiFi/LAN):

```powershell
# Clone repo and update discovery.rs with your laptop's LOCAL IP for LAN testing
# Then run as client:
cargo run -p stealth-browser -- --socks-port 1089 --status-port 8089
```

### 7.2 Test from External Network

From a phone on mobile data or another location:

1. Ensure your laptop's super peer is running
2. Update discovery.rs with your PUBLIC IP
3. Build and run client on external device
4. Verify connection in logs

### 7.3 Test Exit Node

From external client:
```powershell
# Test SOCKS5 exit through your home super peer
curl.exe --socks5-hostname <YOUR_PUBLIC_IP>:1088 https://httpbin.org/ip
```

Should return your home public IP.

---

## Part 8: Running 24/7 (Optional)

### 8.1 Create Scheduled Task for Auto-Start

Run PowerShell as Administrator:

```powershell
$action = New-ScheduledTaskAction -Execute "P:\GITHUB\qnet\target\release\stealth-browser.exe" -Argument "--helper-mode super"
$trigger = New-ScheduledTaskTrigger -AtStartup
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -StartWhenAvailable
$principal = New-ScheduledTaskPrincipal -UserId "$env:USERNAME" -LogonType Interactive -RunLevel Highest

Register-ScheduledTask -TaskName "QNet Super Peer" -Action $action -Trigger $trigger -Settings $settings -Principal $principal
```

### 8.2 Build Release Binary First

```powershell
cargo build --release -p stealth-browser
```

### 8.3 Disable Sleep/Hibernate

For 24/7 operation:
1. Open **Power Options** (search in Start menu)
2. Click **Change plan settings** for current plan
3. Set "Put the computer to sleep" to **Never**
4. Click **Change advanced power settings**
5. Expand **Sleep** → Set everything to **Never** or **0**

### 8.4 Keep Network Connection Active

Some routers disconnect idle connections. Options:
- Enable "Keep WiFi on during sleep" in adapter settings
- Use Ethernet instead of WiFi
- The keepalive ping (every 30s) should prevent idle disconnects

---

## Part 9: Limitations of Home Super Peer

| Limitation | Impact | Mitigation |
|------------|--------|------------|
| Dynamic IP | Public IP may change | Use DDNS service (No-IP, DuckDNS) |
| Uptime | Laptop may restart, lose power | Use UPS, disable updates during testing |
| Bandwidth | Home upload speeds often limited | Monitor with `netstat` |
| ISP TOS | Some ISPs prohibit servers | Check ISP terms, use low bandwidth |
| NAT/Firewall | Complex setup | Follow this guide carefully |
| Security | Home network exposed | Use firewall, monitor logs |

---

## Troubleshooting

### Connection Issues

**Clients can't connect to your super peer:**
1. Verify super peer is running: `curl http://127.0.0.1:8088/ping`
2. Check port forwarding: Use external port checker
3. Check firewall: Temporarily disable Windows Firewall to test
4. Check router logs for blocked connections

**Super peer shows 0 connected peers:**
1. No clients connecting yet (normal if just started)
2. Check if port 4001 is forwarded correctly
3. Verify peer ID in discovery.rs matches your node

### Network Issues

**"Connection refused" errors:**
- Service not running on that port
- Firewall blocking the connection

**"Connection timeout" errors:**
- Port forwarding not configured
- CGNAT blocking incoming connections
- ISP blocking the port

**IP address changed:**
1. Check new IP: `Invoke-RestMethod https://ifconfig.me`
2. Update discovery.rs with new IP
3. Rebuild and restart clients
4. Consider DDNS for automatic updates

---

## Next Steps

After successful home testing:

1. **Validate stability**: Run for 24-48 hours, monitor logs
2. **Test reconnection**: Stop/start super peer, verify clients reconnect
3. **Load test**: Connect multiple clients simultaneously
4. **Deploy to droplet**: Follow `droplet-testing.md` for cloud deployment

---

## Quick Reference

| Command | Purpose |
|---------|---------|
| `cargo run -p stealth-browser -- --helper-mode super` | Start super peer |
| `cargo run -p stealth-browser -- --socks-port 1089 --status-port 8089` | Start second instance |
| `Invoke-RestMethod http://127.0.0.1:8088/status` | Check local status |
| `Invoke-RestMethod https://ifconfig.me` | Get public IP |
| `Get-NetFirewallRule -DisplayName "QNet*"` | Check firewall rules |
| `netstat -an \| findstr "1088 8088 4001"` | Check listening ports |
