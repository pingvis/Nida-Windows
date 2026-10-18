# Windows installer and draft releases

The fork's **Windows installer and draft release** workflow builds the same
experimental core profile as the portable ZIP, then wraps that verified payload
with Inno Setup 6.7.1. It is separate from the upstream Mac release workflow.

## What the installer does

- Installs for the current user under `%LOCALAPPDATA%\Programs\Nida Windows`.
  No administrator prompt is requested.
- Creates a Start menu shortcut and offers an optional desktop shortcut. They
  launch from the user profile, so the application's existing default `.varda`
  workspace is used. Existing custom shows can be opened normally.
- Registers an uninstaller in Windows Settings. Upgrades use the same AppId and
  installation folder. Close Nida first; Setup does not automatically close a
  running show. Automatic updating is not included.
- Copies only the application payload. Uninstall removes installer-owned files
  and shortcuts; no recursive wipe of the application folder or user data is
  performed. Keep personal shaders in the workspace, not the program folder.

The EXE and installer are unsigned. The core feature set and remaining device/
GPU validation limits are the same as [the portable package](windows-packaging.md).

## First try it without creating a release

Once the workflow is on the fork's main branch:

1. Open **Actions → Windows installer and draft release → Run workflow**.
2. Select the branch and leave **tag** empty.
3. When the run succeeds, download **Nida-Windows-installer** under **Artifacts**.
4. Extract the Actions download. Run the file ending in `-Setup.exe`. The inner
   portable ZIP is an alternative; it does not need to be installed first.

Pull requests touching the installer also build this test artifact. They never
create releases and have no repository-write permission.

## Prepare a downloadable version

Choose a version from the fork's `Cargo.toml`, with a Windows beta suffix, for
example `windows-v0.9.0-beta.1`. Tags must identify a specific tested commit.
Windows tags start with `windows-v`, so the existing Mac workflow's `v*` trigger
does not run. For example, after merging and testing the installer PR:

```powershell
git switch main
git pull --ff-only origin main
git tag -a windows-v0.9.0-beta.1 -m "Windows installer beta 1"
git push origin windows-v0.9.0-beta.1
```

Pushing that tag starts the workflow automatically. Alternatively, run it by
hand with an **existing** Windows tag in the tag field. The tag must match the
Cargo version, optionally followed by a prerelease suffix.

The workflow builds and verifies the ZIP, compiles Setup, and exercises isolated
test installers: install, shortcut targets, clean-PATH payload/encoder checks,
upgrade to a second display version, and uninstall with user files preserved.
Only after these pass does a separate job with repository-write permission
create a **draft prerelease** and attach:

- `Nida-<version>-windows-x64-Setup.exe` and its `.sha256`;
- the portable ZIP and its `.sha256`.

Open the fork's **Releases** page to find the draft. It is not a public download
until published. Review the notes and test the installer on a clean Windows PC.
Before publishing, attach the exact corresponding FFmpeg/third-party source and
build recipes required by the existing [packaging policy](windows-packaging.md).
The automatic GitHub **Source code** archives contain Nida's source, not every
bundled dependency's corresponding source. The draft notes retain this reminder.
The workflow never publishes automatically and refuses to replace assets on an
already published release. Re-running a tag can update its existing draft assets.

## Local build and test

Run from PowerShell 7 on Windows. First create a verified portable package using
the existing [Windows build instructions](windows-packaging.md), then:

```powershell
$compiler = ./scripts/ci/setup-inno.ps1 -ToolsDirectory .windows-tools/inno
./scripts/ci/build-windows-installer.ps1 -Archive C:\build\Nida-portable.zip -Compiler $compiler
```

The compiler is downloaded from its official immutable GitHub release, checked
against a pinned SHA256, and unpacked portably. No system-wide compiler install
is needed. `-ReleaseVersion 0.9.0-beta.1` supplies the installer display version;
its numeric part must match the portable payload's build manifest.

For an automated install test, compile two fixtures with `-TestIdentity` and
different suffixes, such as `0.9.0-installer-test.1` and `.2`, then pass their
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
