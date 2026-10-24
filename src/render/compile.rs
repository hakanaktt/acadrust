//! Walk a drawing into a [`Scene`](super::Scene).

use std::collections::{HashMap, HashSet};

use crate::document::CadDocument;
use crate::entities::{EntityType, Insert, LwPolyline};
use crate::types::{Handle, Matrix3, Matrix4, Transform, Vector2, Vector3};

use super::curves::{
    arc_points, circle_points, ellipse_points, fan_triangles, point_chains, spline_points,
};
use super::dash::{fit_runs, polyline_length, runs_on_segments, SegmentPiece};
use super::scene::{DrawPath, EntityRef, FallbackEntry, Scene, SkippedEntity, Stroke};
use super::style::{layer_is_hidden, pattern_elements, resolve_style, Frame};
use super::visual_style;
use super::CompileOptions;

pub(crate) fn compile(doc: &CadDocument, options: &CompileOptions) -> Scene {
    let visual = visual_style::resolve(doc, &options.visual_style);
    let block = match &options.space {
        super::DisplaySpace::Model => Some("*Model_Space".to_string()),
        super::DisplaySpace::Layout(name) => super::sheet::layout_block_name(doc, name),
    };
    let Some(block) = block else {
        let mut scene = blank_scene(doc, options, visual, false);
        let name = match &options.space {
            super::DisplaySpace::Layout(name) => name.clone(),
            super::DisplaySpace::Model => "Model".to_string(),
        };
        scene.skipped.push(SkippedEntity {
            kind: "Layout".to_string(),
            reason: format!("layout {name} was not found"),
            source: EntityRef {
                handle: Handle::NULL,
                insert_path: Vec::new(),
            },
        });
        return scene;
    };
    let sheet = super::sheet::is_sheet_block(&block);
    let mut compiler = Compiler {
        doc,
        scene: blank_scene(doc, options, visual, sheet),
        stack: HashSet::new(),
        font_dirs: &options.font_dirs,
        fonts: HashMap::new(),
        sheet,
        pattern_scale: 1.0,
        viewport_frozen: HashSet::new(),
    };
    compiler.walk_block(&block, &Frame::model(), &Transform::identity(), &[]);
    compiler.scene
}

fn blank_scene(
    doc: &CadDocument,
    options: &CompileOptions,
    visual: super::visual_style::ResolvedVisualStyle,
    sheet: bool,
) -> Scene {
    Scene {
        strokes: Vec::new(),
        fills: Vec::new(),
        glyphs: Vec::new(),
        fallbacks: Vec::new(),
        skipped: Vec::new(),
        visual_style: visual,
        model_lineweight_px_per_mm: options.model_lineweight_px_per_mm,
        sheet_units_per_mm: if sheet {
            super::sheet::units_per_mm(doc.header.measurement)
        } else {
            0.0
        },
    }
}

struct Compiler<'a> {
    doc: &'a CadDocument,
    scene: Scene,
    stack: HashSet<String>,
    font_dirs: &'a [std::path::PathBuf],
    fonts: HashMap<String, Option<FaceFont>>,
    /// Layout tab. Lineweights are paper millimetres, and viewports open model space.
    sheet: bool,
    /// Extra multiplier for linetype lengths. Viewport contents use the
    /// viewport scale when `PSLTSCALE` is off, because the picture is already
    /// in paper units.
    pattern_scale: f64,
    viewport_frozen: HashSet<Handle>,
}

enum FaceFont {
    Shape(super::shx::ShapeFont),
    TrueType(std::path::PathBuf),
}

enum PreparedLabel {
    Strokes(Vec<Vec<Vector3>>),
    TrueType(std::path::PathBuf),
}

