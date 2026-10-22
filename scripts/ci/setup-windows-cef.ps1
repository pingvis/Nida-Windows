# Exact SDK/runtime pair for the experimental browser source. No floating latest.
[CmdletBinding()]
param([Parameter(Mandatory)][string]$ToolsDirectory)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
if (!$IsWindows) { throw 'CEF setup requires Windows x64.' }
$version = '152.0.15+gb3d72fb+chromium-152.0.7977.169'
$name = "cef_binary_${version}_windows64"
$url = "https://cef-builds.spotifycdn.com/$name.tar.bz2"
$sha256 = 'c0640c1df5ae2d84705d06f7398798a907745504f730fa7b2abb98ccb878d162'
New-Item -ItemType Directory -Force -Path $ToolsDirectory | Out-Null
$ToolsDirectory = (Resolve-Path -LiteralPath $ToolsDirectory).Path
$archive = Join-Path $ToolsDirectory "$name.tar.bz2"
$cef = Join-Path $ToolsDirectory $name
if (!(Test-Path -LiteralPath $archive)) { Invoke-WebRequest -Uri $url -OutFile $archive -TimeoutSec 600 }
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $sha256) { throw 'CEF SDK checksum mismatch.' }
# Always validate the archive even when reusing an extracted SDK.
if (!(Test-Path -LiteralPath $cef)) {
    & tar -xjf $archive -C $ToolsDirectory
    if ($LASTEXITCODE -ne 0) { throw 'Could not extract CEF SDK.' }
}
foreach ($required in @('Release/libcef.dll', 'Release/bootstrap.exe', 'Resources/icudtl.dat', 'include/cef_version.h')) {
    if (!(Test-Path -LiteralPath (Join-Path $cef $required))) { throw "Incomplete CEF SDK: $required" }
}
@{ version = $version; url = $url; sha256 = $sha256 } | ConvertTo-Json |
    Set-Content -LiteralPath (Join-Path $cef 'nida-cef-inputs.json') -Encoding utf8
$env:NIDA_CEF_ROOT = $cef
if ($env:GITHUB_ENV) { "NIDA_CEF_ROOT=$cef" >> $env:GITHUB_ENV }
Write-Host "CEF SDK ready: $version"
