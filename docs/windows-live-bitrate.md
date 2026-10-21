# Windows live streaming bitrate investigation

Investigated 22 October 2026 against fork main `ea9144b`.
Scope: Windows-fork implementation and reproducible local qualification.
The initial investigation is retained below; implementation status follows it.

## Original software-path limitation

Windows uses a subprocess running FFmpeg's x264/x265 encoders. Nida supplies
the bitrate on process launch and has no control channel to reconfigure it.
The UI uses `StreamStatus::rate_live`; the command handler also rejects live
edits when the encoder cannot apply them.

The Mac path is different: `subprocess/native.rs` owns VideoToolbox, receives
requested bitrate changes through `NativeShared::wanted_kbps`, and emits
timestamped FLV video packets to a persistent FFmpeg audio/mux process. The
existing MPEG-TS fan-out then sends to independently reconnecting destinations.
Non-Mac builds currently use a native-encoder stub. This is an implementation
gap, not a YouTube prohibition on changing bitrate.

There is also a software configuration restriction. Nida's H.264 arguments
include `nal-hrd=cbr:force-cfr=1`. Changing AVCodecContext rate fields in a
standalone x264 session with CBR HRD produced warnings that VBV parameters
cannot change with NAL HRD; output stayed at the old rate. Removing that HRD
option allowed changes, but the test no longer held the requested constant
rate. Do not equate this experiment with a production software solution.

## Hardware experiment

Hardware: NVIDIA GeForce RTX 3060 Ti, driver 616.92.
SDK: FFmpeg `n9.0.2-31-gc89d6ee1ca-20261021`, MSVC import libraries.
avcodec-63.dll SHA256:
`8E257F46E0036FF7ACC5C3C014FF8AB102048A16B3B64AC952DCCF11E519AB71`.

The [standalone probe](../tools/stream-bitrate-probe/README.md) opens one
encoder context and submits nine seconds of deterministic 1440p60 NV12 video.
It requests 32 -> 8 -> 32 Mbps, at three-second intervals, without reopening
the encoder. H.264 only; p4 low-latency NVENC, no B-frames, two-second GOP.

Measured encoded-video rates in Mbps, with the first second of each phase
excluded from the settled measurement:

| Encoder | Requested | Whole phase | Settled |
| --- | --- | --- | --- |
| NVENC | 32 | 31.759 | 31.638 |
| NVENC | 8 | 8.436 | 8.012 |
| NVENC | 32 | 24.830 | 31.838 |
| x264 with CBR HRD | 32 | 30.935 | 32.000 |
| x264 with CBR HRD | 8 | 32.000 | 32.000 |
| x264 with CBR HRD | 32 | 32.000 | 32.000 |
| x264 without HRD | 32 | 20.330 | 18.410 |
| x264 without HRD | 8 | 9.597 | 10.018 |
| x264 without HRD | 32 | 17.071 | 16.657 |

Both rate directions worked on NVENC, with a settling interval, especially
after increasing the target. Do not promise next-frame rate convergence.
The completed probe cases each emitted 540 packets with contiguous encoder
PTS/DTS and decoded without errors. All three timestamped Matroska outputs
were also verified as 540 decoded frames at 60 fps over nine seconds.

This proves local encoder feasibility on this GPU/driver/SDK. It does not
validate Nida integration, audio sync, RTMP connection continuity, YouTube,
real-time queue behavior, colour accuracy, other GPUs, HEVC, quality presets,
device loss or sustained shows. CPU-resident NV12 is uploaded to NVENC; this
is not zero-copy GPU encoding. No full application throughput/CPU claim follows.

## Recommended Windows-fork implementation

Use the existing FFmpeg library dependency to own a Windows NVENC encoder
inside the app, on a dedicated worker. Start with explicitly selected H.264
NVENC and retain software fallback on startup. Avoid a direct NVIDIA SDK
binding unless the FFmpeg wrapper proves insufficient.

1. Generalize the native video encoder boundary while keeping Mac behavior.
   Reuse the timeline, bounded submission, audio/mux process and fan-out.
   Generalize the currently Mac-gated FLV packet writer and encoder events;
   convert NVENC Annex-B NAL units/config into the form the writer expects,
   or use a timestamped libavformat output at that boundary. Never pass bare
   H.264 to a pipe and infer time from frame count.
2. Apply requested bitrate, maximum rate and VBV changes on the encoder-owning
   thread before submitting a frame. Report actual observed behavior and
   reconfiguration failures; simply copying the request into `applied_kbps`
   is insufficient. FFmpeg's NVENC wrapper logs some reconfiguration failures
   rather than returning them through send-frame, so failure reporting needs
   an explicit design and regression coverage before adaptive rate is enabled.
3. Expose live bitrate editing only for a successfully opened, qualified
   backend. Separate bitrate capability from switching CBR/VBR: Mac's current
   VBR implementation strips filler, and that behavior has not been established
   for NVENC. Do not enable all existing native features based on a single flag.
4. Initially retain fixed resolution/frame rate/codec and disabled automatic
   adaptation. Test manual changes first. Keep backend selection explicit;
   preserve old show preferences and identify a software fallback visibly.

Acceptance before shipping: local RTMP output with real mixer audio and
32 -> 8 -> 32 Mbps changes, no destination reconnect, playable output,
monotonic timestamps, stable audio sync, measured transmitted rates/settling,
colour/range checks, bounded queues under backpressure, missing-device/open
failure and runtime-error handling, stop/restart cleanup, and a sustained
1440p60 real-content run. Follow with a private YouTube check. Runtime loss
must be reported; seamless software fallback is not established by this probe.

