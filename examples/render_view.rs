//! Pan and zoom a small drawing. Camera changes edit [`View`] only.
//!
//! ```text
//! cargo run --example render_view --features render
//! cargo run --example render_view --features render -- target/entity-atlas/AC1032/entity_atlas_AC1032_dwg.dwg
//! ```
//!
//! Click an entity to select it. Drag with the left button to pan, and scroll
//! to zoom.

use std::sync::Arc;
use std::{
    error::Error,
    path::{Path, PathBuf},
};

use acadrust::entities::{EntityType, Line};
use acadrust::render::{compile, CompileOptions, Renderer, RendererConfig, View};
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

fn input_path() -> Option<PathBuf> {
    std::env::args_os().nth(1).map(PathBuf::from)
}

fn load_document(path: Option<&Path>) -> Result<CadDocument, Box<dyn Error>> {
    let Some(path) = path else {
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

fn compile_options(path: Option<&Path>) -> CompileOptions {
    let mut options = CompileOptions::default();
    options.font_dirs = font_directories();
    if let Some(path) = path {
        if let Some(parent) = path.parent() {
            options.resource_dirs.push(parent.to_path_buf());
            for ancestor in parent.ancestors() {
                let assets = ancestor.join("assets");
                if assets.is_dir() {
                    options.resource_dirs.push(assets);
                }
            }
        }
    }
    options
}

fn font_directories() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(value) = std::env::var_os("ACADRUST_FONT_DIRS") {
        dirs.extend(std::env::split_paths(&value));
    }
    for path in [
        PathBuf::from(r"C:\Windows\Fonts"),
        PathBuf::from(r"C:\Program Files\Autodesk"),
        PathBuf::from(r"C:\Program Files (x86)\Autodesk"),
    ] {
        collect_font_directories(&path, 2, &mut dirs);
    }
    dirs.retain(|path| path.is_dir());
    dirs.sort();
    dirs.dedup();
    dirs
}

fn collect_font_directories(root: &Path, depth: usize, output: &mut Vec<PathBuf>) {
    if !root.is_dir() {
        return;
    }
    if root
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("Fonts"))
    {
        output.push(root.to_path_buf());
        return;
    }
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        collect_font_directories(&entry.path(), depth - 1, output);
    }
}

struct App {
    doc: CadDocument,
    options: CompileOptions,
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
    drag_moved: bool,
    press_cursor: Option<(f64, f64)>,
    last_cursor: Option<(f64, f64)>,
}

impl Gpu {
    fn new(window: Arc<Window>, doc: &CadDocument, options: &CompileOptions) -> Self {
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
        let scene = compile(doc, options);
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
            drag_moved: false,
            press_cursor: None,
            last_cursor: None,
        }
    }

    fn select_at(&mut self, x: f64, y: f64) {
        let picked = self
            .scene
            .pick_screen(
                &self.view,
                (self.config.width, self.config.height),
                x,
                y,
                4.0,
            )
            .map(|source| source.selection_handle());
        self.renderer.set_selection(picked);
        self.prepared = false;
        self.window.request_redraw();
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
        self.gpu = Some(Gpu::new(window, &self.doc, &self.options));
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
                if state.is_pressed() {
                    gpu.dragging = true;
                    gpu.drag_moved = false;
                    gpu.press_cursor = gpu.last_cursor;
                } else {
                    let clicked = gpu.dragging && !gpu.drag_moved;
                    gpu.dragging = false;
                    gpu.press_cursor = None;
                    if clicked {
                        if let Some((x, y)) = gpu.last_cursor {
                            gpu.select_at(x, y);
                        }
                    }
                    gpu.last_cursor = None;
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if gpu.dragging {
                    if let Some((x, y)) = gpu.last_cursor {
                        if gpu.press_cursor.is_none() {
                            gpu.press_cursor = Some((x, y));
                        }
                        if let Some((px, py)) = gpu.press_cursor {
                            if (position.x - px).abs() + (position.y - py).abs() > 3.0 {
                                gpu.drag_moved = true;
                            }
                        }
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
    let path = input_path();
    let mut app = App {
        doc: load_document(path.as_deref())?,
        options: compile_options(path.as_deref()),
        gpu: None,
    };
    event_loop.run_app(&mut app).expect("run");
    Ok(())
}
