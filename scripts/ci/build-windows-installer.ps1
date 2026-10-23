[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Archive,
    [string]$Compiler = $env:INNO_COMPILER,
    [string]$OutputDirectory,
    [string]$ReleaseVersion,
    [switch]$TestIdentity
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if (!$IsWindows) { throw 'Build installers on Windows.' }
$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
Import-Module (Join-Path $PSScriptRoot 'windows-package.psm1') -Force
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
    $ReleaseVersion = Resolve-WindowsReleaseVersion -RequestedVersion $ReleaseVersion -PackageVersion $metadata.version
    if (!$TestIdentity -and $ReleaseVersion -cne $metadata.version) { throw 'Installer release version must match the portable payload exactly.' }
    $fileVersion = (($metadata.version -split '-')[0]) + '.0'
    $product = 'Nida Windows'
    $identity = 'pingvis.Nida.Windows'
    $suffix = ''
    if ($TestIdentity) { $product += ' Installer Test'; $identity += '.InstallerTest'; $suffix = '-InstallerTest' }
    $name = "Nida-$ReleaseVersion-windows-x64$suffix-Setup"
    & $Compiler "/DProductVersion=$ReleaseVersion" "/DFileVersion=$fileVersion" "/DPayloadDir=$($payload[0].FullName)" "/DProductName=$product" "/DInstallerId=$identity" "/O$OutputDirectory" "/F$name" (Join-Path $root 'scripts/windows/nida.iss') | Out-Host
    if ($LASTEXITCODE -ne 0) { throw 'Inno Setup compilation failed.' }
    $installer = Join-Path $OutputDirectory "$name.exe"
    Write-PackageChecksum -Path $installer
    Get-Item -LiteralPath $installer
} finally {
    # $work is an explicit GUID child created above, never the supplied output directory.
    if ([IO.Path]::GetDirectoryName($work) -ne $OutputDirectory) { throw 'Unsafe installer staging directory.' }
    Remove-Item -LiteralPath $work -Recurse -Force
}
