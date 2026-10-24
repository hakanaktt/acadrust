use std::f64::consts::FRAC_PI_2;

use crate::document::CadDocument;
use crate::entities::ViewportRenderMode;
use crate::entities::{
    Arc, BoundaryEdge, BoundaryPath, Circle, EntityType, Face3D, Hatch, Insert, InvisibleEdgeFlags,
    Line, LwPolyline, LwVertex, Point, Polyline, Polyline2D, Polyline3D, PolylineEdge, Solid,
    Spline, Text, Vertex2D,
};
use crate::objects::{ObjectType, SortEntitiesTable, VisualStyle};
use crate::tables::{BlockRecord, Layer, LineType, LineTypeElement, TableEntry};
use crate::types::{Color, Handle, LineWeight, Vector2, Vector3};

use super::visual_style::{
    EdgeMode, FaceColorMode, FaceShading, LightingModel, StandardVisualStyle, VisualStyleRequest,
};
use super::{compile, CompileOptions, DrawPath, View};

fn doc() -> CadDocument {
    CadDocument::new()
}

fn add_layer(doc: &mut CadDocument, name: &str, color: Color) {
    let mut layer = Layer::new(name);
    layer.color = color;
    layer.line_weight = LineWeight::Value(50);
    doc.layers.add(layer).unwrap();
}

fn add_dashed(doc: &mut CadDocument) {
    let mut linetype = LineType::new("DASHED");
    linetype.elements = vec![LineTypeElement::dash(1.0), LineTypeElement::space(1.0)];
    linetype.pattern_length = 2.0;
    doc.line_types.add(linetype).unwrap();
}

fn add_block_line(doc: &mut CadDocument, name: &str, line: Line) -> Handle {
    let mut record = BlockRecord::new(name);
    let handle = doc.allocate_handle();
    record.set_handle(handle);
    doc.block_records.add(record).unwrap();
    let mut line = line;
    line.common.owner_handle = handle;
    doc.add_entity(EntityType::Line(line)).unwrap();
    handle
}

#[test]
fn bylayer_color_comes_from_the_layer() {
    let mut doc = doc();
    add_layer(&mut doc, "Walls", Color::RED);
    let mut line = Line::from_coords(0.0, 0.0, 0.0, 10.0, 0.0, 0.0);
    line.common.layer = "Walls".to_string();
    doc.add_entity(EntityType::Line(line)).unwrap();

    let scene = compile(&doc, &CompileOptions::default());
    assert_eq!(scene.strokes.len(), 1);
    assert_eq!(scene.strokes[0].style.rgb, [255, 0, 0]);
    assert_eq!(scene.strokes[0].style.layer, "Walls");
    assert!(scene.strokes[0].style.cosmetic);
    assert!(scene.fallbacks.is_empty());
}

#[test]
fn byblock_and_layer_zero_follow_the_insert() {
    let mut doc = doc();
    add_layer(&mut doc, "Walls", Color::GREEN);
    let mut line = Line::from_coords(0.0, 0.0, 0.0, 10.0, 0.0, 0.0);
    line.common.layer = "0".to_string();
    line.common.color = Color::ByBlock;
    add_block_line(&mut doc, "WIDGET", line);

    let mut insert = Insert::new("WIDGET", Vector3::new(5.0, 5.0, 0.0));
    insert.rotation = FRAC_PI_2;
    insert.common.layer = "Walls".to_string();
    insert.common.color = Color::RED;
    doc.add_entity(EntityType::Insert(insert)).unwrap();

    let scene = compile(&doc, &CompileOptions::default());
    assert_eq!(scene.strokes.len(), 1);
    let stroke = &scene.strokes[0];
    assert_eq!(stroke.style.rgb, [255, 0, 0]);
    assert_eq!(stroke.style.layer, "Walls");
    assert_eq!(stroke.path, DrawPath::Dedicated);
    let start = stroke.points[0];
    let end = stroke.points[1];
    assert!((start.x - 5.0).abs() < 1e-6 && (start.y - 5.0).abs() < 1e-6);
    assert!((end.x - 5.0).abs() < 1e-6 && (end.y - 15.0).abs() < 1e-6);
}

