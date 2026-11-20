$ErrorActionPreference='Stop'
if(-not(Test-Path target/gpu-pacing-fix-20261120-qualified/overnight-complete.txt)){throw 'Wait until serial GPU qualification ends'}
. ./.windows-tools/build-env.ps1
$out=(Resolve-Path target/gpu-pacing-fix-20261120-qualified).Path
$deps=(Resolve-Path tools/gpu-video-checks/target/debug/deps).Path
$sys=Get-ChildItem -LiteralPath $deps -Filter 'libwindows_sys-*.rlib' | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if(-not $sys){throw 'Missing CPU harness windows-sys dependency'}
if(Test-Path "$out/wait-cpu-probe.jsonl"){throw 'Preserve existing CPU probe'}
rustc --edition=2024 -O "$out/wait-cpu-probe.rs" --extern "windows_sys=$($sys.FullName)" -L "dependency=$deps" -o "$out/wait-cpu-probe.exe" *> "$out/wait-cpu-probe-build.log"
if($LASTEXITCODE -ne 0){Get-Content "$out/wait-cpu-probe-build.log";throw 'CPU probe compile failed'}
& "$out/wait-cpu-probe.exe" > "$out/wait-cpu-probe.jsonl"
if($LASTEXITCODE -ne 0){throw 'CPU probe failed'}
[ordered]@{scope='CPU-only wait primitive; no video/GPU workload';finished_utc=[DateTimeOffset]::UtcNow.ToString('o');hashes=@(Get-FileHash "$out/wait-cpu-probe.rs","$out/wait-cpu-probe.exe","src/internal/video/command_channel.rs")} | ConvertTo-Json -Depth 4 > "$out/wait-cpu-probe.manifest.json"
