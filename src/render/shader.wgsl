struct Camera {
    view_proj: mat4x4<f32>,
    viewport: vec2<f32>,
    px_per_mm: f32,
    _pad: f32,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex: u32,
    @location(0) start: vec4<f32>,
    @location(1) end: vec4<f32>,
    @location(2) color: vec4<f32>,
    @location(3) params: vec4<f32>,
) -> VertexOut {
    // Two triangles. x is 0 at the start and 1 at the end. y is the side.
    var corner = vec2<f32>(0.0, -1.0);
    if vertex == 1u || vertex == 2u || vertex == 4u {
        corner.x = 1.0;
    }
    if vertex == 2u || vertex == 4u || vertex == 5u {
        corner.y = 1.0;
    }

    let start_clip = camera.view_proj * vec4<f32>(start.xyz, 1.0);
    let end_clip = camera.view_proj * vec4<f32>(end.xyz, 1.0);
    let start_ndc = start_clip.xy / max(start_clip.w, 1e-6);
    let end_ndc = end_clip.xy / max(end_clip.w, 1e-6);
    let dir = end_ndc - start_ndc;
    let len = length(dir);
    var normal = vec2<f32>(0.0, 1.0);
    if len > 1e-5 {
        normal = vec2<f32>(-dir.y, dir.x) / len;
    }
    var width_px = 1.0;
    if params.y < 0.5 {
        width_px = max(params.x * 2.0 * camera.px_per_mm, 1.0);
    }
    let px_ndc = vec2<f32>(2.0, 2.0) / max(camera.viewport, vec2<f32>(1.0, 1.0));
    let offset = normal * corner.y * width_px * 0.5 * px_ndc;
    let pos = mix(start_ndc, end_ndc, corner.x) + offset;
    return VertexOut(vec4<f32>(pos, 0.0, 1.0), color);
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return in.color;
}

struct FillIn {
    @location(0) pos: vec4<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vs_fill(in: FillIn) -> VertexOut {
    let clip = camera.view_proj * vec4<f32>(in.pos.xyz, 1.0);
    return VertexOut(clip, in.color);
}

@fragment
fn fs_fill(in: VertexOut) -> @location(0) vec4<f32> {
    return in.color;
}

@group(1) @binding(0)
var atlas_texture: texture_2d<f32>;

@group(1) @binding(1)
var atlas_sampler: sampler;

struct GlyphIn {
    @location(0) pos: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct GlyphOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vs_glyph(in: GlyphIn) -> GlyphOut {
    let clip = camera.view_proj * vec4<f32>(in.pos.xyz, 1.0);
    return GlyphOut(clip, in.uv, in.color);
}

@fragment
fn fs_glyph(in: GlyphOut) -> @location(0) vec4<f32> {
    let coverage = textureSample(atlas_texture, atlas_sampler, in.uv).r;
    return vec4<f32>(in.color.rgb, in.color.a * coverage);
}
