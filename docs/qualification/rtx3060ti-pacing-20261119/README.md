# RTX 3060 Ti pacing evidence — 19 November 2026

See [the investigation and implementation plan](../../windows-video-frame-pacing.md).
This is diagnostic evidence, not a production scheduling fix or a wider-vendor
qualification. The experimental label, defaults, shader, colour thresholds,
three-slot pool and production transfer policy remain unchanged.

## Contents

- `benchmark-runs.json`: all 14 raw optimized offscreen records and manifests.
  Six controls are three interleaved 30-second runs per resolution. Eight traces
  cover defaults, a scoped timer request, nonblocking polling and requested phase
  shifts. Counts and failed-to-deliver ticks are retained, not redefined.
- `trace-analyses.json`: monotonic timing distributions, decoded/selected/copied/
  published/consumed counts, PTS-selection phase, mailbox replacement and payload
  disposal, sampler-aligned render gaps, and explicitly modelled phase sweeps.
- `trace-index.json`: full local trace paths, byte counts and SHA-256 values.
  Full traces and videos stay under ignored `target/`, not in Git.
- `trace-examples.json`: deterministically selected **first** measured empty tick,
  selection skip and overwrite per traced run, with all events within ±20 ms.
  These illustrate mechanics; the aggregate analyses use the entire interval.
- `app-runs.json`: actual release-app API setup, selected playback snapshots and
  60-second analyses. Headless starts at PTS 5; GUI after warmup seek/pause/resume
  starts at PTS 40. The hidden-launch run has unverified visibility and is excluded
  from foreground comparisons. Normal-launch playback was screenshot-verified
  during warmup; there was no UI automation during its measured interval.
- `untraced-app-controls.json`: two further 70-second actual-app runs with tracing
  disabled. Counter deltas over recorded client/playhead intervals confirm the
  deficit without trace I/O; counter endpoints are asynchronous, so these are
  rate estimates rather than per-picture PTS measurements.
- `perfect-producer-replay.json`: CPU-only follow-up model, 20 November, holding
  recorded GUI consumer times fixed while sweeping perfect 60-Hz producer phase.
  This is not a new playback measurement. Reproduction and interpretation are in
  [the scheduling research note](../../windows-video-frame-pacing-research.md).
- `timer-probe*.rs`, `.jsonl` and `*-summary.json`: optimized standalone standard-
  library wait probes, default → scoped 1 ms request → default. No GPU is created.
- `fixtures.json`: exact continuous fixture hashes, packet count/timestamp spacing
  and stream metadata. Both contain 7,200 pictures at 60/1 fps over 120 seconds.
- `diagnostic-binaries.json`, `diagnostic-source-hashes.json`, `host.json` and
  `artifact-provenance.json`: exact measured build and machine provenance.
- `prior-decomposition.json` and `prior-g14-decomposition.json`: arithmetic from
  the older raw records, separating produced-picture shortfall from net
  unconsumed pictures. These aggregates alone do not identify a timing cause.
- CPU, video regression, picture, interlaced and playlist logs: unchanged tests
  against the new embedded fixed shader, including the three-minute soak.
- `run-app.ps1`, `run-diagnostics.ps1`, `run-validation.ps1`: archived serial
  runners used on this PC. Their local fixture/build paths and completion-marker
  dependencies are intentional; use the generic commands below when adapting.

The local originals live at `target/gpu-pacing-20261119/`. That directory also
preserves pre-existing binaries, the excluded intermediate build, the exact
measured `source-snapshot/`, build/check logs, full app-state snapshots, all raw
traces and the evidence exporter. No source, binary or fixture was downloaded
from another machine for these measurements.

## Exact build distinction

The base is `a5b98f1fe93b9f1e4f82214fc0cc83ce63b2c1bd`, with the opt-in trace patch.
The first build overlapped editing between Cargo's library and test passes; it
was saved as `intermediate-unmeasured-test.exe` and **never used** for measurements.
The subsequent frozen-source release test and app builds were hash-checked.

