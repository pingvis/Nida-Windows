# Windows source-add latency

Investigation: 16 November 2026. Report: on another PC running 0.9.8, all
source types took roughly ten seconds to appear; browser and webcam were
examples. That PC's hardware, driver and phase timings are not yet available.

## Shared work

Every newly created source receives Crop and Transform effects. Previously,
`add_default_effects` called `Effect::new` synchronously twice on the engine
thread. This includes shader compilation, GPU pipeline construction and
resource setup. An API add could not finish, and the engine could not draw
its next frame, until those calls returned. The same implementation is
present in the 0.9.8 integration preceding the 0.9.9 merge.

This is a shared source-add stall, independently of browser navigation or
camera-driver startup. Image decoding, generator compilation and blit
pipeline creation are additional source-specific costs.

## Local measurements before the change

Current 0.9.9 code before this fix, browser-enabled **debug** executable,
RTX 3060 Ti, DX12, 1920x1080 show, scratch workspace. Local two-pixel BMP,
solid colour, `solid_color` generator and inline HTML; no external website.

| Measurement | First of its type | Following two |
| --- | ---: | ---: |
| Solid source API acknowledgement | 578 ms | 31 / 31 ms |
| Image API acknowledgement | 62 ms | 62 / 63 ms |
| Generator API acknowledgement | 297 ms | 47 / 31 ms |
| Browser API acknowledgement | 78 ms | 79 / 94 ms |
| Browser frame uploaded, from add request | 1,562 ms | 204 / 172 ms |

For the first solid, **571.1 ms** was default-effect construction: Crop
307.6 ms and Transform 263.3 ms. Deck textures took 0.2 ms. This directly
identifies the shared stall on this machine, but does not establish the full
cause of the other PC's ten seconds.

The first CEF transport startup spent 105 ms preparing runtime permissions;
the helper was spawned at 273 ms, and its first decoded frame arrived at
1,376 ms. Those are overlapping startup milestones, not times to add.
The browser tests follow the other source types, so their default effects
already benefit from warm shader compilation. These are exploratory local
measurements, not controlled cold-boot results or release/vMix comparisons.

## Patched observations

Two fresh processes using the same fixtures and debug profile, with no other
test process or compilation running:

| Measurement | Run A | Run B |
| --- | ---: | ---: |
| First solid API acknowledgement | 5.2 ms | 6.3 ms |
| First solid's defaults fully ready | 441 ms | 449 ms |
| First image API acknowledgement | 38.8 ms | 38.2 ms |
| First generator API acknowledgement | 1,199 ms | 308 ms |
| First browser API acknowledgement | 30.9 ms | 49.1 ms |
| First browser frame uploaded | 561 ms | 747 ms |

Default-effect admission itself takes about 0.3–1.1 ms in these runs, moving
the first source's observed half-second wait off the engine thread. This
does not eliminate compilation: the separate readiness column records it.
A regression deliberately blocks both default compilers and verifies that
the source still renders, then checks that early edits and UUIDs survive.

These runs also retain variability: one later image add took 349 ms, and the
first generator varied from 308 to 1,199 ms. Generator compilation, media
setup and driver work still deserve profiling. Browser first-paint variation
is not attributed to this patch, and disk/driver caches were not reset. No
general ten-second fix or vMix speed comparison is established by these data.

## Change and diagnostics

New-source default effects use the existing two-worker, bounded effect loader.
Their controls and identity settings exist immediately, in Crop/Transform
order; the source draws while their resources compile. Existing asynchronous
effect completion retains UUIDs, parameter edits, bypass and chain position.
Saving keeps settings rather than an in-flight job. Restored shows retain
their authored chains as before. If worker admission fails, direct construction
preserves both defaults instead of silently dropping controls; large bursts
that saturate the queue can therefore still block.

Debug diagnostics marked `[SOURCE LOAD]` separate default-effect admission,
blit pipeline construction, texture allocation, CEF permissions/process/first
decoded frame, camera format attempts, stream opening and first converted
camera picture. Adapter logs also identify device type and driver, helping
distinguish hardware rendering from an unexpected software adapter.

The Add Input dialog requests a synchronous camera rescan when its Camera
category first opens. Camera enumeration also queries formats by creating
backend camera handles. This is another possible UI stall, separately timed
as `Camera scan and format discovery`; it is not performed for every source
category. The API probe isolates engine adds and does not measure clicking
through the source picker or OS file dialogs.

