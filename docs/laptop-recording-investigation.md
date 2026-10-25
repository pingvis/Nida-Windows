# Laptop playback and dual-recording investigation

Investigated 2026-10-26 on branch `codex/investigate-dual-recording-performance`.
No production code, drivers, or OS GPU preferences changed. No PR or merge.

## Conclusion

The MP4's uneven motion has two demonstrated contributors: 29.97 fps footage
being sampled into 50 fps output, and renderer gaps under recording load.
Nida fills those gaps with repeated pictures, separately from its encoder-drop
counter. Zero reported drops therefore does not establish smooth motion.

The renderer uses the NVIDIA GPU. Having AMD integrated graphics as well is
not evidence that Nida selected the wrong renderer. Hybrid presentation remains
a possible additional cost, but was not isolated by an adapter/display A/B test.

## Machine and installed build

- AMD Ryzen 9 4900HS, 8 cores / 16 logical processors.
- AMD Radeon integrated graphics; NVIDIA GeForce RTX 2060 Max-Q.
- Internal display reported at 1920x1080, 120 Hz; Windows reports the active
  display mode on the AMD adapter. A Parsec virtual adapter is also installed.
- Nida startup explicitly reports NVIDIA rendering through DX12.
- Installed Inno build: 0.9.2, revision `c445b94752aa23e7f72dda5e5cbdcc135c65f085`.
  Investigation source starts at `956cf6d`; the intervening changes are packaging/docs.
- Power query reported AC operation during the investigation. Show-time power,
  temperatures, clocks and utilization were not captured.

## Evidence from the actual show

The desktop source `Around The World 4K - Scenic Relaxation Film With Calming
Music [6KL2QZ91kAM].mp4` is H.264, **1920x1080 at 30000/1001 fps**, approximately
3.43 Mbps video, with AAC audio. Both FFprobe and the original load log agree.
Its filename does not describe its actual resolution.

The NATURE recording metadata reports 1080p50 and `libx264` encoding.
The original completion logs contain:

| Recording | Renderer frames written | Repeats covering gaps | Repeats / total |
| --- | ---: | ---: | ---: |
| NATURE, first take | 56,974 | 5,538 | 8.86% |
| NATURE, second take | 534,650 | 66,771 | 11.10% |
| Recording 1 | 594,376 | 72,618 | 10.89% |

These are writer-inserted repeats, additional to any repetition of a 29.97 fps
source within a 50 fps timeline. They do not count unique source pictures.
Both output files had renderer gaps; the capture recording looking acceptable
does not mean its renderer delivered every requested frame.

Even ideal 29.97-to-50 sampling displays source pictures for unequal numbers
of output frames. Smooth camera pans can expose that cadence particularly well.
This mechanism explains a source-specific component without requiring a decode
failure. A 50 fps recording cannot recover motion the 29.97 fps source lacks.

A 1080p50 H.264 stream was also running before the MP4 was loaded. There are
626 destination-backpressure warnings in the 15:00–18:59 UTC log window.
That establishes additional concurrent work and slow stream destinations;
it does not establish that the network caused the renderer gaps.

## Controlled reproduction in the installed app

The initially loaded saved show contained a Ferrari JPG and a disconnected
UVC capture input, not the show's MP4. Its scene snapshot was retained before
testing. The missing camera deck was temporarily removed, the actual desktop
MP4 loaded and taken to Main, and short H.264 output recordings made locally.

| Configuration | Observed engine rate | Notes |
| --- | --- | --- |
| MP4 playing, target 50, no recording | 49.03 fps | One rolling snapshot; interval mean 20.32 ms |
| MP4 playing, target 50, one recording | 48.12 fps | One rolling snapshot; interval mean 20.73 ms |
| MP4 playing, target 50, two recordings | 42.51–45.03 fps | 12 consecutive samples; rolling mean intervals 23.40–23.89 ms |
| MP4 playing, target 30, two recordings | 30.00–30.03 fps | 10 samples; settled rolling intervals about 33.33 ms, zero late frames |

The dual-output 50 fps recording starting second logged 834 renderer frames
and **147 repeats in 19.6 s** (~15.0% repeats). At 30 fps, the two short takes
logged 587+2 and 586+1 renderer/repeated frames in approximately 19.6 s.
The first 30 fps telemetry sample still included the configuration transition;
the subsequent nine samples reported zero late frames.

