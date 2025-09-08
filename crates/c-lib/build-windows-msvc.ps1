# Build the Rust cdylib and compile the C example using MSVC cl
param(
    [string]$Configuration = "Release"
)

$ErrorActionPreference = "Stop"

# Build Rust library
cargo build --package qnet_c --$Configuration

# Locate artifacts
$target = if ($Configuration -eq "Release") { "release" } else { "debug" }
$libPath = Join-Path -Path (Resolve-Path "..\..\target\$target").Path -ChildPath "qnet_c.dll"
if (!(Test-Path $libPath)) {
    throw "DLL not found: $libPath"
}

# Compile C example
$cl = Get-Command cl.exe -ErrorAction SilentlyContinue
if (-not $cl) { throw "MSVC cl.exe not found in PATH. Open a 'x64 Native Tools Command Prompt for VS'." }

$inc = Resolve-Path ".\include"
$src = Resolve-Path ".\examples\echo.c"
$OutDir = Resolve-Path ".\examples"
Push-Location $OutDir
cl /nologo /I "$inc" "$src" /link /out:echo.exe "$libPath"
Pop-Location

Write-Host "Built echo.exe. Ensure qnet_c.dll is in the same directory or in PATH when running."
