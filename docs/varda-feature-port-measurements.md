# Varda feature port measurements

Measured on 31 October 2026 on Windows with an AMD Ryzen 9 5950X and NVIDIA
GeForce RTX 3060 Ti, driver 32.0.16.1742, wgpu DX12. These are local
microbenchmarks, not a comparison with vMix or a guarantee of show frame rate.

## Analyzer readback

`benches/analyzer_capture.rs` measures serial submission, GPU completion,
mapping and CPU row packing. Pipelines and buffers are created before timing.
The baseline copies a full zero-initialized RGBA8 sRGB texture; the port
reduces that texture on the GPU before copying. This isolates transfer/reduction
overhead rather than varying scene content or model inference. Twenty Criterion samples per case, 500 ms warm-up and at least
two seconds of measurement; optimized release profile, thin LTO.

| Source | Full readback | Brightness (256 px long edge) | Face (1920 px long edge) |
| --- | ---: | ---: | ---: |
| 1920×1080 | 3.082 ms | 0.194 ms | 2.890 ms |
| 3840×2160 | 10.151 ms | 0.263 ms | 3.225 ms |

Values are Criterion's point estimates. For 4K, brightness transfers 147,456
bytes instead of 33,177,600 (225 times fewer); the face request transfers
8,294,400 bytes (four times fewer). Measured serial brightness transfer time
falls by about 16 times at 1080p and 39 times at 4K. Application readback is
asynchronous, so these ratios must not be presented as an increase in show FPS.
At 1080p the face request does not reduce dimensions; the timing intervals
overlap the full-readback baseline. Mixed analyzers share the largest requested
capture, so adding face detection raises the brightness analyzer's input size.

## Audio tempo estimation

The corrected estimator was measured with periodic 120 BPM onset envelopes at
48,000/256 hops per second. This times `estimate_bpm`, not the full FFT worker,
capture callback, device latency or accuracy on recorded music.

| Envelope history | Estimation time |
| --- | ---: |
| 4 seconds | 165.71 µs |
| 6 seconds | 372.91 µs |
| 8 seconds | 632.47 µs |

The worker estimates tempo every half second. The eight-second estimate costs
about 0.13% of one core averaged over that interval, excluding all other audio
processing. Synthetic rhythm/sample-rate/queue tests and live music evaluation
are separate from this timing result.

## NDI runtime loopback

An opt-in local sender/receiver test passed using NDI Runtime 6.2.1.0 and the
same DX12 adapter. It discovers only its uniquely named sender, submits 640×360
frames through the real SDK, receives them on Nida's background thread and
checks the resulting GPU texture. The sender and receiver are released by the
test; it does not change another application's source or receiver.

- Opaque UYVY arrived through the GPU conversion path. The central RGBA pixel
  was `[213,54,21,255]`, identical to Nida's CPU conversion reference.
- Transparent RGB input `[200,80,40,128]` arrived as BGRA in this runtime and
  uploaded directly as a BGRA texture. Canonical RGBA readback was
  `[199,79,39,128]`: one code value of network codec colour difference, with
  alpha retained. The adapter supports both RGBA and BGRA output.

