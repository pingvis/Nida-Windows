//! Byte-preserving box resampling for CPU-backed screen capture.

/// Box-filter downscale of a tightly-packed 4-bytes-per-pixel image.
///
/// Averaging rather than nearest-neighbour matters here: desktop content is
/// full of one-pixel text stems, and point sampling a 4K display down to deck
/// resolution makes them shimmer as the source scrolls. Channel order is
/// irrelevant, so this serves both RGBA and BGRA callers.
#[inline(never)]
pub fn downscale(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    let (dst_w, dst_h) = (dst_w.max(1), dst_h.max(1));
    let mut out = vec![0u8; (dst_w as usize) * (dst_h as usize) * 4];
    let x_ratio = src_w as f32 / dst_w as f32;
    let y_ratio = src_h as f32 / dst_h as f32;
    let stride = src_w as usize * 4;
    let complete = src.len() >= stride * src_h as usize;
    for y in 0..dst_h {
        let y0 = ((y as f32) * y_ratio) as u32;
        let y1 = ((((y + 1) as f32) * y_ratio) as u32).clamp(y0 + 1, src_h);
        for x in 0..dst_w {
            let x0 = ((x as f32) * x_ratio) as u32;
            let x1 = ((((x + 1) as f32) * x_ratio) as u32).clamp(x0 + 1, src_w);
            let o = ((y as usize) * (dst_w as usize) + (x as usize)) * 4;
            if complete {
                let offset = y0 as usize * stride + x0 as usize * 4;
                let pixel = match (x1 - x0, y1 - y0) {
                    (1, 1) => Some(average_box::<1, 1>(src, stride, offset)),
                    (2, 1) => Some(average_box::<2, 1>(src, stride, offset)),
                    (1, 2) => Some(average_box::<1, 2>(src, stride, offset)),
                    (2, 2) => Some(average_box::<2, 2>(src, stride, offset)),
                    _ => None,
                };
                if let Some(pixel) = pixel {
                    out[o..o + 4].copy_from_slice(&pixel);
                    continue;
                }
            }
            let mut acc = [0u32; 4];
            let mut n = 0u32;
            for sy in y0..y1 {
                let row = (sy as usize) * (src_w as usize) * 4;
                for sx in x0..x1 {
                    let i = row + (sx as usize) * 4;
                    if i + 3 < src.len() {
                        acc[0] += u32::from(src[i]);
                        acc[1] += u32::from(src[i + 1]);
                        acc[2] += u32::from(src[i + 2]);
                        acc[3] += u32::from(src[i + 3]);
                        n += 1;
                    }
                }
            }
            // Desktop-to-deck reductions commonly average one, two, or four
            // pixels (for example 2560x1080 to 1920x810, or 4K to 1080p).
            // Keep those exact integer averages free of per-channel division.
            let pixel = match n {
                0 => [0; 4],
                1 => acc.map(|v| v as u8),
                2 => acc.map(|v| (v >> 1) as u8),
                4 => acc.map(|v| (v >> 2) as u8),
                _ => acc.map(|v| (v / n) as u8),
            };
            out[o..o + 4].copy_from_slice(&pixel);
        }
    }
    out
}

/// Fixed small boxes let the compiler unroll the reads and replace division
/// with shifts, while preserving the generic filter's exact integer rounding.
#[inline]
fn average_box<const W: usize, const H: usize>(
    src: &[u8],
    stride: usize,
    offset: usize,
) -> [u8; 4] {
    let mut sum = [0u32; 4];
    for y in 0..H {
        for x in 0..W {
            let i = offset + y * stride + x * 4;
            let pixel = &src[i..i + 4];
            for channel in 0..4 {
                sum[channel] += u32::from(pixel[channel]);
            }
        }
    }
    sum.map(|value| (value / (W * H) as u32) as u8)
}
