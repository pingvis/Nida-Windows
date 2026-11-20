$ErrorActionPreference = 'Stop'
if (-not $env:NIDA_PACING_BUILD_ENV_READY) {
    . ./.windows-tools/build-env.ps1
    $env:NIDA_PACING_BUILD_ENV_READY='1'
}
$out = (Resolve-Path target/gpu-pacing-fix-20261120-readiness).Path
$paths = @('Cargo.toml','Cargo.lock','src/internal/video/mod.rs','src/internal/video/hap.rs','src/internal/video/cadence.rs','src/internal/video/command_channel.rs','src/internal/video/frame_mailbox.rs','src/internal/video/frame_readiness.rs','src/internal/video/gpu_windows/qualification.rs','src/internal/deck/render.rs','src/internal/deck/playlist.rs','src/internal/deck/slides.rs','src/internal/channel/mod.rs','src/internal/mixer/render.rs','src/internal/video/pacing_trace.rs','src/internal/video/playback_diagnostics.rs','src/internal/video/gpu_windows.rs','src/internal/video/gpu_windows/tests.rs','src/internal/video/gpu_video.wgsl','src/internal/renderer/context.rs','src/usecases/ui/runner/mod.rs','src/usecases/ui/runner/engine_thread.rs','scripts/measure-video-pacing.ps1','scripts/analyze-video-pacing.py','tools/gpu-video-checks/src/lib.rs','tools/gpu-video-checks/Cargo.toml','tools/gpu-video-checks/Cargo.lock')
$before = @($paths | ForEach-Object { Get-FileHash -LiteralPath $_ -Algorithm SHA256 })
$before | ConvertTo-Json -Depth 4 > "$out/source-hashes.json"
foreach ($path in $paths) {
    $copy = Join-Path "$out/source-snapshot" $path
    New-Item -ItemType Directory -Force -Path (Split-Path $copy -Parent) | Out-Null
    Copy-Item -LiteralPath $path -Destination $copy
}
if (Test-Path target/release/varda.exe) { Copy-Item -LiteralPath target/release/varda.exe -Destination "$out/prebuild-app.exe" }
cargo test --offline --manifest-path tools/gpu-video-checks/Cargo.toml *> "$out/cpu-checks.log"
if ($LASTEXITCODE -ne 0) { throw 'CPU checks failed' }
cargo clippy --offline --lib --no-default-features --features screen-capture,windows-browser -j 1 -- -D warnings *> "$out/clippy.log"
if ($LASTEXITCODE -ne 0) { throw 'Clippy failed' }
cargo check --offline --tests --no-default-features --features screen-capture,windows-browser -j 1 *> "$out/check-tests.log"
if ($LASTEXITCODE -ne 0) { throw 'Test check failed' }
cargo test --offline --release --no-default-features --features screen-capture,windows-browser --lib --no-run -j 1 *> "$out/build-test.log"
if ($LASTEXITCODE -ne 0) { throw 'Release test build failed' }
$test = Get-ChildItem target/release/deps/varda-*.exe | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $test) { throw 'No release test executable' }
Copy-Item -LiteralPath $test.FullName -Destination "$out/pacing-test.exe"
cargo build --offline --release --no-default-features --features screen-capture,windows-browser -j 1 *> "$out/build-app.log"
if ($LASTEXITCODE -ne 0) { throw 'Release app build failed' }
Copy-Item -LiteralPath target/release/varda.exe -Destination "$out/pacing-app.exe"
$after = @($paths | ForEach-Object { Get-FileHash -LiteralPath $_ -Algorithm SHA256 })
if (Compare-Object ($before | ForEach-Object { "$($_.Path) $($_.Hash)" }) ($after | ForEach-Object { "$($_.Path) $($_.Hash)" })) { throw 'Source changed during frozen build' }
Get-FileHash "$out/pacing-test.exe","$out/pacing-app.exe" | ConvertTo-Json > "$out/binary-hashes.json"
[DateTimeOffset]::UtcNow.ToString('o') > "$out/builds-complete.txt"
