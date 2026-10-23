//! Synthetic screen-capture scaling timings; no desktop pixels or audio.
use nida_capture_pixels::downscale;

fn main() {
    for (w, h, dw, dh) in [
        (3840u32, 2160u32, 1920u32, 1080u32),
        (2560, 1440, 1920, 1080),
        (1920, 1080, 1280, 720),
    ] {
        let src: Vec<u8> = (0..w as usize * h as usize * 4)
            .map(|i| ((i * 31 + i / 17) % 256) as u8)
            .collect();
        let mut times = vec![];
        let mut checksum = 0u64;
        for _ in 0..7 {
            let started = std::time::Instant::now();
            let out = std::hint::black_box(downscale(std::hint::black_box(&src), w, h, dw, dh));
            times.push(started.elapsed().as_secs_f64() * 1000.0);
            checksum = out.iter().fold(0u64, |sum, v| {
                sum.wrapping_mul(31).wrapping_add(u64::from(*v))
            });
        }
        times.sort_by(f64::total_cmp);
        println!(
            "{w}x{h},{dw}x{dh},{:.3},{:.3},{checksum}",
            times[3], times[6]
        );
    }
}
