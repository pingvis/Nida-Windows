# Windows integration of Nida 0.9.8

Upstream release `v0.9.8` (`a3b217b`) is merged into the Windows fork's
`main`, preserving the fork's existing history and fixes.

## Integration decisions

- Include upstream's direct-layer and merged-pass compositor, render-size
  hints, LED resizing fixes, asynchronous API media loading, encoder startup,
  file facts, logging, decoder teardown, audio-reader and screen-capture work.
- Keep Windows GPU decoding/encoding, browser sources, PowerPoint, capture,
  WASAPI per-app sound, DeckLink output, analyzer downsampling, beat analysis,
  background effects, MIDI discovery, playlists and responsive shutdown.
- Combine background recording startup and reserved paths with custom take
  names. Freeze the name for recovery files and reserve paths even when an
  encoder is stopped before it finishes starting.
- Track API media workers with the GPU context's build tickets. Track encoder
  startup through recording finalization so quitting cannot miss a cancelled
  start that subsequently creates a recording.
- Put Windows rotating file logs and stderr mirroring behind upstream's
  bounded logging queue; panic reports and exit draining use the same queue.
- Preserve the fork's input and bus FFmpeg EQ on the normal mixer path.
- Retain upstream's shader-cache implementation (already identical to the
  fork's) and text texture reuse, with the new redraw and stable-field tests.
- Keep the fork's FFmpeg 9 SDK selection and executable checks in CI.

Echo removal is included with upstream's platform support: its bundled AEC3
library runs on Apple silicon. Windows has no echo canceller in this release;
importing the new setting does not add a Windows AEC implementation.

## Validation

Windows production features: `--locked --offline --no-default-features
--features screen-capture,windows-browser`. GPU tests use `VARDA_REQUIRE_GPU=1`.

The all-target compiler check and production library/application Clippy pass
with warnings denied for the application. The Windows debug executable builds.
The vendored wgpu-hal build script still emits its existing macro warnings.

The affected integration suites pass **203 tests**, with two explicitly ignored
tests (NVIDIA hardware and a timing probe). These cover audio/video sync,
command roundtrips, direct layers, LED screens, background API media adds,
recording/recovery/custom names, text, UI and visible-part rendering. The
regenerated API reference passes another **three tests**.

Repository-wide formatting and all-target Clippy remain unclean: inherited
formatting differences and test lints, including empty assertions and a
Windows presentation fixture's raw string. Production Clippy is clean;
do not generalize that result to the tests. Resolved Rust files are formatted
and non-vendor whitespace checks pass. The upstream Abseil license is kept
verbatim, including its trailing blank line.

The full Windows library suite passes **4,272 tests**, with zero failures and
19 explicitly ignored tests. This includes the fork's existing Windows,
shader-cache, text reuse, EQ, playlist, NDI, analyzer and shutdown regressions.
The final focused recording/encoder-start/shutdown rerun passes **17 tests**
after the platform-stub lint cleanup; it overlaps the full-library count.

Logs are under `target/upstream-098-*` and are not committed.

Physical DeckLink/camera recovery, long live shows and macOS/Linux runtime
behavior are outside this local Windows qualification. Upstream's performance
figures are not measurements of the Windows fork. No installed application
is replaced and no release is published by this integration.
