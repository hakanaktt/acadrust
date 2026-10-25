//! wgpu draw path for a compiled [`Scene`](super::Scene).
//!
//! The caller owns the device and the surface. [`Renderer`] uploads strokes
//! once and writes a camera uniform on every frame, so a pan does not rebuild
//! geometry. [`render_rgba`] is the offscreen entry point.

use crate::document::CadDocument;
use crate::types::Vector3;

#[cfg(target_arch = "wasm32")]
use std::{
    cell::RefCell,
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};

use super::scene::Scene;
use super::{CompileOptions, View, DEFAULT_MODEL_LINEWEIGHT_PX_PER_MM};

const INSTANCE_STRIDE: u64 = 64;
const FILL_STRIDE: u64 = 48;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
const GLYPH_STRIDE: u64 = 48;
const CAMERA_SIZE: u64 = 256;
const SHADER: &str = include_str!("shader.wgsl");

/// Clear color and swapchain format for an embedded renderer.
#[derive(Debug, Clone)]
pub struct RendererConfig {
    pub format: wgpu::TextureFormat,
    pub samples: u32,
    pub background: [f32; 4],
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            format: wgpu::TextureFormat::Rgba8Unorm,
            samples: 4,
            // AutoCAD's dark model-space background. ACI 7 contrasts against it.
            background: [33.0 / 255.0, 40.0 / 255.0, 48.0 / 255.0, 1.0],
        }
    }
}

/// Tightly packed RGBA8 image from [`render_rgba`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("no GPU adapter is available")]
    NoAdapter,
    #[error("{0}")]
    Device(String),
}

/// Stroke pipeline. Faces, glyphs, and images are later passes over the same scene.
pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    fill_pipeline: wgpu::RenderPipeline,
    camera_bind: wgpu::BindGroup,
    camera_buf: wgpu::Buffer,
    instances: wgpu::Buffer,
    instance_capacity: u64,
    instance_count: u32,
    fills: wgpu::Buffer,
    fill_capacity: u64,
    fill_count: u32,
    glyph_pipeline: wgpu::RenderPipeline,
    glyph_layout: wgpu::BindGroupLayout,
    glyphs: wgpu::Buffer,
    glyph_capacity: u64,
    glyph_count: u32,
    picture_pipeline: wgpu::RenderPipeline,
    pictures: Vec<PreparedPicture>,
    atlas: wgpu::Texture,
    atlas_view: wgpu::TextureView,
    atlas_bind: wgpu::BindGroup,
    atlas_sampler: wgpu::Sampler,
    ttf_fonts: std::collections::HashMap<std::path::PathBuf, fontdue::Font>,
    msaa: Option<wgpu::Texture>,
    msaa_view: Option<wgpu::TextureView>,
    format: wgpu::TextureFormat,
    samples: u32,
    background: wgpu::Color,
    background_rgba: [f32; 4],
    width: u32,
    height: u32,
    px_per_mm: f32,
    z_scale: f32,
    z_bias: f32,
    depth_bounds: Option<(Vector3, Vector3)>,
    depth: wgpu::Texture,
    depth_view: wgpu::TextureView,
}

