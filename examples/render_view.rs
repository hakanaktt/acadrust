//! Pan and zoom a small drawing. Camera changes edit [`View`] only.
//!
//! ```text
//! cargo run --example render_view --features render
//! cargo run --example render_view --features render -- target/entity-atlas/AC1032/entity_atlas_AC1032_dwg.dwg
//! ```
//!
//! Drag with the left button to pan. Scroll to zoom.

use std::sync::Arc;
use std::{error::Error, path::PathBuf};

use acadrust::entities::{EntityType, Line};
use acadrust::render::{compile, Renderer, RendererConfig, View};
use acadrust::types::{Color, LineWeight};
use acadrust::{CadDocument, DwgReader, DxfReader};
use winit::application::ApplicationHandler;
use winit::event::{MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

fn drawing() -> CadDocument {
    let mut doc = CadDocument::new();
    doc.header.lineweight_display = true;
    let weights = [
        (LineWeight::Value(25), Color::RED, 0.0),
        (LineWeight::Value(50), Color::from_rgb(255, 255, 0), 15.0),
        (LineWeight::Value(100), Color::GREEN, 30.0),
        (LineWeight::Value(211), Color::from_rgb(0, 128, 255), 45.0),
    ];
    for (weight, color, y) in weights {
        let mut line = Line::from_coords(0.0, y, 0.0, 100.0, y, 0.0);
        line.common.line_weight = weight;
        line.common.color = color;
        doc.add_entity(EntityType::Line(line)).unwrap();
    }
    doc
}

fn load_document() -> Result<CadDocument, Box<dyn Error>> {
    let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) else {
        return Ok(drawing());
    };
    let is_dxf = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("dxf"));
    if is_dxf {
        Ok(DxfReader::from_file(path)?.read()?)
    } else {
        Ok(DwgReader::from_file(path)?.read()?)
    }
}

struct App {
    doc: CadDocument,
    gpu: Option<Gpu>,
}

struct Gpu {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    scene: acadrust::render::Scene,
    view: View,
    prepared: bool,
    dragging: bool,
    last_cursor: Option<(f64, f64)>,
}

impl Gpu {
    fn new(window: Arc<Window>, doc: &CadDocument) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance
            .create_surface(window.clone())
            .expect("create surface");
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .expect("GPU adapter");
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("GPU device");
        let size = window.inner_size();
        let config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .expect("surface config");
        surface.configure(&device, &config);
        let renderer = Renderer::new(
            &device,
            &queue,
            &RendererConfig {
                format: config.format,
                ..RendererConfig::default()
            },
        );
        let scene = compile(doc, &acadrust::render::CompileOptions::default());
        let view = View::zoom_extents(&scene, config.width, config.height);
        Self {
            window,
            surface,
            device,
            queue,
            config,
            renderer,
            scene,
            view,
            prepared: false,
            dragging: false,
            last_cursor: None,
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.renderer.resize(&self.device, width, height);
        self.window.request_redraw();
    }

    fn redraw(&mut self) {
        if !self.prepared {
            self.renderer.prepare(
                &self.device,
                &self.queue,
                &self.scene,
                &self.view,
                (self.config.width, self.config.height),
            );
            self.prepared = true;
        }
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.resize(self.config.width, self.config.height);
                return;
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return,
        };
        let target = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        self.renderer.render(
            &mut encoder,
            &target,
            &self.view,
            (self.config.width, self.config.height),
            &self.queue,
        );
        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
    }

    fn zoom(&mut self, steps: f32) {
        let factor = 0.9_f64.powf(steps as f64);
        self.view.height = (self.view.height * factor).clamp(1e-3, 1e9);
        self.window.request_redraw();
    }

    fn pan(&mut self, dx_px: f64, dy_px: f64) {
        let pixels_y = self.config.height.max(1) as f64;
        let scale = self.view.height / pixels_y;
        self.view.target.x -= dx_px * scale;
        self.view.target.y += dy_px * scale;
        self.window.request_redraw();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("acadrust render"))
                .expect("window"),
        );
        self.gpu = Some(Gpu::new(window, &self.doc));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(gpu) = self.gpu.as_mut() else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => gpu.resize(size.width, size.height),
            WindowEvent::RedrawRequested => gpu.redraw(),
            WindowEvent::MouseWheel { delta, .. } => {
                let steps = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(delta) => (delta.y / 60.0) as f32,
                };
                gpu.zoom(steps);
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                gpu.dragging = state.is_pressed();
                if !gpu.dragging {
                    gpu.last_cursor = None;
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if gpu.dragging {
                    if let Some((x, y)) = gpu.last_cursor {
                        gpu.pan(position.x - x, position.y - y);
                    }
                }
                gpu.last_cursor = Some((position.x, position.y));
            }
            _ => {}
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new().expect("event loop");
    let mut app = App {
        doc: load_document()?,
        gpu: None,
    };
    event_loop.run_app(&mut app).expect("run");
    Ok(())
}
