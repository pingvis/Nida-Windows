# Laptop GPU recording validation

Test date: 28 October 2026. Work was developed on
`codex/investigate-dual-recording-performance` and rebased onto fork main
`13a5b92` (Nida 0.9.4). Publishing this validated change to the fork's main uses
`[skip ci]` to suppress push-triggered GitHub Actions, as requested by the user.

## Machine and build

- Ryzen 9 4900HS, 16 logical processors, NVIDIA RTX 2060 Max-Q plus AMD integrated
  graphics. The internal screen is 120 Hz. AC power was connected.
- Native Windows MSVC build using Rust 1.99.1, Visual Studio 2022 Build Tools
  14.44, Windows SDK 10.0.26100, FFmpeg 9 shared development SDK and libclang 20.
- App features: `--no-default-features --features screen-capture,windows-browser`.
- Diagnostic application builds use release optimization, with LTO disabled,
  16 codegen units and debug information disabled. This is not the shipping
  release profile. Native tests use package-specific optimization level 1 with
  optimized dependencies.
- A private runtime was assembled under
  `target/performance-investigation/runtime/`. The installed Nida was not replaced.
- The final optimized application build succeeded and reports version 0.9.4.
  The tested source is `5bda570`; binary SHA-256 and base revision are recorded
  in the final run's `provenance.json`. Later changes to this report are documentation only.

## Workload and measurement

The desktop "Around The World 4K" MP4 is H.264, **1920×1080 at 30000/1001 fps**.
It was loaded into a separate test workspace with a visible control window,
1920×1080 rendering and a 50 fps target. Software decoding was forced to isolate
the encoder comparison. Camera, screen-capture discovery, NDI, OSC, HTML and
PowerPoint were disabled. The capture card was unavailable.

Each recording case used two simultaneous outputs showing the same MP4, with
five seconds of warmup and thirty one-second samples. Each output included the
default master audio bus. CPU figures include Nida and its immediate FFmpeg
children, normalized across all sixteen logical processors. Render FPS is the
change in engine frame count divided by sample timestamps; averaging the
displayed FPS estimates gives misleading results during pacing fluctuations.
Delivery figures use counter differences between the first and last sample,
excluding start/stop behavior. Padding means repeated frames used to retain the
50 fps file clock; encoder drops are a separate counter.
The padding counter does not count every repeated image from the 29.97 fps
source itself. A 50 fps render can still repeat source images because their
timestamps do not align with the show clock.

This is a short, fixed-order comparison, not a thermal soak or a quality/size
equivalence test. NVENC CQ 20 and software CRF are different quality controls.

## Final 0.9.4 application results

Two simultaneous recordings, measured within the same application run:

| Two recording outputs | App + encoder CPU | Render FPS | Encoder drops per output during samples | Padded frames per output |
|---|---:|---:|---:|---:|
| H.264 software | 20.07% | 46.70 | 0 / 0 | 106 / 106 |
| H.264 NVENC | 8.90% | 42.34 | 0 / 0 | 220 / 221 |
| HEVC NVENC | 8.53% | 43.73 | 0 / 0 | 184 / 184 |
| HEVC Main10 NVENC | 29.63% | 48.50 | 0 / 0 | 41 / 42 |
| HEVC software | 65.89% | 49.62 | 0 / 0 | 19 / 19 |

The no-recording baseline rendered an estimated 41.77 fps. NVENC substantially
reduced CPU load, but did **not** remove the render/pacing limitation or improve
render FPS in this comparison. Main10 still uses CPU conversion from packed
RGB10 to P010; it consumes more CPU than the NV12 eight-bit path.

All ten files had 1920×1080, 50/1 video, AAC audio and the expected encoder tags.
Both Main10 files retained the Main 10 profile and `yuv420p10le` pixel format.
Full FFmpeg decode of every stream completed without errors. Representative
H.264 and Main10 frames were also visually inspected. Startup counters included
two drops on one H.264 NVENC output and four on one Main10 output, outside the
measured interval; this is not a claim of zero drops across the entire take.

The displayed FPS can conceal uneven pacing: H.264 NVENC's sampled displayed
average was 49.64, while frame-count/time deltas gave about 42.34. The current
`src/app/render.rs` averages instantaneous `1 / dt` values; this is not the
same as frames divided by elapsed time when frame intervals vary. That metric
and the pacing behavior need separate follow-up; they were not changed here.

