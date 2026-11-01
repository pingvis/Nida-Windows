# Windows integration of Nida 0.9.7

Upstream `45e6f0b` (release 0.9.7) is integrated into the Windows fork's main.
The separate text follow-up is
[upstream draft PR #18](https://github.com/kristijonasatpro/Nida/pull/18),
commit `2daa1cf`: same-size text texture/binding reuse and transient pending/
error reporting in Input Settings and the API. Rendering errors retain the
previous picture; upstream's progressive fast typing, fades and bus targets
remain intact. Text settings and saved-show schemas are unchanged by that PR.

## Integration decisions

- Use upstream's native Text and Lower third implementations and schema.
  Our earlier prototype remains on `codex/native-text-inputs` at `b726d6d`;
  it is not merged into main. Its private review shows use a different text
  settings format and need conversion if reused with upstream Text.
- Retain safe shutdown/build tracking, GPU NDI receive conversion, analyzer
  downsampling, the beat-analysis worker and its tempo range, Windows browser
  sources, PowerPoint, capture, per-app sound and DeckLink output support.
- Combine upstream's asynchronous audio open/recovery and live-feed PCM
  gathering with the fork's off-callback beat analysis. Device callbacks tee
  PCM through upstream's gated gatherer; the analysis worker receives samples
  separately, sharing the live tempo-range control across asynchronous opens.
- Preserve the fork's existing FFmpeg equalizer/configuration on the normal
  mixer path alongside upstream's newly added native input processing. The
  upstream Live bus uses its native processing chain; legacy FFmpeg EQ is
  not a Live-callback processor.
- Keep upstream's new macOS Blackmagic input shim code behind macOS guards.
  Windows retains its current camera/capture and DeckLink output paths; this
  merge does not establish native DeckLink-input hardware support on Windows.
- Adapt the Windows native-stream writer to the new timing interface and
  skip-ahead placement. Windows recording retains its existing FFmpeg path.
- Gate the new Unix file-permission integration test to Unix. Other lower-third
  regressions run on Windows; the test is not replaced by a pretend ACL check.
- Regenerate the merged API reference. The earlier untracked investigation
  documents remain local and untracked.

## Validation

Windows flags: `--locked --offline --no-default-features --features
screen-capture,windows-browser`, with `VARDA_REQUIRE_GPU=1` for GPU tests.

The full library suite passed **4,213 tests, zero failures, 18 ignored**. This
includes the existing shutdown, analyzer, NDI and audio-analysis regressions.
The 12 focused text renderer/UI tests also pass, including the new texture
identity/replacement, empty text, error recovery and per-target status cases,
plus upstream's fast-typing and undo cases. Production library Clippy passes
with warnings denied; focused text formatting and whitespace checks pass.
The Windows debug application builds successfully.

The audio integration rerun passed **14 tests, one explicitly hardware-gated
test ignored**. An initial combined audio process ended with Windows native
access violation `0xc0000005`; the isolated sync test and full serial rerun
passed. The original native termination is retained as an unresolved harness/
runtime observation, not attributed to a proved cause or hidden as a pass.

The remaining integration checks passed: 3 API documentation, 51 command,
24 lower-third, 10 recording, 3 text-input and 88 UI tests. Together with
the audio rerun, this is **193 passing integration tests**, one ignored.
The combined current qualification is 4,213 passing library tests plus those
193 integration tests; the focused text runs overlap the library count.

Logs are under `target/upstream-097-*.log`; key files are `full-library`,
`text-focused`, `text-ui-focused`, `audio-rerun`, `integration`, `clippy`,
`debug-build` and `api-update`.

Upstream PR CI's formatting job reports 144 unchanged source paths, none of
the PR's source files. The same formatting failure was reproduced on an
unmodified upstream file; do not describe repository-wide formatting as clean.
Other upstream CI jobs may still be running when the local push completes.
The broader local all-target Clippy run also reports existing test/diagnostic
lints outside the focused text files (notably the new toolchain's empty-
assertion lint); it is not clean. A redundant field in a migrated fork audio
fixture was corrected. Production-library Clippy and the focused text files
are clean; do not generalize that to every repository target.

No installed application is replaced and no release is published by this work.
macOS/Linux runtime tests, long shows, physical DeckLink/camera recovery,
Audio Unit hosting and Windows low-latency hardware-bus measurements are not
qualified by these local checks. The release's Mac latency figures are not
Windows measurements.
