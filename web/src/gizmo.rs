//! Rust port of the browser ViewCube/ViewGizmo.
//!
//! The component owns the gizmo canvas, draws the cube, handles hover and
//! drag-orbit input, and starts short camera animations. It drives the same
//! [`Viewer`] camera API that JavaScript used to call directly.

use std::cmp::Ordering;

use acadrust::Vector3;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

use crate::Viewer;

const TAU: f64 = std::f64::consts::TAU;
const PI: f64 = std::f64::consts::PI;
const FACE_INSET: f64 = 0.74;
const EDGE_LENGTH: f64 = 0.56;
const EDGE_WIDTH: f64 = 0.15;
const CORNER_SIZE: f64 = 0.18;

const FACES: [(&str, Vector3); 6] = [
    ("TOP", Vector3::new(0.0, 0.0, 1.0)),
    ("BOTTOM", Vector3::new(0.0, 0.0, -1.0)),
    ("FRONT", Vector3::new(0.0, -1.0, 0.0)),
    ("BACK", Vector3::new(0.0, 1.0, 0.0)),
    ("LEFT", Vector3::new(-1.0, 0.0, 0.0)),
    ("RIGHT", Vector3::new(1.0, 0.0, 0.0)),
];

const COMPASS: [(&str, Vector3, Vector3); 4] = [
    (
        "N",
        Vector3::new(0.0, 1.0, 0.0),
        Vector3::new(0.0, -1.0, 0.0),
    ),
    (
        "E",
        Vector3::new(1.0, 0.0, 0.0),
        Vector3::new(-1.0, 0.0, 0.0),
    ),
    (
        "S",
        Vector3::new(0.0, -1.0, 0.0),
        Vector3::new(0.0, 1.0, 0.0),
    ),
    (
        "W",
        Vector3::new(-1.0, 0.0, 0.0),
        Vector3::new(1.0, 0.0, 0.0),
    ),
];

const VIEW_NAMES: [(&str, Vector3); 10] = [
    ("Top", Vector3::new(0.0, 0.0, 1.0)),
    ("Bottom", Vector3::new(0.0, 0.0, -1.0)),
    ("Front", Vector3::new(0.0, -1.0, 0.0)),
    ("Back", Vector3::new(0.0, 1.0, 0.0)),
    ("Left", Vector3::new(-1.0, 0.0, 0.0)),
    ("Right", Vector3::new(1.0, 0.0, 0.0)),
    ("SW Isometric", Vector3::new(-1.0, -1.0, 1.0)),
    ("SE Isometric", Vector3::new(1.0, -1.0, 1.0)),
    ("NE Isometric", Vector3::new(1.0, 1.0, 1.0)),
    ("NW Isometric", Vector3::new(-1.0, 1.0, 1.0)),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum PartKind {
    Face,
    Edge,
    Corner,
}

struct Part {
    kind: PartKind,
    name: String,
    direction: Vector3,
    normal: Vector3,
    label: Option<String>,
    anchor: Option<Vector3>,
    corners: [Vector3; 4],
}

#[derive(Clone, Copy)]
struct Orientation {
    dir: Vector3,
    right: Vector3,
    up: Vector3,
    twist: f64,
}

#[derive(Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
}

impl Point {
    fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }
}

#[derive(Clone, Copy)]
struct Button {
    x: f64,
    y: f64,
    r: f64,
}

#[derive(Clone, Copy)]
struct Layout {
    width: f64,
    height: f64,
    center: Point,
    unit: f64,
    home: Button,
    roll_left: Button,
    roll_right: Button,
}

#[derive(Clone, Copy)]
struct Projected {
    sx: f64,
    sy: f64,
    depth: f64,
}

struct DrawnPart<'a> {
    part: &'a Part,
    projected: Vec<Projected>,
    depth: f64,
}

#[derive(Clone, Copy)]
enum Action {
    Home,
    RollLeft,
    RollRight,
    Direction(Vector3),
}

struct Hit {
    name: String,
    action: Action,
}

struct NavDrag {
    id: i32,
    x: f64,
    y: f64,
    moved: bool,
    hit: Option<Hit>,
}

#[derive(Clone, Copy)]
struct Anim {
    from: Vector3,
    to: Vector3,
    from_twist: f64,
    delta_twist: f64,
    begun: f64,
}

