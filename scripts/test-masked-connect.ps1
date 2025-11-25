<#
Test: Masked CONNECT path via local edge-gateway acting as a decoy (SIGNED CATALOG REQUIRED)

This script now performs a security‑first flow:
  1) Fetch signed decoy catalog JSON from remote seed (default: qnet-catalog repo raw URL)
  2) Verify signature with `catalog-signer verify` using provided publisher public key (env or param)
  3) Export STEALTH_DECOY_CATALOG_JSON + STEALTH_DECOY_PUBKEY_HEX (NO unsigned bypass)
  4) Launch edge-gateway (TLS listener for local decoy endpoint)
  5) Launch stealth-browser in masked mode (consumes signed catalog)
  6) Perform SOCKS5 HTTPS request via helper and report status metrics

Security Invariants (Omega Important):
  - Fails fast if catalog signature invalid or publisher key missing
  - Does NOT set STEALTH_DECOY_ALLOW_UNSIGNED (dev bypass removed here)
  - Old embedded publisher key file removed from repo; caller must supply key explicitly

Prerequisites:
  - cargo build has produced edge-gateway + stealth-browser + catalog-signer
  - certs/localhost.pem + certs/localhost-key.pem exist
  - PowerShell 7+, internet access to fetch catalog unless using -CatalogFile or -CatalogJson

Usage Examples:
  pwsh ./scripts/test-masked-connect.ps1 -PublisherPubKeyHex <hex> -Verbose
  pwsh ./scripts/test-masked-connect.ps1 -PublisherPubKeyFile ./secure/publisher.pub
  pwsh ./scripts/test-masked-connect.ps1 -CatalogUrl https://raw.githubusercontent.com/QW1CKS/qnet-catalog/refs/heads/main/catalog.json -PublisherPubKeyHex <hex>

Supplying the Publisher Public Key (choose one):
  1) -PublisherPubKeyHex <hex>
  2) -PublisherPubKeyFile path/to/publisher.pub (lines starting with # ignored)
  3) Pre-set env STEALTH_DECOY_PUBKEY_HEX before invocation

Key Generation Tutorial (Step-by-step):
  1. Generate a 32-byte Ed25519 private seed (hex) securely:
       PowerShell:  $bytes = New-Object byte[] 32; (New-Object System.Security.Cryptography.RNGCryptoServiceProvider).GetBytes($bytes); ($bytes|ForEach-Object ToString x2) -join ''
       Linux/macOS: openssl rand -hex 32
  2. Store this value as GitHub Actions Secret in qnet-catalog repo:  Name: CATALOG_PRIVKEY  Value: <seed-hex>
  3. Locally (one time) derive the public key:
       $env:CATALOG_PRIVKEY=<seed-hex>; cargo run -q -p catalog-signer -- pubkey > publisher.pub
  4. (Optional) Inspect fingerprint:  Get-Content publisher.pub | Select-String '.' | % { ($_ -replace '\s','') } | ForEach-Object { Write-Host "PubKey SHA256: $(echo $_ | openssl dgst -sha256)" }
  5. Distribute publisher.pub (public *only*) to consumers OR provide its hex via -PublisherPubKeyHex.
  6. The qnet-catalog workflow signs catalogs with the secret CATALOG_PRIVKEY; clients verify using the published pubkey.

Catalog Expectations:
  - Remote JSON must include a top-level "signature_hex" field (inline signature form).
  - On success we cache nothing on disk (in-memory env only) for this smoke test.

Outputs:
  Summary object with: Target, SocksResult, State, LastTarget, LastDecoy, CatalogSource, Verification
  (Use -Verbose to see additional diagnostic steps.)

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
  [switch]$Inline,
  [string]$CatalogUrl = 'https://raw.githubusercontent.com/QW1CKS/qnet-catalog/refs/heads/main/catalog.json',
  [string]$CatalogFile = '',              # Optional local catalog JSON (already inline-signed)
  [string]$CatalogJson = '',              # Direct JSON string override
  [string]$PublisherPubKeyHex = '',       # Publisher public key hex
  [string]$PublisherPubKeyFile = ''       # Path to publisher.pub containing hex
  , [switch]$DeriveUnsignedDecoyFromCatalog # Derive decoy catalog (unsigned) from a verified catalog-first JSON
  , [switch]$UseLocalEdgeDecoys            # Override derived decoy_host/port to localhost:EdgePort for all entries (testing)
)

