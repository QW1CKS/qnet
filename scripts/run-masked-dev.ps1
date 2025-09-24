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
    [int]$TsharkDurationSec = 20,
    [ValidateSet('HEAD','RANGE','GET')]
    [string]$VerifyMethod = 'HEAD',
    [int]$VerifyMaxAttempts = 3,
    [int]$VerifyTimeoutSec = 30,
    [switch]$VerboseVerify,
    [int]$StatusWatchSec = 45,
    [int]$LogTailLines = 80,
    [switch]$ForceRestart,
    [switch]$BuildFirst,
    [switch]$NoPause
)

# --- Runtime environment guard: require PowerShell 7+ (Core) 64-bit ---
try {
    $psIsCore = $PSVersionTable.PSEdition -eq 'Core'
    $psMajorOk = $PSVersionTable.PSVersion.Major -ge 7
    $is64 = [IntPtr]::Size -eq 8
    if (-not ($psIsCore -and $psMajorOk -and $is64)) {
        Write-Host "[!] This script requires 64-bit PowerShell 7+. Detected: Edition=$($PSVersionTable.PSEdition) Version=$($PSVersionTable.PSVersion) Arch=$(([IntPtr]::Size * 8))" -ForegroundColor Yellow
        $pwshCmd = Get-Command 'pwsh' -ErrorAction SilentlyContinue
        if ($pwshCmd) {
            # Avoid infinite relaunch loop by passing a marker env var
            if (-not $env:QNET_MASKED_DEV_RELAUNCHED) {
                $env:QNET_MASKED_DEV_RELAUNCHED = '1'
                $argsList = @('-NoLogo','-NoProfile','-File', $PSCommandPath)
                if ($Url) { $argsList += @('-Url', $Url) }
                if ($SkipElevate) { $argsList += '-SkipElevate' }
                if ($Tshark) { $argsList += '-Tshark' }
                if ($TsharkInterface) { $argsList += @('-TsharkInterface', $TsharkInterface) }
                if ($TsharkDurationSec) { $argsList += @('-TsharkDurationSec', [string]$TsharkDurationSec) }
                Start-Process -FilePath $pwshCmd.Source -ArgumentList $argsList -WorkingDirectory (Get-Location) | Out-Null
                return
            }
        } else {
            Write-Host '[x] pwsh (PowerShell 7) not found in PATH. Please install PowerShell 7 x64 from https://github.com/PowerShell/PowerShell and retry.' -ForegroundColor Red
            exit 1
        }
    }
} catch {
    Write-Host '[!] Environment guard check failed (continuing): ' + $_.Exception.Message -ForegroundColor Yellow
}

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

# --- Optional cargo build (before any launches) ---
if ($BuildFirst) {
    Write-Info 'Building workspace (cargo build --bins)...'
    $buildSw = [System.Diagnostics.Stopwatch]::StartNew()
    & cargo build --bins 2>&1 | Tee-Object -Variable buildOut | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Err 'cargo build failed (showing last 40 lines):'
        ($buildOut | Select-Object -Last 40) | ForEach-Object { Write-Host $_ -ForegroundColor Red }
        exit 1
    }
    Write-Ok ("Build succeeded in {0:n1}s" -f $buildSw.Elapsed.TotalSeconds)
}

# --- Process cleanup (kill stale) ---
if ($ForceRestart) {
    Write-Info 'ForceRestart: terminating any existing edge-gateway / stealth-browser processes'
    $targets = Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Name -in @('edge-gateway','stealth-browser') }
    foreach ($p in $targets) {
        try { Write-Host ("Killing PID {0} ({1})" -f $p.Id, $p.Name) -ForegroundColor DarkYellow; $p.Kill() } catch { Write-Warn ("Kill failed PID {0}: {1}" -f $p.Id, $_.Exception.Message) }
    }
    Start-Sleep -Milliseconds 400
}