fn normalize(value: Vector3) -> Vector3 {
    if value.length() > 1e-8 {
        value.normalize()
    } else {
        Vector3::UNIT_Z
    }
}

fn reference_axes(normal: Vector3) -> (Vector3, Vector3) {
    let n = normalize(normal);
    let axis = if n.x.abs() < 1.0 / 64.0 && n.y.abs() < 1.0 / 64.0 {
        Vector3::UNIT_Y
    } else {
        Vector3::UNIT_Z
    };
    let x = normalize(axis.cross(&n));
    (x, normalize(n.cross(&x)))
}

fn project(point: Vector3, orient: &Orientation, center: Point, unit: f64) -> Projected {
    Projected {
        sx: center.x + orient.right.dot(&point) * unit,
        sy: center.y - orient.up.dot(&point) * unit,
        depth: orient.dir.dot(&point),
    }
}

fn point_in_poly(x: f64, y: f64, poly: &[Projected]) -> bool {
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let yi = poly[i].sy;
        let yj = poly[j].sy;
        let xi = poly[i].sx;
        let xj = poly[j].sx;
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn view_name(dir: Vector3) -> String {
    let mut best = "Free orbit";
    let mut score = 0.985;
    for (name, direction) in VIEW_NAMES.iter() {
        let facing = dir.dot(&normalize(*direction));
        if facing > score {
            score = facing;
            best = name;
        }
    }
    best.to_string()
}

fn slerp_dir(a: Vector3, b: Vector3, t: f64) -> Vector3 {
    let cosine = a.dot(&b).clamp(-1.0, 1.0);
    if cosine < -0.9995 {
        let axis = if a.z.abs() < 0.9 {
            normalize(a.cross(&Vector3::UNIT_Z))
        } else {
            normalize(a.cross(&Vector3::UNIT_Y))
        };
        return if t < 0.5 {
            slerp_dir(a, axis, t * 2.0)
        } else {
            slerp_dir(axis, b, t * 2.0 - 1.0)
        };
    }
    let theta = cosine.acos();
    if theta < 1e-4 {
        return normalize(a + (b - a) * t);
    }
    let span = theta.sin();
    let w0 = ((1.0 - t) * theta).sin() / span;
    let w1 = (t * theta).sin() / span;
    a * w0 + b * w1
}

fn device_pixel_ratio() -> f64 {
    web_sys::window()
        .map(|window| window.device_pixel_ratio())
        .filter(|ratio| ratio.is_finite() && *ratio > 0.0)
        .unwrap_or(1.0)
}

fn now_ms() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map(|performance| performance.now())
        .unwrap_or(0.0)
}

fn build_parts() -> Vec<Part> {
    let mut parts = Vec::new();

    for (name, normal) in FACES.iter() {
        let (u, v) = reference_axes(*normal);
        let center = *normal;
        let inset = FACE_INSET;
        parts.push(Part {
            kind: PartKind::Face,
            name: name.to_string(),
            direction: *normal,
            normal: *normal,
            label: Some(name.to_string()),
            anchor: None,
            corners: [
                center + u * inset + v * inset,
                center + u * -inset + v * inset,
                center + u * -inset + v * -inset,
                center + u * inset + v * -inset,
            ],
        });
    }

    for i in 0..FACES.len() {
        for j in (i + 1)..FACES.len() {
            let n1 = FACES[i].1;
            let n2 = FACES[j].1;
            if n1.dot(&n2).abs() > 0.1 {
                continue;
            }
            let along = normalize(n1.cross(&n2));
            let mid = n1 + n2;
            let outward = normalize(mid);
            let side = normalize(along.cross(&outward));
            let origin = mid + outward * 0.02;
            let hl = EDGE_LENGTH;
            let hw = EDGE_WIDTH;
            parts.push(Part {
                kind: PartKind::Edge,
                name: format!("{}-{}", FACES[i].0, FACES[j].0),
                direction: normalize(mid),
                normal: outward,
                label: None,
                anchor: None,
                corners: [
                    origin + along * hl + side * hw,
                    origin + along * -hl + side * hw,
                    origin + along * -hl + side * -hw,
                    origin + along * hl + side * -hw,
                ],
            });
        }
    }

    for i in 0..FACES.len() {
        for j in (i + 1)..FACES.len() {
            for k in (j + 1)..FACES.len() {
                let n1 = FACES[i].1;
                let n2 = FACES[j].1;
                let n3 = FACES[k].1;
                if n1.dot(&n2).abs() > 0.1 || n1.dot(&n3).abs() > 0.1 || n2.dot(&n3).abs() > 0.1 {
                    continue;
                }
                let pos = n1 + n2 + n3;
                let outward = normalize(pos);
                let (u, v) = reference_axes(outward);
                let s = CORNER_SIZE;
                parts.push(Part {
                    kind: PartKind::Corner,
                    name: format!("{}-{}-{}", FACES[i].0, FACES[j].0, FACES[k].0),
                    direction: outward,
                    normal: outward,
                    label: None,
                    anchor: Some(pos),
                    corners: [
                        pos + u * s + v * s,
                        pos + u * -s + v * s,
                        pos + u * -s + v * -s,
                        pos + u * s + v * -s,
                    ],
                });
            }
        }
    }

    parts
}

