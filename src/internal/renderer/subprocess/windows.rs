//! Windows NVENC on its own writer thread; the render queue stays bounded.

use std::io::Write as _;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Instant;

use super::{FfmpegSubprocess, FrameCounters, NativeShared, NativeVideo, Placed, Timeline};
use crate::streaming::flv::{slot_ms, FlvWriter};
use crate::streaming::nvenc::NvEncoder;
use crate::streaming::{RateMode, RatePlan, StreamEncoder, StreamOptions, VideoCodec};

pub(super) type Opened = NvEncoder;

pub(super) fn open(
    options: &StreamOptions,
    size: (u32, u32),
    fps: u32,
    nv12: bool,
    plan: &RatePlan,
) -> Option<Opened> {
    if plan.encoder != StreamEncoder::NvidiaNvenc {
        return None;
    }
    if !nv12
        || options.codec != VideoCodec::H264
        || plan.mode != RateMode::Constant
        || options.adaptive
    {
        log::warn!("NVENC needs GPU NV12 input, H.264, Constant (CBR) and manual bitrate control; using software");
        return None;
    }
    if !crate::streaming::nvenc::available() {
        return None;
    }
    match NvEncoder::new(
        size.0,
        size.1,
        fps,
        options.video_kbps,
        options.keyframe_secs,
    ) {
        Ok(encoder) => Some(encoder),
        Err(error) => {
            log::warn!("Could not open stream NVENC: {error:#}; using software");
            None
        }
    }
}

#[allow(clippy::too_many_arguments)] // Matches the existing native writer boundary.
pub(super) fn start(
    mut encoder: Opened,
    mut stdin: std::process::ChildStdin,
    rx: mpsc::Receiver<(Instant, Vec<u8>)>,
    (fps, codec, audio_kbps): (u32, VideoCodec, u32),
    counters: FrameCounters,
    (write_failed, shutting_down): (Arc<AtomicBool>, Arc<AtomicBool>),
    label: &str,
    start: Option<Instant>,
) -> (std::thread::JoinHandle<()>, NativeVideo) {
    let shared = NativeShared::new(encoder.requested_kbps(), audio_kbps, fps);
    let native = NativeVideo {
        shared: shared.clone(),
        output: None,
    };
    let label = label.to_owned();
    let writer = std::thread::Builder::new()
        .name("stream-nvenc".into())
        .spawn(move || {
            let mut timeline = Timeline::new(start, f64::from(fps.max(1)));
            let mut flv = FlvWriter::new(codec);
            let mut last: Option<Vec<u8>> = None;
            let mut slot = 0u64;
            let mut write = |pictures: Vec<crate::streaming::nvenc::Picture>,
                             config: &[u8]|
             -> anyhow::Result<()> {
                for picture in pictures {
                    let bytes = flv.frame(
                        slot_ms(picture.slot, fps),
                        picture.key,
                        &picture.units,
                        Some(config),
                    );
                    if let Err(error) = stdin.write_all(&bytes) {
                        FfmpegSubprocess::report_write_error(
                            &error,
                            &shutting_down,
                            &write_failed,
                            &label,
                        );
                        return Err(error.into());
                    }
                    shared.came_out();
                }
                Ok(())
            };
            let result = (|| -> anyhow::Result<()> {
                for (made, frame) in rx {
                    if shutting_down.load(Ordering::SeqCst) {
                        break;
                    }
                    let Placed::After(repeats) = timeline.place(made) else {
                        counters.left_out.fetch_add(1, Ordering::Relaxed);
                        continue;
                    };
                    for n in 0..=repeats {
                        if shutting_down.load(Ordering::SeqCst) {
                            return Ok(());
                        }
                        let picture = if n < repeats {
                            last.as_deref().unwrap_or(&frame)
                        } else {
                            &frame
                        };
                        let wanted = shared.wanted_kbps();
                        if wanted != encoder.requested_kbps() {
                            encoder.request_kbps(wanted)?;
                        }
                        shared.going_in();
                        let pictures = encoder.encode(picture, slot)?;
                        // The context-qualified FFmpeg error hook also checks
                        // reconfiguration errors not returned by send_frame.
                        shared.applied(wanted);
                        write(pictures, encoder.config())?;
                        slot += 1;
                    }
                    counters.padded.fetch_add(repeats, Ordering::Relaxed);
                    counters.written.fetch_add(1, Ordering::Relaxed);
                    last = Some(frame);
                }
                if !shutting_down.load(Ordering::SeqCst) {
                    write(encoder.finish()?, encoder.config())?;
                }
                Ok(())
            })();
            if let Err(error) = result {
                if !shutting_down.load(Ordering::SeqCst) {
                    log::error!("Stream NVENC failed: {error:#}");
                    shared.fail();
                }
            }
        })
        .expect("failed to spawn Windows NVENC writer");
    (writer, native)
}