These tests recorded the same MP4 master twice, without live capture or the
show's stream, and without recording audio. They demonstrate a load problem
but are not a complete reproduction of the original capture-card workload.
30 fps is a useful diagnostic and approximate match; it is not exactly 29.97.
For mixed live-camera production, choose a common rate appropriate to all sources.

## Encoding and decoding paths

The output-recording path always selects `libx264` for H.264 in
`src/internal/renderer/subprocess.rs` (`recording_encoder`,
`recording_video_args`, `spawn_recording_with_presentation`). Both test
recordings therefore encode on the CPU, alongside playback and rendering work.
The dedicated REC feature and streaming have separate configuration paths;
their encoder options should not be confused with output recordings.

A direct test using the installed FFmpeg successfully encoded 100 generated
1080p50 frames with `h264_nvenc` to a null sink. NVIDIA hardware encoding is
available on this machine. This does not yet benchmark two NVENC sessions or
prove that replacing the recording encoder alone fixes every render stall.
Startup's generic “Hardware H.264 encoder: Unusable” message is not sufficient
to conclude that NVENC is unavailable; its probe is a separate code path.

The MP4 playback snapshot reported `hardware_decode: false`. Windows hardware
decode defaults off in `src/internal/video/hwdecode.rs` and is opt-in.
No hardware/software decode A/B was performed. Software decoding contributes
work, but a single 1080p clip has not been shown to be the dominant bottleneck.

## Separate confirmed missing-camera stall

`VardaApp::reconcile_cameras` in `src/app/state/inputs.rs` synchronously calls
`camera_manager.scan_devices()` every two seconds while an input waits for its
camera. On Windows, `find_devices` / `read_formats` in
`src/internal/camera/mod.rs` opens inactive devices to read their formats.

With the UVC device disconnected, each retry probed four unavailable NDI
virtual webcams and logged Media Foundation failures on the engine thread.
Before removing the waiting deck, telemetry showed a 788.49 ms maximum frame
interval and 787.74 ms maximum mixer time. An earlier log showed 1415.3 ms in
the mixer. After removal, the paused-source sample's maximum interval was
24.75 ms and the playing-source baseline's was 25.28 ms.

This blocking path should move device/format discovery off the engine thread.
Repeated probes of unchanged failing devices also need caching/backoff.
The original show had only four format failures in the inspected four-hour
window, at startup with the UVC device present. Consequently this confirmed
current-session bug is **not established as the cause of the show's stutter**.

## Hybrid graphics: what remains uncertain

Windows GPU counters attributed Nida activity to two adapter LUIDs; the startup
log identifies the rendering adapter as NVIDIA. Baseline control-window timing
showed roughly 18 ms mean in present, with many approximately 25 ms UI frames.
That can make the preview less smooth than the underlying engine/recording.
GPU drain profiling showed sub-millisecond-scale waits in example frames even
when CPU-side channel work took several milliseconds. These are diagnostic
observations, not proof of an AMD/NVIDIA transfer bottleneck.

An equivalent test on a display attached directly to NVIDIA, or a supported
discrete-only display mode, would be needed to isolate that contribution.
No display routing, registry GPU preference, driver, or firmware setting was changed.

## Recommended engineering follow-up

1. Add explicit NVENC selection to output recordings, with capability testing
   and software fallback; benchmark two sessions with capture and stream active.
2. Move missing-camera discovery and format probing off the engine thread.
3. Make renderer padding prominent in recording health, alongside encoder drops.
   The existing output tooltip already exposes padding, but colors health by drops.
4. Support/review fractional output rates and test 29.97/59.94 timing for this clip.
5. Profile CPU-side channel rendering and presentation separately under the full
   show workload; compare hybrid and NVIDIA-attached presentation before attributing
   the remaining cost to hardware routing.

## Local artifacts and final state

Raw API telemetry, original scene snapshot and short recordings are in
`target/performance-investigation/` (Git-ignored). The original application log
remains in the user's local Nida log directory. Tests used API calls and the
installed FFmpeg/FFprobe; no new packages were installed.

All test recordings were stopped and temporary output definitions removed.
The saved workspace was reloaded, restoring the original JPG/capture show and
its single inactive recording output. Target FPS was restored to the original 60.
No test changes were explicitly saved over the user's show. This report is the
only tracked-file change; no production-code tests were necessary.
