$ErrorActionPreference = 'Stop'
if (-not $env:NIDA_PACING_BUILD_ENV_READY) {
    . ./.windows-tools/build-env.ps1
    $env:NIDA_PACING_BUILD_ENV_READY='1'
}
$out = 'target/gpu-pacing-fix-20261120-readiness'
$until = [DateTime]::UtcNow.AddHours(2)
while (-not (Test-Path "$out/builds-complete.txt")) {
    if ([DateTime]::UtcNow -gt $until) { throw 'Frozen build did not complete; no workloads started' }
    Start-Sleep -Seconds 5
}
$binary = (Resolve-Path "$out/pacing-test.exe").Path
Remove-Item Env:NIDA_VIDEO_BENCH_LEGACY_WAIT,Env:NIDA_VIDEO_BENCH_PERIODIC_WAKE,Env:NIDA_VIDEO_PACING_TRACE,Env:NIDA_VIDEO_BENCH_NO_READINESS -ErrorAction SilentlyContinue
& $binary internal::video::playback_diagnostics::audited_player_preserves_deadline_and_retry_contract --exact --nocapture --test-threads=1 *> "$out/adapter-regression.log"
if ($LASTEXITCODE -ne 0) { throw 'Adapter regression failed' }
& $binary stop_interrupts_the_engine_timer_wait --nocapture --test-threads=1 *> "$out/engine-stop-regression.log"
if ($LASTEXITCODE -ne 0) { throw 'Engine stop regression failed' }
if (-not (Select-String -LiteralPath "$out/engine-stop-regression.log" -SimpleMatch '1 passed; 0 failed')) { throw 'Engine regression did not run exactly one test' }
& "$out/run-measurements.ps1"
foreach ($resolution in @('1080p60','4k60')) {
    & "$out/run-app.ps1" -Name "app-headless-$resolution" -Headless -Seconds 70 -Resolution $resolution
    & 'C:/Program Files/Blender Foundation/Blender 4.2/4.2/python/bin/python.exe' scripts/analyze-video-pacing.py "$out/app-headless-$resolution.trace.jsonl" --pts-start 5 --seconds 60 > "$out/app-headless-$resolution.analysis.json"
    if ($LASTEXITCODE -ne 0) { throw 'App trace analysis failed' }
}
& "$out/run-app.ps1" -Name app-headless-untraced -Headless -Seconds 70 -NoTrace
& "$out/run-app.ps1" -Name app-windowed-untraced-occluded -Seconds 70 -NoTrace
& "$out/run-app.ps1" -Name app-headless-speed2 -Headless -Seconds 40 -Speed 2
& 'C:/Program Files/Blender Foundation/Blender 4.2/4.2/python/bin/python.exe' scripts/analyze-video-pacing.py "$out/app-headless-speed2.trace.jsonl" --pts-start 10 --seconds 20 > "$out/app-headless-speed2.analysis.json"
if ($LASTEXITCODE -ne 0) { throw 'Speed-cap trace analysis failed' }
& "$out/run-validation.ps1"
& "$out/run-app.ps1" -Name app-windowed-final-occluded -Seconds 110 -WarmupControls
& 'C:/Program Files/Blender Foundation/Blender 4.2/4.2/python/bin/python.exe' scripts/analyze-video-pacing.py "$out/app-windowed-final-occluded.trace.jsonl" --pts-start 40 --seconds 60 > "$out/app-windowed-final-occluded.analysis.json"
if ($LASTEXITCODE -ne 0) { throw 'GUI trace analysis failed' }
& "$out/run-app-seek-hold.ps1" -Name app-seek-hold-offset2 -Seconds 96 -PhaseStartMs 0
& 'C:/Program Files/Blender Foundation/Blender 4.2/4.2/python/bin/python.exe' scripts/analyze-video-pacing.py "$out/app-seek-hold-offset2.trace.jsonl" --pts-start 47.002 --seconds 60 > "$out/app-seek-hold-offset2.analysis.json"
if ($LASTEXITCODE -ne 0) { throw '60-second fractional-seek hold analysis failed' }
foreach ($phaseStart in @(0,0.25,0.5,0.75,6,12)) {
    Start-Sleep -Seconds 3
    $name = "app-seek-phase-$phaseStart"
    & "$out/run-app-phase-sweep.ps1" -Name $name -Seconds 72 -PhaseStartMs $phaseStart
    $manifest = Get-Content "$out/$name.manifest.json" -Raw | ConvertFrom-Json
    foreach ($phase in $manifest.warmup_control_events) {
        & 'C:/Program Files/Blender Foundation/Blender 4.2/4.2/python/bin/python.exe' scripts/analyze-video-pacing.py "$out/$name.trace.jsonl" --pts-start $phase.roi_pts --seconds $phase.roi_seconds > "$out/$name-offset$($phase.offset_ms).analysis.json"
        if ($LASTEXITCODE -ne 0) { throw 'Seek-phase trace analysis failed' }
    }
}
$settingsPaths = (Get-Content target/gpu-pacing-fix-20261120/settings-before.json -Raw | ConvertFrom-Json).Path
Get-FileHash -LiteralPath $settingsPaths | ConvertTo-Json > "$out/settings-after.json"
[DateTimeOffset]::UtcNow.ToString('o') > "$out/overnight-complete.txt"
