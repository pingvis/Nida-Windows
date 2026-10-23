# Windows startup measurements — 2026-10-24

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

The first change to pursue is background MIDI discovery with later adoption of
the ready manager. Existing mappings must be restored when device identities
become available, and quitting/saving while discovery is pending must preserve
those mappings. Drawing an immediate startup view would also make GPU and
other remaining initialization visible to the user. These behavior changes
have not been implemented by this investigation.

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
