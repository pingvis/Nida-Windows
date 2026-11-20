param([Parameter(Mandatory)][string]$Name,[switch]$Headless,[int]$Seconds=70,[ValidateSet('1080p60','4k60')][string]$Resolution='1080p60',[switch]$WarmupControls,[switch]$Visible,[switch]$NoTrace,[double]$Speed=1.0)
$ErrorActionPreference='Stop'
if (-not $env:NIDA_PACING_BUILD_ENV_READY) {
    . ./.windows-tools/build-env.ps1
    $env:NIDA_PACING_BUILD_ENV_READY='1'
}
$root=(Resolve-Path target/gpu-pacing-fix-20261120-qualified).Path
$work=Join-Path $root "$Name-workspace"
if(Test-Path $work) { throw 'Use a fresh app workspace' }
New-Item -ItemType Directory $work | Out-Null
$binary=(Resolve-Path target/release/varda.exe).Path
if ((Get-FileHash $binary).Hash -ne (Get-FileHash (Join-Path $root 'pacing-app.exe')).Hash) { throw 'Production release executable differs from the frozen artifact' }
if(Get-NetTCPConnection -State Listen -LocalPort 18089 -ErrorAction SilentlyContinue) { throw 'Diagnostic API port is already in use' }
$activity=& nvidia-smi --query-gpu=timestamp,utilization.gpu,utilization.encoder,utilization.decoder --format=csv,noheader,nounits
$activity > "$root/$Name.gpu-before.csv"
$fields=$activity -split ','
if([int]$fields[1] -gt 30 -or [int]$fields[2] -gt 0) { throw 'GPU is busy; preserve active work' }
if(Get-Process -ErrorAction SilentlyContinue | Where-Object ProcessName -match '^(Adobe Premiere Pro|Adobe Media Encoder|AfterFX|blender|Resolve|obs64)$') { throw 'Potential rendering application appeared' }
$env:NIDA_VIDEO_PACING_TRACE=if($NoTrace){$null}else{Join-Path $root "$Name.trace.jsonl"}
$env:VARDA_NO_CLI_INSTALL='1'
$env:VARDA_LOG_FILE=Join-Path $root "$Name.app.log"
Remove-Item Env:NIDA_TEST_GPU_VIDEO_SHADER,Env:VARDA_SOFTWARE_DECODE -ErrorAction SilentlyContinue
if($env:NIDA_VIDEO_PACING_TRACE -and (Test-Path $env:NIDA_VIDEO_PACING_TRACE)) { throw 'Trace already exists' }
$launchArgs=@('--workspace',('"{0}"' -f $work),'--port','18089','--fps','60','--no-osc','--no-ndi','--no-html','--no-screen-capture','--no-camera','--no-powerpoint')
if($Headless) { $launchArgs += '--headless' }
$windowStyle=if($Visible){'Normal'}else{'Hidden'}
$p=Start-Process -FilePath $binary -ArgumentList $launchArgs -WindowStyle $windowStyle -PassThru -RedirectStandardOutput "$root/$Name.stdout.log" -RedirectStandardError "$root/$Name.stderr.log"
$url='http://127.0.0.1:18089'
$started=[DateTimeOffset]::UtcNow
try {
    $ready=$false
    for($i=0;$i -lt 120;$i++) {
        if($p.HasExited) { throw "App exited at startup: $($p.ExitCode)" }
        try { $null=Invoke-RestMethod "$url/api/health"; $ready=$true; break } catch { Start-Sleep -Milliseconds 500 }
    }
    if(-not $ready) { throw 'API did not become ready' }
    if(-not (Get-NetTCPConnection -State Listen -LocalPort 18089 | Where-Object OwningProcess -eq $p.Id)) { throw 'API listener is not owned by the diagnostic app' }
    if($Resolution -eq '4k60') { $null=Invoke-RestMethod "$url/api/resolution" -Method Put -ContentType application/json -Body '{"width":3840,"height":2160}' }
    $before=Invoke-RestMethod "$url/api/state"
    $before | ConvertTo-Json -Depth 40 > "$root/$Name.before.json"
    $channel=(Invoke-RestMethod "$url/api/channels" -Method Post).uuid
    if(-not $channel) { throw 'No channel UUID' }
    $clip=(Resolve-Path "target/gpu-qualification/fixtures/continuous/h264-$Resolution.mp4").Path
    $deck=(Invoke-RestMethod "$url/api/channels/$channel/decks/video" -Method Post -ContentType application/json -Body (@{path=$clip}|ConvertTo-Json)).uuid
    if(-not $deck) { throw 'No deck UUID' }
    $null=Invoke-RestMethod "$url/api/channels/$channel/opacity" -Method Put -ContentType application/json -Body '{"opacity":1.0}'
    $null=Invoke-RestMethod "$url/api/main/mode" -Method Put -ContentType application/json -Body '{"mode":"program"}'
    $null=Invoke-RestMethod "$url/api/main/take" -Method Post -ContentType application/json -Body (@{source=@{Channel=$channel};fade=$false}|ConvertTo-Json)
    if ($Speed -ne 1.0) { $null=Invoke-RestMethod "$url/api/decks/$deck/video/speed" -Method Put -ContentType application/json -Body (@{speed=$Speed}|ConvertTo-Json) }
    Start-Sleep -Milliseconds 300
    $state=Invoke-RestMethod "$url/api/state"
    $playback=@($state.mixer.channels | ForEach-Object { $_.decks } | Where-Object uuid -eq $deck)[0].video_playback
    if(-not $playback) { throw 'No video playback snapshot' }
    if(-not $playback.playing) { $null=Invoke-RestMethod "$url/api/decks/$deck/video/toggle-play" -Method Post }
    $samples=@()
    $controlEvents=@()
    $sampleClock=[Diagnostics.Stopwatch]::StartNew()
    $previousGpuFrames=0
    for($i=0;$i -lt $Seconds;$i++) {
        Start-Sleep -Seconds 1
        if($WarmupControls -and $i -eq 9) {
            $null=Invoke-RestMethod "$url/api/decks/$deck/video/seek" -Method Put -ContentType application/json -Body '{"position_secs":5.0}'
            $controlEvents += @{action='seek_5s';utc=[DateTimeOffset]::UtcNow.ToString('o')}
        }
        if($WarmupControls -and $i -in @(19,20)) {
            $null=Invoke-RestMethod "$url/api/decks/$deck/video/toggle-play" -Method Post
            $controlEvents += @{action=if($i -eq 19){'pause'}else{'resume'};utc=[DateTimeOffset]::UtcNow.ToString('o')}
        }
        if($i -in @(4,34,64,69) -or $i -eq ($Seconds-1)) {
            $s=Invoke-RestMethod "$url/api/state"
            $sampleSeconds=$sampleClock.Elapsed.TotalSeconds
            $p.Refresh()
            $cpuMs=$p.TotalProcessorTime.TotalMilliseconds
            $s | ConvertTo-Json -Depth 40 > "$root/$Name.state-$i.json"
            $v=@($s.mixer.channels | ForEach-Object { $_.decks } | Where-Object uuid -eq $deck)[0].video_playback
            if(-not $v.gpu_resident -or -not $v.playing -or $v.delivery.downloaded_bytes -ne 0 -or $v.delivery.uploaded_bytes -ne 0 -or $v.delivery.fallback_reason -or $v.delivery.peak_slots -gt 3) { throw 'App playback invariant failed' }
            if($v.delivery.gpu_frames -le $previousGpuFrames) { throw 'App did not deliver new GPU pictures' }
            $previousGpuFrames=$v.delivery.gpu_frames
            $samples += [ordered]@{index=$i;elapsed_s=$sampleSeconds;process_cpu_ms=$cpuMs;video=$v}
        }
    }
    $p.Refresh()
    $samples | ConvertTo-Json -Depth 20 > "$root/$Name.samples.json"
    [ordered]@{started_utc=$started.ToString('o');finished_utc=[DateTimeOffset]::UtcNow.ToString('o');pid=$p.Id;process_start_utc=$p.StartTime.ToUniversalTime().ToString('o');main_window_title=$p.MainWindowTitle;channel=$channel;deck=$deck;clip=$clip;resolution=$Resolution;headless=[bool]$Headless;seconds=$Seconds;requested_speed=$Speed;warmup_control_events=$controlEvents;binary_sha256=(Get-FileHash $binary).Hash;trace=$env:NIDA_VIDEO_PACING_TRACE} | ConvertTo-Json -Depth 5 > "$root/$Name.manifest.json"
} finally {
    if(Get-NetTCPConnection -State Listen -LocalPort 18089 -ErrorAction SilentlyContinue | Where-Object OwningProcess -eq $p.Id) {
        try { $null=Invoke-RestMethod "$url/api/shutdown" -Method Post } catch { Write-Warning $_ }
    }
    if(-not $p.WaitForExit(30000)) { throw "Diagnostic app PID $($p.Id) has not exited; preserve it for inspection" }
}
if($p.ExitCode -ne 0) { throw "App failed: $($p.ExitCode)" }
