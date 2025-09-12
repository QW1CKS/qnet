[CmdletBinding(PositionalBinding=$false)]
param(
  [switch]$WithDecoy,
  [int]$CaptureSeconds = 0,
  [int]$Interface = -1,
  [string]$Origin = "https://example.com",
  [string]$SeedsJsonPath,
  [string]$SeedsList,
  [string[]]$SeedUrls
)

$ErrorActionPreference = 'Stop'

Write-Host "[1/5] Validating bootstrap env..."
# Normalize -SeedsList (string) into -SeedUrls for PS5.1-friendly list input
if (-not $SeedUrls -and $SeedsList) {
  # Split on comma or whitespace
  $SeedUrls = @()
  foreach ($tok in ($SeedsList -split '[,\s]+' | Where-Object { $_ -and $_.Trim().Length -gt 0 })) {
    $SeedUrls += $tok
  }
}
if (-not $env:STEALTH_BOOTSTRAP_CATALOG_JSON) {
  # Allow passing seeds via parameters or a seeds.json file
  $epoch = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
  if ($SeedUrls -and $SeedUrls.Length -gt 0) {
    $entries = @()
    foreach ($u in $SeedUrls) { $entries += @{ url = "$u" } }
    $catalog = @{ catalog = @{ version = 1; updated_at = [int64]$epoch; entries = $entries } } | ConvertTo-Json -Compress -Depth 6
    $env:STEALTH_BOOTSTRAP_CATALOG_JSON = $catalog
    if (-not $env:STEALTH_BOOTSTRAP_ALLOW_UNSIGNED) { $env:STEALTH_BOOTSTRAP_ALLOW_UNSIGNED = '1' }
    Write-Host "  Set STEALTH_BOOTSTRAP_CATALOG_JSON from -SeedUrls"
  } elseif ($SeedsJsonPath -and (Test-Path -LiteralPath $SeedsJsonPath)) {
    $raw = Get-Content -LiteralPath $SeedsJsonPath -Raw
    try {
      $js = $raw | ConvertFrom-Json
      if ($js.catalog -and $js.catalog.entries) {
        $env:STEALTH_BOOTSTRAP_CATALOG_JSON = $raw
  Write-Host "  Loaded catalog from ${SeedsJsonPath}"
      } elseif ($js -is [System.Array]) {
        $entries = @()
        foreach ($u in $js) { $entries += @{ url = "$u" } }
        $catalog = @{ catalog = @{ version = 1; updated_at = [int64]$epoch; entries = $entries } } | ConvertTo-Json -Compress -Depth 6
        $env:STEALTH_BOOTSTRAP_CATALOG_JSON = $catalog
  Write-Host "  Built catalog from URL array in ${SeedsJsonPath}"
      } else {
  throw "Unsupported JSON format in ${SeedsJsonPath}"
      }
      if (-not $env:STEALTH_BOOTSTRAP_ALLOW_UNSIGNED) { $env:STEALTH_BOOTSTRAP_ALLOW_UNSIGNED = '1' }
    } catch {
  throw "Failed to parse ${SeedsJsonPath}: $($_.Exception.Message)"
    }
  } elseif (Test-Path -LiteralPath (Join-Path (Get-Location) 'seeds.json')) {
    $SeedsJsonPath = (Join-Path (Get-Location) 'seeds.json')
  Write-Host "  Found seeds.json at ${SeedsJsonPath}"
    $raw = Get-Content -LiteralPath $SeedsJsonPath -Raw
    try {
      $js = $raw | ConvertFrom-Json
      if ($js.catalog -and $js.catalog.entries) {
        $env:STEALTH_BOOTSTRAP_CATALOG_JSON = $raw
      } elseif ($js -is [System.Array]) {
        $entries = @()
        foreach ($u in $js) { $entries += @{ url = "$u" } }
        $catalog = @{ catalog = @{ version = 1; updated_at = [int64]$epoch; entries = $entries } } | ConvertTo-Json -Compress -Depth 6
        $env:STEALTH_BOOTSTRAP_CATALOG_JSON = $catalog
      } else {
        throw "Unsupported JSON format in seeds.json"
      }
      if (-not $env:STEALTH_BOOTSTRAP_ALLOW_UNSIGNED) { $env:STEALTH_BOOTSTRAP_ALLOW_UNSIGNED = '1' }
    } catch {
      throw "Failed to parse seeds.json: $($_.Exception.Message)"
    }
  } else {
    throw "STEALTH_BOOTSTRAP_CATALOG_JSON not set. Provide -SeedUrls https://a https://b, or -SeedsJsonPath .\\seeds.json, or export the env var."
  }
}
if ($env:STEALTH_BOOTSTRAP_ALLOW_UNSIGNED -ne '1' -and -not $env:STEALTH_BOOTSTRAP_PUBKEY_HEX) {
  Write-Warning "Unsigned bootstrap not enabled and no pubkey provided; continuing, but signed catalog is recommended."
}