impl Renderer {
    pub fn new(device: &wgpu::Device, _queue: &wgpu::Queue, config: &RendererConfig) -> Self {
        let samples = if config.samples >= 4 { 4 } else { 1 };
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("acadrust-render"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("acadrust-camera"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let camera_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("acadrust-camera"),
            size: CAMERA_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("acadrust-camera"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buf.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("acadrust-strokes"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let fill_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("acadrust-fills"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_fill"),
                compilation_options: Default::default(),
                buffers: &[Some(fill_layout())],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(depth_state(true)),
            multisample: wgpu::MultisampleState {
                count: samples,
                ..Default::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_fill"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("acadrust-strokes"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(instance_layout())],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(depth_state(true)),
            multisample: wgpu::MultisampleState {
                count: samples,
                ..Default::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("acadrust-strokes"),
            size: INSTANCE_STRIDE,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let fills = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("acadrust-fills"),
            size: FILL_STRIDE,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let glyph_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("acadrust-atlas"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let glyph_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("acadrust-glyphs"),
                bind_group_layouts: &[Some(&layout), Some(&glyph_layout)],
                immediate_size: 0,
            });
        let glyph_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("acadrust-glyphs"),
            layout: Some(&glyph_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_glyph"),
                compilation_options: Default::default(),
                buffers: &[Some(glyph_layout_vertices())],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(depth_state(false)),
            multisample: wgpu::MultisampleState {
                count: samples,
                ..Default::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_glyph"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let picture_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("acadrust-pictures"),
            layout: Some(&glyph_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_picture"),
                compilation_options: Default::default(),
                buffers: &[Some(picture_layout())],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(depth_state(false)),
            multisample: wgpu::MultisampleState {
                count: samples,
                ..Default::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_picture"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let glyphs = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("acadrust-glyphs"),
            size: GLYPH_STRIDE * 6,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("acadrust-atlas"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let (atlas, atlas_view, atlas_bind) =
            atlas_resources(device, &glyph_layout, &atlas_sampler, 1, &[255]);
        let depth = depth_texture(device, 1, 1, samples);
        let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            pipeline,
            fill_pipeline,
            camera_bind,
            camera_buf,
            instances,
            instance_capacity: INSTANCE_STRIDE,
            instance_count: 0,
            fills,
            fill_capacity: FILL_STRIDE,
            fill_count: 0,
            glyph_pipeline,
            glyph_layout,
            glyphs,
            glyph_capacity: GLYPH_STRIDE * 6,
            glyph_count: 0,
            picture_pipeline,
            pictures: Vec::new(),
            atlas,
            atlas_view,
            atlas_bind,
            atlas_sampler,
            ttf_fonts: std::collections::HashMap::new(),
            msaa: None,
            msaa_view: None,
            format: config.format,
            samples,
            background: to_color(clear_color(config.format, config.background)),
            background_rgba: config.background,
            width: 0,
            height: 0,
            px_per_mm: DEFAULT_MODEL_LINEWEIGHT_PX_PER_MM as f32,
            z_scale: 0.0,
            z_bias: 0.5,
            depth_bounds: None,
            depth,
            depth_view,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.width = width.max(1);
        self.height = height.max(1);
        self.depth = depth_texture(device, self.width, self.height, self.samples);
        self.depth_view = self
            .depth
            .create_view(&wgpu::TextureViewDescriptor::default());
        if self.samples == 1 {
            self.msaa = None;
            self.msaa_view = None;
            return;
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("acadrust-msaa"),
            size: wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: self.samples,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        self.msaa_view = Some(texture.create_view(&wgpu::TextureViewDescriptor::default()));
        self.msaa = Some(texture);
    }

    /// Upload strokes. The camera is not part of the upload.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &Scene,
        _view: &View,
        _viewport_px: (u32, u32),
    ) {
        self.px_per_mm = scene.model_lineweight_px_per_mm as f32;
        let (z_scale, z_bias, bounds) = depth_map(scene);
        self.z_scale = z_scale;
        self.z_bias = z_bias;
        self.depth_bounds = bounds;
        let mut bytes = Vec::new();
        for stroke in &scene.strokes {
            let rgb = if stroke.style.aci7 {
                contrast(self.background_rgba)
            } else {
                stroke.style.rgb
            };
            let color = [
                rgb[0] as f32 / 255.0,
                rgb[1] as f32 / 255.0,
                rgb[2] as f32 / 255.0,
                stroke.style.opacity,
            ];
            let (half_width, mode) = if stroke.style.sheet && scene.sheet_units_per_mm > 0.0 {
                (
                    (stroke.style.lineweight_mm * 0.5 * scene.sheet_units_per_mm) as f32,
                    2.0,
                )
            } else if stroke.style.cosmetic {
                (0.0, 1.0)
            } else {
                ((stroke.style.lineweight_mm * 0.5) as f32, 0.0)
            };
            for pair in stroke.points.windows(2) {
                if !stroke.dot && (pair[1] - pair[0]).length_squared() <= 1e-18 {
                    continue;
                }
                push_segment(&mut bytes, pair[0], pair[1], color, half_width, mode);
            }
        }
        let size = (bytes.len() as u64).max(INSTANCE_STRIDE);
        if size > self.instance_capacity {
            self.instances = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("acadrust-strokes"),
                size,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instance_capacity = size;
        }
        if !bytes.is_empty() {
            queue.write_buffer(&self.instances, 0, &bytes);
        }
        self.instance_count = (bytes.len() as u64 / INSTANCE_STRIDE) as u32;
        self.prepare_fills(device, queue, scene);
        self.prepare_glyphs(device, queue, scene, _view, _viewport_px);
        self.prepare_pictures(device, queue, scene);
    }

    fn prepare_glyphs(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &Scene,
        view: &View,
        viewport_px: (u32, u32),
    ) {
        let built = super::ttf_atlas::build_atlas(&scene.glyphs, view, viewport_px, |path| {
            if !self.ttf_fonts.contains_key(path) {
                let bytes = std::fs::read(path).ok()?;
                let font =
                    fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).ok()?;
                self.ttf_fonts.insert(path.to_path_buf(), font);
            }
            self.ttf_fonts.get(path).cloned()
        });
        let (atlas, atlas_view, atlas_bind) = atlas_resources(
            device,
            &self.glyph_layout,
            &self.atlas_sampler,
            built.size,
            &built.pixels,
        );
        let padded_stride = (built.size as usize).div_ceil(256) * 256;
        let mut padded = vec![0u8; padded_stride * built.size as usize];
        for row in 0..built.size as usize {
            let src = row * built.size as usize;
            let dst = row * padded_stride;
            padded[dst..dst + built.size as usize]
                .copy_from_slice(&built.pixels[src..src + built.size as usize]);
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &atlas,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &padded,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_stride as u32),
                rows_per_image: Some(built.size),
            },
            wgpu::Extent3d {
                width: built.size,
                height: built.size,
                depth_or_array_layers: 1,
            },
        );
        self.atlas = atlas;
        self.atlas_view = atlas_view;
        self.atlas_bind = atlas_bind;
        let mut bytes = Vec::new();
        for quad in &built.quads {
            let corners = [
                quad.corners[0],
                quad.corners[1],
                quad.corners[2],
                quad.corners[0],
                quad.corners[2],
                quad.corners[3],
            ];
            let uvs = [
                (quad.uv[0], quad.uv[1]),
                (quad.uv[2], quad.uv[1]),
                (quad.uv[2], quad.uv[3]),
                (quad.uv[0], quad.uv[1]),
                (quad.uv[2], quad.uv[3]),
                (quad.uv[0], quad.uv[3]),
            ];
            for (corner, uv) in corners.iter().zip(uvs) {
                push_f32(&mut bytes, corner.x as f32);
                push_f32(&mut bytes, corner.y as f32);
                push_f32(&mut bytes, corner.z as f32);
                push_f32(&mut bytes, 1.0);
                push_f32(&mut bytes, uv.0);
                push_f32(&mut bytes, uv.1);
                push_f32(&mut bytes, 0.0);
                push_f32(&mut bytes, 0.0);
                let rgb = if quad.aci7 {
                    contrast(self.background_rgba)
                } else {
                    quad.rgb
                };
                push_f32(&mut bytes, rgb[0] as f32 / 255.0);
                push_f32(&mut bytes, rgb[1] as f32 / 255.0);
                push_f32(&mut bytes, rgb[2] as f32 / 255.0);
                push_f32(&mut bytes, quad.opacity);
            }
        }
        let size = (bytes.len() as u64).max(GLYPH_STRIDE * 6);
        if size > self.glyph_capacity {
            self.glyphs = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("acadrust-glyphs"),
                size,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.glyph_capacity = size;
        }
        if !bytes.is_empty() {
            queue.write_buffer(&self.glyphs, 0, &bytes);
        }
        self.glyph_count = (bytes.len() as u64 / GLYPH_STRIDE) as u32;
    }

    fn prepare_pictures(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, scene: &Scene) {
        self.pictures.clear();
        for picture in &scene.pictures {
            if picture.width == 0
                || picture.height == 0
                || picture.rgba.len() < picture.width as usize * picture.height as usize * 4
                || picture.triangles.len() < 3
                || picture.uvs.len() != picture.triangles.len()
            {
                continue;
            }
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("acadrust-picture"),
                size: wgpu::Extent3d {
                    width: picture.width,
                    height: picture.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let row = picture.width * 4;
            let padded = row.div_ceil(256) * 256;
            let mut upload = vec![0u8; padded as usize * picture.height as usize];
            for y in 0..picture.height as usize {
                let source = y * row as usize;
                let dest = y * padded as usize;
                let next = source + row as usize;
                upload[dest..dest + row as usize].copy_from_slice(&picture.rgba[source..next]);
                if picture.opacity < 0.999 {
                    for pixel in upload[dest..dest + row as usize].chunks_exact_mut(4) {
                        pixel[3] = (pixel[3] as f32 * picture.opacity).round() as u8;
                    }
                }
            }
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &upload,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(picture.height),
                },
                wgpu::Extent3d {
                    width: picture.width,
                    height: picture.height,
                    depth_or_array_layers: 1,
                },
            );
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("acadrust-picture"),
                layout: &self.glyph_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.atlas_sampler),
                    },
                ],
            });
            let mut bytes = Vec::new();
            for (point, uv) in picture.triangles.iter().zip(&picture.uvs) {
                push_f32(&mut bytes, point.x as f32);
                push_f32(&mut bytes, point.y as f32);
                push_f32(&mut bytes, point.z as f32);
                push_f32(&mut bytes, 1.0);
                push_f32(&mut bytes, uv[0]);
                push_f32(&mut bytes, uv[1]);
            }
            let vertices = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("acadrust-picture-vertices"),
                size: bytes.len() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&vertices, 0, &bytes);
            self.pictures.push(PreparedPicture {
                _texture: texture,
                _view: view,
                bind,
                vertices,
                count: picture.triangles.len() as u32,
            });
        }
    }

    fn prepare_fills(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, scene: &Scene) {
        let mut bytes = Vec::new();
        for fill in &scene.fills {
            for (index, point) in fill.triangles.iter().enumerate() {
                let rgb = if fill.background {
                    [
                        (self.background_rgba[0] * 255.0) as u8,
                        (self.background_rgba[1] * 255.0) as u8,
                        (self.background_rgba[2] * 255.0) as u8,
                    ]
                } else if let Some(rgb) = fill.vertex_rgb.get(index) {
                    *rgb
                } else if fill.style.aci7 {
                    contrast(self.background_rgba)
                } else {
                    fill.style.rgb
                };
                let color = [
                    rgb[0] as f32 / 255.0,
                    rgb[1] as f32 / 255.0,
                    rgb[2] as f32 / 255.0,
                    if fill.background {
                        1.0
                    } else {
                        fill.style.opacity
                    },
                ];
                let normal = fill.normals.get(index).copied().unwrap_or(Vector3::ZERO);
                push_f32(&mut bytes, point.x as f32);
                push_f32(&mut bytes, point.y as f32);
                push_f32(&mut bytes, point.z as f32);
                push_f32(&mut bytes, 1.0);
                for channel in color {
                    push_f32(&mut bytes, channel);
                }
                push_f32(&mut bytes, normal.x as f32);
                push_f32(&mut bytes, normal.y as f32);
                push_f32(&mut bytes, normal.z as f32);
                let lighting = match fill.lighting {
                    super::visual_style::LightingModel::None => 0.0,
                    super::visual_style::LightingModel::Constant => 1.0,
                    super::visual_style::LightingModel::Phong => 2.0,
                    super::visual_style::LightingModel::Gooch => 3.0,
                };
                push_f32(&mut bytes, lighting);
            }
        }
        let size = (bytes.len() as u64).max(FILL_STRIDE);
        if size > self.fill_capacity {
            self.fills = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("acadrust-fills"),
                size,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.fill_capacity = size;
        }
        if !bytes.is_empty() {
            queue.write_buffer(&self.fills, 0, &bytes);
        }
        self.fill_count = (bytes.len() as u64 / FILL_STRIDE) as u32;
    }

    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        view: &View,
        viewport_px: (u32, u32),
        queue: &wgpu::Queue,
    ) {
        let width = viewport_px.0.max(1);
        let height = viewport_px.1.max(1);
        queue.write_buffer(
            &self.camera_buf,
            0,
            &camera_bytes(
                view,
                width,
                height,
                self.px_per_mm,
                self.z_scale,
                self.z_bias,
                self.depth_bounds,
            ),
        );
        let (attachment, resolve) = if let Some(msaa) = self.msaa_view.as_ref() {
            (msaa, Some(color))
        } else {
            (color, None)
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("acadrust"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: attachment,
                depth_slice: None,
                resolve_target: resolve,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(self.background),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &self.camera_bind, &[]);
        if !self.pictures.is_empty() {
            pass.set_pipeline(&self.picture_pipeline);
            for picture in &self.pictures {
                pass.set_bind_group(1, &picture.bind, &[]);
                pass.set_vertex_buffer(0, picture.vertices.slice(..));
                pass.draw(0..picture.count, 0..1);
            }
        }
        if self.fill_count > 0 {
            pass.set_pipeline(&self.fill_pipeline);
            pass.set_vertex_buffer(0, self.fills.slice(..));
            pass.draw(0..self.fill_count, 0..1);
        }
        pass.set_pipeline(&self.pipeline);
        if self.instance_count > 0 {
            pass.set_vertex_buffer(0, self.instances.slice(..));
            pass.draw(0..6, 0..self.instance_count);
        }
        if self.glyph_count > 0 {
            pass.set_pipeline(&self.glyph_pipeline);
            pass.set_bind_group(1, &self.atlas_bind, &[]);
            pass.set_vertex_buffer(0, self.glyphs.slice(..));
            pass.draw(0..self.glyph_count, 0..1);
        }
    }
}

/// Compile `doc` and read an RGBA8 image back. Returns [`RenderError::NoAdapter`]
/// when the machine has no wgpu adapter, so headless tests can skip.
pub async fn render_rgba(
    doc: &CadDocument,
    view: &View,
    width: u32,
    height: u32,
    options: &CompileOptions,
) -> Result<RgbaImage, RenderError> {
    render_rgba_with(
        doc,
        view,
        width,
        height,
        options,
        &RendererConfig::default(),
    )
    .await
}

/// [`render_rgba`] with an explicit clear color and sample count.
pub async fn render_rgba_with(
    doc: &CadDocument,
    view: &View,
    width: u32,
    height: u32,
    options: &CompileOptions,
    config: &RendererConfig,
) -> Result<RgbaImage, RenderError> {
    let width = width.max(1);
    let height = height.max(1);
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .map_err(|_| RenderError::NoAdapter)?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
        .map_err(|error| RenderError::Device(error.to_string()))?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = Renderer::new(
        &device,
        &queue,
        &RendererConfig {
            format,
            samples: config.samples,
            background: config.background,
        },
    );
    let scene = super::compile(doc, options);
    renderer.prepare(&device, &queue, &scene, view, (width, height));
    renderer.resize(&device, width, height);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("acadrust-target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    renderer.render(&mut encoder, &target_view, view, (width, height), &queue);
    let padded = padded_bytes_per_row(width);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("acadrust-readback"),
        size: padded * height as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded as u32),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));
    let slice = buffer.slice(..);
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).ok();
        });
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| RenderError::Device(error.to_string()))?;
        receiver
            .recv()
            .map_err(|_| RenderError::Device("readback closed".to_string()))?
            .map_err(|error| RenderError::Device(error.to_string()))?;
    }
    #[cfg(target_arch = "wasm32")]
    map_readback_wasm(slice).await?;
    let mapped = slice
        .get_mapped_range()
        .map_err(|error| RenderError::Device(error.to_string()))?;
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    for row in 0..height as usize {
        let src = row * padded as usize;
        let dst = row * width as usize * 4;
        pixels[dst..dst + width as usize * 4]
            .copy_from_slice(&mapped[src..src + width as usize * 4]);
    }
    drop(mapped);
    buffer.unmap();
    Ok(RgbaImage {
        width,
        height,
        pixels,
    })
}

