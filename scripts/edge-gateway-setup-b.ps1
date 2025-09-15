[CmdletBinding(PositionalBinding=$false)]
param(
    [Parameter(Mandatory=$true)]
    [string]$BIP,

    [string]$QNetRoot = "G:\qnet",
    [int]$Port = 4443,
    [string]$CertsDir,
    [string]$BinDir,
    [switch]$GenerateCerts,
    [switch]$OpenFirewall = $true,
    [switch]$RunGateway = $true,
    [switch]$VerboseLogging,
    [switch]$DisableIPv6,              # Fast, reversible (no reboot); per-NIC binding
    [switch]$PreferIPv4,               # System-wide precedence (requires reboot)
    [string]$ActiveNicName             # Optional NIC name for IPv6 disable; defaults to first Up adapter
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# Resolve defaults
if (-not $CertsDir) { $CertsDir = Join-Path $QNetRoot 'certs' }
if (-not $BinDir)   { $BinDir   = Join-Path $QNetRoot 'bin' }
$LogsDir = Join-Path $QNetRoot 'logs'

Write-Host "[B] Using QNetRoot=$QNetRoot, Certs=$CertsDir, Bin=$BinDir, Port=$Port" -ForegroundColor Cyan

# Ensure folders
$null = New-Item -ItemType Directory -Force -Path $CertsDir
$null = New-Item -ItemType Directory -Force -Path $BinDir
$null = New-Item -ItemType Directory -Force -Path $LogsDir

# Optional: Prefer IPv4 or disable IPv6 binding
if ($PreferIPv4) {
    Write-Host "[B] Setting DisabledComponents=0x20 to prefer IPv4 (reboot required)" -ForegroundColor Yellow
    New-Item -Path "HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip6\Parameters" -Force | Out-Null
    New-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip6\Parameters" -Name "DisabledComponents" -PropertyType DWord -Value 0x20 -Force | Out-Null
}

if ($DisableIPv6) {
    $nic = if ($ActiveNicName) { Get-NetAdapter -Name $ActiveNicName -ErrorAction Stop } else { Get-NetAdapter | Where-Object Status -eq Up | Select-Object -First 1 }
    if ($null -eq $nic) { throw "No active NIC found; specify -ActiveNicName" }
    Write-Host "[B] Disabling IPv6 binding on NIC '$($nic.Name)'" -ForegroundColor Yellow
    Disable-NetAdapterBinding -Name $nic.Name -ComponentID ms_tcpip6 -PassThru | Out-Null
}

# Generate certs if requested or missing
$crt = Join-Path $CertsDir 'edge.crt'
$key = Join-Path $CertsDir 'edge.key'
if ($GenerateCerts -or -not (Test-Path $crt) -or -not (Test-Path $key)) {
    Write-Host "[B] Generating self-signed TLS cert via OpenSSL" -ForegroundColor Yellow
    Set-Location $CertsDir
    & openssl req -x509 -nodes -newkey rsa:2048 `
        -keyout $key `
        -out $crt `
        -days 825 `
        -subj "/C=US/ST=NA/L=Local/O=QNet/OU=Dev/CN=$env:COMPUTERNAME"
}

if ($OpenFirewall) {
    Write-Host "[B] Ensuring inbound firewall rule for TCP $Port" -ForegroundColor Yellow
    $ruleName = "QNet Gateway TLS $Port"
    if (-not (Get-NetFirewallRule -DisplayName $ruleName -ErrorAction SilentlyContinue)) {
        New-NetFirewallRule -DisplayName $ruleName -Direction Inbound -Action Allow -Protocol TCP -LocalPort $Port | Out-Null
    }
}

# Verify binary
$exe = Join-Path $BinDir 'edge-gateway.exe'
if (-not (Test-Path $exe)) {
    throw "edge-gateway.exe not found at $exe. Build it (cargo build --release -p edge-gateway) or copy it here."
}

if ($RunGateway) {
    $ts = Get-Date -Format 'yyyyMMdd_HHmmss'
    $logPath = Join-Path $LogsDir ("edge-gateway_" + $ts + ".log")
    $rl = if ($VerboseLogging) { 'debug' } else { 'info' }

    Write-Host "[B] Launching gateway in a new background PowerShell window (logs -> $logPath)" -ForegroundColor Green

    $cmd = @"
$env:BIND         = '0.0.0.0:$Port';
$env:HTX_TLS_CERT = '$crt';
$env:HTX_TLS_KEY  = '$key';
$env:RUST_LOG     = '$rl';
Set-Location '$BinDir';
& (Join-Path '$BinDir' 'edge-gateway.exe') *>&1 | Tee-Object -FilePath '$logPath'
"@

    Start-Process -FilePath powershell.exe -ArgumentList '-NoProfile','-NoExit','-Command', $cmd -WindowStyle Minimized | Out-Null
    Write-Host "[B] Started. Tip: Use Get-Content -Wait '$logPath' to tail logs." -ForegroundColor DarkGreen
} else {
    Write-Host "[B] Skipping RunGateway per parameters." -ForegroundColor Yellow
}

Write-Host "[B] Done." -ForegroundColor Cyan
