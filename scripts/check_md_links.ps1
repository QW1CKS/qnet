# Validates relative links in Markdown files across the repo
param(
    [string]$Root = (Get-Location).Path
)

$ErrorActionPreference = 'Stop'

$mdFiles = Get-ChildItem -Path $Root -Recurse -Include *.md -File |
    Where-Object { $_.FullName -notmatch '\\target\\' }
$bad = @()

foreach ($file in $mdFiles) {
    try {
        $content = Get-Content -Path $file.FullName -Raw -ErrorAction Stop
    } catch {
        Write-Verbose "Failed reading $($file.FullName): $($_.Exception.Message)"
        $content = ''
    }
    if ($null -eq $content) { $content = '' }
    # Match Markdown links [text](path) excluding http(s), mailto, and anchors-only
    $matches = [regex]::Matches($content, '\\[[^\\]]*\\]\\((?!https?://)(?!mailto:)(?!#)([^)]+)\\)')
    foreach ($m in $matches) {
        $raw = $m.Groups[1].Value.Trim()
        # Skip image data URIs and hashes
        if ($raw -match '^(data:|#)') { continue }
        # Ignore query/fragment after file
        $pathOnly = $raw.Split('#')[0].Split('?')[0]
        if ([string]::IsNullOrWhiteSpace($pathOnly)) { continue }
        # Skip absolute paths (/...) which are GitHub project-absolute; treat as ok if file exists relative to repo root
        if ($pathOnly.StartsWith('/')) {
            $repoRel = $pathOnly.TrimStart('/')
            $candidate = Join-Path $Root $repoRel
        }
        else {
            $candidatePath = Join-Path $file.DirectoryName $pathOnly
            $candidate = Resolve-Path -LiteralPath $candidatePath -ErrorAction SilentlyContinue
            if (-not $candidate) { $candidate = $candidatePath }
        }
        $exists = Test-Path -LiteralPath $candidate
        if (-not $exists) {
            $bad += [pscustomobject]@{
                File = (Resolve-Path $file.FullName).Path
                Link = $raw
            }
        }
    }
}

if ($bad.Count -gt 0) {
    Write-Host "Broken links found:`n" -ForegroundColor Red
    $bad | Sort-Object File, Link | Format-Table -AutoSize
    exit 1
} else {
    Write-Host "All Markdown links resolve." -ForegroundColor Green
}