After every measurement, a Clippy-only `match` → `if let` rewrite was made in
`publish_decoded`. It preserves the publication/recycling/stale paths. Final
production-library Clippy passes with warnings denied. The measured source
snapshot and its hashes refer to the pre-rewrite code; no new release build of
that stylistic edit is claimed. Do not confuse source hashes with binary hashes.

Both measured binaries embed shader
`D651DB90D51608CE57B498F2BBF827F8D0982925F28379DFE1601438A8AF3B4A`.
No shader override or release-profile override was used. The test's existing
self development dependency enables `test-fixtures`; the app does not.

## Reproduce

Use the repository's matching x64 MSVC/FFmpeg environment and release profile.
Build before timing, freeze sources across every Cargo pass, and run GPU work
serially when rendering/exporting/gaming is inactive. On this PC:

```powershell
. ./.windows-tools/build-env.ps1
cargo test --offline --release --no-default-features --features screen-capture,windows-browser --lib --no-run -j 1
cargo build --offline --release --no-default-features --features screen-capture,windows-browser -j 1
$test = '<release lib test executable printed by Cargo>'
$clip = 'target/gpu-qualification/fixtures/continuous/h264-1080p60.mp4'
Remove-Item Env:NIDA_TEST_GPU_VIDEO_SHADER,Env:VARDA_SOFTWARE_DECODE -ErrorAction SilentlyContinue
./scripts/measure-video-pacing.ps1 -TestBinary $test -Clip $clip -Output target/new-pacing.log -Seconds 60 -Trace
python scripts/analyze-video-pacing.py target/new-pacing.log.trace.jsonl > target/new-pacing-analysis.json
```

Repeat with a fresh output name and `-Timer1ms`, `-PollOnly`, or `-PhaseMs 4/8/12`.
Omit `-Trace` for controls; use the 4K fixture for the other resolution. The
wrapper restores environment values and records source/binary/fixture hashes.
Do not interpret the poll-only service metric as GPU-completion latency.

The exact original videos are retained locally with the recorded hashes. For
equivalent newly generated software fixtures, the adjacent
[portable generator](../rtx2060-maxq-20261119/01-generate-fixtures.ps1) includes
120-second H.264 1080p60/4K60 testsrc2 clips. Its
[verification script](../rtx2060-maxq-20261119/03-verify-fixtures.ps1) records probes
and hashes. Different FFmpeg/encoder builds and settings can change file hashes;
do not label regenerated files as the exact measured originals.

Run a timer probe from an x64 developer shell, with no other timed workload:

```powershell
rustc --edition 2021 -O docs/qualification/rtx3060ti-pacing-20261119/timer-probe-condvar.rs -o target/timer-probe-new.exe
./target/timer-probe-new.exe > target/timer-probe-new.jsonl
```

Each probe balances every `timeBeginPeriod(1)` with `timeEndPeriod(1)`. A successful
request is not a playback fix: the measured fine-timer 1080p run still skipped
653 pictures at the PTS eligibility boundary.

For app checks, create a new workspace, use a free API port, and explicitly take
the video channel to Main. The archived app runner verifies listener ownership
before commands/shutdown and checks residency, progress, zero transfers,
fallback absence and the three-slot limit. `-Visible` requests a normal GUI
launch; `-Headless` uses the actual headless app. `-NoTrace` writes timestamped
counter samples instead of a frame trace. Its `-WarmupControls` seeks to
5 s and pauses/resumes before the measured interval. Use a fresh `-Name`.

The GUI screenshot establishes a rendered playing preview during warmup, not
continuous foreground ownership or physical scanout pacing. Windows provides
no actual presented timestamps through this repository's current helper.
The report therefore separates source freshness, API presents and unmeasured
display scanout. NVIDIA results do not qualify AMD/Intel, and the old G14 AMD
D3D11VA/NVIDIA-renderer comparison still does not isolate identical adapters.
