# Windows MIDI discovery crash found during audio qualification

Investigation: 16 November 2026, Windows 0.9.9 integration.

## Evidence

The audio integration process previously ended with native access violation
`0xc0000005` around `a_clips_sound_stays_in_step_with_its_picture`. The
retained Windows crash dumps identify a different subsystem from the test
that happened to be running:

- 1 November, process 9704: `wdmaud!CUserModeSynth::UseDefaultSynthSink`,
  through `CMicrosoftGSWavetableSynth::LoadDLSFile`, `midiOutOpen`, midir's
  output connection, and `MidiDeviceManager::scan_devices` on the
  `midi-discovery` worker.
- 16 November, process 13396: `ntdll!RtlpWaitOnCriticalSection`, through
  `wdmaud!CUserModeSynth::PlayBuffer`, `CMicrosoftGSWavetableSynth::SendMIDIEvent`,
  `midiOutReset`, and midir's output-connection destructor. Discovery finished
  after its receiver had gone away, so dropping the failed send's manager
  reset the synthesizer on the worker.

Both dumps were analyzed locally with Windows Debugger and Microsoft public
symbols. The dump files were not uploaded. Raw analysis logs are under
`target/audio-crash-*-symbols.log` and are not committed. A pre-change serial
run of the four `a_clip`-matching tests passed, confirming that a successful
rerun alone does not explain the earlier native failures.

## Changes

Windows controller discovery excludes the built-in **Microsoft GS Wavetable
Synth** output before matching or connecting ports. This software sound
generator is not a controller feedback port; discovering controllers must not
instantiate and tear down its sound engine. Other hardware and virtual MIDI
ports retain their existing discovery, connections and feedback behavior.
Matching is case-insensitive and applies only on Windows.

Automatic discovery also stays disabled in `test-fixtures` builds, as it
already did in unit-test builds. Integration tests link the library without
`cfg(test)`, so the old unit-only guard accidentally opened the developer's
real MIDI ports for every short-lived test app. MIDI-specific tests continue
to inject detached devices and delayed discovery results. Normal application
builds still discover controllers automatically.

Saved MIDI mappings and configuration formats are unchanged. Pending-discovery
quit remains non-blocking; the fix does not join a potentially stuck driver on
the render thread or change the audio decode/mix synchronization algorithm.

## Validation

- All 87 MIDI-related unit/UI tests pass, including the new output-selection
  regressions and existing pending-discovery save/adoption/quit coverage.
- The production browser-enabled Windows executable builds and production
  library/application Clippy passes with warnings denied for application code.
- Three production discovery/rescan/shutdown cycles pass with exit code zero
  and no application error or panic records. The raw native port enumerator
  still lists Microsoft GS Wavetable Synth; the app's initial and rescanned
  controller snapshots correctly exclude it. No physical controller was
  attached, so this is not a hardware-controller qualification.
- The first post-fix complete audio process finishes without a native crash:
  13 tests pass, one ProRes sync assertion fails, and the NVIDIA workload is
  ignored. Its first flash/beep pair measured +77 ms (limit 50 ms), with the
  other pairs near zero. The isolated ProRes rerun passes. Timing limits are
  unchanged; this observation is separate from the two captured MIDI crashes.
- The subsequent complete serial audio rerun passes all 14 tests, with one
  NVIDIA hardware workload explicitly ignored, in 69.94 seconds. The earlier
  ProRes assertion remains recorded above rather than counted as a pass.
- Three additional serial repetitions of the four `a_clip`-matching tests all
  pass (15.29, 15.40 and 15.31 seconds), including the original failure's test
  order. These repeat existing tests and add no distinct tests to the count.

Native production reproduction, with runtime DLLs on `PATH`:

```powershell
python scripts/ci/test-windows-midi-startup.py target/debug/varda.exe --output target/midi-native
```

The harness requires a normal build without `test-fixtures`, uses scratch
workspaces and ephemeral API ports, and waits for real MIDI adoption before
checking the state. It never connects to another application's API.

Records are under `target/audio-midi-*`, `target/helper-console-*` and
`target/midi-native-fixed/`. Existing vendor build warnings and repository-wide
formatting/test-lint debt remain outside these changes.
