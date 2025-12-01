# QNet Manual Testing Procedures

This document provides step-by-step manual testing procedures for QNet Helper functionality that cannot be fully automated in unit tests. These tests verify real-world behavior of running processes, network communication, and timing-dependent features.

---

## Prerequisites

### System Requirements
- Windows 10/11 with PowerShell 7+ (`pwsh`)
- Rust toolchain (1.70+)
- Two terminal windows (for multi-process tests)
- `curl` available in PATH (or use `Invoke-WebRequest`)

### Build the Helper
```powershell
cd P:\GITHUB\qnet
cargo build --release -p stealth-browser
```

The Helper binary will be at: `target\release\stealth-browser.exe`

### Environment Variables Reference
| Variable | Default | Description |
|----------|---------|-------------|
| `STEALTH_MODE` | `client` | Helper mode: client, relay, bootstrap, exit, super |
| `STEALTH_SOCKS_PORT` | `1088` | SOCKS5 proxy port |
| `STEALTH_STATUS_PORT` | `8088` | Status/API HTTP port |
| `EXIT_ABUSE_EMAIL` | (none) | Required for exit/super modes |
| `RUST_LOG` | `info` | Log level: error, warn, info, debug, trace |

---

## Test 1: Super Peer Mode Startup

**Purpose**: Verify Helper starts correctly in super mode with all features enabled.

**Duration**: ~30 seconds

### Steps

1. **Open Terminal 1** and start Helper in super mode:
   ```powershell
   $env:STEALTH_MODE = "super"
   $env:RUST_LOG = "info"
   cargo run -p stealth-browser
   ```

2. **Wait for startup** (~5-10 seconds). Look for log messages:
   ```
   [INFO] Helper starting in mode: super
   [INFO] Directory service enabled (bootstrap feature)
   [INFO] Exit node enabled
   [INFO] Status server listening on 127.0.0.1:8088
   [INFO] SOCKS5 proxy listening on 127.0.0.1:1088
   ```

3. **Verify status endpoint** (in a new terminal):
   ```powershell
   Invoke-RestMethod http://127.0.0.1:8088/status | ConvertTo-Json -Depth 4
   ```

   **Expected response** includes:
   ```json
   {
     "state": "offline" | "calibrating" | "connected",
     "mode": "super",
     "socks_addr": "127.0.0.1:1088"
   }
   ```

4. **Verify ping endpoint**:
   ```powershell
   Invoke-RestMethod http://127.0.0.1:8088/ping
   ```

   **Expected**: `{ "ok": true, "ts": <timestamp> }`

5. **Stop Helper**: Press `Ctrl+C` in Terminal 1.

### Pass Criteria
- [ ] Helper starts without errors
- [ ] `/status` returns JSON with `mode: "super"`
- [ ] `/ping` returns `ok: true`
- [ ] No crash on shutdown

---

## Test 2: Directory Service Endpoints

**Purpose**: Verify directory HTTP endpoints accept registrations and return peer lists.

**Duration**: ~2 minutes

### Steps

1. **Start Helper in bootstrap or super mode** (Terminal 1):
   ```powershell
   $env:STEALTH_MODE = "bootstrap"
   cargo run -p stealth-browser
   ```

2. **Test POST /api/relay/register** (Terminal 2):
   ```powershell
   $body = @{
       peer_id = "12D3KooWTestPeer1234567890abcdef"
       addrs = @("/ip4/192.168.1.100/tcp/4001")
       country = "US"
       capabilities = @("relay")
       last_seen = [int][double]::Parse((Get-Date -UFormat %s))
       first_seen = [int][double]::Parse((Get-Date -UFormat %s))
   } | ConvertTo-Json

   Invoke-RestMethod -Uri "http://127.0.0.1:8088/api/relay/register" `
       -Method POST `
       -ContentType "application/json" `
       -Body $body
   ```

   **Expected response**:
   ```json
   { "registered": true, "is_new": true }
   ```

3. **Register a second peer** (different country):
   ```powershell
   $body2 = @{
       peer_id = "12D3KooWTestPeer2222222222222222"
       addrs = @("/ip4/10.0.0.50/tcp/4001")
       country = "FR"
       capabilities = @("relay")
       last_seen = [int][double]::Parse((Get-Date -UFormat %s))
       first_seen = [int][double]::Parse((Get-Date -UFormat %s))
   } | ConvertTo-Json

   Invoke-RestMethod -Uri "http://127.0.0.1:8088/api/relay/register" `
       -Method POST `
       -ContentType "application/json" `
       -Body $body2
   ```