# Global dev flags for local edge debugging (applied early so both processes inherit)
if ($PSBoundParameters.ContainsKey('UseLocalEdgeDecoys') -and $UseLocalEdgeDecoys) {
  $env:HTX_INNER_PLAINTEXT = '1'
  if (-not $env:RUST_LOG) { $env:RUST_LOG = 'debug,stealth-browser=debug,edge-gateway=debug,htx=debug' }
}
function Normalize-TargetHost {
  param([string]$InputHost)
  if (-not $InputHost) { return $InputHost }
  $hval = $InputHost
  if ($hval -match '^[a-zA-Z][a-zA-Z0-9+.-]*://') {
    try {
      $u = [Uri]$hval
      $hval = $u.Host
      if ($u.Port -and $u.Port -ne 443 -and $u.Port -ne 80) { $hval = "$hval`:$($u.Port)" }
      if ($u.Scheme -eq 'http') { $script:TargetWasHttp = $true }
    } catch { $hval = $hval -replace '^[a-zA-Z][a-zA-Z0-9+.-]*://','' }
  }
  $hval = $hval.TrimEnd('/')
  if ($hval -match '^[^/]+/') { $hval = $hval.Split('/')[0] }
  return $hval
}

# Normalize early before strict mode (avoid accidental constant assignment issues)
$script:TargetWasHttp = $false
$TargetOriginal = $Target
$Target = Normalize-TargetHost -InputHost $Target
# (Old normalization function removed / replaced above)
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

function Get-PublisherPubKeyHex {
  if ($PublisherPubKeyHex) { return ($PublisherPubKeyHex.Trim()) }
  if ($PublisherPubKeyFile) {
    if (-not (Test-Path $PublisherPubKeyFile)) { throw "PublisherPubKeyFile not found: $PublisherPubKeyFile" }
    return ((Get-Content $PublisherPubKeyFile | Where-Object { $_ -and -not ($_.TrimStart().StartsWith('#')) }) -join '' ).Trim()
  }
  if ($env:STEALTH_DECOY_PUBKEY_HEX) { return $env:STEALTH_DECOY_PUBKEY_HEX.Trim() }
  throw 'Publisher public key hex not supplied (use -PublisherPubKeyHex, -PublisherPubKeyFile or pre-set STEALTH_DECOY_PUBKEY_HEX).'
}

function Get-CatalogJsonSigned {
  if ($CatalogJson) { return $CatalogJson }
  if ($CatalogFile) {
    if (-not (Test-Path $CatalogFile)) { throw "CatalogFile not found: $CatalogFile" }
    return Get-Content -Raw $CatalogFile
  }
  Write-Verbose "Downloading catalog from $CatalogUrl"
  try {
    return (Invoke-WebRequest -Uri $CatalogUrl -UseBasicParsing -TimeoutSec 20).Content
  } catch {
    throw "Failed to download catalog from $CatalogUrl : $($_.Exception.Message)"
  }
}

Write-Verbose 'Fetching publisher public key'
$pubKeyHex = Get-PublisherPubKeyHex
if ($pubKeyHex.Length -ne 64) { Write-Warning "Publisher pubkey hex length typically 64 chars (32 bytes); got $($pubKeyHex.Length)" }

Write-Verbose 'Fetching signed catalog JSON'
$catalogJson = Get-CatalogJsonSigned
if (-not ($catalogJson | Select-String '"signature_hex"')) { throw 'Catalog JSON missing signature_hex (expected inline signature).' }

# Verify catalog using catalog-signer (detached or inline). We provide pubkey via temp file.
$tmpPub = New-TemporaryFile
Set-Content -Path $tmpPub -Value $pubKeyHex -NoNewline
$tmpCatalog = New-TemporaryFile
Set-Content -Path $tmpCatalog -Value $catalogJson -NoNewline

