# No Pester or Windows SDK required; runs under PowerShell 7 on Linux too.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'windows-package.psm1') -Force
function Assert-Throws([scriptblock]$Action, [string]$Expected) {
    $caught = $null
    try { & $Action | Out-Null } catch { $caught = $_ }
    if (!$caught -or $caught.Exception.Message -notlike "*$Expected*") { throw "Expected failure containing: $Expected" }
}
$dependencies = @(ConvertFrom-DumpbinDependencies @('  Image has the following dependencies:', '    KERNEL32.dll', '    avcodec-63.dll', '  Image has the following delay load dependencies:', '    VCRUNTIME140.dll', '    kernel32.dll', '  Summary', '  1000 .data'))
if (($dependencies -join ',') -ne 'avcodec-63.dll,kernel32.dll,vcruntime140.dll') { throw 'Dependency parser lost/doubled normal or delayed imports.' }
$temporary = Join-Path ([IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString('N'))
try {
    foreach ($directory in @('ffmpeg', 'build', 'system', 'crt')) { New-Item -ItemType Directory -Path (Join-Path $temporary $directory) -Force | Out-Null }
    foreach ($label in @('0.9.1-beta.4', 'v0.9.1-beta.4', ' V0.9.1-Beta.4 ')) {
        if ((Resolve-WindowsReleaseVersion -RequestedVersion $label -PackageVersion '0.9.1') -cne '0.9.1-beta.4') { throw 'Release label normalization failed.' }
    }
    if ((Resolve-WindowsReleaseVersion -PackageVersion '0.9.1') -ne '0.9.1') { throw 'Branch build must default to Cargo version.' }
    if ((Resolve-WindowsReleaseVersion -PackageVersion '0.9.1' -TagVersion 'v0.9.1-Beta.1') -cne '0.9.1-beta.1') { throw 'Tag build must use the tag, including its prerelease suffix.' }
    if ((Resolve-WindowsReleaseVersion -RequestedVersion '0.9.1-beta.4' -PackageVersion '0.9.1' -TagVersion 'v0.9.1-beta.4') -ne '0.9.1-beta.4') { throw 'Matching tag and input rejected.' }
    Assert-Throws { Resolve-WindowsReleaseVersion -RequestedVersion '0.9.1-beta.5' -PackageVersion '0.9.1' -TagVersion 'v0.9.1-beta.4' } 'match the selected tag'
    Assert-Throws { Resolve-WindowsReleaseVersion -RequestedVersion '0.9.2-beta.4' -PackageVersion '0.9.1' } 'match the package version'
    foreach ($invalid in @('0.9.1-beta..4', '0.9.1-', '../0.9.1', '0.9.1 beta.4', '0.9.1;echo')) {
        Assert-Throws { Resolve-WindowsReleaseVersion -RequestedVersion $invalid -PackageVersion '0.9.1' } 'Use a release version'
    }
    foreach ($name in @('Nida-0.9.1-beta.4-windows-x64-portable.zip', 'Nida-0.9.1-beta.4-windows-x64-Setup.exe')) {
        $asset = Join-Path $temporary $name
        Set-Content -LiteralPath $asset -Value 'release payload fixture'
        Write-PackageChecksum -Path $asset
        $line = (Get-Content -LiteralPath "$asset.sha256" -Raw).TrimEnd()
        $expectedHash = (Get-FileHash -LiteralPath $asset -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($line -cne "$expectedHash  $name") { throw 'Checksum must name the exact final downloadable file, without a directory.' }
    }
    $sdk = Join-Path $temporary 'ffmpeg'
    $build = Join-Path $temporary 'build'
    $system = Join-Path $temporary 'system'
    $crt = Join-Path $temporary 'crt'
    Set-Content (Join-Path $build 'avcodec-63.dll') 'wrong distribution'
    Assert-Throws { Find-PackageDependency 'avcodec-63.dll' $sdk @($build) $system } 'Missing runtime dependency'
    Set-Content (Join-Path $sdk 'avcodec-63.dll') 'matching distribution'
    if ((Find-PackageDependency 'avcodec-63.dll' $sdk @($build) $system) -ne (Join-Path $sdk 'avcodec-63.dll')) { throw 'FFmpeg SDK precedence failed.' }
    Set-Content (Join-Path $system 'vcruntime140.dll') 'installed on builder only'
    Assert-Throws { Find-PackageDependency 'vcruntime140.dll' $sdk @($build) $system } 'Missing runtime dependency'
    Set-Content (Join-Path $crt 'vcruntime140.dll') 'redistributable'
    if ((Find-PackageDependency 'vcruntime140.dll' $sdk @($crt) $system) -ne (Join-Path $crt 'vcruntime140.dll')) { throw 'App-local CRT resolution failed.' }
    Set-Content (Join-Path $system 'kernel32.dll') 'Windows'
    if ($null -ne (Find-PackageDependency 'kernel32.dll' $sdk @($build) $system)) { throw 'Windows system DLL should not be bundled.' }
    if ($null -ne (Find-PackageDependency 'api-ms-win-core-file-l1-1-0.dll' $sdk @($build) $system)) { throw 'API-set handling failed.' }
    Assert-Throws { Find-PackageDependency 'missing.dll' $sdk @($build) $system } 'Missing runtime dependency'
    Write-Host 'PASS: normal/delayed imports, FFmpeg SDK isolation, missing DLL failures, app-local CRT, Windows/API-set exclusions.'
    Write-Host 'PASS: release/tag versions, invalid/mismatched labels and final ZIP/Setup checksum filenames.'
} finally { Remove-Item -LiteralPath $temporary -Recurse -Force }
