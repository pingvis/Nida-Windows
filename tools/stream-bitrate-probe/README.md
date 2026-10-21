# Windows live bitrate feasibility probe

This standalone C program tests the FFmpeg libraries used by the Windows build.
It does not start Nida, contact a streaming service or change show settings.
See [findings and implementation plan](../../docs/windows-live-bitrate.md).

Run from an x64 Visual Studio Developer PowerShell with an FFmpeg development
SDK containing headers, import libraries, tools and matching DLLs:

```powershell
./tools/stream-bitrate-probe/run.ps1 -FfmpegRoot C:/path/to/ffmpeg-sdk
```

The runner compiles with MSVC and links avcodec, avformat and avutil. It writes
only to `target/stream-bitrate-probe` by default. That directory contains local
video fixtures, per-packet CSVs, encode/decode logs, metadata and the executable.
An NVIDIA GPU/driver supporting the SDK's NVENC version is needed. A listed
encoder alone does not establish hardware availability: this probe opens it.

Each case encodes the same deterministic moving monochrome tile pattern:
2560x1440 NV12, 60 fps, 540 frames (nine seconds), no B-frames, GOP 120.
One encoder context is opened and retained. At frames 180 and 360, its bitrate,
maximum rate and buffer size change from 32 to 8 and back to 32 Mbps.

Cases:

- `h264_nvenc`: p4, low latency, CBR, zero latency.
- `libx264`: veryfast, zerolatency, CBR HRD (as used by Nida's H.264 path).
- `libx264-no-hrd`: same software configuration without CBR HRD, to isolate
  that restriction. This is not equivalent constant-rate behavior.

Output is Matroska with explicit packet timestamps/durations, not RTMP.
The runner checks contiguous encoder PTS/DTS, decodes each video with
`-xerror`, and checks 540 decoded frames, 60 fps and nine-second duration.
Reported rates count encoded video packet bytes (including encoder filler),
excluding container, audio and network overhead. The settled figure excludes
the first second of each three-second phase. Inspect measurements and logs:
successful frame delivery alone does not prove the requested rate was applied.

The program runs as fast as possible. Its elapsed time includes pattern
generation, encoding, muxing and writes; it is not encoder-only timing, a
real-time pacing test, application CPU usage or an end-to-end benchmark.
The compiler currently reports conversion/anonymous-union warnings in the
FFmpeg SDK headers.

## Rust production-backend harness

The Rust harness imports `src/internal/streaming/nvenc.rs` and the existing
FLV writer directly. Run from the same configured development shell:

```powershell
cargo test --locked --offline --manifest-path tools/stream-bitrate-probe/Cargo.toml --target-dir target/nvenc-probe
cargo test --locked --offline --manifest-path tools/stream-bitrate-probe/Cargo.toml --target-dir target/nvenc-probe -- --include-ignored --nocapture
cargo clippy --locked --offline --manifest-path tools/stream-bitrate-probe/Cargo.toml --target-dir target/nvenc-probe --all-targets -- -D warnings
```

The ignored test actually opens NVENC, sends moving 1280x720/60 NV12 frames
through one session at 8 -> 2 -> 8 Mbps, serializes the resulting packets
through Nida's FLV writer, and decodes/probes the temporary video. It checks
continuous slots, all 540 decoded frames, rate decrease and recovery.
It does not start a network destination or the complete application.