impl Compiler<'_> {
    fn walk_block(
        &mut self,
        name: &str,
        frame: &Frame,
        transform: &Transform,
        path: &[crate::types::Handle],
    ) {
        let Some(record) = self.doc.block_records.get(name) else {
            return;
        };
        if record.flags.is_xref
            || record.flags.is_xref_overlay
            || record.flags.is_external
            || !record.xref_path.is_empty()
        {
            return;
        }
        let key = name.to_ascii_lowercase();
        if !self.stack.insert(key.clone()) {
            return;
        }
        let handles = self.ordered_handles(record);
        for handle in handles {
            let Some(entity) = self.doc.get_entity(handle) else {
                continue;
            };
            self.draw(entity, frame, transform, path);
        }
        self.stack.remove(&key);
    }

    fn draw(
        &mut self,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        path: &[crate::types::Handle],
    ) {
        if entity.common().invisible {
            return;
        }
        let source = EntityRef {
            handle: entity.common().handle,
            insert_path: path.to_vec(),
        };
        let layer = effective_layer_of(entity.common(), frame);
        if layer_is_hidden(self.doc, &layer) {
            self.scene.skipped.push(SkippedEntity {
                kind: kind_of(entity).to_string(),
                reason: format!("layer {layer} is off or frozen"),
                source,
            });
            return;
        }
        if self.layer_frozen_in_viewport(&layer) {
            self.scene.skipped.push(SkippedEntity {
                kind: kind_of(entity).to_string(),
                reason: format!("layer {layer} is frozen in this viewport"),
                source,
            });
            return;
        }

        match entity {
            EntityType::Line(line) => {
                let points = world(transform, vec![line.start, line.end]);
                self.emit_dedicated(&points, entity, frame, source);
            }
            EntityType::LwPolyline(poly) => {
                if has_width(poly) {
                    let triangles = world(transform, lwpolyline_wide_triangles(poly));
                    if self.doc.header.fill_mode {
                        self.emit_fill(&triangles, entity, frame, source, false);
                    } else {
                        let points = world(transform, lwpolyline_points(poly));
                        self.emit_dedicated(&points, entity, frame, source);
                    }
                    return;
                }
                let points = world(transform, lwpolyline_points(poly));
                self.emit_dedicated(&points, entity, frame, source);
            }
            EntityType::Circle(circle) => {
                let points = world(transform, circle_points(circle));
                self.emit_dedicated(&points, entity, frame, source);
            }
            EntityType::Arc(arc) => {
                let points = world(transform, arc_points(arc));
                self.emit_dedicated(&points, entity, frame, source);
            }
            EntityType::Ellipse(ellipse) => {
                let points = world(transform, ellipse_points(ellipse));
                self.emit_dedicated(&points, entity, frame, source);
            }
            EntityType::Spline(spline) => {
                let mut points = world(transform, spline_points(spline));
                if spline.flags.closed || spline.cv_frame_visible {
                    if points.len() >= 2 {
                        points.push(points[0]);
                    }
                }
                if self.doc.header.spline_frame && spline.control_points.len() >= 2 {
                    let frame_points = world(transform, spline.control_points.clone());
                    self.emit_dedicated(&frame_points, entity, frame, source.clone());
                }
                self.emit_dedicated(&points, entity, frame, source);
            }
            EntityType::Helix(helix) => {
                let points = world(transform, spline_points(&helix.spline));
                self.emit_dedicated(&points, entity, frame, source.clone());
                let height = helix.turns * helix.turn_height;
                let axis = helix.axis_vector;
                if axis.length_squared() > 1e-18 && height.abs() > 1e-12 {
                    let end = helix.axis_base_point + axis.normalize() * height;
                    let axis_points = world(transform, vec![helix.axis_base_point, end]);
                    self.emit_dedicated(&axis_points, entity, frame, source);
                }
            }
            EntityType::Point(point) => {
                let mut size = self.doc.header.point_display_size;
                if size <= 1e-9 {
                    size = 1.0;
                }
                for chain in point_chains(point, self.doc.header.point_display_mode, size) {
                    let points = world(transform, chain);
                    if points.len() >= 2 && (points[1] - points[0]).length_squared() <= 1e-18 {
                        self.emit_dot(&points, entity, frame, source.clone());
                    } else {
                        self.emit_dedicated(&points, entity, frame, source.clone());
                    }
                }
            }
            EntityType::Polyline2D(poly) => {
                if polyline2d_has_width(poly) {
                    let triangles = world(transform, polyline2d_wide_triangles(poly));
                    if self.doc.header.fill_mode {
                        self.emit_fill(&triangles, entity, frame, source, false);
                    } else {
                        let points = world(transform, polyline2d_points(poly));
                        self.emit_dedicated(&points, entity, frame, source);
                    }
                    return;
                }
                let points = world(transform, polyline2d_points(poly));
                self.emit_dedicated(&points, entity, frame, source);
            }
            EntityType::Polyline(poly) => {
                let mut points: Vec<Vector3> =
                    poly.vertices.iter().map(|vertex| vertex.location).collect();
                if poly.flags.is_closed() && points.len() >= 2 {
                    points.push(points[0]);
                }
                self.emit_dedicated(&world(transform, points), entity, frame, source);
            }
            EntityType::Polyline3D(poly) => {
                let mut points: Vec<Vector3> =
                    poly.vertices.iter().map(|vertex| vertex.position).collect();
                if poly.flags.closed && points.len() >= 2 {
                    points.push(points[0]);
                }
                self.emit_dedicated(&world(transform, points), entity, frame, source);
            }
            EntityType::Solid(solid) => {
                let basis = Matrix3::arbitrary_axis(solid.normal);
                let corners: Vec<Vector3> = solid
                    .boundary_corners()
                    .into_iter()
                    .map(|corner| basis * corner)
                    .collect();
                let corners = world(transform, corners);
                if self.doc.header.fill_mode {
                    self.emit_fill(&fan_triangles(&corners), entity, frame, source, false);
                } else if corners.len() >= 2 {
                    let mut outline = corners;
                    outline.push(outline[0]);
                    self.emit_dedicated(&outline, entity, frame, source);
                }
            }
            EntityType::Face3D(face) => {
                let corners = [
                    face.first_corner,
                    face.second_corner,
                    face.third_corner,
                    face.fourth_corner,
                ];
                let hidden = [
                    face.invisible_edges.is_first_invisible(),
                    face.invisible_edges.is_second_invisible(),
                    face.invisible_edges.is_third_invisible(),
                    face.invisible_edges.is_fourth_invisible(),
                ];
                for index in 0..4 {
                    if hidden[index] {
                        continue;
                    }
                    let start = corners[index];
                    let end = corners[(index + 1) % 4];
                    if (end - start).length_squared() <= 1e-18 {
                        continue;
                    }
                    let points = world(transform, vec![start, end]);
                    self.emit_dedicated(&points, entity, frame, source.clone());
                }
            }
            EntityType::Wipeout(wipeout) => {
                let corners = wipeout.corners();
                let ring = world(transform, corners.to_vec());
                self.emit_fill(&fan_triangles(&ring), entity, frame, source, true);
            }
            EntityType::Hatch(hatch) => {
                if hatch.is_solid || hatch.gradient_color.is_enabled() {
                    let style = resolve_style(
                        self.doc,
                        entity.common(),
                        frame,
                        &self.scene.visual_style.clone(),
                        self.sheet,
                    );
                    let color_of =
                        |color: crate::types::Color| match color {
                            crate::types::Color::Rgb { r, g, b } => [r, g, b],
                            crate::types::Color::Index(index) => {
                                let (r, g, b) = crate::types::aci_table::aci_to_rgb(index)
                                    .unwrap_or((style.rgb[0], style.rgb[1], style.rgb[2]));
                                [r, g, b]
                            }
                            _ => style.rgb,
                        };
                    let (triangles, colors) = super::hatch_fill::solid_hatch(hatch, &color_of);
                    let triangles = world(transform, triangles);
                    if self.doc.header.fill_mode {
                        self.emit_fill_colors(&triangles, colors, entity, frame, source, false);
                    } else {
                        for ring in super::curves::path_rings(hatch) {
                            let mut points = world(
                                transform,
                                ring.into_iter()
                                    .map(|point| super::curves::to_world(hatch, point))
                                    .collect(),
                            );
                            if points.len() >= 2 {
                                points.push(points[0]);
                                self.emit_dedicated(&points, entity, frame, source.clone());
                            }
                        }
                    }
                    return;
                }
                let lines = entity.explode();
                if lines.is_empty() {
                    self.fallback(entity, frame, source);
                    return;
                }
                for line in lines {
                    if let EntityType::Line(segment) = line {
                        let points = world(transform, vec![segment.start, segment.end]);
                        self.emit_dedicated(&points, entity, frame, source.clone());
                    }
                }
            }
            EntityType::Text(text) => self.draw_label(
                &super::text_draw::GlyphLabel {
                    text: &text.value,
                    shape_number: None,
                    insertion: text.insertion_point,
                    alignment: text.alignment_point.unwrap_or(text.insertion_point),
                    use_alignment: text.horizontal_alignment
                        != crate::entities::TextHorizontalAlignment::Left
                        || text.vertical_alignment
                            != crate::entities::TextVerticalAlignment::Baseline,
                    height: text.height,
                    rotation: text.rotation,
                    width_factor: text.width_factor,
                    oblique: text.oblique_angle,
                    horizontal: text.horizontal_alignment as u8,
                    vertical: text.vertical_alignment as u8,
                    normal: text.normal,
                    backward: text.generation_flags & 2 != 0,
                    upside_down: text.generation_flags & 4 != 0,
                },
                &text.style,
                entity,
                frame,
                transform,
                source,
            ),
            EntityType::AttributeDefinition(attribute) => {
                if frame.in_insert || attribute.flags.invisible {
                    return;
                }
                self.draw_label(
                    &super::text_draw::GlyphLabel {
                        text: &attribute.default_value,
                        shape_number: None,
                        insertion: attribute.insertion_point,
                        alignment: attribute.alignment_point,
                        use_alignment: attribute.horizontal_alignment as u8 != 0
                            || attribute.vertical_alignment as u8 != 0,
                        height: attribute.height,
                        rotation: attribute.rotation,
                        width_factor: attribute.width_factor,
                        oblique: attribute.oblique_angle,
                        horizontal: attribute.horizontal_alignment as u8,
                        vertical: attribute.vertical_alignment as u8,
                        normal: attribute.normal,
                        backward: attribute.text_generation_flags & 2 != 0,
                        upside_down: attribute.text_generation_flags & 4 != 0,
                    },
                    &attribute.text_style,
                    entity,
                    frame,
                    transform,
                    source,
                );
            }
            EntityType::AttributeEntity(attribute) => {
                if attribute.flags.invisible {
                    return;
                }
                let text = if attribute.is_multiline {
                    attribute
                        .embedded_mtext
                        .as_ref()
                        .map(|mtext| mtext.value.as_str())
                        .unwrap_or(attribute.value.as_str())
                } else {
                    attribute.value.as_str()
                };
                self.draw_label(
                    &super::text_draw::GlyphLabel {
                        text,
                        shape_number: None,
                        insertion: attribute.insertion_point,
                        alignment: attribute.alignment_point,
                        use_alignment: attribute.horizontal_alignment as u8 != 0
                            || attribute.vertical_alignment as u8 != 0,
                        height: attribute.height,
                        rotation: attribute.rotation,
                        width_factor: attribute.width_factor,
                        oblique: attribute.oblique_angle,
                        horizontal: attribute.horizontal_alignment as u8,
                        vertical: attribute.vertical_alignment as u8,
                        normal: attribute.normal,
                        backward: attribute.text_generation_flags & 2 != 0,
                        upside_down: attribute.text_generation_flags & 4 != 0,
                    },
                    &attribute.text_style,
                    entity,
                    frame,
                    transform,
                    source,
                );
            }
            EntityType::Shape(shape) => self.draw_label(
                &super::text_draw::GlyphLabel {
                    text: &shape.shape_name,
                    shape_number: Some(shape.shape_number as u16),
                    insertion: shape.insertion_point,
                    alignment: shape.insertion_point,
                    use_alignment: false,
                    height: shape.size,
                    rotation: shape.rotation,
                    width_factor: shape.relative_x_scale,
                    oblique: shape.oblique_angle,
                    horizontal: 0,
                    vertical: 0,
                    normal: shape.normal,
                    backward: false,
                    upside_down: false,
                },
                &shape.style_name,
                entity,
                frame,
                transform,
                source,
            ),
            EntityType::MText(_)
            | EntityType::Dimension(_)
            | EntityType::Leader(_)
            | EntityType::MultiLeader(_)
            | EntityType::Tolerance(_)
            | EntityType::MLine(_)
            | EntityType::Table(_) => self.draw_exploded(entity, frame, transform, path, source),
            EntityType::Insert(insert) => self.draw_insert(insert, frame, transform, path),
            EntityType::Block(_) | EntityType::BlockEnd(_) | EntityType::Seqend(_) => {}
            EntityType::Extended(extended) => match &extended.data {
                crate::entities::ExtendedEntityData::LayoutPrintConfig(_)
                | crate::entities::ExtendedEntityData::Format(_)
                | crate::entities::ExtendedEntityData::DynamicBlock(_) => {}
                crate::entities::ExtendedEntityData::Legacy(data) => match data {
                    crate::entities::LegacyEntityData::Load { .. }
                    | crate::entities::LegacyEntityData::Jump { .. } => {}
                    crate::entities::LegacyEntityData::Repeat
                    | crate::entities::LegacyEntityData::EndRepeat { .. } => {
                        self.fallback(entity, frame, source);
                    }
                },
                crate::entities::ExtendedEntityData::SectionObject(data) => {
                    self.draw_section_object(data, entity, frame, transform, source);
                }
                _ => self.fallback(entity, frame, source),
            },
            EntityType::Viewport(viewport) => {
                self.draw_viewport(viewport, entity, frame, transform, source);
            }
            EntityType::ViewBorder(border) => {
                self.draw_view_border(border, entity, frame, transform, source);
            }
            EntityType::SectionSymbol(symbol) => {
                self.draw_section_symbol(symbol, entity, frame, transform, source);
            }
            EntityType::Ray(_)
            | EntityType::XLine(_)
            | EntityType::Mesh(_)
            | EntityType::RasterImage(_)
            | EntityType::Solid3D(_)
            | EntityType::Region(_)
            | EntityType::Body(_)
            | EntityType::Surface(_)
            | EntityType::PolyfaceMesh(_)
            | EntityType::Underlay(_)
            | EntityType::Ole2Frame(_)
            | EntityType::PolygonMesh(_)
            | EntityType::Light(_)
            | EntityType::Unknown(_) => self.fallback(entity, frame, source),
        }
    }

    fn layer_frozen_in_viewport(&self, name: &str) -> bool {
        if self.viewport_frozen.is_empty() {
            return false;
        }
        self.doc.layers.get(name).is_some_and(|layer| {
            !layer.handle.is_null() && self.viewport_frozen.contains(&layer.handle)
        })
    }

    fn draw_viewport(
        &mut self,
        viewport: &crate::entities::Viewport,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        if !self.sheet {
            self.scene.skipped.push(SkippedEntity {
                kind: "Viewport".to_string(),
                reason: "viewport belongs to a layout".to_string(),
                source,
            });
            return;
        }
        if !super::sheet::viewport_shows_model(viewport) {
            let reason = if viewport.id == 1 {
                "paper-space viewport"
            } else {
                "viewport is off"
            };
            self.scene.skipped.push(SkippedEntity {
                kind: "Viewport".to_string(),
                reason: reason.to_string(),
                source,
            });
            return;
        }
        let scale = super::sheet::viewport_scale(viewport);
        let frozen: HashSet<Handle> = viewport.frozen_layers.iter().copied().collect();
        let custom = self.boundary_polygon(viewport.clip_boundary_handle);
        let rect = super::sheet::viewport_rectangle(viewport);
        let clip = super::sheet::map_polygon(transform, custom.as_deref().unwrap_or(&rect));
        let border = custom.is_none().then_some(rect);
        let center_z = viewport.center.z;
        let to_paper = super::sheet::model_to_paper(viewport).then(transform);
        let saved_scale = self.pattern_scale;
        let saved_frozen = std::mem::replace(&mut self.viewport_frozen, frozen);
        self.pattern_scale = if self.doc.header.paper_space_linetype_scaling {
            1.0
        } else {
            scale.abs()
        };
        let stroke_from = self.scene.strokes.len();
        let fill_from = self.scene.fills.len();
        let glyph_from = self.scene.glyphs.len();
        self.walk_block("*Model_Space", &Frame::model(), &to_paper, &[]);
        self.pattern_scale = saved_scale;
        self.viewport_frozen = saved_frozen;
        clip_recent_geometry(&mut self.scene, stroke_from, fill_from, glyph_from, &clip);
        if let Some(rect) = border {
            let mut points: Vec<Vector3> = rect
                .into_iter()
                .map(|point| Vector3::new(point.x, point.y, center_z))
                .collect();
            if let Some(first) = points.first().copied() {
                points.push(first);
            }
            let points = world(transform, points);
            self.emit_dedicated(&points, entity, frame, source);
        }
    }

    fn boundary_polygon(&self, handle: Handle) -> Option<Vec<Vector2>> {
        if handle.is_null() {
            return None;
        }
        let entity = self.doc.get_entity(handle)?;
        let points = match entity {
            EntityType::LwPolyline(poly) => lwpolyline_points(poly),
            EntityType::Polyline2D(poly) => polyline2d_points(poly),
            EntityType::Circle(circle) => super::curves::circle_points(circle),
            EntityType::Ellipse(ellipse) => super::curves::ellipse_points(ellipse),
            EntityType::Polyline(poly) => {
                poly.vertices.iter().map(|vertex| vertex.location).collect()
            }
            _ => return None,
        };
        if points.len() < 3 {
            return None;
        }
        Some(
            points
                .into_iter()
                .map(|point| Vector2::new(point.x, point.y))
                .collect(),
        )
    }

    fn draw_view_border(
        &mut self,
        border: &crate::entities::ViewBorder,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        let width = border.max[0] - border.min[0];
        let height = border.max[1] - border.min[1];
        if width.abs() <= 1e-9 && height.abs() <= 1e-9 {
            self.fallback(entity, frame, source);
            return;
        }
        let points = world(
            transform,
            vec![
                Vector3::new(border.min[0], border.min[1], 0.0),
                Vector3::new(border.max[0], border.min[1], 0.0),
                Vector3::new(border.max[0], border.max[1], 0.0),
                Vector3::new(border.min[0], border.max[1], 0.0),
                Vector3::new(border.min[0], border.min[1], 0.0),
            ],
        );
        self.emit_dedicated(&points, entity, frame, source);
    }

    fn draw_section_symbol(
        &mut self,
        symbol: &crate::entities::SectionSymbol,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        let vertices = section_vertices(symbol);
        if vertices.len() < 2 {
            self.fallback(entity, frame, source);
            return;
        }
        let path = section_path(&vertices);
        if path.len() < 2 {
            self.fallback(entity, frame, source);
            return;
        }
        let style = self.doc.section_view_style.clone();
        let symbol_scale = if symbol.symbol_scale.abs() > 1e-9 {
            symbol.symbol_scale.abs()
        } else {
            1.0
        };
        let show_plane = style
            .as_ref()
            .map(|item| item.show_plane_line)
            .unwrap_or(true);
        let show_ends = style
            .as_ref()
            .map(|item| item.show_end_lines)
            .unwrap_or(true);
        let mut drew = false;
        if show_plane {
            let points = world(transform, path.clone());
            self.emit_dedicated(&points, entity, frame, source.clone());
            drew = true;
        } else if show_ends {
            let length = style
                .as_ref()
                .map(|item| (item.end_line_length + item.end_line_overshoot).abs())
                .unwrap_or(0.0)
                * symbol_scale;
            for stub in [
                take_length(&path, length, false),
                take_length(&path, length, true),
            ] {
                if stub.len() >= 2 {
                    let points = world(transform, stub);
                    self.emit_dedicated(&points, entity, frame, source.clone());
                    drew = true;
                }
            }
        }
        if style.as_ref().is_some_and(|item| item.show_arrows) {
            let size = style
                .as_ref()
                .map(|item| item.arrow_size.abs())
                .unwrap_or(0.0)
                .max(1e-6)
                * symbol_scale;
            if let Some(direction) = path_side(&path, false) {
                let triangle = arrow_triangle(path[0], direction, size);
                self.emit_fill(
                    &world(transform, triangle),
                    entity,
                    frame,
                    source.clone(),
                    false,
                );
                drew = true;
            }
            if let Some(direction) = path_side(&path, true) {
                let origin = *path.last().unwrap();
                let triangle = arrow_triangle(origin, direction, size);
                self.emit_fill(
                    &world(transform, triangle),
                    entity,
                    frame,
                    source.clone(),
                    false,
                );
                drew = true;
            }
        }
        let label_height = style
            .as_ref()
            .map(|item| item.label_height)
            .filter(|height| *height > 1e-9)
            .unwrap_or(2.5)
            * symbol_scale;
        for vertex in &vertices {
            if vertex.label.is_empty() {
                continue;
            }
            let anchor = vertex.point + vertex.label_offset;
            self.draw_label(
                &super::text_draw::GlyphLabel {
                    text: &vertex.label,
                    shape_number: None,
                    insertion: anchor,
                    alignment: anchor,
                    use_alignment: false,
                    height: label_height,
                    rotation: 0.0,
                    width_factor: 1.0,
                    oblique: 0.0,
                    horizontal: 0,
                    vertical: 0,
                    normal: Vector3::UNIT_Z,
                    backward: false,
                    upside_down: false,
                },
                "Standard",
                entity,
                frame,
                transform,
                source.clone(),
            );
            drew = true;
        }
        if !drew {
            self.fallback(entity, frame, source);
        }
    }

    fn draw_section_object(
        &mut self,
        data: &crate::entities::SectionObjectData,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        let mut chains = Vec::new();
        if let Some(chain) = closed_chain(&data.vertices) {
            chains.push(chain);
        }
        if let Some(chain) = closed_chain(&data.back_line_vertices) {
            chains.push(chain);
        }
        let count = data.vertices.len().min(data.back_line_vertices.len());
        for index in 0..count {
            chains.push(vec![data.vertices[index], data.back_line_vertices[index]]);
        }
        if chains.is_empty() {
            self.fallback(entity, frame, source);
            return;
        }
        for chain in chains {
            self.emit_dedicated(&world(transform, chain), entity, frame, source.clone());
        }
    }

    fn draw_insert(
        &mut self,
        insert: &Insert,
        frame: &Frame,
        transform: &Transform,
        path: &[crate::types::Handle],
    ) {
        let Some(record) = self.doc.block_records.get(&insert.block_name) else {
            self.scene.skipped.push(SkippedEntity {
                kind: "Insert".to_string(),
                reason: format!("block {} is missing", insert.block_name),
                source: EntityRef {
                    handle: insert.common.handle,
                    insert_path: path.to_vec(),
                },
            });
            return;
        };
        if record.flags.is_xref
            || record.flags.is_xref_overlay
            || record.flags.is_external
            || !record.xref_path.is_empty()
        {
            self.scene.skipped.push(SkippedEntity {
                kind: "Insert".to_string(),
                reason: "unresolved xref".to_string(),
                source: EntityRef {
                    handle: insert.common.handle,
                    insert_path: path.to_vec(),
                },
            });
            return;
        }
        let key = insert.block_name.to_ascii_lowercase();
        if self.stack.contains(&key) {
            self.scene.skipped.push(SkippedEntity {
                kind: "Insert".to_string(),
                reason: "block cycle".to_string(),
                source: EntityRef {
                    handle: insert.common.handle,
                    insert_path: path.to_vec(),
                },
            });
            return;
        }
        let cells = cell_transforms(insert);
        if cells.is_empty() {
            self.scene.skipped.push(SkippedEntity {
                kind: "Insert".to_string(),
                reason: "insert array is too large".to_string(),
                source: EntityRef {
                    handle: insert.common.handle,
                    insert_path: path.to_vec(),
                },
            });
            return;
        }
        let child = Frame::from_insert(frame, &insert.common);
        let mut child_path = path.to_vec();
        child_path.push(insert.common.handle);
        let base = Transform::from_translation(record.base_point * -1.0);
        self.stack.insert(key.clone());
        let stroke_from = self.scene.strokes.len();
        let fill_from = self.scene.fills.len();
        let glyph_from = self.scene.glyphs.len();
        let handles = self.ordered_handles(record);
        for cell in cells {
            let to_world = base.then(&cell).then(transform);
            for handle in &handles {
                let Some(entity) = self.doc.get_entity(*handle) else {
                    continue;
                };
                self.draw(entity, &child, &to_world, &child_path);
            }
        }
        for attribute in &insert.attributes {
            if attribute.flags.invisible {
                continue;
            }
            let attribute = EntityType::AttributeEntity(attribute.clone());
            self.draw(&attribute, frame, transform, path);
        }
        if let Some(polygon) = clip_polygon_world(self.doc, insert, transform) {
            clip_recent_geometry(
                &mut self.scene,
                stroke_from,
                fill_from,
                glyph_from,
                &polygon,
            );
        }
        self.stack.remove(&key);
    }

    fn draw_label(
        &mut self,
        label: &super::text_draw::GlyphLabel<'_>,
        style_name: &str,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        if self.doc.header.quick_text_mode {
            let points = world(transform, super::text_draw::quick_box(label));
            let visual = self.scene.visual_style.clone();
            let style = resolve_style(self.doc, entity.common(), frame, &visual, self.sheet);
            self.emit_plain(&points, &[], style, source);
            return;
        }
        let produced = match self.font_for(style_name) {
            None => None,
            Some(FaceFont::Shape(font)) => Some(PreparedLabel::Strokes(
                super::text_draw::place_label(font, label),
            )),
            Some(FaceFont::TrueType(path)) => Some(PreparedLabel::TrueType(path.clone())),
        };
        let Some(produced) = produced else {
            self.scene.skipped.push(SkippedEntity {
                kind: kind_of(entity).to_string(),
                reason: format!("font for style {style_name} was not found"),
                source,
            });
            return;
        };
        let visual = self.scene.visual_style.clone();
        let style = resolve_style(self.doc, entity.common(), frame, &visual, self.sheet);
        match produced {
            PreparedLabel::Strokes(chains) => {
                for chain in chains {
                    self.emit_plain(&world(transform, chain), &[], style.clone(), source.clone());
                }
            }
            PreparedLabel::TrueType(path) => {
                let anchor = world(
                    transform,
                    vec![if label.use_alignment {
                        label.alignment
                    } else {
                        label.insertion
                    }],
                );
                self.scene.glyphs.push(super::scene::GlyphRun {
                    text: label.text.to_string(),
                    anchor: anchor.first().copied().unwrap_or(label.insertion),
                    rotation: label.rotation,
                    height: label.height,
                    width_factor: label.width_factor,
                    horizontal: label.horizontal,
                    vertical: label.vertical,
                    normal: label.normal,
                    font_path: path,
                    rgb: style.rgb,
                    aci7: style.aci7,
                    opacity: style.opacity,
                    source,
                });
            }
        }
    }

    fn font_for(&mut self, style_name: &str) -> Option<&FaceFont> {
        if !self.fonts.contains_key(style_name) {
            let loaded = self.load_font(style_name);
            self.fonts.insert(style_name.to_string(), loaded);
        }
        self.fonts.get(style_name).and_then(|font| font.as_ref())
    }

    fn load_font(&self, style_name: &str) -> Option<FaceFont> {
        let style = self.doc.text_styles.get(style_name)?;
        let mut names = Vec::new();
        if !style.font_file.is_empty() {
            names.push(style.font_file.clone());
        }
        if !style.true_type_font.is_empty() {
            names.push(style.true_type_font.clone());
        }
        if !style.big_font_file.is_empty() {
            names.push(style.big_font_file.clone());
        }
        let prefer_ttf = style.font_file.to_ascii_lowercase().ends_with(".ttf");
        if !prefer_ttf {
            for name in &names {
                if let Some(path) = find_font_file(self.font_dirs, name, &["shp", "shx", ""]) {
                    if let Some(font) = super::shx::ShapeFont::load(&path) {
                        return Some(FaceFont::Shape(font));
                    }
                }
            }
        }
        for name in &names {
            if let Some(path) = find_font_file(self.font_dirs, name, &["ttf", ""]) {
                if path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("ttf"))
                    || name.to_ascii_lowercase().ends_with(".ttf")
                {
                    return Some(FaceFont::TrueType(path));
                }
            }
        }
        None
    }

    fn draw_exploded(
        &mut self,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        path: &[crate::types::Handle],
        source: EntityRef,
    ) {
        let parts = entity.explode();
        if parts.is_empty() {
            self.fallback(entity, frame, source);
            return;
        }
        for part in &parts {
            self.draw(part, frame, transform, path);
        }
        let _ = source;
    }

    fn ordered_handles(&self, record: &crate::tables::BlockRecord) -> Vec<crate::types::Handle> {
        let handles = record.entity_handles.clone();
        if self.doc.header.sort_entities & (4 | 16) == 0 {
            return handles;
        }
        let Some(table) = sort_table(self.doc, record) else {
            return handles;
        };
        let mut ordered: Vec<(u64, usize, crate::types::Handle)> = handles
            .into_iter()
            .enumerate()
            .map(|(index, handle)| {
                let key = table.get_sort_handle(handle).unwrap_or(handle).value();
                (key, index, handle)
            })
            .collect();
        ordered.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
        ordered.into_iter().map(|(_, _, handle)| handle).collect()
    }

    fn emit_dedicated(
        &mut self,
        points: &[Vector3],
        entity: &EntityType,
        frame: &Frame,
        source: EntityRef,
    ) {
        let visual = self.scene.visual_style.clone();
        let style = resolve_style(self.doc, entity.common(), frame, &visual, self.sheet);
        let marks = pattern_elements(
            self.doc,
            entity.common(),
            frame,
            &visual,
            self.pattern_scale,
        );
        let lengths: Vec<f64> = marks.iter().map(|mark| mark.length).collect();
        self.emit_plain(points, &lengths, style, source.clone());
        self.place_complex(points, &marks, entity, frame, source);
    }

    fn emit_plain(
        &mut self,
        points: &[Vector3],
        lengths: &[f64],
        style: super::scene::Style,
        source: EntityRef,
    ) {
        emit_chain(
            &mut self.scene.strokes,
            points,
            lengths,
            style,
            source,
            DrawPath::Dedicated,
        );
    }

    fn place_complex(
        &mut self,
        points: &[Vector3],
        marks: &[super::style::PatternMark],
        entity: &EntityType,
        frame: &Frame,
        source: EntityRef,
    ) {
        if !marks
            .iter()
            .any(|mark| mark.text.is_some() || mark.shape_number.is_some())
        {
            return;
        }
        let length = super::dash::polyline_length(points);
        let period: f64 = marks.iter().map(|mark| mark.length.abs()).sum();
        if length <= 1e-9 || period <= 1e-12 {
            return;
        }
        let repeats = (length / period).round().max(1.0);
        let stretch = length / (repeats * period);
        let mut cursor = 0.0;
        for _ in 0..repeats as usize {
            for mark in marks {
                let span = mark.length.abs() * stretch;
                if mark.text.is_some() || mark.shape_number.is_some() {
                    if let Some((position, tangent)) = point_along(points, cursor + span * 0.5) {
                        let mut angle = if mark.absolute {
                            mark.rotation
                        } else {
                            tangent.y.atan2(tangent.x) + mark.rotation
                        };
                        if mark.upright {
                            let turned = angle.sin().abs();
                            if angle.cos() < 0.0 && turned < 0.999 {
                                angle += std::f64::consts::PI;
                            }
                        }
                        let side = Vector3::new(-tangent.y, tangent.x, 0.0);
                        let anchor =
                            position + tangent * mark.offset_along + side * mark.offset_perp;
                        let text = mark.text.clone().unwrap_or_default();
                        self.draw_label(
                            &super::text_draw::GlyphLabel {
                                text: &text,
                                shape_number: mark.shape_number,
                                insertion: anchor,
                                alignment: anchor,
                                use_alignment: false,
                                height: mark.scale.abs().max(1e-6),
                                rotation: angle,
                                width_factor: 1.0,
                                oblique: 0.0,
                                horizontal: 1,
                                vertical: 2,
                                normal: Vector3::UNIT_Z,
                                backward: false,
                                upside_down: false,
                            },
                            &mark.style_name,
                            entity,
                            frame,
                            &Transform::identity(),
                            source.clone(),
                        );
                    }
                }
                cursor += span;
            }
        }
    }

    fn emit_dot(
        &mut self,
        points: &[Vector3],
        entity: &EntityType,
        frame: &Frame,
        source: EntityRef,
    ) {
        let visual = self.scene.visual_style.clone();
        let style = resolve_style(self.doc, entity.common(), frame, &visual, self.sheet);
        self.scene.strokes.push(Stroke {
            points: points.to_vec(),
            closed: false,
            dot: true,
            style,
            source,
            path: DrawPath::Dedicated,
        });
    }

    fn emit_fill(
        &mut self,
        triangles: &[Vector3],
        entity: &EntityType,
        frame: &Frame,
        source: EntityRef,
        background: bool,
    ) {
        self.emit_fill_colors(triangles, Vec::new(), entity, frame, source, background);
    }

    fn emit_fill_colors(
        &mut self,
        triangles: &[Vector3],
        vertex_rgb: Vec<[u8; 3]>,
        entity: &EntityType,
        frame: &Frame,
        source: EntityRef,
        background: bool,
    ) {
        if triangles.len() < 3 {
            self.fallback(entity, frame, source);
            return;
        }
        let visual = self.scene.visual_style.clone();
        let style = resolve_style(self.doc, entity.common(), frame, &visual, self.sheet);
        self.scene.fills.push(super::scene::Fill {
            triangles: triangles.to_vec(),
            style,
            source,
            path: DrawPath::Dedicated,
            background,
            vertex_rgb,
        });
    }

    fn fallback(&mut self, entity: &EntityType, frame: &Frame, source: EntityRef) {
        let kind = kind_of(entity).to_string();
        let bounds = entity.as_entity().bounding_box();
        let corners = box_corners(bounds.min, bounds.max);
        let style = resolve_style(
            self.doc,
            entity.common(),
            frame,
            &self.scene.visual_style.clone(),
            self.sheet,
        );
        for edge in box_edges(&corners) {
            self.scene.strokes.push(Stroke {
                points: edge.to_vec(),
                closed: false,
                dot: false,
                style: style.clone(),
                source: source.clone(),
                path: DrawPath::Fallback,
            });
        }
        self.scene.fallbacks.push(FallbackEntry { kind, source });
    }
}

