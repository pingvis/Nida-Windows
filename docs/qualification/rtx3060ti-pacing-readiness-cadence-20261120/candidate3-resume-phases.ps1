$ErrorActionPreference='Stop'
. ./.windows-tools/build-env.ps1
$env:NIDA_PACING_BUILD_ENV_READY='1'
$out='target/gpu-pacing-fix-20261120-final'
foreach ($phaseStart in @(0,6,12)) {
    Start-Sleep -Seconds 3
    $name="app-seek-phase-resumed-$phaseStart"
    & "$out/run-app-phase-sweep.ps1" -Name $name -Seconds 72 -PhaseStartMs $phaseStart
    $manifest=Get-Content "$out/$name.manifest.json" -Raw | ConvertFrom-Json
    foreach ($phase in $manifest.warmup_control_events) {
        & 'C:/Program Files/Blender Foundation/Blender 4.2/4.2/python/bin/python.exe' scripts/analyze-video-pacing.py "$out/$name.trace.jsonl" --pts-start $phase.roi_pts --seconds $phase.roi_seconds > "$out/$name-offset$($phase.offset_ms).analysis.json"
        if($LASTEXITCODE -ne 0){throw 'Seek-phase trace analysis failed'}
    }
}
$settingsPaths=(Get-Content target/gpu-pacing-fix-20261120/settings-before.json -Raw | ConvertFrom-Json).Path
Get-FileHash -LiteralPath $settingsPaths | ConvertTo-Json > "$out/settings-after.json"
[DateTimeOffset]::UtcNow.ToString('o') > "$out/overnight-complete.txt"
