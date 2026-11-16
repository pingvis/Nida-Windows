# Upstream PR review against Nida 0.9.7 — 1 November 2026

> Updated after the first maintenance batch: #2/#8 are closed; #3/#9/#14/#16 are refreshed. The original audit below is retained as the inspection record.

## Execution update — first maintenance batch

The sections below record the earlier inspection. Subsequent authorized work changed these PRs:

| PR | Action | Verified result |
| --- | --- | --- |
| [#2](https://github.com/kristijonasatpro/Nida/pull/2) | Posted scope explanation and closed. | Windows capture changes plus Windows/Linux CPU downscale; macOS capture and its fit_within helper unchanged. Linux optimization remains available in the fork. |
| [#8](https://github.com/kristijonasatpro/Nida/pull/8) | Posted scope explanation and closed. | Windows power request only; Mac/Linux runtime behavior preserved. |
| [#3](https://github.com/kristijonasatpro/Nida/pull/3) | Merged v0.9.7, pushed `03d6e5a`, rewrote current description. | Four-file focused diff; 28 existing compiler/metadata tests pass with real shader fixtures. Changed Rust formatting/whitespace pass. |
| [#9](https://github.com/kristijonasatpro/Nida/pull/9) | Merged v0.9.7, pushed `9280d4d`, rewrote current description. | Still only eight added workflow lines; YAML/whitespace pass. Ubuntu execution remains for Actions. |
| [#14](https://github.com/kristijonasatpro/Nida/pull/14) | Merged v0.9.7, fixed both benchmark formatting errors, pushed `03b2e67`, added validation notes and refreshed description. | All seven changed Rust files pass rustfmt. Full production check reproduces exactly two unchanged Windows DeckLink E0425 diagnostics. |
| [#16](https://github.com/kristijonasatpro/Nida/pull/16) | Reconciled the audio conflict with v0.9.7, pushed `a2ec5ba`, documented reconciliation and refreshed description. | 102 existing production audio/clock tests pass in isolated harness; preserves upstream async device lifecycle, live-feed handoff, PCM gathering and silence tracking. Only analysis hops go to the worker. Fixed formatting of the new command; other formatter edits intersecting PR lines were checked. Full production check still reports the same two unchanged DeckLink errors. |

Closures are scope decisions, not adoption claims:
- [#2 explanation](https://github.com/kristijonasatpro/Nida/pull/2#issuecomment-6558368075)
- [#8 explanation](https://github.com/kristijonasatpro/Nida/pull/8#issuecomment-6558368613)

Kept **#1, #6 and #7** open under the user's macOS-effect condition. #1 changes shared FFmpeg child ownership/finalization; #6 changes shared hardware fallback/frame recovery despite Windows-only fast seeking; #7 adds scheduling/audio behavior to Mac DeckLink output. Their Windows titles alone do not justify declaring them irrelevant to Mac.

GitHub confirmed #3/#9/#14/#16 are content-mergeable after the refresh; #16's conflict is cleared. Fresh Actions runs started automatically. Whole-repository formatting remains red; tests/lint were still running at inspection. No green full-suite claim is made. No force-pushes, branch deletions or unrelated upstream formatting sweeps were used. Workspace returned to its original main branch.

Remaining work: reconcile #11's already adopted controls/compressor/meters and partial input-EQ overlap with the still-proposed EQ delta; update other retained branches and resolve their outstanding conflicts individually. The first batch does not claim the full PR backlog is repaired.



Reviewed all 18 pull requests by `pingvis` targeting `kristijonasatpro/Nida`, in numerical order, including each description, changed-source scope and discussion timeline (conversation comments, review submissions and inline comments).

**Result: 0 fully integrated, 1 partly adopted/overlapping, 17 with outstanding implementation. No PRs closed and no GitHub comments posted.** All 18 were open at inspection. Closing only fully integrated proposals would close none.

This supersedes the earlier [0.9.6 audit](upstream-pr-adoption-audit-2026-11-01.md), especially its statement that upstream has no EQ. Version 0.9.7 does have input EQ; it does not cover the whole EQ proposal in #11.

## Evidence and method

- GitHub's live recent-PR listing returned all 18 authored PRs. Fetched the full merged discussion timeline individually for #1–#18. #12–#18 have no comments/reviews in those returned timelines.
- Live GitHub commit search confirmed main is `45e6f0b95741bfd15e9cda16e1284cdf2096f2fd`, matching the existing local upstream/main object used for source inspection.
- Read the published [v0.9.7 release](https://github.com/kristijonasatpro/Nida/releases/tag/v0.9.7) and [v0.9.2 release](https://github.com/kristijonasatpro/Nida/releases/tag/v0.9.2) through GitHub. v0.9.7 was published 1 November at 13:53:52 Europe/Vilnius.
- Compared PR source changes at the exact returned head/base SHAs, inspected current behavior, and compared relevant older/current upstream blobs. Feature equivalence matters: a different implementation can fulfill a proposal.
- The maintainer's [#1 explanation](https://github.com/kristijonasatpro/Nida/pull/1#issuecomment-6297870074) describes importing into an internal branch and publishing squashed releases, then closing PRs with release links. GitHub ancestry/merge status cannot establish adoption in this workflow. A promise to take a change cannot establish public delivery either.
- This is a source/discussion audit. No fresh application build, runtime or hardware qualification was needed to determine whether these implementations are present. Unpublished internal work cannot be inspected.

## One-by-one decisions

| PR | Discussion and review outcome | Current public source evidence | Decision |
| --- | --- | --- | --- |
| [#1 Windows core](https://github.com/kristijonasatpro/Nida/pull/1) | Maintainer defers Windows support because no Windows test machine is available; explains internal intake workflow. | [Recording shutdown](https://github.com/kristijonasatpro/Nida/blob/45e6f0b95741bfd15e9cda16e1284cdf2096f2fd/src/internal/renderer/subprocess.rs#L3473) still launches the Unix placeholder `true`; [DeckLink](https://github.com/kristijonasatpro/Nida/blob/45e6f0b95741bfd15e9cda16e1284cdf2096f2fd/src/internal/decklink/mod.rs#L362) retains the Windows compilation blocker. | Keep open under the requested integrated-only rule. Retiring a declined Windows proposal would be a different closure reason. |
| [#2 Capture performance](https://github.com/kristijonasatpro/Nida/pull/2) | Maintainer refers to #1's Windows deferral. | Windows capture and shared CPU resampling are unchanged from the prior audited upstream. [Readback](https://github.com/kristijonasatpro/Nida/blob/45e6f0b95741bfd15e9cda16e1284cdf2096f2fd/src/internal/screen_capture/platform/windows.rs#L590) still allocates its staging texture inside each read. | Keep open. |
| [#3 Shader cache](https://github.com/kristijonasatpro/Nida/pull/3) | Maintainer says tested on macOS and taking as-is. | [Compiler](https://github.com/kristijonasatpro/Nida/blob/45e6f0b95741bfd15e9cda16e1284cdf2096f2fd/src/internal/isf/compiler.rs#L11) still constructs shaderc and compiles every call. Blob remains `8e8e2165085be58948050fdcbbd635dd2be6bfec`. | Keep open: accepted in discussion, not publicly integrated. |
| [#4 Recording outputs/names](https://github.com/kristijonasatpro/Nida/pull/4) | Maintainer accepts with a shared atomic file-reservation fix for REC and Library recorders. | [RecordOptions](https://github.com/kristijonasatpro/Nida/blob/45e6f0b95741bfd15e9cda16e1284cdf2096f2fd/src/internal/record/mod.rs#L119) still lacks the proposed name setting; [file_path](https://github.com/kristijonasatpro/Nida/blob/45e6f0b95741bfd15e9cda16e1284cdf2096f2fd/src/internal/record/mod.rs#L462) still uses `exists()`. v0.9.7's recording/stream throughput fixes address different behavior. | Keep open. |
| [#5 Background effects](https://github.com/kristijonasatpro/Nida/pull/5) | Maintainer identifies first-mask atomicity/test failures and plans a correction internally. | [Effect insertion](https://github.com/kristijonasatpro/Nida/blob/45e6f0b95741bfd15e9cda16e1284cdf2096f2fd/src/app/engine_impl.rs#L843) still invokes constructors synchronously. Proposed loader/pending-state integration is absent. | Keep open. |
| [#6 Windows decoding/seek](https://github.com/kristijonasatpro/Nida/pull/6) | Maintainer defers Windows and flags Mac seek changes. Contributor's follow-up restricts the fast path to Windows and documents tests. | [Hardware decoding](https://github.com/kristijonasatpro/Nida/blob/45e6f0b95741bfd15e9cda16e1284cdf2096f2fd/src/internal/video/hwdecode.rs#L23) still opts in on macOS only. Video sources are unchanged since the earlier audit. Fixing the PR does not mean upstream adopted it. | Keep open. |
| [#7 Windows DeckLink/audio](https://github.com/kristijonasatpro/Nida/pull/7) | Maintainer flags scheduling lead and legacy audio behavior. Contributor reports fixes and default-off embedded-audio consent; native validation remains pending. | [Non-Mac SDK implementation](https://github.com/kristijonasatpro/Nida/blob/45e6f0b95741bfd15e9cda16e1284cdf2096f2fd/src/internal/decklink/mod.rs#L1234) remains a stub. v0.9.7's Mac Blackmagic input/audio work does not implement Windows output support. | Keep open. |
| [#8 Windows keep-awake](https://github.com/kristijonasatpro/Nida/pull/8) | Maintainer flags a REASON guard integration issue. Contributor reports the guard fix and portable cross-compilation checks, with native checks pending. | [KeepAwake](https://github.com/kristijonasatpro/Nida/blob/45e6f0b95741bfd15e9cda16e1284cdf2096f2fd/src/app/keep_awake.rs#L26) still holds only Mac/Linux platform resources; unsupported platforms report held without a Windows power request. | Keep open. |
| [#9 FFmpeg tools in CI](https://github.com/kristijonasatpro/Nida/pull/9) | Maintainer says taking it. | [Linux test dependencies](https://github.com/kristijonasatpro/Nida/blob/45e6f0b95741bfd15e9cda16e1284cdf2096f2fd/.github/workflows/test.yml#L60) still omit the executable package/checks. Workflow blob remains `574348992b29657d1e81cab885dce43982e98ada`, the PR's before-side content. | Keep open: acceptance is not public delivery. |
| [#10 Properties effect cards](https://github.com/kristijonasatpro/Nida/pull/10) | Maintainer plans adoption with #5 and moving build-failure status onto collapsed headers. | [Properties effects](https://github.com/kristijonasatpro/Nida/blob/45e6f0b95741bfd15e9cda16e1284cdf2096f2fd/src/usecases/ui/panels/effects.rs#L316) still uses `channel_fx_hscroll` and `horizontal_top`; this file is unchanged in v0.9.7. The separate pre-existing vertical Effects tab is not this change. | Keep open. |
| [#11 Audio settings/compressor/meters/EQ](https://github.com/kristijonasatpro/Nida/pull/11) | Maintainer identifies combined-gain ramp, snapshots and docs issues; allows native DSP under contributed UI/meters. Contributor fixes ramp and expands scope to EQ. | Settings/meters and compressor behavior shipped in v0.9.2 with explicit contribution credit. v0.9.7 adds four-band input EQ. The proposal also supplies built-in bus EQ, three bells, trim and a graphical editor, absent upstream. Details below. | Partly adopted/overlapping; keep current expanded PR open. |
| [#12 Playlist controls](https://github.com/kristijonasatpro/Nida/pull/12) | No discussion/review entries returned. | [PlaylistSettings](https://github.com/kristijonasatpro/Nida/blob/45e6f0b95741bfd15e9cda16e1284cdf2096f2fd/src/engine/value/playlist.rs) still lacks Auto Next/Program autoplay settings. [Playback snapshots](https://github.com/kristijonasatpro/Nida/blob/45e6f0b95741bfd15e9cda16e1284cdf2096f2fd/src/internal/deck/mod.rs#L1038) cover Video/HapVideo only. Generic clip controls in release notes do not cover this playlist proposal. | Keep open. |
| [#13 MIDI startup](https://github.com/kristijonasatpro/Nida/pull/13) | No discussion/review entries returned. | [App initialization](https://github.com/kristijonasatpro/Nida/blob/45e6f0b95741bfd15e9cda16e1284cdf2096f2fd/src/app/mod.rs#L919) still enumerates MIDI synchronously; proposed MIDI discovery/startup modules are absent. Background audio/camera discovery is separate. | Keep open. |
| [#14 Analyzer downsampling](https://github.com/kristijonasatpro/Nida/pull/14) | No discussion/review entries returned. | [capture_frame](https://github.com/kristijonasatpro/Nida/blob/45e6f0b95741bfd15e9cda16e1284cdf2096f2fd/src/internal/analyzer/mod.rs#L302) uses full source texture dimensions for readback. Analyzer sources are unchanged in v0.9.7; the per-worker reduction capture is absent. Sharper output scaling is different work. | Keep open. |
| [#15 GPU NDI receive](https://github.com/kristijonasatpro/Nida/pull/15) | No discussion/review entries returned. | [Receiver](https://github.com/kristijonasatpro/Nida/blob/45e6f0b95741bfd15e9cda16e1284cdf2096f2fd/src/internal/ndi/mod.rs#L449) still requests RGB mode and [converts on CPU](https://github.com/kristijonasatpro/Nida/blob/45e6f0b95741bfd15e9cda16e1284cdf2096f2fd/src/internal/ndi/mod.rs#L531). NDI sources are unchanged in v0.9.7; the packed receive GPU pass is absent. | Keep open. |
| [#16 Tempo/beat detection](https://github.com/kristijonasatpro/Nida/pull/16) | No discussion/review entries returned. | [Estimator](https://github.com/kristijonasatpro/Nida/blob/45e6f0b95741bfd15e9cda16e1284cdf2096f2fd/src/internal/audio/mod.rs#L569) still scores beat intervals, with 16-entry history. The analysis worker, low-band/autocorrelation approach and configurable tempo-range command are absent. v0.9.7's audio mixer work does not replace this estimator. | Keep open. |
| [#17 Safe shutdown](https://github.com/kristijonasatpro/Nida/pull/17) | No discussion/review entries returned. | [Engine stop](https://github.com/kristijonasatpro/Nida/blob/45e6f0b95741bfd15e9cda16e1284cdf2096f2fd/src/usecases/ui/runner/engine_thread.rs#L381) still gives up and drops the handle after its two-second patience. Proposed build/finalizer lifetime tracking and closing view are absent. | Keep open. |
| [#18 Text reuse/status](https://github.com/kristijonasatpro/Nida/pull/18) | No discussion/review entries returned; draft. | [Text drawing](https://github.com/kristijonasatpro/Nida/blob/45e6f0b95741bfd15e9cda16e1284cdf2096f2fd/src/internal/deck/text.rs#L290) replaces texture resources each drawing. [TextSnapshot](https://github.com/kristijonasatpro/Nida/blob/45e6f0b95741bfd15e9cda16e1284cdf2096f2fd/src/engine/value/text.rs#L210) only contains settings. The newly released base Text feature does not include this follow-up. | Keep open. |

## #11: what shipped and what remains

The [v0.9.2 release](https://github.com/kristijonasatpro/Nida/releases/tag/v0.9.2) explicitly attributes expandable settings and calibrated meters to Augustas's #11. [Implementation commit 34fa288](https://github.com/kristijonasatpro/Nida/commit/34fa288) credits the contributor. A native compressor is a valid implementation of this part: the maintainer explicitly discussed that substitution. Requiring FFmpeg internals verbatim would incorrectly deny the adopted contribution.

The [v0.9.7 release](https://github.com/kristijonasatpro/Nida/releases/tag/v0.9.7) adds a useful input EQ, but does not claim to import #11's later EQ implementation. Treat this as functional overlap unless provenance is established.

| Area | Current #11 proposal | Public v0.9.7 |
| --- | --- | --- |
| Settings, compression, calibrated meters | Expandable source/bus controls, compressor, calibrated peaks | Adopted in v0.9.2 with credit; subsequently developed further |
| EQ targets | Sources and buses | Built-in EQ on source strips; bus can use a macOS Audio Unit plug-in |
| EQ bands | High-pass, low/high shelves, three bells | Separate low cut plus low/high shelves and two bells |
| EQ editor | Logarithmic response graph, draggable points, numerical controls | Sliders and per-band controls |
| EQ output trim | Dedicated trim before compression | No equivalent dedicated EQ trim |
| Backend/API | FFmpeg rack, SetAudioBusEqualizer / SetAudioSourceEqualizer | Native DSP, SetAudioSourceEq; plug-in host is a separate bus facility |

Evidence: [proposed EQ specification](https://github.com/pingvis/Nida-Windows/blob/2eb28a8d3b300ef8ea970dbcaf50548352cfaff4/docs/audio-equalizer.md), [proposed commands](https://github.com/pingvis/Nida-Windows/blob/2eb28a8d3b300ef8ea970dbcaf50548352cfaff4/src/engine/mod.rs#L1740), [upstream mixer documentation](https://github.com/kristijonasatpro/Nida/blob/45e6f0b95741bfd15e9cda16e1284cdf2096f2fd/docs/16-audio-mixer.md), [upstream source EQ editor](https://github.com/kristijonasatpro/Nida/blob/45e6f0b95741bfd15e9cda16e1284cdf2096f2fd/src/usecases/ui/panels/audio_mixer.rs#L484).

Keeping this expanded PR open preserves remaining work. A future cleanup can separate the remaining EQ delta and close the original adopted portion with release evidence; this audit did not create a replacement PR, edit branches or retire outstanding scope.

## Exact PR revisions inspected

| PR | Base | Head | Discussion entries |
| --- | --- | --- | --- |
| #1 | `c74d97e838bc41f936e233873cd4c0132eb13c06` | `31ac0cb1fc3f8e3fc11864ffeb7b82ef2f04ded3` | 1 |
| #2 | `c74d97e838bc41f936e233873cd4c0132eb13c06` | `61c8cc62df3cfa7926444b1078ad466e4c01d06c` | 1 |
| #3 | `c74d97e838bc41f936e233873cd4c0132eb13c06` | `b93996b848a77400a352364c7edc46970d96789e` | 1 |
| #4 | `c74d97e838bc41f936e233873cd4c0132eb13c06` | `7dfa1f39d234f10531e815417f6369e10f452135` | 1 |
| #5 | `c74d97e838bc41f936e233873cd4c0132eb13c06` | `3b5b5bdf8f4f33b43013e4fee37121ca7eb7c7d2` | 1 |
| #6 | `c74d97e838bc41f936e233873cd4c0132eb13c06` | `3f1d1a183b0d993d62a39becf089d6f5efea39fa` | 2 |
| #7 | `c74d97e838bc41f936e233873cd4c0132eb13c06` | `f853e6591ba0bf62b2d494565e040e39579b6ae8` | 2 |
| #8 | `c74d97e838bc41f936e233873cd4c0132eb13c06` | `7c84ec349a62b6a3794696948242e8b49757e718` | 2 |
| #9 | `c74d97e838bc41f936e233873cd4c0132eb13c06` | `612b56750a9d20ab9617624ebaf87d2cee17025d` | 1 |
| #10 | `c74d97e838bc41f936e233873cd4c0132eb13c06` | `bdd69977787a67771ebfa94bf01745999e7ef9cc` | 1 |
| #11 | `c74d97e838bc41f936e233873cd4c0132eb13c06` | `2eb28a8d3b300ef8ea970dbcaf50548352cfaff4` | 2 |
| #12 | `a312a18469fb0aa503fbaedac64672d8d127544c` | `c7a9fff948ec43498e01f02b3ac3a838505e00f6` | 0 |
| #13 | `a0c8e864424b9038baf16336b1c2b4d2b56f30d6` | `d6e125f951f4708d2cda2fd97a80edf8f2d69e42` | 0 |
| #14 | `0c9643c0ce747d0f63ccffda62dc2bf537841848` | `e6c78878dc53fbf6641a63d63dc6f53f0279906e` | 0 |
| #15 | `0c9643c0ce747d0f63ccffda62dc2bf537841848` | `2dc12c07ed0fb24bb08451f1ef9904df669079ec` | 0 |
| #16 | `0c9643c0ce747d0f63ccffda62dc2bf537841848` | `b7404ca47c147e5e221d0640443b1863d2a20919` | 0 |
| #17 | `0c9643c0ce747d0f63ccffda62dc2bf537841848` | `53e5b1a7b36f0654b99d4ff13b2dd0270d99e2a6` | 0 |
| #18 | `45e6f0b95741bfd15e9cda16e1284cdf2096f2fd` | `2daa1cf8cfbfb1b66c7d8a8b96e1dbb1f89264a3` | 0 |

## Follow-up: failed checks, conflicts and Windows scope

Checked live PR mergeability and latest workflows for each exact current head. This is a snapshot; #18 still had tests and Windows Clippy running when inspected.

| PRs | Latest workflow state |
| --- | --- |
| #1–#12 | Current-head workflows return `action_required`, rather than a completed test failure. Sampled #1/#12 runs have no jobs. This is consistent with fork-workflow authorization awaiting a maintainer. |
| #13 | Formatting and Clippy fail; Linux library tests: 3,552 pass, 9 fail, 10 ignored. |
| #14 | Formatting and Linux/Windows Clippy fail; Mac matrix job canceled. Linux library tests: 3,754 pass, 10 fail, 10 ignored. |
| #15 | Formatting and Linux/Windows Clippy fail; Mac matrix job canceled. Linux library tests: 3,754 pass, 10 fail, 11 ignored. |
| #16 | Formatting and Linux/Windows Clippy fail; Mac matrix job canceled. Linux library tests: 3,765 pass, 11 fail, 10 ignored. |
| #17 | Formatting and Linux/Windows Clippy fail; Mac matrix job canceled. Linux library tests: 3,757 pass, 11 fail, 10 ignored. |
| #18 | Formatting and Linux/Mac Clippy have failed; test job and Windows Clippy still running at inspection. |

Read logs for all 24 completed failing jobs on #13–#18 at inspection. The failures have several causes:

- Windows compilation on #13–#17 stops in unchanged upstream DeckLink references to Unix-only `clock_gettime`/`CLOCK_THREAD_CPUTIME_ID`. [Example job](https://github.com/kristijonasatpro/Nida/actions/runs/41225176460/job/124607717307).
- Linux Clippy rejects platform stubs/dead code and other existing helpers with warnings denied: 10 diagnostics on #13, 23 on #14–#17, 32 on #18. Mac Clippy on #13 and #18 reaches assertion-style lints in unchanged integration tests. [Latest Linux job](https://github.com/kristijonasatpro/Nida/actions/runs/41307439503/job/124850610405), [Mac job](https://github.com/kristijonasatpro/Nida/actions/runs/41307439503/job/124850610491).
- Linux library failures recur across distinct proposals: systemd inhibitor test, DeckLink range values, enum-size threshold, FFmpeg fixture generation, Mac-only PDF/presentation assumptions, and sometimes camera timing. The test job stops on library failures, so that job does not establish successful later integration suites. [#14 test job](https://github.com/kristijonasatpro/Nida/actions/runs/41225176404/job/124607716756).
- Formatting is widespread upstream debt, but cannot all be dismissed as inherited. **#14's new `benches/analyzer_capture.rs` itself has two rustfmt failures**, confirmed against the exact new file in that PR. [Formatting job](https://github.com/kristijonasatpro/Nida/actions/runs/41225176405/job/124607717496). Other PRs' formatting failures overlap some edited files, so hunk comparison is needed before assigning responsibility. #18's 144 flagged files exclude all three edited source files.
- Canceled matrix jobs provide no passing result. Local Windows validation of the combined fork does not establish a green upstream PR build on Mac/Linux.

GitHub currently reports actual merge conflicts (`mergeable=false`, `dirty`) on **#4, #5, #7, #10, #11, #12, #13, #16 and #17**. The other nine are content-mergeable but have check-related status. These conflicts reflect diverging edits since each branch's base; they neither establish adoption nor imply the proposed feature is wrong. Older runs on a previously mergeable branch do not validate a newly resolved merge. Shared areas include audio for #11, startup/app initialization for #13, and runner/shutdown for #17.

For retained upstream proposals, the sensible order is: reconcile with current upstream, remove already-adopted scope, fix introduced formatting/lints, then distinguish remaining baseline failures with evidence. Re-running unchanged deterministic failures alone will not resolve them. The maintainer's internal import workflow may avoid a GitHub merge, but stale/conflicting proposals still impose review and porting work.

The maintainer explicitly deferred **#1, #2, #6, #7 and #8** as Windows scope in his #1 comment. Closing these as **maintained in Nida-Windows / outside current upstream scope** is reasonable. They should not be marked integrated. Closing leaves the fork, commits and branches intact. Portable pieces in #1/#2/#7 can be proposed separately if useful; the general-purpose upstream PRs should not be retired just because development/testing happened on Windows.

This follow-up explains the recommended closure category; it has not changed PR states, posted comments, rebased branches or rerun CI.


