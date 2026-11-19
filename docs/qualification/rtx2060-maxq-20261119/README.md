# RTX 2060 Max-Q / hybrid AMD qualification evidence

See [the hardware report](../../windows-gpu-resident-qualification-rtx2060-maxq.md).
The measured source is base `1e4cf2055e8881ab3d8e0cd2002bedf984fe3256` plus the
SDR P010 fix in `src/internal/video/gpu_video.wgsl`. `fixed-binaries.json` records
the exact release app, test binary and embedded shader hashes. Videos, native
binaries, settings backups and large telemetry streams are retained locally under
`target/gpu-qualification-local/`; they are not committed.

Archived text logs have normalized line endings and trailing whitespace; their
measurements and messages are unchanged. Original logs remain local.

## Evidence contents

- `fixtures.json`: SHA-256 and probed codec/profile, depth, colour, field, frame
  rate, dimensions, SAR and rotation metadata for all 20 qualification fixtures.
- `correctness.json`: 17 supported progressive groups, each with five seek
  comparisons and pool/reuse/ping-pong checks; AV1 is explicitly unsupported.
  `packed_mean` is a diagnostic against the downloaded hardware picture converted
  on CPU. The independent software-reference `< 0.5` mean-error check is a
  separate, unchanged assertion in the test.
- `benchmark-runs.json`: all 30 raw measurement records and exact binary hashes.
- `benchmark-summary.json`: five-run medians/ranges, delivery counters, sampled
  RSS, and global NVIDIA telemetry. CPU percentages use one core as 100%.
- `short-stability.json`: completed 180-second production playlist run.
- `two-hour-soak-status.json`: **partial snapshot only**, until a completed
  7,200-second result replaces it. It must not be counted as a two-hour pass.
- `baseline-pictures-hevc10-1080p60.log` and fixed picture logs: reproduction of
  the colour failure and the passing embedded production shader.
- Native adapter probe sources/logs, a fresh soak GPU-process snapshot, driver
  event-log snapshot, build/runtime provenance, and settings hash verification.

The continuous Windows GPU counter collector returned empty per-process sample
arrays during the timed runs. No per-run decoder LUID or GPU-process-memory
measurement is claimed from that collector. A fresh snapshot during the soak
does show NVIDIA 3D and VideoDecode engines on LUID `0:f857`. The standalone
production-call D3D11VA device probe selects AMD LUID `0:e217`. These limits are
important when interpreting the hybrid comparison.

## Reproduce the fixtures

Use an x64 MSVC developer shell and the matching FFmpeg 9 development SDK described
in [Windows development](../../windows-development.md). Put that SDK's `bin` on
PATH. The recorded build is `n9.0.2-36-g6882969ce9-20261027`; different FFmpeg or
encoder builds may produce different binary hashes. These scripts use software
encoders only, probe their outputs, and refuse to overwrite existing videos.

Run from the repository root, in this order:

```powershell
$evidence = 'docs/qualification/rtx2060-maxq-20261119'
& "$evidence/01-generate-fixtures.ps1"
& "$evidence/02-extra-fixtures.ps1"
& "$evidence/03-verify-fixtures.ps1"
& "$evidence/04-p010-regressions.ps1"
```

Outputs are under `target/gpu-qualification-repro/`. Use `fixtures-verified/`
for the primary matrix; the first-pass raw fixtures are retained separately.
The continuous H.264 performance clips are 120 seconds at 1080p60 and 4K60.
The three added SDR P010 cases cover full range and wide/narrow SAR.

## Build and CPU checks

Configure `FFMPEG_DIR`, `LIBCLANG_PATH`, native AEC3 and the x64 MSVC/Windows SDK
environment per the development guide. Use the repository release profile without
optimization overrides. A short target directory avoids MSVC object-path limits.

```powershell
$env:CARGO_TARGET_DIR = Join-Path $PWD 'target/gq'
Remove-Item Env:VARDA_SOFTWARE_DECODE, Env:NIDA_TEST_GPU_VIDEO_SHADER, Env:NIDA_TEST_GPU_VIDEO_DUMP -ErrorAction SilentlyContinue
cargo check --offline --tests --no-default-features --features screen-capture,windows-browser -j 1
cargo test --offline --manifest-path tools/gpu-video-checks/Cargo.toml --target-dir target/gpu-video-cpu-checks -j 1
cargo build --offline --release --no-default-features --features screen-capture,windows-browser -j 1
cargo test --offline --release --no-default-features --features screen-capture,windows-browser --lib --no-run -j 1
```

Set `$test` to the `Executable unittests src/lib.rs` path printed by Cargo, not
the app executable. Keep the matching FFmpeg runtime DLLs on PATH; app-local DLLs
beside the test executable resolved the loader issue on this PC.

## Serial GPU correctness checks

Run GPU workloads only after confirming that other rendering/exporting/gaming is
finished. Do not run these concurrently with another GPU qualification process.

