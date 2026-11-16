# Smaller vMix features and original Varda port candidates

Reviewed 31 October 2026 against local Nida Windows `6a40079` and original
[`im-knots/varda` main `31133902`](https://github.com/im-knots/varda/tree/31133902cc1331563ba4d1cb02c85a46118be634),
dated 26 October. Varda's latest release is
[v0.9.0](https://github.com/im-knots/varda/releases/tag/v0.9.0).
Nida and Varda version numbers are independent.

This is a documentation and source audit, not a runtime benchmark or hardware
qualification. Missing below means no equivalent built-in workflow was found
in this checkout; a script or external controller can sometimes supply it.
Effort bands are relative implementation scope, not delivery promises.

## Smaller vMix features worth considering

| Feature and vMix evidence | Nida assessment | Suggested scope |
| --- | --- | --- |
| [Live Pause](https://www.vmix.com/help29/LivePause.html): hold a live input's picture while reception and audio continue. | No equivalent general live-input freeze command/control found. Clip pause is different. `shaders/freeze.fs` mixes the current source with a hold colour/noise; it does not store the captured picture. | Small to medium: retain one GPU picture at a defined point in the input pipeline. Define whether shared/nested uses freeze together and whether downstream effects continue animating. Audio continues. |
| [Low-volume alert](https://www.vmix.com/help29/Alerts.html): warn after the master stays below a threshold for five seconds, on operator displays only. | Meters, quiet/unrouted-source indication and a NOT RECORDING reminder already exist. No equivalent configurable sustained low-master-volume alert found. | Small: threshold, hold time and arm/acknowledge state from existing bus meters; keep warnings out of Program. |
| [Copy From](https://www.vmix.com/help29/CopyFrom.html): copy selected settings from another input. | Effects, decks and channels can already be copied. There is no selective paste-attributes operation onto an existing input while keeping its source and identity. | Small to medium: explicit choices for transform/crop, colour/key, effects and audio. Preserve destination UUID/routing and make it undoable. |
| [Triggers](https://www.vmix.com/help29/Triggers.html): transition, overlay, completion and playback-time events invoke actions, with optional delays. | Macros, rundown, clip-end auto-transitions and on-air clip behavior exist; a general per-input event/action editor was not found. | Medium: start with take-in, take-out and clip completion driving existing engine commands. Bound recursion and pending actions; cancel stale delayed actions when an input/show disappears. |
| [Phone tally](https://www.vmix.com/help29/WebControllerTallyLights.html): a browser shows whether a selected camera is in Preview or Program. | Nida has operator tallies and REST/WebSocket state, but no built-in phone tally page was found. | Small to medium: serve a read-only page using existing state; define tally for nested inputs, overlays, transitions and multiple buses. No new camera protocol is required for the first slice. |
| [Statistics window](https://www.vmix.com/help29/Statistics.html): source/renderer drops, resync, arrival timing, audio and encoder status in one place. | Nida already has performance API data, GPU/CPU popovers and output/encoder counters. The gap is an operator-facing per-source troubleshooting table and any missing measurements, not diagnostics from scratch. | Medium: show actual source cadence/frame age, renderer misses and encoder repeats separately, with explicit unavailable values. Do not copy vMix's drop classifications unless Nida measures equivalent causes. |
| [Preview Skip](https://www.vmix.com/help29/AdvancedSettings.html): update preview pictures less frequently than outputs. | Preview preparation is already visibility-driven, targets follow displayed size, unchanged channel previews are skipped, and the control window idles at lower rates. No independent low-rate moving-thumbnail control was found. | Small to medium: profile first; cap thumbnail encoding independently from Program/output and input decoding. Whole-window throttling can also affect meters and controls. |
| [Preset Bundle](https://www.vmix.com/help29/PresetBundle.html): package a show with its media. | Saved shows, presets, path repair and copied workspace folders are useful foundations; no collect-and-package-media command was found. | Medium: copy referenced assets and rewrite exported references without mutating the original. Handle filenames, nested playlists, presentations, LUTs and shaders; report external websites/devices/fonts that cannot be bundled. |
| [External MultiView](https://www.vmix.com/help29/SettingsOutputs.html): independently laid out monitoring feeds with labels and meters. | Nida's Broadcast UI, nested inputs and output routing can cover portions of this. No dedicated independent multiview output with automatic input labels/tallies/meters was found. | Medium to large: useful, but not a quick toggle. Keep it as an operator output, and prevent its generation from unnecessarily rendering every hidden input at full resolution. |
| [Stinger transition](https://www.vmix.com/help29/StingerTransitions.html): an animation with a configured cut point. | Nida has shader transitions and rundown-started clips/stings. A dedicated animation-backed Main transition with a cut-point control was not found. | Medium: preloaded alpha-capable animation, timed switch, audio policy and predictable abort behavior. A rundown sting is not automatically equivalent. |

The first three are the most contained operator improvements. Phone tally is
also attractive when camera operators need it. General triggers and collecting
show media have more edge cases than their UI suggests.

## Things that should not be listed as missing

- **Independent destination controls:** `src/app/state/streaming.rs::cmd_set_stream_destination`
  changes a destination live without touching the others. Nida shares one encode
  across destinations; independently encoded qualities/sources are a separate,
  larger streaming feature. See [vMix's controls](https://www.vmix.com/help29/IndependentStreamControl.html).
- **Auto play/pause/restart on air:** already in `src/engine/value/video.rs` and
  the on-air playback wiring. This is not general suspension of every off-air
  camera/NDI/browser worker.
- **Copy/duplicate effects and inputs:** already in `src/app/state/clipboard.rs`
  and `src/engine/types.rs`. Selective attributes remain a distinct improvement.
- **Clean feeds:** `src/internal/bus/mod.rs::Bus::hidden_layers` can leave
  selected layers out of the master feed for a bus. Do not build a second
  clean-feed system without comparing that existing behavior.
- **NOT RECORDING reminder and quiet-source indication:** included in the
  current 0.9.6 integration (`panels/record.rs`, `panels/audio_mixer.rs`).
- **Lower-resolution/slower decks:** `DeckRenderScale`, `DeckRenderFps` and
  adaptive drawing exist. These change source/deck work, unlike preview-only
  refresh limits.
- **Shader caching, background interactive effect loading and demand-driven
  previews:** implemented already; Varda's similarly named commits are not
  evidence they must be ported again.
- **Multicore software decoding:** `video/hwdecode.rs::open_with` already sets
  FFmpeg thread count to automatic, with frame/slice selection for clips/live
  streams. Varda's later decode commit also contains other changes worth
  investigating, but merely turning on threads would duplicate existing work.

## Original Varda: candidates confirmed against current source

### 1. Library-wide search — best contained usability port

[Commit `606501ad`](https://github.com/im-knots/varda/commit/606501ada5)
adds case-insensitive filtering across library sections, names and detail text,
opens matching sections, hides empty ones and restores expansion state when
cleared. Nida's `panels/library.rs` has no equivalent; Add Input's generator
search (`panels/add_input.rs`) searches only that chooser.

Adapt the search behavior and its tests to Nida's library data. Varda now uses
generic provider sections; copying its whole panel would lose or disturb
Nida-specific controls. This is the main new Varda feature since the prior
24 October audit.

### 2. Shader-build shutdown handling — reliability candidate to assess first

[Commit `66627642`](https://github.com/im-knots/varda/commit/666276428d)
stops accepting work, skips queued builds and waits a bounded time for active
compiler work before process exit. It addresses a reported exit crash while
shader compiler global state was being torn down.

Nida's `internal/deck/loading.rs::EffectLoader` uses two workers and a bounded
queue of 32. It discards their JoinHandles and exposes no explicit shutdown
drain. `app/render.rs` also starts background deck loads separately. Its
Shutdown command requests event-loop exit; it does not itself drain compilers.

Reproduce close-during-load first, then adapt lifecycle tracking to both worker
paths. Retain Nida's queue bound/cancellation, and define what happens if a
compiler exceeds the shutdown deadline. This is an unresolved source-level
risk, not a reproduced Nida Windows crash.

### 3. NDI receive conversion on the GPU — relevant performance adaptation

[Commit `8e94fcd5`](https://github.com/im-knots/varda/commit/8e94fcd5a4)
includes `internal/ndi/receive.rs` and `receive.wgsl`: transfer packed source
frames and expand UYVY on the GPU, with reusable frame handoff buffers.
Nida still calls CPU `uyvy_to_rgba` and performs CPU BGRA swizzling in
`internal/ndi/mod.rs` before uploading RGBA.

Adapt only the receive conversion/handoff. Nida's existing receive audio,
timestamps, transparency, colour behavior, reconnect and high-precision output
paths must survive. This does not turn NDI network decoding into a zero-copy
path. Benchmark with real NDI inputs before claiming a benefit.

The same Varda commit introduces a ping-pong `renderer/layers.rs` compositor
that avoids some snapshot copies before blending. Nida still has explicit
`Composite Snapshot Copy` branches in `channel/mod.rs` and mixer rendering.
This is another profiling candidate, but broad renderer surgery is lower
priority than the isolated NDI path. Preserve Nida's Merge, masks, occlusion,
colour pipeline and nested composition rather than replacing them wholesale.

### 4. Analyzer GPU downsampling — contained, workload-dependent performance port

[Commit `e45d0878`](https://github.com/im-knots/varda/commit/e45d08781e)
adds `analyzer/capture.rs`/`.wgsl`, sizes capture for analyzer needs and reduces
the frame on the GPU before readback. Nida's `analyzer/mod.rs::capture_frame`
creates a readback at the source texture's full dimensions.

Adapt the reduction pass and requested-size contract, retaining correct colour
encoding and coordinate mapping. Benefit applies when brightness/face or other
CPU frame analyzers are actually active; it is not a general camera scaling
fix or a guaranteed gain in ordinary switcher shows.

### 5. Modulator lookup indexing — smaller CPU optimization for complex shows

[Commit `85da48d5`](https://github.com/im-knots/varda/commit/85da48d53f)
indexes modulator-to-modulator inputs. Nida's
`modulation/engine.rs::get_mod_source_offset` still scans assignments for each
target parameter, using `mod:{uuid}:{param}` keys.

Adapt the index while keeping existing saved keys and UUID semantics. Invalidate
it on edits, removals and load. Profile large modulation shows; a basic
conference show without these assignments has little reason to prioritize it.

### 6. Improved beat detection — useful mainly for music-driven visuals

Varda's [BPM PR #161](https://github.com/im-knots/varda/pull/161), including
`40451d4d`, `182449ea` and `caa37ddf`, adds kick-band analysis, autocorrelation,
configurable tempo ranges and beat-phase/scoring corrections. Nida still uses
interval-history `estimate_bpm` and lacks the corresponding tempo-range command.

Port the complete corrected algorithm and tests if music synchronization is a
priority; this concerns the analysis/modulation engine, not the broadcast audio
mixer's compressor, EQ or LUFS system.

Follow-up upstream check, 31 October: also checked Kristijonas's published Nida
main, its recent commits, branch list and pull requests. Its current head is
[`0c9643c`](https://github.com/kristijonasatpro/Nida/commit/0c9643c0ce747d0f63ccffda62dc2bf537841848),
already an ancestor of this Windows checkout. The local audio-analysis, clock,
timebase and modulation-engine files have no diff against that upstream head.
Nida already has manual/MIDI/OSC/audio clock selection, beat-following
modulators, and phase/timebase handling. Those are not missing BPM features.

Kristijonas's recent
[`34fa288`](https://github.com/kristijonasatpro/Nida/commit/34fa2880b71133b99f83750798cc100128052521)
changes audio-device identity, retry/recovery and LTC reconnection, but does not
replace the interval-based BPM estimator. The Varda candidate above is narrowly
its October audio-to-tempo detection rewrite and configurable tempo range, not
a replacement of Nida's clock/timebase system. No newer published Nida BPM
rewrite was found in this check; an unpublished change cannot be assessed here.

### 7. Native text — highest-value larger feature

[Varda Text](https://github.com/im-knots/varda/tree/31133902cc1331563ba4d1cb02c85a46118be634/src/internal/text)
supports static, crawl, ticker and step modes, with TXT/LRC/SRT/WebVTT import.
It caches rasterized coverage masks and draws them on the GPU; it is not
literally an entirely GPU text-layout engine. Nida has timers but no general
native Text source/module.

Start with editable static titles and lower thirds, cached rendering, normal
placement, API editing and save/restore. Timed captions and scrolling modes
can follow. Adapt its renderer/layout modules to a working Nida source rather
than importing the entire provider migration as a prerequisite.

### 8. Spout — useful GPU interoperability reference, needs native qualification

[Varda Spout](https://github.com/im-knots/varda/tree/31133902cc1331563ba4d1cb02c85a46118be634/src/internal/spout)
has a D3D11On12 bridge on wgpu's DX12 device, using a GPU copy instead of CPU
readback. Nida has no corresponding module/source/output variants.

Assess if local exchange with another graphics app is needed. Reuse the
protocol/bridge ideas, but review acquire/release ownership, resource states,
queue serialization and sender synchronization against Nida's multithreaded
renderer. Its software-adapter probes are not proof of interoperability with
real external applications. It also does not automatically solve WGC capture,
video-decoder or NVENC handoffs.

### 9. Selective shader additions — inspect compatibility before copying

Comparing current `.fs` paths found 17 upstream shader files absent locally:
boxinator, contour, cube_subdivision, curves, dither, droste, fractal_explorer,
lens_streaks, light_rays, matte_tools, mosaic, paint, pixel_sort, polar,
rutt_etra, shake and water_ripples.

For conference production, prioritize
[Curves](https://github.com/im-knots/varda/blob/31133902cc1331563ba4d1cb02c85a46118be634/shaders/curves.fs)
and [Matte Tools](https://github.com/im-knots/varda/blob/31133902cc1331563ba4d1cb02c85a46118be634/shaders/matte_tools.fs)
over additional visual generators. Curves supplies master/channel tone controls;
Matte Tools adds alpha erosion/dilation/feathering and matte display. The latter
uses six passes and explicit intermediate formats; validate Nida's ISF bindings,
formats, colour/alpha conventions, shader licences and performance first.
Missing filenames do not mean Nida lacks every corresponding operation.

### 10. Decode direct upload — later experiment, not an automatic improvement

[Commit `92cf2ee7`](https://github.com/im-knots/varda/commit/92cf2ee7d9)
also moves CPU-decoded frame upload onto decoder workers through an armed
`DirectUpload` path. That is distinct from NVDEC/D3D11VA GPU-surface sharing.
Nida already has reusable staging uploads and hardware decode/fallback work;
worker-side writes could alter pacing and contention on its shared queue.
Compare both implementations with source cadence, frame age, seeks and several
clips before choosing. Preserve the current hardware decode and audio behavior.

## Suggested order

1. Assess close-during-build lifecycle; adapt a fix if the risk is confirmed.
2. Port library-wide search as a focused usability change.
3. Add low-audio alerts and real live-input freeze in separate changes.
4. Profile NDI receive conversion; adapt its GPU path if NDI matters to shows.
5. Port analyzer downsampling or modulation indexing when those workloads matter.
6. Deliver static native text, then selective attributes and simple event triggers.
7. Add phone tally, Spout or a dedicated multiview when the workflow calls for it.

Do not wholesale merge Varda main. Its provider, sink, parameter-address and
renderer changes overlap substantially with Nida-specific switching, audio,
presentation and Windows work. Candidate ports need focused tests and release
measurements. No code was ported, no installed app changed and no existing
roadmap completion boxes were rewritten by this audit.
