# Frame pacing: scheduling precedent and revised recommendation

Research follow-up, 2026-11-20. This supplements the
[local investigation](windows-video-frame-pacing.md); it does not change
production scheduling. Sources are official documentation and version-pinned
source code. No new GPU workloads or head-to-head product benchmarks were run.

## How serious is the measured deficit?

The affected app runs deliver roughly 50–53 fresh pictures/s from verified
60-fps files. The untraced normal-GUI control estimates 49.68 conversion-encoded
pictures/s; that estimate uses asynchronous counter endpoints, not a PTS trace.
The traced normal-GUI interval encodes 52.63 fresh pictures/s. This establishes
a material pre-presentation deficit beyond the benchmark harness.

If that loss reaches a matched 60-Hz output, motion has repeated pictures and
uneven holds. Pans, sports, scrolling text and projection are relevant use cases.
Static backgrounds are less revealing. This is worth fixing for smooth-motion
playback, but it is not evidence of colour corruption, unsafe GPU lifetime or
failed decoding. Keeping the experimental label and defaults is appropriate.

These counts do not measure physical scanout. Different source/output rates
also legitimately require repeats or drops: 60 versus 59.94 differs by about
one picture every 16.7 seconds, far less than the sustained deficit here. Do not
promise broadcast-grade output or a perceptual improvement without measuring
the actual output path.

## What established implementations do

### OBS Studio 33.0.1

