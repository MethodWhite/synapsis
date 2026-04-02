# Synapsis Windows Uninstaller
# PROPRIETARY - All Rights Reserved

Write-Host "╔══════════════════════════════════════════════════════════╗"
Write-Host "║  Synapsis Windows Uninstaller                            ║"
Write-Host "║  PROPRIETARY SOFTWARE - LICENSED, NOT SOLD               ║"
Write-Host "╚══════════════════════════════════════════════════════════╝"
Write-Host ""

$binDir = "$env:USERPROFILE\.local\bin"
Write-Host "🗑️  Removing $binDir\synapsis.exe..."
Remove-Item -Force "$binDir\synapsis.exe" -ErrorAction SilentlyContinue
Remove-Item -Force "$binDir\synapsis-mcp.exe" -ErrorAction SilentlyContinue

Write-Host ""
Write-Host "╔══════════════════════════════════════════════════════════╗"
Write-Host "║  Uninstall Complete ✅                                   ║"
Write-Host "╚══════════════════════════════════════════════════════════╝"
Write-Host ""