# --- Port availability pre-flight ---
function Test-PortFree($Port){
    try { $l = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, $Port); $l.Start(); $l.Stop(); return $true } catch { return $false }
}
$statusPort = 8088; $socksPort = 1088
foreach ($p in @($statusPort,$socksPort)) {
    if (-not (Test-PortFree $p)) {
        if ($ForceRestart) {
            Write-Warn ("Port {0} still in use after restart attempt; continuing but may hit bind failure." -f $p)
        } else {
            Write-Err ("Port {0} is already in use. Re-run with -ForceRestart or free the port." -f $p)
            exit 2
        }
    }
}

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
Write-Info ("Initial verify method: {0}" -f $VerifyMethod)

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
$edgeCmd = "Set-Location '{0}'; Write-Host 'Starting edge-gateway ...' -ForegroundColor Cyan; `$env:BIND = '127.0.0.1:4443'; `$env:HTX_TLS_CERT = '{1}'; `$env:HTX_TLS_KEY = '{2}'; `$env:NO_COLOR = '1'; `$env:RUST_LOG_STYLE = 'never'; `$env:RUST_LOG = 'info,htx=debug,edge_gateway=info'; `$env:RUST_BACKTRACE = '1'; `& '{3}' 2>&1 | Tee-Object -FilePath '{4}' -Append" -f $repo, $edgeCert, $edgeKey, $edgeExe, $edgeLog
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
$helperCmd = "Set-Location '{0}'; Write-Host 'Starting stealth-browser ...' -ForegroundColor Cyan; `$env:STEALTH_MODE = 'masked'; `$env:STEALTH_SOCKS_PORT = '1088'; `$env:STEALTH_STATUS_PORT = '8088'; `$env:STEALTH_DECOY_DEV_LOCAL = '1'; `$env:STEALTH_DECOY_ALLOW_UNSIGNED = '1'; `$env:STEALTH_DEBUG_STATUS = '1'; {1}; `$env:NO_COLOR = '1'; `$env:RUST_LOG_STYLE = 'never'; `$env:RUST_LOG = 'info,htx=debug,stealth_browser=info'; `$env:RUST_BACKTRACE = '1'; Write-Host ('Env STEALTH_MODE=' + `$env:STEALTH_MODE); `& '{2}' --mode=masked 2>&1 | Tee-Object -FilePath '{3}' -Append" -f $repo, $trustLine, $helperExe, $helperLog
Start-Process (Get-DefaultShellExe) -ArgumentList @('-NoExit','-NoLogo','-Command', $helperCmd) -WorkingDirectory $repo -WindowStyle Normal -ErrorAction Stop | Out-Null

if (-not (Wait-Port '127.0.0.1' 1088 'Helper SOCKS')) { Write-Warn 'Continuing anyway...' }
if (-not (Wait-Port '127.0.0.1' 8088 'Helper Status')) { Write-Warn 'Continuing anyway...' }

# Validate helper actually bound status server (inspect log quickly)
Write-Info 'Checking helper log for status server binding...'
$statusOk = $false; $statusBindErr = $false
for ($i=0; $i -lt 20; $i++) {
    if (Test-Path $helperLog) {
        $tail = Get-Content $helperLog -Tail 40 -ErrorAction SilentlyContinue
        # Accept either legacy phrase "status server listening" or current log form "status-server:bound"
        if ($tail -match 'status server listening' -or $tail -match 'status-server:bound') { $statusOk = $true; break }
        if ($tail -match 'status server bind failed') { $statusBindErr = $true; break }
    }
    Start-Sleep -Milliseconds 150
}
if (-not $statusOk) {
    if ($statusBindErr) {
        Write-Err 'Helper failed to bind status server (port likely in use by stale process). Use -ForceRestart.'
        exit 3
    } else {
        Write-Warn 'Did not detect status server listening line yet (continuing).'
    }
} else { Write-Ok 'Helper status server is listening.' }

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

Write-Info "Verifying via curl.exe through SOCKS (adaptive) (new window)... (logs: $verifyLog)"

# Build adaptive verification temp script
$verifyScript = Join-Path $sessionDir 'verify-run.ps1'
$methodsOrder = @('HEAD','RANGE','GET')
# Reorder based on requested initial method
if ($VerifyMethod -ne 'HEAD') {
    $methodsOrder = ,$VerifyMethod + ($methodsOrder | Where-Object { $_ -ne $VerifyMethod })
}

$curlBase = "curl.exe --no-progress-meter -sS --socks5-hostname 127.0.0.1:1088 -L -H 'User-Agent: qnet-dev/1.0' --retry 0"
if ($VerboseVerify) { $curlBase += ' -v' }

