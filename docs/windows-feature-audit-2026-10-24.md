# Windows feature audit — 24 October 2026

Later update: background MIDI discovery, the startup screen and Windows logs
are implemented. Upstream 0.9.2 is now integrated with the fork's additions;
see [the integration notes](windows-upstream-092.md). The review below records
the earlier 0.9.1 baseline and is retained as historical context.

Scope: read-only code/history review of fork main `e519c84`, current macOS
Nida main, and current original Varda main. No application or hardware tests
were run for this audit. Existing validation results below are historical.

## Repository baseline

- Original project: **Varda**, `im-knots/varda` (the project referred to as
  “valda”). GitHub main is `6e31c83d`, 14 October, following the 0.8.0 release.
- macOS Nida: `kristijonasatpro/Nida`, main `a312a184`, 20 October, version
  0.9.1. This exact commit is already an ancestor of fork main. There are no
  newer main-branch commits to merge as of this review.
- Windows fork: `pingvis/Nida-Windows`, local main `e519c84`. Recent work is
  concentrated on Windows packaging/CEF CI, browser sources, per-app audio,
  capture, and startup measurement.
- Version numbers belong to different products: Nida 0.9.1 does not mean it
  includes everything in Varda 0.8.0. Nida branched from Varda 0.5.0.

## Recommended next work

1. **Finish startup responsiveness.** The latest investigation measured a
   10.238-second synchronous MIDI discovery call before the UI became ready.
   Move discovery off the event thread, adopt its result later, preserve saved
   mappings while discovery is pending, and paint an initial startup view.
   Measure first paint and initialization separately. This is a measured stall
   from one initial launch, not a claim that every launch takes ten seconds.
   See [startup investigation](windows-startup-investigation.md) and
   `src/app/mod.rs:850`.
2. **Add native text inputs.** This is the strongest missing user-facing
   Varda feature for conferences: titles, speaker names, lower thirds and
   notices without a browser page or prepared image. First deliver font,
   size, colour, alignment, transparency, placement, UI/API editing and saved
   show restoration; captions/lyrics follow. The existing roadmap proposes
   one working Colour source-provider conversion before Text. Keep that slice
   small: completing every source/output migration is not required first.
3. **Add hardware encoding for REC on Windows.** STREAM's NVENC implementation
   does not accelerate REC. `StreamEncoder::resolved` explicitly maps NVENC
   to Software for the recording path. Start with NVIDIA H.264, measured CPU
   reduction and simultaneous REC/STREAM; then consider HEVC and Intel/AMD
   support. Preserve crash-safe containers, audio sync and software fallback.
4. **Implement camera stall/disconnect recovery.** The Mac camera worker
   detects stalled frames and reopens a device by stable identity. The Windows
   nokhwa loop retries `camera.frame()` on the existing handle with backoff;
   it does not contain the equivalent reopen loop. Test unplug/replug and
   driver stalls while preserving mode, colour settings and input identity.
5. **Add default persistent Windows logs.** Upstream's latest logging commit
   is merged, but its automatic log-file choice is Mac-only. Windows can use
   `VARDA_LOG_FILE`; it has no equivalent default file. Choose an appropriate
   per-user log directory, retain rotation, and expose an Open logs action.
6. **Investigate quitting during background shader builds.** Varda's
   `66627642` adds a bounded shutdown wait because active shader compilation
   could crash at process exit. Our effect loader uses a different bounded
   implementation, but discards worker JoinHandles and has no explicit drain.
   Reproduce and assess both effect and deck-load workers before adapting the
   fix. This is a code-level risk, not a reproduced Windows crash.
7. **Add Spout if local application interoperability is useful.** Varda has
   Windows source and sink providers using a D3D11On12 bridge and shared-memory
   sender discovery. Our fork has no Spout implementation. It could connect
   Windows visual/broadcast tools locally without an NDI encode path. Test
   actual sender/receiver pairs; do not import an entire architecture migration
   solely to obtain this feature.
8. **Profile Windows display timing before changing the backend.** The Mac
   display clock, presentation timestamps, minimum frame hold and scheduling
   priority are not implemented on Windows. Establish a real output cadence
   baseline first; then assess Windows timing primitives. A DX12 switch alone
   is not evidence of smoother output.

For a short next slice, choose startup plus logs. For a substantial new
feature, choose native text. For broadcast reliability, choose hardware REC
and camera recovery before experimental visual features.

## Varda features and fixes missing from this fork

| Area | Varda evidence | Fork assessment |
| --- | --- | --- |
| Native text, lyrics and captions | `aef79833`; `src/internal/text/` supports TXT, LRC, WebVTT and SubRip, static/crawl/ticker/step modes | No native Text source/module; timers already draw text but are not general text inputs |
| Spout source/output | `src/internal/spout/`, source and sink providers | No Spout module or source/output variants |
| Improved BPM and beat phase | `40451d4d`, `182449ea`, `caa37ddf`; kick-band onset analysis, autocorrelation, configurable tempo range | Still uses interval-history `estimate_bpm`; no `TempoRange`/`SetAudioTempoRange` |
| New effects and generators | October shader additions | 17 Varda `.fs` paths absent locally: boxinator, contour, cube_subdivision, curves, dither, droste, fractal_explorer, lens_streaks, light_rays, matte_tools, mosaic, paint, pixel_sort, polar, rutt_etra, shake, water_ripples. File absence does not prove every visual operation is unavailable through existing effects |
| Fractal Explorer | `2b0765d3` and later work; fractal module and shader | No corresponding module/shader; experimental and lower priority for conference workflows |
| HDR output/recording/streaming | Varda HDR math and presentation modes; README describes HDR10/HLG/EDR and HEVC/AV1 paths | Presentation depth currently SDR8/SDR10. HDR input tone mapping is already present and does not establish HDR output support |
| Source and sink providers | `330281e6`, `ab1a18ed`, `src/engine/value/provider.rs` | Legacy source enum remains; architectural reference rather than a feature to merge wholesale |
| Decoder/analyzer optimizations | `92cf2ee7`, `e45d0878` | Candidates for targeted profiling/adaptation. Fork lacks the provider-based DirectUpload and analyzer capture module; no claim of measured Windows benefit |

