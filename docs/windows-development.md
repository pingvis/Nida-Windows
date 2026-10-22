# Windows development

Windows support is being validated. Start with the existing shared engine and
the Windows screen/window capture backend; optional features can be enabled
after this baseline builds. This is a development setup, not a claim that the
Windows application is ready for a live show.

For an experimental portable ZIP and its runtime checks, see
[Windows packaging](windows-packaging.md).

## Build prerequisites

- Stable Rust for `x86_64-pc-windows-msvc`, including rustfmt and Clippy.
- Visual Studio C++ Build Tools with the Windows SDK. Use an x64 Developer
  PowerShell so the compiler and SDK are available.
- CMake on `PATH` (shaderc is built from source).
- libclang, with `LIBCLANG_PATH` pointing to the folder containing
  `libclang.dll`.
- An FFmpeg development distribution compatible with `ffmpeg-next` 9, including
  headers, MSVC import libraries, DLLs, `ffmpeg.exe`, and `ffprobe.exe`.
  An executable-only FFmpeg download is insufficient for linking the app.

Set paths for the current shell; substitute your own installation folders:

```powershell
$env:FFMPEG_DIR = 'C:\tools\ffmpeg'
$env:LIBCLANG_PATH = 'C:\tools\llvm\bin'
$env:PATH = "$env:FFMPEG_DIR\bin;$env:PATH"
```

`FFMPEG_DIR` must contain `include` and `lib`. Keep the runtime DLLs and tools
from that same distribution on `PATH`, so the linked libraries and subprocess
tools match. This setup uses `FFMPEG_DIR` directly rather than requiring FFmpeg
at the packaging script's default vcpkg location.

The optional `html` feature also builds Servo's native dependencies. With recent
MSVC headers, use LLVM/libclang 19 or newer; older versions fail while generating
`mozangle` bindings with `STL1000: Unexpected compiler version`.

## Build and run the baseline

```powershell
cargo check --locked --no-default-features --features screen-capture
cargo build --release --locked --no-default-features --features screen-capture
cargo run --release --locked --no-default-features --features screen-capture
```

Use `target\release\varda.exe` for interactive use and performance checks. The
unoptimized development build does not establish release performance.

Use the same feature selection for validation:

```powershell
cargo fmt --all -- --check
cargo clippy --locked --all-targets --no-default-features --features screen-capture -- -D warnings
cargo test --locked --no-default-features --features screen-capture --lib -- --test-threads=1
cargo test --locked --no-default-features --features screen-capture --test ui_integration -- --test-threads=1
```

Run GPU-heavy Windows tests serially: parallel runs on the tested RTX 3060 Ti
setup encountered native access violations. GPU tests need a working adapter;
golden image comparisons are not qualified by this baseline.

Recording and streaming integration tests also require working FFmpeg tools and
their codecs. Screen-capture smoke tests need an interactive Windows desktop;
automated compilation does not validate cameras, audio devices, or displays.

The optional `html` and `face-detection` features are excluded from the initial
baseline, not removed from the project. The core portable and installer profiles
exclude them; the Windows Clippy job separately selects them for compilation. The `depth` feature requires native libfreenect and is omitted on
Windows, as in the existing packaging and Clippy workflows.

## Porting conventions

Keep switching, rendering, audio mixing, show formats, UI, and API logic shared.
Put OS-specific calls behind existing platform interfaces. Validate Windows
changes alongside the existing macOS/Linux checks, and keep fixes focused so
they can be contributed upstream independently.

## Historical Windows validation

The original Windows compatibility commit (`1063649`) recorded these local
results with Rust 1.99.1, FFmpeg 9 shared libraries, and `screen-capture` on
Windows x64:

- Native development build and core application Clippy check passed.
- Seven focused FFmpeg lifecycle, tool-discovery, and progress-parser tests passed.
- All 87 `ui_integration` tests passed.
- The headless engine served its health/state API, recorded a five-second
  1920x1080 30 fps H.264 MP4 with an AAC track, and stopped cleanly. ffprobe
  confirmed the resulting streams and duration. Device capture was disabled
  during this smoke test.

These are earlier results, not a full Windows rerun of the focused revision.
The focused revision retains that compatibility code and adds portable path
fixtures and CRLF normalization in the API-documentation test. Its complete
Windows suite has not been rerun. A separate later Windows run reported four
unresolved streaming tests (reconnect timing and native live-rate control).

Repository-wide formatting checks report differences, and all-target Clippy
encounters `assert_is_empty` diagnostics with this Rust version. The full Linux
suite and the all-platform CI matrix remain unverified for this revision.
The original passing core lint command was:

```powershell
cargo clippy --locked --lib --bin varda --no-default-features --features screen-capture -- -D warnings
```

Real camera/audio capture, desktop capture, multi-monitor presentation, NDI,
optional features, and long-running stability still need validation. The
macOS-specific app-audio, hardware media, and Blackmagic implementations have
not been ported by these baseline fixes. Windows sleep prevention is not
implemented here. The reported secondary-display color mismatch is unresolved;
capture timing and resize/readback performance are separate work.