fn emit_chain(
    strokes: &mut Vec<Stroke>,
    points: &[Vector3],
    elements: &[f64],
    style: super::scene::Style,
    source: EntityRef,
    path: DrawPath,
) {
    if points.len() < 2 || polyline_length(points) <= 1e-12 {
        return;
    }
    let patterned = elements.iter().any(|element| element.abs() > 1e-12);
    if !patterned {
        strokes.push(Stroke {
            points: points.to_vec(),
            closed: false,
            dot: false,
            style,
            source,
            path,
        });
        return;
    }
    let runs = fit_runs(polyline_length(points), elements);
    for piece in runs_on_segments(points, &runs) {
        let (points, dot) = match piece {
            SegmentPiece::Dash { start, end } => (vec![start, end], false),
            SegmentPiece::Dot(point) => (vec![point, point], true),
        };
        strokes.push(Stroke {
            points,
            closed: false,
            dot,
            style: style.clone(),
            source: source.clone(),
            path,
        });
    }
}

fn sort_table<'a>(
    doc: &'a CadDocument,
    record: &crate::tables::BlockRecord,
) -> Option<&'a crate::objects::SortEntitiesTable> {
    if let Some(entity) = doc.get_entity(record.block_entity_handle) {
        if let Some(dictionary) = entity.common().xdictionary_handle {
            if let Some(crate::objects::ObjectType::Dictionary(dictionary)) =
                doc.objects.get(&dictionary)
            {
                if let Some((_, handle)) = dictionary.entries.iter().find(|(name, _)| {
                    name.eq_ignore_ascii_case(crate::objects::SortEntitiesTable::DICTIONARY_KEY)
                }) {
                    if let Some(crate::objects::ObjectType::SortEntitiesTable(table)) =
                        doc.objects.get(handle)
                    {
                        return Some(table);
                    }
                }
            }
        }
    }
    doc.objects.values().find_map(|object| match object {
        crate::objects::ObjectType::SortEntitiesTable(table)
            if table.block_owner_handle == record.handle =>
        {
            Some(table)
        }
        _ => None,
    })
}

