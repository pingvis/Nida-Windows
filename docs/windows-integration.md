# Windows fork integration — 19 October 2026

This update brings the native Windows DeckLink backend and embedded stereo
mixer-bus audio into the existing Windows integration build. It also includes
the supported recording Rename label and its existing Enter/Escape interaction
test. The existing Windows core, capture performance, shader cache, recording
outputs, background effect loading, opt-in GPU decoding, portable package and
installer remain part of the fork's main.

The upstream DeckLink branch remains separate at
`codex/windows-decklink-output`. Opening its draft PR through the GitHub
connector was rejected with `403: Resource not accessible by integration`.
The branch is prepared for manual upstream submission; packaging and installer
workflows remain fork-only.

## Evidence and limits

- DeckLink code revision `deb5056db4ad75684f2d9135edd96bcd0cb96cac` passed
  all four jobs in https://github.com/pingvis/Nida-Windows/actions/runs/39319183811:
  native Windows/macOS and Linux module builds/tests/lint, and the locked
  Linux core application check. See windows-decklink.md for details.
- The pencil-fix PR's font-coverage regression passed. That run reported
  3,382 library tests passed, eight failed and ten ignored; it was not green.
- Two failures were DeckLink full-range conversion regressions addressed by
  the DeckLink branch. Remaining failures were the OS keep-awake assertion,
  the output-variant size check, the streaming audio-header check, and three
  sound-only video tests.
- Broader Clippy failures include the output-variant size difference,
  assertion-style lints and similar local names in video tests. Repository-wide
  rustfmt also reports differences outside the rename fix.
- The pencil PR modifies one UI file; the output enum definition is unchanged,
  and the earlier recording change adds a last-take path to HeadlessOutput,
  which reduces rather than causes the reported 320-byte window/headless gap.
- This integration records those issues rather than suppressing tests or
  reformatting unrelated files.
- Fresh combined-branch CI includes the module checks, locked Linux core
  application check and existing font/rename interaction regressions.
  Its result is pending when the integration is prepared; no Windows full-app
  build or combined complete test-suite pass is claimed.
- Real DeckLink hardware audio, physical lip sync and sustained playback
  still require a Windows PC, card and receiver.

The application's Cargo.lock is unchanged. The native harness selects its
cc version from that lockfile so a newer harness dependency cannot hide an
application build-script API mismatch.
