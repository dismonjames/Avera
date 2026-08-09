# Avera Installation Script for Windows
# usage: irm https://raw.githubusercontent.com/dismonjames/Avera/main/install.ps1 | iex

$Repo = "dismonjames/Avera"
$ApiUrl = "https://api.github.com/repos/$Repo/releases/latest"

$Arch = $env:PROCESSOR_ARCHITECTURE
if ($Arch -eq "AMD64") {
    $ArchName = "amd64"
} elseif ($Arch -eq "ARM64") {
    $ArchName = "arm64"
} else {
    Write-Host "Unsupported Architecture: $Arch" -ForegroundColor Red
    exit 1
}

$AssetName = "avera-windows-$ArchName.zip"

Write-Host "Fetching latest release from $Repo..."
$ReleaseInfo = Invoke-RestMethod -Uri $ApiUrl
$Asset = $ReleaseInfo.assets | Where-Object { $_.name -eq $AssetName }

if ($null -eq $Asset) {
    Write-Host "Could not find release asset $AssetName. Check if the release exists." -ForegroundColor Red
    exit 1
}

$InstallDir = "$env:USERPROFILE\.avera\bin"
if (!(Test-Path -Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir | Out-Null
}

$TempZip = "$env:TEMP\$AssetName"
Write-Host "Downloading $AssetName..."
Invoke-WebRequest -Uri $Asset.browser_download_url -OutFile $TempZip

Write-Host "Extracting to $InstallDir..."
Expand-Archive -Path $TempZip -DestinationPath $InstallDir -Force
Remove-Item -Path $TempZip

$AveraPath = "$InstallDir\avera.exe"
Write-Host "Avera installed successfully at $AveraPath" -ForegroundColor Green

# Add to PATH temporarily for this session
$env:Path += ";$InstallDir"

# Add to user PATH permanently if not already there
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notmatch [regex]::Escape($InstallDir)) {
    $NewPath = $UserPath + ";$InstallDir"
    [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
    Write-Host "Added $InstallDir to your PATH. Restart your terminal to apply changes." -ForegroundColor Yellow
}
