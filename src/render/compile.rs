//! Walk a drawing into a [`Scene`](super::Scene).

use std::collections::{HashMap, HashSet};

use crate::document::CadDocument;
use crate::entities::{EntityType, Insert, LwPolyline, Text};
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
        resource_dirs: &options.resource_dirs,
        fonts: HashMap::new(),
        sheet,
        pattern_scale: 1.0,
        viewport_frozen: HashSet::new(),
        solid_style: options.solid_style,
        active_transform: Transform::identity(),
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
        pictures: Vec::new(),
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
    resource_dirs: &'a [std::path::PathBuf],
    fonts: HashMap<String, Option<FaceFont>>,
    /// Layout tab. Lineweights are paper millimetres, and viewports open model space.
    sheet: bool,
    /// Extra multiplier for linetype lengths. Viewport contents use the
    /// viewport scale when `PSLTSCALE` is off, because the picture is already
    /// in paper units.
    pattern_scale: f64,
    viewport_frozen: HashSet<Handle>,
    solid_style: super::SolidStyle,
    /// Transform of the entity currently being emitted. This lets helpers
    /// that receive already-world-space points still inherit insert scale.
    active_transform: Transform,
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
        let saved = self.active_transform.clone();
        self.active_transform = transform.clone();
        self.draw_inner(entity, frame, transform, path);
        self.active_transform = saved;
    }

    fn draw_inner(
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
                let visual = self.scene.visual_style.clone();
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
                if visual.edges != super::visual_style::EdgeMode::None {
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
                if visual.faces != super::visual_style::FaceShading::None {
                    let mut ring = corners
                        .into_iter()
                        .map(|corner| transform.apply(corner))
                        .collect::<Vec<_>>();
                    if ring[3] == ring[2] {
                        ring.pop();
                    }
                    self.emit_fill(&fan_triangles(&ring), entity, frame, source, false);
                }
            }
            EntityType::Wipeout(wipeout) => {
                if !wipeout
                    .flags
                    .contains(crate::entities::WipeoutDisplayFlags::SHOW_IMAGE)
                {
                    return;
                }
                let triangles = wipeout_triangles(wipeout);
                self.emit_fill(&world(transform, triangles), entity, frame, source, true);
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
                        // Standalone ATTDEFs are labels for the attribute tag.
                        // The default value is shown on an inserted ATTRIB,
                        // while AutoCAD displays the tag while editing the
                        // block definition itself.
                        text: &attribute.tag,
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
                crate::entities::ExtendedEntityData::Legacy(_) => {
                    self.scene.skipped.push(SkippedEntity {
                        kind: "Legacy".to_string(),
                        reason: "legacy command is not graphical".to_string(),
                        source,
                    });
                }
                crate::entities::ExtendedEntityData::SectionObject(data) => {
                    self.draw_section_object(data, entity, frame, transform, source);
                }
                crate::entities::ExtendedEntityData::Camera { view_handle } => {
                    self.draw_camera(*view_handle, entity, frame, transform, source);
                }
                crate::entities::ExtendedEntityData::ArcAlignedText(data) => {
                    self.draw_arc_text(data, entity, frame, transform, source);
                }
                crate::entities::ExtendedEntityData::RemoteText(data) => {
                    self.draw_remote_text(data, entity, frame, transform, source);
                }
                crate::entities::ExtendedEntityData::GeoPositionMarker(data) => {
                    self.draw_geo_marker(data, entity, frame, transform, source);
                }
                crate::entities::ExtendedEntityData::PointCloud(data) => {
                    self.draw_point_cloud_box(
                        data.extents_min,
                        data.extents_max,
                        &data.clippings,
                        entity,
                        frame,
                        transform,
                        source,
                    );
                }
                crate::entities::ExtendedEntityData::PointCloudEx(data) => {
                    self.draw_point_cloud_box(
                        data.extents_min,
                        data.extents_max,
                        &[],
                        entity,
                        frame,
                        transform,
                        source,
                    );
                }
                crate::entities::ExtendedEntityData::CoordinationModel(data) => {
                    self.draw_coordination(data, entity, frame, transform, source);
                }
                crate::entities::ExtendedEntityData::Proxy(_)
                | crate::entities::ExtendedEntityData::RegisteredClass(_)
                | crate::entities::ExtendedEntityData::OleFrame(_) => {
                    self.draw_cached_graphics(entity, frame, transform, source);
                }
                crate::entities::ExtendedEntityData::Arch(data) => {
                    self.draw_arch(data, entity, frame, transform, source);
                }
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
            EntityType::Solid3D(solid) => {
                self.draw_acis(&solid.acis_data, entity, frame, transform, source);
            }
            EntityType::Region(region) => {
                self.draw_acis(&region.acis_data, entity, frame, transform, source);
            }
            EntityType::Body(body) => {
                self.draw_acis(&body.acis_data, entity, frame, transform, source);
            }
            EntityType::Surface(surface) => {
                self.draw_acis(&surface.acis_data, entity, frame, transform, source);
            }
            EntityType::Mesh(mesh) => self.draw_mesh_entity(mesh, entity, frame, transform, source),
            EntityType::PolyfaceMesh(mesh) => {
                self.draw_polyface(mesh, entity, frame, transform, source);
            }
            EntityType::PolygonMesh(mesh) => {
                self.draw_polygon_mesh(mesh, entity, frame, transform, source);
            }
            EntityType::Light(light) => self.draw_light(light, entity, frame, transform, source),
            EntityType::RasterImage(image) => {
                self.draw_raster(image, entity, frame, transform, source);
            }
            EntityType::Ole2Frame(frame_entity) => {
                self.draw_ole(frame_entity, entity, frame, transform, source);
            }
            EntityType::Underlay(underlay) => {
                self.draw_underlay(underlay, entity, frame, transform, source);
            }
            EntityType::Ray(ray) => self.draw_ray(ray, entity, frame, transform, source),
            EntityType::XLine(line) => self.draw_xline(line, entity, frame, transform, source),
            EntityType::Unknown(_) => self.draw_cached_graphics(entity, frame, transform, source),
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
        let saved_visual = self.scene.visual_style.clone();
        self.scene.visual_style = if !viewport.visual_style_handle.is_null() {
            visual_style::resolve(
                self.doc,
                &visual_style::VisualStyleRequest::Object(viewport.visual_style_handle),
            )
        } else {
            visual_style::resolve(
                self.doc,
                &visual_style::VisualStyleRequest::RenderMode(viewport.render_mode),
            )
        };
        self.pattern_scale = if self.doc.header.paper_space_linetype_scaling {
            1.0
        } else {
            scale.abs()
        };
        let stroke_from = self.scene.strokes.len();
        let fill_from = self.scene.fills.len();
        let glyph_from = self.scene.glyphs.len();
        let picture_from = self.scene.pictures.len();
        self.walk_block("*Model_Space", &Frame::model(), &to_paper, &[]);
        self.pattern_scale = saved_scale;
        self.viewport_frozen = saved_frozen;
        self.scene.visual_style = saved_visual;
        clip_viewport_depth(
            &mut self.scene,
            stroke_from,
            fill_from,
            glyph_from,
            picture_from,
            viewport,
            &to_paper,
        );
        clip_recent_geometry(
            &mut self.scene,
            stroke_from,
            fill_from,
            glyph_from,
            picture_from,
            &clip,
        );
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
        let handles = self.ordered_handles(record);
        for cell in cells {
            let to_world = base.then(&cell).then(transform);
            let stroke_from = self.scene.strokes.len();
            let fill_from = self.scene.fills.len();
            let glyph_from = self.scene.glyphs.len();
            let picture_from = self.scene.pictures.len();
            for handle in &handles {
                let Some(entity) = self.doc.get_entity(*handle) else {
                    continue;
                };
                self.draw(entity, &child, &to_world, &child_path);
            }
            if let Some(polygon) = clip_polygon_world(self.doc, insert, &to_world) {
                clip_recent_geometry(
                    &mut self.scene,
                    stroke_from,
                    fill_from,
                    glyph_from,
                    picture_from,
                    &polygon,
                );
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
        self.draw_label_with_transforms(
            label, style_name, entity, frame, transform, transform, source,
        );
    }

    fn draw_label_with_transforms(
        &mut self,
        label: &super::text_draw::GlyphLabel<'_>,
        style_name: &str,
        entity: &EntityType,
        frame: &Frame,
        anchor_transform: &Transform,
        glyph_transform: &Transform,
        source: EntityRef,
    ) {
        let effective_label = self.effective_label(label, style_name);
        let label = &effective_label;
        if self.doc.header.quick_text_mode {
            let points = world(anchor_transform, super::text_draw::quick_box(label));
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
        let mut style = resolve_style(self.doc, entity.common(), frame, &visual, self.sheet);
        apply_face_color(&mut style.rgb, visual.face_color);
        match produced {
            PreparedLabel::Strokes(chains) => {
                for chain in chains {
                    self.emit_plain(
                        &world(anchor_transform, chain),
                        &[],
                        style.clone(),
                        source.clone(),
                    );
                }
            }
            PreparedLabel::TrueType(path) => {
                let anchor = world(
                    anchor_transform,
                    vec![if label.use_alignment {
                        label.alignment
                    } else {
                        label.insertion
                    }],
                );
                let local_x = Matrix3::arbitrary_axis(label.normal)
                    * (Matrix3::rotation_z(label.rotation) * Vector3::UNIT_X);
                let local_y = Matrix3::arbitrary_axis(label.normal)
                    * (Matrix3::rotation_z(label.rotation) * Vector3::UNIT_Y);
                let axis_x = glyph_transform.apply_rotation(local_x);
                let axis_y = glyph_transform.apply_rotation(local_y);
                let scale_x = axis_x.length().max(1e-9);
                let scale_y = axis_y.length().max(1e-9);
                self.scene.glyphs.push(super::scene::GlyphRun {
                    text: label.text.to_string(),
                    anchor: anchor.first().copied().unwrap_or(label.insertion),
                    rotation: 0.0,
                    height: label.height * scale_y,
                    width_factor: label.width_factor * scale_x / scale_y,
                    horizontal: label.horizontal,
                    vertical: label.vertical,
                    normal: {
                        let normal = axis_x.cross(&axis_y);
                        if normal.length_squared() > 1e-18 {
                            normal * (1.0 / normal.length())
                        } else {
                            Vector3::UNIT_Z
                        }
                    },
                    axis_x: axis_x * (1.0 / scale_x),
                    axis_y: axis_y * (1.0 / scale_y),
                    oblique: label.oblique,
                    backward: label.backward,
                    upside_down: label.upside_down,
                    font_path: path,
                    rgb: style.rgb,
                    aci7: style.aci7,
                    opacity: style.opacity,
                    source,
                });
            }
        }
    }

    fn effective_label<'a>(
        &self,
        label: &super::text_draw::GlyphLabel<'a>,
        style_name: &str,
    ) -> super::text_draw::GlyphLabel<'a> {
        let Some(style) = self.doc.text_styles.get(style_name) else {
            return super::text_draw::GlyphLabel {
                text: label.text,
                shape_number: label.shape_number,
                insertion: label.insertion,
                alignment: label.alignment,
                use_alignment: label.use_alignment,
                height: label.height,
                rotation: label.rotation,
                width_factor: label.width_factor,
                oblique: label.oblique,
                horizontal: label.horizontal,
                vertical: label.vertical,
                normal: label.normal,
                backward: label.backward,
                upside_down: label.upside_down,
            };
        };
        let entity_width = if label.width_factor == 0.0 {
            1.0
        } else {
            label.width_factor
        };
        let style_width = if style.width_factor == 0.0 {
            1.0
        } else {
            style.width_factor
        };
        super::text_draw::GlyphLabel {
            text: label.text,
            shape_number: label.shape_number,
            insertion: label.insertion,
            alignment: label.alignment,
            use_alignment: label.use_alignment,
            height: label.height,
            rotation: label.rotation,
            width_factor: entity_width * style_width,
            oblique: label.oblique + style.oblique_angle,
            horizontal: label.horizontal,
            vertical: label.vertical,
            normal: label.normal,
            backward: label.backward || style.flags.backward,
            upside_down: label.upside_down || style.flags.upside_down,
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
        let mut parts = entity.explode();
        if let EntityType::MText(mtext) = entity {
            self.layout_mtext_parts(mtext, &mut parts);
        }
        if parts.is_empty() {
            self.fallback(entity, frame, source);
            return;
        }
        for part in &parts {
            self.draw(part, frame, transform, path);
        }
        let _ = source;
    }

    fn layout_mtext_parts(&mut self, mtext: &crate::entities::MText, parts: &mut [EntityType]) {
        let normal = if mtext.normal.length_squared() > 1e-20 {
            mtext.normal.normalize()
        } else {
            Vector3::UNIT_Z
        };
        let fallback = Vector3::new(mtext.rotation.cos(), mtext.rotation.sin(), 0.0);
        let raw_x = mtext.dwg_x_direction.unwrap_or(fallback);
        let projected = raw_x - normal * raw_x.dot(&normal);
        let x_axis = if projected.length_squared() > 1e-20 {
            projected.normalize()
        } else {
            (Matrix3::arbitrary_axis(normal) * Vector3::UNIT_X).normalize()
        };
        let y_axis = normal.cross(&x_axis).normalize();

        let mut row_y: Option<f64> = None;
        let mut row_start = Vector3::ZERO;
        let mut advance = 0.0;
        for part in parts {
            let EntityType::Text(text) = part else {
                continue;
            };
            let relative = text.insertion_point - mtext.insertion_point;
            let y = relative.dot(&y_axis);
            if row_y.is_none_or(|previous| (previous - y).abs() > 1e-7) {
                row_y = Some(y);
                row_start = text.insertion_point;
                advance = 0.0;
            }
            text.insertion_point = row_start + x_axis * advance;
            if text.alignment_point.is_some() {
                text.alignment_point = Some(text.insertion_point);
            }
            advance += self.text_advance(text);
        }
    }

    fn text_advance(&mut self, text: &Text) -> f64 {
        let height = text.height.abs();
        if height <= 1e-12 {
            return 0.0;
        }
        let entity_width = if text.width_factor == 0.0 {
            1.0
        } else {
            text.width_factor
        };
        let style_width = self
            .doc
            .text_styles
            .get(&text.style)
            .map(|style| {
                if style.width_factor == 0.0 {
                    1.0
                } else {
                    style.width_factor
                }
            })
            .unwrap_or(1.0);
        let width_factor = entity_width * style_width;
        let fallback =
            crate::entities::text::estimated_width_in_heights(&text.value) * height * width_factor;
        let Some(face) = self.font_for(&text.style) else {
            return fallback;
        };
        match face {
            FaceFont::Shape(font) => {
                text.value
                    .chars()
                    .map(|ch| font.glyph_for_char(ch).1)
                    .sum::<f64>()
                    * height
                    / font.above()
                    * width_factor
            }
            FaceFont::TrueType(path) => {
                let Some(bytes) = std::fs::read(path).ok() else {
                    return fallback;
                };
                let Some(font) =
                    fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).ok()
                else {
                    return fallback;
                };
                text.value
                    .chars()
                    .map(|ch| font.metrics(ch, 1.0).advance_width as f64)
                    .sum::<f64>()
                    * height
                    * width_factor
            }
        }
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
        self.emit_path(points, entity, frame, source, DrawPath::Dedicated);
    }

    fn emit_path(
        &mut self,
        points: &[Vector3],
        entity: &EntityType,
        frame: &Frame,
        source: EntityRef,
        path: DrawPath,
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
        emit_chain(
            &mut self.scene.strokes,
            points,
            &lengths,
            style,
            source.clone(),
            path,
        );
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
                        let glyph_transform = self.active_transform.clone();
                        let shape_scale = if matches!(
                            self.font_for(&mark.style_name),
                            Some(FaceFont::Shape(_))
                        ) {
                            glyph_transform
                                .apply_rotation(Vector3::UNIT_X)
                                .length()
                                .max(1e-9)
                        } else {
                            1.0
                        };
                        self.draw_label_with_transforms(
                            &super::text_draw::GlyphLabel {
                                text: &text,
                                shape_number: mark.shape_number,
                                insertion: anchor,
                                alignment: anchor,
                                use_alignment: false,
                                height: mark.scale.abs().max(1e-6) * shape_scale,
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
                            &glyph_transform,
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
            normals: Vec::new(),
            lighting: visual.lighting,
        });
    }

    fn draw_acis(
        &mut self,
        data: &crate::entities::solid3d::AcisData,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        let Some(document) = data.parse() else {
            self.fallback(entity, frame, source);
            return;
        };
        let tess = super::tess::tessellate(&document);
        if tess.triangles.is_empty() && tess.edges.is_empty() {
            self.fallback(entity, frame, source);
            return;
        }
        let visual = self.scene.visual_style.clone();
        let wireframe = self.solid_style == super::SolidStyle::Wireframe;
        let depth_only = !wireframe
            && visual.faces == super::visual_style::FaceShading::None
            && visual.edges == super::visual_style::EdgeMode::Facet;
        let draw_faces = !wireframe
            && (visual.faces != super::visual_style::FaceShading::None
                || visual.kind == super::visual_style::StandardVisualStyle::Wireframe2D);
        if draw_faces && !tess.triangles.is_empty() {
            let triangles = world(transform, tess.triangles);
            self.emit_fill(&triangles, entity, frame, source.clone(), false);
            if let Some(fill) = self.scene.fills.last_mut() {
                fill.normals = transform_normals(transform, &tess.normals);
                if visual.lighting == super::visual_style::LightingModel::None
                    && self.solid_style == super::SolidStyle::Wireframe
                {
                    fill.normals.clear();
                }
                if depth_only {
                    fill.style.opacity = 0.0;
                } else {
                    fill.style.opacity *= visual.face_opacity as f32;
                }
            }
        }
        let draw_edges = wireframe
            || self.solid_style == super::SolidStyle::ShadedWithEdges
            || visual.edges != super::visual_style::EdgeMode::None;
        if draw_edges {
            for edge in &tess.edges {
                if edge.len() >= 2 {
                    self.emit_dedicated(
                        &world(transform, edge.clone()),
                        entity,
                        frame,
                        source.clone(),
                    );
                }
            }
        }
        if wireframe || visual.edges == super::visual_style::EdgeMode::Isolines {
            for line in &tess.isolines {
                if line.len() >= 2 {
                    self.emit_dedicated(
                        &world(transform, line.clone()),
                        entity,
                        frame,
                        source.clone(),
                    );
                }
            }
        }
    }

    fn draw_mesh_entity(
        &mut self,
        mesh: &crate::entities::Mesh,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        let faces = mesh
            .faces
            .iter()
            .map(|face| face.indices().to_vec())
            .collect::<Vec<_>>();
        self.draw_faceted(&mesh.vertices, &faces, entity, frame, transform, source);
    }

    fn draw_polyface(
        &mut self,
        mesh: &crate::entities::PolyfaceMesh,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        let vertices = mesh
            .vertices
            .iter()
            .map(|vertex| vertex.location)
            .collect::<Vec<_>>();
        let mut faces = Vec::new();
        let mut edges = Vec::new();
        for face in &mesh.faces {
            let indices = [
                face.index1,
                face.index2,
                face.index3,
                if face.is_quad() { face.index4 } else { 0 },
            ];
            let mut polygon = Vec::new();
            for (edge_index, index) in indices.into_iter().enumerate() {
                if index == 0 {
                    continue;
                }
                let slot = index.unsigned_abs() as usize;
                if slot == 0 || slot > vertices.len() {
                    continue;
                }
                polygon.push(slot - 1);
                let invisible = match edge_index {
                    0 => face.is_edge1_invisible(),
                    1 => face.is_edge2_invisible(),
                    2 => face.is_edge3_invisible(),
                    _ => face.is_edge4_invisible(),
                };
                if !invisible {
                    let next = indices
                        .iter()
                        .cycle()
                        .skip(edge_index + 1)
                        .find(|candidate| **candidate != 0)
                        .copied()
                        .unwrap_or(0);
                    let next_slot = next.unsigned_abs() as usize;
                    if next_slot > 0 && next_slot <= vertices.len() {
                        edges.push(vec![vertices[slot - 1], vertices[next_slot - 1]]);
                    }
                }
            }
            if polygon.len() >= 3 {
                faces.push(polygon);
            }
        }
        let mut tess = super::tess::facet_faces(&vertices, &faces);
        tess.edges.clear();
        self.emit_tess(&tess, edges, entity, frame, transform, source);
    }

    fn draw_polygon_mesh(
        &mut self,
        mesh: &crate::entities::polygon_mesh::PolygonMesh,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        let vertices = mesh
            .vertices
            .iter()
            .map(|vertex| vertex.location)
            .collect::<Vec<_>>();
        let tess = super::tess::facet_grid(
            &vertices,
            mesh.m_vertex_count.max(0) as usize,
            mesh.n_vertex_count.max(0) as usize,
            mesh.is_closed_m(),
            mesh.is_closed_n(),
        );
        self.emit_tess(&tess, Vec::new(), entity, frame, transform, source);
    }

    fn draw_faceted(
        &mut self,
        vertices: &[Vector3],
        faces: &[Vec<usize>],
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        if faces.is_empty() {
            self.fallback(entity, frame, source);
            return;
        }
        let tess = super::tess::facet_faces(vertices, faces);
        self.emit_tess(&tess, Vec::new(), entity, frame, transform, source);
    }

    fn emit_tess(
        &mut self,
        tess: &super::tess::Tessellation,
        extra_edges: Vec<Vec<Vector3>>,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        if tess.triangles.is_empty() && tess.edges.is_empty() && extra_edges.is_empty() {
            self.fallback(entity, frame, source);
            return;
        }
        let visual = self.scene.visual_style.clone();
        let draw_faces = self.solid_style != super::SolidStyle::Wireframe
            && (visual.faces != super::visual_style::FaceShading::None
                || visual.kind == super::visual_style::StandardVisualStyle::Wireframe2D);
        if draw_faces && !tess.triangles.is_empty() {
            let triangles = world(transform, tess.triangles.clone());
            self.emit_fill(&triangles, entity, frame, source.clone(), false);
            if let Some(fill) = self.scene.fills.last_mut() {
                fill.normals = transform_normals(transform, &tess.normals);
                if visual.lighting == super::visual_style::LightingModel::None
                    && self.solid_style == super::SolidStyle::Wireframe
                {
                    fill.normals.clear();
                }
            }
        }
        if self.solid_style == super::SolidStyle::Wireframe
            && visual.edges == super::visual_style::EdgeMode::None
        {
            return;
        }
        for edge in tess.edges.iter().chain(extra_edges.iter()) {
            if edge.len() >= 2 {
                self.emit_dedicated(
                    &world(transform, edge.clone()),
                    entity,
                    frame,
                    source.clone(),
                );
            }
        }
    }

    fn draw_light(
        &mut self,
        light: &crate::entities::Light,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        let position = light.position;
        let size = 1.0;
        let arms = [
            vec![
                position + Vector3::new(-size, 0.0, 0.0),
                position + Vector3::new(size, 0.0, 0.0),
            ],
            vec![
                position + Vector3::new(0.0, -size, 0.0),
                position + Vector3::new(0.0, size, 0.0),
            ],
            vec![
                position + Vector3::new(0.0, 0.0, -size),
                position + Vector3::new(0.0, 0.0, size),
            ],
        ];
        for arm in arms {
            self.emit_dedicated(&world(transform, arm), entity, frame, source.clone());
        }
        if (light.target - position).length_squared() > 1e-8 {
            self.emit_dedicated(
                &world(transform, vec![position, light.target]),
                entity,
                frame,
                source,
            );
        }
    }

    fn draw_camera(
        &mut self,
        view_handle: crate::types::Handle,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        let Some(view) = self
            .doc
            .views
            .iter()
            .find(|view| view.handle == view_handle)
        else {
            self.scene.skipped.push(SkippedEntity {
                kind: "Camera".to_string(),
                reason: "camera view was not found".to_string(),
                source,
            });
            return;
        };
        let direction = if view.direction.length_squared() > 1e-12 {
            view.direction.normalize()
        } else {
            Vector3::UNIT_Z
        };
        let distance = view.lens_length.abs().max(1.0);
        let eye = view.target + direction * distance;
        let side = if direction.x.abs() < 0.9 {
            direction.cross(&Vector3::UNIT_X).normalize()
        } else {
            direction.cross(&Vector3::UNIT_Y).normalize()
        };
        let up = side.cross(&direction).normalize();
        let span = distance * 0.2;
        let corners = [
            view.target + side * span + up * span,
            view.target - side * span + up * span,
            view.target - side * span - up * span,
            view.target + side * span - up * span,
        ];
        for index in 0..4 {
            self.emit_dedicated(
                &world(transform, vec![eye, corners[index]]),
                entity,
                frame,
                source.clone(),
            );
            self.emit_dedicated(
                &world(transform, vec![corners[index], corners[(index + 1) % 4]]),
                entity,
                frame,
                source.clone(),
            );
        }
    }

    fn draw_ray(
        &mut self,
        ray: &crate::entities::Ray,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        self.draw_infinite(
            ray.base_point,
            ray.direction,
            false,
            entity,
            frame,
            transform,
            source,
        );
    }

    fn draw_xline(
        &mut self,
        line: &crate::entities::XLine,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        self.draw_infinite(
            line.base_point,
            line.direction,
            true,
            entity,
            frame,
            transform,
            source,
        );
    }

    fn draw_infinite(
        &mut self,
        origin: Vector3,
        direction: Vector3,
        both_ways: bool,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        if direction.length_squared() <= 1e-18 {
            self.scene.skipped.push(SkippedEntity {
                kind: kind_of(entity).to_string(),
                reason: "direction is zero".to_string(),
                source,
            });
            return;
        }
        let direction = direction.normalize();
        let reach = infinite_reach(self.doc);
        let start = if both_ways {
            origin - direction * reach
        } else {
            origin
        };
        let end = origin + direction * reach;
        self.emit_path(
            &world(transform, vec![start, end]),
            entity,
            frame,
            source,
            DrawPath::Infinite,
        );
    }

    fn draw_point_cloud_box(
        &mut self,
        min: Vector3,
        max: Vector3,
        clips: &[crate::entities::PointCloudClip],
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        if !extent_is_real(min, max) {
            self.scene.skipped.push(SkippedEntity {
                kind: kind_of(entity).to_string(),
                reason: "point cloud extents were not stored".to_string(),
                source,
            });
            return;
        }
        self.draw_extent_edges(min, max, entity, frame, transform, source.clone());
        for clip in clips {
            if clip.vertices.len() < 2 {
                continue;
            }
            let mut points: Vec<Vector3> = clip
                .vertices
                .iter()
                .map(|point| Vector3::new(point.x, point.y, min.z))
                .collect();
            points.push(points[0]);
            self.emit_dedicated(&world(transform, points), entity, frame, source.clone());
        }
    }

    fn draw_coordination(
        &mut self,
        data: &crate::entities::CoordinationModelData,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        let definition = self
            .doc
            .objects
            .get(&data.definition_handle)
            .and_then(|object| match object {
                crate::objects::ObjectType::ClassObject(class) => match &class.data {
                    crate::objects::ClassObjectData::NavisworksModelDefinition(definition) => {
                        Some(definition)
                    }
                    _ => None,
                },
                _ => None,
            });
        let Some(definition) = definition else {
            self.scene.skipped.push(SkippedEntity {
                kind: "CoordinationModel".to_string(),
                reason: "coordination model extents were not stored".to_string(),
                source,
            });
            return;
        };
        if !extent_is_real(definition.extents_min, definition.extents_max) {
            self.scene.skipped.push(SkippedEntity {
                kind: "CoordinationModel".to_string(),
                reason: "coordination model extents were not stored".to_string(),
                source,
            });
            return;
        }
        let scale = if data.unit_factor.is_finite() && data.unit_factor.abs() > 1e-12 {
            data.unit_factor
        } else {
            1.0
        };
        let placement = matrix_from_rows(&data.transform);
        let corners = box_corners(definition.extents_min, definition.extents_max)
            .map(|corner| placement.transform_point(corner * scale));
        for edge in box_edges(&corners) {
            self.emit_dedicated(
                &world(transform, edge.to_vec()),
                entity,
                frame,
                source.clone(),
            );
        }
    }

    fn draw_cached_graphics(
        &mut self,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        let kind = kind_of(entity).to_string();
        let Some(graphics) = entity.common().proxy_graphics() else {
            self.scene.skipped.push(SkippedEntity {
                kind,
                reason: "proxy graphics were not stored".to_string(),
                source,
            });
            return;
        };
        if !self.draw_proxy_records(&graphics, entity, frame, transform, source.clone()) {
            self.scene.skipped.push(SkippedEntity {
                kind,
                reason: "proxy graphics had no supported records".to_string(),
                source,
            });
        }
    }

    fn draw_arch(
        &mut self,
        data: &crate::classes::arch::ArchEntityData,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        let mut drew = false;
        for profile in &data.profiles {
            if profile.len() >= 2 {
                self.emit_dedicated(
                    &world(transform, profile.clone()),
                    entity,
                    frame,
                    source.clone(),
                );
                drew = true;
            }
        }
        if let Some(wall) = data.straight_wall_geometry() {
            let direction = wall.end - wall.start;
            let mut side = Vector3::new(-direction.y, direction.x, 0.0);
            if side.length_squared() > 1e-18 {
                side = side.normalize() * wall.width;
                let ring = vec![
                    wall.start,
                    wall.end,
                    wall.end + side,
                    wall.start + side,
                    wall.start,
                ];
                self.emit_dedicated(&world(transform, ring), entity, frame, source.clone());
                drew = true;
            }
        }
        if !drew {
            self.draw_cached_graphics(entity, frame, transform, source);
        }
    }

    fn draw_proxy_records(
        &mut self,
        graphics: &crate::entities::ProxyGraphics,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) -> bool {
        let mut drew = false;
        for record in &graphics.records {
            match record {
                crate::entities::ProxyGraphicRecord::UnicodeText(text) => {
                    self.draw_label(
                        &super::text_draw::GlyphLabel {
                            text: &text.text,
                            shape_number: None,
                            insertion: text.position,
                            alignment: text.position,
                            use_alignment: false,
                            height: text.height.abs().max(1e-6),
                            rotation: text.direction.y.atan2(text.direction.x),
                            width_factor: if text.width_factor == 0.0 {
                                1.0
                            } else {
                                text.width_factor
                            },
                            oblique: text.oblique_angle,
                            horizontal: 0,
                            vertical: 0,
                            normal: text.normal,
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
                crate::entities::ProxyGraphicRecord::Unknown {
                    record_type: 6,
                    data,
                } => {
                    if let Some(points) = crate::classes::arch::polyline_from_type6(data) {
                        self.emit_dedicated(
                            &world(transform, points),
                            entity,
                            frame,
                            source.clone(),
                        );
                        drew = true;
                    }
                }
                crate::entities::ProxyGraphicRecord::FillOff
                | crate::entities::ProxyGraphicRecord::Unknown { .. } => {}
            }
        }
        drew
    }

    fn draw_placed_underlay(
        &mut self,
        underlay: &crate::entities::Underlay,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        if !underlay
            .flags
            .contains(crate::entities::UnderlayDisplayFlags::ON)
        {
            self.scene.skipped.push(SkippedEntity {
                kind: "Underlay".to_string(),
                reason: "underlay is off".to_string(),
                source,
            });
            return;
        }
        let kind = if underlay.underlay_type == crate::entities::UnderlayType::Dgn {
            "dgn"
        } else {
            "dwf"
        };
        if underlay
            .flags
            .contains(crate::entities::UnderlayDisplayFlags::CLIPPING)
            && underlay.clip_boundary_vertices.len() >= 2
        {
            let mut points: Vec<Vector3> = underlay
                .clip_boundary_vertices
                .iter()
                .map(|point| place_underlay_local(underlay, point.x, point.y))
                .collect();
            points.push(points[0]);
            self.emit_dedicated(&world(transform, points), entity, frame, source.clone());
        }
        let path = self.underlay_path(underlay);
        let Some(bytes) = self.read_resource(&path) else {
            self.scene.skipped.push(SkippedEntity {
                kind: "Underlay".to_string(),
                reason: format!("{kind} file {path} was not found"),
                source,
            });
            return;
        };
        if underlay.underlay_type == crate::entities::UnderlayType::Dgn {
            self.scene.skipped.push(SkippedEntity {
                kind: "Underlay".to_string(),
                reason: "dgn elements were not decoded".to_string(),
                source,
            });
            return;
        }
        match super::external::read_dwf(&bytes) {
            Ok(vectors) => {
                let stroke_from = self.scene.strokes.len();
                let fill_from = self.scene.fills.len();
                let glyph_from = self.scene.glyphs.len();
                let picture_from = self.scene.pictures.len();
                for stroke in vectors.strokes {
                    let points = world(
                        transform,
                        stroke
                            .into_iter()
                            .map(|(x, y)| place_underlay_local(underlay, x, y))
                            .collect(),
                    );
                    self.emit_dedicated(&points, entity, frame, source.clone());
                }
                if underlay
                    .flags
                    .contains(crate::entities::UnderlayDisplayFlags::CLIPPING)
                    && !underlay.clip_inverted
                {
                    let boundary = world(transform, underlay.world_clip_boundary())
                        .into_iter()
                        .map(|point| crate::types::Vector2::new(point.x, point.y))
                        .collect::<Vec<_>>();
                    clip_recent_geometry(
                        &mut self.scene,
                        stroke_from,
                        fill_from,
                        glyph_from,
                        picture_from,
                        &boundary,
                    );
                }
                apply_underlay_appearance(&mut self.scene, stroke_from, fill_from, underlay);
            }
            Err(reason) => {
                self.scene.skipped.push(SkippedEntity {
                    kind: "Underlay".to_string(),
                    reason: reason.to_string(),
                    source,
                });
            }
        }
    }

    fn draw_extent_edges(
        &mut self,
        min: Vector3,
        max: Vector3,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        let corners = box_corners(min, max);
        for edge in box_edges(&corners) {
            self.emit_dedicated(
                &world(transform, edge.to_vec()),
                entity,
                frame,
                source.clone(),
            );
        }
    }

    fn draw_raster(
        &mut self,
        image: &crate::entities::RasterImage,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        if !image
            .flags
            .contains(crate::entities::ImageDisplayFlags::SHOW_IMAGE)
        {
            self.scene.skipped.push(SkippedEntity {
                kind: "RasterImage".to_string(),
                reason: "image display is off".to_string(),
                source,
            });
            return;
        }
        self.draw_image_frame(image.corners(), entity, frame, transform, source.clone());
        let path = self.image_file_path(image);
        let Some(bytes) = self.read_resource(&path) else {
            self.scene.skipped.push(SkippedEntity {
                kind: "RasterImage".to_string(),
                reason: format!("image file {path} was not found"),
                source,
            });
            return;
        };
        match super::bitmap::decode(&bytes) {
            Ok(mut decoded) => {
                let opacity = super::bitmap::adjust(
                    &mut decoded,
                    image.brightness,
                    image.contrast,
                    image.fade,
                );
                if !image
                    .flags
                    .contains(crate::entities::ImageDisplayFlags::TRANSPARENCY_ON)
                {
                    for pixel in decoded.pixels.chunks_exact_mut(4) {
                        pixel[3] = 255;
                    }
                }
                self.push_raster(image, &decoded, opacity, transform, source);
            }
            Err(reason) => {
                self.scene.skipped.push(SkippedEntity {
                    kind: "RasterImage".to_string(),
                    reason: reason.to_string(),
                    source,
                });
            }
        }
    }

    fn draw_ole(
        &mut self,
        ole: &crate::entities::Ole2Frame,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        let corners = ole_corners(ole.upper_left_corner, ole.lower_right_corner);
        self.draw_image_frame(corners, entity, frame, transform, source.clone());
        let Some(presentation) = ole.presentation() else {
            self.scene.skipped.push(SkippedEntity {
                kind: "Ole2Frame".to_string(),
                reason: "ole frame had no picture".to_string(),
                source,
            });
            return;
        };
        match presentation {
            crate::entities::OlePresentation::Raster(bytes)
            | crate::entities::OlePresentation::Dib(bytes) => match super::bitmap::decode(&bytes) {
                Ok(decoded) => {
                    self.push_picture(&decoded, corners, 1.0, transform, source);
                }
                Err(reason) => {
                    self.scene.skipped.push(SkippedEntity {
                        kind: "Ole2Frame".to_string(),
                        reason: reason.to_string(),
                        source,
                    });
                }
            },
            crate::entities::OlePresentation::Emf(bytes) => {
                self.draw_metafile(
                    super::metafile::parse_emf(&bytes),
                    corners,
                    entity,
                    frame,
                    transform,
                    source,
                );
            }
            crate::entities::OlePresentation::Wmf(bytes) => {
                self.draw_metafile(
                    super::metafile::parse_wmf(&bytes),
                    corners,
                    entity,
                    frame,
                    transform,
                    source,
                );
            }
        }
    }

    fn draw_underlay(
        &mut self,
        underlay: &crate::entities::Underlay,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        if underlay.underlay_type != crate::entities::UnderlayType::Pdf {
            self.draw_placed_underlay(underlay, entity, frame, transform, source);
            return;
        }
        if !underlay
            .flags
            .contains(crate::entities::UnderlayDisplayFlags::ON)
        {
            self.scene.skipped.push(SkippedEntity {
                kind: "Underlay".to_string(),
                reason: "pdf underlay is off".to_string(),
                source,
            });
            return;
        }
        let path = self.underlay_path(underlay);
        let Some(bytes) = self.read_resource(&path) else {
            self.scene.skipped.push(SkippedEntity {
                kind: "Underlay".to_string(),
                reason: format!("pdf file {path} was not found"),
                source,
            });
            return;
        };
        let page = underlay_page(self.doc, underlay);
        match super::pdf_page::read_page(&bytes, page) {
            Ok(vectors) => {
                let empty = vectors.strokes.is_empty() && vectors.fills.is_empty();
                let stroke_from = self.scene.strokes.len();
                let fill_from = self.scene.fills.len();
                let glyph_from = self.scene.glyphs.len();
                let picture_from = self.scene.pictures.len();
                let map = |point: (f64, f64)| {
                    underlay_point(underlay, vectors.media_min, vectors.media_max, point)
                };
                for stroke in vectors.strokes {
                    let points = world(transform, stroke.into_iter().map(map).collect());
                    self.emit_dedicated(&points, entity, frame, source.clone());
                }
                for fill in vectors.fills {
                    let ring: Vec<Vector3> = fill.into_iter().map(map).collect();
                    let triangles = super::curves::fan_triangles(&world(transform, ring));
                    self.emit_fill(&triangles, entity, frame, source.clone(), false);
                }
                let page_frame = [
                    map((vectors.media_min.0, vectors.media_min.1)),
                    map((vectors.media_max.0, vectors.media_min.1)),
                    map((vectors.media_max.0, vectors.media_max.1)),
                    map((vectors.media_min.0, vectors.media_max.1)),
                    map((vectors.media_min.0, vectors.media_min.1)),
                ];
                self.emit_dedicated(
                    &world(transform, page_frame.to_vec()),
                    entity,
                    frame,
                    source.clone(),
                );
                if underlay
                    .flags
                    .contains(crate::entities::UnderlayDisplayFlags::CLIPPING)
                    && !underlay.clip_inverted
                    && underlay.clip_boundary_vertices.len() >= 3
                {
                    let boundary = world(transform, underlay.world_clip_boundary())
                        .into_iter()
                        .map(|point| crate::types::Vector2::new(point.x, point.y))
                        .collect::<Vec<_>>();
                    clip_recent_geometry(
                        &mut self.scene,
                        stroke_from,
                        fill_from,
                        glyph_from,
                        picture_from,
                        &boundary,
                    );
                }
                apply_underlay_appearance(&mut self.scene, stroke_from, fill_from, underlay);
                if empty {
                    self.scene.skipped.push(SkippedEntity {
                        kind: "Underlay".to_string(),
                        reason: "pdf page had no supported path operators".to_string(),
                        source,
                    });
                }
            }
            Err(reason) => {
                self.scene.skipped.push(SkippedEntity {
                    kind: "Underlay".to_string(),
                    reason: reason.to_string(),
                    source,
                });
            }
        }
    }

    fn draw_arc_text(
        &mut self,
        data: &crate::entities::ArcAlignedTextData,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        let style_name = if data.style_name.is_empty() {
            "Standard"
        } else {
            data.style_name.as_str()
        };
        let (advances, above) = {
            let Some(face) = self.font_for(style_name) else {
                self.scene.skipped.push(SkippedEntity {
                    kind: "ArcAlignedText".to_string(),
                    reason: format!("font for style {style_name} was not found"),
                    source,
                });
                return;
            };
            match face {
                FaceFont::Shape(font) => (
                    data.text
                        .chars()
                        .map(|ch| font.glyph_for_char(ch).1)
                        .collect::<Vec<_>>(),
                    font.above(),
                ),
                FaceFont::TrueType(path) => {
                    let advances = std::fs::read(path)
                        .ok()
                        .and_then(|bytes| {
                            fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).ok()
                        })
                        .map(|font| {
                            data.text
                                .chars()
                                .map(|ch| font.metrics(ch, 1.0).advance_width as f64)
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_else(|| vec![0.5; data.text.chars().count()]);
                    (advances, 1.0)
                }
            }
        };
        let unit = if data.text_size > 0.0 && above > 0.0 {
            data.text_size / above
        } else {
            data.text_size.abs().max(1.0)
        };
        let widths: Vec<f64> = advances
            .iter()
            .map(|advance| advance * unit * data.x_scale.abs().max(1e-6) + data.character_spacing)
            .collect();
        let anchors = arc_anchors(
            data.center,
            data.radius + data.offset_from_arc,
            data.start_angle,
            &widths,
            data.reverse,
        );
        for (ch, anchor) in data.text.chars().zip(anchors) {
            let text = ch.to_string();
            self.draw_label(
                &super::text_draw::GlyphLabel {
                    text: &text,
                    shape_number: None,
                    insertion: anchor.position,
                    alignment: anchor.position,
                    use_alignment: false,
                    height: data.text_size.abs().max(1e-6),
                    rotation: anchor.rotation,
                    width_factor: data.x_scale.abs().max(1e-6),
                    oblique: 0.0,
                    horizontal: 0,
                    vertical: 0,
                    normal: data.normal,
                    backward: false,
                    upside_down: false,
                },
                style_name,
                entity,
                frame,
                transform,
                source.clone(),
            );
        }
    }

    fn draw_remote_text(
        &mut self,
        data: &crate::entities::RemoteTextData,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        let style_name = if data.style_name.is_empty() {
            "Standard"
        } else {
            data.style_name.as_str()
        };
        self.draw_label(
            &super::text_draw::GlyphLabel {
                text: &data.text,
                shape_number: None,
                insertion: data.position,
                alignment: data.position,
                use_alignment: false,
                height: data.height.abs().max(1e-6),
                rotation: data.rotation,
                width_factor: 1.0,
                oblique: 0.0,
                horizontal: 0,
                vertical: 0,
                normal: data.normal,
                backward: false,
                upside_down: false,
            },
            style_name,
            entity,
            frame,
            transform,
            source,
        );
    }

    fn draw_geo_marker(
        &mut self,
        data: &crate::entities::GeoPositionMarkerData,
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        let radius = if data.radius > 1e-9 { data.radius } else { 1.0 };
        let circle = crate::entities::Circle::from_coords(
            data.position.x,
            data.position.y,
            data.position.z,
            radius,
        );
        self.emit_dedicated(
            &world(transform, super::curves::circle_points(&circle)),
            entity,
            frame,
            source.clone(),
        );
        if data.landing_gap.abs() > 1e-9 {
            self.emit_dedicated(
                &world(
                    transform,
                    vec![
                        data.position,
                        data.position + Vector3::new(0.0, 0.0, -data.landing_gap),
                    ],
                ),
                entity,
                frame,
                source.clone(),
            );
        }
        if data.mtext_visible {
            if let Some(mtext) = &data.embedded_mtext {
                let mtext = EntityType::MText(Box::new(mtext.clone()));
                self.draw(&mtext, frame, transform, &[]);
            }
        }
    }

    fn draw_metafile(
        &mut self,
        picture: Option<super::metafile::MetaPicture>,
        corners: [Vector3; 4],
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        let Some(picture) = picture else {
            self.scene.skipped.push(SkippedEntity {
                kind: "Ole2Frame".to_string(),
                reason: "metafile was not recognized".to_string(),
                source,
            });
            return;
        };
        if picture.strokes.is_empty() && picture.fills.is_empty() {
            self.scene.skipped.push(SkippedEntity {
                kind: "Ole2Frame".to_string(),
                reason: "metafile had no supported records".to_string(),
                source,
            });
            return;
        }
        let map = |point: (f64, f64)| meta_point(picture.bounds, corners, point);
        for stroke in picture.strokes {
            self.emit_dedicated(
                &world(transform, stroke.into_iter().map(map).collect()),
                entity,
                frame,
                source.clone(),
            );
        }
        for fill in picture.fills {
            let ring: Vec<Vector3> = fill.into_iter().map(map).collect();
            self.emit_fill(
                &super::curves::fan_triangles(&world(transform, ring)),
                entity,
                frame,
                source.clone(),
                false,
            );
        }
    }

    fn draw_image_frame(
        &mut self,
        corners: [Vector3; 4],
        entity: &EntityType,
        frame: &Frame,
        transform: &Transform,
        source: EntityRef,
    ) {
        let mut points = corners.to_vec();
        points.push(corners[0]);
        self.emit_dedicated(&world(transform, points), entity, frame, source);
    }

    fn push_raster(
        &mut self,
        image: &crate::entities::RasterImage,
        decoded: &super::bitmap::RgbaImage,
        opacity: f32,
        transform: &Transform,
        source: EntityRef,
    ) {
        let clipped = image.clipping_enabled
            && image
                .flags
                .contains(crate::entities::ImageDisplayFlags::USE_CLIPPING_BOUNDARY)
            && image.clip_boundary.vertices.len() >= 2;
        let (triangles, uvs) = if clipped {
            clip_triangles(image)
        } else {
            quad_triangles(image.corners(), image.size.x as f32, image.size.y as f32)
        };
        self.scene.pictures.push(super::scene::Picture {
            triangles: world(transform, triangles),
            uvs,
            rgba: decoded.pixels.clone(),
            width: decoded.width,
            height: decoded.height,
            opacity,
            source,
        });
    }

    fn push_picture(
        &mut self,
        decoded: &super::bitmap::RgbaImage,
        corners: [Vector3; 4],
        opacity: f32,
        transform: &Transform,
        source: EntityRef,
    ) {
        let (triangles, uvs) = quad_triangles(corners, decoded.width as f32, decoded.height as f32);
        self.scene.pictures.push(super::scene::Picture {
            triangles: world(transform, triangles),
            uvs,
            rgba: decoded.pixels.clone(),
            width: decoded.width,
            height: decoded.height,
            opacity,
            source,
        });
    }

    fn image_file_path(&self, image: &crate::entities::RasterImage) -> String {
        if !image.file_path.is_empty() {
            return image.file_path.clone();
        }
        image
            .definition_handle
            .and_then(|handle| self.doc.objects.get(&handle))
            .and_then(|object| match object {
                crate::objects::ObjectType::ImageDefinition(definition) => {
                    Some(definition.file_name.clone())
                }
                _ => None,
            })
            .unwrap_or_default()
    }

    fn underlay_path(&self, underlay: &crate::entities::Underlay) -> String {
        self.doc
            .objects
            .get(&underlay.definition_handle)
            .and_then(|object| match object {
                crate::objects::ObjectType::UnderlayDefinition(definition) => {
                    Some(definition.file_path.clone())
                }
                _ => None,
            })
            .unwrap_or_default()
    }

    fn read_resource(&self, path: &str) -> Option<Vec<u8>> {
        if path.is_empty() {
            return None;
        }
        let direct = std::path::Path::new(path);
        if direct.is_file() {
            return std::fs::read(direct).ok();
        }
        for dir in self.resource_dirs {
            let candidate = dir.join(direct);
            if candidate.is_file() {
                return std::fs::read(candidate).ok();
            }
            if let Some(name) = direct.file_name() {
                let candidate = dir.join(name);
                if candidate.is_file() {
                    return std::fs::read(candidate).ok();
                }
            }
        }
        None
    }

    fn fallback(&mut self, entity: &EntityType, frame: &Frame, source: EntityRef) {
        let kind = kind_of(entity).to_string();
        let bounds = entity.as_entity().bounding_box();
        let local_corners = box_corners(bounds.min, bounds.max);
        let corners =
            std::array::from_fn(|index| self.active_transform.apply(local_corners[index]));
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

fn clip_viewport_depth(
    scene: &mut Scene,
    stroke_from: usize,
    fill_from: usize,
    glyph_from: usize,
    picture_from: usize,
    viewport: &crate::entities::Viewport,
    paper_transform: &Transform,
) {
    if !viewport.status.front_clipping && !viewport.status.back_clipping {
        return;
    }
    let linear = Matrix3::from_rows(
        [
            paper_transform.matrix.m[0][0],
            paper_transform.matrix.m[0][1],
            paper_transform.matrix.m[0][2],
        ],
        [
            paper_transform.matrix.m[1][0],
            paper_transform.matrix.m[1][1],
            paper_transform.matrix.m[1][2],
        ],
        [
            paper_transform.matrix.m[2][0],
            paper_transform.matrix.m[2][1],
            paper_transform.matrix.m[2][2],
        ],
    );
    let inverse = linear.inverse();
    let translation = Vector3::new(
        paper_transform.matrix.m[0][3],
        paper_transform.matrix.m[1][3],
        paper_transform.matrix.m[2][3],
    );
    let depth = |point: Vector3| {
        let model = inverse
            .as_ref()
            .map(|transform| *transform * (point - translation))
            .unwrap_or(point);
        (model - viewport.view_target).dot(&viewport.view_direction)
    };
    let visible = |point: Vector3| {
        let value = depth(point);
        (!viewport.status.front_clipping || value <= viewport.front_clip_z)
            && (!viewport.status.back_clipping || value >= viewport.back_clip_z)
    };
    let strokes = scene
        .strokes
        .split_off(stroke_from.min(scene.strokes.len()));
    scene.strokes.extend(
        strokes
            .into_iter()
            .filter(|stroke| stroke.points.iter().any(|point| visible(*point))),
    );
    let fills = scene.fills.split_off(fill_from.min(scene.fills.len()));
    scene.fills.extend(
        fills
            .into_iter()
            .filter(|fill| fill.triangles.iter().any(|point| visible(*point))),
    );
    let pictures = scene
        .pictures
        .split_off(picture_from.min(scene.pictures.len()));
    scene.pictures.extend(
        pictures
            .into_iter()
            .filter(|picture| picture.triangles.iter().any(|point| visible(*point))),
    );
    let glyphs = scene.glyphs.split_off(glyph_from.min(scene.glyphs.len()));
    scene
        .glyphs
        .extend(glyphs.into_iter().filter(|glyph| visible(glyph.anchor)));
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

fn transform_normals(transform: &Transform, normals: &[Vector3]) -> Vec<Vector3> {
    let linear = Matrix3::from_rows(
        [
            transform.matrix.m[0][0],
            transform.matrix.m[0][1],
            transform.matrix.m[0][2],
        ],
        [
            transform.matrix.m[1][0],
            transform.matrix.m[1][1],
            transform.matrix.m[1][2],
        ],
        [
            transform.matrix.m[2][0],
            transform.matrix.m[2][1],
            transform.matrix.m[2][2],
        ],
    );
    let normal_matrix = linear.inverse().map(|matrix| matrix.transpose());
    normals
        .iter()
        .map(|normal| {
            let turned = normal_matrix
                .as_ref()
                .map(|matrix| *matrix * *normal)
                .unwrap_or_else(|| transform.apply_rotation(*normal));
            let length = turned.length();
            if length > 1e-12 {
                turned * (1.0 / length)
            } else {
                *normal
            }
        })
        .collect()
}

fn apply_face_color(rgb: &mut [u8; 3], mode: super::visual_style::FaceColorMode) {
    match mode {
        super::visual_style::FaceColorMode::Object => {}
        super::visual_style::FaceColorMode::Mono => {
            let gray = (0.299 * rgb[0] as f64 + 0.587 * rgb[1] as f64 + 0.114 * rgb[2] as f64)
                .round()
                .clamp(0.0, 255.0) as u8;
            *rgb = [gray, gray, gray];
        }
        super::visual_style::FaceColorMode::Tint => {
            rgb[0] = ((rgb[0] as f64 * 0.7 + 255.0 * 0.3).round()).clamp(0.0, 255.0) as u8;
            rgb[1] = ((rgb[1] as f64 * 0.75 + 180.0 * 0.25).round()).clamp(0.0, 255.0) as u8;
        }
        super::visual_style::FaceColorMode::Desaturate => {
            let gray = (rgb[0] as u16 + rgb[1] as u16 + rgb[2] as u16) / 3;
            for channel in rgb.iter_mut() {
                *channel = ((*channel as u16 + gray) / 2) as u8;
            }
        }
        // The actual background is supplied by the GPU at render time; keep
        // the object color here rather than baking an arbitrary background.
        super::visual_style::FaceColorMode::Background => {}
    }
}

pub(crate) struct ArcAnchor {
    pub position: Vector3,
    pub rotation: f64,
}

pub(crate) fn arc_anchors(
    center: Vector3,
    radius: f64,
    start: f64,
    widths: &[f64],
    reverse: bool,
) -> Vec<ArcAnchor> {
    let radius = if radius.abs() < 1e-9 { 1.0 } else { radius };
    let direction = if reverse { -1.0 } else { 1.0 };
    let mut angle = start;
    let mut anchors = Vec::with_capacity(widths.len());
    for width in widths {
        let step = width / radius;
        let middle = angle + direction * step * 0.5;
        anchors.push(ArcAnchor {
            position: center + Vector3::new(middle.cos(), middle.sin(), 0.0) * radius.abs(),
            rotation: middle + direction * std::f64::consts::FRAC_PI_2,
        });
        angle += direction * step;
    }
    anchors
}

fn ole_corners(upper_left: Vector3, lower_right: Vector3) -> [Vector3; 4] {
    [
        Vector3::new(upper_left.x, lower_right.y, upper_left.z),
        Vector3::new(lower_right.x, lower_right.y, lower_right.z),
        Vector3::new(lower_right.x, upper_left.y, lower_right.z),
        upper_left,
    ]
}

fn quad_triangles(
    corners: [Vector3; 4],
    _width: f32,
    _height: f32,
) -> (Vec<Vector3>, Vec<[f32; 2]>) {
    let uvs = [
        [0.0, 1.0],
        [1.0, 1.0],
        [1.0, 0.0],
        [0.0, 1.0],
        [1.0, 0.0],
        [0.0, 0.0],
    ];
    let triangles = vec![
        corners[0], corners[1], corners[2], corners[0], corners[2], corners[3],
    ];
    (triangles, uvs.to_vec())
}

fn clip_triangles(image: &crate::entities::RasterImage) -> (Vec<Vector3>, Vec<[f32; 2]>) {
    let boundary = &image.clip_boundary;
    let polygon = if boundary.vertices.len() == 2 {
        let min = boundary.vertices[0];
        let max = boundary.vertices[1];
        vec![
            crate::types::Vector2::new(min.x.min(max.x), min.y.min(max.y)),
            crate::types::Vector2::new(max.x.max(min.x), min.y.min(max.y)),
            crate::types::Vector2::new(max.x.max(min.x), max.y.max(min.y)),
            crate::types::Vector2::new(min.x.min(max.x), max.y.max(min.y)),
        ]
    } else {
        boundary.vertices.clone()
    };
    let map = |point: crate::types::Vector2| {
        (
            image.insertion_point + image.u_vector * point.x + image.v_vector * point.y,
            [
                (point.x / image.size.x.max(1.0)) as f32,
                1.0 - (point.y / image.size.y.max(1.0)) as f32,
            ],
        )
    };
    let inverted = boundary.clip_mode == crate::entities::ClipMode::Inside;
    let outer = if inverted {
        vec![
            crate::types::Vector2::new(0.0, 0.0),
            crate::types::Vector2::new(image.size.x, 0.0),
            crate::types::Vector2::new(image.size.x, image.size.y),
            crate::types::Vector2::new(0.0, image.size.y),
        ]
    } else {
        Vec::new()
    };
    let mut flat = Vec::new();
    let mut located = Vec::new();
    for point in outer.iter().chain(polygon.iter()) {
        let (world, uv) = map(*point);
        flat.push(point.x);
        flat.push(point.y);
        located.push((world, uv));
    }
    let holes = if inverted {
        vec![outer.len()]
    } else {
        Vec::new()
    };
    let indices = earcutr::earcut(&flat, &holes, 2).unwrap_or_default();
    let mut triangles = Vec::new();
    let mut uvs = Vec::new();
    for index in indices {
        if let Some((point, uv)) = located.get(index) {
            triangles.push(*point);
            uvs.push(*uv);
        }
    }
    if triangles.is_empty() {
        return quad_triangles(image.corners(), image.size.x as f32, image.size.y as f32);
    }
    (triangles, uvs)
}

fn meta_point(bounds: (f64, f64, f64, f64), corners: [Vector3; 4], point: (f64, f64)) -> Vector3 {
    let (left, top, right, bottom) = bounds;
    let width = (right - left).abs().max(1e-9);
    let height = (bottom - top).abs().max(1e-9);
    let u = ((point.0 - left) / width).clamp(0.0, 1.0);
    let v = ((point.1 - top) / height).clamp(0.0, 1.0);
    let bottom_left = corners[0];
    let across = corners[1] - corners[0];
    let up = corners[3] - corners[0];
    bottom_left + across * u + up * (1.0 - v)
}

fn underlay_page(
    doc: &crate::document::CadDocument,
    underlay: &crate::entities::Underlay,
) -> usize {
    let name = doc
        .objects
        .get(&underlay.definition_handle)
        .and_then(|object| match object {
            crate::objects::ObjectType::UnderlayDefinition(definition) => {
                Some(definition.page_name.as_str())
            }
            _ => None,
        })
        .unwrap_or("");
    name.parse::<usize>()
        .ok()
        .filter(|page| *page > 0)
        .map(|page| page - 1)
        .unwrap_or(0)
}

fn place_underlay_local(underlay: &crate::entities::Underlay, x: f64, y: f64) -> Vector3 {
    underlay_point(underlay, (0.0, 0.0), (1.0, 1.0), (x, y))
}

fn apply_underlay_appearance(
    scene: &mut Scene,
    stroke_from: usize,
    fill_from: usize,
    underlay: &crate::entities::Underlay,
) {
    let opacity = (1.0 - underlay.fade as f32 / 100.0).clamp(0.0, 1.0);
    let monochrome = underlay
        .flags
        .contains(crate::entities::UnderlayDisplayFlags::MONOCHROME);
    let stroke_start = stroke_from.min(scene.strokes.len());
    for stroke in &mut scene.strokes[stroke_start..] {
        stroke.style.opacity *= opacity;
        if monochrome {
            let gray = (0.299 * stroke.style.rgb[0] as f64
                + 0.587 * stroke.style.rgb[1] as f64
                + 0.114 * stroke.style.rgb[2] as f64)
                .round()
                .clamp(0.0, 255.0) as u8;
            stroke.style.rgb = [gray, gray, gray];
        }
    }
    let fill_start = fill_from.min(scene.fills.len());
    for fill in &mut scene.fills[fill_start..] {
        fill.style.opacity *= opacity;
        if monochrome {
            let gray = (0.299 * fill.style.rgb[0] as f64
                + 0.587 * fill.style.rgb[1] as f64
                + 0.114 * fill.style.rgb[2] as f64)
                .round()
                .clamp(0.0, 255.0) as u8;
            fill.style.rgb = [gray, gray, gray];
        }
    }
}

fn infinite_reach(doc: &crate::document::CadDocument) -> f64 {
    let min = doc.header.model_space_extents_min;
    let max = doc.header.model_space_extents_max;
    if min.x < max.x && min.y < max.y {
        (max - min).length().max(1.0) * 1_000_000.0
    } else {
        1.0e9
    }
}

fn wipeout_triangles(wipeout: &crate::entities::Wipeout) -> Vec<Vector3> {
    let outer = wipeout.corners().to_vec();
    if !wipeout.clipping_enabled
        || !wipeout
            .flags
            .contains(crate::entities::WipeoutDisplayFlags::USE_CLIPPING_BOUNDARY)
        || wipeout.clip_boundary_vertices.len() < 2
    {
        return fan_triangles(&outer);
    }
    let boundary = if wipeout.clip_type == crate::entities::WipeoutClipType::Rectangular
        && wipeout.clip_boundary_vertices.len() >= 2
    {
        let min = wipeout.clip_boundary_vertices[0];
        let max = wipeout.clip_boundary_vertices[1];
        [
            crate::types::Vector2::new(min.x.min(max.x), min.y.min(max.y)),
            crate::types::Vector2::new(max.x.max(min.x), min.y.min(max.y)),
            crate::types::Vector2::new(max.x.max(min.x), max.y.max(min.y)),
            crate::types::Vector2::new(min.x.min(max.x), max.y.max(min.y)),
        ]
        .to_vec()
    } else {
        wipeout.clip_boundary_vertices.clone()
    };
    let to_world = |point: crate::types::Vector2| {
        wipeout.insertion_point
            + wipeout.u_vector * (point.x + wipeout.size.x * 0.5)
            + wipeout.v_vector * (wipeout.size.y * 0.5 - point.y)
    };
    let hole: Vec<Vector3> = boundary.into_iter().map(to_world).collect();
    if hole.len() < 3 {
        return fan_triangles(&outer);
    }
    let mut rings = outer.clone();
    let mut flat = Vec::with_capacity((outer.len() + hole.len()) * 2);
    for point in &outer {
        flat.extend([point.x, point.y]);
    }
    let hole_start = outer.len();
    rings.extend(hole.iter().copied());
    for point in &hole {
        flat.extend([point.x, point.y]);
    }
    let indices = if wipeout.clip_mode == crate::entities::WipeoutClipMode::Inside {
        earcutr::earcut(&flat, &[hole_start], 2).unwrap_or_default()
    } else {
        let mut clipped = Vec::new();
        let hole_flat: Vec<f64> = hole.iter().flat_map(|point| [point.x, point.y]).collect();
        let indices = earcutr::earcut(&hole_flat, &[], 2).unwrap_or_default();
        for index in indices {
            if let Some(point) = hole.get(index) {
                clipped.push(*point);
            }
        }
        return if clipped.is_empty() {
            fan_triangles(&outer)
        } else {
            clipped
        };
    };
    indices
        .into_iter()
        .filter_map(|index| rings.get(index).copied())
        .collect()
}

fn extent_is_real(min: Vector3, max: Vector3) -> bool {
    min.x <= max.x
        && min.y <= max.y
        && min.z <= max.z
        && (min.x < max.x || min.y < max.y || min.z < max.z)
        && min.x.is_finite()
        && max.x.is_finite()
        && min.y.is_finite()
        && max.y.is_finite()
}

fn matrix_from_rows(values: &[f64; 16]) -> crate::types::Matrix4 {
    let mut matrix = crate::types::Matrix4::zero();
    for row in 0..4 {
        matrix.m[row].copy_from_slice(&values[row * 4..row * 4 + 4]);
    }
    matrix
}

fn underlay_point(
    underlay: &crate::entities::Underlay,
    media_min: (f64, f64),
    _media_max: (f64, f64),
    point: (f64, f64),
) -> Vector3 {
    let local_x = (point.0 - media_min.0) * underlay.x_scale;
    let local_y = (point.1 - media_min.1) * underlay.y_scale;
    let (sin, cos) = underlay.rotation.sin_cos();
    underlay.insertion_point
        + Matrix3::arbitrary_axis(underlay.normal)
            * Vector3::new(
                cos * local_x - sin * local_y,
                sin * local_x + cos * local_y,
                0.0,
            )
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
    // Drawings often preserve the absolute path from the machine that wrote
    // them. When that machine's path is unavailable, the configured font
    // directories should still be able to resolve the same file by basename.
    let basename = std::path::Path::new(name)
        .file_name()
        .and_then(|file| file.to_str())
        .filter(|file| *file != name);
    for dir in dirs {
        for candidate in [Some(name), basename].into_iter().flatten() {
            for extension in extensions {
                let path = if extension.is_empty() {
                    dir.join(candidate)
                } else {
                    dir.join(format!("{candidate}.{extension}"))
                };
                if path.is_file() {
                    return Some(path);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::find_font_file;

    #[test]
    fn font_lookup_retries_an_absolute_path_by_basename() {
        let root = std::env::temp_dir().join(format!(
            "acadrust-render-font-lookup-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let expected = root.join("mini.shx");
        std::fs::write(&expected, b"font").unwrap();
        let missing_path = root.join("other-machine").join("mini.shx");
        let found = find_font_file(
            std::slice::from_ref(&root),
            &missing_path.to_string_lossy(),
            &["shp", "shx", ""],
        );
        assert_eq!(found, Some(expected));
        let _ = std::fs::remove_file(root.join("mini.shx"));
        let _ = std::fs::remove_dir(root);
    }
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
    if !filter.display_enabled {
        return None;
    }
    if filter.boundary_points.len() < 2 {
        return None;
    }
    let local = if filter.boundary_points.len() == 2 {
        super::clip::rectangle(filter.boundary_points[0], filter.boundary_points[1])
    } else {
        filter.boundary_points.clone()
    };
    let polygon: Vec<crate::types::Vector2> = local
        .into_iter()
        .map(|point| {
            let placed = filter
                .clip_bound_transform
                .transform_point(Vector3::new(point.x, point.y, 0.0));
            let world = parent.apply(placed);
            crate::types::Vector2::new(world.x, world.y)
        })
        .collect();
    if polygon.len() >= 3 {
        let area: f64 = polygon
            .iter()
            .enumerate()
            .map(|(index, point)| {
                let next = polygon[(index + 1) % polygon.len()];
                point.x * next.y - next.x * point.y
            })
            .sum();
        if area < 0.0 {
            let mut polygon = polygon;
            polygon.reverse();
            return Some(polygon);
        }
    }
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
    picture_from: usize,
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
        let mut normals = Vec::with_capacity(fill.triangles.len());
        for triangle in fill.triangles.chunks_exact(3) {
            let normal = (triangle[1] - triangle[0]).cross(&(triangle[2] - triangle[0]));
            let normal = if normal.length_squared() > 1e-18 {
                normal * (1.0 / normal.length())
            } else {
                Vector3::ZERO
            };
            normals.extend([normal; 3]);
        }
        fill.normals = normals;
    }
    let pictures = scene
        .pictures
        .split_off(picture_from.min(scene.pictures.len()));
    for mut picture in pictures {
        let mut triangles = Vec::new();
        let mut uvs = Vec::new();
        for (triangle, uv) in picture
            .triangles
            .chunks_exact(3)
            .zip(picture.uvs.chunks_exact(3))
        {
            let clipped = clip_picture_triangle(triangle, uv, polygon);
            for (points, coords) in clipped {
                triangles.extend(points);
                uvs.extend(coords);
            }
        }
        picture.triangles = triangles;
        picture.uvs = uvs;
        if !picture.triangles.is_empty() {
            scene.pictures.push(picture);
        }
    }
    let glyphs = scene.glyphs.split_off(glyph_from.min(scene.glyphs.len()));
    scene.glyphs.extend(glyphs.into_iter().filter(|glyph| {
        let width = glyph.height
            * glyph.width_factor.abs()
            * glyph.text.chars().count().max(1) as f64
            * 0.6;
        let corners = [
            glyph.anchor,
            glyph.anchor + glyph.axis_x * width,
            glyph.anchor + glyph.axis_x * width + glyph.axis_y * glyph.height,
            glyph.anchor + glyph.axis_y * glyph.height,
        ];
        corners.iter().any(|point| {
            super::clip::clip_segment(*point, *point + Vector3::new(1e-3, 0.0, 0.0), polygon)
                .is_some()
        })
    }));
}

fn clip_picture_triangle(
    triangle: &[Vector3],
    uv: &[[f32; 2]],
    polygon: &[crate::types::Vector2],
) -> Vec<(Vec<Vector3>, Vec<[f32; 2]>)> {
    if triangle.len() != 3 || uv.len() != 3 || polygon.len() < 3 {
        return Vec::new();
    }
    let mut output: Vec<(Vector3, [f32; 2])> =
        triangle.iter().copied().zip(uv.iter().copied()).collect();
    for index in 0..polygon.len() {
        if output.is_empty() {
            break;
        }
        let start = polygon[index];
        let end = polygon[(index + 1) % polygon.len()];
        let input = output;
        output = Vec::new();
        let mut previous = *input.last().unwrap();
        for current in input {
            let previous_inside = clip_inside(previous.0, start, end);
            let current_inside = clip_inside(current.0, start, end);
            if current_inside != previous_inside {
                output.push(clip_picture_intersection(previous, current, start, end));
            }
            if current_inside {
                output.push(current);
            }
            previous = current;
        }
    }
    let mut result = Vec::new();
    for index in 1..output.len().saturating_sub(1) {
        result.push((
            vec![output[0].0, output[index].0, output[index + 1].0],
            vec![output[0].1, output[index].1, output[index + 1].1],
        ));
    }
    result
}

fn clip_inside(point: Vector3, start: crate::types::Vector2, end: crate::types::Vector2) -> bool {
    (end.x - start.x) * (point.y - start.y) - (end.y - start.y) * (point.x - start.x) >= -1e-8
}

fn clip_picture_intersection(
    start: (Vector3, [f32; 2]),
    end: (Vector3, [f32; 2]),
    edge_start: crate::types::Vector2,
    edge_end: crate::types::Vector2,
) -> (Vector3, [f32; 2]) {
    let segment = end.0 - start.0;
    let edge = crate::types::Vector2::new(edge_end.x - edge_start.x, edge_end.y - edge_start.y);
    let denom = segment.x * edge.y - segment.y * edge.x;
    let t = if denom.abs() <= 1e-12 {
        0.0
    } else {
        ((edge_start.x - start.0.x) * edge.y - (edge_start.y - start.0.y) * edge.x) / denom
    }
    .clamp(0.0, 1.0);
    (
        start.0 + segment * t,
        [
            start.1[0] + (end.1[0] - start.1[0]) * t as f32,
            start.1[1] + (end.1[1] - start.1[1]) * t as f32,
        ],
    )
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

/// Kinds whose only picture is still an extents box. Phase 8 leaves this empty.
pub const FALLBACK_KINDS: &[&str] = &[];
