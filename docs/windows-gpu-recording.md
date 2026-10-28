# NVIDIA encoding for output recordings

Implementation on `codex/investigate-dual-recording-performance`. This is a
source change; it does not update an installed Nida executable automatically.

For a Windows H.264 or H.265 (HEVC) recording output, the output controls now
offer **Encoder: CPU (software)** or **NVIDIA (NVENC)**. Stop the output before
changing its encoder. Codec, source, folder, audio device and encoder preference
are independent settings. The encoder preference survives saving/loading a show.
Existing shows and newly created outputs retain software encoding by default.

The HTTP API accepts the same choice through the existing output target routes:

```json
{
  "target": {
    "Recording": {
      "path": "C:/Recordings/show.mp4",
      "codec": "H265",
      "encoder": "NvidiaNvenc",
      "audio_device": null
    }
  }
}
```

Use `Software` to select the CPU. Omitting `encoder` also selects software.
The dedicated top-bar REC feature and streaming settings are separate paths.

## Capability checks and fallback

Startup's existing encoder-probe worker now tests H.264 NVENC, HEVC NVENC and
HEVC Main10 with the recording options. It opens and encodes real frames rather
than trusting FFmpeg's compiled encoder list. Each subprocess probe has a
five-second deadline; REC reads an atomic result and never waits for a probe.

If a codec/platform/precision is unsupported, qualification fails, qualification
is still pending, or encoder metadata cannot be queried, the take uses software.
Its output presentation status and notification report that fallback. A pending
probe does not switch a running take later; stop/start after qualification to retry.
A later explicit start request retries failed qualification in the background, so
a temporary startup failure does not disable acceleration for the whole session.
Runtime failures (for example, exhausted sessions or unsupported output sizes)
remain recording errors. There is no mid-take encoder switch or automatic restart.
The startup probe is a general capability check, not a reservation of GPU resources.

NVENC uses preset P4, low-latency tuning, constant-quality VBR (CQ 20), no B frames
or lookahead, and Rec.709 limited-range output metadata. CQ is not equivalent to
the software encoder's CRF. Files may differ in size and image quality.

Eight-bit GPU NV12 readback feeds NVENC without CPU RGB-to-YUV conversion.
HEVC Main10 uses the existing packed RGB10 readback and FFmpeg's P010 conversion.
A software Main10 plan is retained if hardware cannot preserve its precision.
ProRes, HAP, alpha recording and AV1 keep their existing software paths.

This change accelerates encoding. It does not enable GPU decoding, remove GPU
readback, repair missed render frames, or interpolate a 29.97 fps clip into 50 fps.

## Validation and remaining work

The laptop's installed FFmpeg and RTX 2060 Max-Q successfully ran three concurrent
1080p50 encoder sessions: H.264, HEVC Main and HEVC Main10. Each produced 150 frames;
FFprobe verified the codec, 50/1 rate, frame count and Main10 pixel depth/profile.
These were generated-source driver tests, not a rebuilt Nida performance benchmark.

An isolated Rust harness compiles the actual output value types and extracts the
recording-plan functions/tests from source. It covers old API defaults, serialization,
NV12 input, P010/Main10 output, unsupported codecs and software precision fallback.
To reproduce on Windows with Rust, Node, FFmpeg and FFprobe on PATH:

```powershell
node scripts/ci/prepare-recording-contract-check.cjs
cargo test --manifest-path target/performance-investigation/nvenc-harness/Cargo.toml -- --nocapture
```

The harness's hardware test requires a working NVIDIA encoder and runs all three
sessions together. Generated test crates and videos stay under the ignored
`target/performance-investigation/` folder. This harness is deliberately narrower
than the full application: it does not type-check the output UI, persistence wiring,
or subprocess lifecycle integration.

The native Windows build and application checks are documented in
[Laptop GPU recording validation](laptop-recording-validation.md), including
CPU measurements, audio synchronization, Main10, output lifecycle and session
exhaustion/recovery. MSVC and the FFmpeg development SDK are now available on
the laptop. The installed app has been left untouched.

GPU encoding reduced CPU load, but the measured render/pacing limitation remains.
Before show use, repeat the real MP4 plus moving capture-card, two-output, 50 fps
workload and a longer recording soak. The capture card was unavailable for these
checks. Do not treat a file's 50 fps metadata as proof of fifty distinct frames
each second; inspect padding and encoder drops separately.
