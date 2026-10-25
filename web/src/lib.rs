use std::io::Cursor;

use acadrust::render::{
    compile, CompileOptions, Renderer, RendererConfig, Scene, StandardVisualStyle, View,
    VisualStyleRequest,
};
use acadrust::{CadDocument, DwgReader};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

fn js_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}

/// A persistent browser viewer. The document and GPU resources stay alive so
/// camera changes only update the camera uniform and redraw the existing scene.
#[wasm_bindgen]
pub struct Viewer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    document: CadDocument,
    options: CompileOptions,
    scene: Scene,
    view: View,
}

#[wasm_bindgen]
impl Viewer {
    /// Render the current camera state to the canvas.
    pub fn render(&mut self) -> Result<(), JsValue> {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            other => return Err(js_error(format!("canvas surface unavailable: {other:?}"))),
        };
        let target = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("acadrust-wasm-viewer"),
            });
        self.renderer.render(
            &mut encoder,
            &target,
            &self.view,
            (self.config.width, self.config.height),
            &self.queue,
        );
        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
        Ok(())
    }

    /// Update the drawing-buffer size after JavaScript resizes the canvas.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), JsValue> {
        let width = width.max(1);
        let height = height.max(1);
        if self.config.width == width && self.config.height == height {
            return Ok(());
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.renderer.resize(&self.device, width, height);
        self.render()
    }

    /// Restore an extents view using the current canvas aspect ratio.
    pub fn fit(&mut self) -> Result<(), JsValue> {
        self.view = View::zoom_extents(&self.scene, self.config.width, self.config.height);
        self.render()
    }

    /// Zoom around a canvas-pixel position. A factor below one zooms in.
    pub fn zoom_at(&mut self, factor: f64, x: f64, y: f64) -> Result<(), JsValue> {
        if !factor.is_finite() || factor <= 0.0 {
            return Err(js_error("zoom factor must be positive"));
        }
        let old_offset = self.world_offset(x, y);
        self.view.height = (self.view.height * factor).clamp(1e-9, 1e300);
        let new_offset = self.world_offset(x, y);
        self.view.target.x += old_offset.0 - new_offset.0;
        self.view.target.y += old_offset.1 - new_offset.1;
        self.render()
    }

    /// Pan by a screen-pixel drag delta.
    pub fn pan(&mut self, dx: f64, dy: f64) -> Result<(), JsValue> {
        if !dx.is_finite() || !dy.is_finite() {
            return Err(js_error("pan delta must be finite"));
        }
        let scale = self.view.height / self.config.height.max(1) as f64;
        let u = -dx * scale;
        let v = dy * scale;
        let (sin, cos) = self.view.twist.sin_cos();
        self.view.target.x += cos * u - sin * v;
        self.view.target.y += sin * u + cos * v;
        self.render()
    }

    /// Rotate the plan view around a canvas-pixel position.
    pub fn rotate_at(&mut self, radians: f64, x: f64, y: f64) -> Result<(), JsValue> {
        if !radians.is_finite() {
            return Err(js_error("rotation must be finite"));
        }
        let old_offset = self.world_offset(x, y);
        self.view.twist += radians;
        let new_offset = self.world_offset(x, y);
        self.view.target.x += old_offset.0 - new_offset.0;
        self.view.target.y += old_offset.1 - new_offset.1;
        self.render()
    }

    /// Select a built-in style or restore the drawing's active viewport style.
    pub fn set_visual_style(&mut self, name: String) -> Result<(), JsValue> {
        let trimmed = name.trim();
        self.options.visual_style = if trimmed.is_empty()
            || trimmed.eq_ignore_ascii_case("active viewport")
            || trimmed.eq_ignore_ascii_case("activeviewport")
        {
            VisualStyleRequest::ActiveViewport
        } else {
            let kind = StandardVisualStyle::from_name(trimmed)
                .ok_or_else(|| js_error(format!("unknown visual style: {trimmed}")))?;
            VisualStyleRequest::Standard(kind)
        };
        self.scene = compile(&self.document, &self.options);
        self.renderer.prepare(
            &self.device,
            &self.queue,
            &self.scene,
            &self.view,
            (self.config.width, self.config.height),
        );
        self.render()
    }

    fn world_offset(&self, x: f64, y: f64) -> (f64, f64) {
        let width = self.config.width.max(1) as f64;
        let height = self.config.height.max(1) as f64;
        let scale = self.view.height / height;
        let u = (x - width * 0.5) * scale;
        let v = -(y - height * 0.5) * scale;
        let (sin, cos) = self.view.twist.sin_cos();
        (cos * u - sin * v, sin * u + cos * v)
    }

    async fn create_inner(
        canvas: HtmlCanvasElement,
        bytes: Vec<u8>,
        font: Option<(String, Vec<u8>)>,
    ) -> Result<Viewer, JsValue> {
        let document = DwgReader::from_stream(Cursor::new(bytes))
            .read()
            .map_err(js_error)?;
        let mut options = CompileOptions::default();
        if let Some(font) = font {
            options.font_files.push(font);
        }
        let scene = compile(&document, &options);
        Self::from_parts(canvas, document, options, scene).await
    }

    async fn from_parts(
        canvas: HtmlCanvasElement,
        document: CadDocument,
        options: CompileOptions,
        scene: Scene,
    ) -> Result<Viewer, JsValue> {
        let width = canvas.width().max(1);
        let height = canvas.height().max(1);
        let view = View::zoom_extents(&scene, width, height);
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface: wgpu::Surface<'static> = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(js_error)?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(js_error)?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .map_err(js_error)?;
        let config = surface
            .get_default_config(&adapter, width, height)
            .ok_or_else(|| js_error("the adapter has no compatible canvas format"))?;
        surface.configure(&device, &config);
        let mut renderer = Renderer::new(
            &device,
            &queue,
            &RendererConfig {
                format: config.format,
                ..RendererConfig::default()
            },
        );
        renderer.resize(&device, width, height);
        renderer.prepare(&device, &queue, &scene, &view, (width, height));
        Ok(Viewer {
            surface,
            device,
            queue,
            config,
            renderer,
            document,
            options,
            scene,
            view,
        })
    }
}

/// Render a DWG byte buffer into `canvas` using the persistent viewer path.
#[wasm_bindgen]
pub async fn render_dwg(canvas: HtmlCanvasElement, bytes: Vec<u8>) -> Result<(), JsValue> {
    let mut viewer = Viewer::create_inner(canvas, bytes, None).await?;
    viewer.render()
}

/// Render a drawing with a caller-provided SHX/SHP font.
#[wasm_bindgen]
pub async fn render_dwg_with_font(
    canvas: HtmlCanvasElement,
    bytes: Vec<u8>,
    font_name: String,
    font_bytes: Vec<u8>,
) -> Result<(), JsValue> {
    let mut viewer = Viewer::create_inner(canvas, bytes, Some((font_name, font_bytes))).await?;
    viewer.render()
}

/// Create a persistent viewer from a DWG byte buffer.
#[wasm_bindgen]
pub async fn create_viewer(
    canvas: HtmlCanvasElement,
    bytes: Vec<u8>,
) -> Result<Viewer, JsValue> {
    Viewer::create_inner(canvas, bytes, None).await
}

/// Create a persistent viewer with a caller-provided SHX/SHP font.
#[wasm_bindgen]
pub async fn create_viewer_with_font(
    canvas: HtmlCanvasElement,
    bytes: Vec<u8>,
    font_name: String,
    font_bytes: Vec<u8>,
) -> Result<Viewer, JsValue> {
    Viewer::create_inner(canvas, bytes, Some((font_name, font_bytes))).await
}
