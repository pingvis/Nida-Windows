# Varda feature ports — completed implementation and local qualification

The four approved ports are implemented and pushed to `pingvis/Nida-Windows`
main. Four independent draft PRs target `kristijonasatpro/Nida` main. The
installed application was not replaced, no release was published, and no usage
reset credit was consumed. The earlier two untracked investigation documents
were preserved unchanged.

## Review branches

Managed worktree:
`C:/Users/augus/.codex/worktrees/varda-feature-ports-upstream/Nida-Windows`.
Each branch starts directly at upstream `0c9643c`, excluding Windows backend
changes. Upstream main was verified to still be that commit during validation.

| Port | Branch | Draft PR |
| --- | --- | --- |
| Analyzer | `codex/varda-analyzer-downsampling` | https://github.com/kristijonasatpro/Nida/pull/14 |
| NDI | `codex/varda-gpu-ndi-receive` | https://github.com/kristijonasatpro/Nida/pull/15 |
| Beat detection | `codex/varda-beat-detection` | https://github.com/kristijonasatpro/Nida/pull/16 |
| Shutdown | `codex/varda-safe-shutdown` | https://github.com/kristijonasatpro/Nida/pull/17 |

All four descriptions include verified results and explicit CI/platform limits.
Keep them draft for review of upstream baseline blockers. Do not merge or
publish releases without further user direction.

## Implemented behavior

- Safe shutdown reserves tickets before work is handed to a thread, closes
  admission, cancels queued builds and waits for active work and GPU-handle
  cleanup. Status remains responsive and logs outstanding work after five
  seconds; that threshold never forces process exit.
- Idle effect workers in the fork retain lifetime tickets through destruction
  of their GPU-owning builder closures. App load workers release captured GPU
  contexts before releasing their tickets.
- A late engine is retained and polled until it returns, then its show is saved,
  commands refused, outputs stopped and saves flushed. The former two-second
  stop could detach it and discard the app before saving. The direct OS
  event-loop exit fallback now drives these same phases.
- GPU initialization before a context exists and detached recording finalizers,
  including previously stopped takes, also hold shutdown open. Existing FFmpeg
  finalization timeout behavior is unchanged.
- Analyzer input is reduced/display-encoded on the GPU before readback, using
  the largest active requested size, never upscaling. Brightness requests 256,
  faces 1920. Workers share one buffer; metadata-only analysis skips readback.
- Portable wgpu NDI receive conversion uploads packed UYVY and expands it in a
  fragment render pass. BGRA/RGBA upload directly, preserving alpha. CPU fallback
  remains. Existing SD BT.601 / HD BT.709 appearance is retained. Networking and
  codec decompression remain SDK work; this is not zero-copy GPU NDI decode.
- Beat detection uses kick-band spectral flux, autocorrelation, tempo hysteresis
  and phase tracking on a bounded worker outside the capture callback. Sample
  timing follows the device rate; dropped hops reset stale history. The session
  audio tempo range has command/state/UI support and preserves clock priority,
  timebase, device recovery and PCM delivery.

Source commits on fork main include `5a12247`, `5cd46de`, `3e0f277`, `4611ce9`,
`b4827d1`, `b580e8c`, `40633de`, `c4ccb31`, `8fac7ae` and `662012f`.
Latest clean review heads: analyzer `e6c7887`, NDI `2dc12c0`, beat `b7404ca`,
shutdown `53e5b1a`. Fork-only effect loader and MIDI-discovery code are excluded
from the shutdown branch. If background-effects PR #5 is merged upstream,
reconcile its build/worker lifetime tracking; the fork already contains it.

## Verified results

[Measurements and CI audit](varda-feature-port-measurements.md) contains the
method, exact adapter/profile, reproducible commands and limits.

- Combined Windows full library run: **3,888 passed, zero failed, 17 explicitly
  ignored**, with normal local permissions, external TEMP and GPU required.
