# Windows source-add latency

Investigation: 16 November 2026. Report: on another PC running 0.9.8, all
source types took roughly ten seconds to appear; browser and webcam were
examples. That PC's hardware, driver and phase timings are not yet available.

## Shared work

Every newly created source receives Crop and Transform effects. Previously,
`add_default_effects` called `Effect::new` synchronously twice on the engine
thread. This includes shader compilation, GPU pipeline construction and
resource setup. An API add could not finish, and the engine could not draw
its next frame, until those calls returned. The same implementation is
present in the 0.9.8 integration preceding the 0.9.9 merge.

This is a shared source-add stall, independently of browser navigation or
camera-driver startup. Image decoding, generator compilation and blit
pipeline creation are additional source-specific costs.

## Local measurements before the change

Current 0.9.9 code before this fix, browser-enabled **debug** executable,
RTX 3060 Ti, DX12, 1920x1080 show, scratch workspace. Local two-pixel BMP,
solid colour, `solid_color` generator and inline HTML; no external website.

| Measurement | First of its type | Following two |
| --- | ---: | ---: |
| Solid source API acknowledgement | 578 ms | 31 / 31 ms |
| Image API acknowledgement | 62 ms | 62 / 63 ms |
| Generator API acknowledgement | 297 ms | 47 / 31 ms |
| Browser API acknowledgement | 78 ms | 79 / 94 ms |
| Browser frame uploaded, from add request | 1,562 ms | 204 / 172 ms |

For the first solid, **571.1 ms** was default-effect construction: Crop
307.6 ms and Transform 263.3 ms. Deck textures took 0.2 ms. This directly
identifies the shared stall on this machine, but does not establish the full
cause of the other PC's ten seconds.

The first CEF transport startup spent 105 ms preparing runtime permissions;
the helper was spawned at 273 ms, and its first decoded frame arrived at
1,376 ms. Those are overlapping startup milestones, not times to add.
The browser tests follow the other source types, so their default effects
already benefit from warm shader compilation. These are exploratory local
measurements, not controlled cold-boot results or release/vMix comparisons.

## Patched observations

Two fresh processes using the same fixtures and debug profile, with no other
test process or compilation running:

| Measurement | Run A | Run B |
| --- | ---: | ---: |
| First solid API acknowledgement | 5.2 ms | 6.3 ms |
| First solid's defaults fully ready | 441 ms | 449 ms |
| First image API acknowledgement | 38.8 ms | 38.2 ms |
| First generator API acknowledgement | 1,199 ms | 308 ms |
| First browser API acknowledgement | 30.9 ms | 49.1 ms |
| First browser frame uploaded | 561 ms | 747 ms |

Default-effect admission itself takes about 0.3–1.1 ms in these runs, moving
the first source's observed half-second wait off the engine thread. This
does not eliminate compilation: the separate readiness column records it.
A regression deliberately blocks both default compilers and verifies that
the source still renders, then checks that early edits and UUIDs survive.

These runs also retain variability: one later image add took 349 ms, and the
first generator varied from 308 to 1,199 ms. Generator compilation, media
setup and driver work still deserve profiling. Browser first-paint variation
is not attributed to this patch, and disk/driver caches were not reset. No
general ten-second fix or vMix speed comparison is established by these data.

## Change and diagnostics

New-source default effects use the existing two-worker, bounded effect loader.
Their controls and identity settings exist immediately, in Crop/Transform
order; the source draws while their resources compile. Existing asynchronous
effect completion retains UUIDs, parameter edits, bypass and chain position.
Saving keeps settings rather than an in-flight job. Restored shows retain
their authored chains as before. If worker admission fails, direct construction
preserves both defaults instead of silently dropping controls; large bursts
that saturate the queue can therefore still block.

Debug diagnostics marked `[SOURCE LOAD]` separate default-effect admission,
blit pipeline construction, texture allocation, CEF permissions/process/first
decoded frame, camera format attempts, stream opening and first converted
camera picture. Adapter logs also identify device type and driver, helping
distinguish hardware rendering from an unexpected software adapter.

