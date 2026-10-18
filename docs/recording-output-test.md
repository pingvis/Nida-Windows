# Recording outputs — local Windows test

This change stays on `codex/recording-output-efficiency`. No PR is opened until
the user has tested the separate release build.

## What to try

1. In Library → Outputs, add two Recording outputs. Keep H.264 initially.
2. Choose Main for one and a production bus for the other. Keep or choose
   their audio buses independently. Existing rotation, surfaces, edge blending
   and SDR precision controls remain available.
3. Set a file base for each, such as `D:/Show/Program.mp4` and
   `D:/Show/Clean.mp4`. You can also give both the same base to test collision
   protection: they must create different takes.
4. Start both. Check their separate clocks and dropped-frame counters, and
   the full **Writing** path. **Show file** opens the location; **Copy path**
   copies the actual filename.
5. Stop the first. The second must continue recording. Start the first again;
   it must create a new timestamped take, leaving its earlier take intact.
6. Stop both, allow a moment for background finalization, then play the files.
   Check motion, the correct picture/audio for each feed, and audio sync.
7. Repeat with the actual show resolution and workload, and with top-bar REC
   if that will run simultaneously. Top-bar REC controls only its own take.

## Scope

Eight-bit H.264/HEVC outputs can use the existing GPU NV12 conversion and the
same bounded buffering policy as top-bar REC. Unsupported dimensions retain
RGB readback; higher precision and alpha formats keep their existing paths.
The output recorder's fixed-quality software encoding is retained. NVENC and
new per-output bitrate/resolution settings are outside this fix.

Saved-show settings keep their existing shape. The configured filename now
acts as a base for unique takes instead of an overwrite destination. The actual
last take is runtime status (`recording_file` in the outputs state API), retained
after Stop until the app closes. Existing relative bases remain supported.

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
- Portable-package checks: pending.
