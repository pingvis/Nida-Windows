# Upstream 0.9.2 integration — 24 October 2026

This merge brings upstream `a0c8e864424b9038baf16336b1c2b4d2b56f30d6`
into the Windows fork. Both upstream and the fork's existing main remain in
the merge's history; Windows-specific functionality stays in this repository.

## Combined behavior

- Keep the Windows capture, browser, DeckLink, application audio, GPU decode,
  NVENC streaming/recording, packaging, Properties layout, background effects,
  playlist controls, NDI discovery and rotating logs.
- Keep background MIDI discovery, deferred saved mappings and the startup
  screen. Share upstream's atomic shutdown flag with the initialized app,
  including startup and headless shutdown paths.
- Adopt upstream's native compressor, low cut, leveler, bus limiter and LUFS
  readings. Keep the fork's optional parametric EQ, its controls, settings and
  pre/post EQ telemetry. EQ precedes source/bus processing and runs once per
  strip, independent of the number of sends.
- Follow upstream's processed pre-fader listen: headphones hear source EQ and
  processing before its fader and sync delay. The bus limiter adds 1.5 ms of
  fixed lookahead, including when bypassed. Existing shows receive upstream's
  default enabled bus limiter; EQ and source processing remain opt-in.
- Combine offline saved effects with asynchronous compilation. An effect is
  active only when enabled, loaded and not marked offline; saving preserves
  its original show path.
- Include upstream's presentation library, presenter UI and live-show fixes.
  Its actual PowerPoint driver remains macOS AppleScript; this merge does not
  add a native Windows PowerPoint driver. Night Shift management remains macOS
  specific too.
- Use Windows PATH separators (`;`) for shader-directory lists, preserving
  drive-letter paths and spaces. Other platforms retain `:`. Saved-show path
  checks compare serialized native paths, and recording tests inspect the
  fork's reserved take filenames instead of assuming overwrite destinations.

The MIDI/startup upstream draft remains a separate focused branch and PR.
This integration does not add Windows logs or other fork changes to that PR.

## Validation

Native Windows checks use the locked core build with `screen-capture` and
the existing FFmpeg/Visual Studio toolchain.

- Library coverage was split into serial groups after the initial parallel
  run stalled: 3,309 non-application tests passed, with one playlist assertion
  corrected and passed separately; 313 application tests passed. The new
  drive-letter regression and updated path-list test both passed separately.
  Fifteen manual/hardware tests remain ignored across the groups. The playlist
  assertion now checks Preview's actual controls: Program may independently
  have an empty-transport message.
- 88 UI interaction tests, three generated API-documentation checks, 127
  engine integration tests and 42 persistence integration tests passed.
- All ten show tests passed after the Windows shader-path fixes. Thirteen
  fake PowerPoint integration tests passed in the first run; the remaining
  placement test passed after using the platform title-bar height.
- Ten audio integration tests passed in the broad run. The reserved-recording
  filename check passed after reading the actual output path. The ProRes A/V
  sync check passed in isolation; its first run alongside other GPU suites
  captured too few flashes. No timing tolerance was widened.
- The two Processing and two playlist snapshots were regenerated for the
  combined layout and visually inspected. Playlist video controls remain.
- Startup/shutdown smoke: startup view, background MIDI discovery and API
  shutdown completed with exit code zero and no application error records.
  The final rebuilt executable also passed with two semicolon-separated
  absolute Windows shader paths (including spaces), zero warnings and zero
  errors.
- Production application Clippy with Windows browser support passed. The
  repository-wide all-target check still reports inherited test/style debt
  (115 library-test diagnostics in this run), and the full formatting check
  reports differences in existing files. No lint suppression or broad
  formatting pass was added.

These checks do not establish a successful parallel full-library run or
physical DeckLink/NDI/capture/audio-device validation.
