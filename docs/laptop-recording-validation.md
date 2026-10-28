# Laptop GPU recording validation

Test date: 28 October 2026. Work stays on
`codex/investigate-dual-recording-performance`; no pull request, push, or merge
to main was performed by this recording task. The branch was rebased onto fork
main `13a5b92` (Nida 0.9.4).

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

## Initial application results

The first native build, before rebasing to 0.9.4, produced these results:

| Two recording outputs | App + encoder CPU | Render FPS | Encoder drops per output during samples | Padded frames per output |
|---|---:|---:|---:|---:|
| H.264 software | 20.30% | 41.98 | 0 / 0 | 235 / 235 |
| H.264 NVENC | 8.77% | 41.68 | 0 / 0 | 239 / 239 |
| HEVC NVENC | 8.91% | 44.04 | 0 / 0 | 174 / 174 |
| HEVC software | 83.71% | 47.85 | 180 / 499 | 248 / 568 |

The no-recording baseline rendered 41.69 fps. NVENC substantially reduced CPU
load and avoided the HEVC software encoder backlog. It did **not** remove the
remaining render/pacing limitation. Faster rendering alone is not proof that
recording kept up: software HEVC rendered faster in this run while losing many
more frames downstream.

All eight files had 50/1 video, AAC audio and the expected encoder tags. Full
FFmpeg decode of every stream completed without errors. H.264 NVENC had three
startup drops per output, outside the measured interval; this is not a claim
of zero drops across the entire take.

Artifacts: `target/performance-investigation/full-app-20261028-105741/`, including
`measured-summary.json`, full sample data, encoder logs and `file-verification.json`.

## Additional application and hardware checks

On the initial native build:

- 26 recording-related native unit/UI tests passed, with one hardware test
  separately invoked; 11 NVENC-related tests passed.
- The explicit hardware worker test passed: simultaneous H.264 and HEVC,
  repeated stop/start, unique take filenames, and idempotent stop.
- Five audio/recording integration tests passed, including the explicit NVENC
  H.264 + HEVC test with 50 fps output. Flash/beep timing uses the existing
  less-than-50-ms synchronization assertion.
- Real app smoke tests passed repeated H.264 takes, HEVC Main10 through Nida's
  RGB10-to-P010 path, active output state, encoder persistence across save/load,
  and orderly shutdown. These lifecycle/Main10 smoke tests used 320×180 at 50 fps;
  they do not establish end-to-end 1080p50 Main10 performance.
- A deliberate NVENC session exhaustion test made a recording fail and stop,
  while the API stayed healthy. After the test sessions were freed, restarting
  that same output produced an active GPU recording. No mid-file CPU switch
  occurred. Artifacts: `target/performance-investigation/api-smoke-20261028-015102/`.

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
  50 fps motion.
