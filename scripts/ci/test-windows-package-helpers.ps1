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
} finally { Remove-Item -LiteralPath $temporary -Recurse -Force }