The Add Input dialog requests a synchronous camera rescan when its Camera
category first opens. Camera enumeration also queries formats by creating
backend camera handles. This is another possible UI stall, separately timed
as `Camera scan and format discovery`; it is not performed for every source
category. The API probe isolates engine adds and does not measure clicking
through the source picker or OS file dialogs.

Browser/camera backend delays remain distinct from source creation. Windows
camera opening runs in the background; its eight-second timeout is an upper
bound, not an intentional eight-second sleep for every source. Multiple
failed format attempts, driver negotiation or first-frame delivery need
actual hardware timings. No webcam is attached to this development PC.

## Reproduction on the affected PC

With a checkout and Python 3.11+, use the executable and matching browser/DLL
payload from the build being measured:

```powershell
python scripts/ci/benchmark-windows-source-load.py path/to/varda.exe --output target/source-load
# Also measure the first enumerated webcam, when attached:
python scripts/ci/benchmark-windows-source-load.py path/to/varda.exe --output target/source-load-camera --camera-id first
```

`--no-browser` measures the common solid/image/generator path without CEF.
`--backend dx12` or `--backend vulkan` explicitly selects a backend for a
separate comparison; the normal backend selection is unchanged.

The probe owns a fresh workspace and ephemeral API port, retains executable
SHA256, OS/GPU information, phase logs and JSON results, and uses a separate
CEF profile through `NIDA_BROWSER_PROFILE` (available in this build, including
release). Older debug builds support `NIDA_BROWSER_DEBUG_PROFILE`; older
release builds do not honor the new override, so use a separate Windows test
account or `--no-browser` when comparing those. No show or browser cookies
from an operator's running instance are used by the current probe/build.

`api_add_ms` measures command acknowledgement, not physical display latency.
`effects_ready_ms` separately tracks full default-effect readiness.
`backend_ready_ms` is a browser frame uploaded or camera picture reported
through the state snapshot, including polling delay. Solid/image/generator
physical first-pixel times are deliberately not inferred from an API reply.

## Validation

The production browser-enabled Windows debug executable builds, and production
library/application Clippy passes with warnings denied for application code.
Two effect-loader regressions pass, including the deliberately blocked default
compilers. Source-command roundtrips pass 51 tests, direct layers pass five,
and render correctness passes 75, including default-effect neutrality and
colour precision. The complete frame suite passes 33 tests with one explicit
measurement ignored: **166 distinct targeted tests pass** in total. The frame
fixtures now wait for explicit effect readiness
before asserting completed framing; their pixel assertions are unchanged.
That readiness wait is separate from the non-blocking first-picture regression.

Both patched native probe processes exit cleanly without application error or
panic records. No physical webcam or affected-PC run is claimed. Records are
under `target/source-load-before/`, `target/source-load-after/`,
`target/source-load-after-repeat/` and `target/source-latency-*`.
Changed sections/new modules are formatted; existing unrelated formatting
differences in the large camera/engine files and vendor build warnings remain.

## Free Windows coverage