The FFmpeg media worker prepares the next frames and calculates absolute wake
times from PTS differences. Resume resets timestamp mapping. It remains
independent of the graphics thread. See
[`mp_media_calc_next_ns` and the media loop](https://github.com/obsproject/obs-studio/blob/33.0.1/shared/media-playback/media-playback/media.c#L467).

The async-video path queues timestamped frames. The graphics tick chooses
against its video time, retires overdue candidates, and smooths small boundary
differences. Explicit unbuffered mode instead keeps the newest frame. See
[`async_tick`, `ready_async_frame`, `get_closest_frame`](https://github.com/obsproject/obs-studio/blob/33.0.1/libobs/obs-source.c#L3853).
Ordinary FFmpeg Media Source uses the async-video interface; its source
implementation does not select unbuffered mode. See
[`obs-ffmpeg-source.c`](https://github.com/obsproject/obs-studio/blob/33.0.1/plugins/obs-ffmpeg/obs-ffmpeg-source.c#L219).

Its Windows graphics wait sleeps most of the interval, then spins to a QPC
deadline. The media worker uses a millisecond sleep instead.
This is evidence that source delivery and output timing are different concerns,
not a reason to copy the CPU/power tradeoff of a spin tail. See
[`platform-windows.c`](https://github.com/obsproject/obs-studio/blob/33.0.1/libobs/util/platform-windows.c#L320).

Important comparison limit: this OBS media decode path downloads hardware
pictures using `av_hwframe_transfer_data` before software-frame output. Its
scheduling design is relevant; its picture-transfer performance does not
represent Nida's GPU-resident path. See
[`decode.c`](https://github.com/obsproject/obs-studio/blob/33.0.1/shared/media-playback/media-playback/decode.c#L284).

### FFmpeg ffplay n8.0.1

ffplay has a bounded three-picture queue, with producer backpressure. Pictures
carry PTS, duration and a seek serial. `video_refresh` chooses when pictures are
due using the selected master clock, normally audio; pause/resume adjusts clock
mapping and stale serials invalidate pre-seek work. It can retain the current
picture or drop late pictures to preserve synchronization. See
[`ffplay.c`](https://github.com/FFmpeg/FFmpeg/blob/n8.0.1/fftools/ffplay.c#L1484).

This is scheduling precedent, not a claim that ffplay's three queue entries are
equivalent to Nida's three GPU resources, or that its thresholds should be
copied. Resource ownership and presentation policy are separate contracts.

### vMix 29

Official documentation describes a PC master clock for audio/video, and
conversion of inputs to the project's master frame rate. Display presentation
has separate Low Latency and Smooth choices; Smooth may add a couple of frames
of delay. Those display choices do not change recording, streaming or external
outputs. See [Performance](https://www.vmix.com/help29/Performance.html) and
[Display](https://www.vmix.com/help29/Display.html).

A [2019 staff explanation](https://forums.vmix.com/posts/t18257-Is-vMix-the-right-choice-for-NDI-production)
describes small buffers for live inputs to absorb arrival jitter and system
load, adding at most roughly 1–1.5 frames versus unbuffered solutions. That is
historical evidence about live inputs,
not disclosure of current local-file playback internals.

vMix also distinguishes source drops, renderer drops and input resynchronizing,
and exposes arrival timing. See [Statistics](https://www.vmix.com/help29/Statistics.html).
Its proprietary implementation does not let us verify the current file queue,
PTS-selection algorithm, Windows wait primitive or equivalent zero-transfer
decode path. No claim of guaranteed perfect vMix pacing on this PC is supported.

### Windows media architecture

Microsoft describes a shared presentation timeline used by sinks to render
timestamped samples. Sources/transforms produce samples when requested rather
than scheduling delivery against that clock. See
[Presentation Clock](https://learn.microsoft.com/en-us/windows/win32/medfound/presentation-clock).
This supports separating preparation from presentation. It does not require
replacing Nida's FFmpeg/D3D12VA/wgpu architecture with Media Foundation.

## What changes in the recommendation

The earlier shorthand of giving everything “one clock” was imprecise. Nida's
playback and worker already use `std::time::Instant`. The measured issues involve
independent epochs/deadlines, late waking and selection at the wrong boundary;
clock-rate drift was not established.

The current path can discard a picture before native copy when multiple PTS
values are eligible, then discard another when publication replaces the
latest-only mailbox. The renderer consumes without a target-PTS choice. Better
sleep precision reduces one disturbance but does not establish a delivery
contract between those stages.

**Inference from the precedents:** a robust contract is to prepare a small
amount ahead, keep timestamps and seek generations, then let output timing
choose the due picture. Future pictures stay available; genuinely late pictures
can still be discarded deliberately. Independent threads need not wake in
lockstep. A single-slot handoff could still suffice if coordinated deadlines
and preparation lead pass the same phase/jitter tests; a queue is not proven
necessary merely because other players use one.

For files, preparing future pictures does not inherently add steady-state
presentation delay if the original PTS mapping is preserved. It can require
startup/seek preroll and occupy more leases. Live jitter buffering and delaying
display presentation are different choices and can add latency. Any readiness
queue must fit the existing three-slot completion-based lifetime contract.
Increasing capacity or introducing picture transfers is not justified.

## A worker-only counterfactual

Replayed the recorded visible-GUI consumer timestamps against a hypothetical
perfect exact-60-Hz producer: zero processing time, constant publication phase,
and the existing latest-only handoff. Used the same 60-second interval starting
at the first encoded PTS at or above 40, with all 3,599 `take_ready` calls.

Exhaustive phase boundaries produce **55.417–59.433 distinct pictures/s**.
Even the best producer phase cannot meet 59.9 with that recorded consumer
cadence. This is a model, not a playback measurement or a prediction of a
queue's performance. Fixing the producer may change contention and therefore
consumer timing; the counterfactual deliberately holds consumer times fixed.
A queue also cannot undo a missed physical output deadline.

The [result and input hash](qualification/rtx3060ti-pacing-20261119/perfect-producer-replay.json)
are retained. Reproduce with Node from the repository root; this is CPU-only:

```powershell
@'
const fs = require('node:fs');
const rows = fs.readFileSync('target/gpu-pacing-20261119/app-windowed-visible.trace.jsonl', 'utf8')
  .replace(/^\uFEFF/, '').trim().split(/\r?\n/).map(JSON.parse).sort((a,b) => a[0]-b[0]);
const begin = rows.find(r => r[2] === 'encode' && r[3] >= 40)[0];
const scaled = rows.filter(r => r[0] >= begin && r[0] <= begin+60e9 && r[2] === 'take_ready')
  .map(r => (r[0]-begin)*60);
const phases = [...new Set([0, ...scaled.map(n => (n%1e9+1)%1e9)])];
const counts = phases.map(p => new Set(scaled.map(n => Math.floor((n-p)/1e9))).size);
console.log({ticks: scaled.length, phases: phases.length,
  best: Math.max(...counts)/60, worst: Math.min(...counts)/60});
'@ | node
```

## Smallest defensible next step

1. Correct the harness sampling placement independently, preserving historical
   counters and an explicitly labelled historical mode.
2. Fix paused-work accounting on Play and command wakes consuming future
   periodic deadlines. Derive worker deadlines from the next PTS and output
   target; choose preparation lead and eligibility policy in step 4. Use a
   precise interruptible Windows wait with an idle/failure fallback.
   Measure accounting and wait changes separately to preserve attribution.
3. Measure actual engine wait request/return, baton handoff and output deadline.
   The observed long GUI gaps require this; a standalone Condvar probe cannot
   establish their cause.
4. Compare deadline-coordinated delivery and a bounded PTS-ordered ready handoff
   against measured jitter and synthetic phases. Implement the smaller policy
   that passes, retaining wall-clock transport authority and three-slot safety.
5. Validate serial release runs for 1080p60 and 4K60 in the app as well as the
   corrected harness. Report fresh PTS, late starts, holds and output timing
   where available. Keep seek/pause/preload/step, speed/VFR/caps, generations,
   fallback, colour and zero-transfer checks, plus a three-minute playlist soak.

This is a targeted scheduling correction. A broad rewrite, larger pool and
relaxed counters/tests are not supported. No production fix or physical-output
qualification is claimed by this research.
