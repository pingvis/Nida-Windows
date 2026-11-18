# Windows UI responsiveness

18 November 2026. Scope: menu/panel interaction and repeated snapshot work.

The control window now attempts engine access without waiting. If the engine
is occupied, it draws the last complete view model with current UI-owned
selection and panel layout. Engine edits are retained in order, with a limit
of 32 pending UI frames. At that limit, backpressure occurs before more input
is consumed. This is bounded buffering, not an unlimited command queue.
Input removal is resolved to the displayed UUID before enqueueing. Accepted
edits are drained before the final shutdown save once the engine returns.

Active interaction also has a roughly 60 Hz redraw deadline independent of
engine wakeups. Idle/sound throttling remains. Actual interaction speed still
depends on OS scheduling, layout, GPU work and presentation. Source loading,
foreground engine command execution and display acquisition can still stall;
these changes do not move the entire UI to worker threads.

The registry snapshot is reused until the shader registry's revision changes.
Scanning, successful hot reload and provider removal invalidate it, including
same-count edits that change a shader's classification. The API/show formats
are unchanged.

## Validation

Production library/application Clippy passes with warnings denied. Six
responsiveness tests, two registry checks and all 88 UI integration tests pass.
One registry timing diagnostic is explicitly ignored by the normal test run.
With the actual engine baton held, the test measured 106.5388 ms on the
inherited admission path and 5.8 microseconds on the non-blocking path. This
single debug-build result demonstrates removal of the wait, not a general
UI speedup ratio or a physical display-latency measurement.

Use the normal Windows build environment and production feature selection:

```powershell
. ./.windows-tools/build-env.ps1
cargo clippy --locked --offline --no-default-features --features screen-capture,windows-browser --lib --bin varda -- -D warnings
cargo test --locked --offline --no-default-features --features screen-capture,windows-browser --lib responsiveness -- --test-threads=1 --nocapture
cargo test --locked --offline --no-default-features --features screen-capture,windows-browser --lib registry_snapshot -- --test-threads=1 --nocapture
cargo test --locked --offline --no-default-features --features screen-capture,windows-browser --test ui_integration -- --test-threads=1
```

The busy-engine test compares the inherited 100 ms admission path with the
non-blocking path while deliberately retaining the actual engine baton. Its
timings measure admission, not click-to-screen latency. The ignored registry
diagnostic compares fresh collection and cached snapshots within one build;
run optimized and with nothing else compiling before making performance claims.

Native verification used an isolated show. An initial attempt encountered a
locked desktop. After the user reported it unlocked, the responsiveness build
was checked successfully: Layout menu opening/closing, panel visibility,
Setup/Audio Mixer tab switching and opening the Master audio settings.
No physical menu-opening latency or visual smoothness measurement is claimed.
A baseline app's localhost counters were readable outside the network
sandbox, but idle counters do not reproduce the reported symptom.
The installed application and user shows were not replaced.

Direct previews and moving UI surface acquisition to a presenter thread remain
separate follow-ups. Preserve the existing Preview/Program frame buffers.

## Local test build

The final optimized Windows build passed. Launch it with
`scripts/run-ui-responsiveness-test.ps1`; it supplies the local FFmpeg runtime
path and uses `target/ui-responsiveness/user-test-show`, leaving the installed
application alone. Browser runtime files remain beside the executable in the
existing `target/release/browser` directory.

Executable: `target/release/Nida-UI-Responsiveness-Test.exe`.
SHA-256: `DC37969C5763EFCAC3F827DFFA9FC5DBFB026669E44BCCE6013A4473A396798F`.
