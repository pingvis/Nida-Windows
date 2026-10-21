//! Native tests import the production encoder and FLV writer unchanged.
#![allow(dead_code)]

#[derive(Debug, Clone, Copy)]
enum VideoCodec {
    H264,
    Hevc,
}

#[path = "../../src/internal/streaming/flv.rs"]
mod flv;
#[path = "../../src/internal/streaming/nvenc.rs"]
mod nvenc;

fn main() {
    println!("Run cargo test --manifest-path tools/stream-bitrate-probe/Cargo.toml");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    #[test]
    #[ignore = "requires NVIDIA NVENC and matching FFmpeg tools"]
    fn production_encoder_preserves_limited_range_colour_bars() {
        // BT.709 limited-range black, white, red, green and blue. Use an
        // unaligned width to exercise FFmpeg's padded luma/chroma strides.
        let (width, height) = (650usize, 180usize);
        let bars = [(16, 128, 128), (235, 128, 128), (63, 102, 240), (173, 42, 26), (32, 240, 118)];
        let mut pixels = vec![0u8; width * height * 3 / 2];
        for y in 0..height {
            for x in 0..width {
                pixels[y * width + x] = bars[x / 130].0;
            }
        }
        for y in 0..height / 2 {
            for x in (0..width).step_by(2) {
                let bar = bars[x / 130];
                pixels[width * height + y * width + x] = bar.1;
                pixels[width * height + y * width + x + 1] = bar.2;
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("colour.flv");
        let mut output = std::fs::File::create(&path).unwrap();
        let mut encoder = nvenc::NvEncoder::new(width as u32, height as u32, 60, 8000, 2).unwrap();
        let mut writer = flv::FlvWriter::new(VideoCodec::H264);
        for slot in 0..30 {
            for picture in encoder.encode(&pixels, slot).unwrap() {
                output.write_all(&writer.frame(flv::slot_ms(picture.slot, 60), picture.key, &picture.units, Some(encoder.config()))).unwrap();
            }
        }
        for picture in encoder.finish().unwrap() {
            output.write_all(&writer.frame(flv::slot_ms(picture.slot, 60), picture.key, &picture.units, Some(encoder.config()))).unwrap();
        }
        drop(output);
        let decode = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-xerror", "-i"])
            .arg(path)
            .args(["-frames:v", "1", "-pix_fmt", "nv12", "-f", "rawvideo", "pipe:1"])
            .output().unwrap();
        assert!(decode.status.success(), "{}", String::from_utf8_lossy(&decode.stderr));
        assert_eq!(decode.stdout.len(), pixels.len());
        for (index, &(luma, cb, cr)) in bars.iter().enumerate() {
            let x = index * 130 + 64;
            for (at, expected) in [(height / 2 * width + x, luma), (width * height + height / 4 * width + x, cb), (width * height + height / 4 * width + x + 1, cr)] {
                assert!(decode.stdout[at].abs_diff(expected) <= 3, "bar {index}, byte {at}: expected {expected}, got {}", decode.stdout[at]);
            }
        }
    }

    #[test]
    #[ignore = "requires NVIDIA NVENC and matching FFmpeg tools; writes only a temporary fixture"]
    fn production_encoder_changes_rate_and_flv_decodes() {
        assert!(
            nvenc::available(),
            "driver must pass the live-rate qualification probe"
        );
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("native.flv");
        let mut output = std::fs::File::create(&path).unwrap();
        let mut encoder = nvenc::NvEncoder::new(1280, 720, 60, 8000, 2).unwrap();
        let mut writer = flv::FlvWriter::new(VideoCodec::H264);
        let mut pixels = vec![128u8; 1280 * 720 * 3 / 2];
        let mut sizes = [0usize; 3];
        let mut slots = Vec::new();
        for slot in 0u64..540 {
            if slot == 180 {
                encoder.request_kbps(2000).unwrap();
            }
            if slot == 360 {
                encoder.request_kbps(8000).unwrap();
            }
            // Moving texture spends rate, identical across the three phases.
            for y in 0usize..720 {
                for x in 0usize..1280 {
                    let mut hash = (((x + slot as usize * 7) / 4) as u32).wrapping_mul(2654435761);
                    hash ^= (((y + slot as usize * 3) / 4) as u32).wrapping_mul(2246822519);
                    hash ^= hash >> 13;
                    hash = hash.wrapping_mul(3266489917);
                    hash ^= hash >> 16;
                    pixels[y * 1280 + x] = (16 + hash % 220) as u8;
                }
            }
            for picture in encoder.encode(&pixels, slot).unwrap() {
                sizes[picture.slot as usize / 180] += picture.units.len();
                slots.push(picture.slot);
                output
                    .write_all(&writer.frame(
                        flv::slot_ms(picture.slot, 60),
                        picture.key,
                        &picture.units,
                        Some(encoder.config()),
                    ))
                    .unwrap();
            }
        }
        for picture in encoder.finish().unwrap() {
            sizes[picture.slot as usize / 180] += picture.units.len();
            slots.push(picture.slot);
            output
                .write_all(&writer.frame(
                    flv::slot_ms(picture.slot, 60),
                    picture.key,
                    &picture.units,
                    Some(encoder.config()),
                ))
                .unwrap();
        }
        drop(output);
        assert_eq!(slots, (0..540).collect::<Vec<_>>());
        assert!(sizes[1] * 2 < sizes[0], "rate must decrease: {sizes:?}");
        assert!(sizes[1] * 2 < sizes[2], "rate must recover: {sizes:?}");
        assert_eq!(encoder.requested_kbps(), 8000);
        let decode = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-xerror", "-i"])
            .arg(&path)
            .args(["-f", "null", "-"])
            .output()
            .unwrap();
        assert!(
            decode.status.success(),
            "{}",
            String::from_utf8_lossy(&decode.stderr)
        );
        let probe = std::process::Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-count_frames",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=nb_read_frames",
                "-of",
                "csv=p=0",
            ])
            .arg(&path)
            .output()
            .unwrap();
        assert!(probe.status.success());
        assert_eq!(String::from_utf8_lossy(&probe.stdout).trim(), "540");
        eprintln!("native production packet bytes by 3s phase: {sizes:?}");
    }
}
