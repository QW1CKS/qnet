param(
    [Parameter(Mandatory = $true, HelpMessage = "Label for output pcap (e.g., qnet-stealth or chrome-baseline)")]
    [string]$Label,

    [int]$DurationSeconds = 60,

    [string]$Interface, # Optional: tshark interface index or name (e.g., 1 or "Wi-Fi")

    # Compute default output directory after param-binding for better PS 5.1 compatibility
    [string]$OutDir
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

function Require-Tool {
    param([string]$Name)
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        Write-Error "Required tool '$Name' not found in PATH. Install Wireshark (includes tshark) and ensure tshark.exe is on PATH."
    }
}

Require-Tool -Name 'tshark'

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
        Write-Warning "Npcap service is stopped and current shell is not elevated. Open PowerShell as Administrator and run: `nSet-Service -Name npcap -StartupType Automatic; Start-Service -Name npcap` then re-run this script."
    }
}

$ts = Get-Date -Format 'yyyyMMdd_HHmmss'
$outfile = Join-Path $OutDir ("$Label-$ts.pcapng")

# Capture filter: port 443 (TCP/UDP)
$filter = 'tcp port 443 or udp port 443'

Write-Host "Capturing $DurationSeconds s on port 443 -> $outfile"

if (-not $Interface) {
    Write-Host "No interface specified. Available interfaces:" -ForegroundColor Yellow
    tshark -D
    Write-Host "Tip: Re-run with -Interface <index> (from list above) for non-interactive use." -ForegroundColor Yellow
}

$args = @()
if ($Interface) { $args += @('-i', $Interface) }
$args += @('-f', $filter, '-a', "duration:$DurationSeconds", '-w', $outfile)

& tshark @args

Write-Host "Saved: $outfile"
Write-Host "Done."