fn read_orientation(viewer: &Viewer) -> Orientation {
    let value = viewer.orientation();
    if value.len() < 10 {
        return Orientation::default();
    }
    Orientation {
        dir: Vector3::new(value[0], value[1], value[2]),
        twist: value[3],
        right: Vector3::new(value[4], value[5], value[6]),
        up: Vector3::new(value[7], value[8], value[9]),
    }
}

impl Default for Orientation {
    fn default() -> Self {
        Orientation {
            dir: Vector3::UNIT_Z,
            right: Vector3::UNIT_X,
            up: Vector3::UNIT_Y,
            twist: 0.0,
        }
    }
}

/// A browser ViewCube gizmo rendered on a 2D canvas.
#[wasm_bindgen]
pub struct ViewCube {
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    parts: Vec<Part>,
    hover: Option<String>,
    drag: Option<NavDrag>,
    anim: Option<Anim>,
}

#[wasm_bindgen]
impl ViewCube {
    /// Create a gizmo tied to `canvas`. Call [`ViewCube::draw`] to paint it.
    #[wasm_bindgen(constructor)]
    pub fn new(canvas: HtmlCanvasElement) -> Result<ViewCube, JsValue> {
        let ctx = canvas
            .get_context("2d")?
            .ok_or_else(|| JsValue::from_str("2d canvas context is unavailable"))?
            .dyn_into::<CanvasRenderingContext2d>()?;
        Ok(ViewCube {
            canvas,
            ctx,
            parts: build_parts(),
            hover: None,
            drag: None,
            anim: None,
        })
    }

    /// Draw the cube in the orientation of `viewer`.
    pub fn draw(&mut self, viewer: &Viewer) -> Result<(), JsValue> {
        let orient = read_orientation(viewer);
        self.draw_nav(&orient, true)
    }

    /// Draw the disabled placeholder used when no drawing is loaded.
    pub fn draw_idle(&mut self) -> Result<(), JsValue> {
        self.draw_nav(&Orientation::default(), false)
    }

    /// Press a pointer on the gizmo. `x` and `y` are CSS pixels.
    pub fn pointer_down(&mut self, viewer: &Viewer, x: f64, y: f64, pointer_id: i32) {
        self.anim = None;
        let orient = read_orientation(viewer);
        let layout = self.layout();
        let hit = self.pick(x, y, &orient, &layout);
        self.drag = Some(NavDrag {
            id: pointer_id,
            x,
            y,
            moved: false,
            hit,
        });
    }

    /// Move a pointer over the gizmo, orbiting while dragging.
    pub fn pointer_move(
        &mut self,
        viewer: &mut Viewer,
        x: f64,
        y: f64,
        pointer_id: i32,
    ) -> Result<(), JsValue> {
        let mut orbit = None;
        if let Some(drag) = self.drag.as_mut() {
            if drag.id == pointer_id {
                let dx = x - drag.x;
                let dy = y - drag.y;
                if !drag.moved && dx * dx + dy * dy < 9.0 {
                    return Ok(());
                }
                drag.moved = true;
                drag.x = x;
                drag.y = y;
                orbit = Some((dx, dy));
            }
        }
        if let Some((dx, dy)) = orbit {
            viewer.orbit(-dx / 78.0 * PI, dy / 78.0 * PI)?;
            return self.draw(&*viewer);
        }

        let orient = read_orientation(viewer);
        let layout = self.layout();
        let next = self.pick(x, y, &orient, &layout).map(|hit| hit.name);
        if next != self.hover {
            let pointer = next.is_some();
            self.set_cursor(pointer);
            self.hover = next;
            self.draw(&*viewer)?;
        }
        Ok(())
    }