#[test]
fn linetype_scale_fits_alignment_a() {
    let mut doc = doc();
    add_dashed(&mut doc);
    doc.header.linetype_scale = 1.0;
    let mut line = Line::from_coords(0.0, 0.0, 0.0, 4.0, 0.0, 0.0);
    line.common.linetype = "DASHED".to_string();
    doc.add_entity(EntityType::Line(line)).unwrap();

    let scene = compile(&doc, &CompileOptions::default());
    let dashes: Vec<_> = scene.strokes.iter().filter(|stroke| !stroke.dot).collect();
    assert_eq!(dashes.len(), 2);
    assert!((dashes[0].points[0].x - 0.0).abs() < 1e-6);
    assert!((dashes[0].points[1].x - 1.0).abs() < 1e-6);
    assert!((dashes[1].points[0].x - 2.0).abs() < 1e-6);
    assert!((dashes[1].points[1].x - 3.0).abs() < 1e-6);
}

#[test]
fn plinegen_continues_the_pattern_across_vertices() {
    let mut doc = doc();
    add_dashed(&mut doc);
    let mut restarted = LwPolyline::new();
    restarted.common.linetype = "DASHED".to_string();
    restarted.vertices = vec![
        LwVertex::new(Vector2::new(0.0, 0.0)),
        LwVertex::new(Vector2::new(2.0, 0.0)),
        LwVertex::new(Vector2::new(4.0, 0.0)),
    ];
    let mut continuous = restarted.clone();
    continuous.plinegen = true;
    doc.add_entity(EntityType::LwPolyline(restarted)).unwrap();
    doc.add_entity(EntityType::LwPolyline(continuous)).unwrap();

    let scene = compile(&doc, &CompileOptions::default());
    let dedicated: Vec<_> = scene
        .strokes
        .iter()
        .filter(|stroke| stroke.path == DrawPath::Dedicated && !stroke.dot)
        .collect();
    // Each 2-unit segment, fitted on its own, contributes one dash. The
    // generated polyline covers length 4 with the same pattern and also
    // contributes two dashes. Four dashes in total, against two per segment
    // when generation is off (two segments, one dash each, plus two more).
    assert_eq!(dedicated.len(), 4);
    assert!(scene.fallbacks.is_empty());
}

#[test]
fn default_model_lineweight_matches_the_chart() {
    let scale = super::DEFAULT_MODEL_LINEWEIGHT_PX_PER_MM;
    assert_eq!(scale, 8.0);
    assert_eq!(super::model_lineweight_pixels(0.00, scale), 1);
    assert_eq!(super::model_lineweight_pixels(0.13, scale), 1);
    assert_eq!(super::model_lineweight_pixels(0.25, scale), 2);
    assert_eq!(super::model_lineweight_pixels(0.50, scale), 4);
    assert_eq!(super::model_lineweight_pixels(1.00, scale), 8);
    assert_eq!(super::model_lineweight_pixels(2.11, scale), 17);
}

