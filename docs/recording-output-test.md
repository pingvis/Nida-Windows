# Recording outputs — local Windows test

This change stays on `codex/recording-output-efficiency`. No PR is opened until
the user has tested the separate release build.

## What to try

1. In Library → Outputs, add two Recording outputs. Keep H.264 initially.
2. Choose Main for one and a production bus for the other. Keep or choose
   their audio buses independently. Existing rotation, surfaces, edge blending
   and SDR precision controls remain available.
3. Click each title's pencil to set names such as **Program** and **Clean**.
   Enter or clicking outside saves; Escape cancels. Use **Folder → Choose…**
   to select the destination. Both recorders can share a name and folder;
   they must still create different takes.
4. Start both. Check their separate clocks and dropped-frame counters, and
   the full **Writing** path. **Show file** opens the location; **Copy path**
   copies the actual filename.
5. Stop the first. The second must continue recording. Start the first again;
   it must create a new timestamped take, leaving its earlier take intact.
6. Stop both, allow a moment for background finalization, then play the files.
   Check motion, the correct picture/audio for each feed, and audio sync.
7. Repeat with the actual show resolution and workload, and with top-bar REC
   if that will run simultaneously. Top-bar REC controls only its own take.
8. Rename either recorder while running. Its current file must keep its name;
   the next Start must create `New name timestamp.ext`. In the top REC menu,
   set **Recording name**, then click Start directly without pressing Enter:
   the first take must use that name. Name changes during REC apply to the
   next Start; recovery files within the current take keep its original name.

## Scope

Eight-bit H.264/HEVC outputs can use the existing GPU NV12 conversion and the
same bounded buffering policy as top-bar REC. Unsupported dimensions retain
RGB readback; higher precision and alpha formats keep their existing paths.
The output recorder's fixed-quality software encoding is retained. NVENC and
new per-output bitrate/resolution settings are outside this fix.

Output settings reuse the existing saved title and destination path; the title
now supplies the filename prefix, the path supplies its folder, and the codec
supplies its extension. Top REC adds a backward-compatible `options.name` field,
defaulting to Nida for old shows. The actual last take is runtime status
(`recording_file` in the outputs state API), retained after Stop until the app
closes. Existing relative folders remain supported. API clients can configure
output names/folders through `POST /api/command` with
`ConfigureRecordingOutput { output_uuid, name, folder }`; omit either optional
field to leave it unchanged. A live folder change is rejected.

The user verified the initial recording fix. The naming/folder follow-up is
awaiting user verification; the PR remains on hold.

### Naming and folder follow-up checks

- Recording-related library tests: 55 passed, including pencil Enter/Escape,
  folder-picker targeting, committing a name before Start and preserving it
  when another option is clicked.
- Recording integration: 9 passed with real encoders, including settings
  persistence and retaining the current take's name through crash recovery.
  These tests require the encoder's localhost audio connection; a sandboxed
  attempt could not complete that connection and was rerun with it allowed.
- UI integration: 87 passed. Production library/application Clippy passed
  with warnings denied.
- Optimized Windows build and portable-package checks passed, including clean
  PATH application startup and bundled H.264/AAC encode/probe/decode.
- Release filename smoke check passed: output and REC prefixes, live renames
  leaving current files unchanged, new output takes using the changed name,
  rejecting live folder changes, saved names and readable resulting files.
- The refreshed standalone demo and launcher are in `dist/recording-name-test`,
  with its own workspace, recording folder and API port 18096. The portable ZIP
  is `dist/windows-recording-names/Nida-0.9.0-windows-x64-experimental-b203db0afc96.zip`.

## Validation

- Recording-related library tests: 51 passed, including independent starts,
  stops/restarts, filename reservations and existing REC controls.
- UI integration: 87 passed.
- GPU render correctness: 73 passed.
- Production library/application Clippy: passed with warnings denied.
- Repository-wide rustfmt still reports pre-existing differences; unrelated
  formatting changes are excluded from this branch.
- Release build: passed.
- Short sequential headless release comparison, on the same machine with
  builds idle, using a 4K60 plasma generator and the same fixed-quality H.264
  settings: original 35.93 render fps, updated 59.90 fps. Across the warm
  approximately ten-second sample, repeated frames increased by 253 originally
  and zero after the change. Both encoder-queue drop counters remained zero:
  this case exposed render/readback throughput, not only queue backpressure.
  The baseline executable's SHA256 matches the installed `6d0998b` package.
- Final-build dual 1080p60 check: 59.91 render fps, zero encoder-queue drops on
  either recorder, no added repeats after warm-up, independent stop and new
  restart file verified. Both H.264/AAC files were probed; decoding the second
  file confirmed its separate blue bus. These are short synthetic checks, not
  qualification of long-running capture, lip sync or two 4K60 recordings.
- Portable-package checks: passed, including payload hashes, application startup
  and bundled H.264/AAC encode/probe/decode from an unrelated working directory
  with a clean PATH. An extracted copy has a separate demo workspace and launcher
  under `dist/recording-output-test`; the user's installed app is unchanged.