    /// Release a pointer. Returns true when a camera animation was started, in
    /// which case the caller should call [`ViewCube::tick`] each frame.
    pub fn pointer_up(&mut self, viewer: &mut Viewer, pointer_id: i32) -> Result<bool, JsValue> {
        let Some(drag) = self.drag.take() else {
            self.draw(&*viewer)?;
            return Ok(false);
        };
        if drag.id != pointer_id {
            self.drag = Some(drag);
            return Ok(false);
        }
        let mut animating = false;
        if !drag.moved {
            if let Some(hit) = drag.hit {
                animating = self.activate(viewer, hit)?;
            }
        }
        self.draw(&*viewer)?;
        Ok(animating)
    }

    /// Clear hover feedback when the pointer leaves the gizmo.
    pub fn pointer_leave(&mut self, viewer: &Viewer) -> Result<(), JsValue> {
        if self.drag.is_some() || self.hover.is_none() {
            return Ok(());
        }
        self.hover = None;
        self.set_cursor(false);
        self.draw(viewer)
    }

    /// Advance the current snap animation. Returns true while more frames are
    /// needed. The caller should call this from `requestAnimationFrame`.
    pub fn tick(&mut self, viewer: &mut Viewer) -> Result<bool, JsValue> {
        let Some(anim) = self.anim else {
            return Ok(false);
        };
        let t = ((now_ms() - anim.begun) / 260.0).clamp(0.0, 1.0);
        let eased = if t < 0.5 {
            2.0 * t * t
        } else {
            1.0 - ((-2.0 * t + 2.0).powi(2)) / 2.0
        };
        let dir = slerp_dir(anim.from, anim.to, eased);
        let roll = anim.from_twist + anim.delta_twist * eased;
        viewer.set_oriented(dir.x, dir.y, dir.z, roll)?;
        let more = t < 1.0;
        if !more {
            self.anim = None;
        }
        self.draw(&*viewer)?;
        Ok(more)
    }

    /// Cancel any running snap animation.
    pub fn stop(&mut self) {
        self.anim = None;
    }

    /// True while a snap animation is running.
    pub fn is_animating(&self) -> bool {
        self.anim.is_some()
    }
}

impl ViewCube {
    fn layout(&self) -> Layout {
        let width = {
            let value = self.canvas.client_width() as f64;
            if value > 0.0 {
                value
            } else {
                180.0
            }
        };
        let height = {
            let value = self.canvas.client_height() as f64;
            if value > 0.0 {
                value
            } else {
                214.0
            }
        };
        Layout {
            width,
            height,
            center: Point::new(width * 0.5, height * 0.48),
            unit: 31.0,
            home: Button {
                x: 18.0,
                y: 18.0,
                r: 12.0,
            },
            roll_left: Button {
                x: 28.0,
                y: 46.0,
                r: 13.0,
            },
            roll_right: Button {
                x: width - 28.0,
                y: 46.0,
                r: 13.0,
            },
        }
    }

    fn set_cursor(&self, pointer: bool) {
        let _ = self
            .canvas
            .style()
            .set_property("cursor", if pointer { "pointer" } else { "default" });
    }