#[test]
fn lineweight_display_uses_millimetres() {
    let mut doc = doc();
    doc.header.lineweight_display = true;
    let mut line = Line::from_coords(0.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    line.common.line_weight = LineWeight::Value(25);
    doc.add_entity(EntityType::Line(line)).unwrap();

    let scene = compile(&doc, &CompileOptions::default());
    assert!(!scene.strokes[0].style.cosmetic);
    assert!((scene.strokes[0].style.lineweight_mm - 0.25).abs() < 1e-9);
}

#[test]
fn bulged_polyline_is_more_than_a_chord() {
    let mut doc = doc();
    let mut poly = LwPolyline::new();
    poly.vertices = vec![
        LwVertex::with_bulge(Vector2::new(0.0, 0.0), 1.0),
        LwVertex::new(Vector2::new(10.0, 0.0)),
    ];
    doc.add_entity(EntityType::LwPolyline(poly)).unwrap();

    let scene = compile(&doc, &CompileOptions::default());
    assert_eq!(scene.strokes.len(), 1);
    assert!(scene.strokes[0].points.len() > 2);
    let first = scene.strokes[0].points.first().unwrap();
    let last = scene.strokes[0].points.last().unwrap();
    assert!(first.x.abs() < 1e-6 && first.y.abs() < 1e-6);
    assert!((last.x - 10.0).abs() < 1e-5 && last.y.abs() < 1e-5);
}

#[test]
fn frozen_layer_is_skipped() {
    let mut doc = doc();
    add_layer(&mut doc, "Hidden", Color::RED);
    doc.layers.get_mut("Hidden").unwrap().flags.frozen = true;
    let mut line = Line::from_coords(0.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    line.common.layer = "Hidden".to_string();
    doc.add_entity(EntityType::Line(line)).unwrap();

    let scene = compile(&doc, &CompileOptions::default());
    assert!(scene.strokes.is_empty());
    assert_eq!(scene.skipped.len(), 1);
    assert!(scene.skipped[0].reason.contains("frozen"));
}

#[test]
fn unfinished_entities_stay_on_the_fallback_list() {
    let mut doc = doc();
    doc.add_entity(EntityType::Ray(crate::entities::Ray::new(
        Vector3::ZERO,
        Vector3::new(1.0, 0.0, 0.0),
    )))
    .unwrap();
    let scene = compile(&doc, &CompileOptions::default());
    assert_eq!(scene.fallback_kinds(), vec!["Ray"]);
    assert!(scene
        .strokes
        .iter()
        .all(|stroke| stroke.path == DrawPath::Fallback));
    assert!(super::FALLBACK_KINDS.contains(&"Solid3D"));
    assert!(super::FALLBACK_KINDS.contains(&"Region"));
    assert!(!super::FALLBACK_KINDS.contains(&"Circle"));
}

#[test]
fn every_standard_visual_style_has_a_recipe() {
    assert_eq!(StandardVisualStyle::ALL.len(), 14);
    for kind in StandardVisualStyle::ALL {
        let resolved = super::ResolvedVisualStyle::from_standard(kind);
        assert_eq!(resolved.kind, kind);
        let again = StandardVisualStyle::from_name(kind.as_str()).unwrap();
        assert_eq!(again, kind);
    }
    let hidden = super::ResolvedVisualStyle::from_standard(StandardVisualStyle::Hidden);
    assert_eq!(hidden.faces, FaceShading::None);
    assert!(hidden.hidden_removal);
    let xray = super::ResolvedVisualStyle::from_standard(StandardVisualStyle::XRay);
    assert!(xray.face_opacity < 1.0);
    assert_eq!(xray.edges, EdgeMode::Facet);
    let sketchy = super::ResolvedVisualStyle::from_standard(StandardVisualStyle::Sketchy);
    assert!(sketchy.sketchy);
    let conceptual = super::ResolvedVisualStyle::from_standard(StandardVisualStyle::Conceptual);
    assert_eq!(conceptual.lighting, LightingModel::Gooch);
    assert_eq!(conceptual.face_color, FaceColorMode::Tint);
    let gray = super::ResolvedVisualStyle::from_standard(StandardVisualStyle::ShadesOfGray);
    assert_eq!(gray.face_color, FaceColorMode::Mono);
    let realistic = super::ResolvedVisualStyle::from_standard(StandardVisualStyle::Realistic);
    assert_eq!(realistic.faces, FaceShading::Gouraud);
    assert_eq!(realistic.edges, EdgeMode::None);
}

#[test]
fn render_mode_and_named_style_both_resolve() {
    for mode in [
        ViewportRenderMode::Wireframe2D,
        ViewportRenderMode::Wireframe3D,
        ViewportRenderMode::HiddenLine,
        ViewportRenderMode::FlatShaded,
        ViewportRenderMode::GouraudShaded,
        ViewportRenderMode::FlatShadedWithEdges,
        ViewportRenderMode::GouraudShadedWithEdges,
    ] {
        let resolved = super::ResolvedVisualStyle::from_render_mode(mode);
        assert!(!resolved.name.is_empty());
    }

    let mut doc = doc();
    let mut style = VisualStyle::new();
    style.description = "X-Ray".to_string();
    style.handle = doc.allocate_handle();
    let handle = style.handle;
    doc.objects.insert(handle, ObjectType::VisualStyle(style));
    doc.vports.get_mut("*Active").unwrap().visual_style_handle = handle;

    let scene = compile(&doc, &CompileOptions::default());
    assert_eq!(scene.visual_style.kind, StandardVisualStyle::XRay);

    let mut custom = VisualStyle::new();
    custom.description = "SiteShade".to_string();
    custom.face_lighting_model = 2;
    custom.edge_model = 2;
    custom.face_color_mode = 4;
    let resolved = super::ResolvedVisualStyle::from_object(&custom);
    assert_eq!(resolved.kind, StandardVisualStyle::Custom);
    assert_eq!(resolved.faces, FaceShading::Gouraud);
    assert_eq!(resolved.edges, EdgeMode::Facet);
    assert_eq!(resolved.face_color, FaceColorMode::Mono);

    let mut options = CompileOptions::default();
    options.visual_style = VisualStyleRequest::RenderMode(ViewportRenderMode::HiddenLine);
    let scene = compile(&doc, &options);
    assert_eq!(scene.visual_style.kind, StandardVisualStyle::Hidden);
}

#[test]
fn shaded_styles_drop_linetype_patterns() {
    let mut doc = doc();
    add_dashed(&mut doc);
    doc.vports.get_mut("*Active").unwrap().render_mode = ViewportRenderMode::GouraudShaded;
    let mut line = Line::from_coords(0.0, 0.0, 0.0, 4.0, 0.0, 0.0);
    line.common.linetype = "DASHED".to_string();
    doc.add_entity(EntityType::Line(line)).unwrap();

    let scene = compile(&doc, &CompileOptions::default());
    assert_eq!(scene.visual_style.kind, StandardVisualStyle::GouraudShaded);
    assert_eq!(scene.strokes.len(), 1);
    assert!((scene.strokes[0].points[1].x - 4.0).abs() < 1e-6);
}

#[test]
fn offscreen_frame_draws_a_red_line() {
    let mut doc = doc();
    let mut line = Line::from_coords(0.0, 0.0, 0.0, 100.0, 0.0, 0.0);
    line.common.color = Color::RED;
    doc.add_entity(EntityType::Line(line)).unwrap();
    let view = View {
        target: Vector3::new(50.0, 0.0, 0.0),
        height: 100.0,
        ..View::default()
    };
    let image = pollster::block_on(super::render_rgba(
        &doc,
        &view,
        64,
        64,
        &CompileOptions::default(),
    ));
    let image = match image {
        Err(super::RenderError::NoAdapter) => return,
        Err(error) => panic!("{error}"),
        Ok(image) => image,
    };
    let mut red_pixels = 0u32;
    for pixel in image.pixels.chunks_exact(4) {
        if pixel[0] > 80 && pixel[1] < 80 {
            red_pixels += 1;
        }
    }
    assert!(
        red_pixels >= 16,
        "expected a red stroke across the frame, found {red_pixels} red pixels"
    );
    assert_eq!(&image.pixels[0..3], &[33, 40, 48]);
}

fn red_rows(image: &super::RgbaImage) -> u32 {
    let x = image.width / 2;
    let mut rows = 0;
    for y in 0..image.height {
        let index = ((y * image.width + x) * 4) as usize;
        if image.pixels[index] > 180 && image.pixels[index + 1] < 80 {
            rows += 1;
        }
    }
    rows
}

#[test]
fn model_lineweight_keeps_its_pixel_width_when_zooming() {
    let mut doc = doc();
    doc.header.lineweight_display = true;
    let mut line = Line::from_coords(-50.0, 0.0, 0.0, 50.0, 0.0, 0.0);
    line.common.color = Color::RED;
    line.common.line_weight = LineWeight::Value(50);
    doc.add_entity(EntityType::Line(line)).unwrap();
    let config = super::RendererConfig {
        samples: 1,
        ..super::RendererConfig::default()
    };
    let render_at = |height: f64| {
        let view = View {
            target: Vector3::ZERO,
            height,
            ..View::default()
        };
        pollster::block_on(super::render_rgba_with(
            &doc,
            &view,
            80,
            40,
            &CompileOptions::default(),
            &config,
        ))
    };
    let near = match render_at(20.0) {
        Err(super::RenderError::NoAdapter) => return,
        Err(error) => panic!("{error}"),
        Ok(image) => image,
    };
    let far = render_at(200.0).expect("adapter already opened");
    let near_rows = red_rows(&near);
    let far_rows = red_rows(&far);
    assert_eq!(near_rows, far_rows, "zoom changed a cosmetic lineweight");
    assert!(
        (3..=5).contains(&near_rows),
        "0.50 mm at 8 px/mm should cover about 4 rows, found {near_rows}"
    );
}

#[test]
fn wide_lwpolyline_becomes_a_fill() {
    let mut doc = doc();
    doc.header.fill_mode = true;
    let mut poly = LwPolyline::new();
    poly.constant_width = 2.0;
    poly.vertices = vec![
        LwVertex::new(Vector2::new(0.0, 0.0)),
        LwVertex::new(Vector2::new(10.0, 0.0)),
    ];
    doc.add_entity(EntityType::LwPolyline(poly)).unwrap();
    let scene = compile(&doc, &CompileOptions::default());
    assert!(scene.fallbacks.is_empty());
    assert_eq!(scene.fills.len(), 1);
    assert_eq!(scene.fills[0].triangles.len(), 6);
    let ys: Vec<f64> = scene.fills[0]
        .triangles
        .iter()
        .map(|point| point.y)
        .collect();
    assert!(ys.iter().any(|y| (*y - 1.0).abs() < 1e-6));
    assert!(ys.iter().any(|y| (*y + 1.0).abs() < 1e-6));
}

#[test]
fn circle_and_arc_sample_their_endpoints() {
    let mut doc = doc();
    doc.add_entity(EntityType::Circle(Circle::from_coords(0.0, 0.0, 0.0, 5.0)))
        .unwrap();
    doc.add_entity(EntityType::Arc(Arc::from_center_radius_angles(
        Vector3::new(20.0, 0.0, 0.0),
        5.0,
        0.0,
        std::f64::consts::FRAC_PI_2,
    )))
    .unwrap();
    let scene = compile(&doc, &CompileOptions::default());
    assert!(scene.fallbacks.is_empty());
    let circle = &scene.strokes[0];
    assert!(circle.points.len() > 8);
    let first = circle.points[0];
    let last = circle.points.last().unwrap();
    assert!((first.x - 5.0).abs() < 1e-6 && first.y.abs() < 1e-6);
    assert!((last.x - first.x).abs() < 1e-6 && (last.y - first.y).abs() < 1e-6);
    let arc = &scene.strokes[1];
    let start = arc.points[0];
    let end = arc.points.last().unwrap();
    assert!((start.x - 25.0).abs() < 1e-5 && start.y.abs() < 1e-5);
    assert!((end.x - 20.0).abs() < 1e-5 && (end.y - 5.0).abs() < 1e-5);
}

#[test]
fn point_plus_and_solid_fill() {
    let mut doc = doc();
    doc.header.point_display_mode = 2;
    doc.header.point_display_size = 4.0;
    doc.header.fill_mode = true;
    doc.add_entity(EntityType::Point(Point::from_coords(0.0, 0.0, 0.0)))
        .unwrap();
    doc.add_entity(EntityType::Solid(Solid::triangle(
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(10.0, 0.0, 0.0),
        Vector3::new(0.0, 10.0, 0.0),
    )))
    .unwrap();
    let mut face = Face3D::triangle(
        Vector3::new(0.0, 0.0, 1.0),
        Vector3::new(1.0, 0.0, 1.0),
        Vector3::new(0.0, 1.0, 1.0),
    );
    face.invisible_edges = InvisibleEdgeFlags::FIRST;
    doc.add_entity(EntityType::Face3D(face)).unwrap();

    let scene = compile(&doc, &CompileOptions::default());
    assert!(scene.fallbacks.is_empty());
    assert_eq!(
        scene
            .strokes
            .iter()
            .filter(|stroke| stroke.points.len() == 2)
            .count(),
        2 + 2
    );
    assert_eq!(scene.fills.len(), 1);
    assert_eq!(scene.fills[0].triangles.len(), 3);
}

#[test]
fn polyline_spline_and_solid_hatch() {
    let mut doc = doc();
    let mut heavy = Polyline2D::new();
    heavy.vertices = vec![
        Vertex2D::new(Vector3::new(0.0, 0.0, 0.0)),
        Vertex2D::new(Vector3::new(5.0, 0.0, 0.0)),
    ];
    doc.add_entity(EntityType::Polyline2D(heavy)).unwrap();
    doc.add_entity(EntityType::Polyline(Polyline::from_points(vec![
        Vector3::new(0.0, 1.0, 0.0),
        Vector3::new(4.0, 1.0, 2.0),
    ])))
    .unwrap();
    let mut poly3 = Polyline3D::new();
    poly3.add_vertex(Vector3::new(0.0, 2.0, 0.0));
    poly3.add_vertex(Vector3::new(3.0, 2.0, 1.0));
    doc.add_entity(EntityType::Polyline3D(poly3)).unwrap();
    doc.add_entity(EntityType::Spline(Spline::from_control_points(
        3,
        vec![
            Vector3::new(0.0, 3.0, 0.0),
            Vector3::new(1.0, 4.0, 0.0),
            Vector3::new(2.0, 3.0, 0.0),
            Vector3::new(3.0, 4.0, 0.0),
        ],
    )))
    .unwrap();

    let mut hatch = Hatch::solid();
    let mut path = BoundaryPath::new();
    path.add_edge(BoundaryEdge::Polyline(PolylineEdge::new(
        vec![
            Vector2::new(0.0, 0.0),
            Vector2::new(10.0, 0.0),
            Vector2::new(0.0, 10.0),
        ],
        true,
    )));
    hatch.paths.push(path);
    doc.add_entity(EntityType::Hatch(Box::new(hatch))).unwrap();

    let scene = compile(&doc, &CompileOptions::default());
    assert!(scene.fallbacks.is_empty(), "{:?}", scene.fallback_kinds());
    assert!(scene.strokes.len() >= 4);
    assert_eq!(scene.fills.len(), 1);
    assert_eq!(scene.fills[0].triangles.len(), 3);
}

fn ring(points: &[(f64, f64)]) -> BoundaryPath {
    let mut path = BoundaryPath::new();
    path.add_edge(BoundaryEdge::Polyline(PolylineEdge::new(
        points.iter().map(|(x, y)| Vector2::new(*x, *y)).collect(),
        true,
    )));
    path
}

fn covered(fill: &super::Fill, x: f64, y: f64) -> bool {
    fill.triangles.chunks_exact(3).any(|triangle| {
        let a = triangle[0];
        let b = triangle[1];
        let c = triangle[2];
        let ab = (b.x - a.x) * (y - a.y) - (b.y - a.y) * (x - a.x);
        let bc = (c.x - b.x) * (y - b.y) - (c.y - b.y) * (x - b.x);
        let ca = (a.x - c.x) * (y - c.y) - (a.y - c.y) * (x - c.x);
        (ab >= -1e-6 && bc >= -1e-6 && ca >= -1e-6) || (ab <= 1e-6 && bc <= 1e-6 && ca <= 1e-6)
    })
}

#[test]
fn solid_hatch_leaves_a_hole_and_keeps_an_island() {
    let mut doc = doc();
    let mut hatch = Hatch::solid();
    hatch.style = crate::entities::HatchStyleType::Normal;
    hatch.paths = vec![
        ring(&[(0.0, 0.0), (30.0, 0.0), (30.0, 30.0), (0.0, 30.0)]),
        ring(&[(5.0, 5.0), (25.0, 5.0), (25.0, 25.0), (5.0, 25.0)]),
        ring(&[(10.0, 10.0), (15.0, 10.0), (15.0, 15.0), (10.0, 15.0)]),
    ];
    doc.add_entity(EntityType::Hatch(Box::new(hatch))).unwrap();
    let scene = compile(&doc, &CompileOptions::default());
    assert!(scene.fallbacks.is_empty(), "{:?}", scene.fallback_kinds());
    let fill = &scene.fills[0];
    assert!(covered(fill, 1.0, 1.0));
    assert!(!covered(fill, 7.0, 7.0));
    assert!(covered(fill, 12.0, 12.0));
}

#[test]
fn linear_gradient_runs_from_red_to_blue() {
    let mut doc = doc();
    let mut hatch = Hatch::solid();
    hatch.gradient_color.enabled = true;
    hatch.gradient_color.name = "LINEAR".to_string();
    hatch.gradient_color.angle = 0.0;
    hatch.gradient_color.add_color(0.0, Color::RED);
    hatch
        .gradient_color
        .add_color(1.0, Color::from_rgb(0, 0, 255));
    hatch.paths = vec![ring(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)])];
    doc.add_entity(EntityType::Hatch(Box::new(hatch))).unwrap();
    let scene = compile(&doc, &CompileOptions::default());
    let fill = &scene.fills[0];
    assert_eq!(fill.vertex_rgb.len(), fill.triangles.len());
    let left = fill
        .triangles
        .iter()
        .zip(&fill.vertex_rgb)
        .find(|(point, _)| point.x < 1.0)
        .unwrap()
        .1;
    let right = fill
        .triangles
        .iter()
        .zip(&fill.vertex_rgb)
        .find(|(point, _)| point.x > 9.0)
        .unwrap()
        .1;
    assert!(left[0] > left[2]);
    assert!(right[2] > right[0]);
}

