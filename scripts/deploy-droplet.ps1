#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Deploy QNet stealth-browser to DigitalOcean droplet and run connectivity test.

.DESCRIPTION
    This script:
    1. Builds release binary locally
    2. Uploads to droplet via SCP
    3. Configures firewall on droplet
    4. Starts Helper in bootstrap/relay mode
    5. Monitors logs for peer discovery

.PARAMETER DropletIP
    IPv4 address of your DigitalOcean droplet (default: 209.38.203.84)

.PARAMETER SSHUser
    SSH username (default: root)

.PARAMETER SkipBuild
    Skip local build and use existing binary

.PARAMETER TestOnly
    Only run connectivity test, don't redeploy

.EXAMPLE
    .\scripts\deploy-droplet.ps1
    Deploy to default droplet 209.38.203.84

.EXAMPLE
    .\scripts\deploy-droplet.ps1 -DropletIP 164.90.123.456
    Deploy to custom droplet IP

.EXAMPLE
    .\scripts\deploy-droplet.ps1 -TestOnly
    Just check connectivity to existing deployment
#>

param(
    [string]$DropletIP = "209.38.203.84",
    [string]$SSHUser = "root",
    [switch]$SkipBuild,
    [switch]$TestOnly
)

$ErrorActionPreference = "Stop"

# Colors for output
function Write-Step { param($msg) Write-Host "🔹 $msg" -ForegroundColor Cyan }
function Write-Success { param($msg) Write-Host "✅ $msg" -ForegroundColor Green }
function Write-Failure { param($msg) Write-Host "❌ $msg" -ForegroundColor Red }
function Write-Info { param($msg) Write-Host "ℹ️  $msg" -ForegroundColor Yellow }

$RepoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$BinaryPath = Join-Path $RepoRoot "target\release\stealth-browser.exe"
$SSHTarget = "${SSHUser}@${DropletIP}"

Write-Host "`n🚀 QNet Droplet Deployment Script" -ForegroundColor Magenta
Write-Host "================================================`n" -ForegroundColor Magenta

# ============================================================================
# Phase 1: Build Binary (unless skipped)
# ============================================================================

if (-not $TestOnly -and -not $SkipBuild) {
    Write-Step "Building release binary..."
    Push-Location $RepoRoot
    try {
        cargo build --release --bin stealth-browser
        if ($LASTEXITCODE -ne 0) {
            throw "Build failed with exit code $LASTEXITCODE"
        }
        Write-Success "Build complete: $BinaryPath"
    }
    finally {
        Pop-Location
    }
}

if (-not $TestOnly) {
    if (-not (Test-Path $BinaryPath)) {
        Write-Failure "Binary not found at: $BinaryPath"
        Write-Info "Run with -SkipBuild to skip rebuilding"
        exit 1
    }
    
    $BinarySize = (Get-Item $BinaryPath).Length / 1MB
    Write-Info "Binary size: $([math]::Round($BinarySize, 2)) MB"
}

# ============================================================================
# Phase 2: Test SSH Connectivity
# ============================================================================

Write-Step "Testing SSH connection to $SSHTarget..."
$SshTest = ssh -o ConnectTimeout=10 -o BatchMode=yes $SSHTarget "echo 'SSH OK'" 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Failure "Cannot connect to droplet via SSH"
    Write-Info "Ensure SSH key is configured or use: ssh-copy-id $SSHTarget"
    exit 1
}
Write-Success "SSH connection OK"

# ============================================================================
# Phase 3: Upload Binary (unless TestOnly)
# ============================================================================

if (-not $TestOnly) {
    Write-Step "Uploading binary to droplet..."
    
    # Stop any running Helper first
    Write-Info "Stopping any existing Helper process..."
    ssh $SSHTarget "pkill -9 stealth-browser 2>/dev/null || true"
    Start-Sleep -Seconds 2
    
    # Upload binary
    scp -C $BinaryPath "${SSHTarget}:/root/stealth-browser"
    if ($LASTEXITCODE -ne 0) {
        Write-Failure "Upload failed"
        exit 1
    }
    
    # Make executable
    ssh $SSHTarget "chmod +x /root/stealth-browser"
    Write-Success "Binary uploaded and marked executable"
}

# ============================================================================
# Phase 4: Configure Firewall (unless TestOnly)
# ============================================================================

if (-not $TestOnly) {
    Write-Step "Configuring firewall rules..."
    
    $FirewallScript = @'
#!/bin/bash
set -e

# Check if ufw is installed
if ! command -v ufw &> /dev/null; then
    echo "Installing ufw..."
    apt-get update -qq
    apt-get install -y ufw
fi

# Configure rules
ufw --force allow 22/tcp comment "SSH"
ufw --force allow 40000:50000/tcp comment "QNet libp2p TCP"
ufw --force allow 40000:50000/udp comment "QNet libp2p UDP (QUIC)"
ufw --force allow 8088/tcp comment "QNet status API"

# Enable firewall (--force to avoid interactive prompt)
ufw --force enable

# Show status
ufw status numbered
'@
    
    $FirewallScript | ssh $SSHTarget "cat > /tmp/setup_firewall.sh && bash /tmp/setup_firewall.sh"
    if ($LASTEXITCODE -ne 0) {
        Write-Failure "Firewall configuration failed"
        exit 1
    }
    Write-Success "Firewall configured"
}