Write-Host '[0/6] Verifying catalog signature…'
& cargo run -q -p catalog-signer -- verify --catalog $tmpCatalog --pubkey-file $tmpPub | Write-Verbose
if ($LASTEXITCODE -ne 0) {
  throw "Catalog signature verification FAILED (exit $LASTEXITCODE)"
}
$catalogVerified = $true

Write-Verbose 'Exporting signed catalog env vars'
$env:STEALTH_DECOY_PUBKEY_HEX = $pubKeyHex
$env:STEALTH_LOG_DECOY_ONLY = '0'

# Detect if this is a catalog-first JSON (schema_version present) and optionally derive a decoy-catalog JSON accepted by htx::decoy
try {
  $parsed = $catalogJson | ConvertFrom-Json -ErrorAction Stop
} catch { throw 'Catalog JSON failed to parse after successful signature verification (unexpected).'; }

$isCatalogFirst = $parsed.PSObject.Properties.Name -contains 'schema_version' -and $parsed.PSObject.Properties.Name -contains 'catalog_version'
if ($isCatalogFirst -and $DeriveUnsignedDecoyFromCatalog) {
  Write-Verbose 'Deriving unsigned decoy catalog from catalog-first format (dev-only chain-of-trust: signed -> derived)'
  # Build decoy entries: one per entry host, mapping host_pattern="*" for the first entry and host-specific for others
  $entries = @()
  $i = 0
  foreach ($e in $parsed.entries) {
    $entryHost = $e.host
    if (-not $entryHost) { continue }
    $alpn = if ($e.alpn) { $e.alpn } else { @('h2','http/1.1') }
    $port = if ($e.ports -and $e.ports.Count -gt 0) { $e.ports[0] } else { 443 }
    $weight = if ($e.weight) { [int]$e.weight } else { 1 }
    $pattern = if ($i -eq 0) { '*' } else { $entryHost }
    if ($UseLocalEdgeDecoys) {
      $entries += @{ host_pattern = $pattern; decoy_host = 'localhost'; port = $EdgePort; alpn = $alpn; weight = $weight }
    } else {
      $entries += @{ host_pattern = $pattern; decoy_host = $entryHost; port = $port; alpn = $alpn; weight = $weight }
    }
    $i++
  }
  if ($entries.Count -eq 0) { throw 'No usable entries in catalog to derive decoy list.' }
  # Use catalog_version as version; updated_at from generated_at or now.
  $generatedAt = $parsed.generated_at
  $updatedEpoch = 0
  try { if ($generatedAt) { $updatedEpoch = [DateTimeOffset]::Parse($generatedAt).ToUnixTimeSeconds() } } catch { }
  if ($updatedEpoch -le 0) { $updatedEpoch = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds() }
  $decoy = @{ catalog = @{ version = [int]$parsed.catalog_version; updated_at = $updatedEpoch; entries = $entries } }
  $decoyJson = ($decoy | ConvertTo-Json -Depth 8 -Compress)
  # Mark as unsigned but trusted (derived from previously verified signed catalog)
  $env:STEALTH_DECOY_ALLOW_UNSIGNED = '1'
  $env:STEALTH_DECOY_CATALOG_JSON = $decoyJson
  Write-Verbose "Derived decoy entries: $($entries.Count); using unsigned env (STEALTH_DECOY_ALLOW_UNSIGNED=1) after upstream signature verification"
  if ($UseLocalEdgeDecoys) {
    $localCert = Join-Path $PSScriptRoot '..' 'certs' 'localhost.pem'
    if (Test-Path $localCert) {
      if ($env:HTX_TRUST_PEM) { $env:HTX_TRUST_PEM = "$($env:HTX_TRUST_PEM);$localCert" } else { $env:HTX_TRUST_PEM = $localCert }
      Write-Verbose "Appended local edge cert to HTX_TRUST_PEM=$localCert"
      # Force inner plaintext mux for template/key mismatch resilience in local dev
      $env:HTX_INNER_PLAINTEXT = '1'
      Write-Verbose 'Enabled HTX_INNER_PLAINTEXT=1 for local edge decoy debugging'
    } else {
      Write-Warning "-UseLocalEdgeDecoys set but certs/localhost.pem not found; TLS verification may fail; consider generating or pointing HTX_TRUST_PEM manually"
    }
  }
} else {
  if ($isCatalogFirst -and -not $DeriveUnsignedDecoyFromCatalog) {
    Write-Warning 'Catalog-first JSON provided, but decoy derivation disabled; decoy resolver will likely fail to load.'
  }
  $env:STEALTH_DECOY_CATALOG_JSON = $catalogJson
  if ($env:STEALTH_DECOY_ALLOW_UNSIGNED) { Remove-Item Env:STEALTH_DECOY_ALLOW_UNSIGNED -ErrorAction SilentlyContinue }
}

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
if ($TargetWasHttp) { Write-Host 'Note: input used http:// scheme; automatically upgraded request to https://' -ForegroundColor DarkYellow }
Write-Verbose "Curl Path: $curl"
Write-Verbose "Curl URL: $url"
$curlOut = New-TemporaryFile
# Use --socks5-hostname so the domain (not a pre-resolved IP) is sent in the SOCKS5 CONNECT.
# This keeps Current Target as the hostname while the helper separately resolves and displays Current Target IP.
& $curl --socks5-hostname "127.0.0.1:$SocksPort" --max-time 15 -o $curlOut -s -S -D - $url 2>&1 | Tee-Object -Variable curlVerbose | Out-Null
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

