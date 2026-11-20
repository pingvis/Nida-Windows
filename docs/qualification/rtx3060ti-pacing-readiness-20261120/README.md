# One-millisecond readiness candidate: one residual timeout

This freshly built release candidate adds a single 1 ms publication-readiness
window shared across a render frame. **Overall acceptance remains false.**
It is preserved as an intermediate candidate, not a completed qualification.

The control without readiness reproduced 46 fps and 420 empty takes over 30
seconds at a requested 2.5 ms consumer shift. Enabled cases at 2.25/2.5/2.75/3 ms
delivered approximately 60 fps with no empty takes or replacements. Separate
launches can have different observed phases: one enabled case recovered 32
initial misses; the requested 2.5 ms case naturally required no waits.

All ordinary 60-second freshness checks passed. Three simultaneous 1080p60
clips, including shifted cases, each delivered at least 59.98 fps, with zero
picture transfers/backpressure and at most two occupied slots. Those multi-clip
starts needed no waits, so they do not exercise the runtime multi-wait maximum.
The shared, non-renewing deadline is covered by the policy unit tests.

The 60-second fractional-seek hold passed at approximately 60 distinct pictures/s,
recovering six initial misses with maximum observed readiness duration 0.412 ms.
The finer sweep covered 36 eight-second windows and recovered **628 initial
misses**. One window, `app-seek-phase-0-offset3`, delivered **59.875 fps**, with
one empty take and two replacements. Its 342 completed readiness attempts
recovered 341 pictures; the longest observed timeout was 1.274 ms, and the next
publication followed the empty take by 0.0072 ms. The observed publication was
therefore about 1.281 ms after the attempt. Worker/consumer spacing remained
regular, unlike the original coarse-wait failure. Full trace summaries retain
this failure; no counters or thresholds were changed to conceal it.

That measured tail motivates the next candidate's 1.5 ms shared cap. It needs
its own release qualification; this bundle does not establish that it passes.

Validation: 27 CPU checks pass (one existing timing check ignored), 193 video
tests report pass (two optional HAP fixture checks return early), six picture/
seek/pool groups pass with 30 comparisons, and interlaced fallback passes.
The 180.0004-second playlist soak shows six clips, 11 seeks, one reload and
10,803 GPU pictures; peak RSS is 360,370,176 bytes. Settings hashes match.
Experimental status/defaults, shader, colour thresholds, three GPU leases and
zero supported-path picture transfers remain unchanged.

Untraced harness CPU medians are 3.67% of one core at 1080p and 1.80% at 4K,
with broad variation between starts. Untraced app estimates are about 60 fps
by source-playhead time, with no interval discards. App CPU/cycle readings are
saved separately; no isolated CPU improvement is claimed. RPC and playhead
endpoints are asynchronous, and neither is presentation timing.

Hashes, raw records, app samples, trace analyses, reproduction scripts, prior
candidate history and local trace hashes accompany this file. Exact binaries,
source snapshots and full traces remain in
`target/gpu-pacing-fix-20261120-readiness/`. GUI runs are API driven and obscured;
foreground and physical scanout remain unqualified. No push was performed.