#[cfg(target_arch = "wasm32")]
struct MapState {
    result: Option<Result<(), wgpu::BufferAsyncError>>,
    waker: Option<Waker>,
}

#[cfg(target_arch = "wasm32")]
struct MapFuture {
    state: Rc<RefCell<MapState>>,
}

#[cfg(target_arch = "wasm32")]
impl Future for MapFuture {
    type Output = Result<(), wgpu::BufferAsyncError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.state.borrow_mut();
        if let Some(result) = state.result.take() {
            Poll::Ready(result)
        } else {
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

#[cfg(target_arch = "wasm32")]
async fn map_readback_wasm(slice: wgpu::BufferSlice<'_>) -> Result<(), RenderError> {
    let state = Rc::new(RefCell::new(MapState {
        result: None,
        waker: None,
    }));
    let callback_state = Rc::clone(&state);
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let mut state = callback_state.borrow_mut();
        state.result = Some(result);
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    });
    MapFuture { state }
        .await
        .map_err(|error| RenderError::Device(error.to_string()))
}

fn glyph_layout_vertices() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: GLYPH_STRIDE,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 16,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 32,
                shader_location: 2,
            },
        ],
    }
}

fn atlas_resources(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    size: u32,
    pixels: &[u8],
) -> (wgpu::Texture, wgpu::TextureView, wgpu::BindGroup) {
    let size = size.max(1);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("acadrust-atlas"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    if pixels.len() as u32 >= size * size {
        // Uploaded by the caller through the queue after this returns. The
        // initial 1x1 atlas is left blank; prepare writes the real image.
    }
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("acadrust-atlas"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    (texture, view, bind)
}

fn fill_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: FILL_STRIDE,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 16,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 32,
                shader_location: 2,
            },
        ],
    }
}