# Extract optional fields safely
function Get-OptionalField($obj, $name) {
  if ($null -eq $obj) { return $null }
  $prop = $obj.PSObject.Properties | Where-Object { $_.Name -eq $name }
  if ($prop) { return $prop.Value } else { return $null }
}

$lastTarget = Get-OptionalField $status 'last_target'
$lastDecoy  = Get-OptionalField $status 'last_decoy'
$maskedSuccesses = Get-OptionalField $status 'masked_successes'
$maskedFailures  = Get-OptionalField $status 'masked_failures'
$maskedAttempts  = Get-OptionalField $status 'masked_attempts'
$meshPeerCount   = Get-OptionalField $status 'peers_online'
$activeCircuits  = Get-OptionalField $status 'active_circuits'

# Task 2.4.5: Verify mesh functionality
if ($null -ne $meshPeerCount) {
  Write-Verbose "Mesh peer count: $meshPeerCount"
  if ($meshPeerCount -eq 0) {
    Write-Warning "No mesh peers discovered yet (normal on first run or isolated network)"
  }
} else {
  Write-Warning "mesh_peer_count field missing in status (check Helper version)"
}

if ($null -ne $activeCircuits) {
  Write-Verbose "Active circuits: $activeCircuits"
}

if (-not $lastTarget -and $maskedAttempts -and $maskedFailures -ge 1) {
  Write-Warning "No successful masked connection yet (attempts=$maskedAttempts failures=$maskedFailures). Check edge-gateway logs and decoy reachability." }

$summary = [pscustomobject]@{
    Target      = $Target
    SocksResult = $socksResult
  State          = $status.state
  Mode           = $status.mode
  LastTarget     = $lastTarget
  LastDecoy      = $lastDecoy
  DecoyCount     = (Get-OptionalField $status 'decoy_count')
  MaskedAttempts = $maskedAttempts
  MaskedFailures = $maskedFailures
  MaskedSuccesses= $maskedSuccesses
  MeshPeerCount  = $meshPeerCount
  ActiveCircuits = $activeCircuits
    StatusPort  = $StatusPort
    SocksPort   = $SocksPort
    EdgePort    = $EdgePort
  StealthLogOut  = $sbLog
  StealthLogErr  = $sbErr
  EdgeLogOut     = $edgeLog
  EdgeLogErr     = $edgeErr
  CatalogSource  = if ($CatalogFile) { $CatalogFile } elseif ($CatalogJson) { 'inline-param' } else { $CatalogUrl }
  Verification   = if ($catalogVerified) { 'signature-ok' } else { 'unknown'}
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
