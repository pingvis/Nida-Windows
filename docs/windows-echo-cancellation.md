# Native Windows echo cancellation

Windows x64 uses the same WebRTC AEC3 algorithm and C wrapper as upstream
Nida's Apple-silicon build. The microphone is cleaned for the bus that asks
for echo removal; other buses retain the microphone as captured. Select the
bus feeding the room's speakers as the call bus's echo-removal reference.
This uses Nida's mixer reference, not a Windows system-wide audio effect.

## Building

Run from x64 Visual Studio Developer PowerShell with Python 3.12 installed:

```powershell
./scripts/ci/setup-windows-aec.ps1
cargo build --locked --no-default-features --features screen-capture,windows-browser
```

The setup script creates a local Python venv with pinned Meson 1.9.1 and
Ninja 1.13.0. It verifies the three vendored source archives' SHA256 values,
then compiles WebRTC 2.1, Abseil 20240722.0 and the existing `kk_aec.cc` wrapper
with MSVC, C++20 and the dynamic CRT (`/MD`). C++20 is needed for designated
initializers in this source version on MSVC. Source downloads are disabled.
The Windows static build omits WebRTC's DLL-export define.

Output defaults to `target/windows-aec/nida_aec.lib`. A different
`-ToolsDirectory` is exported as `NIDA_AEC_LIB_DIR`; Cargo accepts that
directory or the default. The build fails with setup instructions if the
library is missing, rather than silently shipping without cancellation.
No extra AEC DLL is required at runtime. Windows ARM64 is not enabled.

The normal Windows CI dependency setup calls this script too. Its cache
checks source, wrapper, recipe and compiler hashes and the resulting library
hash. Changed inputs use a new build folder. The generated manifest and
WebRTC/Abseil notices are copied into the Windows package's licenses folder.

## Mixer integration

The Rust FFI and echo tests use the build capability `nida_aec`, enabled for
Apple silicon and Windows x64. Each microphone has a canceller per referenced
room, running at 48 kHz in 480-sample (10 ms) blocks. Device conversion and
clock-drift handling remain in the mixer.

The fork's input EQ is retained after cleaning, on a separate processor with
the same settings. A room's echo reference also gets the bus EQ. Filter graphs
are prepared outside the mixing lock. Normal and low-latency bus behavior
retains upstream's limitation: echo removal applies to the normal mixer path,
not a Live bus's microphone callback.

## Qualification

Native Windows qualification on 9 November 2026 uses `--locked --offline
--no-default-features --features screen-capture,windows-browser`:

- **270 audio unit tests pass**, two exploratory/timing tests ignored. This
  includes the newly enabled native AEC3 tests and the fork's EQ regressions.
  The synthetic 15-second room test requires more than **30 dB** echo reduction;
  a voice with no speaker playback stays within **1.5 dB** of its original level.
  Mixer tests check call-only cleaning, microphone exclusion from the reference,
  fades, delays, stalled blocks, retained learning and input/bus EQ.
- **153 integration tests pass**: 14 audio, 51 command and 88 UI tests. One
  NVIDIA hardware test is ignored. GPU tests require an adapter explicitly.
- Production library/application Clippy passes with warnings denied. Existing
  repository-wide test lint and formatting debt remains; all-target Clippy
  still fails on inherited empty assertions. Touched Rust files are formatted.
- The native source build and verified cache hit both succeed. Package notice
  generation includes all AEC3 notices, patent grant and build provenance.

An initial audio integration run reported a brief nonzero off-air sample in
`a_clip_off_air_still_shows_what_it_plays`. Three isolated reruns and the full
serial integration rerun pass. This remains an intermittent observation;
no cause or unrelated playback fix is claimed by this port.

### Debug mixer timing

AMD Ryzen 9 5950X; Rust debug build with optimized native AEC3; 600 measured
blocks after 100 warm-up blocks, two buses, compressors off. Each block covers
10 ms. Echo-on fixtures add one speaker feed and a canceller per microphone.
These are whole mixer costs, including that reference path, not isolated
algorithm timings. The fixture uses synthetic tones and no device I/O.

| Microphones | Echo off, p50 ms | Echo on, p50 ms | Echo + input/bus EQ, p50 ms | Echo + EQ, p99 ms | Echo + EQ blocks over 10 ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1 | 0.791 | 1.219 | 1.499 | 1.544 | 0 / 600 |
| 8 | 1.992 | 3.981 | 4.923 | 5.081 | 0 / 600 |
| 16 | 3.344 | 7.101 | 8.806 | 9.105 | 0 / 600 |
| 32 | 6.018 | 13.410 | 16.626 | 17.156 | 600 / 600 |

The debug build has headroom in the eight-microphone fixture, little at sixteen
with EQ, and cannot sustain thirty-two. These figures do not establish release
build capacity or scheduling headroom during a busy video show.

To reproduce, run the existing ignored `mixer_block_budget_probe` alone with
`NIDA_AUDIO_PROBE_BLOCKS=600`. Compare `NIDA_AUDIO_PROBE_ECHO` unset versus `1`;
set `NIDA_AUDIO_PROBE_EQ=sources-and-buses` for the EQ case. The native setup
uses `target/windows-aec`; logs and raw probe output are under
`target/windows-aec-*` and `target/aec-windows/`, outside Git.

Synthetic room tests do not establish performance in a physical room. Device
buffering, acoustic tails, speaker distortion and external audio processing
can change cancellation quality; a venue still needs a microphone/speaker
sound check. Audio played outside the chosen Nida bus is not in its reference.
