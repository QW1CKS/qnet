#Requires -RunAsAdministrator
<#
.SYNOPSIS
    Sets up a Windows machine for QNet development and running.

.DESCRIPTION
    This script installs all required dependencies for building and running QNet:
    - Rust toolchain (via rustup)
    - Visual Studio Build Tools (C++ compiler)
    - Git (if not present)
    
    After running this script, you can build QNet with:
        cargo build --release -p stealth-browser

.NOTES
    Run this script as Administrator in PowerShell 7+
    
.EXAMPLE
    .\setup-windows-dev.ps1
#>

param(
    [switch]$SkipRust,
    [switch]$SkipBuildTools,
    [switch]$SkipGit
)

$ErrorActionPreference = "Stop"

function Write-Step {
    param([string]$Message)
    Write-Host "`n[STEP] $Message" -ForegroundColor Cyan
}

function Write-Success {
    param([string]$Message)
    Write-Host "[OK] $Message" -ForegroundColor Green
}

function Write-Warn {
    param([string]$Message)
    Write-Host "[WARN] $Message" -ForegroundColor Yellow
}

function Write-Fail {
    param([string]$Message)
    Write-Host "[FAIL] $Message" -ForegroundColor Red
}

function Test-Command {
    param([string]$Command)
    $null = Get-Command $Command -ErrorAction SilentlyContinue
    return $?
}

# Banner
Write-Host @"

╔═══════════════════════════════════════════════════════════════╗
║                  QNet Windows Setup Script                    ║
║                                                               ║
║  This script will install:                                    ║
║    • Rust toolchain (rustup + cargo)                          ║
║    • Visual Studio Build Tools (C++ compiler)                 ║
║    • Git (if not installed)                                   ║
╚═══════════════════════════════════════════════════════════════╝

"@ -ForegroundColor Magenta

# Check if running as admin
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Fail "This script requires Administrator privileges."
    Write-Host "Please run PowerShell as Administrator and try again." -ForegroundColor Yellow
    exit 1
}

Write-Success "Running as Administrator"

