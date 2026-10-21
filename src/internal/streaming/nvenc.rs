//! Windows H.264 NVENC with an explicitly owned encoder session.

use anyhow::{bail, Context as _, Result};
use ffmpeg_next as ffmpeg;
use std::cell::Cell;
use std::sync::{Once, OnceLock};

thread_local! {
    static LOG_SCOPE: Cell<(*mut std::ffi::c_void, bool)> = const { Cell::new((std::ptr::null_mut(), false)) };
}

// FFmpeg's NVENC wrapper logs driver reconfiguration failures but may return
// success from send_frame. Preserve its default logging and flag errors only
// for this thread's exact encoder context. No other FFmpeg user installs a
// callback in Nida. Never suppress or reformat the original diagnostics.
unsafe extern "C" fn log_callback(
    context: *mut std::ffi::c_void,
    level: i32,
    format: *const std::ffi::c_char,
    args: ffmpeg::ffi::va_list,
) {
    if level <= ffmpeg::ffi::AV_LOG_ERROR {
        let _ = LOG_SCOPE.try_with(|scope| {
            let (watched, failed) = scope.get();
            scope.set((
                watched,
                failed || (!watched.is_null() && watched == context),
            ));
        });
    }
    // SAFETY: FFmpeg supplied the callback arguments; forward them once.
    unsafe {
        ffmpeg::ffi::av_log_default_callback(context, level, format, args);
    }
}

fn checked_call<T>(
    context: *mut ffmpeg::ffi::AVCodecContext,
    call: impl FnOnce() -> Result<T>,
) -> Result<T> {
    static INSTALLED: Once = Once::new();
    struct Restore((*mut std::ffi::c_void, bool));
    impl Drop for Restore {
        fn drop(&mut self) {
            LOG_SCOPE.with(|scope| scope.set(self.0));
        }
    }
    INSTALLED.call_once(|| {
        // SAFETY: process-lifetime, thread-safe callback with FFmpeg's ABI.
        unsafe {
            ffmpeg::ffi::av_log_set_callback(Some(log_callback));
        }
    });
    let old = LOG_SCOPE.with(|scope| scope.replace((context.cast(), false)));
    let restore = Restore(old);
    let result = call();
    let failed = LOG_SCOPE.with(|scope| scope.get().1);
    drop(restore);
    if failed {
        bail!("NVENC reported an encoder or bitrate reconfiguration error; see the log");
    }
    result
}

/// Prove that the installed driver opens and changes rate in both directions.
/// A codec present in FFmpeg's build alone is not evidence of this capability.
pub(crate) fn available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| match qualify() {
        Ok(()) => true,
        Err(error) => {
            log::warn!("Windows NVENC unavailable: {error:#}");
            false
        }
    })
}

fn qualify() -> Result<()> {
    let mut encoder = NvEncoder::new(320, 180, 60, 1000, 1)?;
    let mut input = vec![128; 320 * 180 * 3 / 2];
    let mut random = 0x2545_f491_u32;
    let mut bytes = [0usize; 3];
    for slot in 0..270 {
        if slot == 90 {
            encoder.request_kbps(300)?;
        }
        if slot == 180 {
            encoder.request_kbps(1000)?;
        }
        for pixel in &mut input[..320 * 180] {
            random ^= random << 13;
            random ^= random >> 17;
            random ^= random << 5;
            *pixel = 16 + u8::try_from(random % 220)?;
        }
        for packet in encoder.encode(&input, slot)? {
            if packet.slot % 90 >= 30 {
                bytes[usize::try_from(packet.slot / 90)?] += packet.units.len();
            }
        }
    }
    for packet in encoder.finish()? {
        if packet.slot % 90 >= 30 {
            bytes[usize::try_from(packet.slot / 90)?] += packet.units.len();
        }
    }
    if bytes[1] == 0 || bytes[0] < bytes[1] * 2 || bytes[2] < bytes[1] * 2 {
        bail!("NVENC driver did not follow the bitrate probe in both directions: {bytes:?}");
    }
    Ok(())
}

/// One encoded H.264 picture, retaining its original stream timeline slot.
pub(crate) struct Picture {
    pub slot: u64,
    pub key: bool,
    pub units: Vec<u8>,
}