## Primary references

- [NVIDIA NVENC programming guide: encoder reconfiguration](https://docs.nvidia.com/video-technologies/video-codec-sdk/13.1/nvenc-video-encoder-api-prog-guide/index.html).
- [FFmpeg 9 NVENC implementation](https://github.com/FFmpeg/FFmpeg/blob/n9.0/libavcodec/nvenc.c): dynamic capability check, bitrate/VBV reconfiguration and error handling.
- [FFmpeg 9 x264 wrapper](https://github.com/FFmpeg/FFmpeg/blob/n9.0/libavcodec/libx264.c): runtime parameter forwarding; the underlying x264 configuration can still reject changes.

## Implementation

Branch: `codex/windows-live-bitrate`, based on fork main `ea9144b`.
The production Rust backend is `src/internal/streaming/nvenc.rs`.
It owns the FFmpeg encoder/frame, accepts validated bitrate requests, drains
timestamped packets, and translates Annex B SPS/PPS and frames into AVC
configuration/length-prefixed units for Nida's FLV writer. The Windows native
worker plugs into the existing timeline, bounded frame queue, audio mux and
destination fan-out. It is explicitly selected as `nvidia_nvenc`; the existing
Mac `hardware` and shared `software` values retain their behavior/defaults.

Selecting NVENC in the UI chooses H.264/CBR/manual control. The command boundary
rejects HEVC, non-CBR and automatic adaptation for this backend. Live bitrate
editing is qualified separately from mode/adaptation (`rate_mode_live`). New
shows and old preferences retain the existing platform encoder defaults.

An actual encode/rate-change probe runs once per process (also warmed on the
existing encoder-probe thread). Missing driver support or failed stream opens
produce software fallback with a visible `rate_note`, not a live-editable
bitrate box that cannot work. Runtime failures are logged and reach the existing
encoder health/restart path. Automatic seamless software recovery is not claimed.

FFmpeg may log a reconfiguration error while returning success from frame
submission. A process-lifetime callback forwards all messages to FFmpeg's
default logger and flags error-level messages only for an explicitly watched
encoder context on its owning thread. Calls reset/restore that scope. Rejected
changes therefore do not publish a new accepted target. This callback is
Windows-only; Nida has no other FFmpeg callback owner. Future logging callback
integrations must preserve this error reporting.

Checks completed on 22 October 2026:

- Windows application library check, locked/offline, no default features,
  `screen-capture`: passed. Existing vendor wgpu-hal build-script warnings.
- Rust standalone harness importing production backend and FLV writer:
  ten tests passed, including the explicitly requested native NVENC test,
  driver qualification and context-scoped error handling.
  720p60, 540 contiguous slots, 8 -> 2 -> 8 Mbps in one session; encoded bytes
  per three-second phase: 2,988,474 / 847,352 / 2,327,861. All frames decoded.
- Focused production library/binary/streaming-test Clippy with warnings denied:
  passed. Repository-wide all-target Clippy still reports existing assertion
  and numeric-literal lints in unchanged tests; no allowances were added.
- All 87 UI integration tests and all three API documentation/schema tests pass.
- Four NVENC integration checks pass: invalid-option rejection, visible software
  fallback, 15-second local RTMP and one-minute 1440p60 local RTMP. The native
  cases include generated 48 kHz stereo sound, a runtime rate increase above
  the starting target, and bounded stop/reopen cleanup at two bitrates.
- The one-minute run requests 8 -> 2 -> 32 Mbps. After excluding the first
  two seconds of each 20-second phase, received video averaged 7.892 / 2.366 /
  31.715 Mbps (packet bytes; excluding audio/container overhead). The stream
  stayed on the same destination generation with zero reconnects and zero
  encoder-queue drops. All received video/audio decoded, and the tone was
  present. Audio/video first PTS differed by 21 ms and last PTS by 38 ms.
  The writer submitted 3,600 generated frames and padded 34 initial timeline
  slots while the audio/video pipeline started; this is not a no-repeat claim.
- The ordinary streaming suite reports 14 passed, two failed and two ignored.
  The failures are the unchanged reconnect tests
  `a_change_of_network_cuts_the_wait_between_attempts_short` and
  `a_destination_that_is_down_reconnects_on_its_own_when_it_comes_back`.
  The latter also fails in isolation. Reconnect problems were already recorded
  in windows-development.md; they are not fixed by this encoder change.
- New Rust modules pass rustfmt; diff whitespace checks pass. Repository-wide
  formatting retains existing differences, so it is not claimed green.

The local RTMP regression is explicitly ignored by default because it needs a
real NVIDIA device and reachable loopback sockets. Run outside a network
sandbox, with the matching FFmpeg tools on PATH:

```powershell
cargo test --locked --offline --no-default-features --features screen-capture --test streaming windows_nvenc -- --ignored --nocapture --test-threads=1
```

The test uses synthetic moving frames and a generated 48 kHz stereo PCM tone,
checks bitrate down/up, continuous timestamps, unchanged destination generation
and zero reconnects, verifies audio/video start/end alignment, decodes the
received file and checks that sound is present. No real streaming service or
user's stream key is used.

Limits: H.264 and NVIDIA only; CPU NV12 upload, no zero-copy; no automatic
adaptation or live mode changes. This does not certify long-running shows,
physical device lip-sync, all drivers, performance relative to x264 or YouTube
quality. Public upstream submission is out of scope; PR targets the Windows
fork only.
