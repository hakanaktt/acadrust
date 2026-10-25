struct Camera {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    viewport: vec2<f32>,
    px_per_mm: f32,
    _pad: f32,
    z_scale: f32,
    z_bias: f32,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
}

struct StrokeOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) @interpolate(flat) color: vec4<f32>,
    @location(1) world: vec2<f32>,
    @location(2) @interpolate(flat) a: vec4<f32>,
    @location(3) @interpolate(flat) b: vec4<f32>,
    @location(4) @interpolate(flat) c: vec4<f32>,
    @location(5) @interpolate(flat) d: vec4<f32>,
    @location(6) @interpolate(flat) params: vec4<f32>,
}

struct FragOut {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
}

const TAU: f32 = 6.28318530718;

fn ndc_of(p: vec3<f32>) -> vec2<f32> {
    let clip = camera.view_proj * vec4<f32>(p, 1.0);
    return clip.xy / max(clip.w, 1e-6);
}

fn depth_of(p: vec3<f32>) -> f32 {
    let clip = camera.view_proj * vec4<f32>(p, 1.0);
    // Keep analytic edges just in front of their coplanar fill.  A large
    // normalized offset makes edges from shallow internal faces pass the
    // depth test through an outer skin (the model depth span can be dozens
    // of drawing units).  1e-4 is enough to resolve rasterization ties while
    // staying below the thickness of ordinary CAD features.
    return clamp(clip.z / max(clip.w, 1e-6) - 0.0001, 0.0, 1.0);
}

fn frag_ndc(frag: vec4<f32>) -> vec2<f32> {
    let viewport = max(camera.viewport, vec2<f32>(1.0, 1.0));
    return vec2<f32>(frag.x / viewport.x * 2.0 - 1.0, 1.0 - frag.y / viewport.y * 2.0);
}

fn ndc_px(delta: vec2<f32>) -> f32 {
    return length(delta * camera.viewport * 0.5);
}

fn px_per_world() -> f32 {
    let axis = vec2<f32>(camera.view_proj[0][0], camera.view_proj[0][1]);
    let axis_px = vec2<f32>(axis.x * camera.viewport.x, axis.y * camera.viewport.y) * 0.5;
    return max(length(axis_px), 1e-6);
}

fn stroke_width_px(params: vec4<f32>) -> f32 {
    if params.y < 0.5 {
        return max(params.x * 2.0 * camera.px_per_mm, 1.0);
    }
    if params.y > 1.5 {
        // params.x is the half-width in drawing units.
        return max(params.x * 2.0 * px_per_world(), 1.0);
    }
    return 1.0;
}

fn in_sweep(t: f32, start: f32, sweep: f32) -> bool {
    if abs(sweep) >= TAU - 1e-3 {
        return true;
    }
    let rel = t - start - floor((t - start) / TAU) * TAU;
    if sweep >= 0.0 {
        return rel <= sweep + 1e-3;
    }
    return rel >= TAU + sweep - 1e-3 || rel <= 1e-3;
}

fn elliptic_at(center: vec3<f32>, u: vec3<f32>, v: vec3<f32>, t: f32) -> vec3<f32> {
    return center + u * cos(t) + v * sin(t);
}

fn include_elliptic(t: f32, start: f32, sweep: f32, center: vec3<f32>, u: vec3<f32>, v: vec3<f32>, bound: vec4<f32>) -> vec4<f32> {
    if !in_sweep(t, start, sweep) && !in_sweep(t + TAU, start, sweep) && !in_sweep(t - TAU, start, sweep) {
        return bound;
    }
    let p = elliptic_at(center, u, v, t).xy;
    return vec4<f32>(min(bound.x, p.x), min(bound.y, p.y), max(bound.z, p.x), max(bound.w, p.y));
}

fn elliptic_bounds(center: vec3<f32>, u: vec3<f32>, v: vec3<f32>, start: f32, sweep: f32) -> vec4<f32> {
    var bound = vec4<f32>(1e20, 1e20, -1e20, -1e20);
    bound = include_elliptic(start, start, sweep, center, u, v, bound);
    bound = include_elliptic(start + sweep, start, sweep, center, u, v, bound);
    bound = include_elliptic(atan2(v.x, u.x), start, sweep, center, u, v, bound);
    bound = include_elliptic(atan2(v.x, u.x) + 3.14159265, start, sweep, center, u, v, bound);
    bound = include_elliptic(atan2(v.y, u.y), start, sweep, center, u, v, bound);
    bound = include_elliptic(atan2(v.y, u.y) + 3.14159265, start, sweep, center, u, v, bound);
    return bound;
}

