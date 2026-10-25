# Windows portable and installer downloads

Use **Actions → Windows portable and installer → Run workflow**. Select the
source branch or tag in **Use workflow from**, then optionally enter a version
such as `v0.9.1-beta.4`. That one version labels both downloads. On a tag, the tag
is used automatically and a conflicting input is rejected. On a branch, an empty
input uses `Cargo.toml`. The numeric version must match Cargo; a leading `v` and
prerelease capitalization are normalized to the existing lowercase filenames.
Pushing a `v*` tag also runs this workflow against the tagged commit.

Download the run's `Nida-<version>-windows-x64` artifact. It contains:

- `Nida-<version>-windows-x64-portable.zip`
- `Nida-<version>-windows-x64-portable.zip.sha256`
- `Nida-<version>-windows-x64-Setup.exe`
- `Nida-<version>-windows-x64-Setup.exe.sha256`

Checksums contain the final asset filenames, so no renaming or checksum editing
is needed before uploading them to a release. Artifacts are retained for 14 days.
The workflow has read-only repository permissions: it builds files and does not
create releases or tags or upload release assets automatically.

The application and CEF helper are built once with
`screen-capture,windows-browser`. The verified portable ZIP is the exact payload
wrapped by Inno Setup 6.7.1. Both downloads include the browser runtime and the
same version, source revision, payload hashes and `nida.ico`. The core-only
portable distribution workflow is retired; the separate browser workflow now
runs development checks without producing a competing release package.

## Installer behavior

The installer installs for the current user under
`%LOCALAPPDATA%\Programs\Nida Windows`, without an administrator prompt. It creates
a Start menu shortcut and offers a desktop shortcut. Both retain the default
workspace under the user profile. Close Nida before upgrading: Setup does not
stop a running show.

Setup embeds `assets/nida.ico`; the payload, shortcuts and uninstall entry use
that same icon. It is a multi-size conversion of the existing `assets/icon.png`
artwork, with 16, 24, 32, 48, 64, 128 and 256-pixel images. The application window
continues using its existing PNG icon.

The AppId and install folder remain stable across upgrades. Uninstall removes
installer-owned files and shortcuts while preserving unknown personal files and
separately stored workspaces and recordings. Packages remain unsigned and
experimental; device/GPU qualification and the corresponding-source requirements
in [Windows packaging](windows-packaging.md) still apply.

## Local build

From PowerShell 7 in an x64 Developer PowerShell environment:

```powershell
./scripts/ci/setup-windows-build.ps1 -ToolsDirectory C:\nida-tools\build
./scripts/ci/setup-windows-cef.ps1 -ToolsDirectory C:\nida-tools\cef
./scripts/ci/build-windows.ps1 -WithBrowser -ReleaseVersion v0.9.1-beta.4
$compiler = ./scripts/ci/setup-inno.ps1 -ToolsDirectory C:\nida-tools\inno
./scripts/ci/build-windows-installer.ps1 -Archive dist/windows/Nida-0.9.1-beta.4-windows-x64-portable.zip -Compiler $compiler
```

Use fresh tools directories. The portable manifest stores the release label as
`version`, Cargo's application version as `package_version`, and the full source
commit as `revision`. The installer inherits the portable manifest's version; a
production override must match it exactly. Windows' numeric file version remains
based on the numeric application version.

The workflow retains the existing package smoke checks and isolated installer
install/upgrade/uninstall checks, and checks the installed icon references.
`test-windows-package-helpers.ps1` covers version normalization, tag/input
conflicts, numeric-version mismatches and final checksum filenames without
requiring a Rust build or the Windows SDK.

Installer test fixtures use `-TestIdentity` with a separate AppId, shortcuts and
temporary install folder; only these fixtures may use different prerelease
labels to exercise an upgrade. The automated test refuses production installers.
Test logs are retained, and user-data fixtures survive uninstall.

The combined workflow uses `build-windows-installer.ps1 -TestInstallUpgrade`
to extract and validate the portable payload once for all three installers.
The two disposable fixtures use ZIP compression; the downloadable installer
keeps LZMA2 compression. Installed and upgraded payloads are still independently
smoke-tested. The packaging workflow does not compile or run the application
library's GPU backend unit tests.
Artifact upload also avoids recompressing the already compressed ZIP and EXE.
