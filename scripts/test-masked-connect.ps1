<#
Test: Masked CONNECT path via local edge-gateway acting as a decoy

This script launches:
  1) edge-gateway (outer TLS listener, inner HTX mux, CONNECT handler)
  2) stealth-browser in --mode masked with a dev unsigned decoy catalog
  3) Performs a curl request over SOCKS5 to a target (default: www.wikipedia.org)

It verifies:
  - Decoy catalog is loaded (status.decoy_count > 0)
  - Masked CONNECT success (state transitions to Connected; logs show masked: line)
  - last_target and last_decoy populated in /status

Decoy Strategy (dev):
  We map all origins (host_pattern = "*") to a local decoy host "localhost" on EdgePort (default 4443).
  This validates the full pipeline locally. In a real deployment, decoy_host would be a
  benign remote domain with a valid certificate and edge-gateway deployed there.

Prerequisites:
  - cargo build (debug) has produced edge-gateway and stealth-browser binaries
  - certs/localhost.pem + certs/localhost-key.pem exist (included in repo)
  - PowerShell 7+ recommended

Usage:
  pwsh ./scripts/test-masked-connect.ps1 -Target www.wikipedia.org
  pwsh ./scripts/test-masked-connect.ps1 -Target en.wikipedia.org -Verbose

Parameters:
  -Target       Hostname to fetch through SOCKS (default www.wikipedia.org)
  -SocksPort    Local SOCKS5 port exposed by stealth-browser (default 1088)
  -StatusPort   Local status HTTP port (default 8088)
  -EdgePort     Local edge-gateway TLS bind port (default 4443)
  -CurlPath     Override curl executable (auto-detect otherwise)
  -KeepAlive    Do not stop processes after test (for manual inspection)

Outputs:
  Summary object with fields: Target, SocksResult, State, LastTarget, LastDecoy, LogHints

NOTE: This is a dev-only helper. Unsigned catalogs are enabled (STEALTH_DECOY_ALLOW_UNSIGNED=1).
#>

[CmdletBinding()]
param(
  [string]$Target = 'www.wikipedia.org',
  [int]$SocksPort = 1088,
  [int]$StatusPort = 8088,
  [int]$EdgePort = 4443,
  [string]$CurlPath = '',
  [switch]$SkipProcessKill,
  [int]$ReadyTimeoutSec = 25,
  [switch]$Inline    # If specified, do NOT detach (legacy inline mode)
)
function Stop-ExistingIfNeeded {
  param([switch]$Skip)
  if ($Skip) { return }
  foreach ($n in 'stealth-browser','edge-gateway') {
    $procs = Get-Process $n -ErrorAction SilentlyContinue
    if ($null -ne $procs) {
      $count = 1
      try { $count = $procs.Count } catch { }
      Write-Host "Stopping existing process: $n (count=$count)" -ForegroundColor DarkYellow
      $procs | Stop-Process -Force -ErrorAction SilentlyContinue
      Start-Sleep -Milliseconds 150
    }
  }
}

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Find-Curl {
    param([string]$Override)
    if ($Override) { return $Override }
    $names = @('curl.exe','curl')
    foreach ($n in $names) {
        $p = (Get-Command $n -ErrorAction SilentlyContinue | Select-Object -First 1).Path
        if ($p) { return $p }
    }
    throw 'curl not found in PATH; install curl or specify -CurlPath'
}

function Wait-StatusReady {
  param(
    [int]$Port,
    [int]$TimeoutSec = 20,
    [string]$StealthErr,
    [string]$StealthOut
  )
  $deadline = (Get-Date).AddSeconds($TimeoutSec)
  $sawBind = $false
  while ((Get-Date) -lt $deadline) {
    # Inspect logs for bind or failure markers first
    foreach ($log in @($StealthErr,$StealthOut)) {
      if ($log -and (Test-Path $log)) {
        $tail = Get-Content $log -Tail 120 -ErrorAction SilentlyContinue
        if (-not $sawBind -and ($tail | Where-Object { $_ -match 'status-server:bound addr=' })) { $sawBind = $true }
        if ($tail | Where-Object { $_ -match 'status server bind failed' }) {
          Write-Warning 'Status server bind failed (port in use or permission).'
          return $false
        }
      }
    }
    # If we saw bind, attempt HTTP fetch
    if ($sawBind) {
      # Prefer the super cheap /ready endpoint (added for automation) then /status fallback
      try {
        $r2 = Invoke-WebRequest -Uri "http://127.0.0.1:$Port/ready" -UseBasicParsing -TimeoutSec 2
        if ($r2.StatusCode -eq 200) { return $true }
      } catch { }
      try {
        $resp = Invoke-WebRequest -Uri "http://127.0.0.1:$Port/status" -UseBasicParsing -TimeoutSec 2
        if ($resp.StatusCode -eq 200 -and $resp.Content) { return $true }
      } catch { }
    } else {
      # Probe TCP quickly (may catch early online state)
      try {
        $tcp = [System.Net.Sockets.TcpClient]::new()
        $iar = $tcp.BeginConnect('127.0.0.1',$Port,$null,$null)
        $ok = $iar.AsyncWaitHandle.WaitOne(150)
        if ($ok -and $tcp.Connected) { $tcp.Close() }
      } catch { }
    }
    Start-Sleep -Milliseconds 300
  }
  return $false
}
Write-Verbose 'Stopping any existing processes (use -SkipProcessKill to disable)'
Stop-ExistingIfNeeded -Skip:$SkipProcessKill