fn instance_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: INSTANCE_STRIDE,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 16,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 32,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 48,
                shader_location: 3,
            },
        ],
    }
}

fn push_segment(
    bytes: &mut Vec<u8>,
    start: Vector3,
    end: Vector3,
    color: [f32; 4],
    half_mm: f32,
    cosmetic: f32,
) {
    push_f32(bytes, start.x as f32);
    push_f32(bytes, start.y as f32);
    push_f32(bytes, start.z as f32);
    push_f32(bytes, 1.0);
    push_f32(bytes, end.x as f32);
    push_f32(bytes, end.y as f32);
    push_f32(bytes, end.z as f32);
    push_f32(bytes, 1.0);
    for channel in color {
        push_f32(bytes, channel);
    }
    push_f32(bytes, half_mm);
    push_f32(bytes, cosmetic);
    push_f32(bytes, 0.0);
    push_f32(bytes, 0.0);
}

fn push_f32(bytes: &mut Vec<u8>, value: f32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn camera_bytes(
    view: &View,
    width: u32,
    height: u32,
    px_per_mm: f32,
    z_scale: f32,
    z_bias: f32,
    bounds: Option<(Vector3, Vector3)>,
) -> Vec<u8> {
    let matrix = camera_matrix(view, width, height, z_scale, z_bias, bounds);
    let mut bytes = Vec::with_capacity(CAMERA_SIZE as usize);
    for value in matrix {
        push_f32(&mut bytes, value);
    }
    push_f32(&mut bytes, width as f32);
    push_f32(&mut bytes, height as f32);
    push_f32(&mut bytes, px_per_mm);
    push_f32(&mut bytes, 0.0);
    push_f32(&mut bytes, z_scale);
    push_f32(&mut bytes, z_bias);
    bytes.resize(CAMERA_SIZE as usize, 0);
    bytes
}

fn camera_matrix(
    view: &View,
    width: u32,
    height: u32,
    z_scale: f32,
    z_bias: f32,
    bounds: Option<(Vector3, Vector3)>,
) -> [f32; 16] {
    let (right, up, normal) = view.basis();
    let aspect = width as f64 / height.max(1) as f64;
    let world_h = view.height.max(1e-9);
    let world_w = world_h * aspect;
    let sx = 2.0 / world_w;
    let sy = 2.0 / world_h;
    let (depth_scale, depth_bias) =
        depth_coefficients(normal, view.target, z_scale, z_bias, bounds);
    let target = view.target;
    [
        (sx * right.x) as f32,
        (sy * up.x) as f32,
        (depth_scale * normal.x) as f32,
        0.0,
        (sx * right.y) as f32,
        (sy * up.y) as f32,
        (depth_scale * normal.y) as f32,
        0.0,
        (sx * right.z) as f32,
        (sy * up.z) as f32,
        (depth_scale * normal.z) as f32,
        0.0,
        (-sx * right.dot(&target)) as f32,
        (-sy * up.dot(&target)) as f32,
        (-depth_scale * normal.dot(&target) + depth_bias) as f32,
        1.0,
    ]
}

/// Depth written into the projection. The plan view keeps the historical
/// world-Z mapping so an existing top view sorts the same pixels. Any other
/// direction maps closeness along the camera axis into 0 at the near side.
fn depth_coefficients(
    normal: Vector3,
    target: Vector3,
    z_scale: f32,
    z_bias: f32,
    bounds: Option<(Vector3, Vector3)>,
) -> (f64, f64) {
    let plan = normal.z > 1.0 - 1e-5 && normal.x.abs() < 1e-5 && normal.y.abs() < 1e-5;
    if plan {
        let scale = z_scale as f64;
        return (scale, scale * target.z + z_bias as f64);
    }
    let Some((min, max)) = bounds else {
        return (-1.0, 0.5);
    };
    let mut min_d = f64::INFINITY;
    let mut max_d = f64::NEG_INFINITY;
    for x in [min.x, max.x] {
        for y in [min.y, max.y] {
            for z in [min.z, max.z] {
                let point = Vector3::new(x, y, z) - target;
                let depth = normal.dot(&point);
                min_d = min_d.min(depth);
                max_d = max_d.max(depth);
            }
        }
    }
    if !min_d.is_finite() || !max_d.is_finite() {
        return (-1.0, 0.5);
    }
    let span = (max_d - min_d).max(1e-4);
    let pad = span * 0.02;
    let min_d = min_d - pad;
    let max_d = max_d + pad;
    let span = (max_d - min_d).max(1e-4);
    (-1.0 / span, max_d / span)
}

fn depth_map(scene: &Scene) -> (f32, f32, Option<(Vector3, Vector3)>) {
    let mut min = Vector3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut max = Vector3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    let mut min_z = f32::INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    let mut any = false;
    let mut include = |point: Vector3| {
        if !point.x.is_finite() || !point.y.is_finite() || !point.z.is_finite() {
            return;
        }
        any = true;
        min.x = min.x.min(point.x);
        min.y = min.y.min(point.y);
        min.z = min.z.min(point.z);
        max.x = max.x.max(point.x);
        max.y = max.y.max(point.y);
        max.z = max.z.max(point.z);
        let z = point.z as f32;
        if z.is_finite() {
            min_z = min_z.min(z);
            max_z = max_z.max(z);
        }
    };
    for stroke in &scene.strokes {
        for point in &stroke.points {
            include(*point);
        }
    }
    for fill in &scene.fills {
        for point in &fill.triangles {
            include(*point);
        }
    }
    for glyph in &scene.glyphs {
        include(glyph.anchor);
    }
    for picture in &scene.pictures {
        for point in &picture.triangles {
            include(*point);
        }
    }
    let bounds = any.then_some((min, max));
    if !min_z.is_finite() || !max_z.is_finite() || max_z - min_z < 1e-4 {
        min_z = -0.5;
        max_z = 0.5;
    }
    let pad = (max_z - min_z) * 0.02;
    min_z -= pad;
    max_z += pad;
    let span = (max_z - min_z).max(1e-4);
    // Larger world Z is closer to a plan camera, so it gets the smaller depth.
    (-1.0 / span, max_z / span, bounds)
}

struct PreparedPicture {
    _texture: wgpu::Texture,
    _view: wgpu::TextureView,
    bind: wgpu::BindGroup,
    vertices: wgpu::Buffer,
    count: u32,
}

fn picture_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: 24,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 16,
                shader_location: 1,
            },
        ],
    }
}

