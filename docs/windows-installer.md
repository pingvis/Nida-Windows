# Windows installer files

The fork's **Windows installer (files only)** workflow builds the experimental
core portable payload, verifies it, and wraps it with Inno Setup 6.7.1. It
outputs a setup executable and SHA256 checksum as downloadable Actions artifacts.
It has read-only repository access and does not create releases, tags or branches.

## Download the setup file

1. Open **Actions → Windows installer (files only) → Run workflow**. The default
   source is `main`; no tag needs to be created.
2. Leave **version** empty to use `Cargo.toml`, or enter a setup label such as
   `0.9.1-beta.2`. A leading `v` is accepted. Its numeric version must match
   the app in `Cargo.toml`. This field labels the installer, not a Git ref.
3. When the run succeeds, download **Nida-Windows-installer** under **Artifacts**.
   Extract that download and run `Nida-<version>-windows-x64-Setup.exe`.

The artifact contains the setup executable and its `.sha256` file. The portable
ZIP is used internally to assemble the installer; use the separate portable
workflow if you want that ZIP instead. Artifacts are retained for 14 days.
Pull requests touching the installer also build and test these files.

The workflow verifies the portable payload, compiles Setup, and exercises
isolated test installers: installation, shortcut targets, clean-PATH payload/
encoder checks, upgrade, and uninstall with user files preserved. Test fixtures
are not included in the downloadable setup artifact.

## What the installer does

- Installs for the current user under `%LOCALAPPDATA%\Programs\Nida Windows`,
  without an administrator prompt.
- Creates a Start menu shortcut and offers an optional desktop shortcut. They
  launch from the user profile, preserving the existing default workspace.
- Registers an uninstaller in Windows Settings. Upgrades use the same AppId and
  installation folder. Close Nida first; Setup does not close a running show.
- Removes installer-owned files and shortcuts on uninstall, preserving personal
  files and separately stored shows and recordings.

The EXE and installer are unsigned. The core feature set and remaining device/
GPU validation limits are the same as [the portable package](windows-packaging.md).
The existing corresponding-source requirements still apply if these files are
later distributed publicly; see the packaging guide.

## Local build and test

Run from PowerShell 7 on Windows. First create a verified portable package using
the existing [Windows build instructions](windows-packaging.md), then:

```powershell
$compiler = ./scripts/ci/setup-inno.ps1 -ToolsDirectory .windows-tools/inno
./scripts/ci/build-windows-installer.ps1 -Archive C:\build\Nida-portable.zip -Compiler $compiler
```

The compiler is downloaded from its official immutable GitHub release, checked
against a pinned SHA256, and unpacked portably. No system-wide compiler install
is needed. `-ReleaseVersion 0.9.1-beta.1` supplies the installer display version;
its numeric part must match the portable payload's build manifest.

For an automated install test, compile two fixtures with `-TestIdentity` and
different suffixes, such as `0.9.1-installer-test.1` and `.2`, then pass their
paths to `test-windows-installer.ps1 -Installer ... -UpgradeInstaller ...`.
The test identity uses separate registration and shortcut names and an isolated
temporary install folder. The test refuses a production installer or an already
registered test installation. Test logs and user-data fixtures are retained.

The product identity and fork links currently belong to `pingvis/Nida-Windows`.
An upstream contribution can reuse the scripts but should choose its own stable
AppId, branding and release policy before distributing installers.

## Local validation — 19 October 2026

The production installer and two isolated test installers compiled with the
pinned compiler. The automated test passed initial installation, both shortcut
targets, a display-version upgrade which discovered the existing install folder,
replacement of installer-owned content, payload hashes and clean-PATH H.264/AAC
encode/probe/decode after both installs, and uninstall. Test registration,
shortcuts and application files were removed; a separately stored show fixture
and an unknown personal file inside the installation folder survived.

PowerShell syntax checks and actionlint passed for the new workflow and scripts.
GitHub-hosted execution and a clean-PC user test remain separate checks.