# Quick probe of seed URLs
$urls = @( ($env:STEALTH_BOOTSTRAP_CATALOG_JSON | ConvertFrom-Json).catalog.entries | % url )
foreach ($u in $urls) {
  try {
    $r = Invoke-WebRequest $u -UseBasicParsing -TimeoutSec 10
    Write-Host "  OK  $u -> $($r.StatusCode)"
  } catch {
    Write-Warning "  ERR $u -> $($_.Exception.Message)"
  }
}

Write-Host "[2/5] Bootstrap smoke test..."
& cargo run -q -p htx --example bootstrap_check | Write-Host

if ($WithDecoy) {
  Write-Host "[3/5] Enabling decoy routing..."
  if (-not $env:STEALTH_DECOY_CATALOG_JSON) {
    $env:STEALTH_DECOY_ALLOW_UNSIGNED = '1'
    $env:STEALTH_DECOY_CATALOG_JSON = '{"catalog":{"version":1,"updated_at":1726128000,"entries":[{"host_pattern":"*","decoy_host":"www.cloudflare.com","port":443,"alpn":["h2","http/1.1"],"weight":1}]}}'
  }
  if (-not $env:STEALTH_LOG_DECOY_ONLY) { $env:STEALTH_LOG_DECOY_ONLY = '1' }
}

$capJob = $null
$capScript = Join-Path (Get-Location) 'qnet-spec\templates\dpi-capture.ps1'
$tsharkGuess = $null
try {
  $cmd = Get-Command 'tshark' -ErrorAction SilentlyContinue
  if ($cmd) { $tsharkGuess = $cmd.Source }
} catch {}
if (-not $tsharkGuess) {
  $pf = $env:ProgramFiles
  $pf86 = ${env:ProgramFiles(x86)}
  $cands = @()
  if ($pf) { $cands += (Join-Path $pf 'Wireshark\tshark.exe') }
  if ($pf86) { $cands += (Join-Path $pf86 'Wireshark\tshark.exe') }
  foreach ($p in $cands) { if (Test-Path -LiteralPath $p) { $tsharkGuess = $p; break } }
}
if ($CaptureSeconds -gt 0) {
  Write-Host "[4/5] Starting DPI capture for $CaptureSeconds s..."
  if ($Interface -lt 0) {
    Write-Warning "No interface index provided; capturing may fail. Use tshark -D to find index."
  }
  $capJob = Start-Job -ScriptBlock {
    param($label,$secs,$iface,$scriptPath,$tsharkPath)
    $exe = Join-Path $PSHOME 'powershell.exe'
    if (-not (Test-Path -LiteralPath $exe)) { $exe = 'powershell.exe' }
    & $exe -NoProfile -ExecutionPolicy Bypass -File $scriptPath -Label $label -DurationSeconds $secs -Interface $iface -TsharkExe $tsharkPath
  } -ArgumentList @('qnet-stealth', $CaptureSeconds, $Interface, $capScript, $tsharkGuess)
}

Write-Host "[5/5] Running secure dial demo..."
& cargo run -q -p htx --features rustls-config --example dial_tls_demo -- $Origin | Write-Host

if ($capJob) {
  $jobOut = Receive-Job -Job $capJob -Wait -AutoRemoveJob -ErrorAction Continue
  if ($jobOut) { $jobOut | Out-Host }
  Write-Host "Capture complete. To compare vs Chrome baseline:"
  Write-Host "  py .\\qnet-spec\\templates\\dpi-compare.py artifacts\\dpi\\qnet-stealth-*.pcapng artifacts\\dpi\\chrome-baseline-*.pcapng"
}

Write-Host "Done."