    fn draw_nav(&mut self, orient: &Orientation, enabled: bool) -> Result<(), JsValue> {
        let ratio = device_pixel_ratio();
        let layout = self.layout();
        let pixel_width = (layout.width * ratio).round().max(1.0) as u32;
        let pixel_height = (layout.height * ratio).round().max(1.0) as u32;
        if self.canvas.width() != pixel_width {
            self.canvas.set_width(pixel_width);
        }
        if self.canvas.height() != pixel_height {
            self.canvas.set_height(pixel_height);
        }

        let ctx = &self.ctx;
        ctx.set_transform(ratio, 0.0, 0.0, ratio, 0.0, 0.0)?;
        ctx.clear_rect(0.0, 0.0, layout.width, layout.height);

        let base_alpha = if enabled { 1.0 } else { 0.45 };
        ctx.set_global_alpha(base_alpha);

        ctx.begin_path();
        for step in 0..64 {
            let angle = (step as f64 / 64.0) * TAU;
            let point = project(
                Vector3::new(angle.cos() * 1.72, angle.sin() * 1.72, -1.2),
                orient,
                layout.center,
                layout.unit,
            );
            if step == 0 {
                ctx.move_to(point.sx, point.sy);
            } else {
                ctx.line_to(point.sx, point.sy);
            }
        }
        ctx.close_path();
        ctx.set_stroke_style_str("rgba(176, 190, 201, 0.55)");
        ctx.set_line_width(1.0);
        ctx.stroke();

        ctx.set_fill_style_str("#d5dee6");
        ctx.set_font("700 11px system-ui, sans-serif");
        ctx.set_text_align("center");
        ctx.set_text_baseline("middle");
        let flatness = orient.dir.dot(&Vector3::UNIT_Z).abs();
        if flatness > 0.2 {
            for (letter, at, _) in COMPASS.iter() {
                let point = project(
                    Vector3::new(at.x * 2.08, at.y * 2.08, -1.2),
                    orient,
                    layout.center,
                    layout.unit,
                );
                ctx.set_global_alpha(base_alpha * (flatness + 0.25).min(1.0));
                ctx.fill_text(letter, point.sx, point.sy)?;
            }
        }
        ctx.set_global_alpha(base_alpha);

        let mut drawn: Vec<DrawnPart<'_>> = self
            .parts
            .iter()
            .map(|part| {
                let projected: Vec<Projected> = part
                    .corners
                    .iter()
                    .map(|corner| project(*corner, orient, layout.center, layout.unit))
                    .collect();
                let depth =
                    projected.iter().map(|point| point.depth).sum::<f64>() / projected.len() as f64;
                DrawnPart {
                    part,
                    projected,
                    depth,
                }
            })
            .collect();
        drawn.sort_by(|a, b| a.depth.partial_cmp(&b.depth).unwrap_or(Ordering::Equal));

        for part in &drawn {
            let facing = part.part.normal.dot(&orient.dir);
            let visible = if part.part.kind == PartKind::Face {
                facing > 0.02
            } else {
                facing > -0.12
            };
            if !visible {
                continue;
            }
            self.paint_part(part, orient.dir)?;
        }
        for part in &drawn {
            self.paint_label(part.part, orient, layout.center, layout.unit)?;
        }

        self.paint_arrow(
            layout.roll_left,
            false,
            self.hover.as_deref() == Some("roll-left"),
        )?;
        self.paint_arrow(
            layout.roll_right,
            true,
            self.hover.as_deref() == Some("roll-right"),
        )?;
        self.paint_home(layout.home, self.hover.as_deref() == Some("home"))?;

        ctx.set_global_alpha(if enabled { 0.9 } else { 0.4 });
        ctx.set_fill_style_str("#c5d0d8");
        ctx.set_font("12px system-ui, sans-serif");
        ctx.set_text_align("center");
        ctx.set_text_baseline("alphabetic");
        let label = if enabled {
            view_name(orient.dir)
        } else {
            "ViewCube".to_string()
        };
        ctx.fill_text(&label, layout.width * 0.5, layout.height - 8.0)?;
        ctx.set_global_alpha(1.0);
        Ok(())
    }

    fn paint_part(&self, drawn: &DrawnPart<'_>, orient_dir: Vector3) -> Result<(), JsValue> {
        let ctx = &self.ctx;
        ctx.begin_path();
        for (index, point) in drawn.projected.iter().enumerate() {
            if index == 0 {
                ctx.move_to(point.sx, point.sy);
            } else {
                ctx.line_to(point.sx, point.sy);
            }
        }
        ctx.close_path();

        let hot = self.hover.as_deref() == Some(drawn.part.name.as_str());
        let facing = drawn.part.normal.dot(&orient_dir).clamp(0.0, 1.0);
        let shade = 0.62 + 0.38 * facing;
        let (r, g, b) = if hot {
            (46.0, 144.0, 214.0)
        } else {
            let base = match drawn.part.kind {
                PartKind::Face => (214.0, 220.0, 226.0),
                PartKind::Edge => (150.0, 158.0, 166.0),
                PartKind::Corner => (176.0, 184.0, 192.0),
            };
            (base.0 * shade, base.1 * shade, base.2 * shade)
        };
        ctx.set_fill_style_str(&format!("rgb({}, {}, {})", r as i32, g as i32, b as i32));
        ctx.fill();
        Ok(())
    }

