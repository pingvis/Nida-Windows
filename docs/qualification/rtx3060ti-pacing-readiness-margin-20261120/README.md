# 1.5 ms margin: normal playback passes, fast-playback gate fails

This intermediate release candidate passes all 36 ordinary seek-phase windows,
recovering 60 initial misses without phase losses. Ordinary 1080p60 and 4K60
freshness checks pass. However, its 2x playback interval delivers **55.85 fps**,
with 84 empty takes and 84 mailbox replacements over 20 seconds.

The cause is a narrow eligibility condition: a 60 fps source at 2x has an
effective rate of 120, while the worker correctly caps production at 60. The
readiness policy incorrectly treats that as a rate-conversion hold and skips
the optional wait. The next candidate extends eligibility to the worker's
actual output cadence, preserving existing decode caps and legitimate holds.

Overall acceptance is false. The exporter originally checked the 2x copy cap
without checking its freshness; that missing analysis gate is now added.
Original run records, timing, counters and thresholds are unchanged. This is
not a completed qualification. Exact binaries/source snapshots/full traces are
preserved under `target/gpu-pacing-fix-20261120-readiness-margin/`.

No colour, three-slot lifetime, production-transfer, experimental-default or
fallback policy changed. GUI measurements remain API driven and obscured;
foreground/physical scanout is not qualified. No push was performed.
