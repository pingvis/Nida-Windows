# Windows capture and rendering performance

The measured bottleneck was CPU box resampling in debug builds. The render
loop could report 60 fps while display-capture workers delivered only a few new
frames per second: scaling a 4K image to 1080p consumed about half a second.

The byte-for-byte existing box filter now lives in the small `nida-capture-pixels`
path dependency. Only that dependency uses optimization level 3 in debug builds;
the application remains debuggable. A non-inlined public boundary keeps the pixel
loop out of unoptimized callers. Geometry, crop, channel order, alpha, averaging,
rate gating, latest-frame delivery and saved-show settings are unchanged. Release
builds already optimized this loop, so this does not claim a release-mode gain.
The shared CPU capture backends benefit; macOS still scales in ScreenCaptureKit.

## Measurements

Windows debug profile, the same machine and three synthetic image sizes. Kernel
checksums match before/after compiler optimization. Native readback measures the
actual D3D11 cropped staging copy, map, row packing and scaling; it checks every
pixel including a non-opaque alpha value. No desktop images or audio are used.
Timings include allocation and exclude verification. Native readback medians discard
three warm-up frames; conversion medians use seven iterations.

| Source -> delivered | Baseline kernel | Optimized compiler reference | Baseline D3D11 readback | Optimized D3D11 readback |
|---|---:|---:|---:|---:|
| 3840x2160 -> 1920x1080 | 541.5 ms | 11.3 ms | 528.7 ms | 26.2 ms |
| 2560x1440 -> 1920x1080 | 389.2 ms | 8.3 ms | 378.9 ms | 15.6 ms |
| 1920x1080 -> 1280x720 | 188.1 ms | 4.0 ms | 182.0 ms | 8.1 ms |

The standalone library measured 13.1 / 9.6 / 4.8 ms in the same case order; all
three checksums match the baseline. Its own Clippy and benchmark compile pass.
A normalized source comparison confirms the filter body is unchanged.

Native readback improved by about 20-24 times. A 26.2 ms 4K-to-1080p result fits
a 30 fps budget but does not establish sustained 60 fps capture. All 51 capture
regressions and production Clippy pass. A renderer API baseline from the isolated picker app
reported 60.04 fps (60 target), mean mixer 2.43 ms, publish 0.55 ms, UI 4.05 ms,
and mean frame interval 16.67 ms. These are CPU/pacing counters, not GPU timestamp
measurements. GPU timestamp groups were empty; no claim about shader GPU speed is
made. This establishes a capture conversion bottleneck separately from rendering.

## Reproduce

```powershell
# Pure conversion, no FFmpeg, capture permission, desktop pixels or audio:
cargo run --manifest-path capture/pixels/Cargo.toml --example resample

# Native GPU staging copy plus scaling, synthetic textures only:
cargo test --locked --no-default-features --features screen-capture,windows-browser --lib native_capture_readback_performance -- --ignored --test-threads=1 --nocapture

# Existing geometry, filter, readback and capture manager regressions:
cargo test --locked --no-default-features --features screen-capture,windows-browser --lib screen_capture -- --test-threads=1
```

Use an otherwise idle machine and repeat timing runs without an active compiler.
The diagnostic test is intentionally ignored by default. Synthetic timings do not
qualify motion smoothness or latency for every display, driver, browser overlay,
output mode or live show. Desktop computer-use app approval timed out during this
session, so no manual source clicks, resize gestures or visual smoothness checks
are claimed. GPU-side downscaling/zero-copy capture remains future work; this change
still reads back the cropped native-size surface before CPU scaling.
