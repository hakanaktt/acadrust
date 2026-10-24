//! Decode the rasters a drawing can carry without inventing pixels.
//!
//! JPEG, PNG, GIF, TIFF, and BMP files go through the `image` codecs. A
//! headerless DIB, the form OLE uses for `CF_DIB`, is read here when the
//! pixels are uncompressed. A file that does not decode is reported. No
//! stand-in picture is substituted.

use std::io::Cursor;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RgbaImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

pub(crate) fn decode(bytes: &[u8]) -> Result<RgbaImage, &'static str> {
    if is_container(bytes) {
        return decode_container(bytes);
    }
    decode_dib(bytes, None).ok_or("bitmap pixels were not decoded")
}

fn is_container(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0xD8, 0xFF])
        || bytes.starts_with(b"GIF8")
        || bytes.starts_with(&[0x89, b'P', b'N', b'G'])
        || bytes.starts_with(b"BM")
        || bytes.starts_with(b"II*\0")
        || bytes.starts_with(b"MM\0*")
}

fn decode_container(bytes: &[u8]) -> Result<RgbaImage, &'static str> {
    let reason = match bytes {
        value if value.starts_with(&[0xFF, 0xD8, 0xFF]) => "jpeg pixels were not decoded",
        value if value.starts_with(b"GIF8") => "gif pixels were not decoded",
        value if value.starts_with(&[0x89, b'P', b'N', b'G']) => "png pixels were not decoded",
        value if value.starts_with(b"II*\0") || value.starts_with(b"MM\0*") => {
            "tiff pixels were not decoded"
        }
        _ => "bitmap pixels were not decoded",
    };
    let reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| reason)?;
    let image = reader.decode().map_err(|_| reason)?;
    let rgba = image.into_rgba8();
    Ok(RgbaImage {
        width: rgba.width(),
        height: rgba.height(),
        pixels: rgba.into_raw(),
    })
}

fn decode_dib(dib: &[u8], pixel_at: Option<usize>) -> Option<RgbaImage> {
    if dib.len() < 40 {
        return None;
    }
    let header = u32::from_le_bytes(dib[0..4].try_into().ok()?) as usize;
    if header < 40 || header > dib.len() {
        return None;
    }
    let width = i32::from_le_bytes(dib[4..8].try_into().ok()?);
    let height_raw = i32::from_le_bytes(dib[8..12].try_into().ok()?);
    let planes = u16::from_le_bytes(dib[12..14].try_into().ok()?);
    let bits = u16::from_le_bytes(dib[14..16].try_into().ok()?);
    let compression = u32::from_le_bytes(dib[16..20].try_into().ok()?);
    if planes != 1 || width <= 0 || height_raw == 0 {
        return None;
    }
    let top_down = height_raw < 0;
    let height = height_raw.unsigned_abs();
    let width = width as u32;
    if width as u64 * height as u64 > 8_000_000 {
        return None;
    }
    if compression != 0 && !(compression == 3 && matches!(bits, 16 | 32)) {
        return None;
    }
    let colors = if bits <= 8 {
        let specified = u32::from_le_bytes(dib.get(32..36)?.try_into().ok()?);
        if specified == 0 {
            1usize << bits
        } else {
            specified as usize
        }
    } else {
        0
    };
    let palette_at = header;
    let palette_bytes = colors.checked_mul(4)?;
    let palette_end = palette_at.checked_add(palette_bytes)?;
    if palette_end > dib.len() {
        return None;
    }
    let pixels_at = pixel_at.unwrap_or(palette_end);
    if pixels_at > dib.len() {
        return None;
    }
    let mut palette = Vec::with_capacity(colors);
    for index in 0..colors {
        let offset = palette_at + index * 4;
        let entry = dib.get(offset..offset + 4)?;
        palette.push([entry[2], entry[1], entry[0], 255]);
    }
    let stride = ((width as usize * bits as usize + 31) / 32) * 4;
    let mut pixels = vec![0u8; width as usize * height as usize * 4];
    for row in 0..height as usize {
        let source_row = if top_down {
            row
        } else {
            height as usize - 1 - row
        };
        let row_bytes = dib.get(pixels_at + source_row * stride..)?.get(..stride)?;
        for column in 0..width as usize {
            let rgba = match bits {
                24 => {
                    let offset = column * 3;
                    let bgr = row_bytes.get(offset..offset + 3)?;
                    [bgr[2], bgr[1], bgr[0], 255]
                }
                32 => {
                    let offset = column * 4;
                    let bgr = row_bytes.get(offset..offset + 4)?;
                    let alpha = if compression == 0
                        && row_bytes.iter().skip(3).step_by(4).all(|byte| *byte == 0)
                    {
                        255
                    } else {
                        bgr[3]
                    };
                    [bgr[2], bgr[1], bgr[0], alpha]
                }
                8 => *palette.get(*row_bytes.get(column)? as usize)?,
                _ => return None,
            };
            let dest = (row * width as usize + column) * 4;
            pixels[dest..dest + 4].copy_from_slice(&rgba);
        }
    }
    Some(RgbaImage {
        width,
        height,
        pixels,
    })
}

