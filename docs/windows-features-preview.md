# Windows features preview checkpoint

The four implementations remain independent branches from main `2527864`:

| Feature | Branch | Commit |
|---|---|---|
| Chroma-key preview picker | `codex/windows-colour-picker` | `e0c7f04` |
| Per-app audio | `codex/windows-app-audio` | `8f2288c` |
| Exclude Nida from display capture | `codex/windows-capture-exclusion` | `d6ccf9c` |
| Capture scaling performance | `codex/windows-capture-render-performance` | `fb58677` |

`codex/windows-features-preview` combines those commits for local review. The only
manual merge resolution retained both module declarations in renderer/mod.rs.
Main has not been changed and these new branches have not been pushed. No PR,
release or tag was created.

Individual validation and limitations are documented in windows-colour-picker.md,
windows-app-audio.md, windows-capture-exclusion.md and windows-capture-performance.md.
The combined debug build succeeded. Production Clippy passed, along with 50
picker tests, 51 capture tests and seven app-audio tests. Interactive/audio
fixtures were excluded. Logs are under `target/windows-features-preview-*.log`.

The preview is running from `target/debug/varda-windows-features-preview.exe`,
using `target/windows-features-preview-workspace`, API port 8100. API smoke checks
confirmed about 60 fps after startup, no configured bus or monitor audio device,
a silent "Picker test" colour input with chroma key, and successful workspace save.
Runtime metadata is in `target/windows-features-preview-current.json`.


**Do not use computer speakers or change Windows volume/routing.** Task tone
fixtures were stopped, and their standalone helpers now refuse implicit default
output. Tests must remain offline/silent. Do not run every ignored test: one
per-app audio test needs a deliberately configured non-speaker fixture. Native
silent process-loopback capture, source reconnect and persisted executable identity
were verified; live nonzero PCM isolation remains unverified under this constraint.

The computer-use skill was read, but native app screenshot approval timed out.
Do not bypass that approval with alternative UI automation or desktop screenshots.
No manual picker click/preview resize/browser interaction gesture validation is
claimed. Native GPU/API tests cover the implementation but do not replace those
user-visible checks.

The four implementations and combined preview are ready for review with the
manual UI and live-audio limitations above. No speaker playback was used during
this resumed verification. Main and remotes remain unchanged. The standalone
benchmark cache ignore was also propagated to the performance branch.

The usage-limit continuation reached its scheduled reset before this resumed run.
Its automation remains ACTIVE: the app's usage/automation tools were unavailable
in this session, so it could not be paused through the supported tool. On a later
wake, read this completed checkpoint, pause that continuation when the tool is
available, and do no further implementation work unless the user requests it.
