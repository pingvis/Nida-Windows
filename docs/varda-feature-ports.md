# Varda feature ports — implementation checkpoint

User-authorized scope: safe shutdown, portable GPU NDI receive conversion,
analyzer GPU downsampling and improved beat detection. Four focused draft PRs
are authorized, with validated work integrated into `pingvis/Nida-Windows` main.
Do not replace the installed application or publish a release. Preserve the
untracked earlier investigations `docs/vmix-gpu-investigation.md` and
`docs/vmix-small-features-varda-audit-2026-10-31.md`.

The user chose waiting safely: report unfinished background work after five
seconds and keep waiting. GPU NDI is portable wgpu work, not Windows-only.
Existing clock selection/timebase, audio-device recovery and PCM loss accounting
must remain intact. Do not use reset credits or buy credits.

## Current implementation

Original checkout: `E:/Custom Apps/Nida-Windows`, branch `main`, original base
`6a40079`. Commits:

- `5a12247`: per-context background build tickets, close/cancel admission,
  deck/effect/engine lifetime tracking and responsive shutdown status.
- `5cd46de`: GPU analyzer reduction, brightness 256 / face 1920 long-edge
  requests, no upscaling, shared frames and metadata-only capture support.
- `3e0f277`: GPU UYVY NDI conversion, direct BGRA/RGBA uploads, reusable frame
  handoff and CPU fallback. Existing SD BT.601 / HD BT.709 appearance retained.
- `4611ce9`: kick-band onsets, autocorrelation tempo, tempo hysteresis and phase
  lock; bounded analysis worker; session audio tempo range, command and UI.
  Sequence gaps reset stale history. Input sample-rate timing is preserved.
- `0db6861`: NDI Clippy fixes.
- `b4827d1`: detached recording-finalizer lifetime tracking, including takes
  stopped before Quit. A late engine drops its baton before releasing its
  ticket so shutdown cannot miss newly registered recording finalizers.
- `b580e8c`: wait for GPU initialization when quitting before a context exists.

Follow-up `40633de`: mixed-analyzer contract regression and native GPU
readback benchmark (`benches/analyzer_capture.rs` plus its Cargo.toml entry).
The benchmark measures serial submit/wait/map/row-pack cost with pipeline
creation excluded. It is not the application's pipelined frame rate.

## Upstream review

Managed worktree:
`C:/Users/augus/.codex/worktrees/varda-feature-ports-upstream/Nida-Windows`.
Review branches start directly from upstream Nida `0c9643c`, not Windows main:

| Port | Branch | Draft PR |
| --- | --- | --- |
| Analyzer | `codex/varda-analyzer-downsampling` | https://github.com/kristijonasatpro/Nida/pull/14 |
| NDI | `codex/varda-gpu-ndi-receive` | https://github.com/kristijonasatpro/Nida/pull/15 |
| Beat detection | `codex/varda-beat-detection` | https://github.com/kristijonasatpro/Nida/pull/16 |
| Shutdown | `codex/varda-safe-shutdown` | https://github.com/kristijonasatpro/Nida/pull/17 |

Each independent production `cargo check` passes on Windows with a temporary
compatibility guard around upstream's existing unguarded DeckLink
`libc::clock_gettime` call. The guard was restored out after each check and is
not committed. Helper: `target/varda-ports/upstream-windows-check-shim.cjs`.
Upstream test builds force the embedded browser test dependency graph, even
with production HTML features disabled; that large build was cancelled.
Do not claim clean upstream Windows tests passed.

Shutdown's clean adaptation excludes fork-only EffectLoader and MIDI discovery
code; it has a small `shutdown_view.rs` instead of importing the fork's startup
renderer. It also transfers FFmpeg child ownership directly (Option<Child>)
rather than creating the Unix `true` placeholder. If upstream merges the
separate background-effect-loader PR #5, reconcile its lifetime integration.
The Windows fork already tracks effect jobs and closes that loader on Quit.

The connected GitHub PR-creation tool returned 403. Escalated `gh` CLI access as
`pingvis` works, as does Git push. Use `gh pr edit/create ... --body-file` with
body files under `target/varda-ports`. Do not expose credential contents.
Git metadata and managed-worktree edits require sandbox escalation.

## Validation

Load `. ./.windows-tools/build-env.ps1` in the original checkout. Fork flags:
`--locked --offline --no-default-features --features screen-capture,windows-browser`.
Clean upstream uses `--features screen-capture` (no Windows browser feature).
The environment script changes cwd to E:; explicitly Set-Location back when
using the managed worktree. Set CARGO_TARGET_DIR to the original target to reuse
builds. Tests using sockets or external scratch workspaces need normal local
permissions (require_escalated). They do not modify the installed application.

Results so far:

