# NVIDIA encoding for top-bar REC on Windows

This is a Windows-fork addition, independent of an upstream PR. Open **REC**,
choose H.264 or HEVC and **Encoder → NVIDIA NVENC**, then start recording.
CPU remains the default. Other GPUs, ProRes and unsupported GPU conversion
use the existing software path; the Mac hardware path is retained.

## Encoder selection and recovery

Top-bar REC reuses the separate recording outputs' background NVIDIA
qualification: the driver must successfully encode real test frames. REC
reads the cached result without waiting for a GPU probe. If the check is
still pending or has failed, this file uses CPU and its window says so.
Qualification finishing does not switch an active file; stop/start to use
NVIDIA on a later take. A new request can retry a failed capability check.

Once qualified, FFmpeg's `h264_nvenc` or `hevc_nvenc` receives the existing
GPU-produced NV12 frames. This accelerates encoding; frames still travel
through the existing readback and subprocess pipe. It is not zero-copy GPU
interop. ProRes, AV1 and Intel/AMD hardware encoders are outside this addition.

NVENC uses the existing P4 / low-latency recording settings, VBR with CQ 20,
no B frames or lookahead, and the recording's selected/Auto bitrate target
and ceiling, with a two-second rate-control buffer.
The selected file frame rate, keyframe interval and crash-safe MP4 fragments
are preserved, as are MOV/MKV, audio routing and AAC bitrate. NVENC CQ and
software CRF are different scales; identical visual quality or file size is
not implied by matching bitrate settings.

The recording window shows **Using: NVIDIA NVENC** or **Using: CPU (software)**.
The API adds optional `status.encoding` with `requested`, `actual` and `note`;
settings already accept `encoder: "nvidia_nvenc"`. Saved shows and the software
default retain their existing settings format. Starting/idle status omits the
new encoding object.

If an active NVIDIA encoder stops, normal recording recovery continues in a
new file on CPU. That fallback lasts for the rest of the take; the saved
NVIDIA preference is tried again on the next take. Recovery files keep the
current take's frozen name; name edits still apply to the next take. The
window and API explain the CPU recovery. Disk or other persistent failures
can still prevent recording and remain errors, as before.

The silence-only audio input now ends with the video input rather than
waiting for the recording finalizer's timeout. Background encoder startup,
recording finalization and shutdown tracking remain in place.

## Validation

Validated on Windows x64 with a real NVIDIA GPU:

- 34 focused recording/encoder/UI checks pass, including H.264 and HEVC NVENC
  files with AAC, frame-rate checks, encoder metadata and full decoding.
- Killing the top-bar NVIDIA encoder continues the take in a CPU file; a
  subsequent take uses NVIDIA again and keeps the edited next-take name.
- All 12 recording integration tests and all 15 audio tests pass, including
  software/ProRes A/V checks and the existing two-output NVIDIA soundtrack test.
- 88 UI integration tests, 51 command round-trip tests and 3 API documentation
  checks pass. Production library/application Clippy passes with warnings denied.

Tests run sequentially with `VARDA_REQUIRE_GPU=1`; A/V runs also use
`VARDA_SOFTWARE_DECODE=1` to isolate encoding from decoding. Earlier runs while
a game was active had timing and A/V failures, including on pre-change main
(`27332c6`). Windows logged NVIDIA driver errors (`nvlddmkm`, event 153) around
the game's crash. Recording and audio suites passed after the game stopped;
this supports GPU contention/disruption as a factor without establishing what
caused the crash.

The file-inspection tests wait for background finalization; a readable MP4
fragment does not mean both complete tracks have finished writing. The color
check decodes a real frame from its constant solid rather than seeking beyond
a short clip on a slow debug runner.

No installed application is replaced by this source change. CPU reduction has
not been benchmarked for the complete application, and NVIDIA encoding still
includes the existing GPU readback and subprocess pipe costs.
