# Windows perf baseline helper for T6.6
# Usage: Run in PowerShell on a dedicated Windows runner before executing benches
# Adjust paths to point to your code repo

Write-Host "Collecting Windows performance baseline..."

$cpu = Get-WmiObject Win32_Processor | Select-Object -Property Name, NumberOfCores, NumberOfLogicalProcessors, MaxClockSpeed
$mem = Get-WmiObject Win32_ComputerSystem | Select-Object -Property TotalPhysicalMemory
$os  = Get-WmiObject Win32_OperatingSystem | Select-Object -Property Caption, Version, BuildNumber

$hwProfile = [PSCustomObject]@{
  CPU   = $cpu
  MemoryBytes = $mem.TotalPhysicalMemory
  OS    = $os
  DateUTC = (Get-Date).ToUniversalTime().ToString("s")
}

$dir = "artifacts"
if (!(Test-Path $dir)) { New-Item -Path $dir -ItemType Directory | Out-Null }
$hwProfile | ConvertTo-Json -Depth 4 | Out-File -FilePath (Join-Path $dir "windows-hw-profile.json") -Encoding utf8

Write-Host "Hardware profile saved to artifacts/windows-hw-profile.json"
