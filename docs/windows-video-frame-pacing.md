# Windows GPU-resident video: frame pacing investigation

Investigation starting from `main` at `a5b98f1fe93b9f1e4f82214fc0cc83ce63b2c1bd`
(also checked against `origin/main`), on the RTX 3060 Ti / Ryzen 9 5950X PC.
The experimental label, defaults, colour shader and thresholds, three-slot pool,
ownership and fallback policies are unchanged. The measurements below describe
the initial diagnostic build. Implementation now proceeds on
`fix/windows-video-frame-pacing`; its separate evidence must not be confused
with those baseline measurements.

**Current branch status:** fresh release builds restore approximately 60 fresh
pictures/s in the ordinary app, 4K and fast-playback checks. Overall qualification
still fails three short seek-phase windows. The
[latest measured candidate](qualification/rtx3060ti-pacing-readiness-cadence-20261120/README.md)
retains these residuals explicitly; this branch is not ready to merge as a
fully qualified pacing fix. No push has been performed.

**Measured result:** the deficit is a scheduling/deadline-alignment problem before
composition, with additional losses in the independently timed latest-picture
mailbox. It occurs in the actual app as well as the offscreen harness. Coarse
Windows timeout behavior contributes, but requesting 1 ms resolution alone does
not fix it: one such run still delivered 49.11 fps. The harness also introduces
its own periodic sampling hitch. Follow-up must address picture availability at
render deadlines as well as worker waits. There is no measured reason to change
the decoder, colour converter or pool capacity. The
[research follow-up](windows-video-frame-pacing-research.md) compares OBS, ffplay
and vMix, and refines the implementation plan below.

## Measurement boundaries

The old offscreen benchmark is not a presentation benchmark. It runs its own
60 Hz clock, samples a latest-picture mailbox, submits conversion/blit commands,
then calls `device.poll(Wait)` every tick. The production deck uses
`take_ready_frame`; the windowed engine and UI have separate clocks and use
nonblocking GPU polling. A low offscreen tick-service time does not measure the
worker's decode/copy wait, or establish that pictures are available at render
deadlines. Windows' existing `take_presented_times` returns an empty vector;
CPU calls to `queue.present` do not establish scanout times.

All measurements must run serially, after checking for active exports, rendering
or games. Record background activity, exact binary/source/shader hashes and the
fixture packet timestamps. Do not compile or generate fixtures during timed
playback. Keep the repository release profile (`opt-level=3`, thin LTO, one codegen
unit, unwind); no shader override is used for this investigation's new builds.

Use the **continuous** H.264 fixtures, not the four-second correctness clips in
the adjacent `performance/` directory. Both continuous files were independently
probed: 7,200 timestamped pictures, 60/1 fps, first PTS 0, last PTS 119.983333 s;
successive sorted packet PTS differ by 0.016666–0.016667 s (ffprobe's decimal
rounding). Measurements shorter than the file avoid loop flushes.

The benchmark's one-second warmup is unrendered. GPU payloads discarded there
increment cumulative `dropped_frames`; CPU pictures have no corresponding GPU
payload destructor counter. Keep those counters intact. New direct atomic
snapshots at the measurement boundaries supplement the historical snapshot
fields; trace markers delimit measured drops, publications and consumption.
Snapshots are individually atomic counter reads, not an atomic transaction with
the worker or the trace marker. Small boundary differences are possible.
Pictures superseded during PTS selection, before a GPU payload is constructed,
do not increment the GPU payload-discard counter either. The trace records those
selection skips separately; changing that counter's definition would obscure
the comparison.

## Prior evidence, before new instrumentation

The ten original RTX 3060 Ti GPU-resident 60-second records already distinguish
producer shortfall from net unconsumed pictures:

| Resolution | Render ticks per run | Produced pictures | Consumed pictures | Net produced minus consumed |
| --- | ---: | ---: | ---: | ---: |
| 1080p60, five runs | 3,600 | 3,106–3,576 | 2,988–3,544 | 1–118 |
| 4K60, five runs | 3,600 | 3,164–3,213 | 3,163–3,212 | 1 in each run |

Thus mailbox replacement cannot explain most of the original 4K deficit.
These aggregate counters alone cannot distinguish missed worker deadlines,
PTS selection, and native readiness. They are clues, not proof of a timer cause.
The G14's slowest 1080p run similarly produced 3,326 and consumed 3,296 pictures
over 3,600 ticks. Its five 4K runs produced 3,601 each and consumed 3,563–3,600;
the extra produced picture reflects the independently sampled measurement
boundaries, not a negative frame-loss count.
The laptop's D3D11VA baseline selects AMD while the renderer selects NVIDIA;
cross-path/cross-machine comparisons do not isolate transfer removal on equal
adapters. No new AMD/Intel qualification is claimed.

