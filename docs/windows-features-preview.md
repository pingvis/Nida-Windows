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
The final combined build and selected tests are running; their logs are under
`target/windows-features-preview-*.log`. The machine-readable current status is
`target/windows-feature-progress.json`. Do not claim combined-build success until
its log and exit status confirm it.

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

Finish the combined targeted checks and build, launch it in a fresh isolated
workspace with no media or configured audio destinations, and perform API smoke
checks. Preserve the individual branches. Then present concrete results and the
remaining manual/live-audio limitations. User allows overwriting test shows.
Do not reboot/shut down the PC, touch unrelated processes, or push/merge main.
At <=5% remaining usage, checkpoint and schedule this chat after the limiting reset;
repeat until the four features are ready for review, then pause the continuation.