fn world(transform: &Transform, points: Vec<Vector3>) -> Vec<Vector3> {
    points
        .into_iter()
        .map(|point| transform.apply(point))
        .collect()
}

fn find_font_file(
    dirs: &[std::path::PathBuf],
    name: &str,
    extensions: &[&str],
) -> Option<std::path::PathBuf> {
    for dir in dirs {
        for extension in extensions {
            let path = if extension.is_empty() {
                dir.join(name)
            } else {
                dir.join(format!("{name}.{extension}"))
            };
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

fn point_along(points: &[Vector3], distance: f64) -> Option<(Vector3, Vector3)> {
    let mut remaining = distance.max(0.0);
    for pair in points.windows(2) {
        let delta = pair[1] - pair[0];
        let length = delta.length();
        if length <= 1e-12 {
            continue;
        }
        if remaining <= length {
            let t = remaining / length;
            let tangent = delta * (1.0 / length);
            return Some((pair[0] + delta * t, tangent));
        }
        remaining -= length;
    }
    let last = points.last()?;
    let prev = points.get(points.len().saturating_sub(2))?;
    let delta = *last - *prev;
    let length = delta.length();
    if length <= 1e-12 {
        return None;
    }
    Some((*last, delta * (1.0 / length)))
}

fn clip_polygon_world(
    doc: &CadDocument,
    insert: &Insert,
    parent: &Transform,
) -> Option<Vec<crate::types::Vector2>> {
    let filter = spatial_filter(doc, insert)?;
    if filter.boundary_points.len() < 2 {
        return None;
    }
    let local = if filter.boundary_points.len() == 2 {
        super::clip::rectangle(filter.boundary_points[0], filter.boundary_points[1])
    } else {
        filter.boundary_points.clone()
    };
    let polygon = local
        .into_iter()
        .map(|point| {
            let placed = filter
                .clip_bound_transform
                .transform_point(Vector3::new(point.x, point.y, 0.0));
            let world = parent.apply(placed);
            crate::types::Vector2::new(world.x, world.y)
        })
        .collect();
    Some(polygon)
}

fn spatial_filter<'a>(
    doc: &'a CadDocument,
    insert: &Insert,
) -> Option<&'a crate::objects::SpatialFilter> {
    let dictionary = insert.common.xdictionary_handle?;
    let crate::objects::ObjectType::Dictionary(dictionary) = doc.objects.get(&dictionary)? else {
        return None;
    };
    let filter_dictionary = dictionary
        .entries
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("ACAD_FILTER"))
        .map(|(_, handle)| *handle)?;
    let crate::objects::ObjectType::Dictionary(filter_dictionary) =
        doc.objects.get(&filter_dictionary)?
    else {
        return None;
    };
    let spatial = filter_dictionary
        .entries
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("SPATIAL"))
        .map(|(_, handle)| *handle)?;
    match doc.objects.get(&spatial)? {
        crate::objects::ObjectType::SpatialFilter(filter) => Some(filter),
        _ => None,
    }
}

