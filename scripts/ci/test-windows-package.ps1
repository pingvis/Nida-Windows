# Exercise the EXTRACTED ZIP without the developer PATH or repository as CWD.
[CmdletBinding()]
param(
    [Parameter(Mandatory, ParameterSetName = 'Archive')][string]$Archive,
    [Parameter(Mandatory, ParameterSetName = 'Installed')][string]$PackageDirectory
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if (!$IsWindows) { throw 'Runtime verification requires Windows.' }
$temporary = Join-Path ([IO.Path]::GetTempPath()) ("Nida package test " + [guid]::NewGuid().ToString('N'))
$oldPath = $env:PATH
$oldNoInstall = $env:VARDA_NO_CLI_INSTALL
$oldFfmpeg = $env:FFMPEG_DIR
New-Item -ItemType Directory -Path $temporary | Out-Null
try {
    if ($PSCmdlet.ParameterSetName -eq 'Archive') {
        Expand-Archive -LiteralPath $Archive -DestinationPath $temporary
        $directories = @(Get-ChildItem -LiteralPath $temporary -Directory)
        if ($directories.Count -ne 1) { throw 'Expected one portable application directory in the ZIP.' }
        $package = $directories[0].FullName
    } else {
        $package = (Resolve-Path -LiteralPath $PackageDirectory).Path
    }
    foreach ($required in @('varda.exe', 'ffmpeg.exe', 'ffprobe.exe', 'shaders', 'licenses/FFmpeg.txt', 'build-info.json', 'Read me first.txt')) {
        if (!(Test-Path -LiteralPath (Join-Path $package $required))) { throw "ZIP is missing $required" }
    }
    $metadata = Get-Content -LiteralPath (Join-Path $package 'build-info.json') -Raw | ConvertFrom-Json
    foreach ($file in $metadata.files) {
        $path = Join-Path $package $file.path
        if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ne $file.sha256) { throw "Payload checksum mismatch: $($file.path)" }
    }
    $env:PATH = "$package;$env:SystemRoot\System32;$env:SystemRoot"
    $env:FFMPEG_DIR = $null
    $env:VARDA_NO_CLI_INSTALL = '1'
    # An unrelated CWD and an extraction path with spaces catch accidental
    # dependencies on the checkout and broken quoting.
    Push-Location $temporary
    try {
        & (Join-Path $package 'varda.exe') --version
        if ($LASTEXITCODE -ne 0) { throw 'Packaged application cannot start.' }
        $ffmpegVersion = @(& (Join-Path $package 'ffmpeg.exe') -hide_banner -version 2>&1)
        if ($LASTEXITCODE -ne 0 -or $ffmpegVersion[0] -notmatch '^ffmpeg version n?9\.') { throw 'FFmpeg 9 runtime check failed.' }
        & (Join-Path $package 'ffprobe.exe') -hide_banner -version
        if ($LASTEXITCODE -ne 0) { throw 'Packaged ffprobe cannot start.' }
        $recording = Join-Path $temporary 'recording test.mp4'
        & (Join-Path $package 'ffmpeg.exe') -hide_banner -loglevel error -nostdin -y `
            -f lavfi -i 'testsrc2=size=64x64:rate=2' -f lavfi -i 'sine=frequency=440:sample_rate=48000' `
            -t 1 -c:v libx264 -pix_fmt yuv420p -c:a aac $recording
        if ($LASTEXITCODE -ne 0) { throw 'Bundled H.264/AAC encoder smoke test failed.' }
        $probe = (& (Join-Path $package 'ffprobe.exe') -v error -show_entries stream=codec_name -of json $recording) | ConvertFrom-Json
        if ($LASTEXITCODE -ne 0 -or 'h264' -notin $probe.streams.codec_name -or 'aac' -notin $probe.streams.codec_name) {
            throw 'Packaged ffprobe did not confirm H.264 and AAC streams.'
        }
        # Also decode the result; starting ffmpeg alone does not test its codecs.
        & (Join-Path $package 'ffmpeg.exe') -v error -nostdin -i $recording -f null -
        if ($LASTEXITCODE -ne 0) { throw 'Bundled decoder smoke test failed.' }
        Write-Host 'Package passed payload hashes, application startup and H.264/AAC encode/probe/decode with a clean PATH.'
    } finally { Pop-Location }
} finally {
    $env:PATH = $oldPath
    $env:VARDA_NO_CLI_INSTALL = $oldNoInstall
    $env:FFMPEG_DIR = $oldFfmpeg
    if ([IO.Path]::GetDirectoryName($temporary) -ne [IO.Path]::GetTempPath().TrimEnd('\')) { throw 'Unsafe package test directory.' }
    Remove-Item -LiteralPath $temporary -Recurse -Force
}