Browser/camera backend delays remain distinct from source creation. Windows
camera opening runs in the background; its eight-second timeout is an upper
bound, not an intentional eight-second sleep for every source. Multiple
failed format attempts, driver negotiation or first-frame delivery need
actual hardware timings. No webcam is attached to this development PC.

## Reproduction on the affected PC

With a checkout and Python 3.11+, use the executable and matching browser/DLL
payload from the build being measured:

```powershell
python scripts/ci/benchmark-windows-source-load.py path/to/varda.exe --output target/source-load
# Also measure the first enumerated webcam, when attached:
python scripts/ci/benchmark-windows-source-load.py path/to/varda.exe --output target/source-load-camera --camera-id first
```

`--no-browser` measures the common solid/image/generator path without CEF.
`--backend dx12` or `--backend vulkan` explicitly selects a backend for a
separate comparison; the normal backend selection is unchanged.

The probe owns a fresh workspace and ephemeral API port, retains executable
SHA256, OS/GPU information, phase logs and JSON results, and uses a separate
CEF profile through `NIDA_BROWSER_PROFILE` (available in this build, including
release). Older debug builds support `NIDA_BROWSER_DEBUG_PROFILE`; older
release builds do not honor the new override, so use a separate Windows test
account or `--no-browser` when comparing those. No show or browser cookies
from an operator's running instance are used by the current probe/build.

`api_add_ms` measures command acknowledgement, not physical display latency.
`effects_ready_ms` separately tracks full default-effect readiness.
`backend_ready_ms` is a browser frame uploaded or camera picture reported
through the state snapshot, including polling delay. Solid/image/generator
physical first-pixel times are deliberately not inferred from an API reply.

## Validation

The production browser-enabled Windows debug executable builds, and production
library/application Clippy passes with warnings denied for application code.
Two effect-loader regressions pass, including the deliberately blocked default
compilers. Source-command roundtrips pass 51 tests, direct layers pass five,
and render correctness passes 75, including default-effect neutrality and
colour precision. The complete frame suite passes 33 tests with one explicit
measurement ignored: **166 distinct targeted tests pass** in total. The frame
fixtures now wait for explicit effect readiness
before asserting completed framing; their pixel assertions are unchanged.
That readiness wait is separate from the non-blocking first-picture regression.

Both patched native probe processes exit cleanly without application error or
panic records. No physical webcam or affected-PC run is claimed. Records are
under `target/source-load-before/`, `target/source-load-after/`,
`target/source-load-after-repeat/` and `target/source-latency-*`.
Changed sections/new modules are formatted; existing unrelated formatting
differences in the large camera/engine files and vendor build warnings remain.

## Free Windows coverage

- Standard [GitHub-hosted runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
  are free for public repositories and provide Windows Server 2022/2025 x64
  images, plus Windows 11 Arm runners. These cover OS/architecture and clean
  package checks; they do not provide a consumer GPU/webcam hardware matrix.
  Larger GPU runners are separately billed. The application currently ships
  x64, so an Arm host is not proof of a native Arm port.
- [Windows Sandbox](https://learn.microsoft.com/en-us/windows/security/application-security/application-isolation/windows-sandbox/)
  is included in supported Pro/Enterprise/Education editions, not Home. Its
  [configuration](https://learn.microsoft.com/en-us/windows/security/application-security/application-isolation/windows-sandbox/windows-sandbox-configure-using-wsb-file)
  supports memory limits, vGPU/WARP and opt-in webcam redirection. It is useful
  for clean-machine and constrained-memory checks, while sharing the host's
  hardware rather than reproducing another GPU vendor or camera driver.
- The [Azure new-customer offer](https://azure.microsoft.com/en-us/free/)
  includes $200 credit for 30 days and limited free VM allowances. Signup
  requires a payment card and eligibility. This is a limited trial/allowance,
  not an unlimited free GPU-device lab.

For the reported latency, collect the same probe on real Intel integrated,
AMD and NVIDIA machines, especially the affected PC. Use synthetic fixtures
for comparable runs, then separately check the real show and real websites.
Virtual machines alone cannot qualify physical USB negotiation, driver
startup, hybrid-GPU selection or sustained live capture.
