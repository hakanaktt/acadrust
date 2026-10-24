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
    let size = 2048u32;
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
        let pixel_height = (run.height * pixels_per_world).clamp(4.0, 96.0) as f32;
        let mut placed = Vec::new();
        let mut advance = 0.0f32;
        for ch in run.text.chars() {
            let (metrics, bitmap) = font.rasterize(ch, pixel_height);
            if metrics.width == 0 || metrics.height == 0 {
                advance += metrics.advance_width * run.width_factor as f32;
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
            advance += metrics.advance_width * run.width_factor as f32;
        }
        let world_per_pixel = 1.0 / pixels_per_world as f32;
        let width = advance * world_per_pixel;
        let shift_x = match run.horizontal {
            1 | 4 => -width * 0.5,
            2 => -width,
            _ => 0.0,
        };
        let shift_y = match run.vertical {
            2 | 4 => -run.height as f32 * 0.5,
            3 => -run.height as f32,
            _ => 0.0,
        };
        if run.horizontal == 4 {
            // Middle alignment is centered both ways. `shift_x` already covers x.
        }
        let basis = Matrix3::arbitrary_axis(run.normal);
        let rotation = Matrix3::rotation_z(run.rotation);
        for (x, y, metrics, pen) in placed {
            let left = pen * world_per_pixel + metrics.xmin as f32 * world_per_pixel + shift_x;
            let bottom = metrics.ymin as f32 * world_per_pixel + shift_y;
            let right =
                left + metrics.width as f32 * run.width_factor.abs() as f32 * world_per_pixel;
            let top = bottom + metrics.height as f32 * world_per_pixel;
            let corner = |x: f32, y: f32| {
                let mut x = x as f64;
                let mut y = y as f64;
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