## Experimental Chromium browser sources

The `windows-browser` feature selects a separate CEF helper instead of Servo on
Windows. Build it with the exact SDK selected by `setup-windows-cef.ps1`:

```powershell
./scripts/ci/setup-windows-build.ps1 -ToolsDirectory C:\nida-build-tools
./scripts/ci/setup-windows-cef.ps1 -ToolsDirectory C:\nida-cef-tools
./scripts/ci/build-windows.ps1 -WithBrowser
```

For development without packaging:

To build and open the debug GUI with a transparent animated chat demo already
added, run this from x64 Developer PowerShell 7 (Rust, Visual Studio C++ tools,
CMake and Python are required):

```powershell
./scripts/run-windows-browser-debug.ps1
# Or test your actual overlay URL:
./scripts/run-windows-browser-debug.ps1 -Url 'https://your-overlay-url'
```

The launcher downloads missing SDKs on first use and leaves Nida open. It uses
`target/browser-debug-workspace` and port 8088, keeping normal saved shows
separate. Use `-SkipBuild` for subsequent launches with the SDK environment
still configured. The default demo is an inline HTML page running inside CEF,
titled **Inline HTML**. Double-click its deck, then select **Source** in Input
Settings. The full encoded URL scrolls inside its field and remains editable;
opening settings does not open an interactive CEF window. On narrow windows,
browser settings put controls below the preview automatically.

Use **Interact** in the preview's top-right corner, or double-click the **This
layer** preview in Input Settings, to operate a page directly, including cookie
consent. Either action brings an existing interaction window forward. It uses a
dark native title bar where supported, displays the existing CEF page and forwards
mouse, keyboard, wheel and IME events to it; it does not create a second
browser or change Program routing. Closing it leaves the source and cookies
intact. For a separate native debug instance, set `NIDA_BROWSER_DEBUG_PROFILE`
to a dedicated writable profile directory in that child process. Release
builds always use the normal persistent browser profile.

For manual build and transport checks:

```powershell
cargo build --release --locked --no-default-features --features screen-capture,windows-browser
./scripts/ci/build-windows-browser.ps1 -OutputDirectory target/release
cargo test --locked --manifest-path browser/checks/Cargo.toml --features windows-browser --lib
python scripts/ci/test-browser-host.py target/release/browser/nida-browser-host.exe
```

The helper must remain in `browser/` beside varda.exe, with its matching DLLs,
resources and locales. It resolves paths relative to the executable, not CWD.
No system-installed Chrome or Edge is used. `--no-html` disables browser startup.
Browser decks from existing shows remain as transparent placeholders when the
backend is unavailable; their settings and effect chains survive save/reopen.

The helper uses CEF's sandbox bootstrap. CEF 152 requires LPAC read/execute
access to its runtime directory, so setup and first use grant
`S-1-15-2-2:(OI)(CI)(RX)` only within `browser/`. This never grants access to
show files or browser profiles. A runtime directory that the user cannot update
will report an error; no elevation or sandbox-disabling fallback is attempted.

A Windows job object owns the Chromium process tree. Browser control and frame
pipes run on background workers, with bounded command queues and latest-frame
slots. A stalled pipe is terminated off the render thread; Refresh restarts a
failed host. The render loop only polls completed frames and uploads textures.

### Browser performance diagnostics

The CEF decoder is a small `nida-browser-protocol` dependency, built with
optimization even in debug mode. Its BGRA/alpha conversion touches every pixel;
unoptimized conversion can limit a 1080p browser to single-digit frame rates
while the show counter still reports 60 fps. The wire format and straight-alpha
output are unchanged. The rest of the application retains its normal debug
build settings.

To measure conversion and actual CEF-to-decoder throughput using a local
animated page and an isolated temporary profile:

```powershell
cargo run --manifest-path browser/protocol/Cargo.toml --example conversion
python scripts/ci/benchmark-browser-host.py target/debug/browser/nida-browser-host.exe --decoder browser/protocol/target/debug/examples/conversion.exe
cargo test --manifest-path browser/protocol/Cargo.toml
```

For an application run, set `NIDA_BROWSER_PERF=1` and enable info logging for
`varda::internal::html::cef_backend`. The host's decoded frame rate is logged
every five seconds (aggregated across browser sources, without URLs). A static
page only paints when changed, so use an animated page to assess throughput.
`GET /api/state/performance` also reports **browser interaction** under control
frame parts, separating interaction drawing from the decoder. These are
diagnostics, not a reason to change a show's resolution or browser frame limit.

The standalone checks compile the actual manager and transport without FFmpeg,
audio or capture dependencies. The host smoke test checks WebSocket-driven
pixels, transparent backgrounds, viewport resize without navigation, refresh,
profile storage and independent same-URL sources. These tests do not qualify
third-party chat providers or application GPU performance. Test actual overlay
URLs and a sustained live show on Windows before relying on this build.