Do not list all Varda performance commits as missing: this fork already has
shader compilation caching, background interactive effect loading and
visibility-driven preview encoding. Nida's preview implementation also sizes
targets to the view and skips unchanged channel previews. Compare actual
behavior before importing overlapping work.

## macOS Nida behavior versus Windows

| Capability | Windows status and evidence |
| --- | --- |
| Hardware REC | Missing equivalent to VideoToolbox H.264/HEVC/ProRes recording. Windows streaming NVENC is separate; REC resolves it to software (`src/internal/streaming/mod.rs:84`, `src/internal/record/mod.rs:339`) |
| Hardware STREAM breadth | Partial: explicit experimental NVIDIA H.264 CBR only. HEVC hardware streaming, Intel QSV and AMD AMF are absent from that backend |
| Automatic network bitrate adaptation | Mac path supports it; Windows NVENC supports manual live bitrate changes but no automatic adaptation. See [NVENC validation](windows-nvenc-quality.md) |
| Hardware video decode | Implemented D3D11VA, experimental and opt-in on Windows; automatic on supported Macs. CPU download/conversion remains, so this is not zero-copy rendering |
| Camera reconnection | Mac worker reopens after stalls/disconnects; Windows retries frame reads on the existing camera handle (`src/internal/camera/mod.rs:1163`, `:1299`) |
| Refresh-clock pacing | Mac display-link clock; Windows `DisplayClock::start` returns None and uses the timer fallback (`src/usecases/ui/runner/vsync.rs:143`) |
| Presentation feedback/frame hold | Mac Metal hooks; Windows gets no presentation timestamps and frame-hold setter is a no-op (`src/internal/renderer/context.rs:172`) |
| Render/output scheduling priority | Mac QoS/real-time implementation; Windows helpers have no equivalent (`src/internal/renderer/presenter.rs:302`) |
| Native GPU utilization | Mac IOKit reading; `gpu_busy_now()` returns None on Windows (`src/internal/sysmon/mod.rs:120`). This does not imply all GPU timing/monitoring is absent |
| Automatic application log file | Mac app-bundle fallback only; Windows requires explicit log-file configuration (`src/app/log_file.rs:25`) |
| App sound | Implemented via WASAPI process loopback, requires build 20348+. Unlike the Mac tap it does not mute the app's ordinary playback. Nonzero signal isolation still needs controlled validation; see [app-audio notes](windows-app-audio.md) |
| Syphon interoperability | Mac-specific mechanism. Spout is the appropriate Windows counterpart to assess, rather than a direct Syphon port |

Core switching, overlays, buses, rundown, timers, routing, audio mixing and the
API are shared implementations. No blanket claim is made that those workflows
are broken on Windows. The isolated engine/output threads also already exist;
the timing gaps above do not mean outputs run entirely on the UI thread.

## Already implemented; qualification still matters

- Browser sources: separate Windows CEF helper and packaged browser payload.
- Per-app audio, chroma-key preview picker and display-capture exclusion:
  integrated into current main despite older checkpoint wording saying main
  was unchanged. Historical UI/audio verification limits still apply.
- DeckLink video and embedded stereo bus audio: implemented; current-package
  physical output, sustained sync and reconnect validation remain important.
- NVENC streaming: implemented and locally measured on an RTX 3060 Ti; those
  results do not certify every GPU, driver or external streaming service.
- Installer/portable packaging: merged. The older roadmap still has unchecked
  PR/merge tasks that recent history supersedes. Distinguish merged code,
  packaged output and clean-PC/manual validation.
- Keep-awake, GPU decode work, recording-output improvements, shader cache,
  background effects and the Properties layout are already integrated.

## Sources

- [Nida main](https://github.com/kristijonasatpro/Nida/commit/a312a18469fb0aa503fbaedac64672d8d127544c)
- [Varda main](https://github.com/im-knots/varda/tree/6e31c83d098e9a667fd1521356fb08c298351bf7)
- [Varda text implementation](https://github.com/im-knots/varda/tree/6e31c83d098e9a667fd1521356fb08c298351bf7/src/internal/text)
- [Varda Spout implementation](https://github.com/im-knots/varda/tree/6e31c83d098e9a667fd1521356fb08c298351bf7/src/internal/spout)
- [Varda BPM follow-up](https://github.com/im-knots/varda/commit/182449ea)
- [Varda final BPM scoring/tests](https://github.com/im-knots/varda/commit/caa37ddf)
- [Varda shutdown correction](https://github.com/im-knots/varda/commit/66627642)
- [Existing Windows roadmap](windows-roadmap.md), whose last stated update is
  20 October and whose completion statuses need reconciliation.
