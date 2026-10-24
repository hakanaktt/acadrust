//! Walk a drawing into a [`Scene`](super::Scene).

use std::collections::{HashMap, HashSet};

use crate::document::CadDocument;
use crate::entities::{EntityType, Insert, LwPolyline};
use crate::types::{Matrix3, Matrix4, Transform, Vector3};

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
    let mut compiler = Compiler {
        doc,
        scene: Scene {
            strokes: Vec::new(),
            fills: Vec::new(),
            fallbacks: Vec::new(),
            skipped: Vec::new(),
            visual_style: visual,
            model_lineweight_px_per_mm: options.model_lineweight_px_per_mm,
        },
        stack: HashSet::new(),
        font_dirs: &options.font_dirs,
        fonts: HashMap::new(),
    };
    compiler.walk_block("*Model_Space", &Frame::model(), &Transform::identity(), &[]);
    compiler.scene
}

struct Compiler<'a> {
    doc: &'a CadDocument,
    scene: Scene,
    stack: HashSet<String>,
    font_dirs: &'a [std::path::PathBuf],
    fonts: HashMap<String, Option<super::shx::ShapeFont>>,
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
                _ => self.fallback(entity, frame, source),
            },
            EntityType::Ray(_)
            | EntityType::XLine(_)
            | EntityType::Viewport(_)
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
            | EntityType::SectionSymbol(_)
            | EntityType::ViewBorder(_)
            | EntityType::Unknown(_) => self.fallback(entity, frame, source),
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
            self.emit_dedicated(&points, entity, frame, source);
            return;
        }
        let Some(font) = self.font_for(style_name) else {
            self.scene.skipped.push(SkippedEntity {
                kind: kind_of(entity).to_string(),
                reason: format!("font for style {style_name} was not found"),
                source,
            });
            return;
        };
        for chain in super::text_draw::place_label(font, label) {
            self.emit_dedicated(&world(transform, chain), entity, frame, source.clone());
        }
    }

    fn font_for(&mut self, style_name: &str) -> Option<&super::shx::ShapeFont> {
        if !self.fonts.contains_key(style_name) {
            let loaded = self.load_font(style_name);
            self.fonts.insert(style_name.to_string(), loaded);
        }
        self.fonts.get(style_name).and_then(|font| font.as_ref())
    }

    fn load_font(&self, style_name: &str) -> Option<super::shx::ShapeFont> {
        let style = self.doc.text_styles.get(style_name)?;
        let mut names = Vec::new();
        if !style.font_file.is_empty() {
            names.push(style.font_file.clone());
        }
        if !style.big_font_file.is_empty() {
            names.push(style.big_font_file.clone());
        }
        for name in names {
            for dir in self.font_dirs {
                for candidate in [
                    dir.join(&name),
                    dir.join(format!("{name}.shp")),
                    dir.join(format!("{name}.shx")),
                ] {
                    if let Some(font) = super::shx::ShapeFont::load(&candidate) {
                        return Some(font);
                    }
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
        let style = resolve_style(self.doc, entity.common(), frame, &visual);
        let elements = pattern_elements(self.doc, entity.common(), frame, &visual);
        emit_chain(
            &mut self.scene.strokes,
            points,
            &elements,
            style,
            source,
            DrawPath::Dedicated,
        );
    }

    fn emit_dot(
        &mut self,
        points: &[Vector3],
        entity: &EntityType,
        frame: &Frame,
        source: EntityRef,
    ) {
        let visual = self.scene.visual_style.clone();
        let style = resolve_style(self.doc, entity.common(), frame, &visual);
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
        let style = resolve_style(self.doc, entity.common(), frame, &visual);
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
    "Viewport",
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
    "SectionSymbol",
    "ViewBorder",
    "Camera",
    "SectionObject",
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