fn bernstein(t: f32) -> vec4<f32> {
    let m = 1.0 - t;
    return vec4<f32>(m * m * m, 3.0 * m * m * t, 3.0 * m * t * t, t * t * t);
}

fn cubic_at(p0: vec4<f32>, p1: vec4<f32>, p2: vec4<f32>, p3: vec4<f32>, t: f32) -> vec3<f32> {
    let b = bernstein(t);
    let w0 = max(abs(p0.w), 1e-4);
    let w1 = max(abs(p1.w), 1e-4);
    let w2 = max(abs(p2.w), 1e-4);
    let w3 = max(abs(p3.w), 1e-4);
    let weight = b.x * w0 + b.y * w1 + b.z * w2 + b.w * w3;
    let point = p0.xyz * (b.x * w0) + p1.xyz * (b.y * w1) + p2.xyz * (b.z * w2) + p3.xyz * (b.w * w3);
    return point / max(abs(weight), 1e-4);
}

fn cubic_bounds(p0: vec4<f32>, p1: vec4<f32>, p2: vec4<f32>, p3: vec4<f32>) -> vec4<f32> {
    let a = cubic_at(p0, p1, p2, p3, 0.0).xy;
    let b = cubic_at(p0, p1, p2, p3, 1.0).xy;
    var bound = vec4<f32>(min(a.x, b.x), min(a.y, b.y), max(a.x, b.x), max(a.y, b.y));
    // The curve stays inside the control hull, so the hull covers the arc.
    let hull = array<vec2<f32>, 4>(p0.xy, p1.xy, p2.xy, p3.xy);
    for (var i = 0; i < 4; i++) {
        bound = vec4<f32>(
            min(bound.x, hull[i].x),
            min(bound.y, hull[i].y),
            max(bound.z, hull[i].x),
            max(bound.w, hull[i].y),
        );
    }
    return bound;
}

fn newton_elliptic(center: vec3<f32>, u: vec3<f32>, v: vec3<f32>, seed: f32, p: vec2<f32>) -> f32 {
    var t = seed;
    for (var i = 0; i < 6; i++) {
        let pos = elliptic_at(center, u, v, t).xy;
        let d1 = -u.xy * sin(t) + v.xy * cos(t);
        let d2 = -u.xy * cos(t) - v.xy * sin(t);
        let diff = pos - p;
        let f = dot(diff, d1);
        let fp = dot(d1, d1) + dot(diff, d2);
        if abs(fp) < 1e-8 {
            break;
        }
        t = t - f / fp;
    }
    return t;
}

fn closest_elliptic(center: vec3<f32>, u: vec3<f32>, v: vec3<f32>, start: f32, sweep: f32, p: vec2<f32>) -> vec4<f32> {
    var best = elliptic_at(center, u, v, start);
    var best_t = start;
    var best_d = distance(best.xy, p);
    let end_t = start + sweep;
    let end = elliptic_at(center, u, v, end_t);
    let end_d = distance(end.xy, p);
    if end_d < best_d {
        best = end;
        best_t = end_t;
        best_d = end_d;
    }
    let det = u.x * v.y - u.y * v.x;
    var seed = start + sweep * 0.5;
    if abs(det) > 1e-6 {
        let q = p - center.xy;
        seed = atan2((u.x * q.y - u.y * q.x) / det, (q.x * v.y - q.y * v.x) / det);
    }
    let seeds = array<f32, 4>(seed, seed + 3.14159265, start, end_t);
    for (var i = 0; i < 4; i++) {
        let t = newton_elliptic(center, u, v, seeds[i], p);
        if in_sweep(t, start, sweep) || in_sweep(t + TAU, start, sweep) || in_sweep(t - TAU, start, sweep) {
            let pos = elliptic_at(center, u, v, t);
            let d = distance(pos.xy, p);
            if d < best_d {
                best = pos;
                best_t = t;
                best_d = d;
            }
        }
    }
    return vec4<f32>(best, best_t);
}