pub(crate) fn adjust(image: &mut RgbaImage, brightness: u8, contrast: u8, fade: u8) -> f32 {
    let shift = (brightness as f32 - 50.0) / 50.0;
    let gain = contrast as f32 / 50.0;
    for pixel in image.pixels.chunks_exact_mut(4) {
        for channel in &mut pixel[..3] {
            let value = *channel as f32 / 255.0;
            let adjusted = ((value - 0.5) * gain + 0.5 + shift * 0.5).clamp(0.0, 1.0);
            *channel = (adjusted * 255.0).round() as u8;
        }
    }
    (1.0 - (fade as f32 / 100.0).clamp(0.0, 1.0)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bmp_24(width: u32, height: u32, bgr_rows_bottom_first: &[[u8; 3]]) -> Vec<u8> {
        let stride = ((width as usize * 3 + 3) / 4) * 4;
        let pixels = stride * height as usize;
        let file_size = 54 + pixels;
        let mut bytes = vec![0u8; file_size];
        bytes[0..2].copy_from_slice(b"BM");
        bytes[2..6].copy_from_slice(&(file_size as u32).to_le_bytes());
        bytes[10..14].copy_from_slice(&54u32.to_le_bytes());
        bytes[14..18].copy_from_slice(&40u32.to_le_bytes());
        bytes[18..22].copy_from_slice(&width.to_le_bytes());
        bytes[22..26].copy_from_slice(&height.to_le_bytes());
        bytes[26..28].copy_from_slice(&1u16.to_le_bytes());
        bytes[28..30].copy_from_slice(&24u16.to_le_bytes());
        for (index, pixel) in bgr_rows_bottom_first.iter().enumerate() {
            let row = index / width as usize;
            let column = index % width as usize;
            let offset = 54 + row * stride + column * 3;
            bytes[offset..offset + 3].copy_from_slice(pixel);
        }
        bytes
    }

    fn encode_rgb(format: image::ImageFormat, rgb: [u8; 3]) -> Vec<u8> {
        let image = image::RgbImage::from_raw(1, 1, rgb.to_vec()).unwrap();
        let mut cursor = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image)
            .write_to(&mut cursor, format)
            .unwrap();
        cursor.into_inner()
    }

    #[test]
    fn bmp_bottom_up_red_pixel_lands_at_the_top_left() {
        let bytes = bmp_24(1, 2, &[[0, 0, 255], [0, 255, 0]]);
        let image = decode(&bytes).unwrap();
        assert_eq!(image.width, 1);
        assert_eq!(image.height, 2);
        assert_eq!(&image.pixels[0..4], &[0, 255, 0, 255]);
        assert_eq!(&image.pixels[4..8], &[255, 0, 0, 255]);
    }

    #[test]
    fn neutral_brightness_and_contrast_keep_the_color() {
        let mut image = decode(&bmp_24(1, 1, &[[0, 0, 128]])).unwrap();
        let opacity = adjust(&mut image, 50, 50, 0);
        assert_eq!(opacity, 1.0);
        assert_eq!(&image.pixels[0..3], &[128, 0, 0]);
    }

    #[test]
    fn jpeg_gif_and_tiff_round_trip_a_red_pixel() {
        for format in [
            image::ImageFormat::Jpeg,
            image::ImageFormat::Gif,
            image::ImageFormat::Tiff,
        ] {
            let image = decode(&encode_rgb(format, [255, 0, 0])).unwrap();
            assert_eq!(image.width, 1);
            assert_eq!(image.height, 1);
            assert!(
                image.pixels[0] > 240 && image.pixels[1] < 20 && image.pixels[2] < 20,
                "{format:?} produced {:?}",
                &image.pixels[..4]
            );
        }
    }

    #[test]
    fn a_broken_jpeg_is_reported() {
        let error = decode(&[0xFF, 0xD8, 0xFF, 0xD9]).unwrap_err();
        assert_eq!(error, "jpeg pixels were not decoded");
    }

    #[test]
    fn png_rgb_round_trips_through_zlib() {
        let raw = [0u8, 255, 0, 0, 0u8, 0, 0, 255];
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        std::io::Write::write_all(&mut encoder, &raw).unwrap();
        let idat = encoder.finish().unwrap();
        let mut png = vec![0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1A, b'\n'];
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&1u32.to_be_bytes());
        ihdr.extend_from_slice(&2u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        push_chunk(&mut png, b"IHDR", &ihdr);
        push_chunk(&mut png, b"IDAT", &idat);
        push_chunk(&mut png, b"IEND", &[]);
        let image = decode(&png).unwrap();
        assert_eq!(&image.pixels[0..4], &[255, 0, 0, 255]);
        assert_eq!(&image.pixels[4..8], &[0, 0, 255, 255]);
    }

    fn push_chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        png.extend_from_slice(&(data.len() as u32).to_be_bytes());
        png.extend_from_slice(kind);
        png.extend_from_slice(data);
        let mut crc_bytes = kind.to_vec();
        crc_bytes.extend_from_slice(data);
        png.extend_from_slice(&crc32(&crc_bytes).to_be_bytes());
    }

    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for byte in data {
            crc ^= *byte as u32;
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
        !crc
    }
}