NDI's [receive format documentation](https://docs.ndi.video/all/developing-with-ndi/sdk/ndi-recv)
describes separate opaque/alpha layouts. The [frame layout reference](https://docs.ndi.video/all/developing-with-ndi/sdk/frame-types)
specifies straight alpha for RGB frames. GPU receive conversion does not move
NDI network transport or codec decompression onto the GPU; this port moves
opaque UYVY-to-RGB conversion and reduces upload bytes.

```powershell
cargo test --locked --offline --no-default-features --features screen-capture,windows-browser --lib local_ndi_sender_reaches_gpu_receiver -- --ignored --nocapture --test-threads=1
```

This requires an installed NDI runtime, permitted discovery/loopback networking
and a GPU. It does not qualify external senders, network loss or long shows.

## NDI receive conversion/upload cost

`benches/ndi_receive.rs` times a fixed UYVY pattern through Nida's frame copy,
conversion, texture upload and serialized GPU completion. The CPU fallback
allocates/expands RGBA; the GPU path reuses packed staging buffers and converts
with a fragment shader. Pipeline construction and all NDI network/codec work
are excluded. Twenty samples per case, 500 ms warm-up and at least two seconds
of measurement, using the same optimized build and hardware as above.

| Source | CPU fallback | GPU UYVY path |
| --- | ---: | ---: |
| 1920×1080 | 12.058 ms | 1.155 ms |
| 3840×2160 | 47.918 ms | 4.247 ms |

That is about 10–11 times lower elapsed time for this serialized path, not a
comparison against a previous complete NDI receiver or vMix. In particular,
the old SDK RGB receive mode can perform conversion inside the SDK; this
benchmark compares Nida's explicit CPU fallback against its new GPU path.
Opaque UYVY upload bytes are halved relative to RGBA (4,147,200 versus
8,294,400 per 1080p frame). A real show overlaps work and adds network decode,
mixing, effects and output costs, so no corresponding FPS increase is implied.

```powershell
cargo bench --locked --offline --no-default-features --features screen-capture,windows-browser --config 'profile.bench.debug=false' --config 'profile.bench.strip="symbols"' --bench ndi_receive -- --noplot
```

Original output: `target/varda-ports-ndi-benchmarks.log`. The extracted CPU
conversion helper was checked byte-for-byte against its previous body, and
all 46 NDI regressions passed after extraction. The opt-in SDK loopback test
also passed with exact alpha preservation.

## Shutdown qualification

The combined Windows library suite passed 3,888 tests with zero failures and
17 explicitly ignored tests, using normal local permissions, an external
temporary directory and `VARDA_REQUIRE_GPU=1`. A subsequent correction keeps a
late engine frame's app instead of detaching and discarding it after two
seconds. All 69 runner/shutdown checks passed after that correction. A gated
worker proves Quit returns promptly while the worker is held, then recovers
the app and saves its changed crossfader state when released.

Worker-lifetime tickets also cover destruction of captured GPU handles. The
fork's idle effect workers stay tracked until their builder closure is
destroyed, with a gated destructor regression confirming shutdown keeps waiting
during cleanup. Production library and all three changed benchmarks pass
Clippy with warnings denied.

The final optimized release build and an isolated headless smoke passed on
1 November. A real Gaussian blur effect was compiled asynchronously and made
ready before recording. Two 640×360 H.264/AAC MP4 takes (2.55/2.59 seconds) were
fully decoded after exit: one stopped before Quit, the other active at Quit.
Both retained red pixels `[255,89,64]`; forced manual 123 BPM survived the audio
range change. Process exit code was zero, about 451 ms after the shutdown
request. This is one local result, not a promised shutdown deadline.

Evidence: `target/varda-ports-full-library-final.log`,
`target/varda-ports-late-engine-tests.log`,
`target/varda-ports-clippy-qualified.log`,
`target/varda-ports-release-final.log` and
`target/varda-ports/smoke-1793484359380/result.json`.

## Reproduction

Load `.windows-tools/build-env.ps1`, then run:

```powershell
cargo bench --locked --offline --no-default-features --features screen-capture,windows-browser --config 'profile.bench.debug=false' --config 'profile.bench.strip="symbols"' --bench analyzer_capture --bench audio_analysis -- --noplot
```

The original output is `target/varda-ports-benchmarks.log`; Criterion's complete
estimates are under `target/criterion`. Use `--features screen-capture` for an
upstream checkout, which does not have the fork's Windows browser feature.

Native Windows tests also verified float-to-sRGB analyzer capture, GPU versus
CPU NDI colour conversion, packed-row/alpha handling and mixed analyzer input
sharing. The upstream Linux CI run passed the new GPU conversion/capture tests
on its software Vulkan adapter, although the overall jobs fail elsewhere.
Physical macOS/Linux GPU and external NDI source qualification remain separate.

## Upstream CI status

The feature branches are based directly on upstream `0c9643c`, which is still
upstream main as of this validation. The overall upstream checks are red and
must not be presented as passing:

- Running `cargo fmt --all -- --check` on an unmodified checkout of that base
  also fails. Its output is saved in `target/varda-ports/ci-baseline-fmt.log`.
  The ports do not include a repository-wide formatting rewrite.
- Windows Clippy cannot compile the existing unconditional DeckLink
  `libc::clock_gettime` call. Independent production checks of the ports passed
  with a temporary platform guard; that unrelated guard is not in the PRs.
- Linux Clippy reports 23 errors in existing platform stubs, unused code,
  lifetime/closure style and the `UnifiedOutput` enum size. The NDI and shutdown
  jobs report the same set. None names the new receive/capture/analysis modules.
- The Linux library jobs have ten failing tests in unchanged keep-awake,
  DeckLink, presentation, output-layout, streaming, sound-only and slide code.
  The port-specific tests in those jobs pass, including the real GPU tests on
  lavapipe. The workflow requires a GPU and verifies its software Vulkan
  adapter before running tests, so these are not no-adapter skips.

See the [NDI test job](https://github.com/kristijonasatpro/Nida/actions/runs/41223584764/job/124602993922),
[analyzer test job](https://github.com/kristijonasatpro/Nida/actions/runs/41225176404/job/124607716756)
and [shutdown Linux Clippy job](https://github.com/kristijonasatpro/Nida/actions/runs/41225671744/job/124609185836).
Logs are saved under `target/varda-ports/ci-*.log`. Physical source/device tests
and fixing unrelated platform CI failures are separate from these four ports.
