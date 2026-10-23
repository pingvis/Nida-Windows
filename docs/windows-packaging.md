# Windows portable package

The **Windows portable and installer** workflow builds both browser-enabled
Windows downloads from the same portable payload. See [Windows installer and
release files](windows-installer.md) for the shared version input, tag behavior,
artifact download and local commands.

For version `0.9.1-beta.4`, the portable files are
`Nida-0.9.1-beta.4-windows-x64-portable.zip` and the matching `.zip.sha256`.
The archive contains one directory with the same base name. Its checksum names
the final ZIP, not an intermediate commit-based filename. The version and source
revision are retained in `build-info.json`; no commit suffix is added to the
release filename.

The combined workflow uses `--no-default-features --features
screen-capture,windows-browser`. It includes `varda.exe`, `nida.ico`, the matching
FFmpeg 9 DLLs, `ffmpeg.exe`, `ffprobe.exe`, required app-local MSVC runtime DLLs,
shaders, dependency notices, the hashed build manifest, and the CEF runtime under
`browser/`. Optional Servo, face detection and Kinect are not enabled. The NDI
runtime is not bundled; install it separately if needed.

## Local packaging options

Follow the [combined build instructions](windows-installer.md#local-build) for
normal distribution packages. `build-windows.ps1 -ReleaseVersion` accepts the
same release labels as the workflow, with an optional leading `v`. Omitting the
label uses Cargo's version.

For development, the portable script still supports a core-only build by omitting
`-WithBrowser`. `-SkipBuild -Binary PATH` packages an existing matching binary;
`-FfmpegRoot` must identify the SDK it was linked against. `-CrtDirectory` selects
the MSVC x64 redistributable directory if discovery through `VCToolsRedistDir`
fails. `-OutputDirectory` changes the default `dist/windows`. The separate
**Windows browser checks (experimental)** workflow tests the helper and Rust
transport; distribution files come from the combined workflow.

## Existing checks and limits

Packaging follows normal and delay-loaded PE imports, rejects non-x64 binaries,
refuses missing runtime DLLs, keeps FFmpeg resolution within the chosen SDK and
applies the existing shader exclusion list.

The ZIP is extracted to a path with spaces, tested from an unrelated working
directory and given a PATH containing only the package and Windows. Existing
smoke checks verify payload hashes, `varda.exe --version`, the real CEF helper
when enabled, FFmpeg/ffprobe and H.264/AAC encode/probe/decode. These are not full
GUI/GPU startup or live-show qualification. The installer runs the same payload
checks after installation and upgrade.

CEF remains pinned to `152.0.15+gb3d72fb+chromium-152.0.7977.169` and its SDK SHA256
in `setup-windows-cef.ps1`. Headers, wrapper, bootstrap and runtime must be updated
together. The browser directory includes the exact SDK URL/hash, CEF license,
Chromium credits exported from that runtime, version header, resources, locales
and payload hashes. The app grants the narrow runtime ACL needed by the sandbox
at first browser use because ZIP extraction does not preserve ACLs.

Python is a build/test dependency, not a runtime dependency. Browser cookies and
storage remain under `%LOCALAPPDATA%\Nida\BrowserCEF`, outside the portable folder
and show files; they are not included in distribution packages. Multiple Nida
instances cannot concurrently own this default browser profile.

The application and installer remain unsigned and experimental. Test real
startup, capture, PGM output and recording on a Windows PC without development
tools before relying on a build. Public distribution still needs the exact
corresponding third-party source and build recipes under the existing policy in
`scripts/ci/build-macos.sh`; a generic source URL and a binary hash do not replace
those materials. This workflow produces Actions artifacts, not a public Release.
