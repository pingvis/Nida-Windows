param(
    [Parameter(Mandatory)][string]$FfmpegRoot,
    [string]$OutputDirectory = 'target/stream-bitrate-probe'
)
$ErrorActionPreference = 'Stop'
$sdk = (Resolve-Path -LiteralPath $FfmpegRoot).Path
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$output = (Resolve-Path -LiteralPath $OutputDirectory).Path
$env:PATH = "$(Join-Path $sdk 'bin');$env:PATH"
$exe = Join-Path $output 'probe.exe'
& cl.exe /nologo /O2 /W4 /TC "/I$(Join-Path $sdk 'include')" "$PSScriptRoot/probe.c" "/Fo$output/probe.obj" "/Fe$exe" /link "/LIBPATH:$(Join-Path $sdk 'lib')" avcodec.lib avformat.lib avutil.lib
if ($LASTEXITCODE -ne 0) { throw 'Probe compilation failed' }
foreach ($encoder in @('h264_nvenc', 'libx264', 'libx264-no-hrd')) {
    $video = Join-Path $output "$encoder.mkv"
    $csv = Join-Path $output "$encoder.csv"
    & $exe $encoder $video $csv 2> (Join-Path $output "$encoder.log")
    if ($LASTEXITCODE -ne 0) { throw "Encoding failed: $encoder" }
    & (Join-Path $sdk 'bin/ffmpeg.exe') -v error -xerror -i $video -f null - 2> (Join-Path $output "$encoder-decode.log")
    if ($LASTEXITCODE -ne 0) { throw "Decode failed: $encoder" }
    $metadata = & (Join-Path $sdk 'bin/ffprobe.exe') -v error -count_frames -select_streams v:0 -show_entries stream=width,height,avg_frame_rate,nb_read_frames:format=duration -of json $video
    if ($LASTEXITCODE -ne 0) { throw "Probe failed: $encoder" }
    $metadata | Set-Content -LiteralPath (Join-Path $output "$encoder-metadata.json")
    $info = ($metadata -join "`n") | ConvertFrom-Json
    if ($info.streams[0].nb_read_frames -ne '540' -or $info.streams[0].avg_frame_rate -ne '60/1' -or [double]$info.format.duration -ne 9.0) {
        throw "Unexpected decoded frame count, frame rate or duration: $encoder"
    }
    $packets = @(Import-Csv -LiteralPath $csv)
    if ($packets.Count -ne 540) { throw "Wrong packet count: $encoder" }
    for ($i = 0; $i -lt $packets.Count; $i++) {
        if ([long]$packets[$i].pts -ne $i -or [long]$packets[$i].dts -ne $i) {
            throw "Non-contiguous timestamps: $encoder packet $i"
        }
    }
    foreach ($phase in 0..2) {
        $bytes = ($packets | Where-Object { [int]$_.pts -ge ($phase * 180) -and [int]$_.pts -lt (($phase + 1) * 180) } | Measure-Object -Property bytes -Sum).Sum
        # Also discard the first second to expose settled rate after reconfiguration.
        $settled = ($packets | Where-Object { [int]$_.pts -ge ($phase * 180 + 60) -and [int]$_.pts -lt (($phase + 1) * 180) } | Measure-Object -Property bytes -Sum).Sum
        [pscustomobject]@{ Encoder=$encoder; Phase=$phase; RequestedMbps=@(32,8,32)[$phase]; MeasuredMbps=[math]::Round($bytes * 8 / 3 / 1e6, 3); SettledMbps=[math]::Round($settled * 8 / 2 / 1e6, 3) }
    }
    Get-Content -LiteralPath (Join-Path $output "$encoder.log") | Select-Object -Last 1
}