4. **Test GET /api/relays/by-country** (all countries):
   ```powershell
   Invoke-RestMethod http://127.0.0.1:8088/api/relays/by-country | ConvertTo-Json -Depth 4
   ```

   **Expected**: JSON object with `US` and `FR` keys, each containing array of relay info.

5. **Test GET /api/relays/by-country?country=US** (filtered):
   ```powershell
   Invoke-RestMethod "http://127.0.0.1:8088/api/relays/by-country?country=US" | ConvertTo-Json -Depth 4
   ```

   **Expected**: Only `US` key with the first peer.

6. **Test updating existing peer** (re-register with new timestamp):
   ```powershell
   $bodyUpdate = @{
       peer_id = "12D3KooWTestPeer1234567890abcdef"
       addrs = @("/ip4/192.168.1.100/tcp/4001", "/ip4/192.168.1.100/udp/4001/quic")
       country = "US"
       capabilities = @("relay", "exit")
       last_seen = [int][double]::Parse((Get-Date -UFormat %s))
       first_seen = [int][double]::Parse((Get-Date -UFormat %s))
   } | ConvertTo-Json

   Invoke-RestMethod -Uri "http://127.0.0.1:8088/api/relay/register" `
       -Method POST `
       -ContentType "application/json" `
       -Body $bodyUpdate
   ```

   **Expected**: `{ "registered": true, "is_new": false }`

7. **Stop Helper**: Press `Ctrl+C`.

### Pass Criteria
- [ ] POST /api/relay/register accepts valid JSON and returns `registered: true`
- [ ] First registration returns `is_new: true`
- [ ] Update registration returns `is_new: false`
- [ ] GET /api/relays/by-country returns all registered peers
- [ ] Country filter parameter works correctly

---

## Test 3: Directory Pruning (Stale Peer Removal)

**Purpose**: Verify stale peers are automatically removed after 120 seconds without heartbeat.

**Duration**: ~3 minutes

### Steps

1. **Start Helper in bootstrap mode** (Terminal 1):
   ```powershell
   $env:STEALTH_MODE = "bootstrap"
   $env:RUST_LOG = "debug"  # See pruning logs
   cargo run -p stealth-browser
   ```

2. **Register a peer with OLD timestamp** (Terminal 2):
   ```powershell
   # Timestamp from 5 minutes ago (will be immediately stale)
   $oldTimestamp = [int][double]::Parse((Get-Date).AddMinutes(-5).ToString("yyyy-MM-ddTHH:mm:ss") | Get-Date -UFormat %s)
   
   $stalePeer = @{
       peer_id = "12D3KooWStalePeerToBeRemoved12345"
       addrs = @("/ip4/10.99.99.99/tcp/4001")
       country = "XX"
       capabilities = @("relay")
       last_seen = $oldTimestamp
       first_seen = $oldTimestamp
   } | ConvertTo-Json

   Invoke-RestMethod -Uri "http://127.0.0.1:8088/api/relay/register" `
       -Method POST `
       -ContentType "application/json" `
       -Body $stalePeer
   ```

3. **Verify peer was registered**:
   ```powershell
   Invoke-RestMethod http://127.0.0.1:8088/api/relays/by-country | ConvertTo-Json -Depth 4
   ```

   **Expected**: Peer with country `XX` should be visible.

4. **Wait for automatic pruning** (pruning runs every 60 seconds):
   ```powershell
   Write-Host "Waiting 70 seconds for automatic pruning cycle..."
   Start-Sleep -Seconds 70
   ```

5. **Verify peer was pruned**:
   ```powershell
   Invoke-RestMethod http://127.0.0.1:8088/api/relays/by-country | ConvertTo-Json -Depth 4
   ```

   **Expected**: Peer with country `XX` should be GONE.

6. **Alternative: Manual prune trigger** (dev endpoint):
   ```powershell
   Invoke-RestMethod http://127.0.0.1:8088/api/relays/prune
   ```

   **Expected**: `{ "pruned": <count> }`

7. **Stop Helper**: Press `Ctrl+C`.

### Pass Criteria
- [ ] Peer with old timestamp is registered successfully
- [ ] After pruning cycle (60s), peer with `last_seen` > 120s ago is removed
- [ ] Manual prune endpoint returns count of pruned peers
- [ ] Fresh peers (recent `last_seen`) are NOT pruned

