# Windows NVENC quality and efficiency follow-up

22 October 2026, following Windows-fork PR #6. This builds on the
[initial implementation](windows-live-bitrate.md) and supersedes the status in
the [cloud handoff checkpoint](windows-nvenc-cloud-handoff.md).

## Changes

The Windows H.264 NVENC backend retains p4, low-latency CBR, no B-frames,
zero-latency output and a one-second VBV. Quarter-resolution two-pass encoding
and spatial adaptive quantization improve bit allocation. Frame reordering,
lookahead and temporal AQ remain disabled. Manual bitrate changes continue
through the existing owner thread and driver qualification.

The FLV writer now appends the codec header and payload directly to its output
buffer. This removes a temporary allocation and a full payload copy per tag,
without changing AVC/Enhanced HEVC headers, timestamps, configuration records
or preceding-tag sizes. This shared packet-writing improvement also applies to
the existing Mac path; it does not change Mac encoder settings.

No encoder-selection, saved-show or API schema changes. The existing explicit
experimental NVIDIA choice and startup software fallback remain in place.

## Measured quality

RTX 3060 Ti / NVIDIA 616.92 / FFmpeg 9. Two deterministic six-second 1440p60
synthetic clips, with lossless FFV1 references: `testsrc2` and a moving
Mandelbrot fractal. FFmpeg SSIM compares decoded pictures against the same
reference at equal requested bitrate.

| Clip / target | Previous p4 | p4 + two-pass + spatial AQ |
| --- | --- | --- |
| Test pattern / 8 Mbps | 0.975735 | 0.977136 |
| Test pattern / 32 Mbps | 0.985769 | 0.985828 |
| Moving fractal / 8 Mbps | 0.936851 | 0.945574 |
| Moving fractal / 32 Mbps | 0.962746 | 0.964876 |

These are aggregate SSIM values, not percentage quality gains. The improvement
is content-dependent and smaller at high bitrate. These are local encoder
measurements, not YouTube transcoding or real-show qualification. Spatial AQ
can improve perceived quality while lowering other objective metrics on some
content. More demanding presets were compared too: p5 alone did not consistently
improve the test pattern; p5 plus two-pass/AQ added wall time for a small gain
over p4 plus two-pass/AQ. The lower preset retains more headroom.

Reproduce with `tools/stream-bitrate-probe/quality.ps1`, choosing `-Source
testsrc2` or `-Source mandelbrot` and distinct output directories. It writes
videos, FFmpeg logs and `results.json` locally. Whole command wall times include
decoding/upload/mux/I/O/startup and must not be reported as encoder-only costs.

## Criterion before/after

The standalone `pipeline` benchmark imports production source. A complete
`before` baseline was saved before changing the encoder options or FLV packet
construction. The matching after run used `--baseline before` on the same GPU.

| Operation | Before mean | After mean |
| --- | --- | --- |
| FLV / 4 KiB payload | 143.96 ns | 78.836 ns |
| FLV / 16 KiB payload | 671.58 ns | 260.22 ns |
| FLV / 64 KiB payload | 2.3067 µs | 1.2481 µs |
| 1440p60 NVENC submit/drain / 8 Mbps | 4.9301 ms | 4.5951 ms |
| 1440p60 NVENC submit/drain / 32 Mbps | 4.8992 ms | 4.6214 ms |

Criterion reported packet-time reductions of about 45%, 61% and 46%, and
encoder submit/drain reductions of about 7% and 6% on this workload. These
figures do not imply equivalent whole-app CPU or throughput improvements.
The encoder benchmark uses four pre-generated moving texture frames and
includes CPU NV12 copy/upload and packet conversion. It excludes render,
audio, mux, network and input generation, and runs without real-time pacing.

```powershell
$env:NIDA_BENCH_NVENC = '1' # Omit this variable for packet-only benchmarks.
cargo bench --locked --offline --manifest-path tools/stream-bitrate-probe/Cargo.toml --target-dir target/nvenc-probe --bench pipeline -- --save-baseline before
# Change production source only after the baseline, then compare:
cargo bench --locked --offline --manifest-path tools/stream-bitrate-probe/Cargo.toml --target-dir target/nvenc-probe --bench pipeline -- --baseline before
```

## Validation

Validation on the native Windows host, using core Windows features
(`--no-default-features --features screen-capture`):

- Standalone production-source harness: **11 passed, zero failed**, including
  actual NVIDIA bitrate down/up, decode/frame-count, driver qualification,
  scoped-error detection and limited-range colour checks. All hardware cases
  were explicitly enabled and run serially.
- Native streaming integration: **five passed, zero failed**, including the
  five-minute 1440p60 soak, one-minute 1440p60 and 15-second cases, plus software
  fallback and command-boundary validation. Run outside the network/filesystem
  sandbox, with matching local FFmpeg tools.
- Five-minute settled video rates: **7.9887 / 1.9863 / 32.0161 Mbps** for
  requested 8 / 2 / 32 Mbps phases. No destination reconnects or encoder-queue
  drops; audio/video first PTS difference **21 ms**, final PTS difference
  **27 ms**. This includes startup timeline padding, not a no-repeat claim.
  Keyframe cadence, sufficient received frames, monotonic timestamps, video
  and audio decode, tone presence and bounded stop/reopen checks passed.
- All **87 UI integration tests passed**; the application binary built.
- Production library/binary/streaming-target Clippy with warnings denied
  passed. The standalone harness's all-target Clippy passed. Existing vendor
  wgpu-hal build-script warnings remain.
- Changed backend/harness modules pass rustfmt; diff whitespace checks pass.
  Whole-repository formatting still reports prior differences elsewhere;
  whole-repository all-target Clippy still fails on existing assertion-style
  lints in `main_program`, `add_input`, `persistence_integration` and `rundown`
  tests. No lints or test assertions were suppressed.

The complete core library regression is the remaining validation at the time
of this intermediate update; its result will be recorded before publication.

Added hardware checks cover limited-range Y/Cb/Cr values through encode/decode
at an unaligned frame width, and a five-minute 1440p60 local RTMP soak using
generated PCM and three bitrate phases. The round-trip checks also enforce
two-second keyframe cadence and sufficient received frame count.

Limits remain: NVIDIA H.264 only, CPU-resident input, no automatic adaptation,
no fresh external service broadcast, no physical-device lip-sync test and no
claim to certify all GPUs/drivers or an entire live show.

References: [NVIDIA multi-pass and AQ documentation](https://docs.nvidia.com/video-technologies/video-codec-sdk/13.1/nvenc-video-encoder-api-prog-guide/index.html),
[FFmpeg 9 NVENC option definitions](https://github.com/FFmpeg/FFmpeg/blob/n9.0/libavcodec/nvenc_h264.c).
