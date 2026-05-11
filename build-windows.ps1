[CmdletBinding()]
Param(
    [Parameter()][Alias('h')][switch]$Help,
    [Parameter()][Alias('a')][string]$Architecture
)

# https://stackoverflow.com/questions/57949031/powershell-script-stops-if-program-fails-like-bash-set-o-errexit
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true

if ($Help) {
    Write-Output "Usage: build-windows.ps1 [-Architecture <x86_64|aarch64>] [-Help]"
    Write-Output "Build Kal'tsit Esperanta for Windows."
    Write-Output ""
    Write-Output "Options:"
    Write-Output "  -Architecture, -a  Which architecture to build (x86_64 or aarch64)"
    Write-Output "  -Help, -h          Show this help message."
    exit 0
}

Write-Output " █████╗ ███╗   ███╗ █████╗  ██╗ ██████╗ "
Write-Output "██╔══██╗████╗ ████║██╔══██╗███║██╔═████╗"
Write-Output "███████║██╔████╔██║███████║╚██║██║██╔██║"
Write-Output "██╔══██║██║╚██╔╝██║██╔══██║ ██║████╔╝██║"
Write-Output "██║  ██║██║ ╚═╝ ██║██║  ██║ ██║╚██████╔╝"
Write-Output "╚═╝  ╚═╝╚═╝     ╚═╝╚═╝  ╚═╝ ╚═╝ ╚═════╝ "
Write-Output "                                        "


# The Cargo binary name produced by `cargo build --package zed`. Must match
# `[[bin]] name` in crates/zed/Cargo.toml.
$BinName = "Kaltsit-Esperanta"
$AppName = "Kaltsit-Esperanta"

$OSArchitecture = switch ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture) {
    "X64" { "x86_64" }
    "Arm64" { "aarch64" }
    default { throw "Unsupported architecture" }
}

if (-not $Architecture) {
    $Architecture = $OSArchitecture
}

$target = "$Architecture-pc-windows-msvc"
$CargoOutDir = ".\target\$target\release"

function Get-VSArch {
    param([string]$Arch)
    switch ($Arch) {
        "x86_64" { "amd64" }
        "aarch64" { "arm64" }
    }
}

Write-Output "check rustup"
if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
    throw "rustup not installed. See https://www.rust-lang.org/tools/install"
}
rustup --version

Write-Output "check cmake"
if (-not (Get-Command cmake -ErrorAction SilentlyContinue)) {
    throw "cmake not installed. See https://cmake.org/download or install via Visual Studio Installer."
}
cmake --version | Select-Object -First 1

Write-Output "check Visual Studio"
$vsDevShellCandidates = @(
    "C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\Tools\Launch-VsDevShell.ps1",
    "C:\Program Files\Microsoft Visual Studio\2022\Professional\Common7\Tools\Launch-VsDevShell.ps1",
    "C:\Program Files\Microsoft Visual Studio\2022\Enterprise\Common7\Tools\Launch-VsDevShell.ps1",
    "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\Launch-VsDevShell.ps1"
)
$vsDevShell = $vsDevShellCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $vsDevShell) {
    throw "Visual Studio 2022 (or Build Tools) not found. See docs/src/development/windows.md."
}

Write-Output "🧰 Setup Toolchain"
$activeToolchain = rustup show active-toolchain 2>$null
if (-not $activeToolchain) {
    rustup toolchain install
}
rustup target add $target

Write-Output "✅ Toolchain Setup Complete"

Write-Output "⬇️ Initialize MSVC Developer Shell"
Push-Location
& $vsDevShell -Arch (Get-VSArch -Arch $Architecture) -HostArch (Get-VSArch -Arch $OSArchitecture)
Pop-Location

Write-Output "✅ Developer Shell Ready"

$channel = (Get-Content "crates\zed\RELEASE_CHANNEL").Trim()
$env:ZED_RELEASE_CHANNEL = $channel
$env:RELEASE_CHANNEL = $channel
$env:ZED_BUNDLE = "true"

Write-Output "🔨 Build Kal'tsit"
cargo build --release --package zed --package cli --target $target

Write-Output "✅ Build Complete"

Write-Output "📦 Package App"

$suffix = ""
if ($channel -ne "stable") {
    $suffix = "-$channel"
}

$packageRoot = ".\target\release"
New-Item -Path $packageRoot -ItemType Directory -Force | Out-Null

$packageName = "$BinName$suffix"
$packageDir = Join-Path $packageRoot $packageName
if (Test-Path $packageDir) {
    Remove-Item -Path $packageDir -Recurse -Force
}
New-Item -Path $packageDir -ItemType Directory -Force | Out-Null
New-Item -Path (Join-Path $packageDir "bin") -ItemType Directory -Force | Out-Null

Copy-Item -Path "$CargoOutDir\$BinName.exe" -Destination "$packageDir\$BinName.exe" -Force
Copy-Item -Path "$CargoOutDir\cli.exe" -Destination "$packageDir\bin\zed.exe" -Force

$iconSrc = "crates\zed\resources\windows\app-icon$suffix.ico"
if (Test-Path $iconSrc) {
    Copy-Item -Path $iconSrc -Destination "$packageDir\app-icon.ico" -Force
}

$archive = Join-Path $packageRoot "$BinName-windows-$Architecture.zip"
if (Test-Path $archive) {
    Remove-Item -Path $archive -Force
}
Compress-Archive -Path "$packageDir\*" -DestinationPath $archive -Force

Write-Output "Bundled $archive"
Write-Output "✅ Package Complete"
