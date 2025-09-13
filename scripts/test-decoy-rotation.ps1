# Requires: stealth-browser built at target\debug\stealth-browser.exe
# Demonstrates DecoyDirect decoy rotation and last_decoy status updates.

param(
    [int]$SocksPort = 1081,
    [int]$StatusPort = 18080,
    [string]$OriginHost = 'wikipedia.org',
    [int]$OriginPort = 443,
    [string]$DecoyCatalogJson = '{"catalog":{"version":1,"updated_at":1726000001,"entries":[{"host_pattern":"wikipedia.org","decoy_host":"www.wikipedia.org","port":443,"weight":1},{"host_pattern":"wikipedia.org","decoy_host":"www.wikidata.org","port":443,"weight":1}]}}'
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

function Start-StealthBrowser {
    param([int]$SocksPort,[int]$StatusPort,[string]$DecoyCatalogJson)
    # Stop any previous instance
    Get-Process -Name stealth-browser -ErrorAction SilentlyContinue | Stop-Process -Force | Out-Null
    Start-Sleep -Milliseconds 200

    $env:STEALTH_MODE = 'decoy-direct'
    $env:STEALTH_STATUS_PORT = "$StatusPort"
    $env:STEALTH_SOCKS_PORT = "$SocksPort"
    $env:STEALTH_DECOY_CATALOG_JSON = $DecoyCatalogJson
    $env:STEALTH_DECOY_ALLOW_UNSIGNED = '1'

    $exe = Join-Path (Get-Location) 'target\debug\stealth-browser.exe'
    if (!(Test-Path $exe)) { throw "Missing $exe. Build first with cargo build -p stealth-browser" }

    $p = Start-Process -FilePath $exe -PassThru -WindowStyle Minimized
    # Wait for status server
    $deadline = (Get-Date).AddSeconds(5)
    do {
        try {
            $r = Invoke-WebRequest -Uri "http://127.0.0.1:$StatusPort/status" -UseBasicParsing -TimeoutSec 2
            if ($r.StatusCode -eq 200) { break }
        } catch { Start-Sleep -Milliseconds 200 }
    } while ((Get-Date) -lt $deadline)
    return $p
}

function Get-StatusJson { param([int]$StatusPort) (Invoke-WebRequest -Uri "http://127.0.0.1:$StatusPort/status" -UseBasicParsing).Content | ConvertFrom-Json }

function Socks-ConnectDomain {
    param([string]$Domain,[int]$Port,[int]$SocksPort)
    $client = New-Object System.Net.Sockets.TcpClient
    $client.Connect('127.0.0.1', $SocksPort)
    $s = $client.GetStream()
    # Greeting: VER=5, NMETHODS=1, METHOD=0x00
    $greet = [byte[]](5,1,0)
    $s.Write($greet,0,$greet.Length)
    $resp = New-Object byte[] 2
    [void]$s.Read($resp,0,2)
    # Build CONNECT request for DOMAIN
    $hb = [System.Text.Encoding]::ASCII.GetBytes($Domain)
    $pHi = [byte]($Port -shr 8)
    $pLo = [byte]($Port -band 0xFF)
    $req = New-Object System.Collections.Generic.List[byte]
    $req.AddRange([byte[]](5,1,0,3,[byte]$hb.Length))
    $req.AddRange($hb)
    $req.AddRange([byte[]]($pHi,$pLo))
    $arr = $req.ToArray()
    $s.Write($arr,0,$arr.Length)
    $rep = New-Object byte[] 10
    [void]$s.Read($rep,0,10)
    $s.Close(); $client.Close()
    return $rep
}

try {
    Write-Host "Starting stealth-browser (DecoyDirect)..."
    $proc = Start-StealthBrowser -SocksPort $SocksPort -StatusPort $StatusPort -DecoyCatalogJson $DecoyCatalogJson

    $origin = "https://${OriginHost}:${OriginPort}/"

    for ($i=1; $i -le 4; $i++) {
    Write-Host "[$i] CONNECT ${OriginHost}:${OriginPort} via SOCKS..."
    [void](Socks-ConnectDomain -Domain $OriginHost -Port $OriginPort -SocksPort $SocksPort)
        Start-Sleep -Milliseconds 300
        $j = Get-StatusJson -StatusPort $StatusPort
        "last_decoy=$($j.last_decoy) state=$($j.state)"
    }

    Write-Host "Done."
} finally {
    Get-Process -Name stealth-browser -ErrorAction SilentlyContinue | Stop-Process -Force | Out-Null
}
