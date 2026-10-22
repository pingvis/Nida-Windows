//! Local before/after measurements of the production encoder and FLV writer.
#![allow(dead_code)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box;

#[derive(Debug, Clone, Copy)]
enum VideoCodec {
    H264,
    Hevc,
}

#[path = "../../src/internal/streaming/flv.rs"]
mod flv;
#[path = "../../src/internal/streaming/nvenc.rs"]
mod nvenc;

fn pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("flv_packet");
    for bytes in [4096, 16384, 65536] {
        let payload = vec![0x55; bytes];
        let mut writer = flv::FlvWriter::new(VideoCodec::H264);
        drop(writer.frame(0, true, &payload, Some(&[1, 2, 3])));
        group.throughput(Throughput::Bytes(bytes as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(bytes),
            &payload,
            |b, payload| {
                b.iter(|| black_box(writer.frame(1000, false, black_box(payload), None)));
            },
        );
    }
    group.finish();
    // Opening an NVIDIA device is opt-in; packet benchmarks need no GPU.
    if std::env::var_os("NIDA_BENCH_NVENC").is_none() {
        return;
    }
    let mut group = c.benchmark_group("nvenc_1440p60");
    group.sample_size(20);
    let frames: Vec<_> = (0usize..4)
        .map(|shift| {
            let mut frame = vec![128u8; 2560 * 1440 * 3 / 2];
            for y in 0usize..1440 {
                for x in 0usize..2560 {
                    let mut hash = (((x + shift * 7) / 4) as u32).wrapping_mul(2654435761);
                    hash ^= (((y + shift * 3) / 4) as u32).wrapping_mul(2246822519);
                    hash ^= hash >> 13;
                    frame[y * 2560 + x] = (16 + hash % 220) as u8;
                }
            }
            frame
        })
        .collect();
    for kbps in [8000, 32000] {
        let mut encoder = nvenc::NvEncoder::new(2560, 1440, 60, kbps, 2).unwrap();
        let mut slot = 0u64;
        group.bench_function(BenchmarkId::from_parameter(kbps), |b| {
            b.iter(|| {
                let output = encoder
                    .encode(black_box(&frames[slot as usize % 4]), slot)
                    .unwrap();
                slot += 1;
                black_box(output)
            });
        });
        black_box(encoder.finish().unwrap());
    }
    group.finish();
}

criterion_group!(benches, pipeline);
criterion_main!(benches);
