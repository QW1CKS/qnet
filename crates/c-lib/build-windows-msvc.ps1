# Build the Rust cdylib and compile the C example using MSVC cl
param(
    [ValidateSet('Debug','Release')]
    [string]$Configuration = "Release"
)

$ErrorActionPreference = "Stop"

# Build Rust library
$cargoArgs = @('build','--package','qnet_c')
if ($Configuration -ieq 'Release') { $cargoArgs += '--release' }
cargo @cargoArgs

# Locate artifacts (DLL and import library may live under target/<cfg>/deps)
$target = if ($Configuration -ieq "Release") { "release" } else { "debug" }
$artifactRoot = (Resolve-Path "..\..\target\$target").Path

$dll = Get-ChildItem -Path $artifactRoot -Recurse -File -Filter 'qnet_c.dll' -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $dll) { throw "qnet_c.dll not found under $artifactRoot (did cargo build succeed?)" }
$dllPath = $dll.FullName

# Rust on MSVC usually emits qnet_c.dll.lib (in deps); accept either name
$implib = Get-ChildItem -Path $artifactRoot -Recurse -File -Include 'qnet_c.lib','qnet_c.dll.lib' -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $implib) { throw "Import library not found under $artifactRoot (looked for qnet_c.lib and qnet_c.dll.lib)" }
$importLibPath = $implib.FullName

Write-Host "Using DLL:      $dllPath"
Write-Host "Using ImportLib: $importLibPath"

# Compile C example (force x64 toolchain)
$preferredCl = $null
if ($env:VCToolsInstallDir) {
    $candidate = Join-Path $env:VCToolsInstallDir "bin\HostX64\x64\cl.exe"
    if (Test-Path $candidate) { $preferredCl = $candidate }
}
if (-not $preferredCl) {
    # Fallback: try common Program Files location
    $glob = Get-ChildItem -Path "C:\Program Files (x86)\Microsoft Visual Studio\2022\*\VC\Tools\MSVC\*\bin\HostX64\x64\cl.exe" -ErrorAction SilentlyContinue | Select-Object -Last 1
    if ($glob) { $preferredCl = $glob.FullName }
}
if (-not $preferredCl) {
    $fallback = Get-Command cl.exe -ErrorAction SilentlyContinue
    if ($fallback) { $preferredCl = $fallback.Source }
}
if (-not $preferredCl) {
    throw "MSVC cl.exe (x64) not found. Open 'x64 Native Tools Command Prompt for VS 2022' and retry."
}
Write-Host "Using cl.exe:   $preferredCl"

$inc = (Resolve-Path ".\include").Path
$src = (Resolve-Path ".\examples\echo.c").Path
$OutDir = (Resolve-Path ".\examples").Path
Push-Location $OutDir
& "$preferredCl" /nologo /I "$inc" "$src" "$importLibPath" /link /MACHINE:X64 /out:echo.exe
$code = $LASTEXITCODE
Pop-Location

if ($code -ne 0 -or !(Test-Path (Join-Path $OutDir 'echo.exe'))) {
    throw "C example build failed (exit $code). Ensure you're using the x64 Native Tools shell."
}

# Copy the DLL next to the example so it can be loaded at runtime
Copy-Item -Force "$dllPath" -Destination "$OutDir"

Write-Host "Built examples/echo.exe and copied qnet_c.dll alongside it."
