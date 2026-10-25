[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Archive,
    [string]$Compiler = $env:INNO_COMPILER,
    [string]$OutputDirectory,
    [string]$ReleaseVersion,
    [switch]$TestIdentity,
    [switch]$TestInstallUpgrade
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if (!$IsWindows) { throw 'Build installers on Windows.' }
if ($TestIdentity -and $TestInstallUpgrade) { throw 'TestInstallUpgrade must start from a production installer identity.' }
$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
Import-Module (Join-Path $PSScriptRoot 'windows-package.psm1') -Force
if (!$OutputDirectory) { $OutputDirectory = Join-Path $root 'dist/windows-installer' }
if (!$Compiler) { $Compiler = (Get-Command ISCC.exe -ErrorAction Stop).Source }
$Compiler = (Resolve-Path -LiteralPath $Compiler).Path
$Archive = (Resolve-Path -LiteralPath $Archive).Path
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$OutputDirectory = (Resolve-Path -LiteralPath $OutputDirectory).Path
$work = Join-Path $OutputDirectory ('Installer staging ' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
try {
    # Extract and validate once, then compile every installer from this payload.
    # Installed and upgraded copies are still smoke-tested independently.
    Expand-Archive -LiteralPath $Archive -DestinationPath $work
    $payload = @(Get-ChildItem -LiteralPath $work -Directory)
    if ($payload.Count -ne 1) { throw 'Expected one portable payload directory.' }
    & (Join-Path $PSScriptRoot 'test-windows-package.ps1') -PackageDirectory $payload[0].FullName | Out-Host
    $metadata = Get-Content -LiteralPath (Join-Path $payload[0].FullName 'build-info.json') -Raw | ConvertFrom-Json
    $ReleaseVersion = Resolve-WindowsReleaseVersion -RequestedVersion $ReleaseVersion -PackageVersion $metadata.version
    if (!$TestIdentity -and $ReleaseVersion -cne $metadata.version) { throw 'Installer release version must match the portable payload exactly.' }
    $fileVersion = (($metadata.version -split '-')[0]) + '.0'
    function Build-Installer([string]$Version, [string]$Destination, [switch]$Fixture) {
        $product = 'Nida Windows'
        $identity = 'pingvis.Nida.Windows'
        $suffix = ''
        $compilerOptions = @()
        if ($Fixture) {
            $product += ' Installer Test'; $identity += '.InstallerTest'; $suffix = '-InstallerTest'
            $compilerOptions += '/DTestInstaller'
        }
        New-Item -ItemType Directory -Path $Destination -Force | Out-Null
        $name = "Nida-$Version-windows-x64$suffix-Setup"
        & $Compiler @compilerOptions "/DProductVersion=$Version" "/DFileVersion=$fileVersion" "/DPayloadDir=$($payload[0].FullName)" "/DProductName=$product" "/DInstallerId=$identity" "/O$Destination" "/F$name" (Join-Path $root 'scripts/windows/nida.iss') | Out-Host
        if ($LASTEXITCODE -ne 0) { throw 'Inno Setup compilation failed.' }
        $installer = Join-Path $Destination "$name.exe"
        Write-PackageChecksum -Path $installer
        Get-Item -LiteralPath $installer
    }
    $installer = Build-Installer -Version $ReleaseVersion -Destination $OutputDirectory -Fixture:$TestIdentity
    if ($TestInstallUpgrade) {
        $version = ($ReleaseVersion -split '-')[0]
        $fixtures = Join-Path $work 'installer-fixtures'
        $first = Build-Installer -Version "$version-installer-test.1" -Destination $fixtures -Fixture
        $next = Build-Installer -Version "$version-installer-test.2" -Destination $fixtures -Fixture
        & (Join-Path $PSScriptRoot 'test-windows-installer.ps1') -Installer $first.FullName -UpgradeInstaller $next.FullName | Out-Host
    }
    $installer
} finally {
    # $work is an explicit temporary child created above, never the supplied output directory.
    if ([IO.Path]::GetDirectoryName($work) -ne $OutputDirectory) { throw 'Unsafe installer staging directory.' }
    Remove-Item -LiteralPath $work -Recurse -Force
}
