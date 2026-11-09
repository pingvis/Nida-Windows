# Native x64 WebRTC AEC3, built only from the vendored source archives.
[CmdletBinding()]
param(
    [string]$ToolsDirectory = (Join-Path $PSScriptRoot '../../target/windows-aec'),
    [string]$Python = 'python'
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if (!$IsWindows -or $env:VSCMD_ARG_TGT_ARCH -ne 'x64') {
    throw 'Run from x64 Developer PowerShell.'
}
New-Item -ItemType Directory -Force -Path $ToolsDirectory | Out-Null
$ToolsDirectory = (Resolve-Path -LiteralPath $ToolsDirectory).Path
$venv = Join-Path $ToolsDirectory 'venv'
$venvPython = Join-Path $venv 'Scripts/python.exe'
if (!(Test-Path -LiteralPath $venvPython)) {
    & $Python -m venv $venv
    if ($LASTEXITCODE -ne 0) { throw 'Could not create AEC build virtual environment.' }
}
& $venvPython -m pip install --disable-pip-version-check meson==1.9.1 ninja==1.13.0
if ($LASTEXITCODE -ne 0) { throw 'Could not install pinned AEC build tools.' }
$oldPath = $env:PATH
try {
    $env:PATH = "$(Join-Path $venv 'Scripts');$oldPath"
    & $venvPython (Join-Path $PSScriptRoot 'build-windows-aec.py') --output $ToolsDirectory
    if ($LASTEXITCODE -ne 0) { throw 'Native Windows AEC3 build failed.' }
} finally {
    $env:PATH = $oldPath
}
$env:NIDA_AEC_LIB_DIR = $ToolsDirectory
if ($env:GITHUB_ENV) { "NIDA_AEC_LIB_DIR=$ToolsDirectory" >> $env:GITHUB_ENV }
Write-Host "Native Windows AEC3 ready: $ToolsDirectory"