fn clip_recent_geometry(
    scene: &mut Scene,
    stroke_from: usize,
    fill_from: usize,
    glyph_from: usize,
    polygon: &[crate::types::Vector2],
) {
    let fresh = scene
        .strokes
        .split_off(stroke_from.min(scene.strokes.len()));
    for stroke in fresh {
        for pair in stroke.points.windows(2) {
            if let Some((start, end)) = super::clip::clip_segment(pair[0], pair[1], polygon) {
                scene.strokes.push(Stroke {
                    points: vec![start, end],
                    closed: false,
                    dot: stroke.dot,
                    style: stroke.style.clone(),
                    source: stroke.source.clone(),
                    path: stroke.path,
                });
            }
        }
    }
    for fill in &mut scene.fills[fill_from..] {
        let mut triangles = Vec::new();
        for corner in fill.triangles.chunks_exact(3) {
            let clipped = super::clip::clip_polygon(corner, polygon);
            for index in 1..clipped.len().saturating_sub(1) {
                triangles.extend([clipped[0], clipped[index], clipped[index + 1]]);
            }
        }
        fill.triangles = triangles;
        if !fill.vertex_rgb.is_empty() {
            fill.vertex_rgb.clear();
        }
    }
    let glyphs = scene.glyphs.split_off(glyph_from.min(scene.glyphs.len()));
    scene.glyphs.extend(glyphs.into_iter().filter(|glyph| {
        super::clip::clip_segment(
            glyph.anchor,
            glyph.anchor + Vector3::new(1e-3, 0.0, 0.0),
            polygon,
        )
        .is_some()
    }));
}

