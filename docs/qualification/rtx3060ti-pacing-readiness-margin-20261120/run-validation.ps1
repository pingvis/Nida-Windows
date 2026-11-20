$ErrorActionPreference='Stop'
if (-not $env:NIDA_PACING_BUILD_ENV_READY) {
    . ./.windows-tools/build-env.ps1
    $env:NIDA_PACING_BUILD_ENV_READY='1'
}
$out='target/gpu-pacing-fix-20261120-readiness-margin'
$until=[DateTime]::UtcNow.AddHours(2)
while(-not(Test-Path "$out/diagnostics-complete.txt")) {
    if([DateTime]::UtcNow -gt $until) { throw 'Diagnostic queue did not finish' }
    Start-Sleep -Seconds 2
}
& "$out/run-readiness-pairs.ps1"
function Assert-GpuIdle {
    $activity=& nvidia-smi --query-gpu=timestamp,utilization.gpu,utilization.encoder,utilization.decoder --format=csv,noheader,nounits
    $activity | Add-Content "$out/validation-gpu-activity.csv"
    $fields=$activity -split ','
    if([int]$fields[1] -gt 30 -or [int]$fields[2] -gt 0) { throw 'GPU is busy; preserve active work' }
    if(Get-Process -ErrorAction SilentlyContinue | Where-Object ProcessName -match '^(varda|Nida-Windows|Adobe Premiere Pro|Adobe Media Encoder|AfterFX|blender|Resolve|obs64)$') { throw 'Potential rendering application appeared' }
}
$binary=(Resolve-Path "$out/pacing-test.exe").Path
Remove-Item Env:NIDA_VIDEO_PACING_TRACE,Env:NIDA_TEST_GPU_VIDEO_SHADER,Env:VARDA_SOFTWARE_DECODE,Env:VARDA_REQUIRE_GPU_RESIDENT,Env:VARDA_REQUIRE_HW_DECODE -ErrorAction SilentlyContinue
Assert-GpuIdle
& $binary 'internal::video::' --nocapture --test-threads=1 *> "$out/video-regressions.log"
if($LASTEXITCODE -ne 0) { throw 'Video regression failed' }
$env:VARDA_REQUIRE_GPU_RESIDENT='1'
$fixtures=@(
    'target/gpu-qualification/fixtures/continuous/h264-1080p60.mp4',
    'target/gpu-qualification/fixtures/continuous/h264-4k60.mp4',
    'target/gpu-qualification/fixtures/performance/hevc10-1080p60.mp4',
    'target/gpu-qualification-repro/p010-regression-fixtures/p010-full.mp4',
    'target/gpu-qualification-repro/p010-regression-fixtures/p010-wide.mp4',
    'target/gpu-qualification-repro/p010-regression-fixtures/p010-narrow.mp4'
)
foreach($fixture in $fixtures) {
    Assert-GpuIdle
    $env:VARDA_VIDEO_BENCH_CLIP=(Resolve-Path $fixture).Path
    $name=[IO.Path]::GetFileNameWithoutExtension($fixture)
    & $binary internal::video::gpu_windows::tests::gpu_resident_picture_seek_pool_and_fallback --exact --ignored --nocapture --test-threads=1 *> "$out/picture-$name.log"
    if($LASTEXITCODE -ne 0) { throw "Picture/seek/pool/fallback check failed: $name" }
}
Assert-GpuIdle
$env:NIDA_GPU_INTERLACED_CLIP=(Resolve-Path target/gpu-qualification/fixtures/h264-interlaced.mp4).Path
& $binary internal::video::gpu_windows::qualification::interlaced_uses_cpu_conversion_without_losing_the_picture --exact --ignored --nocapture --test-threads=1 *> "$out/interlaced.log"
if($LASTEXITCODE -ne 0) { throw 'Interlaced fallback failed' }
Assert-GpuIdle
$soak=@(
    'target/gpu-qualification/fixtures/performance/h264-1080p60.mp4',
    'target/gpu-qualification/fixtures/performance/hevc10-1080p60.mp4',
    'target/gpu-qualification/fixtures/performance/hdr10-1080p60.mp4',
    'target/gpu-qualification-repro/p010-regression-fixtures/p010-full.mp4',
    'target/gpu-qualification-repro/p010-regression-fixtures/p010-wide.mp4',
    'target/gpu-qualification-repro/p010-regression-fixtures/p010-narrow.mp4'
)|ForEach-Object { (Resolve-Path $_).Path }
$env:NIDA_GPU_SOAK_CLIPS=$soak -join [IO.Path]::PathSeparator
$env:NIDA_GPU_SOAK_SECONDS='180'
& $binary internal::video::gpu_windows::qualification::playlist_playback_seek_pause_reload_and_soak --exact --ignored --nocapture --test-threads=1 *> "$out/playlist-180s.log"
if($LASTEXITCODE -ne 0) { throw 'Three-minute playlist soak failed' }
'complete' > "$out/validation-complete.txt"
