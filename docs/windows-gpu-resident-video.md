# Windows GPU-resident file playback

## Implementation and current qualification status

With the experimental Windows GPU-decoding preference enabled, file and playlist
decks try D3D12VA on the renderer's own DX12 device. Hardware capability is checked
per codec/profile; other backends retain the existing decode path. The software
override still takes precedence. Streams and other source types are unchanged.

The path is D3D12 decode → owned GPU plane copy → WGSL colour conversion → the
existing sRGB video source texture → composition. It does not download or upload
uncompressed pictures. GPU-to-GPU plane copies are intentional: display never
holds a decoder reference surface and cannot exhaust FFmpeg's reference pool.

NV12 and P010 cover 8-bit and 10-bit SDR, PQ/HDR10 and HLG. The shader uses the
existing range/matrix defaults and HDR-to-SDR tone-map policy (203-nit reference
white), chroma placement, visible cropping, sample-aspect scaling and rotation.
The sRGB attachment and subsequent sampling apply the transfer exactly once.

Interlaced clips and ping-pong playback reopen in software at the current
position, preserving existing field processing and RAM-cache behaviour. A native
failure also reopens in software. If D3D12VA cannot be selected at startup, the
existing D3D11VA/CPU path remains available. Renderer device loss still belongs
to the renderer's recovery path; software decoding cannot repair a lost device.

**Hardware qualification is in progress on an NVIDIA RTX 3060 Ti.** Fifteen
picture/seek fixture groups, interlaced fallback and 177 video regression tests
have passed in release. Thirty single-clip performance measurements and concurrent
4K stress checks are complete; the two-hour soak was interrupted after 23 minutes
to free the GPU for a Premiere export and remains pending. See the
[qualification results](windows-gpu-resident-qualification.md). Keep the feature
experimental and off by default until wider hardware validation.

## Ownership and synchronization

Each player has at most three owned plane slots. The decode worker waits on the
FFmpeg frame fence, copies the selected array slice's two planes on its private
direct queue, restores decoder subresources to COMMON, and waits for that short
copy off the engine/UI threads. Native submissions use the renderer's surface
configuration gate. No native commands write a slot while wgpu owns its lease.

Only completed copies are published. wgpu imports the initialized planes as
COPY_DST, converts them, and explicitly returns them to COPY_DST. Its consuming
command buffer's completion callback releases the imported wrappers and slot.
An unconsumed mailbox frame can be dropped immediately. Dropping a source or
unsubmitted encoder also retains/reclaims resources through their owners.

When the pool is full, the player retains its pending decoded picture and retries
on a later tick, including while paused. Resolution/format changes wait for all
old slots to finish before rebuilding the pool. A timeout retains native command
resources on an asynchronous reaper until completion or device removal.

Seek commands invalidate the mailbox generation before entering
the decoder's command queue. A picture produced by older work cannot replace the
requested picture. Existing show clocks, frame pacing and playlist scheduling
remain in control.

## Diagnostics

The video info line distinguishes **GPU-resident**, **hardware decode · CPU
transfer**, and **software decode**. Its tooltip explains fallback and reports
GPU frames, plane copies, pool occupancy, skipped frames and frame-transfer bytes.
Engine/API playback snapshots add `gpu_resident` and `delivery`; saved shows do
not change. Counters are per player and cumulative, so a fallback can leave GPU
counts from earlier playback. The `gpu_resident` flag describes the current path.

`copy_wait_ns` measures elapsed worker copy/fence time, not GPU timestamp queries.
Frame-transfer bytes describe uncompressed picture data; small colour uniforms
and compressed input packets are not frame transfers. Pool backpressure and
discarded GPU payloads have separate counters. Picture-test readback is expressly
test-only and does not pass through production transfer counters.

## Safe CPU checks

After loading `.windows-tools/build-env.ps1`, these commands never initialize a
GPU or launch Nida:

```powershell
cargo check --offline --tests --no-default-features --features screen-capture,windows-browser -j 1
cargo test --offline --manifest-path tools/gpu-video-checks/Cargo.toml --target-dir target/gpu-video-cpu-checks -j 1
```

The small harness includes production source modules directly. It validates the
actual WGSL with Naga and tests range/bit-depth/HDR metadata, crop handling, pool
limits, allocation failure, lifetime preservation and slot reuse. Existing CPU
tone-map and seek-generation race tests run there too. Fourteen CPU tests passed;
the ignored 4K timing test remains unrun.

Windows test-target compilation and production-library Clippy with `-D warnings`
passed. Full test-target Clippy stops on an existing `assert_is_empty` warning at
`tests/audio_mix.rs:697`; that unrelated test was not changed. Vendored wgpu-hal
also emits its existing build-script macro warnings.

## Hardware qualification — only after the other render finishes

Start with NVIDIA. Use software-generated fixtures covering H.264, HEVC, AV1,
NV12/P010, 1080p60/4K60, limited/full range, SDR/HDR10/HLG, rotation, padding,
chroma edges and interlacing. Record fixture hashes, GPU and driver versions.
Do not generate NVENC fixtures while protecting another GPU render.

```powershell
./scripts/test-gpu-resident-video.ps1 -RenderFinished -Clips C:/fixtures/h264.mp4,C:/fixtures/hevc10-hdr.mp4
./scripts/test-gpu-resident-video.ps1 -RenderFinished -Mode Benchmark -Clips C:/fixtures/4k60.mp4 -Seconds 60 -ClipCount 1
```

The picture test explicitly requires GPU residency, compares five seek positions
against software at the existing mean-difference limit of 0.5 byte/channel,
checks backpressure/reuse and verifies ping-pong fallback. Use progressive fixtures
for this test: interlaced material intentionally falls back. Hardware tests are
ignored by default and run serially; a software fallback cannot count as success.

The ignored real-time benchmark now supports `VARDA_REQUIRE_GPU_RESIDENT=1` and
emits per-player delivery counters with its existing CPU/RSS/frame timing JSON.
For baselines, omit that variable: `VARDA_REQUIRE_HW_DECODE=1` selects the existing
D3D11VA download path, and omitting both selects software. Run five interleaved
60-second release measurements for each path and resolution, then multiple clips.

Before release qualification, require zero production frame transfers on supported
clips, lower CPU than existing hardware playback, no additional dropped frames,
and no seek/UI regression. Manually check pause/step, speed, loops, playlist
transitions, interlaced fallback, removal, reload and shutdown. Complete a two-hour
playback/seek/playlist soak with bounded memory and no validation faults, corruption
or deadlocks. Wider AMD/Intel/hybrid-device validation is required before removing
the Windows beta label or changing the default.