- Following the final late-engine correction: **69 runner/shutdown checks
  passed**, including responsive gated Quit, app recovery and saved state.
- NDI: **46 regressions passed**, plus an explicit real NDI 6.2.1 loopback pass
  checking GPU colour and exact alpha. SDK alpha frames arrived as BGRA and
  uploaded correctly; this is supported alongside RGBA.
- Production library and analyzer/audio/NDI benchmarks pass Clippy with warnings
  denied. Earlier combined checks passed 140 focused tests, 88 UI checks,
  51 command checks and three API documentation checks.
- Final optimized release build passed. The final isolated smoke compiled a real
  Gaussian blur effect asynchronously, finalized two 640×360 H.264/AAC takes
  (one previously stopped, one active at Quit), fully decoded both and checked
  rendered colour/manual clock. Exit code zero, about 451 ms after shutdown in
  this run. No deadline guarantee is implied.
- RTX 3060 Ti/DX12 serial microbenchmarks: 4K analyzer readback 10.151 ms full,
  0.263 ms brightness, 3.225 ms face. NDI explicit CPU fallback versus GPU path:
  12.058 vs 1.155 ms at 1080p; 47.918 vs 4.247 ms at 4K. These are conversion/
  transfer measurements, not application FPS or whole-receiver comparisons.

Evidence under `target`:
`varda-ports-full-library-final.log`, `varda-ports-late-engine-tests.log`,
`varda-ports-shutdown-cleanup-tests.log`, `varda-ports-ndi-regressions-final.log`,
`varda-ports-ndi-loopback-qualified.log`, `varda-ports-clippy-qualified.log`,
`varda-ports-release-final.log`, `varda-ports-benchmarks.log`,
`varda-ports-ndi-benchmarks.log` and
`varda-ports/smoke-1793484359380/result.json`.
The final smoke reproducer is `target/varda-ports/release-smoke.cjs`.
An earlier attempt used an invalid legacy GLSL fixture; it was replaced with
an actual bundled Vulkan-compatible blur shader. Do not count that fixture
error as a product regression or mistake its log for final validation.

## Upstream limits

Overall upstream CI remains red in existing formatting/platform code. A clean,
unmodified checkout also fails cargo fmt. Windows cannot compile the existing
unguarded DeckLink clock_gettime call; Linux Clippy reports 23 existing stub/
layout/style errors. Library jobs have ten failures in unchanged platform,
presentation, media-fixture and layout code. Latest NDI CI recheck reproduced
those same ten failures and 23 Clippy errors, with no new-module diagnostics.
Port GPU tests pass on CI's required lavapipe software Vulkan adapter.

Independent upstream production checks pass with a temporary Windows guard for
that DeckLink call; the guard was restored out and is not committed. Physical
macOS/Linux GPUs, external NDI senders/network loss, long shows and beat accuracy
on recorded music remain separate qualification. Do not claim overall CI is
passing or import unrelated baseline repairs into these feature PRs.

## Environment and scope

Original checkout: `E:/Custom Apps/Nida-Windows`. Load
`. ./.windows-tools/build-env.ps1`; it changes cwd to that checkout. Fork flags:
`--locked --offline --no-default-features --features screen-capture,windows-browser`.
For upstream use only `--features screen-capture` and explicitly return cwd to
the managed worktree after loading the environment. Use the original target
directory to reuse builds. Socket/external-scratch tests require normal local
permissions. Git metadata writes require escalation; GitHub connector PR writes
returned 403, while escalated Git/gh CLI works. Never expose credentials.

The one-shot continuation resumed after the natural five-hour reset and
finished this qualification. No further unattended monitoring is planned.
Source: MIT-licensed `im-knots/varda`, pinned `31133902`; relevant commits
`666276428d`, `e45d08781e`, `8e94fcd5a4`, `40451d4d40`, `182449eaf2`,
`caa37ddf2b`. Provider/parameter migrations, compositor rewrites and unrelated
features were not imported.