    fn paint_label(
        &self,
        part: &Part,
        orient: &Orientation,
        center: Point,
        unit: f64,
    ) -> Result<(), JsValue> {
        let Some(label) = part.label.as_deref() else {
            return Ok(());
        };
        if part.normal.dot(&orient.dir) < 0.42 {
            return Ok(());
        }
        let (u, _) = reference_axes(part.normal);
        let origin = project(part.normal, orient, center, unit);
        let right = project(part.normal + u, orient, center, unit);
        let rx = right.sx - origin.sx;
        let ry = right.sy - origin.sy;
        let span = rx.hypot(ry);
        if span < 8.0 {
            return Ok(());
        }

        let ctx = &self.ctx;
        ctx.save();
        ctx.translate(origin.sx, origin.sy)?;
        ctx.rotate(ry.atan2(rx))?;
        let scale = (span / 46.0).min(1.15);
        ctx.scale(scale, scale)?;
        ctx.set_fill_style_str(if self.hover.as_deref() == Some(part.name.as_str()) {
            "#f7fbff"
        } else {
            "#1c2630"
        });
        ctx.set_font("700 11px system-ui, sans-serif");
        ctx.set_text_align("center");
        ctx.set_text_baseline("middle");
        ctx.fill_text(label, 0.0, 0.0)?;
        ctx.restore();
        Ok(())
    }

    fn paint_arrow(&self, button: Button, clockwise: bool, hot: bool) -> Result<(), JsValue> {
        let ctx = &self.ctx;
        ctx.save();
        ctx.translate(button.x, button.y)?;
        let color = if hot { "#8ec8f8" } else { "#c5d0d8" };
        ctx.set_stroke_style_str(color);
        ctx.set_fill_style_str(color);
        ctx.set_line_width(1.6);
        ctx.begin_path();
        if clockwise {
            ctx.arc(0.0, 0.0, 9.0, PI * 0.15, PI * 1.35)?;
        } else {
            ctx.arc(0.0, 0.0, 9.0, PI * 0.85, PI * 1.85)?;
        }
        ctx.stroke();

        let tip = if clockwise { PI * 1.35 } else { PI * 0.85 };
        let x = tip.cos() * 9.0;
        let y = tip.sin() * 9.0;
        let tangent = tip + if clockwise { PI / 2.0 } else { -PI / 2.0 };
        ctx.begin_path();
        ctx.move_to(x, y);
        ctx.line_to(
            x - (tangent - 0.45).cos() * 6.0,
            y - (tangent - 0.45).sin() * 6.0,
        );
        ctx.line_to(
            x - (tangent + 0.55).cos() * 6.0,
            y - (tangent + 0.55).sin() * 6.0,
        );
        ctx.close_path();
        ctx.fill();
        ctx.restore();
        Ok(())
    }

    fn paint_home(&self, button: Button, hot: bool) -> Result<(), JsValue> {
        let ctx = &self.ctx;
        ctx.begin_path();
        ctx.arc(button.x, button.y, button.r, 0.0, TAU)?;
        ctx.set_fill_style_str(if hot {
            "#2e90d6"
        } else {
            "rgba(48, 59, 69, 0.92)"
        });
        ctx.fill();
        ctx.set_stroke_style_str("#9eb0bd");
        ctx.set_line_width(1.0);
        ctx.stroke();

        ctx.set_fill_style_str("#e8eef3");
        ctx.begin_path();
        ctx.move_to(button.x, button.y - 6.0);
        ctx.line_to(button.x + 6.5, button.y + 0.5);
        ctx.line_to(button.x + 3.5, button.y + 0.5);
        ctx.line_to(button.x + 3.5, button.y + 5.5);
        ctx.line_to(button.x - 3.5, button.y + 5.5);
        ctx.line_to(button.x - 3.5, button.y + 0.5);
        ctx.line_to(button.x - 6.5, button.y + 0.5);
        ctx.close_path();
        ctx.fill();
        Ok(())
    }