/// A single-owner encoder. Calls, including rate changes, stay on its worker.
pub(crate) struct NvEncoder {
    encoder: ffmpeg::encoder::video::Encoder,
    frame: ffmpeg::frame::Video,
    width: usize,
    height: usize,
    config: Vec<u8>,
    requested_kbps: u32,
    next_slot: u64,
}

impl NvEncoder {
    pub fn new(width: u32, height: u32, fps: u32, kbps: u32, keyframe_secs: u32) -> Result<Self> {
        if width == 0
            || height == 0
            || !width.is_multiple_of(2)
            || !height.is_multiple_of(2)
            || fps == 0
        {
            bail!("NVENC needs an even, nonzero picture size and frame rate");
        }
        let bits = bitrate(kbps)?;
        ffmpeg::init().context("initializing FFmpeg for NVENC")?;
        let codec = ffmpeg::encoder::find_by_name("h264_nvenc")
            .context("FFmpeg has no H.264 NVENC encoder")?;
        let mut encoder = ffmpeg::codec::context::Context::new_with_codec(codec)
            .encoder()
            .video()?;
        encoder.set_width(width);
        encoder.set_height(height);
        encoder.set_format(ffmpeg::format::Pixel::NV12);
        encoder.set_time_base((1, i32::try_from(fps)?));
        encoder.set_frame_rate(Some((i32::try_from(fps)?, 1)));
        encoder.set_gop(
            fps.checked_mul(keyframe_secs.max(1))
                .context("NVENC GOP overflow")?,
        );
        encoder.set_max_b_frames(0);
        encoder.set_flags(ffmpeg::codec::Flags::GLOBAL_HEADER);
        encoder.set_bit_rate(bits);
        encoder.set_max_bit_rate(bits);
        // SAFETY: this context is exclusively owned, unopened and valid.
        unsafe {
            let context = &mut *encoder.as_mut_ptr();
            context.rc_buffer_size = i32::try_from(bits)?;
            context.color_primaries = ffmpeg::ffi::AVColorPrimaries::AVCOL_PRI_BT709;
            context.color_trc = ffmpeg::ffi::AVColorTransferCharacteristic::AVCOL_TRC_BT709;
            context.colorspace = ffmpeg::ffi::AVColorSpace::AVCOL_SPC_BT709;
            context.color_range = ffmpeg::ffi::AVColorRange::AVCOL_RANGE_MPEG;
        }
        let mut options = ffmpeg::Dictionary::new();
        options.set("preset", "p4");
        options.set("tune", "ll");
        options.set("rc", "cbr");
        options.set("zerolatency", "1");
        // SAFETY: exclusively owned context; the log scope only compares its address.
        let pointer = unsafe { encoder.as_mut_ptr() };
        let encoder = checked_call(pointer, || Ok(encoder.open_as_with(codec, options)?))
            .context("opening H.264 NVENC on this GPU/driver")?;
        // SAFETY: the opened context owns extradata until it is dropped. Copy
        // it before returning; no borrowed FFmpeg memory leaves this scope.
        let extra = unsafe {
            let context = &*encoder.as_ptr();
            if context.extradata.is_null() || context.extradata_size <= 0 {
                bail!("NVENC supplied no H.264 configuration");
            }
            std::slice::from_raw_parts(context.extradata, usize::try_from(context.extradata_size)?)
                .to_vec()
        };
        let config = avc_config(&extra)?;
        let frame = ffmpeg::frame::Video::new(ffmpeg::format::Pixel::NV12, width, height);
        Ok(Self {
            encoder,
            frame,
            width: width as usize,
            height: height as usize,
            config,
            requested_kbps: kbps,
            next_slot: 0,
        })
    }

    pub fn config(&self) -> &[u8] {
        &self.config
    }

    /// Target requested from FFmpeg, not a claim of successful driver
    /// reconfiguration or measured network throughput.
    pub fn requested_kbps(&self) -> u32 {
        self.requested_kbps
    }