$methodsLiteral = ($methodsOrder | ForEach-Object { "'$_'" }) -join ','
$verifyBody = @'
Write-Host 'Adaptive verification starting for: __URL__' -ForegroundColor Cyan
$attempt = 0
$success = $false
$chosenCode = ''
$successMethod = ''
$methods = @(__METHODS__)
$maxAttempts = __MAX_ATTEMPTS__
$timeout = __TIMEOUT__
foreach ($m in $methods) {
    if ($success) { break }
    $attempt = 0
    switch ($m) {
        'HEAD'  { $args = '-I' }
        'RANGE' { $args = '-H "Range: bytes=0-2047"' }
        'GET'   { $args = '' }
    }
    while (-not $success -and $attempt -lt $maxAttempts) {
        $attempt++
        Write-Host ("[{0}] Attempt {1}/{2} method={3}" -f (Get-Date -Format HH:mm:ss), $attempt, $maxAttempts, $m) -ForegroundColor DarkCyan
        $cmd = "__CURL_BASE__ $args --connect-timeout 10 --max-time $timeout --write-out 'HTTP_CODE=%{http_code} TOTAL=%{time_total}\n' '__URL__'"
        Write-Host ('curl: ' + $cmd) -ForegroundColor DarkGray
        $out = Invoke-Expression $cmd 2>&1 | Tee-Object -FilePath '__VERIFY_LOG__' -Append
        $codeLine = ($out + (Get-Content '__VERIFY_LOG__' -Tail 5)) | Select-String -Pattern 'HTTP_CODE=' | Select-Object -Last 1
        if ($codeLine) {
             $mcode = ([regex]::Match($codeLine.ToString(),'HTTP_CODE=(\d{3})').Groups[1].Value)
             if ($mcode) { $chosenCode = $mcode; Write-Host ('Observed HTTP_CODE=' + $mcode) -ForegroundColor Yellow }
             if ($mcode -eq '200') { $success = $true; $successMethod = $m; break }
             if ($mcode -match '4..|5..') { Write-Host ('Non-success terminal code ' + $mcode + ' for method ' + $m + ' (not retrying this method)') -ForegroundColor Red; break }
        } else { Write-Host 'No HTTP_CODE parsed from output (timeout or connection issue)' -ForegroundColor Red }
    }
}
if ($success) { 
    if (-not $successMethod) { $successMethod = $m }
    Write-Host ('Verification SUCCESS with method ' + $successMethod + ' code=' + $chosenCode) -ForegroundColor Green 
    # Post-success: poll /status for up to 5s for state transition and target/decoy attribution
    $swPoll = [System.Diagnostics.Stopwatch]::StartNew()
    $seenTarget = $false
    while ($swPoll.Elapsed.TotalSeconds -lt 5) {
        try {
            $st = Invoke-RestMethod -Uri 'http://127.0.0.1:8088/status' -TimeoutSec 2
            if ($st) {
                if ($st.state -eq 'connected') {
                    Write-Host ("Status now connected (target={0} decoy={1})" -f ($st.current_target, $st.current_decoy)) -ForegroundColor Green
                    $seenTarget = $true; break
                } elseif ($st.current_target -or $st.last_target) {
                    Write-Host ("Status target observed (state={0} target={1} decoy={2})" -f $st.state, ($st.current_target,$st.last_target -ne $null | Select -First 1), ($st.current_decoy,$st.last_decoy -ne $null | Select -First 1)) -ForegroundColor Yellow
                    $seenTarget = $true; break
                }
            }
        } catch {}
        Start-Sleep -Milliseconds 300
    }
    if (-not $seenTarget) { Write-Host 'Did not observe connected state within 5s (check helper window for transition markers).' -ForegroundColor Yellow }
} else { Write-Host ('Verification FAILED. Last code=' + $chosenCode) -ForegroundColor Red }
$nl = [Environment]::NewLine; Write-Host ($nl + 'Done. Press Enter to close this window.') -ForegroundColor Yellow
Read-Host
'@
$verifyBody = $verifyBody.Replace('__URL__', $Url)
$verifyBody = $verifyBody.Replace('__METHODS__', $methodsLiteral)
$verifyBody = $verifyBody.Replace('__MAX_ATTEMPTS__', [string]$VerifyMaxAttempts)
$verifyBody = $verifyBody.Replace('__TIMEOUT__', [string]$VerifyTimeoutSec)
$verifyBody = $verifyBody.Replace('__CURL_BASE__', $curlBase)
$verifyBody = $verifyBody.Replace('__VERIFY_LOG__', $verifyLog)
$verifyBody | Set-Content -Path $verifyScript -Encoding UTF8
Start-Process (Get-DefaultShellExe) -ArgumentList @('-NoExit','-NoLogo','-File', $verifyScript) -WorkingDirectory $repo -WindowStyle Normal -ErrorAction Stop | Out-Null

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