Artifacts: `target/performance-investigation/full-app-20261028-113225/`, including
`measured-summary.json`, full sample data, encoder logs, `file-verification.json`
and binary provenance. An earlier pre-0.9.4 run is retained at
`full-app-20261028-105741/`: H.264 CPU fell from 20.30% to 8.77%, and HEVC CPU
from 83.71% to 8.91%. That earlier software HEVC run also dropped 180/499 frames
during the sample interval; this overload was **not reproduced** in the final
run. These are short comparisons under different revisions/runs, not a
controlled before/after test of the 0.9.4 update.

## Additional application and hardware checks

The final native build also passed these checks:

- 26 recording-related native unit/UI tests passed, with one hardware test
  separately invoked; 11 NVENC-related tests passed.
- The explicit hardware worker test passed: simultaneous H.264 and HEVC,
  repeated stop/start, unique take filenames, and idempotent stop.
- Five audio/recording integration tests passed, including the explicit NVENC
  H.264 + HEVC test with 50 fps output. Flash/beep timing uses the existing
  less-than-50-ms synchronization assertion.
- Real app smoke tests passed repeated H.264 takes, HEVC Main10 through Nida's
  RGB10-to-P010 path, active output state, encoder persistence across save/load,
  and orderly shutdown. Lifecycle smoke tests used 320×180 at 50 fps; the
  separate final benchmark above exercised Main10 at 1080p50 with two outputs.
- A deliberate NVENC session exhaustion test made a recording fail and stop,
  while the API stayed healthy. After the test sessions were freed, restarting
  that same output produced an active GPU recording. No mid-file CPU switch
  occurred. The recovered take contained 154 H.264 NVENC frames and AAC audio.
  Artifacts: `target/performance-investigation/api-smoke-20261028-113144/`.

The isolated recording-plan harness and three simultaneous direct-FFmpeg
1080p50 sessions (H.264, HEVC Main, HEVC Main10) also passed earlier. These are
supplementary checks, not replacements for the native app tests.

## Native checks after the 0.9.4 rebase

| Check | Result |
|---|---|
| Recording unit/UI filter | 26 passed, one hardware test invoked separately |
| NVENC unit filter | 11 passed, one hardware test invoked separately |
| Simultaneous hardware workers, stop/restart and unique files | Passed |
| Persistence module | 54 passed |
| Recording/audio integration, including explicit NVENC test | 5 passed |
| API documentation generation and verification without update mode | 3 passed |

Filters overlap; the rows are not a count of distinct tests. The recording/UI,
NVENC worker and persistence checks used the completed native test executable
`varda-02d929ed06748fcd.exe`, which was built after the most recent Rust source
change. The audio and API integration suites used their own native executables.
Detailed results are in `target/performance-investigation/final-*.log`.

With the repository's Windows toolchain and FFmpeg setup on PATH, these are the
relevant test commands (the hardware tests are intentionally opt-in):

```powershell
$features = 'screen-capture,windows-browser'
cargo test --release --locked --no-default-features --features $features --lib recording -- --test-threads=1
cargo test --release --locked --no-default-features --features $features --lib nvenc -- --test-threads=1
cargo test --release --locked --no-default-features --features $features --lib nvenc_recording_workers_stop_and_restart_with_unique_files -- --ignored --test-threads=1
cargo test --release --locked --no-default-features --features $features --lib internal::persistence:: -- --test-threads=1
cargo test --release --locked --no-default-features --features $features --test audio_mix recording -- --include-ignored --test-threads=1
cargo test --release --locked --no-default-features --features $features --test api_docs
```

The local run applies the diagnostic profile overrides described above. The
explicit hardware/audio tests require usable NVIDIA hardware, FFmpeg/FFprobe and
a working GPU renderer; unavailable hardware is not treated as a passing result.

## Remaining checks before show use

- Repeat the real **moving capture card + MP4**, two-output, 1080p50 workload
  once the card is available. Two recordings of one MP4 do not validate capture
  acquisition, USB bandwidth, or a second independent decode/input path.
- Run a longer recording soak and verify the final installer/release profile.
- Resolve or separately accept existing strict Clippy warnings on current main.
  Ordinary Clippy completes; `-D warnings` fails in existing Windows presentation,
  clicker and test code. The strict attempt logs are retained under
  `target/performance-investigation/final-clippy-strict-*.log`.
- Investigate the remaining render/pacing issue separately. GPU encoding does
  not repair GPU decode stutter or turn a 29.97 fps input into evenly spaced
  50 fps motion. Correct the displayed FPS calculation so it does not mask
  irregular intervals.