```powershell
$test = '<release unit-test executable printed by Cargo>'
$fixtureRoot = Join-Path $PWD 'target/gpu-qualification-repro'
$env:VARDA_REQUIRE_GPU_RESIDENT = '1'
$pictureTest = 'internal::video::gpu_windows::tests::gpu_resident_picture_seek_pool_and_fallback'
$progressive = @(Get-ChildItem "$fixtureRoot/fixtures-verified" -Filter '*.mp4' |
    Where-Object { $_.BaseName -notin @('av1-1080p60', 'interlaced', 'h264-playlist') })
$progressive += @(Get-ChildItem "$fixtureRoot/p010-regression-fixtures" -Filter '*.mp4')
foreach ($clip in $progressive) {
    $env:VARDA_VIDEO_BENCH_CLIP = $clip.FullName
    & $test $pictureTest --exact --ignored --nocapture --test-threads=1
    if ($LASTEXITCODE -ne 0) { throw "Required GPU-resident test failed: $clip" }
}
$env:NIDA_GPU_INTERLACED_CLIP = "$fixtureRoot/fixtures-verified/interlaced.mp4"
& $test internal::video::gpu_windows::qualification::interlaced_uses_cpu_conversion_without_losing_the_picture --exact --ignored --nocapture --test-threads=1
```

Run AV1 separately with the same picture test and its fixture. On this machine
it fails because the adapter has no AV1 decode profile; that is not a successful
GPU-resident result. Interlaced and ping-pong successes mean correct **software
fallback**, not residency.

The ordinary regression group is:

```powershell
Remove-Item Env:VARDA_REQUIRE_HW_DECODE, Env:VARDA_REQUIRE_GPU_RESIDENT -ErrorAction SilentlyContinue
& $test 'internal::video::' --nocapture --test-threads=1
```

The two existing media-engine picture/download tests were also run with
`VARDA_REQUIRE_HW_DECODE=1`. Both stop at their required AV1 case on this hardware.
Do not label those whole tests as hardware passes. The ordinary group logs the
actual media-engine choice per codec; required H.264 timed measurements separately
assert hardware decoding throughout.

## Five interleaved 60-second release measurements

After initial correctness checks pass, run one clip at a time, with no compilation
or fixture generation in progress. Each method/resolution cell must contain five
successful measurements. The existing helper asserts the expected decode path.

```powershell
Remove-Item Env:VARDA_REQUIRE_HW_DECODE, Env:VARDA_REQUIRE_GPU_RESIDENT -ErrorAction SilentlyContinue
$methods = @('software', 'hardware-ram', 'gpu-resident')
New-Item -ItemType Directory -Force target/gpu-qualification-repro/results | Out-Null
foreach ($resolution in @('1080p60', '4k60')) {
    for ($round = 0; $round -lt 5; $round++) {
        for ($offset = 0; $offset -lt 3; $offset++) {
            $method = $methods[($round + $offset) % 3]
            $benchOptions = @{
                TestBinary = $test
                Clip = "$fixtureRoot/fixtures-verified/h264-$resolution.mp4"
                Output = "target/gpu-qualification-repro/results/$resolution-$method-$round.log"
                Repeats = 1; Clips = 1; Seconds = 60; Playback = $true
            }
            if ($method -eq 'hardware-ram') { $benchOptions.Hardware = $true }
            if ($method -eq 'gpu-resident') { $benchOptions.GpuResident = $true }
            ./scripts/measure-video.ps1 @benchOptions
        }
    }
}
```

Fresh pictures, render reuse/late ticks and cumulative discarded GPU payloads
are different counters. The latter include the one-second warmup's unconsumed
GPU pictures and are not directly comparable to the CPU path's zero GPU-payload
discard count. `copy_wait_ns` is elapsed worker fence/copy time, not a GPU timestamp.

## Production playlist smoke and soak

Use the embedded production shader and supported H.264/HDR10/SDR Main10 fixtures:

```powershell
$env:VARDA_REQUIRE_GPU_RESIDENT = '1'
$env:NIDA_GPU_SOAK_CLIPS = @(
    "$fixtureRoot/fixtures-verified/h264-playlist.mp4",
    "$fixtureRoot/fixtures-verified/hdr10-1080p60.mp4",
    "$fixtureRoot/fixtures-verified/hevc10-1080p60.mp4"
) -join ';'
$soakTest = 'internal::video::gpu_windows::qualification::playlist_playback_seek_pause_reload_and_soak'
$env:NIDA_GPU_SOAK_SECONDS = '180'
& $test $soakTest --exact --ignored --nocapture --test-threads=1
if ($LASTEXITCODE -ne 0) { throw 'Short stability check failed; do not start the soak' }
$env:NIDA_GPU_SOAK_SECONDS = '7200'
& $test $soakTest --exact --ignored --nocapture --test-threads=1
```

The harness asserts current GPU residency, zero production picture transfers,
pool bounds, validation-fault absence, progress, paused preload preservation and
bounded RSS while exercising seeks, transition changes and source reloads. It
changes the decoding preference in process only and does not persist it.
