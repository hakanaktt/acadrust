//! Rasterize TrueType runs into an atlas and world-space quads.

use crate::types::{Matrix3, Vector3};

use super::scene::GlyphRun;
use super::View;

pub(crate) struct AtlasQuad {
    pub corners: [Vector3; 4],
    pub uv: [f32; 4],
    pub rgb: [u8; 3],
    pub aci7: bool,
    pub opacity: f32,
}

pub(crate) struct BuiltAtlas {
    pub size: u32,
    pub pixels: Vec<u8>,
    pub quads: Vec<AtlasQuad>,
}

pub(crate) fn build_atlas(
    runs: &[GlyphRun],
    view: &View,
    viewport_px: (u32, u32),
    mut font_at: impl FnMut(&std::path::Path) -> Option<fontdue::Font>,
) -> BuiltAtlas {
    let size = 4096u32;
    let mut pixels = vec![0u8; (size * size) as usize];
    let mut cursor_x = 1u32;
    let mut cursor_y = 1u32;
    let mut row_height = 0u32;
    let mut quads = Vec::new();
    let pixels_per_world = viewport_px.1.max(1) as f64 / view.height.max(1e-6);
    let mut fonts = Vec::new();
    for run in runs {
        if !fonts.iter().any(|(path, _)| path == &run.font_path) {
            if let Some(font) = font_at(&run.font_path) {
                fonts.push((run.font_path.clone(), font));
            }
        }
        let Some((_, font)) = fonts.iter().find(|(path, _)| path == &run.font_path) else {
            continue;
        };
        // Keep the raster atlas within a useful size range, but map its pixels
        // back to the requested world-space text height.  Using the viewport
        // scale here after clamping makes small text grow and large text shrink.
        let pixel_height = (run.height * pixels_per_world).clamp(4.0, 96.0) as f32;
        let world_per_raster_pixel = run.height / pixel_height.max(1.0) as f64;
        let mut placed = Vec::new();
        let width_factor = run.width_factor.abs();
        let mut advance = 0.0f32;
        for ch in run.text.chars() {
            let (metrics, bitmap) = font.rasterize(ch, pixel_height);
            if metrics.width == 0 || metrics.height == 0 {
                advance += metrics.advance_width * width_factor as f32;
                continue;
            }
            if cursor_x + metrics.width as u32 + 1 >= size {
                cursor_x = 1;
                cursor_y += row_height + 1;
                row_height = 0;
            }
            if cursor_y + metrics.height as u32 + 1 >= size {
                break;
            }
            blit(
                &mut pixels,
                size,
                cursor_x,
                cursor_y,
                &bitmap,
                metrics.width,
                metrics.height,
            );
            placed.push((cursor_x, cursor_y, metrics, advance));
            cursor_x += metrics.width as u32 + 1;
            row_height = row_height.max(metrics.height as u32);
            advance += metrics.advance_width * width_factor as f32;
        }
        let width = advance as f64 * world_per_raster_pixel;
        let shift_x = match run.horizontal {
            1 | 4 => -width * 0.5,
            2 => -width,
            _ => 0.0,
        };
        let shift_y = match run.vertical {
            2 | 4 => -run.height * 0.5,
            3 => -run.height,
            _ => 0.0,
        };
        if run.horizontal == 4 {
            // Middle alignment is centered both ways. `shift_x` already covers x.
        }
        let basis = Matrix3::arbitrary_axis(run.normal);
        let rotation = Matrix3::rotation_z(run.rotation);
        for (x, y, metrics, pen) in placed {
            let left = pen as f64 * world_per_raster_pixel
                + metrics.xmin as f64 * width_factor * world_per_raster_pixel
                + shift_x;
            let bottom = metrics.ymin as f64 * world_per_raster_pixel + shift_y;
            let right = left + metrics.width as f64 * width_factor * world_per_raster_pixel;
            let top = bottom + metrics.height as f64 * world_per_raster_pixel;
            let corner = |mut x: f64, mut y: f64| {
                if run.backward {
                    x = -x;
                }
                if run.upside_down {
                    y = -y;
                }
                x += y * run.oblique.tan();
                let axis_x = if run.axis_x.length_squared() > 1e-12 {
                    run.axis_x
                } else {
                    basis * (rotation * Vector3::UNIT_X)
                };
                let axis_y = if run.axis_y.length_squared() > 1e-12 {
                    run.axis_y
                } else {
                    basis * (rotation * Vector3::UNIT_Y)
                };
                run.anchor + axis_x * x + axis_y * y
            };
            let u0 = x as f32 / size as f32;
            let v0 = y as f32 / size as f32;
            let u1 = (x + metrics.width as u32) as f32 / size as f32;
            let v1 = (y + metrics.height as u32) as f32 / size as f32;
            quads.push(AtlasQuad {
                corners: [
                    corner(left, top),
                    corner(right, top),
                    corner(right, bottom),
                    corner(left, bottom),
                ],
                uv: [u0, v0, u1, v1],
                rgb: run.rgb,
                aci7: run.aci7,
                opacity: run.opacity,
            });
        }
    }
    BuiltAtlas {
        size,
        pixels,
        quads,
    }
}