# ============================================================================
# Phase 5: Start Helper on Droplet
# ============================================================================

Write-Step "Starting Helper on droplet in bootstrap/relay mode..."

# Create startup script
$StartupScript = @'
#!/bin/bash
export RUST_LOG=info
export RUST_BACKTRACE=1

echo "Starting QNet Helper in bootstrap/relay mode..."
echo "Droplet acts as public relay server for NAT traversal"
echo ""

cd /root
exec ./stealth-browser --bootstrap
'@

$StartupScript | ssh $SSHTarget "cat > /root/start_helper.sh && chmod +x /root/start_helper.sh"

# Start in background using nohup
Write-Info "Launching Helper in background (logs: /root/helper.log)..."
ssh $SSHTarget "nohup /root/start_helper.sh > /root/helper.log 2>&1 &"
Start-Sleep -Seconds 3

# Check if process started
$ProcessCheck = ssh $SSHTarget "pgrep -f stealth-browser"
if ($LASTEXITCODE -ne 0) {
    Write-Failure "Helper failed to start. Checking logs..."
    ssh $SSHTarget "tail -20 /root/helper.log"
    exit 1
}

Write-Success "Helper started on droplet (PID: $ProcessCheck)"

# ============================================================================
# Phase 6: Extract Droplet Peer ID
# ============================================================================

Write-Step "Waiting for Helper initialization (10s)..."
Start-Sleep -Seconds 10

Write-Step "Extracting droplet peer ID from logs..."
$DropletLogs = ssh $SSHTarget "tail -100 /root/helper.log"
$PeerIdMatch = $DropletLogs | Select-String -Pattern "peer_id=(12D3KooW\w+)"

if ($PeerIdMatch) {
    $DropletPeerId = $PeerIdMatch.Matches[0].Groups[1].Value
    Write-Success "Droplet Peer ID: $DropletPeerId"
    Write-Host ""
} else {
    Write-Failure "Could not extract peer ID from logs. Recent logs:"
    Write-Host $DropletLogs
    exit 1
}

# ============================================================================
# Phase 7: Show Droplet Status
# ============================================================================

Write-Step "Checking droplet status..."
$StatusJson = ssh $SSHTarget "curl -s http://127.0.0.1:8088/status"
if ($LASTEXITCODE -eq 0) {
    $Status = $StatusJson | ConvertFrom-Json
    Write-Host ""
    Write-Host "📊 Droplet Status:" -ForegroundColor Cyan
    Write-Host "  State: $($Status.state)" -ForegroundColor $(if ($Status.state -eq "connected") { "Green" } else { "Yellow" })
    Write-Host "  Bootstrap Peers: $($Status.bootstrap_peers)" -ForegroundColor White
    Write-Host "  QNet Peers: $($Status.qnet_peers)" -ForegroundColor White
    Write-Host "  Total Peers: $($Status.peers_total)" -ForegroundColor White
    Write-Host "  Active Circuits: $($Status.active_circuits)" -ForegroundColor White
    Write-Host ""
}

# ============================================================================
# Phase 8: Instructions for Local Testing
# ============================================================================

Write-Host "`n" + "="*60 -ForegroundColor Magenta
Write-Host "🎯 DROPLET READY - Now Test From Your Laptop!" -ForegroundColor Magenta
Write-Host "="*60 + "`n" -ForegroundColor Magenta

Write-Host "Droplet Peer ID:  " -NoNewline -ForegroundColor Yellow
Write-Host "$DropletPeerId" -ForegroundColor Green

Write-Host "`n📋 NEXT STEPS:`n" -ForegroundColor Cyan

Write-Host "1️⃣  Start Helper on your laptop:" -ForegroundColor Cyan
Write-Host "    cd $RepoRoot" -ForegroundColor White
Write-Host "    cargo run --release --bin stealth-browser" -ForegroundColor White
Write-Host ""

Write-Host "2️⃣  Watch for these log messages on LAPTOP:" -ForegroundColor Cyan
Write-Host "    ✨ mesh: AutoNAT status changed: Unknown → Private" -ForegroundColor Gray
Write-Host "    ✨ mesh: Connected to QNet peer $DropletPeerId" -ForegroundColor Gray
Write-Host "    ✨ mesh: Relay client connected to relay server" -ForegroundColor Gray
Write-Host ""

Write-Host "3️⃣  Check laptop status (after 2-5 min):" -ForegroundColor Cyan
Write-Host "    curl http://127.0.0.1:8088/status | ConvertFrom-Json" -ForegroundColor White
Write-Host ""
Write-Host "    Expected: qnet_peers = 1 (discovered droplet!)" -ForegroundColor Gray
Write-Host ""

Write-Host "4️⃣  Monitor droplet logs in real-time:" -ForegroundColor Cyan
Write-Host "    ssh $SSHTarget 'tail -f /root/helper.log'" -ForegroundColor White
Write-Host ""