#[test]
fn sortents_draws_the_lower_handle_first() {
    let mut doc = doc();
    let mut first = Line::from_coords(0.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    first.common.color = Color::RED;
    let mut second = Line::from_coords(0.0, 2.0, 0.0, 1.0, 2.0, 0.0);
    second.common.color = Color::from_rgb(0, 0, 255);
    let first_handle = doc.add_entity(EntityType::Line(first)).unwrap();
    let second_handle = doc.add_entity(EntityType::Line(second)).unwrap();
    let block = doc.block_records.get("*Model_Space").unwrap().handle;
    let mut table = SortEntitiesTable::for_block(block);
    table.handle = doc.allocate_handle();
    table.add_entry(second_handle, Handle::new(1));
    table.add_entry(first_handle, Handle::new(20));
    doc.objects
        .insert(table.handle, ObjectType::SortEntitiesTable(table));

    let scene = compile(&doc, &CompileOptions::default());
    assert_eq!(scene.strokes[0].source.handle, second_handle);
    assert_eq!(scene.strokes[1].source.handle, first_handle);
}

#[test]
fn shp_text_draws_the_letter_strokes() {
    let dir = std::env::temp_dir().join("acadrust-render-font");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("mini.shp"),
        "*0,4,TEST\n6,2,0,0\n*65,12,A\n1,8,(4,6),8,(4,-6),2,8,(2,0),0\n",
    )
    .unwrap();
    let mut doc = doc();
    doc.text_styles.get_mut("Standard").unwrap().font_file = "mini".to_string();
    let mut text = Text::with_value("A", Vector3::ZERO).with_height(6.0);
    text.style = "Standard".to_string();
    doc.add_entity(EntityType::Text(text)).unwrap();
    let mut options = CompileOptions::default();
    options.font_dirs.push(dir);
    let scene = compile(&doc, &options);
    assert!(scene.fallbacks.is_empty());
    assert!(scene.skipped.is_empty(), "{:?}", scene.skipped);
    assert!(scene
        .strokes
        .iter()
        .flat_map(|stroke| &stroke.points)
        .any(|point| (point.y - 6.0).abs() < 1e-4));
}

#[test]
fn missing_font_is_reported_and_not_invented() {
    let mut doc = doc();
    doc.add_entity(EntityType::Text(Text::with_value("Hi", Vector3::ZERO)))
        .unwrap();
    let scene = compile(&doc, &CompileOptions::default());
    assert!(scene.strokes.is_empty());
    assert!(scene.skipped[0].reason.contains("font"));
}

#[test]
fn aligned_dimension_explodes_into_strokes() {
    let mut doc = doc();
    let dimension =
        crate::entities::DimensionAligned::new(Vector3::ZERO, Vector3::new(20.0, 0.0, 0.0));
    doc.add_entity(EntityType::Dimension(Box::new(
        crate::entities::Dimension::Aligned(dimension),
    )))
    .unwrap();
    let scene = compile(&doc, &CompileOptions::default());
    assert!(!scene.fallback_kinds().contains(&"Dimension"));
    assert!(!scene.strokes.is_empty());
}