    fn pick(&self, x: f64, y: f64, orient: &Orientation, layout: &Layout) -> Option<Hit> {
        let buttons = [
            (layout.home, "home", Action::Home),
            (layout.roll_left, "roll-left", Action::RollLeft),
            (layout.roll_right, "roll-right", Action::RollRight),
        ];
        for (button, name, action) in buttons {
            let dx = x - button.x;
            let dy = y - button.y;
            if dx * dx + dy * dy <= button.r * button.r {
                return Some(Hit {
                    name: name.to_string(),
                    action,
                });
            }
        }

        let mut best: Option<(usize, i32, f64)> = None;
        for (index, part) in self.parts.iter().enumerate() {
            let facing = part.normal.dot(&orient.dir);
            let visible = if part.kind == PartKind::Face {
                facing > 0.05
            } else {
                facing > -0.05
            };
            if !visible {
                continue;
            }
            let projected: Vec<Projected> = part
                .corners
                .iter()
                .map(|corner| project(*corner, orient, layout.center, layout.unit))
                .collect();
            let depth =
                projected.iter().map(|point| point.depth).sum::<f64>() / projected.len() as f64;
            let mut hit = point_in_poly(x, y, &projected);
            if !hit {
                if let Some(anchor) = part.anchor {
                    let anchor = project(anchor, orient, layout.center, layout.unit);
                    let dx = x - anchor.sx;
                    let dy = y - anchor.sy;
                    hit = dx * dx + dy * dy <= 12.0 * 12.0;
                }
            }
            if !hit {
                continue;
            }
            let rank = match part.kind {
                PartKind::Corner => 3,
                PartKind::Edge => 2,
                PartKind::Face => 1,
            };
            let replace = match best {
                None => true,
                Some((_, best_rank, best_depth)) => {
                    rank > best_rank || (rank == best_rank && depth > best_depth)
                }
            };
            if replace {
                best = Some((index, rank, depth));
            }
        }
        if let Some((index, _, _)) = best {
            let part = &self.parts[index];
            return Some(Hit {
                name: part.name.clone(),
                action: Action::Direction(part.direction),
            });
        }

        if orient.dir.dot(&Vector3::UNIT_Z).abs() > 0.35 {
            for (letter, at, direction) in COMPASS.iter() {
                let point = project(
                    Vector3::new(at.x * 2.08, at.y * 2.08, -1.2),
                    orient,
                    layout.center,
                    layout.unit,
                );
                let dx = x - point.sx;
                let dy = y - point.sy;
                if dx * dx + dy * dy <= 12.0 * 12.0 {
                    return Some(Hit {
                        name: letter.to_string(),
                        action: Action::Direction(*direction),
                    });
                }
            }
        }

        None
    }

    fn activate(&mut self, viewer: &mut Viewer, hit: Hit) -> Result<bool, JsValue> {
        match hit.action {
            Action::Home => {
                self.anim = None;
                viewer.home()?;
                Ok(false)
            }
            Action::RollLeft => {
                let start = read_orientation(viewer);
                self.start_animation(&start, start.dir, start.twist - PI / 2.0);
                Ok(true)
            }
            Action::RollRight => {
                let start = read_orientation(viewer);
                self.start_animation(&start, start.dir, start.twist + PI / 2.0);
                Ok(true)
            }
            Action::Direction(direction) => {
                let start = read_orientation(viewer);
                self.start_animation(&start, direction, 0.0);
                Ok(true)
            }
        }
    }

    fn start_animation(&mut self, start: &Orientation, to: Vector3, to_twist: f64) {
        let mut delta = to_twist - start.twist;
        while delta > PI {
            delta -= TAU;
        }
        while delta < -PI {
            delta += TAU;
        }
        self.anim = Some(Anim {
            from: start.dir,
            to: normalize(to),
            from_twist: start.twist,
            delta_twist: delta,
            begun: now_ms(),
        });
    }
}
