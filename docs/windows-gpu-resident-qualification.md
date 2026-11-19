# Windows GPU-resident playback qualification

Qualification started on 19 November 2026 after the user's render and game had
stopped. Results apply to this machine; they do not establish AMD, Intel or hybrid
adapter compatibility. The preference remains experimental and off by default.

## Machine and method

- NVIDIA GeForce RTX 3060 Ti, 8 GiB VRAM; driver 621.24.
- AMD Ryzen 9 5950X, 32 logical processors.
- Windows, native DX12 renderer, FFmpeg 9 D3D12VA.
- Release profile with the repository's existing optimizations; no profile overrides.
- Hardware tests run serially and explicitly require the expected decode path.
  A software fallback fails a GPU-resident test.
- GPU picture readback is confined to correctness tests. Production delivery
  counters separately assert zero uncompressed picture downloads and uploads.

Local logs, manifests, fixture hashes and binaries live under the ignored
`target/gpu-qualification/` directory. Reproduction commands are documented in
[the implementation notes](windows-gpu-resident-video.md).

## Correctness and regressions

Fourteen CPU-only tests pass, including production WGSL validation, slot lifetime
and allocation failure handling, seek-generation races, colour metadata and HDR
tone mapping. The ignored CPU 4K timing check is not a correctness test.

The final release binary passed 177 video regression tests and all fifteen
picture fixture groups. The latter cover five seek positions each, the unchanged
mean error limit of 0.5 byte per RGBA channel, three-slot backpressure/reuse and
ping-pong fallback. Coverage includes H.264, HEVC and AV1; NV12 and P010; limited,
full and untagged range/matrix metadata; BT.601; HDR10 and HLG; 1080p60, 4K30 and
4K60; a verified 90-degree rotation; and both wide and narrow sample-aspect ratios.
The software reference is decoded independently, then losslessly repacked to the
same YUV packing before the existing CPU colour converter runs.

Interlaced material successfully reopens through the established software field
processing and delivers its first picture. Paused first-picture preservation
also has a regression test. These checks and the final picture matrix use the
embedded production shader, with no test shader override.

A three-minute development-build run through the actual playlist, deck, render
and submission code delivered 9,055 GPU frames, performed 11 seeks and one source
reload, and showed all three clips. RSS samples were approximately 507–516 MiB.
There were no validation faults, unexpected fallbacks or production picture
transfers through RAM. This is preliminary stability evidence, not a performance
measurement.

## Single-clip performance

The offscreen playback benchmark compares software decode, D3D11VA decode with
CPU conversion and picture transfers, and D3D12VA GPU-resident playback. Each path
runs five times for 60 seconds, rotating execution order. The continuous H.264
fixtures contain 120 seconds, avoiding loop flushes during measurement.

CPU percentages below use one core as 100%, not the whole 32-thread machine.
Fresh-frame rate counts newly delivered pictures; the renderer continues ticking
at 60 Hz and reuses its previous picture when none is ready. Tick-service p95
includes submission and GPU completion in this offscreen harness. It is not a
whole-app UI latency measurement.

Medians of five 60-second measurements per cell:

| 1080p60 path | CPU, % of one core | Fresh pictures/s | Tick-service p95, ms |
| --- | ---: | ---: | ---: |
| Software | 35.21 | 52.27 | 2.648 |
| Existing hardware + RAM transfer | 50.23 | 48.97 | 2.954 |
| GPU-resident | 1.95 | 52.45 | 0.781 |

| 4K60 path | CPU, % of one core | Fresh pictures/s | Tick-service p95, ms |
| --- | ---: | ---: | ---: |
| Software | 150.78 | 38.57 | 8.275 |
| Existing hardware + RAM transfer | 107.24 | 27.07 | 8.258 |
| GPU-resident | 3.15 | 53.33 | 1.601 |

Compared with the existing hardware path, GPU residency reduced median CPU use
by 96.1% at 1080p60 and 97.1% at 4K60. Median tick-service p95 fell by 73.6% and
80.6%, respectively. Every GPU-resident run delivered more fresh pictures than
every existing-hardware run at the same resolution. All required-path assertions
passed; production frame downloads and uploads remained zero, with at most two
of the three slots occupied and no pool-backpressure events.

GPU-resident fresh-frame rates ranged from 49.80 to 59.07/s; the existing hardware
path ranged from 47.73 to 49.75/s at 1080p60. At 4K60 those ranges were 52.72–53.53/s
and 26.48–27.12/s. Thus CPU savings do not imply perfectly uniform 60-fps delivery.
Frame pacing remains a separate area for investigation. GPU-resident peak sampled
RSS was 242 MiB at 1080p60 and 305 MiB at 4K60, compared with 262 and 442 MiB for
the existing hardware path.

## Concurrent 4K60 clips

These are single 60-second stress measurements, not five-run medians:

