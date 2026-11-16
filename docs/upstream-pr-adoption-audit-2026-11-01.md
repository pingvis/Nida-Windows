# Nida PR adoption audit — 1 November 2026

> Historical audit through v0.9.6. Superseded by the [18-PR review against v0.9.7](upstream-pr-review-2026-11-01-v097.md). In particular, v0.9.7 now includes input EQ, although the full EQ scope of #11 remains outstanding.

Repository: `kristijonasatpro/Nida`; contributor: `pingvis`.
Checked all 17 PR discussions, current PR metadata, public main `0c9643c0ce747d0f63ccffda62dc2bf537841848`, and released tags v0.9.0–v0.9.6. All 17 PRs are open. Latest published release is v0.9.6 (28 October). Its `src` and `.github` trees are identical to current main; the later commit only changes documentation.

This audit concerns public upstream Nida, not the Windows fork, where many of these changes already exist. It is a source/release audit, not a fresh runtime qualification.

## Intake workflow

[Maintainer's explanation on PR #1](https://github.com/kristijonasatpro/Nida/pull/1#issuecomment-6297870074): the public repository is a squashed copy of an internal branch. Changes are taken internally, tested on Macs, then published in a release with contributor credit. The PR is closed with a release link rather than merged. Consequently, GitHub's unmerged status alone cannot establish whether a feature shipped.

A comment saying a change is being taken is evidence of intent, not evidence that it is in a published release. Internal, unpublished changes cannot be verified from the public repository.

## Result

**No current PR is fully verified as implemented in the public release.** The clear, credited adoption is part of #11 in v0.9.2. The newer EQ scope in that same PR has not shipped. Do not close an entire PR as implemented solely because one portion overlaps current Nida.

| PR | Feature | Public upstream finding | Closing recommendation |
| --- | --- | --- | --- |
| [#1](https://github.com/kristijonasatpro/Nida/pull/1) | Windows core / portable FFmpeg shutdown | Maintainer explicitly is not taking Windows support for now. The old placeholder-process recording shutdown and Windows DeckLink compilation blocker remain. | Not implemented. Could close as declined/out of current scope if desired, not as delivered. |
| [#2](https://github.com/kristijonasatpro/Nida/pull/2) | Windows capture timing / staging / CPU resize | Included in the maintainer's Windows-scope deferral. Current Windows readback still creates a staging texture inside each frame read, rather than retaining the proposed per-session texture. | Keep unless deliberately retiring the Windows proposal. |
| [#3](https://github.com/kristijonasatpro/Nida/pull/3) | Successful GLSL/SPIR-V compilation cache | Accepted as-is in comments, but the compiler source is unchanged across all seven released tags and still creates/executes shaderc on every call. | Keep open; acceptance has not become visible in the public release. |
| [#4](https://github.com/kristijonasatpro/Nida/pull/4) | Independent recording outputs / unique takes / naming | Accepted with a proposed shared atomic reservation fix. Current RecordOptions has no recording-name field; file_path still checks exists(), and the proposed take-path/name UI is absent. Existing top-bar NV12 does not establish adoption of the independent-output port. | Keep open. |
| [#5](https://github.com/kristijonasatpro/Nida/pull/5) | Background interactive effect builds | Accepted subject to keeping first-mask creation atomic. Current add_effect calls Effect::new/new_with_format synchronously; no EffectLoader, pending build state or loading-status API exists. | Keep open. |
| [#6](https://github.com/kristijonasatpro/Nida/pull/6) | Windows GPU decoding / forward seeking | Maintainer deferred Windows support and raised Mac-seek concerns. Your later reply restricts the new seek path to Windows; that is a PR revision, not upstream adoption. | Not implemented; possible scope-retirement closure only. |
| [#7](https://github.com/kristijonasatpro/Nida/pull/7) | Windows DeckLink / embedded audio | Maintainer deferred Windows support and raised scheduling/legacy-audio concerns. Your fixes are acknowledged in your reply, with no subsequent public adoption evidence. | Not implemented; possible scope-retirement closure only. |
| [#8](https://github.com/kristijonasatpro/Nida/pull/8) | Windows keep-awake | Maintainer deferred Windows support; your guard/cross-compilation follow-up is in the proposal. No released adoption established. | Not implemented; possible scope-retirement closure only. |
| [#9](https://github.com/kristijonasatpro/Nida/pull/9) | FFmpeg/ffprobe executables in Linux test CI | Maintainer said they would take it. The current workflow remains exactly the PR's pre-change blob: no ffmpeg apt package and no executable verification step. Development libraries and Mac build dependencies are not this fix. | Keep open. |
| [#10](https://github.com/kristijonasatpro/Nida/pull/10) | Vertical, discoverable Properties effect cards / async status | Accepted together with #5 and a header-status adjustment. The targeted Properties renderer still uses channel_fx_hscroll and horizontal_top. A separate vertical Effects tab already existed in v0.9.0, before this PR, and does not demonstrate adoption. Reveal/highlight and pending/failure-state behavior are absent. | Keep open; do not count the separate pre-existing tab as this PR landing. |
| [#11](https://github.com/kristijonasatpro/Nida/pull/11) | Expandable audio settings / compressor / calibrated meters / later EQ | **Partly released in v0.9.2 with explicit credit:** expandable settings/meters and a native-DSP compressor instead of FFmpeg, as the maintainer's comment allowed. EQ and its commands/panel are absent; the upstream processing plan still leaves EQ unchecked. | Keep the expanded PR open, or split remaining EQ into another PR and close the original adopted portion with a v0.9.2 link. |
| [#12](https://github.com/kristijonasatpro/Nida/pull/12) | Playlist Auto Next / Program autoplay / clip controls | Generic single-video controls/Auto Play exist, but PlaylistSettings has no auto_next or auto_play, and playback_snapshot handles only Video/HapVideo, not Playlist. The playlist-specific proposal is not implemented. | Keep open. |
| [#13](https://github.com/kristijonasatpro/Nida/pull/13) | Background MIDI enumeration / startup view | App construction still calls MidiDeviceManager::new synchronously. No pending MIDI discovery/startup-status implementation is present. Background audio/camera discovery and existing shutdown handling are different work. | Keep open. |
| [#14](https://github.com/kristijonasatpro/Nida/pull/14) | GPU analyzer downsampling | Current analyzer still creates full-size readback using source dimensions. Requested per-analyzer frame sizes and reduction capture module are absent. | Keep open. |
| [#15](https://github.com/kristijonasatpro/Nida/pull/15) | GPU NDI receive conversion | Receiver requests BGRX/BGRA mode 0 and runs convert_ndi_frame_to_rgba before upload. Packed GPU UYVY receive conversion is absent. | Keep open. |
| [#16](https://github.com/kristijonasatpro/Nida/pull/16) | Corrected audio tempo/phase / range / analysis worker | Current estimator uses 16 beat intervals. TempoRange, SetAudioTempoRange and the new worker/analysis module are absent. | Keep open. |
| [#17](https://github.com/kristijonasatpro/Nida/pull/17) | Safe background-build / recording / late-engine shutdown | The old two-second engine stop/detach remains. There is no Builds tracker, closing-build poll or recording-finalizer tracking. Existing Ctrl-C/API shutdown support does not cover this proposal. | Keep open. |

## Strongest positive evidence: #11

- [v0.9.2 release](https://github.com/kristijonasatpro/Nida/releases/tag/v0.9.2) explicitly credits the expandable settings and calibrated meters from Augustas's PR #11.
- [Implementation commit 34fa288](https://github.com/kristijonasatpro/Nida/commit/34fa288) includes Augustas as co-author and describes the adopted UI/meters with Nida's own DSP.
- [Expandable settings source](https://github.com/kristijonasatpro/Nida/blob/v0.9.2/src/usecases/ui/panels/audio_mixer.rs) and [native compressor implementation](https://github.com/kristijonasatpro/Nida/blob/v0.9.2/src/internal/audio_mix/dsp.rs) are in that release.
- [Maintainer's #11 comment](https://github.com/kristijonasatpro/Nida/pull/11#issuecomment-6297872659) explicitly allows a different compressor core under the contributed UI/meters. That backend substitution is not a reason to deny adoption of this portion.
- The PR later expanded to include `equalizer.rs`, `audio_equalizer.rs`, EQ commands and probes. Those are not present upstream. Closing the whole current PR would conceal remaining work unless that scope is preserved separately.

## Strong negative evidence for accepted-but-unpublished work

- Shader compiler blob `8e8e2165085be58948050fdcbbd635dd2be6bfec` is identical in v0.9.0 through v0.9.6. [Current compiler](https://github.com/kristijonasatpro/Nida/blob/0c9643c0ce747d0f63ccffda62dc2bf537841848/src/internal/isf/compiler.rs#L11) constructs a new Compiler for each call. Compare with the [acceptance comment](https://github.com/kristijonasatpro/Nida/pull/3#issuecomment-6297870462).
- Current test-workflow blob is `574348992b29657d1e81cab885dce43982e98ada`, matching #9's before-side blob. [Current workflow](https://github.com/kristijonasatpro/Nida/blob/0c9643c0ce747d0f63ccffda62dc2bf537841848/.github/workflows/test.yml#L60) omits the executable package and checks shown in [PR #9's diff](https://github.com/kristijonasatpro/Nida/pull/9/files), despite its [acceptance comment](https://github.com/kristijonasatpro/Nida/pull/9#issuecomment-6297870861).
- [Properties renderer](https://github.com/kristijonasatpro/Nida/blob/0c9643c0ce747d0f63ccffda62dc2bf537841848/src/usecases/ui/panels/effects.rs#L316) retains the horizontal layout targeted by #10.

## Closure policy for this audit

No PRs were closed and no comments were posted. This request asked for investigation and the evidence does not support closing a whole PR as fully implemented. A Windows proposal can be retired as out of scope, and the original audio contribution can be acknowledged/closed after preserving remaining EQ scope, but those are distinct decisions from marking a feature fully delivered.
