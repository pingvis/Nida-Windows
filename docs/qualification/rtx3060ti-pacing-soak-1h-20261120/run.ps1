$ErrorActionPreference='Stop'
. ./.windows-tools/build-env.ps1
$out=(Resolve-Path target/gpu-pacing-soak-20261120-1h).Path
$binary=(Resolve-Path target/gpu-pacing-fix-20261120-fence-event/pacing-test.exe).Path
$expected='F8977B60914974B4DBBFBC582505C6BDB184AD56938D9BE4D738E3C7DB13A91C'
if((Get-FileHash $binary).Hash -ne $expected){throw 'Qualified binary mismatch'}
if(Test-Path "$out/soak.log"){throw 'Preserve existing soak evidence'}
$activity=@()
for($sample=0;$sample -lt 3;$sample++){
    $reading=& nvidia-smi --query-gpu=timestamp,utilization.gpu,utilization.encoder,utilization.decoder --format=csv,noheader,nounits
    if($LASTEXITCODE -ne 0){throw 'Cannot verify GPU activity'}
    $activity+=$reading
    $fields=$reading -split ','
    if([int]$fields[1] -gt 30 -or [int]$fields[2] -gt 0 -or [int]$fields[3] -gt 0){throw 'GPU busy; preserve active work'}
    Start-Sleep -Seconds 1
}
if(Get-Process -ErrorAction SilentlyContinue|Where-Object ProcessName -match '^(varda|Nida-Windows|Adobe Premiere Pro|Adobe Media Encoder|AfterFX|blender|Resolve|obs64|vMix64)$'){throw 'Potential active rendering application'}
$activity > "$out/gpu-before.csv"
$clips=@(
    'target/gpu-qualification/fixtures/performance/h264-1080p60.mp4',
    'target/gpu-qualification/fixtures/performance/h264-4k60.mp4',
    'target/gpu-qualification/fixtures/performance/hevc10-1080p60.mp4',
    'target/gpu-qualification/fixtures/performance/hdr10-1080p60.mp4',
    'target/gpu-qualification-repro/p010-regression-fixtures/p010-full.mp4',
    'target/gpu-qualification-repro/p010-regression-fixtures/p010-wide.mp4',
    'target/gpu-qualification-repro/p010-regression-fixtures/p010-narrow.mp4'
)|ForEach-Object{(Resolve-Path $_).Path}
Remove-Item Env:NIDA_VIDEO_PACING_TRACE,Env:NIDA_TEST_GPU_VIDEO_SHADER,Env:NIDA_VIDEO_BENCH_COPY_POLL,Env:NIDA_VIDEO_BENCH_LEGACY_WAIT,Env:NIDA_VIDEO_BENCH_NO_READINESS,Env:VARDA_SOFTWARE_DECODE -ErrorAction SilentlyContinue
$env:VARDA_REQUIRE_GPU_RESIDENT='1'
$env:NIDA_GPU_SOAK_CLIPS=$clips -join [IO.Path]::PathSeparator
$env:NIDA_GPU_SOAK_SECONDS='3600'
$started=[DateTimeOffset]::UtcNow
[ordered]@{started_utc=$started.ToString('o');seconds=3600;qualified_commit='c19e528';binary=$binary;binary_sha256=$expected;fixtures=@(Get-FileHash $clips);kind='release qualification playlist harness; not scanout or per-PTS pacing measurement'}|ConvertTo-Json -Depth 5 > "$out/manifest.json"
& $binary internal::video::gpu_windows::qualification::playlist_playback_seek_pause_reload_and_soak --exact --ignored --nocapture --test-threads=1 *> "$out/soak.log"
$exitCode=$LASTEXITCODE
$complete=Select-String -LiteralPath "$out/soak.log" -Pattern '^GPU_PLAYLIST_SOAK_COMPLETE ' | Select-Object -Last 1
$summary=if($complete){$complete.Line.Substring('GPU_PLAYLIST_SOAK_COMPLETE '.Length)|ConvertFrom-Json}else{$null}
$passed=$exitCode -eq 0 -and $summary -and $summary.wall_s -ge 3600 -and (Select-String -LiteralPath "$out/soak.log" -SimpleMatch '1 passed; 0 failed')
[ordered]@{finished_utc=[DateTimeOffset]::UtcNow.ToString('o');exit_code=$exitCode;passed=[bool]$passed;summary=$summary}|ConvertTo-Json -Depth 5 > "$out/result.json"
if(-not $passed){throw 'One-hour soak failed; inspect preserved log'}
