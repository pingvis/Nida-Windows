$ErrorActionPreference = 'Stop'
if (-not $env:NIDA_PACING_BUILD_ENV_READY) {
    . ./.windows-tools/build-env.ps1
    $env:NIDA_PACING_BUILD_ENV_READY='1'
}
$out = 'target/gpu-pacing-fix-20261120-fence-event'
if (-not (Test-Path "$out/builds-complete.txt")) { throw 'Frozen builds are not complete' }
$binary = (Resolve-Path "$out/pacing-test.exe").Path
Remove-Item Env:NIDA_TEST_GPU_VIDEO_SHADER,Env:VARDA_SOFTWARE_DECODE -ErrorAction SilentlyContinue
$cases = @(
    @{name='readiness-phase225';resolution='1080p60';seconds=30;phase=2.25;trace=$true},
    @{name='readiness-phase250';resolution='1080p60';seconds=30;phase=2.5;trace=$true},
    @{name='readiness-phase275';resolution='1080p60';seconds=30;phase=2.75;trace=$true},
    @{name='readiness-phase300';resolution='1080p60';seconds=30;phase=3;trace=$true},
    @{name='three-clips-phase250';resolution='1080p60';seconds=60;phase=2.5;trace=$true;clips=3},
    @{name='three-clips-phase300';resolution='1080p60';seconds=60;phase=3;trace=$true;clips=3}
)
foreach ($case in $cases) {
    if (Test-Path "$out/$($case.name).log.pacing.json") {
        $saved = Get-Content "$out/$($case.name).log.manifest.json" -Raw | ConvertFrom-Json
        if ($saved.binary_sha256 -ne (Get-FileHash $binary).Hash) { throw 'Completed case uses another binary' }
        continue
    }
    # Wait for idle samples after our preceding process exits. Utilization is
    # sampled over a trailing interval and can still include our completed case.
    $idleSamples = 0
    for ($attempt = 0; $attempt -lt 8 -and $idleSamples -lt 3; $attempt++) {
        $reading = & nvidia-smi --query-gpu=timestamp,utilization.gpu,utilization.encoder,utilization.decoder --format=csv,noheader,nounits
        if ($LASTEXITCODE -ne 0) { throw 'Cannot verify GPU activity' }
        $reading | Add-Content "$out/measurement-gpu-activity.csv"
        $values = $reading -split ','
        if ([int]$values[1] -le 30 -and [int]$values[2] -eq 0 -and [int]$values[3] -eq 0) { $idleSamples++ } else { $idleSamples = 0 }
        Start-Sleep -Seconds 1
    }
    if ($idleSamples -lt 3) { throw 'GPU remains busy; preserve active work' }
    $activity = & nvidia-smi --query-gpu=timestamp,utilization.gpu,utilization.encoder,utilization.decoder --format=csv,noheader,nounits
    if ($LASTEXITCODE -ne 0) { throw 'Cannot verify GPU activity' }
    $activity | Add-Content "$out/measurement-gpu-activity.csv"
    $fields = $activity -split ','
    if ([int]$fields[1] -gt 30 -or [int]$fields[2] -gt 0 -or [int]$fields[3] -gt 0) { throw 'GPU busy; preserve active work' }
    if (Get-Process -ErrorAction SilentlyContinue | Where-Object ProcessName -match '^(varda|Nida-Windows|Adobe Premiere Pro|Adobe Media Encoder|AfterFX|blender|Resolve|obs64|vMix64)$') { throw 'Potential rendering application appeared' }
    if (Test-Path "$out/$($case.name).log") { throw 'Use a fresh measurement name' }
    $options = @{TestBinary=$binary;Clip="target/gpu-qualification/fixtures/continuous/h264-$($case.resolution).mp4";Output="$out/$($case.name).log";Seconds=$case.seconds;Trace=[bool]$case.trace;LegacyWait=[bool]$case.legacy;PeriodicWake=[bool]$case.periodic}
    $options.Clips=if($case.clips){$case.clips}else{1}
    $options.NoReadiness=[bool]$case.noReadiness
    if ($case.phase) { $options.PhaseMs=$case.phase }
    ./scripts/measure-video-pacing.ps1 @options
    Get-Content "$out/$($case.name).log" | Where-Object { $_ -match '^VIDEO_PLAYBACK' }
    if ($case.trace -and $case.clips -gt 1) {
        & 'C:/Program Files/Blender Foundation/Blender 4.2/4.2/python/bin/python.exe' scripts/analyze-video-pacing.py "$out/$($case.name).log.trace.jsonl" --readiness-only > "$out/$($case.name).readiness.json"
        if ($LASTEXITCODE -ne 0) { throw 'Shared-budget analysis failed' }
    } elseif ($case.trace) {
        & 'C:/Program Files/Blender Foundation/Blender 4.2/4.2/python/bin/python.exe' scripts/analyze-video-pacing.py "$out/$($case.name).log.trace.jsonl" > "$out/$($case.name).analysis.json"
        if ($LASTEXITCODE -ne 0) { throw 'Trace analysis failed' }
    }
}
'complete' > "$out/diagnostics-complete.txt"
