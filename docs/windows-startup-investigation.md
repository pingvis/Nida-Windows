# Windows startup measurements — 2026-10-24

## Local implementation follow-up

MIDI discovery now runs on a named worker in the shared application code.
The engine polls its result without waiting and adopts the manager before
draining MIDI messages. Initial discovery does not perform a second native
scan for controller profiles: the current show's profiles are applied to the
discovered ports when the manager arrives.

Saved MIDI mappings remain named while discovery is pending or unavailable,
including when the show is saved, saved as, or quit. Bindings for absent
controllers are retained on subsequent saves. An explicit Clear mappings
still writes an empty configuration. A saved forced MTC port is deferred and
restored too; changing the timecode preference while waiting cancels that
deferred choice. Loading another workspace clears the previous deferred
configuration before restoring that workspace's own settings.

The window accepts close/resize events while GPU initialization is pending.
Once a GPU surface is available, it submits a Nida startup view and returns
to the event loop before constructing/loading the engine. This does not draw
before the GPU is available or move all remaining initialization to workers.
Manual rescanning of an already initialized MIDI manager remains synchronous.

Native verification also exposed that the windowed runner accepted an API
Shutdown command without exiting. It now asks winit to exit when that flag
is observed while borrowing the engine; the normal exit handler saves and
stops the engine after the borrow is returned. This allows a clean API quit
while MIDI discovery is still pending.

This is an upstream candidate: the worker, persistence and startup view are
cross-platform. The measured slow native call was Windows-specific evidence;
Mac and Linux runtime validation remains necessary before an upstream PR.
No PR has been opened.

## Implementation validation

- Application library regression suite: **304 passed, zero failed, one
  ignored**. Includes pending MIDI save/adoption/failure, explicit mapping and
  timecode changes, absent-controller preservation and a blocked worker.
- MIDI module and MIDI UI tests: **63 passed, zero failed**.
- Additional windowed shutdown/pending MIDI persistence regression: **one
  passed**, using an isolated workspace and Recent-shows path.
- Core and browser-enabled production library/binary Clippy passed with
  warnings denied. Browser-enabled debug application built successfully.
- New modules and main pass rustfmt checks; diff whitespace checks pass.
- Three corrected native windowed launches completed with **exit code 0**,
  zero error records, zero warnings, no panic and no forced termination. The
  benchmark retains shutdown logs and rejects error records (including error
  notifications), panics, nonzero exit and forced termination.
- Three Python regression checks pass for shader fixture placement, retaining
  existing shows and rejecting error notifications even without a crash.

The earlier native runs had been accepted on startup/exit status despite a
visible workspace error. Their fixture put only shaders into `.varda`, making
the loader treat it as an incomplete saved show and quarantine its routing.
The corrected fixture uses the normal working-directory `shaders/` library,
leaves `.varda` absent until the app saves, and refuses to reuse saved-show
directories. The real incomplete-show routing protection is unchanged.

Final browser-enabled debug build, RTX 3060 Ti / Vulkan, isolated empty shows
with 139 shaders; camera, screen capture and NDI disabled:

| Run | Startup view submitted | Initialization complete | Exit code |
| --- | ---: | ---: | ---: |
| Default Windows log, first run | 1.549 s | 3.415 s | 0 |
| Default Windows log, repeat | 1.398 s | 3.021 s | 0 |
| Explicit Unicode log path | 1.389 s | 3.013 s | 0 |

Records: `target/startup-benchmark-clean-fixture/results.json` and
`target/startup-benchmark-clean-override/results.json`. These are logged submission
times, not physical first-frame measurements or a controlled cold-boot
before/after comparison. They do not measure the user's saved show, active
browser navigation, capture or production streaming.

An earlier native smoke run during this implementation reproduced the slow
MIDI scan: initialization completed at **3.453 s**, while MIDI was adopted at
**12.942 s**. This confirms the slow native operation ran independently of
show initialization. That run exposed the windowed API Shutdown issue and
required forced termination; it is not counted as a successful final run.
Its record is `target/startup-benchmark-after-fixed/no-devices-1/startup-log.json`.

The measurements below are the original **before** investigation.

The first measured local launch stalled in synchronous MIDI discovery before
the first UI could draw. This matches a window appearing with a blank interior
and then becoming usable much later.

## Measurements

Wall-clock seconds from process launch to existing startup log messages:

| Test | Window created | GPU ready | Initialization complete |
| --- | ---: | ---: | ---: |
| First browser-enabled development launch, empty shader library | 1.599 | 2.267 | 13.343 |
| Warm browser-enabled development launch, 139 shaders | 0.103 | 0.688 | 2.144 |
| Repeat of that launch | 0.106 | 0.672 | 1.757 |
| Browser disabled, 139 shaders | 0.106 | 0.704 | 1.749 |
| Repeat with browser disabled | 0.105 | 0.658 | 1.748 |
| Optimized core release, 139 shaders, no concurrent benchmark | 0.071 | 0.636 | 1.437 |
| Repeat of optimized core release | 0.074 | 0.596 | 1.260 |

In the slow first launch, `MIDI init` was logged at 2.355 seconds and `MIDI
scan: 0 input(s), 1 output(s)` at about 12.593 seconds. The 10.238-second gap
occurred while initializing/enumerating native MIDI ports, before opening the
reported Microsoft GS Wavetable Synth output. Warm MIDI initialization took
about 0.11 seconds. Audio initialization took about 0.01 seconds.

## Code path

`UIRunner::resumed` creates the window and initializes the GPU in the background.
Once the GPU is ready, `finish_init` synchronously calls `VardaApp::new` on the
window/event thread. That constructor calls `MidiDeviceManager::new`, whose
`scan_devices` enumerates native output and input ports synchronously.

The runner does not draw its normal UI until engine initialization finishes.
Before then, `window_event` returns early because neither the engine nor app
has been installed. A slow native MIDI driver therefore delays painting and
handling the window's normal events, even when no MIDI controller is in use.

The first change identified was background MIDI discovery with later adoption of
the ready manager. Existing mappings must be restored when device identities
become available, and quitting/saving while discovery is pending must preserve
those mappings. A startup view was also identified to make remaining
initialization visible. The local implementation is described above.

## Reproduction and evidence

Run from a shell with the matching FFmpeg DLL directory on PATH:

```powershell
python scripts/ci/benchmark-windows-startup.py PATH/TO/varda.exe `
  --output target/startup-check --shader-library shaders --iterations 2
```

The benchmark uses explicit isolated workspaces, disables CLI installation and
OSC input, requests an unused API port, timestamps log lines, then requests
shutdown. It records `results.json` and each run's `startup-log.json`. It does
not dispatch GitHub Actions or modify the user's saved show.

Local evidence is in `target/startup-benchmark-before/`,
`target/startup-benchmark-with-shaders/`, and
`target/startup-benchmark-release-isolated/`.

## Limits

These timings measure logged initialization completion, not display hardware's
first visible frame. The first measured run was not a controlled cold boot;
the exact reason the native MIDI call took ten seconds remains unprofiled.
The browser-enabled binary was an existing development build; the optimized
release binary was an existing core build. These are not measurements of a
newly built browser-enabled release or the user's saved show. Disabling HTML
in an empty show tests base application initialization, not navigation of a
restored browser source. The machine used an NVIDIA RTX 3060 Ti with Vulkan.
