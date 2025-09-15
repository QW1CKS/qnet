[CmdletBinding(PositionalBinding=$false)]
param(
    [Parameter(Mandatory=$true)]
    [string]$BIP,

    [string]$QNetRoot = "P:\GITHUB\qnet",
    [int]$Port = 4443,
    [string]$Decoy = 'www.cloudflare.com',
    [int]$SocksPort = 1080,
    [int]$StatusPort = 18080,
    [switch]$WriteHosts = $true,
    [switch]$StartClient = $true,
    [switch]$StartCapture = $true,
    [int]$CaptureInterface = 4,
    [switch]$DriveTraffic
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Write-Host "[A] Using QNetRoot=$QNetRoot, BIP=$BIP, Port=$Port, Decoy=$Decoy" -ForegroundColor Cyan

# Build paths
$ClientExe = Join-Path $QNetRoot 'target\release\stealth-browser.exe'
if (-not (Test-Path $ClientExe)) {
    Write-Host "[A] Building stealth-browser (release)" -ForegroundColor Yellow
    Set-Location $QNetRoot
    & cargo build --release -p stealth-browser
}

# Hosts mapping (Decoy -> BIP)
if ($WriteHosts) {
    $hosts = "$env:SystemRoot\System32\drivers\etc\hosts"
    Write-Host "[A] Updating hosts: $Decoy -> $BIP" -ForegroundColor Yellow
    Copy-Item $hosts "$hosts.bak" -Force
    if ((Get-Item $hosts).Attributes -band [IO.FileAttributes]::ReadOnly) { attrib -r $hosts }
    $content = Get-Content -Path $hosts -ErrorAction Stop | Where-Object { $_ -notmatch "\b$([regex]::Escape($Decoy))\b" }
    $line    = "$BIP`t$Decoy"
    $new     = @(); $new += $content; $new += $line
    Set-Content -Path $hosts -Encoding ASCII -Force -Value $new
    Write-Host "[A] Flushing DNS cache" -ForegroundColor Yellow
    try { Clear-DnsClientCache } catch { }
    & "$env:SystemRoot\System32\ipconfig.exe" /flushdns | Out-Null
    Write-Host "[A] Resolve $Decoy ->" -NoNewline
    try { $r = Resolve-DnsName $Decoy -ErrorAction Stop; Write-Host " $(($r | Where-Object Type -eq 'A' | Select-Object -First 1 -ExpandProperty IPAddress))" -ForegroundColor Green } catch { Write-Host " failed" -ForegroundColor Red }
}

# Configure env (Option C - unsigned dev catalog, port aligned)
$env:STEALTH_DECOY_ALLOW_UNSIGNED = '1'
$catalog = @{ catalog = @{ version=1; updated_at=1726128000; entries=@(@{ host_pattern='*'; decoy_host=$Decoy; port=$Port; alpn=@('h2','http/1.1'); weight=1 }) } }
$env:STEALTH_DECOY_CATALOG_JSON = ($catalog | ConvertTo-Json -Depth 6 -Compress)
$env:STEALTH_LOG_DECOY_ONLY = '1'
$env:HTX_INSECURE_NO_VERIFY = '1'
$env:STEALTH_MODE = 'masked'
$env:STEALTH_SOCKS_PORT = "$SocksPort"
$env:STEALTH_STATUS_PORT = "$StatusPort"
$env:RUST_LOG = 'info'

# Start client in background window
if ($StartClient) {
    Write-Host "[A] Launching stealth-browser in a new background PowerShell window" -ForegroundColor Green
    $json = $env:STEALTH_DECOY_CATALOG_JSON
    $cmd = @"
$env:STEALTH_DECOY_ALLOW_UNSIGNED="1";
$env:STEALTH_DECOY_CATALOG_JSON=@"
$json
"@;
$env:STEALTH_LOG_DECOY_ONLY="1";
$env:HTX_INSECURE_NO_VERIFY="1";
$env:STEALTH_MODE="masked";
$env:STEALTH_SOCKS_PORT="$SocksPort";
$env:STEALTH_STATUS_PORT="$StatusPort";
$env:RUST_LOG="info";
Set-Location "$QNetRoot";
& "$ClientExe"
"@
    Start-Process -FilePath powershell.exe -ArgumentList '-NoProfile','-NoExit','-Command', $cmd -WindowStyle Minimized | Out-Null
}

# Start tshark capture in background window
if ($StartCapture) {
    Write-Host "[A] Launching tshark capture (iface $CaptureInterface, port $Port) in a new window" -ForegroundColor Green
    $capCmd = @"
$port = $Port;
tshark -i $CaptureInterface -f ("tcp port {0}" -f $port) -Y "tls.handshake.extensions_server_name || ip" `
  -T fields -e frame.time -e ip.dst -e tls.handshake.extensions_server_name
"@
    Start-Process -FilePath powershell.exe -ArgumentList '-NoProfile','-NoExit','-Command', $capCmd -WindowStyle Minimized | Out-Null
}

# Optional: drive traffic
if ($DriveTraffic) {
    Write-Host "[A] Driving sample traffic via SOCKS" -ForegroundColor Yellow
    & curl.exe -x "socks5h://127.0.0.1:$SocksPort" https://example.com -I | Write-Host
    & curl.exe -x "socks5h://127.0.0.1:$SocksPort" https://www.microsoft.com -I | Write-Host
    & curl.exe -x "socks5h://127.0.0.1:$SocksPort" https://ifconfig.me -I | Write-Host
}

Write-Host "[A] Done." -ForegroundColor Cyan