| Path | Clips | CPU, % of one core | Fresh pictures/s per clip | Tick-service p95, ms |
| --- | ---: | ---: | --- | ---: |
| Software | 2 | 600.89 | 41.29, 41.24 | 25.879 |
| Existing hardware + RAM transfer | 2 | 213.56 | 23.65, 23.62 | 15.697 |
| GPU-resident | 2 | 4.24 | 48.00, 51.78 | 2.777 |
| GPU-resident | 3 | 10.39 | 47.32, 47.82, 51.60 | 3.140 |
| Existing hardware + RAM transfer | 4 | 406.30 | 7.62, 7.42, 7.47, 7.47 | 14.448 |
| GPU-resident, repeat 1 | 4 | 6.64 | 8.00, 8.02, 8.05, 8.02 | 2.119 |
| GPU-resident, repeat 2 | 4 | 5.05 | 7.95, 8.00, 7.97, 7.98 | 2.093 |

Four concurrent 4K60 streams expose a sharp throughput cliff on both hardware
paths. This workload is **not performance-qualified**. During the three-stream
GPU run, NVIDIA telemetry reached 100% decoder utilization with about 1.6 GiB
VRAM in use. This supports decoder/driver contention as an explanation, but does
not establish its exact cause. Low CPU use does not remove hardware decode limits.
No GPU-resident stress run fell back, transferred uncompressed pictures through
RAM or exhausted the three-slot pool.

## Two-hour release soak

The initial run was stopped after 23 minutes to free the GPU for a Premiere
export. Its log remains separate. After that export, a fresh uninterrupted run
passed from 03:09:48 to 05:09:50 Europe/Vilnius on 19 November 2026.

The release playlist test ran for 7,200 seconds through H.264, HDR10 and AV1
1080p60 clips using the embedded production shader. It delivered 365,631 GPU
frames, performed 479 seeks and 59 source reloads, and showed all three clips.
Pause/resume, preloaded paused pictures and changing transition types remained
under test throughout. There were no GPU validation faults, unexpected fallbacks,
picture downloads or uploads, pool-limit violations or progress-watchdog failures.

The 120 one-minute RSS samples ranged from 347 to 415 MiB and ended at 414 MiB.
The final half-hour ranged from 383 to 415 MiB. This stayed within the bounded
memory assertion; it does not establish zero memory leakage.

The run used revision `0ce0cf03eee4eec1d60f5ef4924cf3a4116f5b3b` and test binary
SHA-256 `54F9ED028A095087B2A1FEB8D7E768C70CB30B317B992FA4708E30DF1EA884FF`.
Its log and run metadata are
`target/gpu-qualification/playlist-two-hour-release-20261119T0109Z.log` and the
adjacent `.run.json`. The release app executable also built successfully.

## SDR P010 fix recheck on RTX 3060 Ti

After the G14 qualification found and fixed the SDR P010 conversion mismatch,
the shader from revision `b38c1e4257ded683915bfdad06fc37a64cc6abc1` was rechecked
on this RTX 3060 Ti. All 18 fixture groups passed five independent software-reference
seek comparisons each (90 total), including the original 15 groups and the new
full-range, wide-SAR and narrow-SAR P010 fixtures. The existing error threshold,
three-slot backpressure/reuse and ping-pong fallback assertions were unchanged.
Interlaced software fallback and all 14 CPU checks also passed.

The requested short playlist soak passed in 180 seconds: 9,106 GPU pictures,
11 seeks, one source reload and all six clips shown. The playlist mixed H.264,
SDR HEVC Main10, HDR10 and the three P010 regressions. No GPU validation faults,
unexpected fallbacks, production picture transfers, pool-bound violations or
progress stalls occurred. Sampled RSS peaked at 365 MiB and ended at 333 MiB.

This focused recheck used the existing optimized test binary recorded above with
the explicit test-only `NIDA_TEST_GPU_VIDEO_SHADER` override pointing to the exact
updated source. Shader SHA-256:
`D651DB90D51608CE57B498F2BBF827F8D0982925F28379DFE1601438A8AF3B4A`.
It verifies the fixed shader on this adapter; it is not a new embedded-shader
production build or another two-hour soak. The earlier two-hour result applies
to the pre-fix embedded shader. Logs, fixture hashes, runner and manifest are
under `target/gpu-qualification/p010-recheck-3060ti/`.

## Remaining qualification

Wider vendor and hybrid-device tests remain necessary
before removing the Windows beta label or changing the default.

Production-library Clippy passes with warnings denied. Full test-target Clippy
has unrelated existing lint failures, including assertion-style warnings in the
audio and other test modules. The all-target check stops on the existing
`assert_is_empty` lint in `tests/persistence_integration.rs:2287`. The repository-wide
format check also reports existing formatting differences across unrelated files;
the new GPU source modules are clean. These were not bulk-rewritten for this
change. `git diff --check` passes. Saved performance preferences retain their
pre-qualification hash.
