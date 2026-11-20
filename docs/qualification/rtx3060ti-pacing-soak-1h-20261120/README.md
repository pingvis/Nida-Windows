# One-hour GPU playlist soak: passed

Requested follow-up to the completed pacing qualification at `c19e528`.
Verified the same frozen release test executable by SHA256 before launch.
Seven fixtures cover H264 1080p60/4K60, HEVC10, HDR10 and three SDR P010 ranges.
Fresh host activity checks found the GPU idle; workloads ran serially.

Started 2026-11-20 17:16:44 Vilnius; finished 18:16:45. Result: exit zero,
one test passed, 3,600.0003 seconds, 216,002 pictures, 239 seeks, 29 reloads
and all seven items shown. Peak RSS: 503,422,976 bytes (480.1 MiB); final
sample: 448,147,456 bytes. The existing steady-state growth limit was retained.

Each tick checks GPU validation faults, fallback, zero production picture
downloads/uploads and three-slot bounds. The test also checks delivery progress,
paused preloaded-picture retention, playlist movement and teardown. All passed.
This is a release qualification-harness stability soak, not an actual-app
one-hour PTS/presentation benchmark or a physical scanout measurement. Picture
counts include intentional seek/reload behavior and do not establish fresh fps.

`run.ps1`, fixture/binary manifest, raw log, result, initial GPU readings and
hashes accompany this record. Run from repository root with the documented
Windows build environment and qualified local binary. Choose a fresh output
directory to preserve prior evidence. The source/binary qualification bundle
is [here](../rtx3060ti-pacing-fence-event-20261120/README.md).
No colour thresholds, lifetime safeguards, experimental defaults or counters
were weakened. No merge or push was performed.
