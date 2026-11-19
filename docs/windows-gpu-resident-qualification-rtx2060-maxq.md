# Windows GPU-resident playback qualification — ASUS G14 / RTX 2060 Max-Q

Qualification in progress on 19 November 2026. Initial hardware tests found an
SDR P010 colour mismatch; the production fix has passed release revalidation.
All 30 serial performance measurements pass. The two-hour soak remains in
progress; this report does not yet claim a completed two-hour stability pass.
The user confirmed that no GPU-heavy work was running
and authorized the initial tests and, subject to their results, a two-hour soak.
The experimental preference, beta label and production thresholds are unchanged.

## Revision and machine

- Fork: `https://github.com/pingvis/Nida-Windows`, `main`.
- Base revision: `1e4cf2055e8881ab3d8e0cd2002bedf984fe3256`, verified against the
  remote before building. The initial tests use that unmodified revision.
  Revalidation adds the local SDR P010 fix in `src/internal/video/gpu_video.wgsl`.
  Exact measured binary/shader hashes are recorded below; the measured build is
  the base revision plus this patch, not an unmodified-main qualification.
- ASUS ROG Zephyrus G14 `GA401IV_GA401IV`.
- AMD Ryzen 9 4900HS, 8 cores / 16 logical processors; 16 GiB installed RAM
  (15.42 GiB reported usable by Windows).
- Windows 11 Home x64, version `10.0.26300`, build `26300`.
- NVIDIA GeForce RTX 2060 with Max-Q Design, 6 GiB VRAM; NVIDIA driver `617.42`
  (Windows driver `32.0.16.1742`, dated 30 September 2026).
- Integrated AMD Radeon Graphics, PCI `1002:1636`; Windows driver
  `27.20.11020.14001`, dated 17 June 2021.
- Parsec Virtual Display Adapter, driver `0.45.0.0`.
- AC power and the existing High performance power plan were recorded; no power
  settings or drivers were changed.

Raw evidence, fixtures, scripts and manifests are retained locally in the ignored
`target/gpu-qualification-local/` directory. The shortened Cargo build cache is
`target/gq/`.

## Toolchain and preparation

The existing local tools were reused: Rust/Cargo `1.99.1` for
`x86_64-pc-windows-msvc`, MSVC tools `14.44.35207`, Windows SDK `10.0.26100.0`,
CMake `3.31.6-msvc6`, Python `3.12.14`, libclang `20.1.2`, and FFmpeg
`n9.0.2-36-g6882969ce9-20261027`. The required AEC3 library was built from the
repository's checksum-verified vendored sources with the pinned Meson/Ninja tools.

The repository release profile is used without overrides: optimization level 3,
thin LTO, one codegen unit and unwind panics. Default features are disabled;
`screen-capture,windows-browser` are enabled. Builds use one Cargo job. Hardware
tests run serially with one test thread. The production shader is embedded.

Preparation problems and remedies are retained in the logs:

1. The CPU harness initially exited with `0xc0000135` because Windows could not
   find `avutil-61.dll`. The matching FFmpeg runtime DLLs were placed beside the
   test executable. The documented Cargo harness command subsequently passed.
2. MSVC failed a shaderc/SPIR-V object build with `C1083` because the long build
   directory exceeded the native object-path limit. Moving the qualification
   cache to `target/gq/` resolved the native build. The failed cache and log remain.
3. The cached Swagger UI archive is supplied with a proper `file:///C:/...` URI
   for offline building. Its SHA-256 is recorded separately.
4. Fixture probes caught absent primaries/transfer tags and rotation in the first
   encoding pass. Verified fixtures were produced with explicit bitstream colour
   metadata and an input display-rotation override. Raw and corrected fixtures,
   generation commands, full probes and hashes are retained separately.

## CPU results and hardware capabilities

The documented `cargo check --offline --tests --no-default-features --features
screen-capture,windows-browser -j 1` passes. The small CPU harness passes 14 tests;
its one ignored 4K timing experiment is not a correctness test. These cover the
production WGSL, colour/HDR metadata, seek generation and three-slot pool lifetime,
allocation failure and reuse. Existing vendored wgpu-hal macro warnings remain.

A native DX12 capability probe enumerated H.264, HEVC Main and HEVC Main10 on both
physical GPUs. Neither advertises an AV1 decode profile. The AV1 fixture decodes
successfully in software. The required-residency test fails with FFmpeg's
"D3D12 video decode is not supported on this device" message and no GPU payload.
This is an unsupported-codec result, not successful GPU-resident playback.

