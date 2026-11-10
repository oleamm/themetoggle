//! Embedded tray glyphs.
//!
//! The icon shows the current appearance and is coloured for the surface it is
//! drawn on: a dark sun on a light taskbar, a light moon on a dark taskbar.

use std::io::Cursor;
use std::sync::OnceLock;

const LIGHT_TASKBAR_PNG: &[u8] = include_bytes!("../assets/tray-light.png");
const DARK_TASKBAR_PNG: &[u8] = include_bytes!("../assets/tray-dark.png");

struct DecodedIcon {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

static LIGHT_TASKBAR_ICON: OnceLock<DecodedIcon> = OnceLock::new();
static DARK_TASKBAR_ICON: OnceLock<DecodedIcon> = OnceLock::new();

/// Icon matching the taskbar background, so it never disappears into it.
pub fn tray_icon(dark_taskbar: bool) -> tray_icon::Icon {
    let (cache, bytes) = if dark_taskbar {
        (&DARK_TASKBAR_ICON, DARK_TASKBAR_PNG)
    } else {
        (&LIGHT_TASKBAR_ICON, LIGHT_TASKBAR_PNG)
    };
    let icon = cache.get_or_init(|| decode(bytes));
    tray_icon::Icon::from_rgba(icon.rgba.clone(), icon.width, icon.height)
        .expect("failed to create the tray icon from the embedded PNG")
}

fn decode(bytes: &[u8]) -> DecodedIcon {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder
        .read_info()
        .expect("failed to read the embedded tray icon PNG");
    let mut rgba = vec![
        0;
        reader
            .output_buffer_size()
            .expect("embedded tray icon dimensions overflow")
    ];
    let output = reader
        .next_frame(&mut rgba)
        .expect("failed to decode the embedded tray icon PNG");
    rgba.truncate(output.buffer_size());
    assert_eq!(output.color_type, png::ColorType::Rgba);
    assert_eq!(output.bit_depth, png::BitDepth::Eight);
    DecodedIcon {
        rgba,
        width: output.width,
        height: output.height,
    }
}

#[cfg(test)]
mod tests {
    use super::{DARK_TASKBAR_PNG, LIGHT_TASKBAR_PNG, decode};

    #[test]
    fn both_glyphs_decode_as_square_rgba() {
        for bytes in [LIGHT_TASKBAR_PNG, DARK_TASKBAR_PNG] {
            let icon = decode(bytes);
            assert!(icon.width >= 16);
            assert_eq!(icon.width, icon.height);
            assert_eq!(icon.rgba.len(), (icon.width * icon.height * 4) as usize);
        }
    }

    #[test]
    fn glyphs_are_not_blank() {
        for bytes in [LIGHT_TASKBAR_PNG, DARK_TASKBAR_PNG] {
            let icon = decode(bytes);
            let opaque = icon.rgba.chunks_exact(4).filter(|pixel| pixel[3] > 128).count();
            assert!(opaque > 100, "glyph is almost empty: {opaque} opaque pixels");
        }
    }
}