    pub fn request_kbps(&mut self, kbps: u32) -> Result<()> {
        let bits = bitrate(kbps)?;
        self.encoder.set_bit_rate(bits);
        self.encoder.set_max_bit_rate(bits);
        // SAFETY: this worker exclusively owns the live context. FFmpeg's
        // NVENC wrapper reads these fields when the next frame is submitted.
        unsafe {
            (*self.encoder.as_mut_ptr()).rc_buffer_size = i32::try_from(bits)?;
        }
        self.requested_kbps = kbps;
        Ok(())
    }

    pub fn encode(&mut self, nv12: &[u8], slot: u64) -> Result<Vec<Picture>> {
        let expected = self
            .width
            .checked_mul(self.height)
            .and_then(|n| n.checked_mul(3))
            .map(|n| n / 2)
            .context("NVENC frame size overflow")?;
        if nv12.len() != expected || slot < self.next_slot {
            bail!("Invalid NVENC picture size or timeline slot");
        }
        // SAFETY: a valid owned frame; make writable before touching buffers
        // that a previous encoder submission may still reference.
        let status = unsafe { ffmpeg::ffi::av_frame_make_writable(self.frame.as_mut_ptr()) };
        if status < 0 {
            return Err(ffmpeg::Error::from(status).into());
        }
        for (plane, rows, offset) in [
            (0, self.height, 0),
            (1, self.height / 2, self.width * self.height),
        ] {
            let stride = self.frame.stride(plane);
            for row in 0..rows {
                self.frame.data_mut(plane)[row * stride..row * stride + self.width]
                    .copy_from_slice(
                        &nv12[offset + row * self.width..offset + (row + 1) * self.width],
                    );
            }
        }
        self.frame.set_pts(Some(i64::try_from(slot)?));
        // SAFETY: the worker exclusively owns this live context.
        let pointer = unsafe { self.encoder.as_mut_ptr() };
        checked_call(pointer, || Ok(self.encoder.send_frame(&self.frame)?))?;
        self.next_slot = slot.checked_add(1).context("NVENC slot overflow")?;
        self.receive()
    }

    pub fn finish(&mut self) -> Result<Vec<Picture>> {
        self.encoder.send_eof()?;
        self.receive()
    }

