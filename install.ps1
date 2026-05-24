# Serez-Code installer for Windows
# Usage: irm https://raw.githubusercontent.com/Sergio3215/serez-code/main/install.ps1 | iex
$ErrorActionPreference = "Stop"

$Repo      = "Sergio3215/serez-code"
$InstallDir = "$env:LOCALAPPDATA\SerezCode\bin"

Write-Host "Fetching latest release..."
$Release = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest"
$Tag     = $Release.tag_name
$Asset   = $Release.assets | Where-Object { $_.name -eq "sz-windows-x64.exe" }

if (-not $Asset) {
    Write-Error "Windows binary not found in release $Tag. Try again later."
    exit 1
}

Write-Host "Installing Serez-Code $Tag..."
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
Invoke-WebRequest -Uri $Asset.browser_download_url -OutFile "$InstallDir\sz.exe"

# Add to user PATH if not already present
$UserPath = [Environment]::GetEnvironmentVariable("PATH", "User") ?? ""
if ($UserPath -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable("PATH", "$UserPath;$InstallDir", "User")
    Write-Host ""
    Write-Host "Added $InstallDir to PATH."
    Write-Host "Restart your terminal, then run: sz --version"
} else {
    Write-Host ""
    Write-Host "Installed: $InstallDir\sz.exe"
    Write-Host "Run: sz --version"
}
