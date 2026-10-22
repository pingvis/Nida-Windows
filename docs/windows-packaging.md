# Experimental Windows portable package

The fork's **Windows portable (experimental)** Actions workflow builds the
combined Windows code in its selected branch and uploads a portable ZIP and
SHA256 checksum. It does not publish a Release or change either upstream PR.
Download the run's `Nida-Windows-x64-experimental` artifact, extract the outer
Actions download, then extract the inner application ZIP before running it.

The first package uses `--no-default-features --features screen-capture`, the
same core profile used for Windows validation. It includes `varda.exe`, the
matching FFmpeg 9 DLLs, `ffmpeg.exe`, `ffprobe.exe`, required app-local MSVC
runtime DLLs, shaders, dependency notices and a hashed build manifest. Optional
HTML/Servo, face detection and Kinect are not included. The NDI runtime is not
bundled; install it separately if needed.

## Build on Windows

Install Rust, Python 3.11+, CMake and Visual Studio C++ Build Tools. Open **x64
Developer PowerShell**, then use PowerShell 7 (`pwsh`):

```powershell
./scripts/ci/setup-windows-build.ps1 -ToolsDirectory C:\nida-build-tools
./scripts/ci/build-windows.ps1
```

Use a fresh tools directory. Setup downloads the FFmpeg 9 shared SDK, checks its
published SHA256, generates MSVC import libraries from those exact DLLs, and
sets `FFMPEG_DIR`, `LIBCLANG_PATH` and the build PATH in the current process.
It also records the input URL and hash. CI pins Rust 1.99.1 and libclang 20.1.2.

An existing matching FFmpeg SDK can be supplied with `-FfmpegRoot`. To package
an existing **core-profile** release build, use `-SkipBuild -Binary PATH` and
the SDK it was linked against. `-CrtDirectory` selects the MSVC x64 app-local
redistributable directory if it cannot be found through `VCToolsRedistDir`.
`-OutputDirectory` changes the default `dist/windows`. No version-specific
FFmpeg DLL names or fixed vcpkg location are assumed.

## Checks and limits

The build follows normal and delay-loaded PE imports, rejects non-x64 binaries,
refuses missing runtime DLLs, and keeps FFmpeg DLL resolution inside the chosen
SDK. It applies the existing Mac package's shader exclusion list.

The final ZIP is extracted to a path containing spaces, tested from an unrelated
working directory, and given a PATH containing only the package and Windows.
The smoke test checks every payload hash, application startup, FFmpeg/ffprobe,
and a one-second H.264/AAC encode/probe/decode. A failed check prevents the ZIP
from being published as a build artifact.

These checks do not prove GUI/GPU operation or an application recording works
on a clean user PC. Before calling it a supported release, test startup,
desktop/window capture, PGM output and recording on Windows without development
tools installed. The known streaming and secondary-display color issues remain.
The EXE is unsigned and this is a portable folder, not an installer.

CI artifacts are experimental evaluation builds. A public distribution must
also carry the exact corresponding FFmpeg/third-party source and build recipes,
following the existing source-archive policy in `scripts/ci/build-macos.sh`;
the recorded binary hash and a generic FFmpeg source URL are not a replacement.
This workflow deliberately does not publish a public Release.

## Windows installer

The separate [Inno Setup workflow](windows-installer.md) reuses
this verified ZIP as its payload. It adds a per-user installation, shortcuts,
upgrade/uninstall checks, and uploads Setup.exe and its checksum as Actions
artifacts. It does not create releases, tags or branches.

## Experimental browser-enabled package

`build-windows.ps1 -WithBrowser` builds with
`--no-default-features --features screen-capture,windows-browser` and adds the
sandboxed CEF helper under `browser/`. The ordinary core ZIP and installer
selection are unchanged. The **Windows browser sources (experimental)** workflow
uploads a browser-enabled evaluation ZIP; it never creates releases or tags.

CEF is pinned to `152.0.15+gb3d72fb+chromium-152.0.7977.169` and its SDK SHA256 in
`setup-windows-cef.ps1`. Headers, wrapper, bootstrap and runtime must be updated
together. The archive includes the exact SDK URL/hash, CEF license, version
header, resources, locales and hashed payload manifest. Browser binaries receive
the same import checks as the core application. Resource files are explicitly
copied because PE import scanning cannot discover them.

The extracted-package test runs a local WebSocket overlay through the real CEF
host with a clean PATH and unrelated CWD. Python is a build/test dependency,
not part of the runtime. Browser cookies and storage live under
`%LOCALAPPDATA%\Nida\BrowserCEF`, outside the portable folder and show files.
They are not included in ZIPs or installer payloads. Multiple Nida instances
cannot concurrently own this default profile.

CEF/Chromium adds substantial disk and memory use and requires ongoing security
updates. The included CEF license is not a complete replacement for Chromium's
third-party notices: public distribution must also carry the exact Chromium
credits and applicable sources/build recipes. Until those distribution materials
and native provider/performance checks are complete, this remains an experimental
CI artifact, not a supported public release. Installer browser payload validation
is not enabled by default.