fn closest_cubic(p0: vec4<f32>, p1: vec4<f32>, p2: vec4<f32>, p3: vec4<f32>, p: vec2<f32>) -> vec4<f32> {
    var best_t = 0.0;
    var best_d = distance(cubic_at(p0, p1, p2, p3, 0.0).xy, p);
    for (var i = 1; i <= 8; i++) {
        let t = f32(i) / 8.0;
        let d = distance(cubic_at(p0, p1, p2, p3, t).xy, p);
        if d < best_d {
            best_d = d;
            best_t = t;
        }
    }
    var t = best_t;
    for (var n = 0; n < 5; n++) {
        let dt = 0.002;
        let left_t = clamp(t - dt, 0.0, 1.0);
        let right_t = clamp(t + dt, 0.0, 1.0);
        let left = cubic_at(p0, p1, p2, p3, left_t).xy;
        let right = cubic_at(p0, p1, p2, p3, right_t).xy;
        let mid = cubic_at(p0, p1, p2, p3, t).xy;
        let span = max(right_t - left_t, 1e-4);
        let slope = (right - left) / span;
        let diff = mid - p;
        let f = dot(diff, slope);
        let fp = max(dot(slope, slope), 1e-8);
        t = clamp(t - f / fp, 0.0, 1.0);
    }
    let polished = cubic_at(p0, p1, p2, p3, t);
    if distance(polished.xy, p) < best_d {
        return vec4<f32>(polished, t);
    }
    return vec4<f32>(cubic_at(p0, p1, p2, p3, best_t), best_t);
}

fn project_vec(origin: vec3<f32>, axis: vec3<f32>) -> vec3<f32> {
    let delta = ndc_of(origin + axis) - ndc_of(origin);
    return vec3<f32>(delta, 0.0);
}

fn project_cubic(p: vec4<f32>) -> vec4<f32> {
    let ndc = ndc_of(p.xyz);
    return vec4<f32>(ndc, 0.0, p.w);
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex: u32,
    @location(0) a: vec4<f32>,
    @location(1) b: vec4<f32>,
    @location(2) color: vec4<f32>,
    @location(3) params: vec4<f32>,
    @location(4) c: vec4<f32>,
    @location(5) d: vec4<f32>,
) -> StrokeOut {
    // Two triangles. x is 0 at the start and 1 at the end. y is the side.
    var corner = vec2<f32>(0.0, -1.0);
    if vertex == 1u || vertex == 2u || vertex == 4u {
        corner.x = 1.0;
    }
    if vertex == 2u || vertex == 4u || vertex == 5u {
        corner.y = 1.0;
    }

    if params.z < 0.5 {
        let start_clip = camera.view_proj * vec4<f32>(a.xyz, 1.0);
        let end_clip = camera.view_proj * vec4<f32>(b.xyz, 1.0);
        let start_ndc = start_clip.xy / max(start_clip.w, 1e-6);
        let end_ndc = end_clip.xy / max(end_clip.w, 1e-6);
        let dir = end_ndc - start_ndc;
        let len = length(dir);
        var normal = vec2<f32>(0.0, 1.0);
        if len > 1e-5 {
            normal = vec2<f32>(-dir.y, dir.x) / len;
        }
        let width_px = stroke_width_px(params);
        let px_ndc = vec2<f32>(2.0, 2.0) / max(camera.viewport, vec2<f32>(1.0, 1.0));
        var pos = mix(start_ndc, end_ndc, corner.x);
        if len <= 1e-5 {
            // A point is uploaded as a zero-length segment.  Expanding only along
            // `normal` would leave both triangles edge-on, so make all four
            // corners of a small screen-space square around the point.
            let half = width_px * 0.5 * px_ndc;
            pos = start_ndc
                + vec2<f32>((corner.x * 2.0 - 1.0) * half.x, corner.y * half.y);
        } else {
            let offset = normal * corner.y * width_px * 0.5 * px_ndc;
            pos += offset;
        }
        let z0 = depth_of(a.xyz);
        let z1 = depth_of(b.xyz);
        // A small pull toward the camera keeps an edge from flickering on its face.
        let depth = mix(z0, z1, corner.x);
        return StrokeOut(vec4<f32>(pos, depth, 1.0), color, vec2<f32>(0.0), a, b, c, d, params);
    }

    var bound = vec4<f32>(1e20, 1e20, -1e20, -1e20);
    var depth_p = a.xyz;
    if params.z < 1.5 {
        let origin = ndc_of(a.xyz);
        bound = elliptic_bounds(
            vec3<f32>(origin, 0.0),
            project_vec(a.xyz, b.xyz),
            project_vec(a.xyz, c.xyz),
            d.x,
            d.y,
        );
        depth_p = a.xyz;
    } else {
        bound = cubic_bounds(project_cubic(a), project_cubic(b), project_cubic(c), project_cubic(d));
        depth_p = (a.xyz + d.xyz) * 0.5;
    }
    let width_px = stroke_width_px(params);
    let pad = (width_px * 0.5 + 1.25) * vec2<f32>(2.0, 2.0) / max(camera.viewport, vec2<f32>(1.0, 1.0));
    let min_ndc = bound.xy - pad;
    let max_ndc = bound.zw + pad;
    var uv = vec2<f32>(0.0, 0.0);
    if vertex == 1u || vertex == 2u || vertex == 4u {
        uv.x = 1.0;
    }
    if vertex == 2u || vertex == 4u || vertex == 5u {
        uv.y = 1.0;
    }
    let ndc = mix(min_ndc, max_ndc, uv);
    return StrokeOut(vec4<f32>(ndc, depth_of(depth_p), 1.0), color, ndc, a, b, c, d, params);
}