fn cell_transforms(insert: &Insert) -> Vec<Transform> {
    let ocs = Matrix4::from_matrix3(Matrix3::arbitrary_axis(insert.normal));
    let rotation = Matrix4::rotation_z(insert.rotation);
    let scale = Matrix4::scaling(insert.x_scale(), insert.y_scale(), insert.z_scale());
    insert
        .array_points()
        .into_iter()
        .map(|point| {
            let translation = Matrix4::translation(point.x, point.y, point.z);
            Transform::from_matrix(ocs * translation * rotation * scale)
        })
        .collect()
}

fn has_width(poly: &LwPolyline) -> bool {
    poly.constant_width.abs() > 1e-9
        || poly
            .vertices
            .iter()
            .any(|vertex| vertex.start_width.abs() > 1e-9 || vertex.end_width.abs() > 1e-9)
}

fn lwpolyline_points(poly: &LwPolyline) -> Vec<Vector3> {
    let verts = &poly.vertices;
    if verts.is_empty() {
        return Vec::new();
    }
    let ocs = Matrix3::arbitrary_axis(poly.normal);
    let segments = if poly.is_closed && verts.len() >= 2 {
        verts.len()
    } else {
        verts.len().saturating_sub(1)
    };
    let mut raw = Vec::new();
    for index in 0..segments {
        let from = &verts[index];
        let to = &verts[(index + 1) % verts.len()];
        let start = Vector3::new(from.location.x, from.location.y, poly.elevation);
        let end = Vector3::new(to.location.x, to.location.y, poly.elevation);
        let mut segment = tessellate_bulge(start, end, from.bulge);
        if index > 0 {
            segment.drain(..1.min(segment.len()));
        }
        raw.extend(segment);
    }
    if raw.is_empty() && verts.len() == 1 {
        raw.push(Vector3::new(
            verts[0].location.x,
            verts[0].location.y,
            poly.elevation,
        ));
    }
    raw.into_iter().map(|point| ocs * point).collect()
}

fn segment_widths(start_width: f64, end_width: f64, constant: f64) -> (f64, f64) {
    let start = if start_width.abs() > 1e-9 {
        start_width
    } else {
        constant
    };
    let end = if end_width.abs() > 1e-9 {
        end_width
    } else if start_width.abs() > 1e-9 {
        start_width
    } else {
        constant
    };
    (start.abs(), end.abs())
}

