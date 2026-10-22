# Experimental core Windows portable ZIP. Run from x64 Developer PowerShell.
# Requires Python 3.11+, MSVC tools and the FFmpeg SDK used to link the app.
[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [switch]$WithBrowser,
    [string]$CefRoot = $env:NIDA_CEF_ROOT,
    [string]$FfmpegRoot = $env:FFMPEG_DIR,
    [string]$Binary,
    [string]$CrtDirectory,
    [string]$OutputDirectory
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if (!$IsWindows) { throw 'Build and smoke-test this package on Windows x64.' }
Import-Module (Join-Path $PSScriptRoot 'windows-package.psm1') -Force
$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
if (!$OutputDirectory) { $OutputDirectory = Join-Path $root 'dist/windows' }
if (!$Binary) { $Binary = Join-Path $root 'target/release/varda.exe' }
if (!$FfmpegRoot) { throw 'Set FFMPEG_DIR or pass -FfmpegRoot for the FFmpeg SDK used to build the application.' }
$FfmpegRoot = (Resolve-Path -LiteralPath $FfmpegRoot).Path
$ffmpegBin = Join-Path $FfmpegRoot 'bin'
foreach ($file in @('bin/ffmpeg.exe', 'bin/ffprobe.exe', 'include/libavcodec/avcodec.h', 'LICENSE.txt')) {
    if (!(Test-Path -LiteralPath (Join-Path $FfmpegRoot $file) -PathType Leaf)) { throw "FFmpeg SDK is incomplete: $file" }
}
Get-Command dumpbin.exe, python, cargo -ErrorAction Stop | Out-Null
$env:FFMPEG_DIR = $FfmpegRoot
$oldPath = $env:PATH
Push-Location $root
try {
    $env:PATH = "$ffmpegBin;$oldPath"
    if (!$SkipBuild) {
        $features = if ($WithBrowser) { 'screen-capture,windows-browser' } else { 'screen-capture' }
        & cargo build --release --locked --no-default-features --features $features
        if ($LASTEXITCODE -ne 0) { throw 'Core release build failed.' }
    }
    if (!(Test-Path -LiteralPath $Binary -PathType Leaf)) { throw "Missing application binary: $Binary" }
    if (!$CrtDirectory -and $env:VCToolsRedistDir) {
        $crt = @(Get-ChildItem (Join-Path $env:VCToolsRedistDir 'x64') -Directory -Filter 'Microsoft.VC*.CRT' |
            Sort-Object Name -Descending)
        if ($crt.Count) { $CrtDirectory = $crt[0].FullName }
    }
    New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
    $OutputDirectory = (Resolve-Path -LiteralPath $OutputDirectory).Path
    $versionLine = Select-String -LiteralPath (Join-Path $root 'Cargo.toml') -Pattern '^version = "([^"]+)"' | Select-Object -First 1
    if (!$versionLine) { throw 'Cannot read the application version.' }
    $version = $versionLine.Matches[0].Groups[1].Value
    $revision = (& git rev-parse --short=12 HEAD).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Cannot identify source revision.' }
    $name = "Nida-$version-windows-x64-experimental-$revision"
    # Only remove our own fresh temporary directory, never a caller's directory.
    $work = Join-Path $OutputDirectory ([guid]::NewGuid().ToString('N'))
    $stage = Join-Path $work $name
    New-Item -ItemType Directory -Path $stage -Force | Out-Null
    try {
        Copy-Item -LiteralPath $Binary -Destination (Join-Path $stage 'varda.exe')
        if ($WithBrowser) {
            & (Join-Path $PSScriptRoot 'build-windows-browser.ps1') -CefRoot $CefRoot -OutputDirectory $stage
            # Browser binaries live in their own DLL search directory.
            Copy-PackageDependencies (Join-Path $stage 'browser') $ffmpegBin @($CrtDirectory)
        }
        foreach ($tool in @('ffmpeg.exe', 'ffprobe.exe')) {
            Copy-Item -LiteralPath (Join-Path $ffmpegBin $tool) -Destination $stage
        }
        Get-ChildItem -LiteralPath $ffmpegBin -Filter '*.dll' -File | Copy-Item -Destination $stage
        Copy-PackageDependencies $stage $ffmpegBin @((Split-Path $Binary), $CrtDirectory)
        Copy-Item -LiteralPath (Join-Path $root 'shaders') -Destination $stage -Recurse
        foreach ($line in Get-Content -LiteralPath (Join-Path $root 'scripts/ci/shaders-left-out.txt')) {
            $shader = $line.Trim()
            if (!$shader -or $shader.StartsWith('#')) { continue }
            if ([IO.Path]::GetFileName($shader) -ne $shader) { throw "Invalid shader exclusion: $shader" }
            Remove-Item -LiteralPath (Join-Path $stage "shaders/$shader") -ErrorAction SilentlyContinue
        }
        $licenses = Join-Path $stage 'licenses'
        New-Item -ItemType Directory -Path $licenses | Out-Null
        Copy-Item -LiteralPath (Join-Path $root 'LICENSE') -Destination (Join-Path $licenses 'Nida-MIT.txt')
        Copy-Item -LiteralPath (Join-Path $FfmpegRoot 'LICENSE.txt') -Destination (Join-Path $licenses 'FFmpeg.txt')
        & python (Join-Path $PSScriptRoot 'windows-notices.py') --output $licenses
        if ($LASTEXITCODE -ne 0) { throw 'Could not collect dependency notices.' }
        Copy-Item -LiteralPath (Join-Path $root 'docs/windows-portable-readme.txt') -Destination (Join-Path $stage 'Read me first.txt')
        Copy-Item -LiteralPath (Join-Path $root 'Cargo.lock') -Destination $licenses
        $provenance = Join-Path $FfmpegRoot 'nida-build-inputs.json'
        if (Test-Path -LiteralPath $provenance) { Copy-Item -LiteralPath $provenance -Destination $licenses }
        (& (Join-Path $stage 'ffmpeg.exe') -hide_banner -version 2>&1) | Set-Content (Join-Path $licenses 'FFmpeg-build.txt')
        if ($LASTEXITCODE -ne 0) { throw 'Bundled FFmpeg cannot start.' }
        $metadata = [ordered]@{
            version = $version; revision = (& git rev-parse HEAD).Trim(); experimental = $true
            features = $(if ($WithBrowser) { @('screen-capture', 'windows-browser') } else { @('screen-capture') }); target = 'x86_64-pc-windows-msvc'
            rust = (& rustc --version).Trim(); ffmpeg = (Get-Content (Join-Path $licenses 'FFmpeg-build.txt') -First 1)
            files = @(Get-ChildItem -LiteralPath $stage -Recurse -File | Sort-Object FullName | ForEach-Object {
                @{ path = [IO.Path]::GetRelativePath($stage, $_.FullName).Replace('\', '/'); sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() }
            })
        }
        $metadata | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $stage 'build-info.json') -Encoding utf8
        $candidate = Join-Path $work "$name.zip"
        Compress-Archive -LiteralPath $stage -DestinationPath $candidate
        & (Join-Path $PSScriptRoot 'test-windows-package.ps1') -Archive $candidate
        $zip = Join-Path $OutputDirectory "$name.zip"
        Move-Item -LiteralPath $candidate -Destination $zip -Force
        $hash = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
        "$hash  $name.zip" | Set-Content -LiteralPath "$zip.sha256" -Encoding ascii
        Write-Host "Verified experimental package: $zip"
    } finally { Remove-Item -LiteralPath $work -Recurse -Force }
} finally {
    $env:PATH = $oldPath
    Pop-Location
}
