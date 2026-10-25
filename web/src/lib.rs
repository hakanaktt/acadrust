use std::io::Cursor;

use acadrust::render::{compile, CompileOptions, Renderer, RendererConfig, View};
use acadrust::DwgReader;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

fn js_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}

/// Render a DWG byte buffer into `canvas` using the same scene compiler and
/// wgpu draw path as the native viewer.
#[wasm_bindgen]
pub async fn render_dwg(canvas: HtmlCanvasElement, bytes: Vec<u8>) -> Result<(), JsValue> {
    render_dwg_inner(canvas, bytes, None).await
}

/// Render a drawing with a caller-provided SHX/SHP font. Browser callers use
/// this for fonts that would otherwise be resolved from a native filesystem.
#[wasm_bindgen]
pub async fn render_dwg_with_font(
    canvas: HtmlCanvasElement,
    bytes: Vec<u8>,
    font_name: String,
    font_bytes: Vec<u8>,
) -> Result<(), JsValue> {
    render_dwg_inner(canvas, bytes, Some((font_name, font_bytes))).await
}

async fn render_dwg_inner(
    canvas: HtmlCanvasElement,
    bytes: Vec<u8>,
    font: Option<(String, Vec<u8>)>,
) -> Result<(), JsValue> {
    let document = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .map_err(js_error)?;
    let mut options = CompileOptions::default();
    if let Some(font) = font {
        options.font_files.push(font);
    }
    let scene = compile(&document, &options);
    let width = canvas.width().max(1);
    let height = canvas.height().max(1);
    let view = View::zoom_extents(&scene, width, height);
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
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
    let frame = match surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(frame)
        | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
        other => return Err(js_error(format!("canvas surface unavailable: {other:?}"))),
    };
    let target = frame
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("acadrust-wasm-viewer"),
    });
    renderer.render(&mut encoder, &target, &view, (width, height), &queue);
    queue.submit(Some(encoder.finish()));
    queue.present(frame);
    Ok(())
}
