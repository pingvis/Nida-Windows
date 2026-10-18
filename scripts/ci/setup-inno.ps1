# A pinned official compiler, kept portable: no system-wide compiler install.
[CmdletBinding()]
param([Parameter(Mandatory)][string]$ToolsDirectory)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Path $ToolsDirectory -Force | Out-Null
$ToolsDirectory = (Resolve-Path -LiteralPath $ToolsDirectory).Path
$compiler = Join-Path $ToolsDirectory 'compiler/ISCC.exe'
if (Test-Path -LiteralPath $compiler) { throw 'Use a fresh Inno tools directory.' }
$download = Join-Path $ToolsDirectory 'innosetup-6.7.1.exe'
Invoke-WebRequest 'https://github.com/jrsoftware/issrc/releases/download/is-6_7_1/innosetup-6.7.1.exe' -OutFile $download
if ((Get-FileHash -LiteralPath $download -Algorithm SHA256).Hash -ne '4d11e8050b6185e0d49bd9e8cc661a7a59f44959a621d31d11033124c4e8a7b0') {
    throw 'Inno Setup compiler download checksum mismatch.'
}
$process = Start-Process -FilePath $download -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/SP-', '/CURRENTUSER', '/PORTABLE=1', ('/DIR="' + (Join-Path $ToolsDirectory 'compiler') + '"')) -WindowStyle Hidden -Wait -PassThru
if ($process.ExitCode -ne 0 -or !(Test-Path -LiteralPath $compiler)) { throw 'Portable Inno compiler setup failed.' }
if ($env:GITHUB_ENV) { "INNO_COMPILER=$compiler" >> $env:GITHUB_ENV }
$compiler
