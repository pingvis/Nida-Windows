# Upstream 0.9.6 integration — 30 October 2026

Integrates upstream `0c9643c` (0.9.6 plus the capture-dot documentation) into
fork main `6ec8cfc`. The fork had already integrated 0.9.4; its newer Windows
PowerPoint, DirectX 12 default, NVENC output-recording and workflow changes
are retained.

## Combined behavior

- Include upstream's rundown improvements, input colours/buttons, output
  audio meters, quiet-input indication, optional NOT RECORDING reminder and
  lower control-window redraw rates when the show stands still.
- Preserve the fork's compressor/EQ integration, background effects, Properties
  layout, MIDI discovery/startup screen, logging, Windows capture/DeckLink,
  browser sources, PowerPoint COM driver, GPU decoding and NVENC recording.
- Count upstream playback seeks while keeping the fork's nearby-forward-seek
  optimization. Preserve playlist Auto Next, Program autoplay, filename display
  and current-clip transport while adopting compact transports and upstream's
  button feedback.
- Recording source and reminder can change while recording. A changed name
  applies to the next take; the active take keeps its name and other file
  settings remain locked. Independent output recorders retain their NVENC
  choices and unique take paths.
- Adapt EQ meters and the startup frame to upstream's revised UI interfaces.
  Add EQ state to upstream's new meter-test fixtures. Compile private macOS
  clicker/PDF/web helpers only on macOS or for their unit tests, rather than
  leaving unused backend functions in Windows production builds.
- Keep the fork's four-workflow CI/release arrangement. This integration does
  not publish an installer or change upstream PR branches.

The upstream global HID clicker and macOS privacy-dot behavior remain specific
to macOS. The existing Windows PowerPoint driver and foreground shortcuts are
separate retained Windows implementations.

## Validation

Native Windows checks use Rust 1.99.1, the existing FFmpeg 9/MSVC toolchain
and `--locked --offline --no-default-features --features
screen-capture,windows-browser`.

- Full serial library suite: **3,860 passed, zero failed, 16 ignored**.
  After the final PDF stub and private PowerPoint response-type cleanup,
  all **79 presentation helper tests** passed again.
- UI integration: **88 passed**. API documentation: **three passed**, with
  the combined OpenAPI schema regenerated.
- GPU-required integration groups, run serially: rundown **28**, playlist
  **18**, on-air **eight**, PowerPoint **14**, recording **nine**; all passed.
  These cover the retained Auto Next/Program autoplay behavior, current-clip
  control and preloading, fake presentation lifecycle, readable recordings,
  codecs/containers and persisted recording settings.
- Windows PowerShell 5.1 PowerPoint command/helper tests passed without Office.
  The real CEF helper rendered and closed its process tree successfully in
  its native Windows transport test.
- Four targeted audio-processing/playlist snapshots were regenerated for the
  combined layout. Processing and playlist controls were visually reviewed.
- The rebuilt browser-enabled executable passed a fresh-workspace GUI smoke
  test: startup frame submitted, initialization completed in about two
  seconds, graceful API shutdown, exit zero, no warnings or error records.
- Production library/binary Clippy with warnings denied passed. The full
  all-target check reports **12 test assertion-style diagnostics** in the
  inherited add-input, command-roundtrip, engine and rundown tests. The full
  formatting check still reports repository-wide differences. No lint
  suppression or repository-wide formatting rewrite was added.
- Workflow component-selection tests (**six**) and release-publishing helper
  tests (**14**) passed. These do not publish a release or validate an installer.

Physical Office, DeckLink and capture hardware checks were not repeated. The
earlier laptop NVENC throughput/quality measurements were not repeated and
are not measurements of this integration's performance. No installed app or
user show was replaced. Build/test logs are under `target/upstream-096-*`.
