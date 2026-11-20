# Fence-notification pacing fix: qualification passed

Fresh release harness and production app, based on main `a5b98f1`, on branch
`fix/windows-video-frame-pacing`. `acceptance.json` reports all checks passed.
No shader override, merge or push. Previous failed candidates remain preserved.

The completed change combines precise interruptible decoder/engine waits,
deadline accounting and one shared 1.5 ms mailbox-readiness window with native
D3D12 copy-fence notification. Notification replaces ordinary 1 ms copy polling;
the decoder rechecks the exact fence serial and retains the event with native
resources during timeout retirement. API failures retain safe polling fallback.
Source clocks, output caps, colour shader/thresholds, three-slot lifetime safety,
production zero-transfer policy, experimental status and defaults are unchanged.

## Measurements

| Actual app | Fresh pictures/s | Interval |
| --- | ---: | ---: |
| Headless 1080p60 | approximately 60 | 60 s |
| Headless 4K60 | 60.00 | 60 s |
| GUI after pause/seek | 60.00 | 60 s |
| Fractional-seek hold | 60.00 | 60 s |
| 60 fps source at 2x | 60.00 | 20 s |

All these intervals have zero empty takes, mailbox replacements or repeated/
backward encoded PTS. Inclusive boundary counting produces 3,601 pictures in
one 60-second interval; this does not represent throughput above 60 fps.
All 36 eight-second seek-phase windows pass, recovering 899 initial misses
without residual loss. Maximum observed readiness wait is 0.4928 ms.

26 harness records include explicit legacy-copy-poll controls. Paired observed
copy-readiness p95 falls from 1.641 to 0.805 ms at 1080p, and from 2.861 to
1.461 ms at 4K. These are CPU observations, not GPU timestamp measurements.
Default traced runs have zero copy-poll sleeps or fallback. Untraced three-run
harness medians are 59.983 fps at both resolutions. CPU medians are 3.44% and
1.98% of one core, respectively; variability prevents a general CPU-gain claim.

Three-clip playback exercises 5,505 successful readiness waits, including up to
three waits per frame. Maximum observed shared-window span is 1.3854 ms; total
waiting per frame peaks at 1.2509 ms. The other two three-clip cases do not wait.
All meet freshness targets, with zero production picture transfers/backpressure.

Validation: 28 CPU checks pass (one existing timing probe ignored), 194 release
video regressions pass (seven explicit hardware tests ignored in that group),
and the native fence timeout/spurious-wake/reuse test passes separately. Six
colour/seek/pool groups pass 30 picture comparisons; interlaced fallback, adapter
deadline forwarding and engine-stop interruption pass. Three-minute soak passes:
10,809 pictures, 11 seeks, one reload, six shown items; peak RSS 349,728,768 bytes.
Strict production Clippy and test compilation pass. Source/settings hashes match.

## Reproduction and limits

Copied build/runners/exporter, manifests, fixture hashes, raw records and analyses
document reproduction from repository root. Choose fresh output paths: traces
refuse overwrite. Dependencies/build setup use `.windows-tools/build-env.ps1`.
Exact binaries, selected source snapshots and full traces remain local under
`target/gpu-pacing-fix-20261120-fence-event/`; provenance and hashes accompany this
compact export. Native-copy polling control is test-only
`NIDA_VIDEO_BENCH_COPY_POLL=1`; production apps ignore it.

Results measure distinct pictures entering conversion/composition. GUI runs are
API driven and obscured; unobstructed foreground and physical scanout are not
qualified. No universal Windows deadline, audio-sync or arbitrary-load guarantee
is claimed. Finite passing phase checks do not prove every possible phase.
GPU discard totals include warmup payloads and are not comparable directly with
the CPU path's GPU-payload-discard counter. AMD/Intel qualification stays deferred.
