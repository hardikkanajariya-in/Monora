use crate::capture::frame::CapturedFrame;

const PREVIEW_MAX_WIDTH: u32 = 480;

pub fn preview_jpeg_base64(frame: &CapturedFrame) -> Option<String> {
    let (w, h, bgra) = downscale_bgra(frame, PREVIEW_MAX_WIDTH);
    if w == 0 || h == 0 {
        return None;
    }
    let rgb = bgra_to_rgb(&bgra);
    let img: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
        image::ImageBuffer::from_raw(w, h, rgb)?;
    let mut bytes = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut bytes);
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, 72);
    if encoder
        .encode(img.as_raw(), w, h, image::ExtendedColorType::Rgb8)
        .is_err()
    {
        return None;
    }
    use base64::Engine;
    Some(base64::engine::general_purpose::STANDARD.encode(bytes))
}

fn downscale_bgra(frame: &CapturedFrame, max_width: u32) -> (u32, u32, Vec<u8>) {
    if frame.width == 0 || frame.height == 0 {
        return (0, 0, Vec::new());
    }
    let scale = if frame.width <= max_width {
        1.0f32
    } else {
        max_width as f32 / frame.width as f32
    };
    let w = ((frame.width as f32) * scale).round().max(1.0) as u32;
    let h = ((frame.height as f32) * scale).round().max(1.0) as u32;
    let mut out = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let sx = ((x as f32 / scale).floor() as u32).min(frame.width - 1);
            let sy = ((y as f32 / scale).floor() as u32).min(frame.height - 1);
            let src_i = ((sy * frame.width + sx) * 4) as usize;
            let dst_i = ((y * w + x) * 4) as usize;
            out[dst_i..dst_i + 4].copy_from_slice(&frame.bgra[src_i..src_i + 4]);
        }
    }
    (w, h, out)
}

fn bgra_to_rgb(bgra: &[u8]) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(bgra.len() / 4 * 3);
    for chunk in bgra.chunks_exact(4) {
        rgb.push(chunk[2]);
        rgb.push(chunk[1]);
        rgb.push(chunk[0]);
    }
    rgb
}
