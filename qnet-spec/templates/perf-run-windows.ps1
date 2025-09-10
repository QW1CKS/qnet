# PowerShell perf run helper for T6.6 (Windows)
# Usage: .\perf-run-windows.ps1 -Features "perf-bench,quic"
param(
  [string]$Features = "perf-bench"
)

Write-Host "[perf] Collecting hardware profile..."
if (!(Test-Path artifacts)) { New-Item -Path artifacts -ItemType Directory | Out-Null }
Get-ComputerInfo | Out-File -FilePath artifacts\windows-hw-profile.txt -Encoding utf8

Write-Host "[perf] Running benches with features: $Features"
$env:RUSTFLAGS = "-C target-cpu=native"
# Note: assumes Rust toolchain and repo-ready benches exist in the code repo
cargo bench --features $Features --no-default-features
if ($LASTEXITCODE -ne 0) {
  Write-Warning "perf-bench run returned non-zero exit ($LASTEXITCODE)"
}

$env:MESH_BENCH_MODE = "full"
Write-Host "[perf] Running mesh echo benches (TCP) in $env:MESH_BENCH_MODE mode"
cargo bench -p core-mesh --bench echo --no-default-features --features with-libp2p
if ($LASTEXITCODE -ne 0) {
  Write-Warning "mesh TCP bench returned non-zero exit ($LASTEXITCODE)"
}

Write-Host "[perf] Running mesh echo benches (QUIC) in $env:MESH_BENCH_MODE mode"
cargo bench -p core-mesh --bench echo --no-default-features --features "with-libp2p quic"
if ($LASTEXITCODE -ne 0) {
  Write-Warning "mesh QUIC bench returned non-zero exit ($LASTEXITCODE)"
}

Write-Host "[perf] Collecting Criterion reports..."
if (!(Test-Path artifacts\criterion)) { New-Item -Path artifacts\criterion -ItemType Directory | Out-Null }
if (Test-Path target\criterion) {
  Copy-Item -Recurse -Force target\criterion\* artifacts\criterion\ -ErrorAction SilentlyContinue
}

if (Test-Path qnet-spec\templates\perf-summary-template.md) {
  Copy-Item qnet-spec\templates\perf-summary-template.md artifacts\perf-summary.md -ErrorAction SilentlyContinue
}

Write-Host "[perf] Done. See artifacts/ for outputs."