- Full serial library run: 3,877 passed, eight environment failures, 16 ignored.
  All eight failures passed in a focused rerun with normal local permissions
  and an external TEMP. Rerun included one additional workspace test: nine pass.
  The sandbox blocked local TCP/UDP traffic; workspace-local TEMP violated two
  intentional source-tree isolation assertions. No test expectations were weakened.
- Latest shutdown name filter: eight pass, including startup GPU initialization.
- Build tracker and gated recording-finalizer regressions: four pass.
- Analyzer tests include native Rgba16Float reduction, display-encoded readback
  and mixed-worker sharing/metadata-only behavior. The final focused run across
  all ports passed 140 tests (two ignored).
- NDI tests: 46 pass, including native GPU vs CPU SD/HD comparisons, fallback,
  padded BGRA/RGBA, alpha, format changes and malformed frame rejection.
- Audio tests cover tempo/phase across synthetic rhythms, gain, silence,
  live range changes, missing/extra hits, 44.1/48/96 kHz and queue overload.
- Command integration: 51 pass; API documentation: three pass. Manual updated.
- Production library and both changed benchmarks pass Clippy with -D warnings.
- All 88 UI integration tests pass.

Logs under target:
`varda-ports-full-library.log`, `varda-ports-environment-rechecks.log`,
`varda-ports-shutdown-final.log`, `varda-ports-lifetime-tests.log`,
`varda-ports-integration.log`, `varda-ports-combined-focused.log`,
`varda-ports-clippy-combined-final.log`, `varda-ports-release-build.log`,
`varda-ports-benchmarks.log` and `varda-ports-upstream-*-check.log`.
Vendor wgpu-hal's 161 future-compat warnings are pre-existing.

## Remaining work

The Windows release build passed. Workspace-only headless smoke passed twice;
the stronger run is target/varda-ports/smoke-1793468322870/result.json. Two
640×360 H.264/AAC takes (2.55/2.58 seconds) decoded fully, with expected red
pixels [255,89,64]. One take stopped before Quit; the other was active at Quit.
Exit code 0, about 449 ms to exit. Forced manual 123 BPM survived changing the
audio range. No installed application was changed. Reproducer:
target/varda-ports/release-smoke.cjs (requires the build environment).

1. Finish native analyzer/audio benchmarks. Current cargo bench session 56331
   writes target/varda-ports-benchmarks.log. It uses the normal release features,
   --config profile.bench.debug=false and profile.bench.strip="symbols" to
   align dependencies with the release build. It may finish while this chat
   waits for usage reset. Inspect the log/process before starting another build.
   Record exact adapter/profile, timings and limitations; serial submit/map
   timing is not show FPS. No measured speedup has been claimed yet.
2. Add/perform representative GPU NDI timings and, if the local NDI runtime is
   available, a controlled sender/receiver loopback without affecting other
   applications. The packed UYVY upload halves transfer bytes but needs timing
   and live-source validation. Preserve colour/alpha behavior and fallback.
3. Review any relevant CI/reviewer findings and updated benchmark results. All
   four PR descriptions now include test and release smoke results. Keep draft
   until remaining meaningful validation is addressed; report physical source
   and macOS/Linux hardware limits honestly. Upstream test builds currently
   force the large Servo dev dependency graph on Windows.
4. Windows main includes formatting commits affd70d and 545f05c and this final
   checkpoint, pushed before yielding. Their clean
   branch cherry-picks are pushed (beat b7404ca, shutdown eb3c34a); analyzer
   e6c7887 is also pushed. Preserve the two untracked research documents.
5. If upstream merges background-effects PR #5, reconcile build-ticket tracking
   in that loader. Do not introduce the Windows-only loader into shutdown PR
   merely to make its branch look identical to the fork.

Source attribution: MIT-licensed `im-knots/varda`, pinned main
`31133902cc1331563ba4d1cb02c85a46118be634`. Relevant commits: shutdown
`666276428d`, analyzer `e45d08781e`, NDI `8e94fcd5a4`, beat detection
`40451d4d40`, `182449eaf2`, `caa37ddf2b`. No provider/parameter migration or
compositor rewrite is included.

## Usage and continuation

Read `mcp__codex_app__get_usage_limits` between substantial units. At about 95%
five-hour consumption, save the checkpoint and create/update same-chat heartbeat
“Continue Varda feature ports” after the reported resetsAt plus two minutes.
Recheck usage when waking, preserve work, continue this scope and disable the
heartbeat after completion. Initial reset: 31 October 2026 at 23:18 Vilnius;
re-read rather than assuming. Last observed consumption: 94% five-hour, 60% weekly. Same-chat heartbeat
continue-varda-feature-ports is ACTIVE, scheduled once for 31 October at 23:21
Europe/Vilnius, after the reported 23:18:44 reset plus two minutes. Read usage
on waking and update this same automation if another continuation is needed.
