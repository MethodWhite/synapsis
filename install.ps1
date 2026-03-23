# Synapsis Windows Installer
# PROPRIETARY - All Rights Reserved

Write-Host "╔══════════════════════════════════════════════════════════╗"
Write-Host "║  Synapsis Windows Installer                              ║"
Write-Host "║  PROPRIETARY SOFTWARE - LICENSED, NOT SOLD               ║"
Write-Host "╚══════════════════════════════════════════════════════════╝"
Write-Host ""

# Check Rust
Write-Host "📦 Checking Rust..."
if (-not (Get-Command rustc -ErrorAction SilentlyContinue)) {
    Write-Host "📦 Installing Rust..."
    Invoke-WebRequest https://win.rustup.rs/x86_64 -OutFile $env:TEMP\rustup-init.exe
    Start-Process -Wait $env:TEMP\rustup-init.exe -ArgumentList '-y'
    Remove-Item $env:TEMP\rustup-init.exe
    $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
}

# Build Synapsis
Write-Host "📦 Building Synapsis..."
$SynapsisPath = "C:\Users\$env:USERNAME\Projects\synapsis"
if (Test-Path $SynapsisPath) {
    Set-Location $SynapsisPath
    cargo build --release
    
    # Create bin directory
    $BinDir = "$env:USERPROFILE\.local\bin"
    if (-not (Test-Path $BinDir)) {
        New-Item -ItemType Directory -Path $BinDir | Out-Null
    }
    
    # Copy binary
    Copy-Item "target\release\synapsis.exe" "$BinDir\synapsis.exe" -Force
    
    # Add to PATH
    $CurrentPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($CurrentPath -notlike "*$BinDir*") {
        [Environment]::SetEnvironmentVariable("Path", "$CurrentPath;$BinDir", "User")
    }
} else {
    Write-Host "⚠️  Synapsis path not found. Please clone repository first."
    exit 1
}

Write-Host ""
Write-Host "╔══════════════════════════════════════════════════════════╗"
Write-Host "║  Installation Complete ✅                                ║"
Write-Host "╚══════════════════════════════════════════════════════════╝"
Write-Host ""
Write-Host "Binary: $env:USERPROFILE\.local\bin\synapsis.exe"
Write-Host ""
Write-Host "Usage:"
Write-Host "  synapsis --help"
Write-Host "  synapsis mcp"