    fn receive(&mut self) -> Result<Vec<Picture>> {
        let mut output = Vec::new();
        loop {
            let mut packet = ffmpeg::Packet::empty();
            match self.encoder.receive_packet(&mut packet) {
                Ok(()) => {
                    let slot =
                        u64::try_from(packet.pts().context("NVENC packet has no timestamp")?)?;
                    if packet.dts() != packet.pts() {
                        bail!("NVENC reordered a low-latency packet");
                    }
                    output.push(Picture {
                        slot,
                        key: packet.is_key(),
                        units: length_prefixed(packet.data().context("NVENC packet is empty")?)?,
                    });
                }
                Err(ffmpeg::Error::Eof) => return Ok(output),
                Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::error::EAGAIN => {
                    return Ok(output)
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
}

fn bitrate(kbps: u32) -> Result<usize> {
    let bits = kbps
        .checked_mul(1000)
        .filter(|n| *n > 0 && i32::try_from(*n).is_ok())
        .context("NVENC bitrate outside supported integer range")?;
    Ok(usize::try_from(bits)?)
}

/// Split Annex B without assuming all start codes have the same length.
fn annex_b(bytes: &[u8]) -> Result<Vec<&[u8]>> {
    let mut starts = Vec::new();
    let mut at = 0;
    while at + 3 <= bytes.len() {
        let count = if bytes[at..].starts_with(&[0, 0, 0, 1]) {
            4
        } else if bytes[at..].starts_with(&[0, 0, 1]) {
            3
        } else {
            at += 1;
            continue;
        };
        starts.push((at, at + count));
        at += count;
    }
    if starts.is_empty() || bytes[..starts[0].0].iter().any(|b| *b != 0) {
        bail!("NVENC returned invalid Annex B H.264");
    }
    let mut units = Vec::new();
    for (i, &(_, begin)) in starts.iter().enumerate() {
        let mut end = starts.get(i + 1).map_or(bytes.len(), |s| s.0);
        while end > begin && bytes[end - 1] == 0 {
            end -= 1;
        }
        if begin == end {
            bail!("NVENC returned an empty H.264 unit");
        }
        units.push(&bytes[begin..end]);
    }
    Ok(units)
}

fn length_prefixed(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::with_capacity(bytes.len());
    for unit in annex_b(bytes)? {
        output.extend_from_slice(&u32::try_from(unit.len())?.to_be_bytes());
        output.extend_from_slice(unit);
    }
    Ok(output)
}

fn avc_config(bytes: &[u8]) -> Result<Vec<u8>> {
    let units = annex_b(bytes)?;
    let sps: Vec<_> = units.iter().copied().filter(|n| n[0] & 31 == 7).collect();
    let pps: Vec<_> = units.iter().copied().filter(|n| n[0] & 31 == 8).collect();
    let first = sps
        .first()
        .filter(|s| s.len() >= 4)
        .context("NVENC has no complete SPS")?;
    if sps.len() > 31 || pps.is_empty() || pps.len() > 255 {
        bail!("Invalid NVENC parameter sets");
    }
    let mut config = vec![
        1,
        first[1],
        first[2],
        first[3],
        0xff,
        0xe0 | u8::try_from(sps.len())?,
    ];
    for unit in sps {
        config.extend_from_slice(&u16::try_from(unit.len())?.to_be_bytes());
        config.extend_from_slice(unit);
    }
    config.push(u8::try_from(pps.len())?);
    for unit in pps {
        config.extend_from_slice(&u16::try_from(unit.len())?.to_be_bytes());
        config.extend_from_slice(unit);
    }
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logged_driver_errors_are_not_reported_as_success() {
        let codec = ffmpeg::encoder::find_by_name("h264_nvenc").unwrap();
        let mut context = ffmpeg::codec::context::Context::new_with_codec(codec);
        let pointer = unsafe { context.as_mut_ptr() };
        // Exercise the same behavior as NVENC reconfiguration: FFmpeg logs
        // an error for this context while the function returns success.
        let result = checked_call(pointer, || {
            unsafe {
                ffmpeg::ffi::av_log(
                    pointer.cast(),
                    ffmpeg::ffi::AV_LOG_ERROR,
                    c"injected NVENC rejection\n".as_ptr(),
                );
            }
            Ok(())
        });
        assert!(result.is_err());
        assert!(
            checked_call(pointer, || Ok(())).is_ok(),
            "error state must reset for every call"
        );
        assert!(
            checked_call(pointer, || {
                unsafe {
                    ffmpeg::ffi::av_log(
                        std::ptr::null_mut(),
                        ffmpeg::ffi::AV_LOG_ERROR,
                        c"unrelated FFmpeg error\n".as_ptr(),
                    );
                }
                Ok(())
            })
            .is_ok(),
            "other contexts must not poison NVENC"
        );
    }

    #[test]
    fn annex_b_becomes_avc_without_losing_units() {
        let source = [
            0, 0, 0, 1, 0x67, 0x64, 0, 0x33, 0x80, 0, 0, 1, 0x68, 0xaa, 0, 0, 0, 1, 0x65, 0xff,
        ];
        assert_eq!(
            avc_config(&source).unwrap(),
            [1, 0x64, 0, 0x33, 0xff, 0xe1, 0, 5, 0x67, 0x64, 0, 0x33, 0x80, 1, 0, 2, 0x68, 0xaa]
        );
        assert_eq!(
            length_prefixed(&[0, 0, 1, 0x65, 0xff]).unwrap(),
            [0, 0, 0, 2, 0x65, 0xff]
        );
        for invalid in [
            &[][..],
            &[1, 2, 3][..],
            &[0, 0, 1][..],
            &[0, 0, 1, 0x65][..],
        ] {
            assert!(avc_config(invalid).is_err());
        }
        assert!(length_prefixed(&[0, 0, 1, 0, 0, 1, 0x65]).is_err());
    }

    #[test]
    fn invalid_requests_do_not_open_the_driver() {
        assert!(NvEncoder::new(0, 180, 60, 1000, 2).is_err());
        assert!(NvEncoder::new(321, 180, 60, 1000, 2).is_err());
        assert!(NvEncoder::new(320, 180, 0, 1000, 2).is_err());
        assert!(bitrate(0).is_err());
        assert!(bitrate(u32::MAX).is_err());
    }
}
