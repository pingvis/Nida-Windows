# Nida Windows roadmap

Updated: 19 October 2026.

This is the working checklist for incremental improvements to Nida Windows.
Each feature gets a focused branch and PR. Update this file when its status
changes, and add the PR link when one exists. Check a feature off after it is
merged and verified in the resulting Windows build.

## Baseline

- [x] Windows core portability fixes, capture performance work and experimental
  portable packaging are present in our fork's main at `9f2fd1f`.
- [x] Investigate capture freezing when a window is covered. Spotify reproduces
  in OBS too; investigation stopped. No fix or PR. Brave's black-video symptom
  remains inconclusive. Reopen only with new evidence or a production need.
- [ ] Verify DeckLink on the current packaged build: required modes, sustained
  playback, audio sync, and disconnect/reconnect. Record hardware, mode,
  duration and result; current support is not proof of hardware validation.
- [ ] Establish the broadcast regression baseline before architecture changes:
  existing switching, overlays, audio, rundown and saved-show tests, with any
  failures recorded separately.

## Planned work, in order

### Channel-effects Properties layout — local testing only

- [x] Cap the channel preview and allow a larger view on click.
- [x] Move routing into a collapsible sidebar and stack collapsible effect
  cards vertically; preserve controls, context menus and drag reordering.
- [x] Reveal and briefly highlight newly added channel effects.
- [x] Pass five focused layout tests, eight drag/drop tests and the production
  application Clippy check.
- [ ] Verify the integrated release build and user feedback.

No PR or push for this change until the user explicitly requests it.

### Windows installer — implementation and testing

- [x] Wrap the verified core portable package in a per-user Inno Setup installer.
- [x] Validate install, shortcut targets, version upgrade and uninstall while
  preserving user files; retain the existing payload and encoder smoke tests.
- [x] Add a Windows-specific workflow that uploads Setup, ZIP and checksums to
  a draft prerelease after checks pass, without triggering Mac releases.
- [ ] User tests Setup on Windows; merge the fork-only installer PR.
- [ ] Before public publishing, attach exact corresponding third-party sources
  and build recipes and verify a clean PC. See [release guide](windows-installer.md).

Fork PR: [Nida-Windows #2](https://github.com/pingvis/Nida-Windows/pull/2).

### Recording outputs fix — ready for review

- [x] Retain independent recording outputs and their existing source, audio,
  codec, rotation, surfaces and precision controls.
- [x] Adapt GPU NV12 delivery and bounded recording buffers to compatible
  H.264/HEVC outputs; preserve higher precision and alpha paths.
- [x] Reserve unique timestamped takes and show the actual path during/after
  recording, with file-manager and copy-path actions.
- [x] Validate simultaneous recorders, restarts and short synthetic release
  performance checks. See [test notes](recording-output-test.md).
- [x] User verified the initial separate Windows recording test build.
- [x] User verifies the follow-up pencil rename, folder chooser and top REC
  recording-name field before a PR is opened.
- [ ] Submit a focused PR, merge and verify the resulting package.

User approved opening the PR after testing the separate builds.

### 1. Shader compilation cache

- [x] Confirm the current compiler path and extract the relevant Varda cache
  design without importing unrelated changes.
- [x] Implement bounded, thread-safe reuse of duplicate shader compilations.
- [x] Preserve hot reload and error reporting; changed source or compiler
  options must not reuse a stale result.
- [x] Measure cold versus warm compilation and run relevant tests: 87 UI and
  73 GPU tests pass; the library suite passes 3,316 tests with two existing
  DeckLink failures and four ignored tests. See [validation](shader-cache.md).
- [x] Submit the cache-only upstream PR:
  [Nida #3](https://github.com/kristijonasatpro/Nida/pull/3) (draft).
- [ ] Upstream PR accepted and resulting portable release verified.

The tested Windows integration is maintained on our fork's main separately
from upstream review. Production library/benchmark lint checks pass; existing
repository-wide formatting and test lint failures remain outside this feature.

Done means duplicate loads reuse compilation with a measured benefit and
edited shaders still update correctly. This is independent of text inputs.

### 2. Background effect loading

- [x] Adapt Varda's worker pool and pending-effect construction to Nida.
- [x] Keep frames rendering while the first compilation/pipeline build runs.
- [x] Expose building/ready/failed status and preserve chain order, parameters,
  removal, undo and useful error messages while builds are pending.
- [x] Validate responsiveness and output continuity; submitted as draft
  [Nida #5](https://github.com/kristijonasatpro/Nida/pull/5). Interactive effect
  additions only; source swaps and transition loading remain separate slices.
- [ ] Verify background loading in the resulting Windows installer.

This follows caching: cache hits speed repeated loads, while background loading
addresses first-load stalls. It does not require a full source-provider rewrite.

### 3. Source-provider foundation for text

- [ ] Recheck the relevant Varda interfaces against current source.
- [ ] Convert one real source, Colour, end to end: creation, editing,
  rendering, UI/API, save and restore.
- [ ] Preserve Nida's broadcast behavior and existing saved-show format.
- [ ] Remove Colour's replaced runtime path and identify the remaining source
  conversions; avoid unused interfaces or permanent duplicate dispatch.
- [ ] Merge a foundation-only PR and verify the packaged build. PR: pending.

Done means the new interface actually runs Colour and round-trips existing
shows. A general output rewrite is outside this work.

### 4. Native text inputs

- [ ] Add editable text with font, size, colour, alignment and transparency.
- [ ] Integrate placement and Preview/Program operation into the broadcast desk.
- [ ] Support save/reload and API editing; verify UUIDs and routing survive.
- [ ] Keep text editing from triggering show-control keyboard shortcuts.
- [ ] Verify representative titles, multiline text and Lithuanian characters.
- [ ] Merge a text-only PR and verify the packaged build. PR: pending.

Done means an operator can create, edit, position, take and reload a title in
a real show. Subtitle/lyrics import is a separate follow-up.

### 5. Complete source-provider migration

- [ ] Convert images and generators in coherent PRs.
- [ ] Convert video and live sources while preserving clip audio, on-air
  playback, camera settings and reconnect behavior.
- [ ] Convert timers, playlists and nested inputs while preserving scheduling
  and transitions.
- [ ] Remove the temporary legacy dispatcher after all conversions pass.

Text can ship before this phase is complete. Track conversion PRs here as the
groups are confirmed during implementation.

## Later, when justified

- [ ] DX12 backend selection: first identify the active backend and a concrete
  failure or measurable improvement. Existing wgpu backend availability does
  not establish a need for a backend switch.
- [ ] Further desktop-capture performance: prioritize only if desktop capture
  becomes important to actual shows; use a release-build baseline.
- [ ] Subtitle/lyrics import using the text input implementation.
- [ ] Video decode/upload improvements, based on profiling.
- [ ] Output-provider migration or Spout, if a specific workflow requires it.

## Completion checklist for every feature PR

- [ ] One stated problem and scope; unrelated fixes and features split out.
- [ ] Relevant regression tests pass; required format/lint checks completed,
  with any pre-existing failures identified.
- [ ] Existing shows and broadcast behavior remain compatible, or a deliberate
  migration is documented and tested.
- [ ] User-visible behavior checked in the Windows build; performance claims
  backed by measurements where applicable.
- [ ] PR description records behavior, validation and remaining limitations.
- [ ] PR merged, resulting packaged build verified, and this roadmap updated
  with the PR link and outcome.

Varda implementations are references to assess and adapt, not a commitment to
merge Varda wholesale. Priorities can change when show requirements or measured
problems justify it.