- Standard [GitHub-hosted runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
  are free for public repositories and provide Windows Server 2022/2025 x64
  images, plus Windows 11 Arm runners. These cover OS/architecture and clean
  package checks; they do not provide a consumer GPU/webcam hardware matrix.
  Larger GPU runners are separately billed. The application currently ships
  x64, so an Arm host is not proof of a native Arm port.
- [Windows Sandbox](https://learn.microsoft.com/en-us/windows/security/application-security/application-isolation/windows-sandbox/)
  is included in supported Pro/Enterprise/Education editions, not Home. Its
  [configuration](https://learn.microsoft.com/en-us/windows/security/application-security/application-isolation/windows-sandbox/windows-sandbox-configure-using-wsb-file)
  supports memory limits, vGPU/WARP and opt-in webcam redirection. It is useful
  for clean-machine and constrained-memory checks, while sharing the host's
  hardware rather than reproducing another GPU vendor or camera driver.
- The [Azure new-customer offer](https://azure.microsoft.com/en-us/free/)
  includes $200 credit for 30 days and limited free VM allowances. Signup
  requires a payment card and eligibility. This is a limited trial/allowance,
  not an unlimited free GPU-device lab.

For the reported latency, collect the same probe on real Intel integrated,
AMD and NVIDIA machines, especially the affected PC. Use synthetic fixtures
for comparable runs, then separately check the real show and real websites.
Virtual machines alone cannot qualify physical USB negotiation, driver
startup, hybrid-GPU selection or sustained live capture.

## Follow-up performance audit (17 November 2026)

The follow-up uses the current checkout after the asynchronous-default fix.
It does not compare against 0.9.8 or establish the cause of the other PC's
ten-second delay. Layer-mask construction remains synchronous.

### Environment and comparison conditions

Local hardware is an AMD Ryzen 9 5950X and a discrete NVIDIA GeForce RTX
3060 Ti, driver `32.0.16.2124`, using DX12. The retained probe records report
`Windows-11-10.0.26300-SP0`. The application uses the release profile:
optimization level 3, thin LTO, one codegen unit, and
`--no-default-features --features screen-capture,windows-browser`.
Criterion binaries use the optimized bench profile, which retains debug
symbols; keep their results separate from the application's paced workflows.

The frozen application baseline is `target/release/varda-before-perf.exe`
(SHA256 `47cab0feb841a64e473eb2de784374bd3ea4c16659ab895c9228f9eaa4f5b391`).
Its matching benchmark executables are
`target/perf-audit/bin-before/presentation.exe` and
`target/perf-audit/bin-before/compositing.exe`. Preserve these binaries and
their matching FFmpeg DLL/CEF payload while rebuilding the changed code.

The initial baseline has three fresh headless processes under
`target/perf-audit/before-headless-1` through `before-headless-3`, each with
seven admissions per source type, plus one windowed process under
`before-windowed-flushed` with three steady samples per scene. Each sample
lasts at least six seconds and spans at least 310 published engine frames.
The earlier `before-windowed` run did not flush the UI history and is excluded;
`harness-smoke` is debug-only infrastructure validation and is also excluded.

A roughly four-hour pause before resuming the task separates that baseline
from the forthcoming changed build. Repeat the frozen baseline beside the
changed executable before drawing comparisons: retain
`paired-before-headless-1` through `paired-before-headless-3`,
`paired-before-windowed`, `after-headless-1` through `after-headless-3`, and
`after-windowed`. Alternate the execution order between pairs. Finish all
compilation first, and run only one application or GPU benchmark at a time.
Fresh processes do not clear OS/driver caches, change the GPU's power state
in a controlled way, or remove thermal/background-load drift.

### Execution paths reviewed

- Source commands construct a channel separately from its deck. The original
  probe timed only the deck command; `channel_add_ms` now records the other
  foreground cost. `Channel::new` builds Normal/Add/Screen blits and a
  composite pipeline. Image ingestion builds a blit; external-source decks
  build straight and premultiplied blits. Mutable uniforms belong to each
  instance even when the underlying shader is identical.
- GLSL-to-SPIR-V compilation already caches full source, name and stage in
  the process. A warm compiler cache does not avoid creating wgpu pipeline
  objects. Image/video file commands already use bounded background media
  loading. Video type detection and actual opening nevertheless probe a
  normal file twice; that is additional background work, not evidence of a
  shared ten-second foreground stall.
- The existing retained-picture path checks source identity, dimensions,
  framing parameters, effect readiness, modulation, masks and analyzers.
  Images, text and unchanged capture frames already participate. Solid
  colours previously cleared their texture every frame and prevented the
  containing channel from retaining its picture.
- The UI builds registry lists and sorts names while building snapshots.
  This is a plausible allocation/CPU cost for a large shader library, but
  has not been isolated by the headless measurements here.
- Undoable commands capture an in-memory scene/stage snapshot before the
  mutation; media completion does this when placing the result. This scales
  with the authored show. File saving already runs on its own worker, and
  live Main takes do not create undo snapshots. Large-show history cost is
  a profiling candidate, not a confirmed explanation from a scratch scene.
- Analyzer preprocessor textures are uploaded on every render even when
  their immutable latest snapshot has not changed. This is confirmed
  redundant work for analyzer scenes; its cost is not measured here.
- Normal video, camera and browser consumption already uses new frames and
  reusable buffers. Conventional video still crosses decode/conversion,
  mailbox and staging copies. Existing stage/realtime diagnostics in
  `scripts/measure-video.ps1` and `docs/windows-gpu-decoding-investigation.md`
  should precede an ownership or decoder rewrite.
- Two background effect workers share the live device. No explicit engine
  lock or GPU completion wait was found in their construction path; driver
  serialization is still possible. The submit gate serializes surface
  reconfiguration against submissions, rather than all worker construction
  against rendering. Queue saturation can still fall back to synchronous
  default construction, deliberately preserving controls.

### Scoped changes being measured

Blits for source pictures and channel compositing reuse immutable pipeline,
layout and sampler resources for the exact wgpu Instance, Device, target
format and blend state. Device equality alone is insufficient across separate
wgpu instances, whose numeric device IDs can coincide. The cache remains
owned by the GPU context and shared by its clones; construction runs outside
its short lookup lock. Each blit instance still owns its mutable uniform and
ring buffers, preventing one source's opacity, UV or presentation settings
from changing another source's draw. This removes repeated construction work;
it does not merely move compilation to another foreground/background thread.

Solid colours now participate in the existing retained-picture path. Their
colour and alpha bits, dimensions and framing/effect state determine whether
the previous pixels can be reused. Colour/framing edits, resource readiness
and texture replacement invalidate that picture. Animated effects, masks,
modulation and analyzers retain their existing rendering paths. The solid's
logical TIME and FRAMEINDEX continue advancing on each eligible render
request, including requests that reuse pixels, so inserting or enabling an
effect later sees the clock it saw before this optimization. Muted, off-air
and rate-skipped requests follow their existing clock behavior. Changes remain
local and uncommitted while measurement and behavioral validation proceed.

### Measurement definitions

The extended source probe records executable hash, build/cache labels,
backend, channel and deck admission, API-observed resource/backend readiness,
Main cuts between loaded sources, interval history and engine-process CPU per
frame. It can add a supplied local video fixture with `--video` and run steady
scenes with `--workflows`. The CEF child processes are excluded from the
engine-process CPU figure.

`--windowed` runs the same fixtures with a visible control UI and fresh default
layout, collecting the existing engine/control frame-part diagnostics. Keep
those results separate from headless runs and use the clock/rate reported in
each snapshot. The loading workflow keeps the active generator on Main while
40 sequential off-air solids build their default effects, with at most one
added source outstanding. It reports any quiet tail needed to fill the history;
it does not test queue saturation or isolate driver-internal contention.

Readiness is sampled from snapshots published every ten frames, so it is an
upper bound with publication and polling delay. A browser readiness signal
means its texture was uploaded; neither that nor acknowledgement means
physical first pixels. First video upload is not exposed by this API.
Video workflow validation checks that its player is running and its playhead
changes; it does not treat that as evidence of an uploaded or displayed frame.
Headless mode does not populate frame-part timers; its frame intervals include
timer pacing, and its CPU figure is whole-process work. GPU-completion-inclusive
Criterion renders measure a different quantity from paced frame intervals.
The pacing history spans 300 frames, rather than a fixed five seconds.
The probe flushes 310 engine frames before each steady scene. Windowed runs
also wait for 310 Main-monitor picks to replace the independently paced
control history; keep the control window visible. Those picks can include
rare abandoned window frames and are not physical-presentation timestamps.
Six-second sample windows extend when necessary to cover 310 engine frames.
Windows `GetProcessTimes` CPU totals are quantized on this machine, so small
differences in CPU milliseconds per frame need repeated windows and ranges.
Their nominal 100 ns storage units do not imply 100 ns measurement precision.

Cold-process results use a new process/workspace/CEF profile. The first
resource of each type is distinguished from later repetitions, but shared
startup resources and earlier source types can already be warm. OS page and
GPU driver disk caches are not reset; these are not cold-boot measurements.
The `source_pipeline_setup` Criterion group measures warm repeated
construction, including resource allocation/destruction and GPU completion
polls, with an already-decoded two-pixel image and a camera deck constructor
that never opens a physical camera.

### Exact follow-up reproduction

From this checkout, use the existing Windows development environment and the
same retained local fixture for every before/after run. The fixture is a
60-second 1920x1080, 30 fps H.264/YUV420P test pattern. Its retained SHA256 is
`c413981dc80dfae4b8a1ddc89ed6dd1e48db38d3f906754b135e319100aea47e`.
The command below records how it was generated; regenerate only when it is
missing, not between comparison runs. Different FFmpeg versions/mux metadata
can produce different file hashes.

```powershell
Set-Location -LiteralPath 'E:\Custom Apps\Nida-Windows'
. .\.windows-tools\build-env.ps1
if (-not (Test-Path -LiteralPath target/perf-audit-fixture.mp4)) {
    & "$env:FFMPEG_DIR/bin/ffmpeg.exe" -hide_banner -loglevel error `
      -f lavfi -i 'testsrc2=size=1920x1080:rate=30' -t 60 -an `
      -c:v libx264 -preset ultrafast -crf 28 -pix_fmt yuv420p `
      -y target/perf-audit-fixture.mp4
}
Get-FileHash -Algorithm SHA256 target/perf-audit-fixture.mp4
& "$env:FFMPEG_DIR/bin/ffprobe.exe" -v error -select_streams v:0 `
  -show_entries stream=codec_name,width,height,r_frame_rate,pix_fmt:format=duration `
  -of json target/perf-audit-fixture.mp4

# Build the changed executable only after the baseline has been preserved.
cargo build --release --locked --no-default-features `
  --features screen-capture,windows-browser --bin varda
Copy-Item -LiteralPath target/release/varda.exe `
  -Destination target/release/varda-after-perf.exe
cargo bench --profile release --no-run --locked --no-default-features `
  --features screen-capture,windows-browser --bench presentation --bench compositing
New-Item -ItemType Directory -Force target/perf-audit/bin-after | Out-Null
foreach ($name in @('presentation', 'compositing')) {
    $binary = Get-ChildItem -LiteralPath target/release/deps -Filter "$name-*.exe" |
      Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1
    Copy-Item -LiteralPath $binary.FullName -Destination "target/perf-audit/bin-after/$name.exe"
}
```

Keep both executables beside the same matching `target/release/browser`
payload and runtime DLLs. Run the same current probe against both binaries;
the probe records their actual hashes independently of the supplied labels.

```powershell
for ($pair = 1; $pair -le 3; $pair++) {
    $phases = if ($pair % 2) { @('before', 'after') } else { @('after', 'before') }
    foreach ($phase in $phases) {
        $prefix = if ($phase -eq 'before') { 'paired-before' } else { 'after' }
        python scripts/ci/benchmark-windows-source-load.py `
          "target/release/varda-$phase-perf.exe" `
          --output "target/perf-audit/$prefix-headless-$pair" `
          --backend dx12 --iterations 7 --workflows `
          --workflow-samples 3 --workflow-seconds 6 --switches 30 --loading-adds 40 `
          --video target/perf-audit-fixture.mp4 `
          --build-label "release opt3 thinLTO codegen-units1 screen-capture,windows-browser $phase"
    }
}
foreach ($phase in @('before', 'after')) {
    $prefix = if ($phase -eq 'before') { 'paired-before' } else { 'after' }
    python scripts/ci/benchmark-windows-source-load.py `
      "target/release/varda-$phase-perf.exe" `
      --output "target/perf-audit/$prefix-windowed" --windowed `
      --backend dx12 --iterations 3 --workflows `
      --workflow-samples 3 --workflow-seconds 6 --switches 30 --loading-adds 40 `
      --video target/perf-audit-fixture.mp4 `
      --build-label "release opt3 thinLTO codegen-units1 screen-capture,windows-browser $phase; UI history flushed"
}
```

For the isolated warm construction and idle/active render benchmarks, run the
preserved `bin-before` and newly copied `bin-after` executables separately:

```powershell
$env:WGPU_BACKEND = 'dx12'
$benchPass = 0
foreach ($phase in @('before', 'after', 'after', 'before')) {
    $benchPass++
    & "./target/perf-audit/bin-$phase/presentation.exe" --bench `
      source_pipeline_setup --warm-up-time 2 --measurement-time 5 `
      --save-baseline "perf-paired-$phase-$benchPass"
    & "./target/perf-audit/bin-$phase/compositing.exe" --bench `
      'channel_composite_(solid|shader)/decks/1$' --warm-up-time 2 --measurement-time 5 `
      --save-baseline "perf-paired-$phase-$benchPass"
}
```

The source-setup group uses 30 Criterion samples and solid/shader compositing
groups 50, after Criterion warm-up. Preserve raw
Criterion output and JSON estimates; do not compare those confidence intervals
as though they were distributions of API admission or frame presentation.
No physical webcam, DeckLink device, alternate GPU vendor or affected-PC
validation follows from these runs.

### Follow-up results

The strongest confirmed bottlenecks are repeated immutable blit construction
and redundant rendering of unchanged solids. Both optimizations remove work;
neither changes the bounded effect-loader policy or synchronous mask creation.

The clean adjacent warm benchmark passes (after pass 3, before pass 4) report
the following Criterion estimates and 95% confidence intervals. Construction
uses 30 samples; rendering uses 50. Earlier comparison passes overlapped final
validation activity and are excluded from these conclusions.

| Work | Before | After |
| --- | ---: | ---: |
| Decoded image constructor | 2.076 ms [2.048–2.111] | 0.219 ms [0.215–0.222] |
| External deck constructor, no physical camera | 3.293 ms [3.248–3.335] | 0.759 ms [0.066–2.137] |
| Channel constructor | 6.118 ms [5.983–6.279] | 1.347 ms [1.325–1.370] |
| Unchanged solid, GPU-completion-inclusive render | 450 µs [444–456] | 272 µs [269–277] |
| Active bars control, same render measurement | 511 µs [504–518] | 499 µs [493–505] |
| Solid-scene submissions per frame | 4 | 2 |

The external-constructor after run contains a large outlier; its broad interval
is retained rather than presenting its fastest samples as typical performance.
The generator control does not establish a general rendering-speed improvement.

Three paired fresh-process runs per version each added seven of every type.
The warm rows below exclude the first of each type: 18 samples per row.
Each process also measured 30 loaded-source cuts (90 per version), three
steady windows per scene (nine per version), and 40 sequential background
default builds while the generator remained on Main.

| API acknowledgement, median / p95 (ms) | Before | After |
| --- | ---: | ---: |
| Warm solid | 17.35 / 33.46 | 5.05 / 37.90 |
| Warm image | 33.93 / 50.67 | 21.64 / 54.83 |
| Warm generator | 18.54 / 34.69 | 6.19 / 22.25 |
| Warm browser | 21.18 / 37.89 | 5.21 / 21.51 |
| Warm H.264 video | 66.51 / 83.44 | 38.23 / 55.22 |
| Cut between loaded solids | 16.46 / 16.97 | 16.48 / 32.85 |

These are whole workflow observations, not equivalent CPU savings. Each deck
request follows channel creation, so their render-cadence phases are coupled.
The constructor benchmarks isolate eliminated setup work. In particular, a
solid's acknowledgement improvement is not evidence of 12 ms less work inside
its otherwise nearly unchanged constructor. Cut latency did not improve, and
several admission tails did not improve despite lower medians.

First generator admission remained expensive: 232–252 ms before and 212–230 ms
after (three first-of-type samples each), compared with much faster warm adds.
Its synchronous generator construction remains a foreground cost. First solid
defaults became API-observed ready in 279–415 ms before and 300–456 ms after:
this patch does not eliminate default-effect compilation. First browser frame
upload was observed in 631–1,325 ms before and 514–663 ms after; startup/cache
variability prevents attributing this difference to blit reuse.

In the separate windowed run, the solid mixer phase's mean ranged from
0.482–0.503 ms before to 0.118–0.125 ms after across three flushed histories.
Whole-process CPU/frame ranges overlapped: 1.13–3.08 versus 0.78–1.35 ms.
Headless solid CPU/frame medians were 0.95 versus 0.82 ms over nine windows,
also with overlapping ranges. Active-media whole-process costs remain variable:
windowed video 6.51–10.11 versus 5.03–9.11 ms/frame, and browser 2.39–6.60
versus 5.08–7.90. No active-media CPU improvement is claimed.

Both windowed versions showed roughly 31–32 ms worst intervals and late frames
in otherwise unchanged scenes. The idle optimization reduces mixer service
work but does not resolve that pacing behavior. The background-load workflow
is an unsaturated warm control, not proof that driver contention cannot occur.
UI registry rebuilding, redundant analyzer uploads, duplicate media probing,
large-show history cost and driver/internal loading contention remain separate
investigation candidates; none is claimed fixed here. The remote ten-second
delay and physical first-visible-pixel latency remain unestablished.

Raw records are in `target/perf-audit/`, with paired probe hashes/settings and
`summary.json`; warm benchmark logs are `target/perf-audit-paired-*.log`.

### Follow-up validation and limits

The release application and optimized benchmark executables build. DX12 GPU
checks pass for immutable-cache sharing with independent mutable buffers,
separate instance/device ownership, RGBA16F pixels, solid colour/alpha/framing
edits, resize, animation, modulation, UUID preservation and logical clocks.
The blocked-default regression confirms early edits survive compilation and
layer masks still construct synchronously.

Final integration results: command roundtrips 51, direct layers 5, frames 33
(one existing measurement ignored), Main program 4, Main transitions 40,
media adds 6, render correctness 78 and UI integration 88: **405 passed**.
Focused library results: cache isolation 2, FPS scheduling 14, default loading
and atomic masking 2, merge behavior 5 and capture sizing 1: **24 passed**.
Framing fixtures now wait for completed resources before classifying or
asserting completed framing; pixel/assertion expectations are unchanged and
the waits do not add a render of a newly created tap.

The separate Add Input suite passes six tests but retains a pre-existing
failure: its disabled-browser test expects text containing `no browser`, while
the unchanged producer returns `Browser rendering is disabled or its runtime
is missing`. This failure is not an optimization regression.

**The complete library suite is not verified.** A serial run stalled for more
than ten minutes during `GpuContext::new_headless` adapter selection. Native
stacks show release of an unselected DX12 adapter inside D3D12 debug-layer and
NVIDIA driver code, waiting on an SRW lock, before the new context's cache
initializes. The same OSC test passes alone in 1.10 seconds. A batched attempt
retained 336 passes, three framing-readiness failures subsequently corrected
and verified above, and one ignored test before another stall. Both attempts
were stopped after retaining logs/stacks; no validation layer or assertions
were disabled to manufacture a pass. Changed resource teardown timing has not
been ruled out as an influence on the native stall. Relevant diagnostic files:
`target/perf-audit/library-hang-stacks.log`, `library-batch-hang-stacks.log`,
`library-batches/results.json`, and `target/perf-audit-full-lib-tests.log`.

Production library/application, render-correctness tests and benchmark Clippy
checks pass with application warnings denied. Repository-wide Clippy reports
pre-existing `assert_is_empty` lints in `tests/audio_mix.rs`,
`tests/persistence_integration.rs`, `tests/slides.rs`; a focused run also finds
an unchanged assertion in `tests/command_roundtrip.rs`. Repository-wide
`cargo fmt --all -- --check` already failed before these fixes in numerous
unrelated sections. Changed Rust sections are formatted; `git diff --check`
passes. Vendor wgpu-hal build-script warnings remain pre-existing.

The investigation itself made no commits, pushes or upstream PR edits. At the
user's subsequent request, the shared changes were opened as independent
upstream drafts: [pipeline reuse (#19)](https://github.com/kristijonasatpro/Nida/pull/19)
and [solid-frame retention (#20)](https://github.com/kristijonasatpro/Nida/pull/20).
Both target upstream main directly; PR #5 is unchanged. The complete audit,
Windows diagnostics and integration fixes are retained on the fork's main.
Standalone upstream Windows checking encounters the existing DeckLink use of
`libc::clock_gettime` and `CLOCK_THREAD_CPUTIME_ID`, which are unavailable on
Windows; qualification of those upstream trees remains pending.

The next useful
measurements are the same isolated probe on the affected PC, including its
actual source-picker interaction and physical webcam, plus a dedicated trace
of the windowed pacing and native debug-driver stalls. OS/driver disk caches,
physical first pixels, other GPUs and real capture-device negotiation remain
outside this local qualification.