fn wide_triangles(
    samples: &[Vector3],
    start_width: f64,
    end_width: f64,
    normal: Vector3,
) -> Vec<Vector3> {
    if samples.len() < 2 || (start_width <= 1e-9 && end_width <= 1e-9) {
        return Vec::new();
    }
    let mut triangles = Vec::new();
    let steps = samples.len() - 1;
    for index in 0..steps {
        let t0 = index as f64 / steps as f64;
        let t1 = (index + 1) as f64 / steps as f64;
        let width0 = start_width + (end_width - start_width) * t0;
        let width1 = start_width + (end_width - start_width) * t1;
        let a = samples[index];
        let b = samples[index + 1];
        let direction = b - a;
        let mut side = normal.cross(&direction);
        if side.length_squared() <= 1e-18 {
            side = Vector3::new(-direction.y, direction.x, 0.0);
        }
        if side.length_squared() <= 1e-18 {
            continue;
        }
        side = side.normalize();
        let a_left = a + side * (width0 * 0.5);
        let a_right = a - side * (width0 * 0.5);
        let b_left = b + side * (width1 * 0.5);
        let b_right = b - side * (width1 * 0.5);
        triangles.extend([a_left, b_left, b_right, a_left, b_right, a_right]);
    }
    triangles
}

fn lwpolyline_wide_triangles(poly: &LwPolyline) -> Vec<Vector3> {
    let verts = &poly.vertices;
    if verts.len() < 2 {
        return Vec::new();
    }
    let ocs = Matrix3::arbitrary_axis(poly.normal);
    let segments = if poly.is_closed {
        verts.len()
    } else {
        verts.len() - 1
    };
    let mut triangles = Vec::new();
    for index in 0..segments {
        let from = &verts[index];
        let to = &verts[(index + 1) % verts.len()];
        let (start_width, end_width) =
            segment_widths(from.start_width, from.end_width, poly.constant_width);
        let start = Vector3::new(from.location.x, from.location.y, poly.elevation);
        let end = Vector3::new(to.location.x, to.location.y, poly.elevation);
        let samples = tessellate_bulge(start, end, from.bulge);
        let local = wide_triangles(&samples, start_width, end_width, Vector3::UNIT_Z);
        triangles.extend(local.into_iter().map(|point| ocs * point));
    }
    triangles
}

fn polyline2d_has_width(poly: &crate::entities::Polyline2D) -> bool {
    poly.start_width.abs() > 1e-9
        || poly.end_width.abs() > 1e-9
        || poly
            .vertices
            .iter()
            .any(|vertex| vertex.start_width.abs() > 1e-9 || vertex.end_width.abs() > 1e-9)
}

fn polyline2d_wide_triangles(poly: &crate::entities::Polyline2D) -> Vec<Vector3> {
    let verts = &poly.vertices;
    if verts.len() < 2 {
        return Vec::new();
    }
    let ocs = Matrix3::arbitrary_axis(poly.normal);
    let segments = if poly.is_closed() {
        verts.len()
    } else {
        verts.len() - 1
    };
    let mut triangles = Vec::new();
    for index in 0..segments {
        let from = &verts[index];
        let to = &verts[(index + 1) % verts.len()];
        let constant = if index == 0 {
            poly.start_width
        } else {
            poly.end_width
        };
        let (start_width, end_width) = segment_widths(from.start_width, from.end_width, constant);
        let start = Vector3::new(from.location.x, from.location.y, poly.elevation);
        let end = Vector3::new(to.location.x, to.location.y, poly.elevation);
        let samples = tessellate_bulge(start, end, from.bulge);
        let local = wide_triangles(&samples, start_width, end_width, Vector3::UNIT_Z);
        triangles.extend(local.into_iter().map(|point| ocs * point));
    }
    triangles
}

fn polyline2d_points(poly: &crate::entities::Polyline2D) -> Vec<Vector3> {
    let verts = &poly.vertices;
    if verts.is_empty() {
        return Vec::new();
    }
    let ocs = Matrix3::arbitrary_axis(poly.normal);
    let segments = if poly.is_closed() && verts.len() >= 2 {
        verts.len()
    } else {
        verts.len().saturating_sub(1)
    };
    let mut raw = Vec::new();
    for index in 0..segments {
        let from = &verts[index];
        let to = &verts[(index + 1) % verts.len()];
        let start = Vector3::new(from.location.x, from.location.y, poly.elevation);
        let end = Vector3::new(to.location.x, to.location.y, poly.elevation);
        let mut segment = tessellate_bulge(start, end, from.bulge);
        if index > 0 {
            segment.drain(..1.min(segment.len()));
        }
        raw.extend(segment);
    }
    raw.into_iter().map(|point| ocs * point).collect()
}

pub(crate) fn tessellate_bulge(start: Vector3, end: Vector3, bulge: f64) -> Vec<Vector3> {
    if bulge.abs() < 1e-10 {
        return vec![start, end];
    }
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let chord = (dx * dx + dy * dy).sqrt();
    if chord < 1e-10 {
        return vec![start];
    }
    let bulge_sq = bulge * bulge;
    let radius = chord * (1.0 + bulge_sq) / (4.0 * bulge.abs());
    let offset = chord * (1.0 - bulge_sq) / (4.0 * bulge);
    let mid_x = (start.x + end.x) / 2.0;
    let mid_y = (start.y + end.y) / 2.0;
    let center_x = mid_x + (-dy / chord) * offset;
    let center_y = mid_y + (dx / chord) * offset;
    let start_angle = (start.y - center_y).atan2(start.x - center_x);
    let end_angle = (end.y - center_y).atan2(end.x - center_x);
    let mut sweep = end_angle - start_angle;
    if bulge > 0.0 && sweep < 0.0 {
        sweep += std::f64::consts::TAU;
    } else if bulge < 0.0 && sweep > 0.0 {
        sweep -= std::f64::consts::TAU;
    }
    let steps = ((sweep.abs() / 10.0_f64.to_radians()).ceil() as usize).clamp(2, 128);
    (0..=steps)
        .map(|step| {
            let angle = start_angle + sweep * step as f64 / steps as f64;
            Vector3::new(
                center_x + radius * angle.cos(),
                center_y + radius * angle.sin(),
                start.z,
            )
        })
        .collect()
}

fn effective_layer_of(common: &crate::entities::EntityCommon, frame: &Frame) -> String {
    if frame.in_insert && common.layer.eq_ignore_ascii_case("0") {
        frame.layer.clone()
    } else if common.layer.is_empty() {
        "0".to_string()
    } else {
        common.layer.clone()
    }
}

fn box_corners(min: Vector3, max: Vector3) -> [Vector3; 8] {
    [
        Vector3::new(min.x, min.y, min.z),
        Vector3::new(max.x, min.y, min.z),
        Vector3::new(max.x, max.y, min.z),
        Vector3::new(min.x, max.y, min.z),
        Vector3::new(min.x, min.y, max.z),
        Vector3::new(max.x, min.y, max.z),
        Vector3::new(max.x, max.y, max.z),
        Vector3::new(min.x, max.y, max.z),
    ]
}

fn box_edges(corners: &[Vector3; 8]) -> [[Vector3; 2]; 12] {
    let edge = |a: usize, b: usize| [corners[a], corners[b]];
    [
        edge(0, 1),
        edge(1, 2),
        edge(2, 3),
        edge(3, 0),
        edge(4, 5),
        edge(5, 6),
        edge(6, 7),
        edge(7, 4),
        edge(0, 4),
        edge(1, 5),
        edge(2, 6),
        edge(3, 7),
    ]
}

struct SymbolVertex {
    point: Vector3,
    bulge: f64,
    label: String,
    label_offset: Vector3,
}