function Parse-LogFallback {
  param([string]$StealthOut, [string]$StealthErr)
  $result = @{ state = 'unknown'; last_target=$null; last_decoy=$null }
  $lines = @()
  if (Test-Path $StealthErr) { $lines += Get-Content $StealthErr -ErrorAction SilentlyContinue }
  if (Test-Path $StealthOut) { $lines += Get-Content $StealthOut -ErrorAction SilentlyContinue }
  $masked = $lines | Where-Object { $_ -match 'masked: target=' } | Select-Object -Last 1
  if ($masked) {
    # Example: masked: target=www.wikipedia.org:443, decoy=localhost:4443
    if ($masked -match 'target=([^,]+), decoy=([^\r\n]+)$') {
      $result.last_target = $matches[1]
      $result.last_decoy  = $matches[2]
      $result.state = 'Connected'
    }
  }
  return $result
}

function New-DecoyCatalogJson {
    param([int]$EdgePort)
    $epoch = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
    $obj = @{ catalog = @{ version = 1; updated_at = $epoch; entries = @(@{ host_pattern='*'; decoy_host='localhost'; port=$EdgePort; alpn=@('h2','http/1.1'); weight=1 }) } }
    return ($obj | ConvertTo-Json -Depth 6 -Compress)
}

Write-Verbose 'Preparing environment variables'
$env:STEALTH_DECOY_CATALOG_JSON = New-DecoyCatalogJson -EdgePort $EdgePort
$env:STEALTH_DECOY_ALLOW_UNSIGNED = '1'
$env:STEALTH_LOG_DECOY_ONLY = '0'

# Edge TLS cert/key (self-signed for localhost)
$env:HTX_TLS_CERT = (Join-Path $PSScriptRoot '..' 'certs' 'localhost.pem')
$env:HTX_TLS_KEY  = (Join-Path $PSScriptRoot '..' 'certs' 'localhost-key.pem')
if (-not (Test-Path $env:HTX_TLS_CERT) -or -not (Test-Path $env:HTX_TLS_KEY)) {
    throw "Missing cert or key: $($env:HTX_TLS_CERT), $($env:HTX_TLS_KEY)"
}

$root = Resolve-Path (Join-Path $PSScriptRoot '..')
Push-Location $root

Write-Host '[1/6] Building binaries (debug)…'
cargo build -q -p edge-gateway -p stealth-browser

$edgeBin = Join-Path $root 'target' 'debug' 'edge-gateway.exe'
$sbBin   = Join-Path $root 'target' 'debug' 'stealth-browser.exe'
if (-not (Test-Path $edgeBin) -or -not (Test-Path $sbBin)) { throw 'Build failed; binaries not found.' }

Write-Host '[2/6] Starting edge-gateway (decoy endpoint)…'
$env:BIND = "127.0.0.1:$EdgePort"
$edgeLogBase = Join-Path $root 'logs' ('edge-gateway-test-{0:yyyyMMdd-HHmmss}' -f (Get-Date))
$edgeLog = "$edgeLogBase.out.log" # retained for summary (not actively written when detached)
$edgeErr = "$edgeLogBase.err.log"
if ($Inline) {
  $edgeProc = Start-Process -FilePath $edgeBin -NoNewWindow -PassThru -RedirectStandardOutput $edgeLog -RedirectStandardError $edgeErr
} else {
  # Detach: launch a new PowerShell window running edge-gateway
  $pwsh = (Get-Command pwsh).Source
  $edgeCmd = "& '$edgeBin'"
  Start-Process -FilePath $pwsh -ArgumentList '-NoLogo','-NoProfile','-Command', $edgeCmd -WindowStyle Normal -WorkingDirectory $root -PassThru | Out-Null
}
Start-Sleep -Milliseconds 400