The probe also enumerated the Microsoft Basic Render Driver (no video-device
interface) and a second AMD entry with a distinct LUID. These enumeration results
are not claims that every entry is a separate physical GPU or that AMD playback
has been qualified. Both the actual headless and windowed renderers identify the
RTX 2060 Max-Q, PCI `0000:01:00.0`, on DX12. The windowed application also completed
a graceful API shutdown. Existing global settings hashes are unchanged.

A separate probe using the production D3D11VA call
`av_hwdevice_ctx_create(..., AV_HWDEVICE_TYPE_D3D11VA, nullptr, nullptr, 0)`
selected AMD Radeon Graphics, LUID `0:e217`. NVIDIA's DX12 high-performance
enumeration LUID is `0:f857`. This is a hybrid comparison concern: the existing
hardware decoder and renderer-owned decoder may use different physical GPUs.
The continuous Windows GPU counter sampler returned empty per-process sample
arrays for newly-created benchmark processes. It does not establish the decoder
LUID or per-process GPU memory for those timed runs. A fresh snapshot during the
soak confirms NVIDIA 3D and VideoDecode activity on `0:f857`. NVIDIA global decoder
utilization was zero in the existing-hardware timed runs, consistent with the
standalone AMD device probe, but not a substitute for per-run LUID evidence.
Consequently these are production-default hybrid comparisons, not measurements
that isolate transfer removal on an identical decoder adapter. AMD GPU-resident
playback remains unqualified.

## Playback qualification results

The unmodified release test binary SHA-256 is
`4DCE161C0CB7110D1D3397F40B86F2BD33B63294E49A3757E14095628074AD3A`.
It is preserved as `target/gpu-qualification-local/baseline-main/varda-main.exe`,
with matching runtime DLLs and the initial matrix logs.

A three-second H.264 smoke test explicitly required GPU residency. It delivered
180 fresh pictures over 180 render ticks, with zero picture downloads/uploads,
no fallback and peak occupancy of two slots. Compilation was still running, so
this is smoke evidence only and is excluded from performance measurements.

The original 15-group picture matrix passed 13 groups. AV1 failed as unsupported.
SDR HEVC Main10 failed at seek zero with mean difference
`0.8054416232638889` bytes per RGBA channel, exceeding the unchanged `< 0.5` limit.
The separate interlaced software-fallback test passed. Passing progressive groups
also completed their seek, three-slot pool/reuse and ping-pong fallback assertions.

The Main10 failure reproduced exactly. The independently software-decoded CPU
reference and hardware-picture download converted on the CPU are byte-identical.
The GPU output is brighter (mean signed R/G/B differences approximately
1.4454/0.9349/0.8414), localizing the discrepancy to colour conversion rather than
showing a decoder/reference mismatch. The shader's SDR integer-compatibility path
was restricted to NV12. Normalizing P010 codes into that path fixes the diagnostic
comparison: all five original seeks pass, with approximately 0.008–0.013 diagnostic
`packed_mean` error. That diagnostic compares against the downloaded hardware
picture converted on CPU; the independent software-reference assertion also
passes at its unchanged `< 0.5` limit.
Additional full-range, wide-SAR and narrow-SAR P010 fixtures also pass all five
seeks with the diagnostic shader. HDR retains its existing full-precision path.

The small production patch applies that normalization without changing any
acceptance limit, pool bound, beta label or default. Both production release
builds succeeded with the embedded fixed shader. The release test binary SHA-256
is `88D0FBB18253C21E4A0D3A68B96A8782C8B0CE271BC7B43A6968B5D301DDB3D8`;
the app SHA-256 is
`0D431A0A8FA6835CE15878432569777E46F010680CD2399016DFCCD22163168C`.
The production shader SHA-256 is
`D651DB90D51608CE57B498F2BBF827F8D0982925F28379DFE1601438A8AF3B4A`.

The fixed embedded production shader passes all 14 supported original fixture
groups and all three added SDR P010 groups: 85 seek-picture comparisons in total,
with the unchanged `< 0.5` mean-error limit. Each group also passes its native
three-slot backpressure/reuse, zero-production-transfer and ping-pong fallback
assertions. AV1 still fails explicitly as unsupported. The separate interlaced
fallback passes. All 177 regular video regressions pass; six explicit diagnostics
and hardware tests remain ignored in that ordinary group and are run separately
where applicable. The fixed CPU harness and full test-target compile check pass.