Static constraints also narrow the hypotheses. The offscreen benchmark has no
surface reconfiguration, and native/wgpu submissions both hold the submission
gate shared; they do not exclude one another there. The native copy fence wait
runs outside that gate. Neither constraint establishes that driver execution,
GPU polling or operating-system wake behavior is harmless; those remain timing
measurements.

## Reproduce the diagnostic controls

Load the local x64 MSVC/FFmpeg build environment, then build and save the exact
release test executable and app. Freeze source during **all** Cargo passes:
`cargo test --lib` also builds the normal library through a development dependency
before compiling the test crate. A build started before editing is not sufficient
proof that its test binary predates the edits. The initial overlapping build was
preserved as intermediate/unmeasured and excluded from final evidence.

```powershell
. ./.windows-tools/build-env.ps1
cargo check --offline --tests --no-default-features --features screen-capture,windows-browser -j 1
cargo test --offline --release --no-default-features --features screen-capture,windows-browser --lib --no-run -j 1
cargo build --offline --release --no-default-features --features screen-capture,windows-browser -j 1
Remove-Item Env:NIDA_TEST_GPU_VIDEO_SHADER,Env:VARDA_SOFTWARE_DECODE -ErrorAction SilentlyContinue

$binary = '<release lib test executable printed by Cargo>'
$clip = 'target/gpu-qualification/fixtures/continuous/h264-1080p60.mp4'
# Every output must have a fresh name. Use 4k60 for the other resolution.
./scripts/measure-video-pacing.ps1 -TestBinary $binary -Clip $clip -Output target/pacing-control.log -Seconds 60
./scripts/measure-video-pacing.ps1 -TestBinary $binary -Clip $clip -Output target/pacing-trace.log -Seconds 60 -Trace
./scripts/measure-video-pacing.ps1 -TestBinary $binary -Clip $clip -Output target/pacing-poll.log -Seconds 60 -Trace -PollOnly
./scripts/measure-video-pacing.ps1 -TestBinary $binary -Clip $clip -Output target/pacing-timer.log -Seconds 60 -Trace -Timer1ms
./scripts/measure-video-pacing.ps1 -TestBinary $binary -Clip $clip -Output target/pacing-phase8.log -Seconds 30 -Trace -PhaseMs 8
python scripts/analyze-video-pacing.py target/pacing-trace.log.trace.jsonl > target/pacing-analysis.json
```

`PhaseMs` shifts the offscreen consumer after warmup; `PollOnly` substitutes
nonblocking polling in that harness; `Timer1ms` requests and balances
`timeBeginPeriod(1)` / `timeEndPeriod(1)` inside the test process. These controls
do not themselves change production scheduling. The current harness samples
CPU/RSS before its deadline sleep; `-HistoricalSampling` restores the old
sampling placement. `-LegacyWait` and `-PeriodicWake` are test-only controls for
the old Windows wait primitive and periodic worker policy. They do not undo
the playback accounting fix. Use the archived diagnostic binary for the exact
baseline behavior described below.
The wrapper restores environment variables and writes source hashes alongside
the existing binary/fixture/adapter manifest. Compare traced with untraced runs:
trace I/O and additional fence observations can perturb timing.