fn section_vertices(symbol: &crate::entities::SectionSymbol) -> Vec<SymbolVertex> {
    if !symbol.points.is_empty() {
        return symbol
            .points
            .iter()
            .map(|point| SymbolVertex {
                point: point.point,
                bulge: point.bulge,
                label: point.label.clone(),
                label_offset: point.label_offset,
            })
            .collect();
    }
    if symbol.end_a == [0.0; 2] && symbol.end_b == [0.0; 2] {
        return Vec::new();
    }
    vec![
        SymbolVertex {
            point: Vector3::new(symbol.end_a[0], symbol.end_a[1], 0.0),
            bulge: 0.0,
            label: symbol.label.clone(),
            label_offset: Vector3::ZERO,
        },
        SymbolVertex {
            point: Vector3::new(symbol.end_b[0], symbol.end_b[1], 0.0),
            bulge: 0.0,
            label: String::new(),
            label_offset: Vector3::ZERO,
        },
    ]
}

fn section_path(vertices: &[SymbolVertex]) -> Vec<Vector3> {
    let mut path = Vec::new();
    for index in 0..vertices.len().saturating_sub(1) {
        let mut segment = tessellate_bulge(
            vertices[index].point,
            vertices[index + 1].point,
            vertices[index].bulge,
        );
        if index > 0 {
            segment.drain(..1.min(segment.len()));
        }
        path.extend(segment);
    }
    path
}

fn take_length(points: &[Vector3], length: f64, from_end: bool) -> Vec<Vector3> {
    if points.len() < 2 || length <= 1e-9 {
        return Vec::new();
    }
    let owned: Vec<Vector3> = if from_end {
        points.iter().rev().copied().collect()
    } else {
        points.to_vec()
    };
    let mut taken = vec![owned[0]];
    let mut remaining = length;
    for pair in owned.windows(2) {
        let delta = pair[1] - pair[0];
        let span = delta.length();
        if span <= 1e-12 {
            continue;
        }
        if remaining <= span {
            taken.push(pair[0] + delta * (remaining / span));
            break;
        }
        taken.push(pair[1]);
        remaining -= span;
    }
    if from_end {
        taken.reverse();
    }
    taken
}

fn path_side(path: &[Vector3], at_end: bool) -> Option<Vector3> {
    let (start, end) = if at_end {
        let end = *path.last()?;
        let start = *path.get(path.len().saturating_sub(2))?;
        (start, end)
    } else {
        (*path.first()?, *path.get(1)?)
    };
    let delta = end - start;
    let length = delta.length();
    if length <= 1e-12 {
        return None;
    }
    let tangent = delta * (1.0 / length);
    Some(Vector3::new(-tangent.y, tangent.x, 0.0))
}

fn arrow_triangle(origin: Vector3, direction: Vector3, size: f64) -> Vec<Vector3> {
    let tangent = Vector3::new(direction.y, -direction.x, 0.0);
    let half = size * 0.3;
    vec![
        origin + direction * size,
        origin + tangent * half,
        origin - tangent * half,
    ]
}

fn closed_chain(points: &[Vector3]) -> Option<Vec<Vector3>> {
    if points.len() < 2 {
        return None;
    }
    let mut chain = points.to_vec();
    if points.len() >= 3 {
        let first = points[0];
        let last = *points.last().unwrap();
        if (first - last).length_squared() > 1e-12 {
            chain.push(first);
        }
    }
    Some(chain)
}

fn kind_of(entity: &EntityType) -> &'static str {
    match entity {
        EntityType::Point(_) => "Point",
        EntityType::Line(_) => "Line",
        EntityType::Circle(_) => "Circle",
        EntityType::Arc(_) => "Arc",
        EntityType::Ellipse(_) => "Ellipse",
        EntityType::Polyline(_) => "Polyline",
        EntityType::Polyline2D(_) => "Polyline2D",
        EntityType::Polyline3D(_) => "Polyline3D",
        EntityType::LwPolyline(_) => "LwPolyline",
        EntityType::Text(_) => "Text",
        EntityType::MText(_) => "MText",
        EntityType::Spline(_) => "Spline",
        EntityType::Helix(_) => "Helix",
        EntityType::Dimension(_) => "Dimension",
        EntityType::Hatch(_) => "Hatch",
        EntityType::Solid(_) => "Solid",
        EntityType::Face3D(_) => "Face3D",
        EntityType::Insert(_) => "Insert",
        EntityType::Block(_) => "Block",
        EntityType::BlockEnd(_) => "BlockEnd",
        EntityType::Ray(_) => "Ray",
        EntityType::XLine(_) => "XLine",
        EntityType::Viewport(_) => "Viewport",
        EntityType::AttributeDefinition(_) => "AttributeDefinition",
        EntityType::AttributeEntity(_) => "AttributeEntity",
        EntityType::Leader(_) => "Leader",
        EntityType::MultiLeader(_) => "MultiLeader",
        EntityType::MLine(_) => "MLine",
        EntityType::Mesh(_) => "Mesh",
        EntityType::RasterImage(_) => "RasterImage",
        EntityType::Solid3D(_) => "Solid3D",
        EntityType::Region(_) => "Region",
        EntityType::Body(_) => "Body",
        EntityType::Surface(_) => "Surface",
        EntityType::Table(_) => "Table",
        EntityType::Tolerance(_) => "Tolerance",
        EntityType::PolyfaceMesh(_) => "PolyfaceMesh",
        EntityType::Wipeout(_) => "Wipeout",
        EntityType::Shape(_) => "Shape",
        EntityType::Underlay(_) => "Underlay",
        EntityType::Seqend(_) => "Seqend",
        EntityType::Ole2Frame(_) => "Ole2Frame",
        EntityType::PolygonMesh(_) => "PolygonMesh",
        EntityType::Light(_) => "Light",
        EntityType::SectionSymbol(_) => "SectionSymbol",
        EntityType::ViewBorder(_) => "ViewBorder",
        EntityType::Extended(extended) => match &extended.data {
            crate::entities::ExtendedEntityData::Camera { .. } => "Camera",
            crate::entities::ExtendedEntityData::SectionObject(_) => "SectionObject",
            crate::entities::ExtendedEntityData::ArcAlignedText(_) => "ArcAlignedText",
            crate::entities::ExtendedEntityData::RemoteText(_) => "RemoteText",
            crate::entities::ExtendedEntityData::GeoPositionMarker(_) => "GeoPositionMarker",
            crate::entities::ExtendedEntityData::CoordinationModel(_) => "CoordinationModel",
            crate::entities::ExtendedEntityData::PointCloud(_) => "PointCloud",
            crate::entities::ExtendedEntityData::PointCloudEx(_) => "PointCloudEx",
            crate::entities::ExtendedEntityData::Proxy(_) => "Proxy",
            crate::entities::ExtendedEntityData::OleFrame(_) => "OleFrame",
            crate::entities::ExtendedEntityData::LayoutPrintConfig(_) => "LayoutPrintConfig",
            crate::entities::ExtendedEntityData::Format(_) => "Format",
            crate::entities::ExtendedEntityData::Legacy(_) => "Legacy",
            crate::entities::ExtendedEntityData::DynamicBlock(_) => "DynamicBlock",
            crate::entities::ExtendedEntityData::RegisteredClass(_) => "RegisteredClass",
            crate::entities::ExtendedEntityData::Arch(_) => "Arch",
        },
        EntityType::Unknown(_) => "Unknown",
    }
}

/// Kinds whose only picture today is the extents box.
pub const FALLBACK_KINDS: &[&str] = &[
    "Ray",
    "XLine",
    "Mesh",
    "RasterImage",
    "Solid3D",
    "Region",
    "Body",
    "Surface",
    "PolyfaceMesh",
    "Underlay",
    "Ole2Frame",
    "PolygonMesh",
    "Light",
    "Camera",
    "ArcAlignedText",
    "RemoteText",
    "GeoPositionMarker",
    "CoordinationModel",
    "PointCloud",
    "PointCloudEx",
    "Proxy",
    "OleFrame",
    "Legacy",
    "RegisteredClass",
    "Arch",
    "Unknown",
];
