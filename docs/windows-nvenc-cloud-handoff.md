# Windows NVENC optimization handoff — paused 22 October 2026

Work stopped at the user's request, before any production optimization was
applied. This is a work-in-progress checkpoint, not a validated implementation.

## Goal and starting point

Improve quality, efficiency and sustained-stream reliability of the Windows
NVENC backend. Keep changes in `pingvis/Nida-Windows`; upstream submission is
out of scope. Software x264 live bitrate control is not the current priority.

Fork main includes [PR #6](https://github.com/pingvis/Nida-Windows/pull/6),
merge commit `f6311e67465aaf8d86c9c265d4afa6a6f458c214`.
Resume branch: `codex/nvenc-quality-reliability`.

The production encoder still uses **p4 / low latency / CBR / zero latency /
no B-frames / one-second VBV / two-second GOP by default**. It uploads
CPU-resident NV12, signals limited-range BT.709 and supports manual live bitrate
changes. No zero-copy, automatic adaptation, HEVC or live rate-mode switching.
The existing worker, audio/mux and destination fan-out stay in use.

See [the earlier investigation](windows-live-bitrate.md) for the merged
implementation and previous test results. Those results predate this WIP.

## Measurements completed in this session

Local hardware: RTX 3060 Ti, driver 616.92; matching FFmpeg 9 SDK. Encoded one
deterministic six-second FFmpeg `testsrc2` clip at 2560x1440/60, from a lossless
FFV1 reference. All candidates used the same low-latency, no-B-frame CBR
settings as production. Compared decoded output with the reference using
FFmpeg SSIM. This is synthetic material, not user footage or YouTube output.

| Encoder options | 8 Mbps SSIM (All) | 32 Mbps SSIM (All) | 8 / 32 Mbps encode wall seconds |
| --- | --- | --- | --- |
| p4 (current) | 0.975735 | 0.985769 | 1.625 / 1.618 |
| p4 + quarter-resolution two-pass + spatial AQ | 0.977136 | 0.985828 | 2.061 / 2.099 |
| p5 alone | 0.975130 | 0.985777 | 2.832 / 2.838 |
| p5 + quarter-resolution two-pass | 0.976241 | 0.985745 | 2.376 / 2.410 |
| p5 + quarter-resolution two-pass + spatial AQ | 0.977888 | 0.985856 | 2.374 / 2.418 |

Interpretation: switching to p5 alone is not justified by this clip. Two-pass
plus spatial AQ looks promising, especially at constrained bitrate. At 32 Mbps
the objective differences are small. No default was changed. These wall times
include demux/decode, upload, encode, mux, disk I/O and startup; they are **not**
encoder-only costs, render headroom, CPU utilization or Criterion measurements.
They came from one final comparison run; an earlier run showed the same SSIM
values for the original four candidates. Do not claim universal visual gains.

Raw results/logs and fixtures are local-only under `target/nvenc-quality-final`
and `.windows-tools/nvenc-quality-*`; they are deliberately not committed.
`tools/stream-bitrate-probe/quality.ps1` reproduces the comparison and writes
`results.json`, candidate videos and logs beneath its output directory.

Primary references checked:

- [NVIDIA programming guide](https://docs.nvidia.com/video-technologies/video-codec-sdk/13.1/nvenc-video-encoder-api-prog-guide/index.html), multi-pass and AQ sections. Two-pass costs throughput; spatial AQ redistributes bits and may reduce PSNR even when perceptual quality improves. Temporal AQ can vary per-frame sizes and add CUDA work; it was not enabled.
- [FFmpeg 9 NVENC H.264 options](https://github.com/FFmpeg/FFmpeg/blob/n9.0/libavcodec/nvenc_h264.c), exact preset, `multipass=qres`, `spatial-aq` names.

## Unfinished checkpoint changes

No production source files have been modified in this branch.

- `quality.ps1`: completed the comparison above successfully. Creates local
  synthetic fixtures; never contacts a streaming service.
- `pipeline.rs`: new Criterion harness imports the production NVENC encoder
  and FLV writer directly. Packet benchmarks use 4/16/64 KiB payloads; actual
  1440p encoder submission is opt-in with `NIDA_BENCH_NVENC=1`. Pre-generated
  moving texture avoids counting generation in the encode iteration. It does
  not measure the whole app or claim real-content quality.
- Standalone probe `Cargo.toml` / `Cargo.lock`: added Criterion 0.5. Root app
  Cargo.toml, Cargo.lock and dependencies are unchanged.
- `rust.rs`: added an ignored native colour-bar test, including an unaligned
  width to exercise padded luma/chroma strides. Encodes and decodes limited-range
  BT.709 bars, samples Y/Cb/Cr centres within three code values. **Not run yet.**
- `tests/streaming.rs`: added an ignored five-minute 1440p60 local RTMP soak
  using the existing round-trip helper (8 → 2 → 32 Mbps, 100 seconds each).
  It inherits audio, timestamps, rates, queue-drop, reconnect and stop/reopen
  checks. Uses synthetic moving texture and generated PCM. **Not run yet.**

Criterion baseline was interrupted at the user's stop request during the
first packet benchmark. **No completed baseline or comparison is claimed.**
Its first compile failed because the local benchmark `VideoCodec` enum lacked
Debug; that was fixed. The rerun compiled, but warned about `use super::*` in
the imported NVENC module's test section when built as a benchmark. Resolve
that properly before claiming Clippy clean; do not blanket-suppress warnings.
New WIP files have not had formatting/Clippy/full tests run.

## Next work

1. Review this checkpoint and finish formatting/testing the harness. Use a
   separate follow-up commit; preserve the distinction from merged PR #6.
2. Capture a completed Criterion baseline **before** changing production. A
   concrete optimization candidate is `FlvWriter::tag`: it currently builds a
   temporary body buffer, then copies the payload into the final output.
   Writing the tag header and payload straight into the existing output buffer
   could remove one allocation and payload copy. Preserve exact AVC/HEVC
   bytes, configuration updates and timestamps; run the existing FLV tests.
3. Compare the same Criterion cases after that change. No estimated speedup.
4. Evaluate quality settings using more representative moving visuals and
   static/text/gradient material. Measure the cost on an NVIDIA Windows host
   before promoting a preset. Preserve no-reordering and live-rate behavior;
   do not add B-frames/lookahead to the current FLV timing contract casually.
5. Run the new colour check and five-minute RTMP soak on the hardware host,
   then appropriate library/UI/schema/streaming checks. Investigate failures;
   do not weaken assertions or silently skip required hardware tests.

## Commands and cloud limits

On the original Windows workspace, `.windows-tools/build-env.ps1` selects the
cached MSVC/Rust/FFmpeg SDK environment. That ignored local tooling and the
local GPU **will not be present in a fresh cloud checkout**. Install matching
development dependencies through the cloud environment's supported workflow.
Read applicable AGENTS.md, CONTRIBUTING.md and cloud runtime instructions.

```powershell
. ./.windows-tools/build-env.ps1
cargo test --locked --offline --manifest-path tools/stream-bitrate-probe/Cargo.toml --target-dir target/nvenc-probe
cargo test --locked --offline --manifest-path tools/stream-bitrate-probe/Cargo.toml --target-dir target/nvenc-probe -- --include-ignored --nocapture --test-threads=1
$env:NIDA_BENCH_NVENC = '1'
cargo bench --locked --offline --manifest-path tools/stream-bitrate-probe/Cargo.toml --target-dir target/nvenc-probe --bench pipeline -- --save-baseline before
# Apply changes only after capturing the baseline, then:
cargo bench --locked --offline --manifest-path tools/stream-bitrate-probe/Cargo.toml --target-dir target/nvenc-probe --bench pipeline -- --baseline before
cargo test --locked --offline --no-default-features --features screen-capture --test streaming windows_nvenc_1440p_five_minute_soak -- --ignored --nocapture --test-threads=1
```

In a cloud instance without NVIDIA access, packet benchmarks and pure FLV tests
can still be useful with FFmpeg development libraries installed. Do not set
`NIDA_BENCH_NVENC` or claim native encode, driver qualification, Windows build,
live bitrate, audio-sync or sustained-GPU validation. Prepare the changes and
list exact hardware checks left for the Windows machine. No user stream keys,
external broadcasts or public YouTube tests are needed for this phase.

Known earlier limits: ordinary streaming suite had two unchanged software
reconnect failures; whole-repository formatting/all-target Clippy had existing
failures in other tests. Earlier full core Windows library and UI suites passed.
Report new results separately and do not use historical passes to certify WIP.

Do not merge or shut down any machine as part of this handoff. The previous
shutdown request was canceled during the clarification; this session's work
was explicitly stopped for transfer to cloud.