# --- Automatic status watch / diagnostics ---
function Invoke-StatusWatch {
    param(
        [int]$Seconds,
        [int]$TailLines,
        [string]$HelperLogPath
    )
    if ($Seconds -le 0) { return }
    Write-Info ("Watching /status for {0}s ..." -f $Seconds)
    $prevState = $null
    $prevErr = $null
    $offlinePersistStart = $null
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $failCount = 0
    while ($sw.Elapsed.TotalSeconds -lt $Seconds) {
        try {
            $s = Invoke-RestMethod -Uri 'http://127.0.0.1:8088/status' -TimeoutSec 3
            if ($s) {
                $failCount = 0
                $st = $s.state
                $err = $s.last_update.error
                if ($st -ne $prevState -or $err -ne $prevErr) {
                    Write-Host ("[status] t={0}s state={1} peers={2} err={3}" -f [int]$sw.Elapsed.TotalSeconds, $st, $s.peers_online, (if ($err) { $err } else { '-' })) -ForegroundColor DarkCyan
                    $prevState = $st; $prevErr = $err
                }
                if (-not $st) {
                    Write-Warn 'Status JSON lacks state field; dumping raw object keys.'
                    ($s | Get-Member -MemberType NoteProperty | Select-Object -ExpandProperty Name) | ForEach-Object { Write-Host ("  key: $_") -ForegroundColor DarkGray }
                }
                if ($st -eq 'offline') {
                    if (-not $offlinePersistStart) { $offlinePersistStart = $sw.Elapsed }
                    elseif (($sw.Elapsed - $offlinePersistStart).TotalSeconds -ge 10 -and $err) {
                        Write-Warn ('State offline for >=10s with error=' + $err + ' — tailing helper log...')
                        if (Test-Path $HelperLogPath) {
                            Get-Content $HelperLogPath -Tail $TailLines | ForEach-Object { Write-Host ('[tail] ' + $_) -ForegroundColor DarkGray }
                        }
                        $offlinePersistStart = $sw.Elapsed.Add([TimeSpan]::FromSeconds(9999)) # avoid re-tail spam
                    }
                }
                elseif ($st -eq 'online' -or $st -eq 'connected') {
                    Write-Ok ("Status transitioned to {0}; stopping watch." -f $st)
                    return
                }
            }
        } catch {
            $failCount++
            if ($failCount -le 5 -or ($failCount % 10) -eq 0) {
                Write-Warn ("Status fetch failed (count={0}): {1}" -f $failCount, $_.Exception.Message)
            }
        }
        Start-Sleep -Milliseconds 900
    }
    Write-Warn 'Status watch ended without reaching online/connected state.'
}

Invoke-StatusWatch -Seconds $StatusWatchSec -TailLines $LogTailLines -HelperLogPath $helperLog
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

# Persistent hold so the invoking window never closes unless user opts out
if (-not $NoPause) {
    Write-Host "=== Session hold active. Press Ctrl+C or type 'exit' to terminate. ===" -ForegroundColor Magenta
    while ($true) {
        Write-Host -NoNewline '(qnet-dev) > ' -ForegroundColor DarkGray
        $line = Read-Host
        if ($line -match '^(exit|quit|q)$') { break }
        if ($line -match '^tail\s*') {
            $parts = $line.Split(' ',2)
            $target = if ($parts.Count -gt 1 -and $parts[1]) { $parts[1] } else { $helperLog }
            if (Test-Path $target) { Get-Content $target -Tail 40 } else { Write-Warn "File not found: $target" }
        } elseif ($line -match '^status$') {
            try { Invoke-RestMethod -Uri 'http://127.0.0.1:8088/status' -TimeoutSec 3 | ConvertTo-Json -Depth 4 } catch { Write-Warn $_.Exception.Message }
        } elseif ($line -match '^help$') {
            Write-Host "Commands: exit|quit|q, status, tail [path]" -ForegroundColor DarkGray
        } elseif ($line.Trim()) {
            Write-Warn 'Unknown command. Type help.'
        }
    }
}
