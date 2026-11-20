# Output-cadence readiness qualification

Status: measurements complete; overall qualification **fails** because three
short seek-phase windows retain losses. All 60-second freshness checks and the
20-second fast-playback checks pass. The release test harness and production app were rebuilt from
the same frozen source on `fix/windows-video-frame-pacing`, based on main
`a5b98f1fe93b9f1e4f82214fc0cc83ce63b2c1bd`.

## Change and measured cause

Late Windows timed waits caused missed decoder and engine deadlines. Precise,
interruptible native waits address that delay. A separate mailbox race remains
when consumption happens just before publication: the following publication
can replace an unconsumed picture. One optional 1.5 ms readiness window spans
the entire mixer video-update batch, including nested decks. Publication wakes
the consumer; seek invalidation cancels the old generation's wait.

Eligibility follows existing worker cadence, including a 60 fps source at 2x
whose payload production remains capped at 60/s. Legitimate rate-conversion
holds, paused CPU staging, reverse/chase/modulation and fallback behavior retain
their existing policy. No ordered picture queue or additional GPU slot is added.
The requested wait is not a hard scheduling bound: Windows can resume a waiting
thread after its deadline. Actual durations are recorded independently.

## Results

| Measurement | Fresh pictures/s | Interval |
| --- | ---: | ---: |
| Actual app, headless 1080p60 | approximately 60 | 60 s |
| Actual app, headless 4K60 | 59.97 | 60 s |
| Actual app, GUI after pause/seek | approximately 60 | 60 s |
| Actual app, fractional-seek hold | 60.00 | 60 s |
| Actual app, 60 fps source at 2x | 60.00 | 20 s |

An inclusive boundary gives 3,601 pictures in two 60-second app windows;
this is not throughput above 60 fps. The 4K app window contains three empty
takes and three replacements. The fast-playback window has zero empty takes
or replacements and 1,200 copies, preserving the output cap.

All 22 harness records are retained. At two deliberately shifted phases,
disabling readiness gives 54.73 and 56.63 fps; enabling it gives approximately
60 at both phases. All enabled 60-second harness cases, including three-clip
cases, meet the unchanged 59.9 fps threshold. Production picture transfers
and backpressure are zero. The three-clip cases did not exercise readiness
waiting, so they do not establish its worst-case multi-deck behavior.

Across 36 eight-second seek-phase windows, 676 initial misses are recovered.
Three windows retain a total of two empty takes and four replacements; their
counts must not be added as independent dropped-picture counts. One interval
delivers 59.875 fps. Maximum observed readiness wait is 1.8971 ms despite the
1.5 ms request. See `acceptance.json` for the failed overall result and
`trace-analyses.json` for every window, including passing and failing cases.

One residual is isolated in `residual-copy-poll.json`: the 1.5 ms readiness
wait expires, copy polling's sleep lasts 1.4863 ms, and publication follows
the empty take by 0.0066 ms. Actual GPU fence signal time is not measured.
The next narrow investigation should compare native D3D12 fence notification
against the existing 1 ms polling sleep while preserving timeout/device-removal
retirement ownership. Increasing grace again is not a guarantee against this
boundary. An ordered timestamped handoff remains the more structural option
if precise completion notification leaves a sustained phase problem. Neither
follow-up is implemented or claimed qualified in this candidate.

Validation: 28 CPU checks pass (one existing timing probe ignored); strict
Clippy and test compilation pass; release video regressions report 194 passed,
six ignored; six colour/seek/pool fixture groups pass 30 picture comparisons;
interlaced fallback and engine-stop interruption pass. The three-minute playlist
soak passes with 10,799 pictures, 11 seeks, one reload and six shown items.
Saved settings hashes and all frozen measured source hashes match. The shader
is the production shader, without the earlier recheck's shader override.

## Evidence and reproduction

Exact binaries, frozen source snapshots and full JSONL traces remain local in
`target/gpu-pacing-fix-20261120-readiness-cadence/`. Compact evidence was exported
after the serial queue completed. Prior failed candidates remain in their
own bundles; no counter or acceptance threshold is changed to hide failures.

The copied runners document the release build and serial measurements.
`build.ps1` builds the frozen candidate; `overnight.ps1` runs the qualification.
Run them from repository root after inspecting their local paths and choosing
a fresh output directory: traces refuse to overwrite existing evidence. Build
environment is provided by `.windows-tools/build-env.ps1`; dependencies are
resolved offline. Fixture paths and hashes, source/binary hashes, actual app
manifests, raw harness records, analyses and validation logs accompany the final
export. Re-running archived scripts requires updating their output paths.

## Limits

Measurements count distinct source PTS entering colour conversion/composition.
They do not establish physical scanout, unobstructed foreground playback, audio
synchronization, broadcast timing or arbitrary Windows load tolerance. GUI runs
are API driven and obscured. Async API counter estimates are separate from PTS
trace measurements. Three-clip measurements do not prove worst-case simultaneous
waiting when no readiness waits occur in those runs; shared-budget ownership is
also checked by CPU tests.

CPU readings vary substantially between starts. No CPU improvement is claimed.
GPU discard totals include warmup payloads and cannot be compared directly with
the CPU path's GPU-payload-discard counter. Colour thresholds, three-slot lifetime
safety, zero production picture transfers, experimental status and defaults stay
unchanged. NVIDIA qualification is sufficient for this investigation; AMD/Intel
qualification remains deferred. No push is authorized or performed.