fn blit(atlas: &mut [u8], size: u32, x: u32, y: u32, bitmap: &[u8], width: usize, height: usize) {
    for row in 0..height {
        let src = row * width;
        let dst = ((y as usize + row) * size as usize) + x as usize;
        let end = dst + width;
        if end <= atlas.len() && src + width <= bitmap.len() {
            atlas[dst..end].copy_from_slice(&bitmap[src..src + width]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Handle;

    fn test_font() -> Option<std::path::PathBuf> {
        [
            r"C:\Windows\Fonts\arial.ttf",
            r"C:\Windows\Fonts\calibri.ttf",
        ]
        .into_iter()
        .map(std::path::PathBuf::from)
        .find(|path| path.is_file())
    }

    fn rasterized_height(run: &GlyphRun, view: &View, viewport_px: (u32, u32)) -> f64 {
        let path = run.font_path.clone();
        let atlas = build_atlas(std::slice::from_ref(run), view, viewport_px, move |_| {
            let bytes = std::fs::read(&path).ok()?;
            fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).ok()
        });
        let quad = atlas.quads.first().expect("test glyph was not rasterized");
        let min = quad
            .corners
            .iter()
            .map(|point| point.y)
            .fold(f64::INFINITY, f64::min);
        let max = quad
            .corners
            .iter()
            .map(|point| point.y)
            .fold(f64::NEG_INFINITY, f64::max);
        max - min
    }

    #[test]
    fn clamped_raster_size_preserves_ttf_world_height() {
        let Some(font_path) = test_font() else {
            return;
        };
        let run = GlyphRun {
            text: "A".to_string(),
            anchor: Vector3::ZERO,
            rotation: 0.0,
            height: 10.0,
            width_factor: 1.0,
            horizontal: 0,
            vertical: 0,
            normal: Vector3::UNIT_Z,
            axis_x: Vector3::UNIT_X,
            axis_y: Vector3::UNIT_Y,
            oblique: 0.0,
            backward: false,
            upside_down: false,
            font_path,
            rgb: [255, 255, 255],
            aci7: false,
            opacity: 1.0,
            source: super::super::scene::EntityRef {
                handle: Handle::new(1),
                insert_path: Vec::new(),
            },
        };
        let unclamped = rasterized_height(
            &run,
            &View {
                height: 100.0,
                ..View::default()
            },
            (100, 100),
        );
        let clamped = rasterized_height(
            &run,
            &View {
                height: 1.0,
                ..View::default()
            },
            (1000, 1000),
        );
        assert!(
            (unclamped - clamped).abs() < 1.0,
            "raster clamp changed world glyph height: unclamped={unclamped}, clamped={clamped}"
        );
    }
}