The three-minute release playlist test passes using H.264, HDR10 and SDR HEVC
Main10. It delivers 10,154 GPU pictures, performs 11 seeks and one reload, and shows
all three clips. The sampled peak RSS is 368,967,680 bytes (352 MiB). Pause/resume,
preloaded paused pictures, changing transitions, zero production picture
downloads/uploads, the three-slot bound and absence of GPU validation faults are
asserted by the production playlist harness throughout.

The queue initially stopped before its first benchmark because repeated MSVC
environment loading removed Git from PATH. No timed measurement ran in that
attempt. The runner now explicitly adds the existing Git installation to its
process PATH; the failed attempt is preserved in `benchmark-environment-failure/`.
The resumed performance queue completed all 30 measurements and passed its
initial CPU/fresh-frame comparison gate. It then started the two-hour soak.
Unexpected failures stop the queue and preserve their logs.

## Single-clip performance

Five interleaved 60-second release runs per path/resolution, with the order
rotating each round. The H.264 High fixtures contain 120 seconds at 60 fps, so no
loop flush occurs during measurement. Compilation and fixture generation were
finished before these measurements. The expected decoder/delivery path is
asserted throughout; a fallback cannot pass a required-residency measurement.

Medians of five runs. CPU uses **one core = 100%**. Fresh rate counts new pictures;
the renderer reuses its previous picture when none is ready. Tick-service p95
includes offscreen submission and GPU completion, not whole-app UI latency.

| Resolution | Path | CPU, % of one core | Fresh pictures/s | Tick-service p95, ms | Peak sampled RSS, MiB |
| --- | --- | ---: | ---: | ---: | ---: |
| 1080p60 | Software | 56.87 | 59.99 | 8.125 | 313 |
| 1080p60 | Existing hardware + RAM | 61.97 | 58.03 | 8.315 | 243 |
| 1080p60 | GPU-resident | 8.46 | 59.91 | 3.643 | 224 |
| 4K60 | Software | 229.05 | 58.03 | 17.075 | 737 |
| 4K60 | Existing hardware + RAM | 100.01 | 28.28 | 17.235 | 513 |
| 4K60 | GPU-resident | 12.86 | 59.98 | 4.800 | 278 |

Compared with the existing hardware path, GPU-resident median CPU use is lower
by **86.3% at 1080p60** and **87.1% at 4K60**. Median service p95 is lower by
56.2% and 72.2%. Every GPU-resident benchmark has zero production picture
download/upload bytes, no fallback, zero pool-backpressure events and peak
occupancy of two of the three allowed slots. GPU-to-GPU plane copies are
intentional and are not RAM picture transfers.

Frame pacing is not perfectly uniform. GPU-resident fresh rates range from
54.93–59.99/s at 1080p60 and 59.38–59.99/s at 4K60; existing-hardware ranges are
43.06–58.09/s and 22.01–29.18/s. The GPU-resident median is better, but not every
1080p GPU run exceeds every hardware run. This is a pacing/delivery limitation,
not evidence of fallback or unsupported H.264. Do not claim guaranteed continuous
60-fps delivery. GPU-resident late-render-tick counts are 0–1 per run. The raw
delivery `dropped_frames` counter is 61–75 at 1080p and 59–98 at 4K; it includes
discarded GPU payloads from the unrendered one-second warmup. It is not directly
comparable with the CPU path's zero GPU-payload-discard counter.

NVIDIA global telemetry reaches 223 MiB VRAM / 9% decoder utilization during
1080p GPU-resident runs and 321 MiB / 24% at 4K. These are sampled adapter-wide
readings, not per-process allocation peaks. The missing Windows per-run process
samples must not be replaced with zero-valued measurements. A fresh soak snapshot
records 288,976,896 dedicated bytes and 198,455,296 shared bytes on the NVIDIA
process adapter. Driver memory accounting is separate from the production
picture download/upload counters.

The picture and playlist harnesses explicitly assert no GPU validation faults.
The real-time benchmark asserts path, transfer, pool and device-poll success but
does not export a validation-fault count; none is invented here. At the
18:01:53 Europe/Vilnius event-log snapshot, no warning/error/critical events from
registered `Display`, `nvlddmkm` or `amdkmdag` providers matched the benchmark/soak
time window. `amdwddmg` is not registered on this PC, so that provider is an
explicit observability limit.

## Two-hour soak — pending