fn depth_texture(device: &wgpu::Device, width: u32, height: u32, samples: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("acadrust-depth"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: samples.max(1),
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    })
}

fn depth_state(write: bool) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(write),
        depth_compare: Some(wgpu::CompareFunction::LessEqual),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    }
}

fn padded_bytes_per_row(width: u32) -> u64 {
    let unpadded = width as u64 * 4;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as u64;
    unpadded.div_ceil(align) * align
}

fn to_color(rgba: [f32; 4]) -> wgpu::Color {
    wgpu::Color {
        r: rgba[0] as f64,
        g: rgba[1] as f64,
        b: rgba[2] as f64,
        a: rgba[3] as f64,
    }
}

pub(super) fn clear_color(format: wgpu::TextureFormat, rgba: [f32; 4]) -> [f32; 4] {
    if !format.is_srgb() {
        return rgba;
    }
    [
        srgb_to_linear(rgba[0]),
        srgb_to_linear(rgba[1]),
        srgb_to_linear(rgba[2]),
        rgba[3],
    ]
}

fn srgb_to_linear(channel: f32) -> f32 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

fn contrast(background: [f32; 4]) -> [u8; 3] {
    let luminance = 0.2126 * background[0] + 0.7152 * background[1] + 0.0722 * background[2];
    if luminance > 0.5 {
        [0, 0, 0]
    } else {
        [255, 255, 255]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(matrix: [f32; 16], point: Vector3) -> [f32; 3] {
        let p = [point.x as f32, point.y as f32, point.z as f32, 1.0];
        let mut out = [0.0; 3];
        for row in 0..3 {
            out[row] = matrix[row] * p[0]
                + matrix[4 + row] * p[1]
                + matrix[8 + row] * p[2]
                + matrix[12 + row] * p[3];
        }
        out
    }

    #[test]
    fn plan_matrix_keeps_the_top_view_and_its_depth() {
        let view = View {
            target: Vector3::new(10.0, -4.0, 5.0),
            height: 100.0,
            twist: 0.4,
            ..View::default()
        };
        let z_scale = -0.02;
        let z_bias = 0.6;
        let matrix = camera_matrix(&view, 200, 100, z_scale, z_bias, None);
        let aspect = 2.0;
        let sx = 2.0 / (100.0 * aspect);
        let sy = 2.0 / 100.0;
        let (sin, cos) = 0.4_f64.sin_cos();
        let point = Vector3::new(12.0, 3.0, 8.0);
        let dx = point.x - 10.0;
        let dy = point.y - (-4.0);
        let expected_x = sx * (cos * dx + sin * dy);
        let expected_y = sy * (-sin * dx + cos * dy);
        let got = clip(matrix, point);
        assert!((got[0] as f64 - expected_x).abs() < 1e-5);
        assert!((got[1] as f64 - expected_y).abs() < 1e-5);
        let expected_z = z_scale * point.z as f32 + z_bias;
        assert!((got[2] - expected_z).abs() < 1e-5);
    }

    #[test]
    fn front_matrix_puts_world_x_right_and_world_z_up() {
        let view = View {
            target: Vector3::ZERO,
            height: 10.0,
            direction: -Vector3::UNIT_Y,
            ..View::default()
        };
        let bounds = Some((Vector3::new(-5.0, -5.0, -5.0), Vector3::new(5.0, 5.0, 5.0)));
        let matrix = camera_matrix(&view, 100, 100, -1.0, 1.0, bounds);
        let right = clip(matrix, Vector3::UNIT_X);
        let up = clip(matrix, Vector3::UNIT_Z);
        let near = clip(matrix, Vector3::new(0.0, -5.0, 0.0));
        let far = clip(matrix, Vector3::new(0.0, 5.0, 0.0));
        assert!(right[0] > 0.0, "world +X should move right, {right:?}");
        assert!(up[1] > 0.0, "world +Z should move up, {up:?}");
        assert!(
            near[2] < far[2],
            "the camera side should be nearer, near {near:?} far {far:?}"
        );
    }
}
