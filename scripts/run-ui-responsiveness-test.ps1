# Launch the local UI test build in its own show, without replacing the installed app.
$ErrorActionPreference = 'Stop'
$nidaUiRoot = Split-Path $PSScriptRoot -Parent
$nidaUiBinary = Join-Path $nidaUiRoot 'target/release/Nida-UI-Responsiveness-Test.exe'
$nidaUiRuntime = Join-Path $nidaUiRoot '.windows-tools/native/ffmpeg-sdk/ffmpeg-n9.0-latest-win64-gpl-shared-9.0/bin'
$nidaUiShow = Join-Path $nidaUiRoot 'target/ui-responsiveness/user-test-show'
if (!(Test-Path -LiteralPath $nidaUiBinary)) { throw "Test build not found: $nidaUiBinary" }
if (!(Test-Path -LiteralPath $nidaUiRuntime)) { throw "FFmpeg runtime not found: $nidaUiRuntime" }
New-Item -ItemType Directory -Force -Path $nidaUiShow | Out-Null
$nidaUiOldPath = $env:PATH
try {
    $env:PATH = "$nidaUiRuntime;$nidaUiOldPath"
    Start-Process -FilePath $nidaUiBinary -WorkingDirectory $nidaUiRoot -ArgumentList @('--workspace', ('"' + $nidaUiShow + '"')) -WindowStyle Hidden
} finally {
    $env:PATH = $nidaUiOldPath
}
