[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Archive,
    [string]$Compiler = $env:INNO_COMPILER,
    [string]$OutputDirectory,
    [ValidatePattern('^\d+\.\d+\.\d+(-[A-Za-z0-9.-]+)?$')][string]$ReleaseVersion,
    [switch]$TestIdentity
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if (!$IsWindows) { throw 'Build installers on Windows.' }
$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
if (!$OutputDirectory) { $OutputDirectory = Join-Path $root 'dist/windows-installer' }
if (!$Compiler) { $Compiler = (Get-Command ISCC.exe -ErrorAction Stop).Source }
$Compiler = (Resolve-Path -LiteralPath $Compiler).Path
$Archive = (Resolve-Path -LiteralPath $Archive).Path
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$OutputDirectory = (Resolve-Path -LiteralPath $OutputDirectory).Path
$work = Join-Path $OutputDirectory ([guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
try {
    # The same smoke test is used for both the ZIP and installed payload.
    & (Join-Path $PSScriptRoot 'test-windows-package.ps1') -Archive $Archive | Out-Host
    Expand-Archive -LiteralPath $Archive -DestinationPath $work
    $payload = @(Get-ChildItem -LiteralPath $work -Directory)
    if ($payload.Count -ne 1) { throw 'Expected one portable payload directory.' }
    $metadata = Get-Content -LiteralPath (Join-Path $payload[0].FullName 'build-info.json') -Raw | ConvertFrom-Json
    if (!$ReleaseVersion) { $ReleaseVersion = $metadata.version }
    if (($ReleaseVersion -split '-')[0] -ne ($metadata.version -split '-')[0]) { throw 'Release version does not match the portable payload.' }
    $fileVersion = (($metadata.version -split '-')[0]) + '.0'
    $product = 'Nida Windows'
    $identity = 'pingvis.Nida.Windows'
    $suffix = ''
    if ($TestIdentity) { $product += ' Installer Test'; $identity += '.InstallerTest'; $suffix = '-InstallerTest' }
    $name = "Nida-$ReleaseVersion-windows-x64$suffix-Setup"
    & $Compiler "/DProductVersion=$ReleaseVersion" "/DFileVersion=$fileVersion" "/DPayloadDir=$($payload[0].FullName)" "/DProductName=$product" "/DInstallerId=$identity" "/O$OutputDirectory" "/F$name" (Join-Path $root 'scripts/windows/nida.iss') | Out-Host
    if ($LASTEXITCODE -ne 0) { throw 'Inno Setup compilation failed.' }
    $installer = Join-Path $OutputDirectory "$name.exe"
    $hash = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $name.exe" | Set-Content -LiteralPath "$installer.sha256" -Encoding ascii
    Get-Item -LiteralPath $installer
} finally {
    # $work is an explicit GUID child created above, never the supplied output directory.
    if ([IO.Path]::GetDirectoryName($work) -ne $OutputDirectory) { throw 'Unsafe installer staging directory.' }
    Remove-Item -LiteralPath $work -Recurse -Force
}