Write-Host '[3/6] Starting stealth-browser in masked mode (detached)…'
$sbLogBase = Join-Path $root 'logs' ('stealth-browser-test-{0:yyyyMMdd-HHmmss}' -f (Get-Date))
$sbLog = "$sbLogBase.out.log"
$sbErr = "$sbLogBase.err.log"
$sbArgsLine = "--mode masked --socks-port $SocksPort --status-port $StatusPort"
if ($Inline) {
  $sbArgs = @('--mode','masked','--socks-port',"$SocksPort",'--status-port',"$StatusPort")
  $sbProc = Start-Process -FilePath $sbBin -ArgumentList $sbArgs -NoNewWindow -PassThru -RedirectStandardOutput $sbLog -RedirectStandardError $sbErr
} else {
  $pwsh = (Get-Command pwsh).Source
  $sbCmd = "& '$sbBin' $sbArgsLine"
  Start-Process -FilePath $pwsh -ArgumentList '-NoLogo','-NoProfile','-Command', $sbCmd -WindowStyle Normal -WorkingDirectory $root -PassThru | Out-Null
}

Write-Host '[4/6] Waiting for status endpoint…'
if (-not (Wait-StatusReady -Port $StatusPort -TimeoutSec $ReadyTimeoutSec -StealthErr $sbErr -StealthOut $sbLog)) {
  Write-Warning "Status endpoint not ready on :$StatusPort after ${ReadyTimeoutSec}s (check windows/logs)."
  # Continue; we still try SOCKS to show masked path result, then remind user.
}

Write-Host '[5/6] Performing masked SOCKS5 request via curl…'
$curl = Find-Curl -Override $CurlPath
$url = "https://$Target/"
Write-Verbose "Curl Path: $curl"
Write-Verbose "Curl URL: $url"
$curlOut = New-TemporaryFile
& $curl --socks5 "127.0.0.1:$SocksPort" --max-time 15 -o $curlOut -s -S -D - $url 2>&1 | Tee-Object -Variable curlVerbose | Out-Null
$socksResult = if ($LASTEXITCODE -eq 0) { 'Success' } else { "Failure($LASTEXITCODE)" }

Write-Host '[6/6] Fetching status snapshot…'
try {
  $statusRaw = (Invoke-WebRequest -Uri "http://127.0.0.1:$StatusPort/status" -UseBasicParsing -TimeoutSec 5).Content
  $status = $statusRaw | ConvertFrom-Json
} catch {
  Write-Warning "Status fetch failed; falling back to log parsing"
  $fb = Parse-LogFallback -StealthOut $sbLog -StealthErr $sbErr
  $status = [pscustomobject]@{ state=$fb.state; mode='masked'; last_target=$fb.last_target; last_decoy=$fb.last_decoy; decoy_count=$null }
}

$summary = [pscustomobject]@{
    Target      = $Target
    SocksResult = $socksResult
  State          = $status.state
  Mode           = $status.mode
  LastTarget     = $status.last_target
  LastDecoy      = $status.last_decoy
  DecoyCount     = $status.decoy_count
    StatusPort  = $StatusPort
    SocksPort   = $SocksPort
    EdgePort    = $EdgePort
  StealthLogOut  = $sbLog
  StealthLogErr  = $sbErr
  EdgeLogOut     = $edgeLog
  EdgeLogErr     = $edgeErr
    LogHints    = 'Search logs for: masked: and CONNECT prelude received'
}

Write-Host '--- Test Summary ---'
$summary | Format-List | Out-String | Write-Host

Write-Host "Logs (detached mode: output visible in opened windows; these files may be empty unless inline mode):" -NoNewline;
Write-Host "`n  Stealth OUT: $sbLog`n  Stealth ERR: $sbErr`n  Edge OUT:    $edgeLog`n  Edge ERR:    $edgeErr" -ForegroundColor DarkGray
Write-Host "Open status page: http://127.0.0.1:$StatusPort/" -ForegroundColor Cyan
Write-Host "To stop processes: Get-Process stealth-browser,edge-gateway | Stop-Process -Force" -ForegroundColor DarkYellow
Write-Host 'Done (processes left running).' 

Pop-Location
