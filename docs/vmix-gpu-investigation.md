# vMix GPU architecture and a future path for Nida

Investigated 31 October 2026. Nida source baseline: `6a40079`.
Scope: public technical documentation and inspection of this Windows checkout.
No runtime performance measurements or implementation changes were made.

## Finding

vMix uses Direct3D for GPU video processing. This helps it process many pictures
within a frame deadline. It does not execute the whole application on the GPU,
and GPU acceleration alone does not establish crash resistance or broadcast
reliability. Nida already uses the same broad principle: its compositor and
effects run on the GPU through wgpu. This checkout defaults to DX12 on Windows.

The useful future project is **keeping video frames in GPU memory across more
of the pipeline**, while preserving bounded queues and output timing. There is
no evidence here that replacing Nida's renderer with a different graphics API
would deliver the desired benefit.

## The podcast claim

A matching passage appears in the 23 October 2026 WAN Show, *Most Of The Internet
Needs To Be Fixed*, around 1:52:50 in the podcast transcript. The discussion
connects vMix's Windows dependency, GPU drivers and stability, with a speculative
CUDA analogy. This is an informal explanation, not an architecture specification.
Timestamps can differ between podcast and video versions.

- [Episode and official video link](https://whenplane.com/history/show/2026/10/23)
- [Matching transcript](https://pod.enacast.com/c/the-wan-show/most-of-the-internet-needs-to-be-fixed-wan-show-october-23-2026/transcript)
- [Official video](https://www.youtube.com/watch?v=yIcNy60mcAg)

The transcript locates the claim; the technical conclusions below use vendor
documentation and local source instead.

## What is documented about vMix

1. **GPU mixing and processing through Direct3D.** vMix explicitly identifies
   Direct3D as the reason its video mixing engine depends on Windows.
   [vMix: Can vMix run on a Mac?](https://www.vmix.com/knowledgebase/article.aspx/7/can-vmix-run-on-a-mac)
2. **CPU work remains.** Its performance guide describes CPU compression work
   and optional hardware encoding for streams and MP4 recordings. GPU mixing
   and hardware compression are separate stages.
   [vMix: Optimising Performance](https://www.vmix.com/knowledgebase/article.aspx/32/optimising-performance)
3. **Input transfers still matter.** vMix documents system-to-GPU transfer
   pressure for live inputs and optimizations involving Resizable BAR. It
   recommends disabling High Input Performance Mode when ReBAR is enabled,
   because that mode introduces additional memory use and copies.
   [vMix: Resizable BAR support](https://www.vmix.com/knowledgebase/article.aspx/309/improving-gpu-performance-in-vmix-with-resizable-bar-support)
4. **Timing is an explicit tradeoff.** Its display settings offer smoother
   playback with additional delay or lower latency with potentially less smooth
   playback. The documented master clock can drift relative to external gear.
   [vMix 29: Performance](https://www.vmix.com/help29/Performance.html)

These sources do not establish that vMix has an entirely zero-copy pipeline,
requires CUDA for its compositor, uses a secret driver exploit, or universally
transfers capture-card frames directly into GPU memory. Direct3D, CUDA, NVENC
and GPUDirect describe different technologies and should not be conflated.
vMix is proprietary; its complete internal scheduling and memory design cannot
be reconstructed from these public documents.

The engineering inference is that moving pixel operations onto the GPU and
controlling transfer costs creates processing headroom. More headroom can reduce
missed frames. Crash recovery, driver correctness, audio timing, resource
lifetimes and overload handling still require separate engineering and tests.
At 50 fps the frame interval is 20 ms; at 60 fps it is about 16.67 ms. Low average
CPU usage cannot prove every frame met that deadline.

## What Nida already does, and where frames leave the GPU

The following findings come from production source at the baseline above,
rather than assuming older investigation notes describe the current code.

| Stage | Current source behavior | Implication |
| --- | --- | --- |
| GPU backend | `src/internal/renderer/context.rs`, `gpu_instance_descriptor`: Windows defaults to DX12; explicit backend overrides remain supported. | Nida already uses Direct3D on Windows. A proposal to add DX12 is obsolete for this checkout. |
| Composition and effects | `src/internal/mixer/render.rs`, `src/internal/deck/render.rs`, renderer pipelines and shaders. | Retain this GPU renderer as the foundation. |
| Engine scheduling | `src/usecases/ui/runner/engine_thread.rs`; started in `runner/mod.rs`. Engine runs separately from UI layout. | A valuable isolation mechanism already exists; shared locks and surface configuration can still cause delays. |
| Hardware clip decoding | `src/internal/video/hwdecode.rs`: opt-in D3D11VA on Windows; `to_memory` calls `av_hwframe_transfer_data`. `video/mod.rs::convert_decoded` then converts downloaded pictures to RGBA/RGBA64. | Hardware decoding currently saves decode work but retains download, CPU conversion and upload costs. |
| Clip upload | `src/internal/deck/render.rs::update_video_frame` uses staging uploads of CPU frame data. | A native GPU frame path needs a real consumer here, not just a decoder toggle. |
| Window/display capture | `src/internal/screen_capture/platform/windows.rs`: WGC D3D11 texture, cropped staging copy, CPU `Map`, packing and CPU downscaling, followed by GPU upload. | GPU scaling is an incremental opportunity; full texture sharing is a larger follow-up. |
| Output conversion | `src/internal/renderer/nv12.rs`: GPU RGBA-to-NV12 compute conversion into a storage buffer, then asynchronous staging readback. | CPU colour conversion is already avoided for compatible outputs; uncompressed readback remains. |
| Windows stream NVENC | `src/internal/streaming/nvenc.rs::encode` accepts a CPU byte slice and copies its planes into a software NV12 AVFrame. `renderer/subprocess/windows.rs` runs this on a bounded writer path. | Hardware compression is implemented, but its input is not a shared renderer texture. |
| Recording outputs | `src/app/render.rs` converts compatible frames to NV12 or uses the typed readback path, then feeds the output subprocess. | Changing stream NVENC alone would not remove recording-output readback. |
| Windows browser | `browser/cef/main.cpp::OnPaint` copies CPU pixels into packets for transport. | An accelerated browser-frame transport would be a separate project. |
| Readback isolation | `src/internal/renderer/readback.rs`: asynchronous staging state machine and non-blocking polling. | Existing readback should remain available for software encoders and consumers requiring CPU pixels. |

The engine thread shares ownership through a baton; it is not lock-free.
`src/internal/renderer/submit_gate.rs` also deliberately serializes surface
configuration against submissions. GPU residency would not remove these pacing
constraints or make slow disk/network consumers safe automatically.

Simplified current Windows hardware clip path:

```text
Compressed clip -> D3D11 hardware decode -> RAM -> CPU colour conversion
                -> GPU upload -> GPU composition -> display
```

Simplified compatible hardware-encoded output path:

```text
GPU composition -> GPU NV12 buffer -> RAM readback -> encoder input upload
                -> hardware compression -> compressed packets -> disk/network
```

Target for qualified paths:

```text
GPU source surfaces -> GPU conversion/composition -> GPU encoder surfaces
                    -> hardware compression -> compressed packets in RAM
```

"GPU resident" here means avoiding uncompressed frames passing through RAM.
GPU-to-GPU copies, conversion passes, fences and compressed packet handling may
still be necessary. Do not advertise literal zero-copy without measuring the
specific path.

## Proposed implementation sequence

### 1. Establish a release-build transfer and timing baseline

Use an isolated show matching actual production: moving capture-card footage,
clips, two display outputs, recording and streaming where normally used. Record
adapter identity, backend, driver, frame rate, resolution and enabled codecs.

Extend existing diagnostics where needed to distinguish source frame arrivals,
uploads, CPU conversion, readback, encoder input/output, frame age, queue depth,
padding/repeats, drops and actual GPU timings when supported. Report p50/p95/p99
and worst frame times, not just show FPS. An empty GPU timing sample must remain
"unavailable", not be relabeled as CPU-derived GPU time.

Start with 1080p50 and 1080p60. Add 4K only where the intended show needs it.
Compare with the same workload and machine; prior debug capture measurements
are evidence of a past bottleneck, not a new release-build baseline.

### 2. Build one narrow native GPU handoff prototype

Recommended default first candidate: **one H.264 NVENC output accepting a
DX12 GPU resource**, if the baseline shows recording/stream transfer pressure.
It is a contained consumer with a concrete before/after measurement. If capture
or clip conversion dominates instead, prioritize that measured source first.

NVIDIA documents DX12 input resources, resource registration and input/output
fences. This makes the route technically plausible, not already implemented.
[NVIDIA NVENC programming guide](https://docs.nvidia.com/video-technologies/video-codec-sdk/13.0/nvenc-video-encoder-api-prog-guide/index.html)

Keep native interoperability in an internal Windows-specific module. The local
wgpu-hal DX12 implementation exposes `raw_device`, `raw_queue` and
`texture_from_raw` (`vendor/wgpu-hal/src/dx12/device.rs`). wgpu 30 also exposes
unsafe HAL access and texture wrapping. These are low-level building blocks,
not a turnkey safe integration.
[wgpu Device API](https://docs.rs/wgpu/30.0.1/wgpu/struct.Device.html)

Before wiring a real output, prove all of the following in the prototype:

- Same adapter identity across renderer and encoder; reject unsupported
  cross-adapter arrangements instead of silently assuming shared memory.
- A compatible input format. The present NV12 converter produces a packed
  storage buffer, not an encoder-ready planar DX12 texture. Query encoder
  formats and prove the required GPU conversion/copy; do not reinterpret it.
- Explicit resource states and fences, coordinated with wgpu submissions and
  its state tracking. Buffers must not be reused before encoding completes.
- A small fixed surface pool, asynchronous completion and worker-side packet
  delivery. Pool exhaustion must not block the show thread or grow memory.
- Preserved timestamps, Rec.709/range, alpha policy, frame rate and audio sync.
- Capability-qualified selection before a take starts, clear failure reporting
  and a retained CPU-frame path where necessary. Do not silently switch codecs
  or precision mid-take.

Choose between extending the FFmpeg encoder integration with hardware frames
and a narrow direct NVENC adapter after the resource prototype. Preserve current
bitrate controls, packet/muxing behavior and recording lifecycle. This is not a
proposal to replace all FFmpeg use.

### 3. Optimize the source paths independently

- **Capture:** GPU downscale before readback can reduce both CPU work and bytes
  transferred with less architectural change. Then prototype a D3D11 WGC to
  DX12 shared-texture path with proper synchronization and source lifetime.
- **Clips:** keep D3D11VA frames on the GPU, introduce an internally owned GPU
  frame representation alongside CPU frames, and convert NV12/P010 to the
  working colour space on the GPU. Preserve frame timestamps, seek/paused-frame
  behavior, colour metadata, HDR handling and software recovery. Decoder-owned
  surfaces cannot be retained without bounds or recycled while rendering.
- **Browser:** evaluate CEF accelerated paint/shared textures against the exact
  bundled CEF version. Retain process isolation, transparency and restart
  behavior; CPU paint remains a valid fallback.
- **Camera, DeckLink and NDI:** inspect each SDK's delivery contract and measure
  it separately. Do not assume all capture devices or SDKs support GPU sharing.

Microsoft documents D3D11/D3D12 resource sharing and synchronization; native
interop needs ownership, queue and resource-state contracts.
[Microsoft: Direct3D 12 interop](https://learn.microsoft.com/en-us/windows/win32/direct3d12/direct3d-12-with-direct3d-11--direct-2d-and-gdi)

### 4. Qualify behavior under production stress

For each independently reviewable change, compare the release baseline and run
the affected regression suites. Use moving, frame-identifiable content to
distinguish distinct frames from a valid file containing repeated pictures.

Acceptance should include lower transfer/CPU cost, no regression in frame
deadline misses or measured latency, stable memory use, correct colours and
audio sync, and independently stalled consumers that do not stop other outputs.
Exercise start/stop, seek, resize, display changes, disconnect/reconnect,
unsupported formats, exhausted encoder sessions and process failure. Explicitly
test device-loss behavior; retaining CPU readback is not GPU-device recovery.

Use a multi-hour soak appropriate to conference work before changing defaults.
Validate NVIDIA and a non-NVIDIA fallback, plus a hybrid laptop configuration.
No speedup percentage or delivery-date estimate is justified by this source
inspection alone. Native handoffs require several focused changes and hardware
qualification, not one settings switch.

## Planning recommendation

Retain wgpu/DX12, the current engine/UI separation, asynchronous readback and
bounded workers. Add a measured GPU-frame handoff project to future planning,
starting with instrumentation and one qualified path. Avoid a full renderer
rewrite, a CUDA-only compositor, or ReBAR as an application-level substitute
for reducing transfers. ReBAR availability can be reported diagnostically;
vMix's particular memory tuning cannot be assumed to apply to wgpu allocations.

Related historical work: [GPU decoding](windows-gpu-decoding.md),
[GPU recording](windows-gpu-recording.md),
[capture performance](windows-capture-performance.md),
[browser performance](windows-browser-performance.md), and the
[Windows roadmap](windows-roadmap.md). Their recorded measurements retain their
original scope and are not revalidation of this baseline.
