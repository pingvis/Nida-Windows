param(
    [string]$Ffmpeg = 'ffmpeg',
    [string]$OutputDirectory = 'target/nvenc-quality',
    [int]$Seconds = 6
)

# Deterministic local material only; no streaming service or credentials.
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$source = Join-Path $OutputDirectory 'reference.nut'
& $Ffmpeg -hide_banner -v error -y -f lavfi -i "testsrc2=size=2560x1440:rate=60:duration=$Seconds" -c:v ffv1 -level 3 $source
if ($LASTEXITCODE -ne 0) { throw 'Reference generation failed' }
$cases = @(
    @{ Name = 'p4'; Options = @('-preset', 'p4') },
    @{ Name = 'p4-qres-aq'; Options = @('-preset', 'p4', '-multipass', 'qres', '-spatial-aq', '1') },
    @{ Name = 'p5'; Options = @('-preset', 'p5') },
    @{ Name = 'p5-qres'; Options = @('-preset', 'p5', '-multipass', 'qres') },
    @{ Name = 'p5-qres-aq'; Options = @('-preset', 'p5', '-multipass', 'qres', '-spatial-aq', '1') }
)
$results = foreach ($kbps in @(8000, 32000)) {
    foreach ($case in $cases) {
        $name = "$($case.Name)-$kbps"
        $output = Join-Path $OutputDirectory "$name.mkv"
        $encodeLog = Join-Path $OutputDirectory "$name-encode.log"
        $qualityLog = Join-Path $OutputDirectory "$name-quality.log"
        $timer = [Diagnostics.Stopwatch]::StartNew()
        & $Ffmpeg -hide_banner -nostats -y -i $source -pix_fmt nv12 -c:v h264_nvenc @($case.Options) -tune ll -rc cbr -zerolatency 1 -bf 0 -g 120 -b:v "${kbps}k" -maxrate "${kbps}k" -bufsize "${kbps}k" $output 2> $encodeLog
        if ($LASTEXITCODE -ne 0) { throw "Encoding failed: $encodeLog" }
        $timer.Stop()
        & $Ffmpeg -hide_banner -nostats -i $output -i $source -lavfi '[0:v][1:v]ssim' -an -f null - 2> $qualityLog
        if ($LASTEXITCODE -ne 0) { throw "Quality measurement failed: $qualityLog" }
        $summary = (Select-String -LiteralPath $qualityLog -Pattern 'SSIM Y:').Line
        $result = [pscustomobject]@{
            Case = $case.Name
            Kbps = $kbps
            Seconds = $Seconds
            EncodeWallSeconds = [math]::Round($timer.Elapsed.TotalSeconds, 3)
            FileBytes = (Get-Item -LiteralPath $output).Length
            SSIM = $summary
        }
        $result | Format-List | Out-Host
        $result
    }
}
$results | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $OutputDirectory 'results.json')
