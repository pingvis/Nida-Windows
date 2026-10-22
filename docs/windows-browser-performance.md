# Windows browser performance investigation — 2026-10-22

The slow debug browser was primarily limited by CPU pixel conversion, rather
than the interaction window's native decorations. Nida's 60 fps show counter
does not measure how often Chromium delivers a fresh browser frame.

## Measurements

Local animated inline HTML, 1920×1080 browser viewport, requested 30 fps,
60 fps show clock. No external website or network timing is included.

| Check | Before | Corrected debug build |
| --- | ---: | ---: |
| Actual CEF → pipe → decoder throughput, same current host | 8.33 fps | 30.32 fps |
| Browser throughput inside the application, after warm-up | — | 30.0 fps |
| Show rendering with interaction open | about 60 fps | about 60 fps |
| Interaction drawing, sampled means | not instrumented | 0.57–0.82 ms |

The final show sample reported 60.06 fps and zero late frames in its rolling
300-frame window. Interaction drawing peaked at 4.26 ms in that sample. This
is a short local test, not a sustained live-show qualification.

An initial direct decoder benchmark measured opaque 1080p conversion at
127 ms in unoptimized code; the corrected dependency benchmark measured
4.05 ms, and translucent conversion measured 8.06 ms. Those microbenchmarks
ran under different background compilation loads, so the same-host throughput
comparison above is the more useful result.

A full optimized comparison application also sustained approximately 60 fps
with the identical browser fixture and interaction open. Its main UI layout
averaged 0.53 ms versus 2.08 ms in debug. That comparison build predates the
final UI cleanup and decoder packaging; it establishes the cost of debug app
code, not a newly packaged release. No releases or tags were created.

## Changes

- Compile the existing, unchanged wire/alpha conversion through the small
  `nida-browser-protocol` dependency with optimization enabled in debug builds.
- Keep the pixel conversion non-generic and out of line. Otherwise the generic
  `Read` decoder can put the loop back into an unoptimized application instance.
- Add opt-in decoded-frame throughput logging and an interaction drawing part
  in the existing performance endpoint.
- Put **Interact** in the preview's top-right corner, retain the preview's
  double-click shortcut, and request a dark native interaction title bar.

Application testing also found a stale `target/debug/browser/nida-browser-host.dll`.
The CEF bootstrap EXE's hash does not identify the loaded client implementation:
the DLL must match too. The debug runtime now has the current DLL, and the
previously used debug executable paths have been updated to the corrected app.

## Verification and reproduction

52 preview/Input Settings tests, 12 browser manager/transport tests (including
the real Windows process/pipe test), and 2 protocol tests passed. The real CEF
smoke test passed cookie consent, persistent cookies, Unicode, shortcuts, IME,
wheel input, transparency, resize, refresh, independent sources and scoped
YouTube theme repair. The standalone manager test used the application's local
`wgpu-hal` patch and matching debug build settings to reuse offline dependencies.
That offline run used the application's cached dependency versions; its lockfile
is retained locally as `target/browser-checks-validated.Cargo.lock`. The checked-in
standalone lockfile keeps its existing version pins, adding only the two browser
dependencies, so unrelated dependency downgrades are not part of this change.

Native Windows checks confirmed a DOM button click, text `Nida ą你`, the visible
preview shortcut, and double-clicking the Input Settings preview bringing the
existing interaction window forward without losing page state. Long URLs,
narrow layouts and input switching were covered by the Input Settings tests;
those cases were not separately repeated through native automation this time.

The corrected debug application is left open in an isolated test show. User
show files were not reset. The separate optimized comparison instance was
closed after saving its test show.

See [Windows development](windows-development.md#browser-performance-diagnostics)
for repeatable conversion/CEF throughput commands. Raw local measurements are
under `target/browser-perf-*.json`, `target/browser-pipeline-*.json`, and the
test instance's `target/browser-perf-*/stderr-final.log`.

Third-party chat/site load time, heavy WebGL, multiple browser sources and
long-running show contention remain unmeasured. The source retains its authored
30 fps limit; fixing this delay does not make that source update at 60 fps.