The uninterrupted release run started at **16:49:14 Europe/Vilnius on 19 November
2026**, with H.264, HDR10 and SDR HEVC Main10, using the fixed embedded production
shader. The committed partial snapshot covers 71 completed minutes: 242,566 GPU
pictures, 283 seeks, 35 reloads and peak sampled RSS of 399,982,592 bytes (381 MiB).
No residency/transfer/pool/validation/progress assertion has failed in that
snapshot. The requested 7,200-second run is **not complete in this report**;
its expected end is approximately 18:49. A final completion record and remaining
memory samples must be collected before reporting a two-hour pass.

## Reproduction and evidence

The [committed evidence and reproduction guide](qualification/rtx2060-maxq-20261119/README.md)
contains portable software-fixture generators, all 20 fixture SHA-256 hashes and
metadata, all 30 raw performance records, correctness outcomes, baseline/fixed
failure logs, native adapter probes, event-log coverage and the partial soak
snapshot. Videos and binaries remain local and are not committed.

On this PC, start in the repository root with its local x64 MSVC/FFmpeg environment:

```powershell
. ./target/gpu-qualification-local/env.ps1
cargo check --offline --tests --no-default-features --features screen-capture,windows-browser -j 1
cargo test --offline --manifest-path tools/gpu-video-checks/Cargo.toml --target-dir target/gpu-qualification-local/cpu-checks -j 1
cargo test --offline --release --no-default-features --features screen-capture,windows-browser --lib --no-run -j 1
cargo build --offline --release --no-default-features --features screen-capture,windows-browser -j 1
```

`env.ps1` loads the existing MSVC and SDK setup, removes its release-profile and
software-decode overrides, selects `target/gq/`, supplies the native AEC library
and cached Swagger archive, and clears qualification/shader override variables.
It changes only the child process environment.

The evidence directory contains `build-manifest.json`, `source-hashes.json`,
`runtime-hashes.json`, hardware inventory JSON, the native adapter probe source,
binary and hashes, and `fixture-matrix.json`. The verified fixtures' individual
SHA-256 values and complete probes are in `fixtures-verified/hashes.jsonl` and
the adjacent `.probe.json` files. The committed `fixtures.json` also includes
the three added P010 regressions. Full commands for strict hardware tests,
interleaved measurements and playlist smoke/soak are in the reproduction guide.

## Relationship to the RTX 3060 Ti report

The reference report uses an RTX 3060 Ti with 8 GiB VRAM, driver 621.24 and a
Ryzen 9 5950X with 32 logical processors. This is a 6 GiB RTX 2060 Max-Q hybrid
laptop with a different CPU and driver. The reference report does not identify
an exact FFmpeg binary build to match against this SDK. Fixtures were generated
locally; the reference machine's original fixture binaries are not present here.
Cross-machine differences therefore cannot be attributed solely to the GPU or
solely to picture-transfer elimination.

Unlike the reference machine, neither GPU here advertises AV1 hardware decoding.
The GPU-resident medians compare as follows:

| Resolution | Machine | CPU, % of one core | Fresh pictures/s | Tick-service p95, ms |
| --- | --- | ---: | ---: | ---: |
| 1080p60 | Reference RTX 3060 Ti | 1.95 | 52.45 | 0.781 |
| 1080p60 | This RTX 2060 Max-Q hybrid | 8.46 | 59.91 | 3.643 |
| 4K60 | Reference RTX 3060 Ti | 3.15 | 53.33 | 1.601 |
| 4K60 | This RTX 2060 Max-Q hybrid | 12.86 | 59.98 | 4.800 |

This machine has higher CPU/service cost but higher delivered fresh-frame rate
in these measurements. Both reports show pacing variability; neither establishes
uniform 60-fps delivery across every run. The reference report has a completed
two-hour soak; this report currently has only a passing three-minute test and
an in-progress two-hour run. Its concurrent 4K stress matrix was not repeated
here, so no multi-stream 4K throughput qualification is claimed.

This qualification exposed an SDR P010 conversion mismatch not reported in the
reference fixture matrix. The evidence does not establish a hardware-specific
decoder or interoperability bug: both CPU references agree, and the normalized
SDR shader path passes independent-reference checks. Recheck this fix and the
full-range/SAR P010 regressions on the original machine and other FFmpeg builds.
AMD and Intel GPU-resident playback still require tests with a renderer actually
using those adapters. Keep the feature experimental and off by default.