@fragment
fn fs_main(in: StrokeOut) -> FragOut {
    if in.color.a <= 0.001 {
        discard;
    }
    if in.params.z < 0.5 {
        return FragOut(in.color, in.clip.z);
    }
    let pixel = frag_ndc(in.clip);
    var closest_ndc = pixel;
    var depth = in.clip.z;
    if in.params.z < 1.5 {
        let origin = ndc_of(in.a.xyz);
        let hit = closest_elliptic(
            vec3<f32>(origin, 0.0),
            project_vec(in.a.xyz, in.b.xyz),
            project_vec(in.a.xyz, in.c.xyz),
            in.d.x,
            in.d.y,
            pixel,
        );
        closest_ndc = hit.xy;
        depth = depth_of(elliptic_at(in.a.xyz, in.b.xyz, in.c.xyz, hit.w));
    } else {
        let hit = closest_cubic(
            project_cubic(in.a),
            project_cubic(in.b),
            project_cubic(in.c),
            project_cubic(in.d),
            pixel,
        );
        closest_ndc = hit.xy;
        depth = depth_of(cubic_at(in.a, in.b, in.c, in.d, hit.w));
    }
    let dist_px = ndc_px(closest_ndc - pixel);
    let half_px = stroke_width_px(in.params) * 0.5;
    let coverage = 1.0 - smoothstep(half_px - 0.6, half_px + 0.6, dist_px);
    let alpha = in.color.a * coverage;
    if alpha <= 0.01 {
        discard;
    }
    return FragOut(vec4<f32>(in.color.rgb, alpha), depth);
}

struct FillIn {
    @location(0) pos: vec4<f32>,
    @location(1) color: vec4<f32>,
    @location(2) normal: vec4<f32>,
}

struct FillOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) @interpolate(flat) lighting: f32,
}

@vertex
fn vs_fill(in: FillIn) -> FillOut {
    let clip = camera.view_proj * vec4<f32>(in.pos.xyz, 1.0);
    return FillOut(clip, in.color, in.normal.xyz, in.normal.w);
}

@fragment
fn fs_fill(in: FillOut, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    if in.color.a <= 0.001 {
        discard;
    }

    if in.lighting < 0.5 || length(in.normal) <= 1e-4 {
        return in.color;
    }
    if in.lighting < 1.5 {
        return vec4<f32>(in.color.rgb * 0.72, in.color.a);
    }

    // Triangle winding is normalized against the ACIS surface normal before
    // upload. Face-forward the normal for the visible side so open shells
    // retain readable cool shading without exposing winding artifacts.
    var normal = normalize(in.normal);
    if !front_facing {
        normal = -normal;
    }
    let light = normalize(vec3<f32>(0.22, 0.42, 0.88));
    let diffuse = dot(normal, light);
    if in.lighting > 2.5 {
        // AutoCAD's Conceptual palette is a light olive warm side and a
        // restrained slate-blue cool side. Scale it by material luminance and
        // retain a little of the source color so colored solids still differ.
        let material_luma = dot(in.color.rgb, vec3<f32>(0.30, 0.59, 0.11));
        let material_scale = clamp(material_luma / 0.54, 0.75, 1.15);
        // These are linear-light values; the sRGB render target encodes them
        // to the familiar pale olive and slate-blue viewport colors.
        let warm_base = vec3<f32>(0.47, 0.56, 0.265) * material_scale;
        let cool_base = vec3<f32>(0.03, 0.11, 0.33) * material_scale;
        let warm = mix(warm_base, in.color.rgb, 0.30);
        let cool = mix(cool_base, in.color.rgb, 0.30);
        let conceptual_light = normalize(vec3<f32>(-0.40, 0.70, 0.60));
        let factor = clamp(dot(normal, conceptual_light) * 0.5 + 0.5, 0.0, 1.0);
        let gooch = mix(cool, warm, factor);
        let shade = 0.82 + 0.18 * max(diffuse, 0.0);
        return vec4<f32>(gooch * shade, in.color.a);
    }

    let shade = 0.28 + 0.72 * max(diffuse, 0.0);
    return vec4<f32>(in.color.rgb * shade, in.color.a);
}