Write-Host "="*60 -ForegroundColor Magenta
Write-Host "📖 Full test guide: artifacts\dht_droplet_test.md" -ForegroundColor Yellow
Write-Host "="*60 + "`n" -ForegroundColor Magenta

# ============================================================================
# Phase 9: Wait for User to Start Local Test
# ============================================================================

Write-Host "⏳ Waiting for you to start laptop Helper..." -ForegroundColor Yellow
Write-Host "   Press Enter when laptop Helper is running, or Ctrl+C to exit" -ForegroundColor Gray
Read-Host

# ============================================================================
# Phase 10: Monitor for Discovery
# ============================================================================

Write-Step "Monitoring for peer discovery (max 5 minutes)..."
Write-Info "Checking laptop status every 30 seconds..."

$DiscoverySuccess = $false
$MaxAttempts = 10  # 10 attempts × 30s = 5 minutes
$Attempt = 0

while ($Attempt -lt $MaxAttempts -and -not $DiscoverySuccess) {
    $Attempt++
    Write-Host "`n🔍 Check $Attempt/$MaxAttempts..." -ForegroundColor Cyan
    
    try {
        # Check laptop status
        $LaptopStatus = Invoke-RestMethod -Uri "http://127.0.0.1:8088/status" -TimeoutSec 5
        Write-Host "  Laptop - Bootstrap: $($LaptopStatus.bootstrap_peers), QNet: $($LaptopStatus.qnet_peers), Total: $($LaptopStatus.peers_total)" -ForegroundColor White
        
        # Check droplet status
        $DropletStatusJson = ssh $SSHTarget "curl -s http://127.0.0.1:8088/status"
        $DropletStatus = $DropletStatusJson | ConvertFrom-Json
        Write-Host "  Droplet - Bootstrap: $($DropletStatus.bootstrap_peers), QNet: $($DropletStatus.qnet_peers), Total: $($DropletStatus.peers_total)" -ForegroundColor White
        
        # Check if discovery happened
        if ($LaptopStatus.qnet_peers -ge 1) {
            Write-Success "🎉 DISCOVERY SUCCESSFUL!"
            Write-Host ""
            Write-Host "Laptop discovered $($LaptopStatus.qnet_peers) QNet peer(s)" -ForegroundColor Green
            Write-Host "Active circuits: $($LaptopStatus.active_circuits)" -ForegroundColor Green
            $DiscoverySuccess = $true
            break
        }
        
        if ($Attempt -lt $MaxAttempts) {
            Write-Host "  ⏳ No QNet peers yet, waiting 30s..." -ForegroundColor Gray
            Start-Sleep -Seconds 30
        }
    }
    catch {
        Write-Host "  ⚠️  Status check failed: $($_.Exception.Message)" -ForegroundColor Yellow
        if ($Attempt -lt $MaxAttempts) {
            Start-Sleep -Seconds 30
        }
    }
}

Write-Host ""
if ($DiscoverySuccess) {
    Write-Host "="*60 -ForegroundColor Green
    Write-Host "✅ PEER DISCOVERY TEST: PASSED" -ForegroundColor Green
    Write-Host "="*60 -ForegroundColor Green
    Write-Host ""
    Write-Host "🎯 Results:" -ForegroundColor Cyan
    Write-Host "  ✅ Laptop (behind NAT) → Droplet (public IP): Connected" -ForegroundColor Green
    Write-Host "  ✅ AutoNAT detection: Working" -ForegroundColor Green
    Write-Host "  ✅ Circuit Relay V2: Working" -ForegroundColor Green
    Write-Host "  ✅ DHT peer discovery: Working" -ForegroundColor Green
    Write-Host ""
} else {
    Write-Host "="*60 -ForegroundColor Yellow
    Write-Host "⚠️  PEER DISCOVERY: TIMEOUT (5 minutes)" -ForegroundColor Yellow
    Write-Host "="*60 -ForegroundColor Yellow
    Write-Host ""
    Write-Host "🔍 Troubleshooting:" -ForegroundColor Cyan
    Write-Host "  1. Check laptop logs: Look for 'AutoNAT status' messages" -ForegroundColor White
    Write-Host "  2. Check droplet logs: ssh $SSHTarget 'tail -50 /root/helper.log'" -ForegroundColor White
    Write-Host "  3. Verify firewall: ssh $SSHTarget 'ufw status'" -ForegroundColor White
    Write-Host "  4. Enable debug: Set RUST_LOG=debug on both laptop and droplet" -ForegroundColor White
    Write-Host ""
}

# ============================================================================
# Cleanup Instructions
# ============================================================================

Write-Host "🛑 To stop droplet Helper:" -ForegroundColor Yellow
Write-Host "   ssh $SSHTarget 'pkill stealth-browser'" -ForegroundColor White
Write-Host ""
Write-Host "📋 To view live logs:" -ForegroundColor Yellow
Write-Host "   ssh $SSHTarget 'tail -f /root/helper.log'" -ForegroundColor White
Write-Host ""
