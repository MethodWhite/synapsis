# Synapsis Installer - Windows (PowerShell)
# Pure Rust - No Python dependencies
param(
    [string]$Version = "0.8.2"
)

$Repo = "methodwhite/synapsis"
$InstallDir = "$env:USERPROFILE\.local\bin"
$DataDir = "$env:USERPROFILE\.local\share\synapsis"

Write-Host "Synapsis v$Version Installer (Windows)" -ForegroundColor Cyan
Write-Host "========================================"
Write-Host ""

# Detect architecture
$Arch = (Get-WmiObject Win32_Processor).Architecture
switch ($Arch) {
    9 { $Target = "x86_64-pc-windows-msvc" }
    5 { $Target = "aarch64-pc-windows-msvc" }
    default { 
        Write-Host "Unsupported CPU architecture: $Arch" -ForegroundColor Red
        Write-Host "Install Rust from https://rustup.rs and build from source."
        exit 1
    }
}

Write-Host "Platform: Windows ($Target)"
Write-Host "Install dir: $InstallDir"
Write-Host "Data dir: $DataDir"
Write-Host ""

# Create directories
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
New-Item -ItemType Directory -Force -Path $DataDir | Out-Null

# Download URL
$DownloadUrl = "https://github.com/$Repo/releases/download/v$Version/synapsis-$Target.zip"
$TempFile = "$env:TEMP\synapsis-$Version.zip"

try {
    Write-Host "Downloading synapsis v$Version for $Target..."
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $TempFile -ErrorAction Stop
    
    Write-Host "Extracting..."
    Expand-Archive -Path $TempFile -DestinationPath "$env:TEMP\synapsis-extract" -Force
    
    Copy-Item "$env:TEMP\synapsis-extract\synapsis\synapsis.exe" "$InstallDir\synapsis.exe" -Force -ErrorAction SilentlyContinue
    Copy-Item "$env:TEMP\synapsis-extract\synapsis\synapsis-mcp.exe" "$InstallDir\synapsis-mcp.exe" -Force -ErrorAction SilentlyContinue
    
    Remove-Item $TempFile -Force -ErrorAction SilentlyContinue
    Remove-Item "$env:TEMP\synapsis-extract" -Recurse -Force -ErrorAction SilentlyContinue
    
    Write-Host ""
    Write-Host "Synapsis v$Version installed successfully!" -ForegroundColor Green
}
catch {
    Write-Host "Download failed. Build from source:" -ForegroundColor Yellow
    Write-Host "  1. Install Rust: https://rustup.rs"
    Write-Host "  2. git clone https://github.com/$Repo.git"
    Write-Host "  3. cd synapsis && cargo build --release"
    Write-Host "  4. Copy target/release/synapsis.exe to $InstallDir"
    exit 1
}

# Add to PATH
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -notlike "*$InstallDir*") {
    Write-Host ""
    Write-Host "Adding $InstallDir to PATH..."
    [Environment]::SetEnvironmentVariable("Path", "$userPath;$InstallDir", "User")
    $env:Path = "$env:Path;$InstallDir"
}

Write-Host ""
Write-Host "Done! Run 'synapsis mcp' to start the MCP server."
Write-Host "Run 'synapsis update' to check for updates."
Write-Host ""
Write-Host "Configure your IDE MCP client:"
Write-Host "  command: $InstallDir\synapsis.exe"
Write-Host '  args: ["mcp"]'