// --- Analytic closed surfaces -------------------------------------------
//
// A filled analytic face is uploaded as a descriptor instead of a triangle
// mesh. Each patch is drawn as one full-screen triangle and evaluated per
// fragment, so its silhouette follows the true surface at any zoom.

struct SurfacePatch {
    // x: 0 sphere, 1 torus. y: sense flip. z: lighting model.
    kind: vec4<f32>,
    color: vec4<f32>,
    // xyz center. w sphere radius.
    center: vec4<f32>,
    // xyz torus axis. w major radius.
    axis: vec4<f32>,
    // xyz ring east. w minor radius.
    east: vec4<f32>,
    // xyz ring north.
    north: vec4<f32>,
}

@group(2) @binding(0)
var<storage, read> patches: array<SurfacePatch>;

struct SurfaceOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) @interpolate(flat) surface_index: u32,
}

@vertex
fn vs_surface(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> SurfaceOut {
    var corners = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return SurfaceOut(vec4<f32>(corners[vertex], 0.0, 1.0), instance);
}

fn ray_through(ndc: vec2<f32>) -> vec3<f32> {
    let near4 = camera.inv_view_proj * vec4<f32>(ndc, 0.0, 1.0);
    let far4 = camera.inv_view_proj * vec4<f32>(ndc, 1.0, 1.0);
    return far4.xyz / far4.w - near4.xyz / near4.w;
}

fn hit_sphere(o: vec3<f32>, d: vec3<f32>, c: vec3<f32>, r: f32) -> f32 {
    let oc = o - c;
    let b = dot(oc, d);
    let disc = b * b - (dot(oc, oc) - r * r);
    if disc < 0.0 {
        return -1.0;
    }
    let s = sqrt(disc);
    var t = -b - s;
    if t < 0.0 {
        t = -b + s;
    }
    return t;
}

fn torus_value(p: vec3<f32>, c: vec3<f32>, a: vec3<f32>, major: f32, minor: f32) -> f32 {
    let q = p - c;
    let qa = dot(q, a);
    let qp = q - a * qa;
    let s = dot(qp, qp) + qa * qa + major * major - minor * minor;
    return s * s - 4.0 * major * major * dot(qp, qp);
}

/// Nearest positive root of the torus quartic. A bounded sign search over the
/// bounding-sphere chord is robust and keeps the silhouette smooth; a grazing
/// contact still lands within half a step, well under a pixel.
fn hit_torus(
    o: vec3<f32>,
    d: vec3<f32>,
    c: vec3<f32>,
    a: vec3<f32>,
    major: f32,
    minor: f32,
) -> f32 {
    let oc = o - c;
    let b = dot(oc, d);
    let reach = major + minor;
    let disc = b * b - (dot(oc, oc) - reach * reach);
    if disc < 0.0 {
        return -1.0;
    }
    let s = sqrt(disc);
    let t1 = -b + s;
    if t1 < 0.0 {
        return -1.0;
    }
    let t0 = max(-b - s, 0.0);
    let steps = 64;
    var prev_t = t0;
    var prev = torus_value(o + d * prev_t, c, a, major, minor);
    if abs(prev) < 1e-6 {
        return prev_t;
    }
    for (var i = 1; i <= steps; i++) {
        let t = t0 + (t1 - t0) * f32(i) / f32(steps);
        let value = torus_value(o + d * t, c, a, major, minor);
        if value * prev <= 0.0 {
            var lo = prev_t;
            var hi = t;
            var flo = prev;
            for (var k = 0; k < 24; k++) {
                let mid = (lo + hi) * 0.5;
                let fm = torus_value(o + d * mid, c, a, major, minor);
                if flo * fm <= 0.0 {
                    hi = mid;
                } else {
                    lo = mid;
                    flo = fm;
                }
            }
            return (lo + hi) * 0.5;
        }
        prev_t = t;
        prev = value;
    }
    return -1.0;
}

fn surface_normal(surface: SurfacePatch, hit: vec3<f32>, ray: vec3<f32>) -> vec3<f32> {
    var normal = surface.axis.xyz;
    if surface.kind.x < 0.5 {
        normal = normalize(hit - surface.center.xyz);
    } else {
        let q = hit - surface.center.xyz;
        let qa = dot(q, surface.axis.xyz);
        let radial = q - surface.axis.xyz * qa;
        let length_radial = length(radial);
        if length_radial > 1e-6 {
            let tube = surface.center.xyz + radial / length_radial * surface.axis.w;
            normal = normalize(hit - tube);
        }
    }
    if surface.kind.y > 0.5 {
        normal = -normal;
    }
    // Two-sided: face the viewer so open or reversed shells stay readable.
    if dot(normal, ray) > 0.0 {
        normal = -normal;
    }
    return normalize(normal);
}

@fragment
fn fs_surface(in: SurfaceOut) -> FragOut {
    let surface = patches[in.surface_index];
    let ndc = frag_ndc(in.clip);
    let delta = ray_through(ndc);
    let span = length(delta);
    if span <= 1e-9 {
        discard;
    }
    let dir = delta / span;
    let near4 = camera.inv_view_proj * vec4<f32>(ndc, 0.0, 1.0);
    let origin = near4.xyz / near4.w;
    var t = -1.0;
    if surface.kind.x < 0.5 {
        t = hit_sphere(origin, dir, surface.center.xyz, surface.center.w);
    } else {
        t = hit_torus(origin, dir, surface.center.xyz, surface.axis.xyz, surface.axis.w, surface.east.w);
    }
    if t < 0.0 || t > span {
        discard;
    }
    let hit = origin + dir * t;
    let clip = camera.view_proj * vec4<f32>(hit, 1.0);
    let depth = clamp(clip.z / max(clip.w, 1e-6), 0.0, 1.0);
    let normal = surface_normal(surface, hit, dir);
    let lighting = surface.kind.z;
    let color = surface.color;
    if color.a <= 0.001 {
        discard;
    }
    if lighting < 0.5 {
        return FragOut(color, depth);
    }
    if lighting < 1.5 {
        return FragOut(vec4<f32>(color.rgb * 0.72, color.a), depth);
    }
    let light = normalize(vec3<f32>(0.22, 0.42, 0.88));
    let diffuse = dot(normal, light);
    if lighting > 2.5 {
        let material_luma = dot(color.rgb, vec3<f32>(0.30, 0.59, 0.11));
        let material_scale = clamp(material_luma / 0.54, 0.75, 1.15);
        let warm_base = vec3<f32>(0.47, 0.56, 0.265) * material_scale;
        let cool_base = vec3<f32>(0.03, 0.11, 0.33) * material_scale;
        let warm = mix(warm_base, color.rgb, 0.30);
        let cool = mix(cool_base, color.rgb, 0.30);
        let conceptual_light = normalize(vec3<f32>(-0.40, 0.70, 0.60));
        let factor = clamp(dot(normal, conceptual_light) * 0.5 + 0.5, 0.0, 1.0);
        let gooch = mix(cool, warm, factor);
        let shade = 0.82 + 0.18 * max(diffuse, 0.0);
        return FragOut(vec4<f32>(gooch * shade, color.a), depth);
    }
    let shade = 0.28 + 0.72 * max(diffuse, 0.0);
    return FragOut(vec4<f32>(color.rgb * shade, color.a), depth);
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
    let alpha = in.color.a * coverage;
    if alpha <= 0.001 {
        discard;
    }
    return vec4<f32>(in.color.rgb, alpha);
}

struct PictureIn {
    @location(0) pos: vec4<f32>,
    @location(1) uv: vec2<f32>,
}

struct PictureOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_picture(in: PictureIn) -> PictureOut {
    let clip = camera.view_proj * vec4<f32>(in.pos.xyz, 1.0);
    return PictureOut(clip, in.uv);
}

@fragment
fn fs_picture(in: PictureOut) -> @location(0) vec4<f32> {
    let color = textureSample(atlas_texture, atlas_sampler, in.uv);
    if color.a <= 0.001 {
        discard;
    }
    return color;
}
