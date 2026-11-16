# Windows integration of Nida 0.9.9

Upstream release `v0.9.9` (`6f1b4d7`) is merged into the Windows fork's
`main`, preserving its history and Windows fixes. The tag includes the
background/frame additions and the Calm centre fade follow-up.

The RTMP reconnect failures below record the original merge qualification.
They are resolved by the subsequent [Windows RTMP recovery follow-up](windows-rtmp-recovery.md):
the supported streaming suite now passes 14 tests, with five platform/hardware
workloads explicitly ignored.

## Integration decisions

- Include upstream's frame filters and backgrounds, independent slide pictures,
  compositor reuse and late-frame scheduling, capture sizing, asynchronous NDI
  sending, and audio-output changes.
- Preserve Windows GPU decoding/encoding and top-bar NVIDIA recording, native
  AEC3, browser sources, PowerPoint COM automation, per-app sound, capture,
  DeckLink output, input/bus EQ, playlists and bounded shutdown/build tracking.
- Combine per-pass shader uniform buffers with asynchronous effect loading:
  pending effects start without GPU buffers and install all pass buffers when
  their background build completes. A regression checks the completed load.
- Keep optimized capture-pixel downscaling alongside upstream's new drawn-size
  calculation.
- Pass upstream's beside-show PDF-export flag and timeout through the Windows
  structured COM request. The helper checks show state for ordinary exports
  and permits an explicitly authorized beside-show export. Its stand-in tests
  verify both paths and that the running show is retained.
- PDF rendering and validation still require macOS Core Graphics. The portable
  slide-picture worker tests check the platform error and partial-file cleanup
  on Windows, while continuing to exercise queueing and show scheduling.
- Adapt frame-pixel fixtures to wait, with a deadline and explicit failure
  reporting, for the fork's asynchronous shader build to finish. Normalize
  CRLF in the shader-editing fixture before matching source structure.
- Mark native Core Graphics slide tests explicitly ignored outside macOS;
  retain web capture/readiness coverage using ordinary still-image fixtures.
  Mark the two native macOS live-bitrate tests separately from Windows NVENC's
  existing hardware tests. GPU availability alone does not provide either
  platform's native media capabilities.
- Regenerate the API reference from the combined routes and schema.

## Validation

Windows production features: `--locked --offline --no-default-features
--features screen-capture,windows-browser`. GPU tests use `VARDA_REQUIRE_GPU=1`.

The all-target compiler check, production library/application Clippy with
warnings denied, and Windows debug executable build pass. The regenerated API
reference passes three consistency tests. The Windows PowerPoint COM helper's
stand-in tests pass, including permitted and refused exports beside a show.

The broad library run records **4,545 passes, two failures and 20 ignored**.
The new asynchronous-effect regression initially used an incomplete shader;
its corrected fixture loads the release's actual multipass Neon ring and
passes. The SRT loopback initially received no frames, then passed four
isolated runs. Together these qualify **4,547 distinct library tests**, with
20 explicitly ignored tests; they are not a claim that the original broad
process had zero failures. The initial SRT connection failure remains an
intermittent observation without a proved cause.

Repository-wide formatting and all-target Clippy remain unclean, with inherited
formatting differences and test lints (notably empty assertions, native-test
fixtures and direct queue submits). The corrected effect fixture adds no new
lint failure. Production Clippy passes; do not extend that claim to all tests.
Resolved Rust files and new fixture helpers are formatted; whitespace checks
pass outside vendor code. Existing upstream formatting is kept in the large
frame/slide fixtures to avoid an unrelated file-wide rewrite.

The 12 selected integration suites qualify **374 passing tests**, **33
explicitly ignored tests**, and **two unresolved RTMP reconnect failures**.
The passing counts are 14 audio, 51 command, 33 frame, 9 LED, 43 persistence,
11 recording, 75 render-correctness, 19 slide, 12 streaming, 3 text, 88 UI,
and 16 visibility tests. The API reference's three tests are additional.

The frame suite's first run assumed synchronous effect construction. With
bounded waits for shader readiness, 32 pass; the remaining source-comparison
fixture then passes after CRLF normalization. This is qualification across
those runs, not a claim that the initial frame process passed unchanged.
The slide rerun passes 19 tests and explicitly ignores 24 native Core Graphics
cases. Web-capture fixtures use still pictures where PDF page rendering is
irrelevant to the behavior under test.

The initial audio process ended natively during its sync test, as observed in
previous Windows qualifications. The isolated sync test and complete serial
audio rerun pass (14 passes, one NVIDIA hardware test ignored). The initial
termination remains an unresolved observation, not a proved merge defect or
a hidden pass.

The broad streaming run passes 12 tests, fails four, and ignores three NVIDIA
hardware workloads. Two failed tests require the macOS native live-bitrate
encoder; they are now explicitly ignored outside macOS, alongside the separate
Windows NVENC hardware tests. The two remaining failures also reproduce in
isolation:

- `a_change_of_network_cuts_the_wait_between_attempts_short`: only two retries
  within 30 seconds; FFmpeg reports `Error number -138 occurred`.
- `a_destination_that_is_down_reconnects_on_its_own_when_it_comes_back`: no
  Reconnecting state observed within the fixture's eight-second window.

The RTMP sender and subprocess sources are unchanged by this merge. These
results do not establish their cause or qualify every RTMP recovery scenario.
Other streaming checks pass, including reconnect-on-network-change,
multiple destinations, picture/audio, upload adaptation and colour conversion.
Do not describe the entire integration selection as green.

Logs are under `target/upstream-099-*` and are not committed.

No installed application is replaced and no release is published by this merge.
Physical device recovery, long live shows and macOS/Linux runtime behavior are
outside this local Windows qualification. No upstream performance figures are
claimed as Windows measurements.