---

## Test 4: Heartbeat Registration (Relay Mode)

**Purpose**: Verify relay nodes send heartbeat registration every 30 seconds.

**Duration**: ~2 minutes

### Steps

1. **Start Helper in bootstrap mode** (Terminal 1) - acts as directory server:
   ```powershell
   $env:STEALTH_MODE = "bootstrap"
   $env:STEALTH_STATUS_PORT = "8088"
   cargo run -p stealth-browser
   ```

2. **Start second Helper in relay mode** (Terminal 2):
   ```powershell
   # Note: In production, relay would point to real operator nodes
   # For testing, we need to modify hardcoded_operator_nodes() or use env override
   $env:STEALTH_MODE = "relay"
   $env:STEALTH_STATUS_PORT = "8089"  # Different port to avoid conflict
   $env:STEALTH_SOCKS_PORT = "1089"
   $env:RUST_LOG = "debug"
   cargo run -p stealth-browser
   ```

3. **Watch Terminal 1 for incoming registrations**. Look for:
   ```
   [INFO] Received relay registration from peer: 12D3KooW...
   ```

4. **Query directory after 30+ seconds** (Terminal 3):
   ```powershell
   Invoke-RestMethod http://127.0.0.1:8088/api/relays/by-country | ConvertTo-Json -Depth 4
   ```

   **Expected**: The relay peer should appear in the directory.

5. **Wait another 30 seconds** and query again:
   ```powershell
   Start-Sleep -Seconds 35
   Invoke-RestMethod http://127.0.0.1:8088/api/relays/by-country | ConvertTo-Json -Depth 4
   ```

   **Expected**: Same peer with updated `last_seen` timestamp.

6. **Stop relay Helper** (Terminal 2): Press `Ctrl+C`.

7. **Wait 2+ minutes** for peer to become stale and get pruned.

8. **Query directory again**:
   ```powershell
   Start-Sleep -Seconds 130
   Invoke-RestMethod http://127.0.0.1:8088/api/relays/by-country | ConvertTo-Json -Depth 4
   ```

   **Expected**: Relay peer should be GONE (pruned due to no heartbeat).

9. **Stop bootstrap Helper**: Press `Ctrl+C`.

### Pass Criteria
- [ ] Relay mode sends POST /api/relay/register on startup
- [ ] Heartbeat repeats every ~30 seconds (check logs)
- [ ] Directory shows peer with updating `last_seen`
- [ ] After relay stops, peer is eventually pruned (120s TTL)

---

## Test 5: All Helper Modes Verification

**Purpose**: Verify each Helper mode starts correctly and enables appropriate features.

**Duration**: ~5 minutes (1 minute per mode)

### Steps

For each mode, run and verify:

#### Client Mode (default)
```powershell
$env:STEALTH_MODE = "client"
cargo run -p stealth-browser
```

**Verify**:
- [ ] Status shows `mode: "client"`
- [ ] No directory endpoints (404 on `/api/relay/register`)
- [ ] No heartbeat logs ("Sending heartbeat" NOT in logs)

#### Relay Mode
```powershell
$env:STEALTH_MODE = "relay"
cargo run -p stealth-browser
```

**Verify**:
- [ ] Status shows `mode: "relay"`
- [ ] No directory endpoints (404 on `/api/relay/register`)
- [ ] Heartbeat logs appear ("Sending heartbeat to operator...")

#### Bootstrap Mode
```powershell
$env:STEALTH_MODE = "bootstrap"
cargo run -p stealth-browser
```

**Verify**:
- [ ] Status shows `mode: "bootstrap"`
- [ ] Directory endpoints work (200 on `/api/relay/register`)
- [ ] No heartbeat logs
- [ ] No exit functionality

#### Exit Mode
```powershell
$env:STEALTH_MODE = "exit"
cargo run -p stealth-browser
```

**Verify**:
- [ ] Status shows `mode: "exit"`
- [ ] No directory endpoints (404 on `/api/relay/register`)
- [ ] Heartbeat logs appear
- [ ] Exit node logs indicate readiness

#### Super Mode
```powershell
$env:STEALTH_MODE = "super"
cargo run -p stealth-browser
```

**Verify**:
- [ ] Status shows `mode: "super"`
- [ ] Directory endpoints work (200 on `/api/relay/register`)
- [ ] Heartbeat logs appear
- [ ] Exit node logs indicate readiness

### Quick Verification Script

