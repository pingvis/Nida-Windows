$ErrorActionPreference='Stop'
. ./.windows-tools/build-env.ps1
$out='target/gpu-pacing-20261119'
$until=[DateTime]::UtcNow.AddHours(2)
while(-not(Test-Path "$out/builds-complete.txt")) {
    if([DateTime]::UtcNow -gt $until) { throw 'Build queue did not finish' }
    Start-Sleep -Seconds 2
}
Remove-Item Env:NIDA_TEST_GPU_VIDEO_SHADER,Env:VARDA_SOFTWARE_DECODE,Env:NIDA_VIDEO_PACING_TRACE -ErrorAction SilentlyContinue
rustc --edition 2021 -O "$out/timer-probe.rs" -o "$out/timer-probe.exe"
if($LASTEXITCODE -ne 0) { throw 'Timer probe did not compile' }
& "$out/timer-probe.exe" > "$out/timer-probe.jsonl"
if($LASTEXITCODE -ne 0) { throw 'Timer probe failed' }
cargo test --offline --manifest-path tools/gpu-video-checks/Cargo.toml --target-dir target/gpu-video-cpu-checks -j 1 *> "$out/cpu-checks.log"
if($LASTEXITCODE -ne 0) { throw 'CPU checks failed' }
$binary=(Resolve-Path "$out/diagnostic-test.exe").Path
$matrix=@(
    @{name='untraced-1080-0';resolution='1080p60';seconds=30;trace=$false},
    @{name='untraced-4k-0';resolution='4k60';seconds=30;trace=$false},
    @{name='untraced-1080-1';resolution='1080p60';seconds=30;trace=$false},
    @{name='untraced-4k-1';resolution='4k60';seconds=30;trace=$false},
    @{name='untraced-1080-2';resolution='1080p60';seconds=30;trace=$false},
    @{name='untraced-4k-2';resolution='4k60';seconds=30;trace=$false},
    @{name='traced-1080';resolution='1080p60';seconds=60;trace=$true},
    @{name='traced-4k';resolution='4k60';seconds=60;trace=$true},
    @{name='timer1-1080';resolution='1080p60';seconds=60;trace=$true;timer=$true},
    @{name='timer1-4k';resolution='4k60';seconds=60;trace=$true;timer=$true},
    @{name='poll-1080';resolution='1080p60';seconds=60;trace=$true;poll=$true},
    @{name='phase4-1080';resolution='1080p60';seconds=30;trace=$true;phase=4},
    @{name='phase8-1080';resolution='1080p60';seconds=30;trace=$true;phase=8},
    @{name='phase12-1080';resolution='1080p60';seconds=30;trace=$true;phase=12}
)
foreach($case in $matrix) {
    $activity=& nvidia-smi --query-gpu=timestamp,utilization.gpu,utilization.encoder,utilization.decoder --format=csv,noheader,nounits
    $activity | Add-Content "$out/diagnostic-gpu-activity.csv"
    $fields=$activity -split ','
    if([int]$fields[1] -gt 30 -or [int]$fields[2] -gt 0) { throw 'GPU is busy; preserve active work' }
    if(Get-Process -ErrorAction SilentlyContinue | Where-Object ProcessName -match '^(Adobe Premiere Pro|Adobe Media Encoder|AfterFX|blender|Resolve|obs64)$') { throw 'Potential rendering application appeared' }
    $options=@{TestBinary=$binary;Clip="target/gpu-qualification/fixtures/continuous/h264-$($case.resolution).mp4";Output="$out/$($case.name).log";Seconds=$case.seconds;Trace=$case.trace}
    if($case.timer) { $options.Timer1ms=$true }
    if($case.poll) { $options.PollOnly=$true }
    if($case.phase) { $options.PhaseMs=$case.phase }
    ./scripts/measure-video-pacing.ps1 @options
    Get-Content "$out/$($case.name).log" | Where-Object { $_ -match '^VIDEO_PLAYBACK' }
    if($case.trace) {
        & 'C:/Program Files/Blender Foundation/Blender 4.2/4.2/python/bin/python.exe' scripts/analyze-video-pacing.py "$out/$($case.name).log.trace.jsonl" > "$out/$($case.name).analysis.json"
        if($LASTEXITCODE -ne 0) { throw 'Trace analysis failed' }
    }
}
'complete' > "$out/diagnostics-complete.txt"
