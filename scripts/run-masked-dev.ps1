# QNet masked-mode dev runner (Windows, PowerShell)
# - Launches edge-gateway and stealth-browser in separate pwsh windows
# - Prompts for a target website and verifies via curl.exe through SOCKS
# - Opens the local QNet status page in your default browser
# - Optionally generates a dev CA and localhost leaf with OpenSSL for secure TLS (no insecure bypass)

param(
    [string]$Url,
    [switch]$SkipElevate,
    [switch]$Tshark,
    [string]$TsharkInterface,
    [int]$TsharkDurationSec = 20
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Write-Info($msg)  { Write-Host "[i] $msg" -ForegroundColor Cyan }
function Write-Warn($msg)  { Write-Host "[!] $msg" -ForegroundColor Yellow }
function Write-Err($msg)   { Write-Host "[x] $msg" -ForegroundColor Red }
function Write-Ok($msg)    { Write-Host "[OK] $msg" -ForegroundColor Green }

# Self-elevate to Administrator if needed (for firewall prompts and smoother local binds)
function Ensure-Admin {
    try {
        $wi = [Security.Principal.WindowsIdentity]::GetCurrent()
        $wp = New-Object Security.Principal.WindowsPrincipal($wi)
        if (-not $SkipElevate -and -not $wp.IsInRole([Security.Principal.WindowsBuiltinRole]::Administrator)) {
            Write-Warn "Requesting Administrator privileges..."
            $scriptPath = $PSCommandPath
            if (-not $scriptPath) { $scriptPath = $MyInvocation.MyCommand.Path }
            $args = @('-NoLogo','-NoProfile','-File', $scriptPath)
            if ($Url) { $args += @('-Url', $Url) }
            # Ensure elevated child doesn't attempt re-elevation
            $args += '-SkipElevate'
            $shell = Get-DefaultShellExe
            Start-Process -FilePath $shell -ArgumentList $args -Verb RunAs | Out-Null
            exit
        }
    } catch {
        Write-Warn "Admin detection failed: $($_.Exception.Message) — continuing without elevation"
    }
}
function Get-DefaultShellExe {
    $pwsh = (Get-Command 'pwsh' -ErrorAction SilentlyContinue)
    if ($pwsh) { return $pwsh.Source }
    $ps = (Get-Command 'powershell.exe' -ErrorAction SilentlyContinue)
    if ($ps) { return $ps.Source }
    throw "Neither pwsh nor powershell.exe found in PATH"
}
Ensure-Admin

function Get-RepoRoot {
    if ($PSScriptRoot) { return (Split-Path $PSScriptRoot -Parent) }
    return (Resolve-Path (Join-Path (Split-Path $MyInvocation.MyCommand.Path -Parent) '..')).Path
}

# --- tshark helpers (optional Wireshark CLI capture) ---
function Find-Tshark {
    $t = Get-Command 'tshark' -ErrorAction SilentlyContinue
    if ($t) { return $t.Source }
    return $null
}
function Get-TsharkOutboundInterface {
    param([string]$TsharkExe)
    try {
        $list = & $TsharkExe -D 2>$null
        # Heuristic: pick first non-loopback interface mentioning Ethernet/Wi-Fi/Wireless
        $cand = ($list | Select-String -Pattern 'Ethernet|Wi-Fi|Wireless' | Where-Object { $_ -notmatch 'Loopback' } | Select-Object -First 1).ToString()
        if (-not $cand) {
            # Fallback: first interface that's not loopback
            $cand = ($list | Where-Object { $_ -notmatch 'Loopback' } | Select-Object -First 1).ToString()
        }
        if (-not $cand) { return $null }
        $m = [regex]::Match($cand, '^(\d+)\.')
        if ($m.Success) { return $m.Groups[1].Value }
        return $null
    } catch { return $null }
}

$repo = Get-RepoRoot
$certs = Join-Path $repo 'certs'
# Prefer debug builds; fall back to release if debug not present
$edgeExeDebug   = Join-Path $repo 'target\debug\edge-gateway.exe'
$edgeExeRelease = Join-Path $repo 'target\release\edge-gateway.exe'
$helperExeDebug   = Join-Path $repo 'target\debug\stealth-browser.exe'
$helperExeRelease = Join-Path $repo 'target\release\stealth-browser.exe'

if (Test-Path $edgeExeDebug) { $edgeExe = $edgeExeDebug }
elseif (Test-Path $edgeExeRelease) { $edgeExe = $edgeExeRelease }
else { $edgeExe = $edgeExeDebug }

if (Test-Path $helperExeDebug) { $helperExe = $helperExeDebug }
elseif (Test-Path $helperExeRelease) { $helperExe = $helperExeRelease }
else { $helperExe = $helperExeDebug }

$devRoot    = Join-Path $certs 'dev-rootCA.pem'
$devRootKey = Join-Path $certs 'dev-rootCA.key'
$leafCrt    = Join-Path $certs 'localhost-leaf.pem'
$leafKey    = Join-Path $certs 'localhost-leaf.key'

$fallbackLeaf = Join-Path $certs 'localhost.pem'
$fallbackKey  = Join-Path $certs 'localhost-key.pem'

# Log session folder
$ts = Get-Date -Format 'yyyyMMdd-HHmmss'
$sessionDir = Join-Path $repo ("logs\\masked-dev-" + $ts)
New-Item -ItemType Directory -Force -Path $sessionDir | Out-Null
$edgeLog   = Join-Path $sessionDir 'edge-gateway.log'
$helperLog = Join-Path $sessionDir 'stealth-browser.log'
$verifyLog = Join-Path $sessionDir 'verify.log'
$pcapFile  = Join-Path $sessionDir 'loopback-4443.pcapng'
$tsharkTxt = Join-Path $sessionDir 'tshark-stdout.log'

Write-Host "QNet masked-mode dev runner" -ForegroundColor Magenta
Write-Host ("Repo: {0}" -f $repo)

if (-not (Test-Path $edgeExe))   { Write-Err "Missing edge-gateway binary. Build with: cargo build -p htx --features 'rustls-config stealth-mode'; cargo build -p edge-gateway --release"; exit 1 }
if (-not (Test-Path $helperExe)) { Write-Err "Missing stealth-browser binary. Build with: cargo build -p htx --features 'rustls-config stealth-mode'; cargo build -p stealth-browser --release"; exit 1 }

if (-not $Url -or [string]::IsNullOrWhiteSpace($Url)) {
    do {
        $Url = Read-Host "Enter website to fetch via SOCKS (e.g. https://example.com)"
        if ([string]::IsNullOrWhiteSpace($Url)) { Write-Warn "Please enter a non-empty URL." }
    } while ([string]::IsNullOrWhiteSpace($Url))
}
Write-Info "Target URL: $Url"

New-Item -ItemType Directory -Force -Path $certs | Out-Null

function Ensure-DevCertChain {
    $openssl = Get-Command 'openssl.exe' -ErrorAction SilentlyContinue
    if (-not $openssl) {
        Write-Warn "OpenSSL not found in PATH. Will use fallback certs or insecure bypass."
        return $false
    }

    if (-not (Test-Path $devRoot -PathType Leaf)) {
        Write-Info "Generating dev root CA..."
        & $openssl.Path req -x509 -new -nodes -days 3650 -newkey rsa:2048 `
            -keyout $devRootKey -out $devRoot `
            -subj "/CN=QNet Dev Root CA" `
            -addext "basicConstraints=critical,CA:TRUE" `
            -addext "keyUsage=critical,keyCertSign,cRLSign" | Out-Null
        Write-Ok "Dev Root CA created: $devRoot"
    } else {
        Write-Info "Dev Root CA exists: $devRoot"
    }

    if (-not (Test-Path $leafCrt -PathType Leaf)) {
        Write-Info "Generating localhost leaf cert signed by dev root..."
        $leafCsr = Join-Path $certs 'localhost-leaf.csr'
        & $openssl.Path req -new -newkey rsa:2048 -nodes -keyout $leafKey -out $leafCsr -subj "/CN=localhost" | Out-Null

    $extLines = @(
        'basicConstraints=critical,CA:FALSE',
        'keyUsage=critical,digitalSignature,keyEncipherment',
        'extendedKeyUsage=serverAuth',
        'subjectAltName=DNS:localhost,IP:127.0.0.1'
    )
    $ext = [string]::Join([Environment]::NewLine, $extLines)
    $extFile = Join-Path $env:TEMP 'leaf-ext.cnf'
    Set-Content -Path $extFile -Value $ext -NoNewline

        & $openssl.Path x509 -req -in $leafCsr -CA $devRoot -CAkey $devRootKey -CAcreateserial `
            -out $leafCrt -days 825 -extfile $extFile | Out-Null
        Remove-Item $leafCsr -ErrorAction SilentlyContinue
        Write-Ok "Leaf created: $leafCrt"

        $verify = & $openssl.Path verify -CAfile $devRoot $leafCrt 2>$null
        if (-not $verify -or -not ($verify -match ": OK$")) { Write-Warn "OpenSSL verify did not return OK: $verify" }
    } else {
        Write-Info "Leaf exists: $leafCrt"
    }
    return $true
}

$haveChain = Ensure-DevCertChain

# Decide which certs/env to use
if ($haveChain) {
    $edgeCert = $leafCrt
    $edgeKey  = $leafKey
    $trustLine = ('$env:HTX_TRUST_PEM = ''{0}''' -f $devRoot)
    Write-Ok "Using dev CA trust: $devRoot"
} elseif ((Test-Path $fallbackLeaf) -and (Test-Path $fallbackKey)) {
    Write-Warn "Falling back to bundled localhost.pem/key. Enabling insecure verification for helper (dev only)."
    $edgeCert = $fallbackLeaf
    $edgeKey  = $fallbackKey
    $trustLine = '$env:HTX_INSECURE_NO_VERIFY = ''1''' 
} else {
    Write-Err "No OpenSSL and no fallback certs found. Cannot proceed."
    Write-Host "Expected one of: $devRoot + $leafCrt, or $fallbackLeaf + $fallbackKey" -ForegroundColor Yellow
    exit 1
}

Write-Info "Launching Edge Gateway (new window)... (logs: $edgeLog)"
$edgeCmd = "Set-Location '{0}'; Write-Host 'Starting edge-gateway ...' -ForegroundColor Cyan; `$env:BIND = '127.0.0.1:4443'; `$env:HTX_TLS_CERT = '{1}'; `$env:HTX_TLS_KEY = '{2}'; `$env:NO_COLOR = '1'; `$env:RUST_LOG_STYLE = 'never'; `$env:RUST_LOG = 'info,htx=debug,edge_gateway=info'; `& '{3}' 2>&1 | Tee-Object -FilePath '{4}' -Append" -f $repo, $edgeCert, $edgeKey, $edgeExe, $edgeLog
Start-Process (Get-DefaultShellExe) -ArgumentList @('-NoExit','-NoLogo','-Command', $edgeCmd) -WorkingDirectory $repo -WindowStyle Normal -ErrorAction Stop | Out-Null

# Wait for edge port readiness to avoid verify hangs
function Wait-Port($HostName, $Port, $Label, $TimeoutSec = 20) {
    $msg = 'Waiting for {0} ({1}:{2}) ...' -f $Label, $HostName, $Port
    Write-Info $msg
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    while ($sw.Elapsed.TotalSeconds -lt $TimeoutSec) {
        try {
            $client = New-Object System.Net.Sockets.TcpClient
            $iar = $client.BeginConnect($HostName, [int]$Port, $null, $null)
            if ($iar.AsyncWaitHandle.WaitOne(300) -and $client.Connected) {
                $client.Close()
                $up = ('{0} is up' -f $Label)
                Write-Ok $up
                return $true
            }
            $client.Close()
        } catch {}
        Start-Sleep -Milliseconds 200
    }
    $msg2 = 'Timeout waiting for {0} ({1}:{2})' -f $Label, $HostName, $Port
    Write-Err $msg2
    return $false
}

if (-not (Wait-Port '127.0.0.1' 4443 'Edge Gateway')) { Write-Warn 'Continuing anyway...' }

Write-Info "Launching Helper (new window)... (logs: $helperLog)"
$helperCmd = "Set-Location '{0}'; Write-Host 'Starting stealth-browser ...' -ForegroundColor Cyan; `$env:STEALTH_MODE = 'masked'; `$env:STEALTH_SOCKS_PORT = '1088'; `$env:STEALTH_STATUS_PORT = '8088'; `$env:STEALTH_DECOY_DEV_LOCAL = '1'; `$env:STEALTH_DECOY_ALLOW_UNSIGNED = '1'; {1}; `$env:NO_COLOR = '1'; `$env:RUST_LOG_STYLE = 'never'; `$env:RUST_LOG = 'info,htx=debug,stealth_browser=info'; `& '{2}' 2>&1 | Tee-Object -FilePath '{3}' -Append" -f $repo, $trustLine, $helperExe, $helperLog
Start-Process (Get-DefaultShellExe) -ArgumentList @('-NoExit','-NoLogo','-Command', $helperCmd) -WorkingDirectory $repo -WindowStyle Normal -ErrorAction Stop | Out-Null

if (-not (Wait-Port '127.0.0.1' 1088 'Helper SOCKS')) { Write-Warn 'Continuing anyway...' }
if (-not (Wait-Port '127.0.0.1' 8088 'Helper Status')) { Write-Warn 'Continuing anyway...' }

Write-Info "Opening QNet status page..."
Start-Process "http://127.0.0.1:8088/" | Out-Null

Start-Sleep -Seconds 1

if ($Tshark) {
    $tsharkExe = Find-Tshark
    if ($tsharkExe) {
        $iface = if ($TsharkInterface) { $TsharkInterface } else { Get-TsharkOutboundInterface -TsharkExe $tsharkExe }
        if ($iface) {
            Write-Info ("Starting tshark capture on outbound interface {0} for {1}s (tcp port 443) ..." -f $iface, $TsharkDurationSec)
            $filter = 'tcp port 443'
            $args = @('-i', $iface, '-f', $filter, '-w', $pcapFile)
            $psi = New-Object System.Diagnostics.ProcessStartInfo
            $psi.FileName = $tsharkExe
            $psi.Arguments = ($args -join ' ')
            $psi.RedirectStandardOutput = $true
            $psi.RedirectStandardError = $true
            $psi.UseShellExecute = $false
            $global:TsharkProc = [System.Diagnostics.Process]::Start($psi)
        } else {
            Write-Warn "Could not determine outbound interface for tshark; skipping capture."
        }
    } else { Write-Warn "tshark not found in PATH; skipping capture." }
}

Write-Info "Verifying via curl.exe through SOCKS (new window)... (logs: $verifyLog)"
$verifyCmd = "Set-Location '{0}'; Write-Host 'Requesting: {1}' -ForegroundColor Cyan; curl.exe -L --socks5-hostname 127.0.0.1:1088 --connect-timeout 15 --max-time 90 --retry 2 --retry-all-errors --retry-delay 2 -v -H 'User-Agent: qnet-dev/1.0' --write-out '`nHTTP_CODE=%{{http_code}} REDIRECT_URL=%{{redirect_url}} CONNECT_TIME=%{{time_connect}} TLS_HS=%{{time_appconnect}} TOTAL=%{{time_total}}`n' '{1}' 2>&1 | Tee-Object -FilePath '{2}' -Append; `$nl = [Environment]::NewLine; Write-Host (`$nl + 'Done. Press Enter to close this window.') -ForegroundColor Yellow; Read-Host" -f $repo, $Url, $verifyLog
Start-Process (Get-DefaultShellExe) -ArgumentList @('-NoExit','-NoLogo','-Command', $verifyCmd) -WorkingDirectory $repo -WindowStyle Normal -ErrorAction Stop | Out-Null

Write-Ok "All windows launched. Watch the Edge/Helper logs."
Write-Host "- Edge window shows: 'outer TLS accepted' and 'CONNECT prelude ...' when requests flow" -ForegroundColor DarkGray
Write-Host "- Helper status at http://127.0.0.1:8088/ should show state: connected after the curl" -ForegroundColor DarkGray

Write-Info "Opening log session folder..."
Start-Process -FilePath explorer.exe -ArgumentList $sessionDir | Out-Null

# Probe status shortly after launching verify to surface target/decoy info in this console
Start-Sleep -Seconds 2
try {
    $status = Invoke-RestMethod -Uri 'http://127.0.0.1:8088/status' -TimeoutSec 5
    if ($null -ne $status) {
        $lt = $status.last_target
        $ld = $status.last_decoy
        if ($lt -or $ld) {
            Write-Ok ("Status confirms last_target={0} last_decoy={1}" -f ($lt | ForEach-Object { $_ }), ($ld | ForEach-Object { $_ }))
        } else {
            Write-Warn "Status JSON available but last_target/last_decoy not yet set (will appear after first successful request)."
        }
    }
} catch {
    Write-Warn ("Couldn't fetch status JSON yet: {0}" -f $_.Exception.Message)
}

# If tshark capture was started earlier, stop after a short window and analyze SNI
if ($Tshark -and $global:TsharkProc) {
    Start-Sleep -Seconds $TsharkDurationSec
    try { $global:TsharkProc.Kill() } catch {}
    try {
        $tsharkExe = Find-Tshark
        if ($tsharkExe) {
            Write-Info "Analyzing capture for TLS SNI (decoy hostname) ..."
            & $tsharkExe -r $pcapFile -Y "ssl.handshake.extensions_server_name || tls.handshake.extensions_server_name" -T fields -e tls.handshake.extensions_server_name 2>$null | Where-Object { $_ -and $_ -ne '-' } | Set-Content -Path $tsharkTxt
            if (Test-Path $tsharkTxt) {
                $names = Get-Content $tsharkTxt
                if ($names -and $names.Count -gt 0) {
                    $uniq = $names | Sort-Object -Unique
                    Write-Ok ("tshark SNI observed: {0}" -f ($uniq -join ', '))
                    # Try to fetch status and compare programmatically
                    try {
                        $status = Invoke-RestMethod -Uri 'http://127.0.0.1:8088/status' -TimeoutSec 3
                        $dec = $status.current_decoy
                        if (-not $dec) { $dec = $status.last_decoy }
                        if ($dec) {
                            if ($uniq -contains $dec) {
                                Write-Ok ("Capture confirms decoy masking (SNI matches status decoy: {0})" -f $dec)
                            } else {
                                Write-Warn ("SNI list did not include status decoy ({0}). Names: {1}" -f $dec, ($uniq -join ', '))
                            }
                        }
                    } catch {}
                } else {
                    Write-Warn "No TLS SNI observed in capture window (possibly resumed/HTTP2/QUIC or timing)."
                }
            }
        }
    } catch {
        Write-Warn ("tshark analysis error: {0}" -f $_.Exception.Message)
    }
}