```powershell
function Test-HelperMode {
    param([string]$Mode)
    
    Write-Host "`n=== Testing mode: $Mode ===" -ForegroundColor Cyan
    
    $env:STEALTH_MODE = $Mode
    $job = Start-Job -ScriptBlock {
        Set-Location $using:PWD
        cargo run -p stealth-browser 2>&1
    }
    
    Start-Sleep -Seconds 8
    
    try {
        $status = Invoke-RestMethod http://127.0.0.1:8088/status -TimeoutSec 3
        Write-Host "  Status mode: $($status.mode)" -ForegroundColor Green
        
        try {
            $dir = Invoke-RestMethod http://127.0.0.1:8088/api/relays/by-country -TimeoutSec 2
            Write-Host "  Directory: ENABLED" -ForegroundColor Green
        } catch {
            Write-Host "  Directory: disabled (expected for $Mode)" -ForegroundColor Yellow
        }
    } catch {
        Write-Host "  ERROR: Could not reach Helper" -ForegroundColor Red
    }
    
    Stop-Job $job -PassThru | Remove-Job
}

# Test each mode
@("client", "relay", "bootstrap", "exit", "super") | ForEach-Object { Test-HelperMode $_ }
```

---

## Test 6: Exit Node Functionality

**Purpose**: Verify exit node correctly forwards HTTPS CONNECT requests.

**Duration**: ~2 minutes

### Prerequisites
- Exit node requires special handling for outbound connections
- This test verifies the CONNECT parsing and validation logic

### Steps

1. **Start Helper in exit or super mode**:
   ```powershell
   $env:STEALTH_MODE = "super"
   cargo run -p stealth-browser
   ```

2. **Test SOCKS5 proxy with curl** (if available):
   ```powershell
   curl.exe -v --socks5-hostname 127.0.0.1:1088 https://httpbin.org/ip
   ```

   **Expected**: Response showing exit node's public IP (or connection attempt).

3. **Alternative: Test with PowerShell**:
   ```powershell
   # Note: PowerShell's Invoke-WebRequest doesn't support SOCKS5 directly
   # Use curl.exe or a SOCKS-aware HTTP client
   ```

4. **Check logs** for exit handling:
   ```
   [INFO] Exit: Handling CONNECT request to httpbin.org:443
   [INFO] Exit: TLS passthrough established
   ```

### Pass Criteria
- [ ] SOCKS5 proxy accepts connection
- [ ] CONNECT requests are parsed correctly
- [ ] Blocked ports (25, 110, 143) return error
- [ ] Private IP ranges (127.x, 10.x, 192.168.x) are blocked (SSRF prevention)

---

## Test 7: Graceful Shutdown

**Purpose**: Verify Helper shuts down cleanly without data loss or hanging connections.

**Duration**: ~1 minute

### Steps

1. **Start Helper in any mode**:
   ```powershell
   $env:STEALTH_MODE = "super"
   cargo run -p stealth-browser
   ```

2. **Make some requests** to establish state:
   ```powershell
   # Register a peer
   $body = @{ peer_id = "test"; addrs = @(); country = "US"; capabilities = @(); last_seen = 0; first_seen = 0 } | ConvertTo-Json
   Invoke-RestMethod -Uri "http://127.0.0.1:8088/api/relay/register" -Method POST -ContentType "application/json" -Body $body
   
   # Check status
   Invoke-RestMethod http://127.0.0.1:8088/status
   ```

3. **Send SIGINT** (Ctrl+C) to Helper.

4. **Observe shutdown logs**:
   ```
   [INFO] Received shutdown signal
   [INFO] Closing status server...
   [INFO] Closing SOCKS5 proxy...
   [INFO] Helper shutdown complete
   ```

5. **Verify port is released**:
   ```powershell
   # Should fail (port no longer in use)
   Test-NetConnection -ComputerName 127.0.0.1 -Port 8088 -WarningAction SilentlyContinue
   ```

### Pass Criteria
- [ ] Ctrl+C triggers graceful shutdown
- [ ] Shutdown logs indicate orderly closure
- [ ] Process exits with code 0
- [ ] Ports are released (no "address in use" on restart)

---

## Test 8: Load Testing Directory Endpoints

**Purpose**: Verify directory handles concurrent registrations and queries.

**Duration**: ~5 minutes

### Steps

1. **Start Helper in bootstrap mode**:
   ```powershell
   $env:STEALTH_MODE = "bootstrap"
   cargo run --release -p stealth-browser
   ```

2. **Run concurrent registrations** (100 peers):
   ```powershell
   $jobs = 1..100 | ForEach-Object {
       Start-Job -ScriptBlock {
           param($id)
           $body = @{
               peer_id = "12D3KooWLoadTest$($id.ToString('D4'))"
               addrs = @("/ip4/10.0.0.$($id % 256)/tcp/4001")
               country = @("US", "FR", "DE", "JP", "AU")[$id % 5]
               capabilities = @("relay")
               last_seen = [int][double]::Parse((Get-Date -UFormat %s))
               first_seen = [int][double]::Parse((Get-Date -UFormat %s))
           } | ConvertTo-Json
           
           try {
               Invoke-RestMethod -Uri "http://127.0.0.1:8088/api/relay/register" `
                   -Method POST -ContentType "application/json" -Body $body -TimeoutSec 5
               return "OK"
           } catch {
               return "FAIL: $_"
           }
       } -ArgumentList $_
   }

   # Wait for all jobs
   $results = $jobs | Wait-Job | Receive-Job
   $jobs | Remove-Job

   # Summary
   $success = ($results | Where-Object { $_ -eq "OK" }).Count
   Write-Host "Registrations: $success/100 succeeded"
   ```

3. **Run concurrent queries** (1000 queries):
   ```powershell
   $queryJobs = 1..1000 | ForEach-Object {
       Start-Job -ScriptBlock {
           try {
               Invoke-RestMethod "http://127.0.0.1:8088/api/relays/by-country" -TimeoutSec 5 | Out-Null
               return "OK"
           } catch {
               return "FAIL"
           }
       }
   }

   $queryResults = $queryJobs | Wait-Job | Receive-Job
   $queryJobs | Remove-Job

   $querySuccess = ($queryResults | Where-Object { $_ -eq "OK" }).Count
   Write-Host "Queries: $querySuccess/1000 succeeded"
   ```

4. **Verify all peers registered**:
   ```powershell
   $allPeers = Invoke-RestMethod http://127.0.0.1:8088/api/relays/by-country
   $totalPeers = ($allPeers.PSObject.Properties | ForEach-Object { $_.Value.Count } | Measure-Object -Sum).Sum
   Write-Host "Total peers in directory: $totalPeers"
   ```

### Pass Criteria
- [ ] 100/100 registrations succeed
- [ ] 1000/1000 queries succeed (or >99%)
- [ ] No crashes or hangs under load
- [ ] Response times remain reasonable (<100ms p99)

---

## Test Results Template

Use this template to record test results:

```markdown
## Manual Test Run: [DATE]

### Environment
- OS: Windows 11
- Rust: 1.xx.x
- Commit: [HASH]

### Results

| Test | Status | Notes |
|------|--------|-------|
| 1. Super Peer Startup | ✅/❌ | |
| 2. Directory Endpoints | ✅/❌ | |
| 3. Pruning | ✅/❌ | |
| 4. Heartbeat | ✅/❌ | |
| 5. All Modes | ✅/❌ | |
| 6. Exit Node | ✅/❌ | |
| 7. Graceful Shutdown | ✅/❌ | |
| 8. Load Test | ✅/❌ | Reg: X/100, Query: X/1000 |

### Issues Found
- [Issue description]

### Sign-off
- Tester: [NAME]
- Date: [DATE]
```

---

## Troubleshooting

### Helper won't start
- Check if port 8088/1088 is already in use: `netstat -an | findstr 8088`
- Kill existing process: `Stop-Process -Name stealth-browser -Force`

### Directory endpoints return 404
- Verify mode is `bootstrap` or `super`
- Check status: `Invoke-RestMethod http://127.0.0.1:8088/status`

### Heartbeat not sending
- Verify mode is `relay`, `exit`, or `super`
- Check logs for "heartbeat" messages
- Verify operator nodes are reachable

### Pruning not working
- Background pruning runs every 60 seconds
- Peers must have `last_seen` > 120 seconds ago to be pruned
- Use manual prune: `GET /api/relays/prune`

### Load test failures
- Increase timeouts for slow systems
- Check system resources (CPU, memory)
- Reduce concurrency if needed

---

## Related Documentation

- [Helper Documentation](helper.md) - API reference and configuration
- [Extension Integration](extension.md) - Browser extension development
- [Architecture](../../docs/ARCHITECTURE.md) - System design overview
- [Contributing](../../docs/CONTRIBUTING.md) - Development guidelines
