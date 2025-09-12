param(
    [Parameter(Mandatory = $true, HelpMessage = "Label for output pcap (e.g., qnet-stealth or chrome-baseline)")]
    [string]$Label,

    [int]$DurationSeconds = 60,

    [string]$Interface, # Optional: tshark interface index or name (e.g., 1 or "Wi-Fi")

    # Compute default output directory after param-binding for better PS 5.1 compatibility
    [string]$OutDir,

    # Optional: full path to tshark.exe (helps when PATH is not propagated into Job runspaces)
    [string]$TsharkExe
)

$ErrorActionPreference = 'Stop'

# Fallback for $PSScriptRoot on older hosts or edge cases
if (-not $PSScriptRoot) {
    $PSScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Definition
}

# Resolve default OutDir if not provided
if (-not $OutDir -or [string]::IsNullOrWhiteSpace($OutDir)) {
    $repoRoot = Split-Path -Path (Split-Path -Path $PSScriptRoot -Parent) -Parent
    $OutDir = Join-Path -Path $repoRoot -ChildPath 'artifacts'
    $OutDir = Join-Path -Path $OutDir -ChildPath 'dpi'
}

if ($DurationSeconds -lt 1) {
    Write-Error "-DurationSeconds must be a positive integer (got: $DurationSeconds)"
}

function Resolve-Tshark {
    param([string]$Explicit)
    if ($Explicit -and (Test-Path -LiteralPath $Explicit)) { return $Explicit }
    $cmd = Get-Command 'tshark' -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    $candidates = @()
    if ($env:ProgramFiles) { $candidates += (Join-Path $env:ProgramFiles 'Wireshark\tshark.exe') }
    if (${env:ProgramFiles(x86)}) { $candidates += (Join-Path ${env:ProgramFiles(x86)} 'Wireshark\tshark.exe') }
    foreach ($p in $candidates) { if (Test-Path -LiteralPath $p) { return $p } }
    Write-Error "Required tool 'tshark' not found. Install Wireshark (includes tshark) and ensure tshark.exe is on PATH or pass -TsharkExe."
}

$tshark = Resolve-Tshark -Explicit $TsharkExe

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$svc = Get-Service -Name 'npcap' -ErrorAction SilentlyContinue
if (-not $svc) {
    Write-Warning "Npcap driver not found. Install Wireshark (include Npcap) or install Npcap from https://npcap.com, then re-run."
} elseif ($svc.Status -ne 'Running') {
    $isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltinRole]::Administrator)
    if ($isAdmin) {
        try {
            Write-Host "Starting Npcap service..." -ForegroundColor Yellow
            Start-Service -Name 'npcap'
            Start-Sleep -Seconds 1
        } catch {
            Write-Warning "Failed to start Npcap service automatically: $($_.Exception.Message). You may need to repair/reinstall Npcap."
        }
    } else {
        Write-Warning "Npcap service is stopped and current shell is not elevated. Open PowerShell as Administrator and run: Set-Service -Name npcap -StartupType Automatic; Start-Service -Name npcap then re-run this script."
    }
}

$ts = Get-Date -Format 'yyyyMMdd_HHmmss'
$outfile = Join-Path $OutDir ("$Label-$ts.pcapng")

# Capture filter: port 443 (TCP/UDP)
$filter = 'tcp port 443 or udp port 443'

Write-Host "Capturing $DurationSeconds s on port 443 -> $outfile"

if (-not $Interface) {
    Write-Host "No interface specified. Available interfaces:" -ForegroundColor Yellow
    $tmpOut = [System.IO.Path]::GetTempFileName()
    $tmpErr = [System.IO.Path]::GetTempFileName()
    Start-Process -FilePath $tshark -ArgumentList @('-D') -NoNewWindow -Wait -PassThru -RedirectStandardOutput $tmpOut -RedirectStandardError $tmpErr | Out-Null
    if (Test-Path -LiteralPath $tmpOut) { Get-Content -LiteralPath $tmpOut | Out-Host }
    if (Test-Path -LiteralPath $tmpErr) { Get-Content -LiteralPath $tmpErr | Out-Host }
    Remove-Item -Force -ErrorAction SilentlyContinue $tmpOut, $tmpErr
    Write-Host "Tip: Re-run with -Interface <index> (from list above) for non-interactive use." -ForegroundColor Yellow
}

# Build tshark argument list. Avoid using the automatic $args variable name.
$tsharkArgs = @()
if ($Interface) { $tsharkArgs += @('-i', "$Interface") }
# Pass capture filter only via -f to avoid duplicate-filter warnings.
$tsharkArgs += @('-f', "$filter", '-a', "duration:$DurationSeconds", '-w', "$outfile")

$tmpOut2 = [System.IO.Path]::GetTempFileName()
$tmpErr2 = [System.IO.Path]::GetTempFileName()
Start-Process -FilePath $tshark -ArgumentList $tsharkArgs -NoNewWindow -Wait -PassThru -RedirectStandardOutput $tmpOut2 -RedirectStandardError $tmpErr2 | Out-Null
if (Test-Path -LiteralPath $tmpOut2) { Get-Content -LiteralPath $tmpOut2 | Out-Host }
if (Test-Path -LiteralPath $tmpErr2) { Get-Content -LiteralPath $tmpErr2 | Out-Host }
Remove-Item -Force -ErrorAction SilentlyContinue $tmpOut2, $tmpErr2

Write-Host "Saved: $outfile"
Write-Host "Done."