Timer granularity is an experimental variable. Microsoft documents that
[`timeBeginPeriod`](https://learn.microsoft.com/en-us/windows/win32/api/timeapi/nf-timeapi-timebeginperiod)
affects wait accuracy, has process-specific behavior on recent Windows, and is
not guaranteed for an occluded window-owning process on Windows 11. It does not
improve the performance counter. A successful request is not proof of a fix;
measure actual wait overshoot and picture delivery.

## Trace format and limits

Set `NIDA_VIDEO_PACING_TRACE` to a **new** local JSONL path before launching the
test or app. Each row is `[monotonic_ns, thread_id, event, a, b]`, on one shared
`Instant` origin. Rows are sorted by timestamp during analysis. The file writer
has a 64 KiB buffer and flushes at benchmark completion and traced thread exit.
No picture bytes are recorded. A forcibly terminated process can lose its final
buffer; do not treat such a trace as a completed run.

| Event | `a` | `b` |
| --- | --- | --- |
| `worker_wake`, `decode_begin`, `decode_none` | playhead seconds | 0 |
| `worker_wait` | requested wait seconds | interval seconds |
| `worker_timeout`, `worker_command` | elapsed wait seconds | requested wait seconds |
| `schedule_reset` | overshoot beyond next schedule slot, seconds | interval seconds |
| `playhead` | advanced playhead seconds | lookahead PTS, or -1 |
| `decoded_pts` | received FFmpeg picture PTS, seconds | 0 |
| `select` | selected PTS | 1 if superseding an already selected picture in this call |
| `copy_begin`, `encode`, `gpu_complete` | picture PTS | 0 |
| `decoder_fence_at_submit` | picture PTS | 1 if decode fence already complete |
| `decoder_fence_observed` | picture PTS | 0 |
| `native_submit` | picture PTS | worker-local copy serial |
| `copy_sleep` | picture PTS | observed sleep seconds |
| `copy_complete` | picture PTS | one-millisecond polling sleeps performed |
| `pool_full` | retained picture PTS | 0 |
| `publish_begin`, `publish`, `publish_stale`, `invalidate` | mailbox generation | `publish`/`invalidate`: 1 if displacing a picture |
| `take` | 1 if consuming a picture | 0 |
| `take_ready` | 1 if consuming a picture | CPU upload staging ready |
| `payload_drop` | picture PTS | 1 if encoded (0 means unencoded discard) |
| `render_tick` | offscreen tick sequence | elapsed measured seconds |
| `render_wait` | requested sleep seconds | next offscreen tick sequence |
| `render_complete` | offscreen tick sequence | 0 |
| `submit_begin`, `submit_end` | 0 | 0 |
| `present_begin`, `present_end` | output frame sequence | 0 |
| `ui_present_begin`, `ui_present_end` | UI frame sequence | 0 |
| `engine_timer_wait` | requested fallback wait seconds | 0 |
| `engine_timer_return` | elapsed fallback wait seconds | requested wait seconds |
| `engine_frame` | baton wait seconds | GPU wait seconds |
| `measurement_begin` | harness phase milliseconds | nonblocking poll enabled |
| `measurement_end` | measured seconds | render ticks |

Mailbox publication and consumption events are recorded while holding their
ownership mutex, preserving their causal order. Native and decode fence events
are **CPU observations**, not GPU timestamp queries. A completion callback's time
also includes delay until wgpu is polled. `native_submit` begins before the
submission gate; it does not say when the GPU executed the copy.

The analyzer is deliberately for a **single continuous 60-fps clip**. It joins
native payloads by PTS, counts PTS gaps, worker timeouts, empty takes, mailbox
replacement and unencoded drops. Do not use its PTS joins across concurrent
clips, seeks, loops or reloads. For the actual app, select a steady interval with
`--pts-start 5 --seconds 60`; no warmup marker is injected into app scheduling.
Its consumer-phase sweep is an offline model using the same observed producer
publications and ideal 60 Hz samples. It is not another playback measurement.

## New measurements and implementation recommendation

The [evidence bundle](qualification/rtx3060ti-pacing-20261119/README.md) contains
all 14 raw benchmark records, trace summaries and deterministic event excerpts,
actual-app traces and untraced controls, timer probes, fixture hashes and
correctness logs. Full
traces, exact measured source copies and binaries remain local under
`target/gpu-pacing-20261119/`. All GPU workloads ran serially; activity checks
preceded each run. No compilation or fixture generation ran during timed playback.

### Release builds and scope

- Base: `a5b98f1`, plus opt-in diagnostic instrumentation. Normal repository
  release profile and `screen-capture,windows-browser`, without default features.
  The test binary inherits `test-fixtures` through the repository's existing
  development dependency; the production app does not.
- Release test SHA-256:
  `745D5651533B4FCB906D451095142C5DF7F19626CC1DF331D10AD5C13B8D9B01`.
- Release app SHA-256:
  `00B7C142BC8FF8FDBA734FDC97DAE015D7A104FC76DB55CB237775FD199CC59C`.
- Both embed fixed production shader SHA-256
  `D651DB90D51608CE57B498F2BBF827F8D0982925F28379DFE1601438A8AF3B4A`.
  **No `NIDA_TEST_GPU_VIDEO_SHADER` override** was used. This is a fresh build,
  unlike the earlier focused shader-override recheck.
- Source hashes were checked before and after the frozen builds. One subsequent
  source edit rewrote `publish_decoded`'s `match` to the equivalent `if let` to
  satisfy Clippy's `single_match_else`. The measured source snapshot is retained;
  no rebuild of that lint-only rewrite is claimed. Final production-library
  Clippy passes with warnings denied. `git diff --check` passes.

### Reproduction and processing headroom

Three interleaved **30-second untraced** runs per resolution, with the same
historical offscreen waits and 120-second continuous fixtures:

| Resolution | Fresh fps median (range) | CPU median, % of one core | Tick-service p95 median, ms |
| --- | ---: | ---: | ---: |
| 1080p60 | 48.82 (47.42–51.12) | 1.30 | 0.866 |
| 4K60 | 47.89 (47.55–47.92) | 4.32 | 1.600 |

These are new short controls, not replacement five-run 60-second medians for
the earlier qualification. They reproduce the shortfall without trace I/O.
All 14 benchmark runs retain zero production picture downloads/uploads and
zero pool-backpressure events. Peak occupancy is two, except the nonblocking
poll experiment's three. No required-path or GPU-validation assertion failed.

The two 60-second default traces show the loss accounting explicitly:

| Stage/count | 1080p60 | 4K60 |
| --- | ---: | ---: |
| Render ticks / worker wakes | 3,600 / 3,600 | 3,600 / 3,600 |
| FFmpeg pictures received / selected | 3,600 / 3,600 | 3,600 / 3,600 |
| Selected pictures superseded before GPU copy | 4 | 453 |
| Completed GPU payloads published | 3,596 | 3,147 |
| Mailbox replacements / unencoded payload drops | 472 / 472 | 279 / 279 |
| Pictures encoded / empty takes | 3,124 / 476 | 2,867 / 733 |
| Fresh fps | 52.06 | 47.78 |
| Worker timeout excess p95, ms | 15.10 | 15.24 |
| Worker spacing p95, ms | 30.93 | 30.75 |
| Decode-through-publication p95, ms | 1.790 | 2.877 |
| Native copy completion observed p95, ms | 1.624 | 2.635 |

Every checked decode fence was already complete before native submission in
these intervals. No pool-full event occurred. The decoder receives approximately
60 timestamped pictures/s and the active processing portions fit comfortably
inside 16.67 ms. The worker nevertheless samples the playhead unevenly: gaps near
31 ms followed by short catch-up intervals. Its **average wake count of 60/s is
not evidence of uniform delivery**. In one phase, almost all pictures become
payloads but arrive in bursts that overwrite the mailbox; in another, the PTS
selector supersedes a picture and later produces none. A one-picture boundary
imbalance is possible when a payload straddles the measurement markers.

### Timer precision and PTS alignment are separate

The CPU-only default → 1 ms request → default probes use the same installed
Rust standard library. For each condition and primitive there are 100 samples.
In the extended probe, a requested 16.667 ms wait has these p95 elapsed times:

| Primitive | Default before, ms | Scoped 1 ms request, ms | Default restored, ms |
| --- | ---: | ---: | ---: |
| `thread::sleep` | 17.183 | 17.183 | 17.169 |
| `mpsc::Receiver::recv_timeout` | 31.438 | 17.840 | 31.998 |
| `Condvar::wait_timeout` | 32.040 | 17.871 | 32.022 |

This directly measures coarse timed-wait behavior. It is not an inference from
CPU/GPU utilization. It also distinguishes the native copy's one-millisecond
polling sleep from the worker's channel timeout. The default playback traces'
copy sleeps have p95 approximately 1.53 ms, not 15–32 ms.

However, the 1 ms request is **not sufficient**:

| 60-second experiment | Fresh fps | Worker timeout excess p95, ms | Pre-copy selection skips | Mailbox replacements |
| --- | ---: | ---: | ---: | ---: |
| 1080p60, 1 ms request | 49.11 | 1.587 | 653 | 0 |
| 4K60, 1 ms request | 59.99 | 1.727 | 0 | 0 |

The failing 1080p run's `none` ticks sample the playhead a median **0.292 ms
before** the half-frame eligibility cutoff. Its superseding ticks land a median
**0.143 ms after** that cutoff. Alternating sides makes the selector return no
new picture on one tick and select two on another, discarding the earlier one.
All 3,600 FFmpeg pictures are still received; only 2,947 payloads are copied.
The passing 4K run's sampled phase is about 4.48 ms beyond that cutoff and stays
clear of it. Both have 3,600 worker wakes.

The playback epoch and worker cadence are independently anchored. Both use
`std::time::Instant`; this is not evidence of independent clock-rate drift.
`PlaybackState::last_advance` is sampled
before the initial paused picture is decoded; `set_playing(true)` only changes
the flag. The worker anchors its independent periodic schedule after that work.
The first worker playhead in the failing fine-timer run is already **40.404 ms**,
although startup requested playback from zero. That offset modulo a 16.667 ms
period, plus wake jitter, puts its steady samples near the half-frame cutoff.
The corresponding 4K startup offset is 45.003 ms. This is a measured startup
alignment difference, not an explanation established for the old laptop runs.

### Harness artifacts and polling

The default harness's intervals ending at tick indexes divisible by 30 have
median gaps **23.14 ms (1080p)** and **23.19 ms (4K)**; other gaps have medians
16.64 ms. Those ticks follow the synchronous CPU/RSS refresh **after** the
deadline sleep. The sample therefore delays the next tick instead of being
absorbed into its wait. Its service p95 excludes that delay. The historical
`late_render_ticks` check also precedes it and counts deadline overruns of the
service, not late starts; zero does not mean every render started on time.

Changing only the harness to nonblocking GPU polling yields **51.29 fresh fps**,
58.58 produced payloads/s and 522 empty ticks, despite reducing the reported
CPU-side service p95 to 0.29 ms. It does not repair delivery. Its completion
callback timings also include delay until the next poll, so they must not be
read as GPU execution times.

Independently phased deadlines plus a latest-only mailbox make consumer phase matter.
Requested 4/8/12 ms harness shifts delivered 47.82/54.15/51.82 fps in separate
30-second processes. Their producer phases differ too, so these are not a
controlled same-producer comparison. The offline sweep holds each observed
producer trace fixed: the default 1080p trace can supply about 59.3 fresh fps to
an ideal consumer at one phase, versus its actual 52.06. That is a counterfactual
model, not a measured fix. Neither picking a lucky phase nor changing counters
would qualify production pacing.

### Actual app behavior

The fresh release app ran through the public API, actual deck, compositor and
submission lifecycle, in separate newly created workspaces. It used its existing
hardware preference without changing the saved preference. The visible case
was launched normally and its playing Main preview was verified by screenshot
during warmup. No UI capture occurred during its measured interval. Seek to 5 s,
pause and resume also occurred in warmup, before the PTS-40 steady interval.

| Actual-app 60-second interval | Fresh fps | Pre-copy selection skips | Mailbox replacements | Consumer spacing p95, ms |
| --- | ---: | ---: | ---: | ---: |
| Headless, 1080p60, PTS ≥5 | 57.23 | 166 | 1 | 17.040 |
| Headless, 4K60, PTS ≥5 | 51.78 | 488 | 5 | 17.051 |
| Normal visible GUI, 1080p60, PTS ≥40 after controls | 52.63 | 307 | 136 | 30.609 |

Worker timeout excess p95 is 14.80–15.05 ms in the headless cases and 14.82 ms
in the visible case. Actual native copy observation p95 is 1.61–1.62 ms; every
checked decode fence is ready before submission. All sampled players remain
GPU-resident, with zero production picture transfers, no fallback or pool
backpressure and peak occupancy at most three. The headless consumer lacks
the benchmark's periodic 23 ms sampling gap. The visible engine's own consumer
cadence is nevertheless uneven; the source code's timed condition-variable
fallback and the standalone condition-variable probe warrant a targeted wait-span
check there when implementing the fix.

The visible interval records 3,599 UI present calls, with a p95 inter-call gap
of 30.79 ms; each call itself takes only 0.269 ms at p95. Approximately 60 present
calls/s can therefore reuse a source picture when no fresh one arrived.
These are API observations, **not physical display scanout timestamps or a
PTS-to-scanout measurement**. No external fullscreen/projection output was
qualified. The earlier hidden-launch GUI run is preserved separately, but its
foreground check failed and it is excluded from foreground claims. The visible
rerun occurred later after approval-review availability recovered; it is not a
controlled visibility-only A/B experiment.

Two final **untraced** app controls check that trace I/O is not creating the
real-app deficit. The `gpu_frames` conversion-encode counter was sampled after approximately
5 and 65 seconds in separate 70-second runs, with the same release app and fresh
workspaces:

| Untraced 1080p60 app | GPU-picture counter delta | Client elapsed, s | Payload rate estimate, fps |
| --- | ---: | ---: | ---: |
| Headless | 3,537 | 60.651 | 58.32 |
| Normal GUI launch | 3,010 | 60.586 | 49.68 |

The corresponding playhead deltas are 60.670 and 60.508 seconds, giving 58.30
and 49.74 encoded pictures/s. These are conversion-encode counter estimates with asynchronous
endpoints, not per-picture PTS traces or exact 60-second acceptance intervals.
They nevertheless confirm the substantial deficit without instrumentation.
Both retain zero picture transfers and backpressure, no fallback, and at most
three occupied slots. The untraced GUI control was normally launched without
additional screenshot capture; the earlier normal-launch traced case supplies
the warmup UI verification. Do not read traced/untraced differences from these
separate starts as a controlled estimate of tracing overhead or visibility effect.

### Smallest justified implementation plan

1. **Repair the harness independently.** Put synchronous CPU/RSS refresh before
   the deadline sleep, or collect it off the pacing thread. Record sample cost
   and tick-start lateness separately from historical service/late counters.
   Keep fresh/produced/empty and payload-discard definitions intact, retain a
   labelled historical mode for comparisons, and keep warmup boundaries explicit.
2. **Give playback and worker deadlines one coherent epoch.** Rebase the active
   wall-clock sample and worker schedule together on Play/resume and
   discontinuities; do not count paused first-picture decode time as playing
   time. Derive deadlines from the next needed PTS and output target mapped
   through the current playback clock/speed and output cap, rather than an
   unrelated spawn-time cadence. The current `PTS - half a source frame`
   eligibility boundary is not a proven preparation deadline: the current
   selector cannot copy a future picture before it becomes eligible. Compare
   retaining that policy with separating preparation from eligibility in step 4.
   An early command wake must not consume a future periodic deadline merely
   because `recv_timeout` returned. Coordinate with the next render deadline to
   leave native-copy lead time before consumption. This is substantive work,
   not an optional timing hint. Preserve wall-clock transport authority; do not
   force one frame per wake or advance position by frame counts.
3. **Use a precise, interruptible Windows deadline wait.** A per-worker
   [high-resolution waitable timer](https://learn.microsoft.com/windows/win32/api/synchapi/nf-synchapi-createwaitabletimerexw)
   plus command/stop notification can keep early seek/pause/stop wakes without
   a busy-spin or a permanent process-wide timer-resolution request. Audit every
   sender, including Drop, and avoid a reset/drain/wait lost-wakeup race. Retain
   suspended idle waits and a safe API-failure fallback. Instrument actual
   engine wait request/return, baton handoff and target-consumption deadline
   before choosing a consumer scheduling change. The standalone Condvar probe
   does not establish which span causes the GUI's long gaps. Leave display-
   clock-driven scheduling unchanged. A `timeBeginPeriod(1)` call alone is not
   the proposed production fix.
4. **Evaluate presentation ownership explicitly.** Established players prepare
   pictures ahead and select by timestamp at the output. Compare a small bounded
   ordered ready-picture handoff with deadline-coordinated latest-only delivery
   using recorded jitter and synthetic phases before a production refactor.
   Any ready queue must share the existing three GPU leases, retain seek
   generations and wait for completion before reuse. Three pool slots are not
   automatically three available presentation-queue entries. A queue cannot
   recover a display deadline the renderer itself missed.
5. **Prove the timing contract before merging.** Add synthetic-clock tests over
   the full frame phase with wake jitter around the cutoff; test costly paused
   preload followed by Play, seek generations, pause/step, resume, speed changes,
   VFR/output caps, loops and chase/modulation authority. Preserve pending-picture
   retries and three-slot completion lifetimes. Then repeat serial release
   1080p60/4K60 measurements across phases and real app controls. Require at least
   59.9 unique fresh pictures/s after warmup in idle 60-second 1x intervals with
   matched 60/1 source and 60/1 output targets; rate conversion, VFR and caps keep
   their own expected behavior. Preserve attribution by measuring accounting,
   wait and handoff changes separately before combined verification. Always
   report every residual gap, tick-start lateness and all original counters.
   Fresh fps alone cannot establish smooth presentation: inspect output cadence
   and obtain actual presentation timing where available. Rerun unchanged
   colour/transfer/pool/fallback tests plus a three-minute playlist soak.

Stage the narrow accounting/deadline changes first, but do not treat a worker-only
patch as a complete repair. A bounded timestamped handoff is a candidate, not a
measured fix; use phase/jitter evidence to choose the smallest passing policy.
Do not expand the pool, remove native fence waits, change colour thresholds, add
picture transfers or change defaults. Any handoff change needs its own
seek/preload/lifetime review.

### Completed verification and remaining limits

The frozen release test binary passed all 177 ordinary video regressions, six
picture/seek/pool/fallback groups (30 independent seek-picture comparisons at
the unchanged threshold), interlaced software fallback and a 180.0003-second
playlist soak. The soak delivered **9,167 GPU pictures**, performed **11 seeks**
and **one reload**, and showed all **six clips**. Sampled peak RSS was 380,772,352
bytes (363.1 MiB), ending at 351,584,256 bytes. This is stability evidence, not
a 60-fps pacing pass. All 14 CPU checks passed; the CPU timing check remains
ignored. No two-hour soak was run for this investigation.

Saved `performance.json`, `recent-shows.json`, `windows.json` and
`screen-names.json` retain their original hashes after the app runs. Existing
test/app executables were copied before replacement. No changes were pushed.
These checks covered the diagnostic build before production scheduling changes.
AMD/Intel and physical display scanout qualification remain deferred.

## Implementation on the pacing branch

The branch applies the timing changes first. It retains the latest-picture
mailbox, original PTS eligibility rule and existing three GPU leases. No ordered
presentation queue or extra preparation lead has been introduced. Whether a
handoff change is justified depends on actual phase-sensitive playback, not
solely on a replay against ideal consumers.

The implementation has four parts:

1. `PlaybackState::set_playing` resets the elapsed-time sample on a real
   Play/Pause transition. Paused first-picture decoding cannot age the new
   playing clock. Repeating Play while already playing preserves elapsed time.
2. For ordinary forward playback, the worker maps its next lookahead PTS to a
   monotonic eligibility deadline using the existing playback position, speed
   and half-frame rule. Suspended, reverse, chase, position modulation and
   output-capped cases retain periodic updates. Periodic scheduling consumes a
   slot at wake-up, so early commands preserve a pending deadline and decoding
   longer than a slot does not add another full interval of sleep.
3. Windows workers wait on a high-resolution waitable timer and an auto-reset
   command event. Sending a command or disconnecting signals the event; the
   receiver rechecks its queue and `Instant` deadline after each wake. API
   failure uses the standard channel timeout. The engine's timer fallback uses
   the same wait mechanism with stop/tick notifications, retaining its state
   lock checks and condition-variable fallback. Display-clock-driven scheduling
   is unchanged. There is no spin loop or permanent `timeBeginPeriod` request.
4. The offscreen harness performs synchronous process sampling before sleeping
   to its next deadline. It records sample cost and actual start lateness in
   additional fields, keeping historical service, late, fresh-picture and
   payload-discard counters intact. An explicit historical mode remains.

Pending native pictures use periodic retries until a pool lease becomes free.
The deadline query deliberately returns no PTS deadline in that state: retries
retain the current picture and do not advance the playhead, so an old deadline
would otherwise cause rapid polling. The pool qualification now exercises this
through the player as well as the converter, checking the same retained PTS
returns after releasing one of the three leases.

The cap check includes effective playback speed. A measured pre-guard 2x app
control copied and published 2,400 pictures over 20 seconds for a 60-fps output;
1,200 were overwritten. The guard keeps the existing periodic cap when either
the raw source rate or source rate multiplied by absolute effective speed
exceeds the output target. This avoids adding redundant copies at high speed;
it does not change the wall-clock playhead or intentional PTS skips.

The worker-only candidate was insufficient in the app: a 60-second steady GUI
interval delivered 55.63 distinct pictures/s, with consumer spacing p95 30.72 ms.
Engine wait spans then measured p95 14.81 ms of excess waiting; baton and GPU
handoff waits were negligible. With the engine timer correction, a separate
60-second interval delivered 60.00 distinct pictures/s, consumer spacing p95
17.05 ms and engine wait excess p95 0.50 ms. These are separate release runs
and CPU timing observations. Both GUI runs were obscured/unverified foreground
runs; they do not qualify physical output cadence.

Candidate history, final binary qualification and residual limits are recorded
separately from the diagnostic baseline above. CPU percentages are variable
between starts; a fresh-frame improvement does not by itself prove a CPU
reduction. The experimental status, defaults, shader, colour thresholds,
transfer counters and pool capacity remain unchanged.

### Attribution controls and phase limits

The third frozen candidate, before those two safety guards, delivered these
60-second 1080p controls with corrected harness sampling:

| Worker policy | Windows wait | Fresh fps |
| --- | --- | ---: |
| Periodic | Standard channel timeout | 48.47 |
| PTS eligibility deadline | Standard channel timeout | 57.38 |
| Periodic | High-resolution timer/event | 60.00 |
| PTS eligibility deadline | High-resolution timer/event | 59.98 |

The native periodic control already passes. These runs establish a substantial
wait-precision benefit; they do not establish that every part of the combined
gain comes from PTS-derived deadlines. The first worker-only candidate's harness
adapter accidentally omitted deadline forwarding, so its nominal PTS controls
were also periodic. That limitation is retained in candidate history rather
than treated as evidence for the deadline policy. The adapter regression now
checks forwarding explicitly.

The third candidate's 18 actual GUI seek-phase windows cover fractional seek
offsets 0 through 16 ms, in separate starts and eight-second steady intervals.
All have zero empty takes, repeated/backward PTS, pre-copy selection skips and
mailbox replacements. Counts are 480 or 481 distinct pictures per interval;
the one-picture boundary difference is not throughput above 60 fps. These short
windows are phase exploration, not 60-second acceptance runs for every phase.

An offline replay against independent, ideal 60-Hz consumers still predicts
bad phases for the observed publications. It omits OS wake correlation and
mailbox mutex blocking. The actual phase checks above have not reproduced
that prediction. Retain the model and the limitation: neither these passing
starts nor the replay proves arbitrary-phase presentation behavior. A bounded
timestamped handoff remains a follow-up if a real residual is reproduced.

### Later candidate: timing-only fix is incomplete

The [implementation evidence](qualification/rtx3060ti-pacing-fix-20261120/README.md)
records a later release build whose ordinary runs and 17 of 18 seek windows
pass, but whose remaining seek phase delivers **53 fps**. Its 57 empty takes
match 57 mailbox replacements, while worker and consumer spacing stay regular.
The next publication follows an empty take by at most 0.452 ms in that window.
This is an actual reproduction of the phase-sensitive handoff problem, not
only an offline model. The earlier passing phase starts do not establish safety.

The branch is therefore unfinished. Next measure a bounded readiness wait,
shared across the entire render frame, using native publication notification
and the precise timer. Compare it against the failing phase before introducing
an ordered queue. Do not add a separate full wait per deck. Retain the source
clock, paused CPU staging, seek generations and the existing three leases.
CPU costs also remain unresolved: the later harness medians are higher than
the diagnostic baseline, with wide variability; all raw readings are retained.

The next candidate implements that readiness window without adding a queue.
One `FrameReadiness` budget is shared through the mixer's video-update batch,
including nested playlists/slides. It starts on the first eligible empty take
and never restarts for another deck. Only matched-rate, ordinary forward GPU
playback may wait. CPU staging, paused/held pictures, chase/modulation and rate
conversion retain nonblocking consumption. Publication or seek invalidation
wakes a native event; generation changes cancel the wait. Unsupported native
waiting skips the optional window instead of using a coarse timeout. The
requested window is 1 ms; OS scheduling can exceed a requested deadline, so
actual wait duration and total waiting per frame are recorded.

A replay of the failing consumer starts and publications exactly reproduces
its 57 empty takes. A 0.5 ms window still leaves one empty; 1 ms recovers all
480 takes in the model, requiring up to 0.810 ms after consumption shifts.
This replay holds producer timing fixed and is not proof of the implementation.
The candidate's 27 CPU checks pass, including notification, seek cancellation,
parked-picture ownership, native failure and shared-budget expiry. Fresh release
app qualification, a 60-second fractional-seek hold and a finer phase sweep
remain necessary before calling the handoff fixed.

The [1 ms candidate's completed evidence](qualification/rtx3060ti-pacing-readiness-20261120/README.md)
records 628 recovered initial misses across 36 short seek windows, but one
remaining timeout. The next publication was observed about 1.281 ms after the
attempt. Its ordinary 60-second runs and long fractional-seek hold passed;
the remaining short-window loss is retained explicitly. The next frozen
candidate changes only the shared window to **1.5 ms**, adding margin for that
measured tail. Acceptance thresholds and counters remain unchanged, and a fresh
release qualification is required before declaring it complete.

The [1.5 ms candidate's evidence](qualification/rtx3060ti-pacing-readiness-margin-20261120/README.md)
passes all 36 ordinary seek-phase windows, but fails fast playback: a 60 fps
source at 2x delivers 55.85 fresh pictures/s over 20 seconds. The readiness
eligibility check incorrectly excluded this output-capped case. The worker
still produces 60 payloads/s; consumption needs the same bounded handoff.
The exporter originally checked its copy cap without checking freshness.
That missing freshness assertion has been added, and the unchanged records
were re-exported as a failed qualification.

The next frozen candidate extends readiness eligibility to playback whose
existing worker policy can supply the output rate. It retains the worker's
decode/copy caps and excludes legitimate holds, such as 30 fps at 4x under
the existing periodic policy. Its 28 CPU checks pass. Fresh release app
measurements deliver 60.00 fresh pictures/s at 2x over 20 seconds and 59.97
at ordinary 4K60 over 60 seconds. The 4K interval still contains three empty
takes and three replacements. These observations do not qualify physical
scanout or establish a CPU reduction.

That queue has now completed. All steady and speed checks pass, including
59.97 fps for 4K and 60.00 fps at 2x. The 36 short seek windows recover 676
initial misses but retain two empty takes and four replacements across three
windows; one window delivers 59.875 fps. Overall acceptance remains false.
The three-minute soak and targeted colour/seek/pool/fallback regressions pass.
Maximum observed readiness wait is 1.8971 ms, exceeding its 1.5 ms request.

The measured residual reinforces the limitation of deadline grace: one copy
polling sleep lasts 1.4863 ms and publication follows the empty take by 6.6 us.
The actual fence signal instant is unknown. The smallest justified next
experiment is native fence completion notification in the decoder, with the
same timeout, device-removal retirement and three-slot lifetime guarantees,
compared against the existing polling path. Do not keep enlarging grace or
declare arbitrary-phase behavior solved. The implementation and evidence remain
on the separate branch for review; no merge or push is performed.
