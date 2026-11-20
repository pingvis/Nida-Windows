# RTX 3060 Ti pacing implementation evidence — incomplete fix

Base: `a5b98f1`; branch: `fix/windows-video-frame-pacing`. This is a freshly
built production app and release test binary, without a shader override.
Binary and selected source hashes are retained alongside this file. Exact
snapshots, binaries and full traces remain in
`target/gpu-pacing-fix-20261120-qualified/`.

**Overall result: not passed.** Precise worker/engine waits and playback clock
accounting repair the large scheduling gaps, but an actual seek phase exposes
the remaining latest-mailbox race. Do not treat ordinary passing starts as a
complete pacing fix. `acceptance.json` explicitly records this failure.

| Three untraced 60-second runs | Fresh fps median | CPU median, % of one core | Service p95 median, ms |
| --- | ---: | ---: | ---: |
| 1080p60 | 59.983 | 7.97 | 0.745 |
| 4K60 | 59.983 | 5.65 | 1.735 |

All 11 harness cases have zero production picture transfers and pool pressure.
CPU readings are higher than the diagnostic baseline and vary widely; no CPU
gain is claimed. The standalone wait probe records nonzero cycles despite
zero reported CPU milliseconds. It does not explain the video CPU variance.

Headless and GUI traced 60-second intervals deliver approximately 60 distinct
pictures/s, with no skips, replacements or empty takes. GUI consumer spacing
p95 is 17.031 ms; engine timer excess p95 is 0.500 ms. Untraced headless counters
give 59.84 pictures/s per client elapsed time and 59.93 per source playhead,
with four payload discards over 60.500 seconds. Untraced GUI gives 60.05 and
60.00 respectively, with no interval discards. These are asynchronous estimates;
see `app-counter-estimates.json`. Warmup/discard counters retain their definitions.

Of 18 eight-second seek-phase windows, `app-seek-phase-0-offset2` delivers
**53 fps**, with **57 empty takes and 57 replacements**, no pre-copy selection
skips, and normal worker/consumer spacing p95 around 17.05 ms. Pictures arrive
0.406 ms after empty takes at median, 0.439 ms at p95 and 0.452 ms maximum.
The earlier candidate's passing phases did not establish arbitrary-phase safety.

The 2x cap guard reduces native picture copies from 2,400 to 1,200 over 20
seconds, removing 1,200 replacements while retaining approximately 60 fresh fps.
Pending pictures keep periodic retries under pool pressure. Colour thresholds,
three-slot lifetime rules, fallbacks, shader and experimental defaults are unchanged.

Validation: 22 CPU checks pass (one existing timing check ignored), five cadence
phase checks pass, 188 video tests report pass (two optional HAP fixture checks
return early because the local birds fixture is absent), six picture/seek/pool
groups pass with 30 picture comparisons, and interlaced fallback passes. The
180.000-second soak delivers 10,789 GPU pictures, shows all six clips, performs
11 seeks and one reload, and peaks at 350,658,560 RSS bytes. Settings hashes match.

Next: test a small native publication-readiness wait with one shared budget per
render frame, using the failing phase before considering an ordered queue.
Preserve CPU staging, paused pictures, generations and the three leases. No
foreground or physical scanout qualification is claimed; GUI launches are API
driven and obscured/unverified. No push was performed.

Reproduction uses the archived PowerShell runners from the repository root with
the existing build environment and fixtures. Use fresh output/workspace names;
preserve existing evidence. `phase-contract.rs` is the exact input compiled from
the local target directory; its relative import assumes that directory depth.
Raw benchmark records, app samples, trace summaries, candidate history and local
trace hashes are included. Full traces can be reanalysed with
`scripts/analyze-video-pacing.py`; short seek windows remain exploratory rather
than 60-second acceptance intervals.
