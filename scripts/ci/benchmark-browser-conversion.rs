//! Standalone benchmark of the actual CEF pipe decoder, without GPU/FFmpeg.
//! cargo run --manifest-path browser/protocol/Cargo.toml --example conversion
//! Reports median frame time. --stream measures actual CEF pipe throughput.
use nida_browser_protocol as protocol;

fn main() {
    use std::{hint::black_box, time::Instant};
    if std::env::args().any(|a| a == "--stream") {
        let mut input = std::io::stdin().lock();
        let mut started = None;
        let mut frames = 0u64;
        while let Ok(frame) = protocol::read_message(&mut input) {
            if frame.error.is_some() { continue; }
            let start = started.get_or_insert_with(Instant::now);
            frames += 1;
            black_box(&frame.pixels);
            if start.elapsed().as_secs_f64() >= 6.0 {
                println!("{{\"frames\":{frames},\"seconds\":{:.3},\"decoded_fps\":{:.2}}}",
                    start.elapsed().as_secs_f64(), (frames - 1) as f64 / start.elapsed().as_secs_f64());
                return;
            }
        }
        panic!("browser stream ended before the measurement completed");
    }
    for (width, height) in [(1280u32, 720u32), (1920, 1080)] {
        for alpha in [255u8, 128, 0] {
            let mut packet = b"NDB1".to_vec();
            packet.extend(1u64.to_le_bytes());
            packet.extend(1u64.to_le_bytes());
            packet.extend(width.to_le_bytes());
            packet.extend(height.to_le_bytes());
            packet.extend((width * height * 4).to_le_bytes());
            for _ in 0..width * height {
                packet.extend([alpha / 4, alpha / 2, alpha, alpha]);
            }
            let mut times = Vec::new();
            for _ in 0..12 {
                let start = Instant::now();
                let frame = protocol::read_message(&mut black_box(packet.as_slice())).unwrap();
                black_box(&frame.pixels);
                times.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            times.sort_by(f64::total_cmp);
            let median = (times[5] + times[6]) / 2.0;
            println!("{{\"width\":{width},\"height\":{height},\"alpha\":{alpha},\"median_ms\":{median:.3},\"max_fps\":{:.1}}}", 1000.0 / median);
        }
    }
}