# ============================================================================
# Step 1: Check/Install Git
# ============================================================================
if (-not $SkipGit) {
    Write-Step "Checking Git installation..."
    
    if (Test-Command "git") {
        $gitVersion = git --version
        Write-Success "Git already installed: $gitVersion"
    } else {
        Write-Host "Git not found. Installing via winget..." -ForegroundColor Yellow
        
        if (Test-Command "winget") {
            winget install --id Git.Git -e --source winget --accept-package-agreements --accept-source-agreements
            
            # Refresh PATH
            $env:Path = [System.Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path", "User")
            
            if (Test-Command "git") {
                Write-Success "Git installed successfully"
            } else {
                Write-Warn "Git installed but not in PATH. You may need to restart your terminal."
            }
        } else {
            Write-Fail "winget not available. Please install Git manually from https://git-scm.com/"
            Write-Host "After installing Git, run this script again with -SkipGit" -ForegroundColor Yellow
        }
    }
}

# ============================================================================
# Step 2: Check/Install Visual Studio Build Tools
# ============================================================================
if (-not $SkipBuildTools) {
    Write-Step "Checking Visual Studio Build Tools..."
    
    # Check for cl.exe (MSVC compiler)
    $vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    $hasBuildTools = $false
    
    if (Test-Path $vsWhere) {
        $vsPath = & $vsWhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>$null
        if ($vsPath) {
            $hasBuildTools = $true
            Write-Success "Visual Studio Build Tools found at: $vsPath"
        }
    }
    
    # Also check for standalone Build Tools
    $buildToolsPaths = @(
        "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools",
        "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2019\BuildTools",
        "${env:ProgramFiles}\Microsoft Visual Studio\2022\BuildTools"
    )
    
    foreach ($path in $buildToolsPaths) {
        if (Test-Path "$path\VC\Tools\MSVC") {
            $hasBuildTools = $true
            Write-Success "Build Tools found at: $path"
            break
        }
    }
    
    if (-not $hasBuildTools) {
        Write-Host "Visual Studio Build Tools not found. Installing..." -ForegroundColor Yellow
        
        if (Test-Command "winget") {
            Write-Host "Installing via winget (this may take 5-10 minutes)..." -ForegroundColor Cyan
            winget install --id Microsoft.VisualStudio.2022.BuildTools -e --source winget `
                --accept-package-agreements --accept-source-agreements `
                --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
            
            Write-Success "Visual Studio Build Tools installation initiated"
            Write-Warn "You may need to restart your terminal after installation completes"
        } else {
            Write-Fail "winget not available."
            Write-Host @"
Please install Visual Studio Build Tools manually:
1. Download from: https://visualstudio.microsoft.com/visual-cpp-build-tools/
2. Run the installer
3. Select "Desktop development with C++" workload
4. Complete installation and restart this script with -SkipBuildTools
"@ -ForegroundColor Yellow
        }
    }
}

# ============================================================================
# Step 3: Check/Install Rust
# ============================================================================
if (-not $SkipRust) {
    Write-Step "Checking Rust installation..."
    
    # Check if rustup is installed
    $rustupPath = "$env:USERPROFILE\.cargo\bin\rustup.exe"
    $cargoPath = "$env:USERPROFILE\.cargo\bin\cargo.exe"
    
    if ((Test-Path $rustupPath) -or (Test-Command "rustup")) {
        # Rust is installed, check version
        $rustVersion = rustc --version 2>$null
        if ($rustVersion) {
            Write-Success "Rust already installed: $rustVersion"
            
            # Update to latest stable
            Write-Host "Updating Rust to latest stable..." -ForegroundColor Cyan
            rustup update stable 2>&1 | Out-Null
            Write-Success "Rust updated"
        }
    } else {
        Write-Host "Rust not found. Installing via rustup..." -ForegroundColor Yellow
        
        # Download and run rustup-init
        $rustupInit = "$env:TEMP\rustup-init.exe"
        
        Write-Host "Downloading rustup-init.exe..." -ForegroundColor Cyan
        Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile $rustupInit
        
        Write-Host "Running rustup installer (this may take a few minutes)..." -ForegroundColor Cyan
        # Install with default options, no prompts
        & $rustupInit -y --default-toolchain stable
        
        # Add cargo to current session PATH
        $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
        
        # Verify installation
        if (Test-Path $cargoPath) {
            $rustVersion = & $cargoPath --version
            Write-Success "Rust installed successfully: $rustVersion"
        } else {
            Write-Fail "Rust installation may have failed. Check the output above."
        }
        
        # Cleanup
        Remove-Item $rustupInit -ErrorAction SilentlyContinue
    }
}

# ============================================================================
# Step 4: Verify everything works
# ============================================================================
Write-Step "Verifying installation..."

$allGood = $true

# Refresh PATH one more time
$env:Path = "$env:USERPROFILE\.cargo\bin;" + [System.Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path", "User")

# Check Git
if (Test-Command "git") {
    Write-Success "Git: $(git --version)"
} else {
    Write-Warn "Git: Not in PATH (may need terminal restart)"
    $allGood = $false
}

# Check Rust
if (Test-Command "rustc") {
    Write-Success "Rust: $(rustc --version)"
} else {
    Write-Warn "Rust: Not in PATH (may need terminal restart)"
    $allGood = $false
}

# Check Cargo
if (Test-Command "cargo") {
    Write-Success "Cargo: $(cargo --version)"
} else {
    Write-Warn "Cargo: Not in PATH (may need terminal restart)"
    $allGood = $false
}

# ============================================================================
# Summary
# ============================================================================
Write-Host "`n"
Write-Host "═══════════════════════════════════════════════════════════════" -ForegroundColor Magenta

if ($allGood) {
    Write-Host @"

  ✅ Setup Complete!

  Next steps:
  1. Clone the QNet repository (if not already):
     git clone https://github.com/QW1CKS/qnet.git
     cd qnet

  2. Build QNet:
     cargo build --release -p stealth-browser

  3. Run as client:
     `$env:RUST_LOG = "info"
     .\target\release\stealth-browser.exe

"@ -ForegroundColor Green
} else {
    Write-Host @"

  ⚠️  Setup partially complete.

  Please restart your terminal (or computer) and run this script again
  to verify all tools are properly installed.

  If issues persist, you can skip already-installed components:
    .\setup-windows-dev.ps1 -SkipGit -SkipRust

"@ -ForegroundColor Yellow
}

Write-Host "═══════════════════════════════════════════════════════════════" -ForegroundColor Magenta
Write-Host ""
