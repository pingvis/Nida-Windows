//! Versioned, bounded pipe protocol shared with browser/cef/main.cpp.
//! Parsing and alpha conversion happen on the pipe reader, never the renderer.
use std::io::{self, Read};

pub const MAX_DIMENSION: u32 = 4096;
pub const MAX_FRAME_BYTES: u32 = MAX_DIMENSION * MAX_DIMENSION * 4;

pub struct Message {
    pub id: u64,
    pub generation: u64,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    pub error: Option<String>,
}

pub fn read_message(input: &mut impl Read) -> io::Result<Message> {
    let mut header = [0u8; 32];
    input.read_exact(&mut header)?;
    let number = |offset| u32::from_le_bytes(header[offset..offset + 4].try_into().unwrap());
    let id = u64::from_le_bytes(header[4..12].try_into().unwrap());
    let generation = u64::from_le_bytes(header[12..20].try_into().unwrap());
    let (width, height, length) = (number(20), number(24), number(28));
    let error = &header[..4] == b"NDE1";
    if error {
        if length > 4096 {
            return Err(io::Error::other("oversized browser error"));
        }
    } else if &header[..4] != b"NDB1"
        || width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || length != width * height * 4
        || length > MAX_FRAME_BYTES
    {
        return Err(io::Error::other("invalid browser frame header"));
    }
    let mut pixels = vec![0; length as usize];
    input.read_exact(&mut pixels)?;
    if error {
        return Ok(Message {
            id,
            generation,
            width,
            height,
            error: Some(String::from_utf8_lossy(&pixels).into_owned()),
            pixels: Vec::new(),
        });
    }
    straighten_bgra(&mut pixels);
    Ok(Message {
        id,
        generation,
        width,
        height,
        pixels,
        error: None,
    })
}

// A non-generic boundary keeps this hot loop in the optimized protocol crate.
// read_message is instantiated for the app's ChildStdout in debug builds; its
// pixel loop must not be inlined back into that unoptimized instantiation.
#[inline(never)]
fn straighten_bgra(pixels: &mut [u8]) {
    // CEF OSR supplies premultiplied BGRA. The deck blit expects straight RGBA
    // in an sRGB texture. Unpremultiply in the browser's byte colour space.
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
        if pixel[3] == 255 {
            continue;
        }
        let alpha = u32::from(pixel[3]);
        for channel in &mut pixel[..3] {
            *channel = if alpha == 0 {
                0
            } else {
                ((u32::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn packet(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
        let mut bytes = b"NDB1".to_vec();
        bytes.extend(7u64.to_le_bytes());
        bytes.extend(3u64.to_le_bytes());
        bytes.extend(width.to_le_bytes());
        bytes.extend(height.to_le_bytes());
        bytes.extend((pixels.len() as u32).to_le_bytes());
        bytes.extend(pixels);
        bytes
    }
    #[test]
    fn converts_translucent_bgra_without_dark_edges() {
        let frame = read_message(&mut &packet(2, 1, &[0, 32, 64, 128, 9, 8, 7, 0])[..]).unwrap();
        assert_eq!(frame.pixels, [128, 64, 0, 128, 0, 0, 0, 0]);
        assert_eq!(
            (frame.id, frame.generation, frame.width, frame.height),
            (7, 3, 2, 1)
        );
        assert!(frame.error.is_none());
    }
    #[test]
    fn rejects_dimensions_lengths_and_truncation() {
        for bytes in [
            packet(u32::MAX, 1, &[]),
            packet(1, 1, &[]),
            packet(1, 1, &[0, 0, 0, 0])[..34].to_vec(),
        ] {
            assert!(read_message(&mut &bytes[..]).is_err());
        }
    }
}
