use std::f64::consts::FRAC_PI_2;

use crate::document::CadDocument;
use crate::entities::ViewportRenderMode;
use crate::entities::{
    Arc, AttributeDefinition, BoundaryEdge, BoundaryPath, Circle, EntityCommon, EntityType,
    ExtendedEntity, ExtendedEntityData, Face3D, Hatch, Insert, InvisibleEdgeFlags, Line,
    LwPolyline, LwVertex, MText, Point, Polyline, Polyline2D, Polyline3D, PolylineEdge,
    SectionObjectData, SectionSymbol, SectionSymbolPoint, SectionViewStyle, Solid, Spline, Text,
    Vertex2D, ViewBorder, Viewport,
};
use crate::objects::{ObjectType, SortEntitiesTable, VisualStyle};
use crate::tables::{BlockRecord, Layer, LineType, LineTypeElement, TableEntry, TextStyle};
use crate::types::{Color, Handle, LineWeight, Vector2, Vector3};

use super::visual_style::{
    EdgeMode, FaceColorMode, FaceShading, LightingModel, StandardVisualStyle, VisualStyleRequest,
};
use super::{compile, CompileOptions, Curve, DisplaySpace, DrawPath, SolidStyle, View};

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
    assert!(
        matches!(scene.strokes[0].curves.as_slice(), [Curve::Elliptic { .. }]),
        "a bulge is an arc, not a polyline"
    );
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
fn fallback_list_is_empty_and_a_ray_is_a_line() {
    assert!(super::FALLBACK_KINDS.is_empty());
    let mut doc = doc();
    doc.add_entity(EntityType::Ray(crate::entities::Ray::new(
        Vector3::new(2.0, 3.0, 0.0),
        Vector3::new(1.0, 0.0, 0.0),
    )))
    .unwrap();
    let scene = compile(&doc, &CompileOptions::default());
    assert!(scene.fallbacks.is_empty());
    assert_eq!(scene.strokes.len(), 1);
    assert_eq!(scene.strokes[0].path, DrawPath::Infinite);
    assert!((scene.strokes[0].points[0].x - 2.0).abs() < 1e-9);
    assert!((scene.strokes[0].points[0].y - 3.0).abs() < 1e-9);
    assert!(scene.strokes[0].points[1].x > 2.0);
    assert!((scene.strokes[0].points[1].y - 3.0).abs() < 1e-9);
}

#[test]
fn infinite_entities_do_not_dominate_zoom_extents() {
    let mut doc = doc();
    doc.add_entity(EntityType::Line(Line::from_coords(
        0.0, 0.0, 0.0, 10.0, 0.0, 0.0,
    )))
    .unwrap();
    doc.add_entity(EntityType::Ray(crate::entities::Ray::new(
        Vector3::new(0.0, 20.0, 0.0),
        Vector3::new(1.0, 0.0, 0.0),
    )))
    .unwrap();
    let scene = compile(&doc, &CompileOptions::default());
    let view = View::zoom_extents(&scene, 800, 600);
    assert!(
        view.height < 20.0,
        "synthetic ray endpoint dominated view: {}",
        view.height
    );
    assert!((view.target.x - 5.0).abs() < 1e-6);
    assert!((view.target.y).abs() < 1e-6);
}

#[test]
fn standard_views_match_the_paper_viewport_axes() {
    let axes = |direction: Vector3| {
        View {
            direction,
            ..View::default()
        }
        .basis()
    };
    let (right, up, _) = axes(Vector3::UNIT_Z);
    assert!(right.distance(&Vector3::UNIT_X) < 1e-9);
    assert!(up.distance(&Vector3::UNIT_Y) < 1e-9);
    let (right, up, _) = axes(-Vector3::UNIT_Y);
    assert!(right.distance(&Vector3::UNIT_X) < 1e-9);
    assert!(up.distance(&Vector3::UNIT_Z) < 1e-9);
    let (right, up, _) = axes(Vector3::UNIT_X);
    assert!(right.distance(&Vector3::UNIT_Y) < 1e-9);
    assert!(up.distance(&Vector3::UNIT_Z) < 1e-9);
}

#[test]
fn orbit_keeps_an_orthonormal_frame() {
    let mut view = View {
        direction: -Vector3::UNIT_Y,
        twist: 0.2,
        ..View::default()
    };
    for _ in 0..24 {
        view.orbit(0.17, -0.11);
        let (right, up, direction) = view.basis();
        assert!((right.length() - 1.0).abs() < 1e-9);
        assert!((up.length() - 1.0).abs() < 1e-9);
        assert!((direction.length() - 1.0).abs() < 1e-9);
        assert!(right.dot(&up).abs() < 1e-9);
        assert!(right.dot(&direction).abs() < 1e-9);
        assert!(up.dot(&direction).abs() < 1e-9);
        let again = View {
            direction: view.direction,
            twist: view.twist,
            ..View::default()
        }
        .basis();
        assert!(again.0.distance(&right) < 1e-9);
    }
}

#[test]
fn frame_fits_the_current_view_plane() {
    let mut doc = doc();
    doc.add_entity(EntityType::Line(Line::from_coords(
        0.0, 0.0, 0.0, 0.0, 0.0, 100.0,
    )))
    .unwrap();
    let scene = compile(&doc, &CompileOptions::default());
    let mut view = View {
        direction: -Vector3::UNIT_Y,
        ..View::default()
    };
    view.frame(&scene, 100, 100);
    assert!((view.target.z - 50.0).abs() < 1e-6);
    assert!(
        view.height > 100.0 && view.height < 120.0,
        "{}",
        view.height
    );
}

#[test]
fn front_view_draws_a_vertical_world_z_line() {
    let mut doc = doc();
    doc.header.lineweight_display = true;
    let mut line = Line::from_coords(0.0, 0.0, 0.0, 0.0, 0.0, 100.0);
    line.common.color = Color::RED;
    line.common.line_weight = LineWeight::Value(50);
    doc.add_entity(EntityType::Line(line)).unwrap();
    let image = match pollster::block_on(super::render_rgba_with(
        &doc,
        &View {
            target: Vector3::new(0.0, 0.0, 50.0),
            height: 150.0,
            direction: -Vector3::UNIT_Y,
            ..View::default()
        },
        64,
        64,
        &CompileOptions::default(),
        &super::RendererConfig {
            samples: 1,
            ..super::RendererConfig::default()
        },
    )) {
        Err(super::RenderError::NoAdapter) => return,
        Err(error) => panic!("{error}"),
        Ok(image) => image,
    };
    let x = image.width / 2;
    let mut red = 0u32;
    for y in 0..image.height {
        let index = ((y * image.width + x) * 4) as usize;
        if image.pixels[index] > 80 && image.pixels[index + 1] < 80 {
            red += 1;
        }
    }
    assert!(
        red > 20,
        "front view lost the vertical line, red rows {red}"
    );
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

    options.visual_style = VisualStyleRequest::Standard(StandardVisualStyle::Conceptual);
    let scene = compile(&doc, &options);
    assert_eq!(scene.visual_style.kind, StandardVisualStyle::Conceptual);
    assert_eq!(scene.visual_style.lighting, LightingModel::Gooch);
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

#[test]
fn srgb_clear_colors_are_linearized_for_the_swapchain() {
    let srgb = [33.0 / 255.0, 40.0 / 255.0, 48.0 / 255.0, 1.0];
    let linear = super::gpu::clear_color(wgpu::TextureFormat::Rgba8UnormSrgb, srgb);
    assert!(linear[0] < srgb[0]);
    assert!(linear[1] < srgb[1]);
    assert!(linear[2] < srgb[2]);
    assert_eq!(
        super::gpu::clear_color(wgpu::TextureFormat::Rgba8Unorm, srgb),
        srgb
    );
}

#[test]
fn offscreen_frame_draws_a_zero_length_point() {
    let mut doc = doc();
    let mut point = Point::from_coords(0.0, 0.0, 0.0);
    point.common.color = Color::RED;
    doc.add_entity(EntityType::Point(point)).unwrap();
    let image = match pollster::block_on(super::render_rgba(
        &doc,
        &View {
            target: Vector3::ZERO,
            height: 20.0,
            ..View::default()
        },
        64,
        64,
        &CompileOptions::default(),
    )) {
        Err(super::RenderError::NoAdapter) => return,
        Err(error) => panic!("{error}"),
        Ok(image) => image,
    };
    let red_pixels = image
        .pixels
        .chunks_exact(4)
        .filter(|pixel| pixel[0] > 80 && pixel[1] < 80 && pixel[2] < 80)
        .count();
    assert!(red_pixels > 0, "zero-length point produced no red pixels");
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
fn dashed_circle_stays_on_elliptic_arcs() {
    let mut doc = doc();
    add_dashed(&mut doc);
    let mut circle = Circle::from_coords(0.0, 0.0, 0.0, 5.0);
    circle.common.linetype = "DASHED".to_string();
    doc.add_entity(EntityType::Circle(circle)).unwrap();
    let scene = compile(&doc, &CompileOptions::default());
    let dashes: Vec<_> = scene.strokes.iter().filter(|stroke| !stroke.dot).collect();
    assert!(
        dashes.len() > 2,
        "expected several dashes, got {}",
        dashes.len()
    );
    for dash in dashes {
        match dash.curves.as_slice() {
            [Curve::Elliptic {
                center, u, sweep, ..
            }] => {
                assert!(center.length() < 1e-6);
                assert!((u.length() - 5.0).abs() < 1e-6);
                assert!(sweep.abs() < std::f64::consts::TAU - 0.2);
                assert!(sweep.abs() > 1e-3);
            }
            other => panic!("dashed circle fell back to chords: {other:?}"),
        }
    }
}

#[test]
fn circle_pixels_follow_the_curve() {
    let mut doc = doc();
    let mut circle = Circle::from_coords(0.0, 0.0, 0.0, 40.0);
    circle.common.color = Color::RED;
    doc.add_entity(EntityType::Circle(circle)).unwrap();
    let crown = 5.0_f64.to_radians();
    let on_curve = Vector3::new(40.0 * crown.cos(), 40.0 * crown.sin(), 0.0);
    let chord_angle = 10.0_f64.to_radians();
    let chord = (Vector3::new(40.0, 0.0, 0.0)
        + Vector3::new(40.0 * chord_angle.cos(), 40.0 * chord_angle.sin(), 0.0))
        * 0.5;
    let view = View {
        target: on_curve,
        height: 1.0,
        ..View::default()
    };
    let image = match pollster::block_on(super::render_rgba(
        &doc,
        &view,
        80,
        80,
        &CompileOptions::default(),
    )) {
        Err(super::RenderError::NoAdapter) => return,
        Err(error) => panic!("{error}"),
        Ok(image) => image,
    };
    let half = view.height * 0.5;
    let mut red = 0u32;
    let mut off_curve = 0u32;
    for y in 0..80usize {
        for x in 0..80usize {
            let pixel = &image.pixels[(y * 80 + x) * 4..];
            if pixel[0] <= 80 || pixel[1] >= 80 {
                continue;
            }
            red += 1;
            let ndc_x = (x as f64 + 0.5) / 80.0 * 2.0 - 1.0;
            let ndc_y = 1.0 - (y as f64 + 0.5) / 80.0 * 2.0;
            let wx = on_curve.x + ndc_x * half;
            let wy = on_curve.y + ndc_y * half;
            let pixels_off = ((wx * wx + wy * wy).sqrt() - 40.0).abs() * 80.0;
            if pixels_off > 2.5 {
                off_curve += 1;
            }
        }
    }
    assert!(red > 8, "the circle stroke did not cross the frame");
    assert_eq!(off_curve, 0, "{off_curve} red pixels left the true circle");
    let ndc_x = (chord.x - on_curve.x) / half;
    let ndc_y = (chord.y - on_curve.y) / half;
    let px = ((ndc_x + 1.0) * 0.5 * 80.0).round() as i32;
    let py = ((1.0 - ndc_y) * 0.5 * 80.0).round() as i32;
    if (0..80).contains(&px) && (0..80).contains(&py) {
        let pixel = &image.pixels[((py as usize) * 80 + px as usize) * 4..];
        assert!(
            pixel[0] < 80 || pixel[1] >= 80,
            "chord midpoint was painted, which is the polyline approximation"
        );
    }
}

#[test]
fn front_view_circle_follows_the_vertical_plane() {
    let mut doc = doc();
    let mut circle = Circle::from_coords(0.0, 0.0, 0.0, 40.0);
    circle.normal = -Vector3::UNIT_Y;
    circle.common.color = Color::RED;
    doc.add_entity(EntityType::Circle(circle)).unwrap();
    let angle = 5.0_f64.to_radians();
    let (axis_x, axis_y) = {
        let basis = crate::types::Matrix3::arbitrary_axis(-Vector3::UNIT_Y);
        (basis * Vector3::UNIT_X, basis * Vector3::UNIT_Y)
    };
    let on_curve = axis_x * (40.0 * angle.cos()) + axis_y * (40.0 * angle.sin());
    let view = View {
        target: on_curve,
        height: 1.0,
        direction: -Vector3::UNIT_Y,
        ..View::default()
    };
    let image = match pollster::block_on(super::render_rgba(
        &doc,
        &view,
        80,
        80,
        &CompileOptions::default(),
    )) {
        Err(super::RenderError::NoAdapter) => return,
        Err(error) => panic!("{error}"),
        Ok(image) => image,
    };
    let (right, up, _) = view.basis();
    let half = view.height * 0.5;
    let mut red = 0u32;
    let mut off_curve = 0u32;
    for y in 0..80usize {
        for x in 0..80usize {
            let pixel = &image.pixels[(y * 80 + x) * 4..];
            if pixel[0] <= 80 || pixel[1] >= 80 {
                continue;
            }
            red += 1;
            let ndc_x = (x as f64 + 0.5) / 80.0 * 2.0 - 1.0;
            let ndc_y = 1.0 - (y as f64 + 0.5) / 80.0 * 2.0;
            let world = on_curve + right * (ndc_x * half) + up * (ndc_y * half);
            let dx = world.dot(&axis_x);
            let dy = world.dot(&axis_y);
            let pixels_off = ((dx * dx + dy * dy).sqrt() - 40.0).abs() * 80.0;
            if pixels_off > 2.5 {
                off_curve += 1;
            }
        }
    }
    assert!(red > 8, "the front-view circle did not cross the frame");
    assert_eq!(
        off_curve, 0,
        "{off_curve} red pixels left the circle in the vertical plane"
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
    match &circle.curves[..] {
        [Curve::Elliptic { u, v, sweep, .. }] => {
            assert!((u.length() - 5.0).abs() < 1e-6);
            assert!((v.length() - 5.0).abs() < 1e-6);
            assert!((sweep.abs() - std::f64::consts::TAU).abs() < 1e-6);
        }
        other => panic!("circle is one elliptic arc, got {other:?}"),
    }
    let arc = &scene.strokes[1];
    let start = arc.points[0];
    let end = arc.points.last().unwrap();
    assert!((start.x - 25.0).abs() < 1e-5 && start.y.abs() < 1e-5);
    assert!((end.x - 20.0).abs() < 1e-5 && (end.y - 5.0).abs() < 1e-5);
    match &arc.curves[..] {
        [Curve::Elliptic { sweep, .. }] => {
            assert!((sweep.abs() - FRAC_PI_2).abs() < 1e-6);
        }
        other => panic!("arc is one elliptic arc, got {other:?}"),
    }
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
    let spline = scene
        .strokes
        .iter()
        .find(|stroke| matches!(stroke.curves.first(), Some(Curve::Cubic { .. })))
        .expect("spline stroke");
    assert!(
        spline
            .curves
            .iter()
            .all(|curve| matches!(curve, Curve::Cubic { .. })),
        "a cubic spline is cubic Bézier spans"
    );
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
fn shp_text_accepts_an_in_memory_font_file() {
    let mut doc = doc();
    doc.text_styles.get_mut("Standard").unwrap().font_file = "mini.shp".to_string();
    let text = Text::with_value("A", Vector3::ZERO).with_height(6.0);
    doc.add_entity(EntityType::Text(text)).unwrap();
    let mut options = CompileOptions::default();
    options.font_files.push((
        "mini.shp".to_string(),
        b"*0,4,TEST\n6,2,0,0\n*65,12,A\n1,8,(4,6),8,(4,-6),2,8,(2,0),0\n".to_vec(),
    ));
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
fn text_style_defaults_are_applied_to_strokes() {
    let dir = std::env::temp_dir().join("acadrust-render-text-style-font");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("mini.shp"),
        "*0,4,TEST\n6,2,0,0\n*65,20,A\n1,8,(8,0),8,(0,6),0\n",
    )
    .unwrap();

    let mut doc = doc();
    let mut narrow = TextStyle::new("Narrow");
    narrow.font_file = "mini".to_string();
    narrow.width_factor = 0.5;
    doc.text_styles.add(narrow).unwrap();
    let mut mirrored = TextStyle::new("Mirrored");
    mirrored.font_file = "mini".to_string();
    mirrored.flags.backward = true;
    doc.text_styles.add(mirrored).unwrap();
    let mut slanted = TextStyle::new("Slanted");
    slanted.font_file = "mini".to_string();
    slanted.oblique_angle = FRAC_PI_2 / 2.0;
    doc.text_styles.add(slanted).unwrap();

    let mut narrow_text = Text::with_value("A", Vector3::ZERO).with_height(6.0);
    narrow_text.style = "Narrow".to_string();
    let narrow_handle = doc.add_entity(EntityType::Text(narrow_text)).unwrap();
    let mut mirrored_text = Text::with_value("A", Vector3::new(20.0, 0.0, 0.0)).with_height(6.0);
    mirrored_text.style = "Mirrored".to_string();
    let mirrored_handle = doc.add_entity(EntityType::Text(mirrored_text)).unwrap();
    let mut slanted_text = Text::with_value("A", Vector3::new(40.0, 0.0, 0.0)).with_height(6.0);
    slanted_text.style = "Slanted".to_string();
    let slanted_handle = doc.add_entity(EntityType::Text(slanted_text)).unwrap();

    let mut options = CompileOptions::default();
    options.font_dirs.push(dir.clone());
    let scene = compile(&doc, &options);
    assert!(scene.skipped.is_empty(), "{:?}", scene.skipped);

    let bounds = |handle: Handle| {
        scene
            .strokes
            .iter()
            .filter(|stroke| stroke.source.handle == handle)
            .flat_map(|stroke| &stroke.points)
            .fold(None, |bounds: Option<(f64, f64, f64, f64)>, point| {
                Some(match bounds {
                    Some((min_x, max_x, min_y, max_y)) => (
                        min_x.min(point.x),
                        max_x.max(point.x),
                        min_y.min(point.y),
                        max_y.max(point.y),
                    ),
                    None => (point.x, point.x, point.y, point.y),
                })
            })
            .expect("text strokes")
    };
    let narrow_bounds = bounds(narrow_handle);
    assert!((narrow_bounds.1 - 4.0).abs() < 1e-6);
    let mirrored_bounds = bounds(mirrored_handle);
    assert!((mirrored_bounds.0 - 12.0).abs() < 1e-6);
    let slanted_bounds = bounds(slanted_handle);
    assert!((slanted_bounds.1 - 54.0).abs() < 1e-6);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn formatted_mtext_uses_font_advances_between_color_runs() {
    let dir = std::env::temp_dir().join("acadrust-render-mtext-font");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("mini.shp"),
        "*0,4,TEST\n6,2,0,0\n*65,20,A\n1,8,(8,0),8,(0,6),0\n",
    )
    .unwrap();

    let mut doc = doc();
    doc.text_styles.get_mut("Standard").unwrap().font_file = "mini".to_string();
    let mtext = MText::with_value(r"A{\C1;A}A", Vector3::ZERO).with_height(6.0);
    doc.add_entity(EntityType::MText(Box::new(mtext))).unwrap();
    let mut options = CompileOptions::default();
    options.font_dirs.push(dir.clone());
    let scene = compile(&doc, &options);
    let mut starts: Vec<f64> = scene
        .strokes
        .iter()
        .map(|stroke| {
            stroke
                .points
                .iter()
                .map(|point| point.x)
                .fold(f64::INFINITY, f64::min)
        })
        .collect();
    starts.sort_by(f64::total_cmp);
    assert_eq!(starts.len(), 3);
    assert!((starts[0] - 0.0).abs() < 1e-6, "starts={starts:?}");
    assert!((starts[1] - 8.0).abs() < 1e-6, "starts={starts:?}");
    assert!((starts[2] - 16.0).abs() < 1e-6, "starts={starts:?}");

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn standalone_attdef_draws_the_tag_instead_of_default_value() {
    let dir = std::env::temp_dir().join("acadrust-render-attdef-font");
    std::fs::create_dir_all(&dir).unwrap();
    // A and T each produce one stroke; G produces two. The fixture's tag
    // (TAG) therefore produces four strokes, while its default value
    // (ATTDEF) would produce only three because D, E and F are absent.
    std::fs::write(
        dir.join("mini.shp"),
        "*0,4,TEST\n6,2,0,0\n*65,8,A\n1,8,(0,6),0\n*84,8,T\n1,8,(0,6),0\n*71,14,G\n1,8,(0,6),2,1,8,(4,0),0\n",
    )
    .unwrap();
    let mut doc = doc();
    doc.text_styles.get_mut("Standard").unwrap().font_file = "mini".to_string();
    doc.add_entity(EntityType::AttributeDefinition(
        AttributeDefinition::constant("TAG", "ATTDEF"),
    ))
    .unwrap();
    let mut options = CompileOptions::default();
    options.font_dirs.push(dir.clone());
    let scene = compile(&doc, &options);
    assert!(scene.fallbacks.is_empty());
    assert!(scene.skipped.is_empty(), "{:?}", scene.skipped);
    assert_eq!(scene.strokes.len(), 4);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn missing_text_font_uses_the_builtin_stroke_font() {
    let mut doc = doc();
    doc.add_entity(EntityType::Text(
        Text::with_value("Se", Vector3::ZERO).with_height(72.0),
    ))
    .unwrap();
    let scene = compile(&doc, &CompileOptions::default());
    assert!(
        scene
            .skipped
            .iter()
            .all(|entry| !entry.reason.contains("font")),
        "{:?}",
        scene.skipped
    );
    let max_y = scene
        .strokes
        .iter()
        .flat_map(|stroke| &stroke.points)
        .map(|point| point.y)
        .fold(0.0, f64::max);
    assert!(max_y > 60.0, "cap height was {max_y}");
    assert!(scene.strokes.len() >= 2, "{}", scene.strokes.len());
}

#[test]
fn missing_shape_file_is_reported_and_not_replaced() {
    let mut doc = doc();
    let style = doc.text_styles.get_mut("Standard").unwrap();
    style.font_file = "ltypeshp.shx".to_string();
    style.is_shape_file = true;
    doc.add_entity(EntityType::Text(Text::with_value("BOX", Vector3::ZERO)))
        .unwrap();
    let scene = compile(&doc, &CompileOptions::default());
    assert!(scene.strokes.is_empty());
    assert!(scene
        .skipped
        .iter()
        .any(|entry| entry.reason.contains("font")));
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

#[test]
fn complex_linetype_places_text_on_the_gap() {
    let dir = std::env::temp_dir().join("acadrust-render-complex");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("mini.shp"),
        "*0,4,TEST\n6,2,0,0\n*65,12,A\n1,8,(4,6),8,(4,-6),2,8,(2,0),0\n",
    )
    .unwrap();
    let mut doc = doc();
    doc.text_styles.get_mut("Standard").unwrap().font_file = "mini".to_string();
    let mut linetype = LineType::new("GAS");
    let mut gap = LineTypeElement::space(4.0);
    gap.complex = Some(crate::tables::LineTypeComplexData {
        content: crate::tables::LineTypeComplexContent::Text {
            text: "A".to_string(),
        },
        scale: 6.0,
        ..crate::tables::LineTypeComplexData::default()
    });
    linetype.elements = vec![LineTypeElement::dash(4.0), gap];
    linetype.pattern_length = 8.0;
    doc.line_types.add(linetype).unwrap();
    let mut line = Line::from_coords(0.0, 0.0, 0.0, 8.0, 0.0, 0.0);
    line.common.linetype = "GAS".to_string();
    doc.add_entity(EntityType::Line(line)).unwrap();
    let mut options = CompileOptions::default();
    options.font_dirs.push(dir);
    let scene = compile(&doc, &options);
    assert!(scene
        .strokes
        .iter()
        .flat_map(|stroke| &stroke.points)
        .any(|point| point.y > 1.0));
}

#[test]
fn insert_clip_cuts_a_line_to_the_rectangle() {
    let mut doc = doc();
    let mut record = BlockRecord::new("CLIPPED");
    let block_handle = doc.allocate_handle();
    record.set_handle(block_handle);
    doc.block_records.add(record).unwrap();
    let mut line = Line::from_coords(0.0, 0.0, 0.0, 10.0, 0.0, 0.0);
    line.common.owner_handle = block_handle;
    doc.add_entity(EntityType::Line(line)).unwrap();

    let mut filter = crate::objects::SpatialFilter::new();
    filter.handle = doc.allocate_handle();
    filter.boundary_points = vec![Vector2::new(0.0, -1.0), Vector2::new(5.0, 1.0)];
    let mut spatial = crate::objects::Dictionary::new();
    spatial.handle = doc.allocate_handle();
    spatial.entries.push(("SPATIAL".to_string(), filter.handle));
    let mut filters = crate::objects::Dictionary::new();
    filters.handle = doc.allocate_handle();
    filters
        .entries
        .push(("ACAD_FILTER".to_string(), spatial.handle));
    doc.objects
        .insert(filter.handle, ObjectType::SpatialFilter(filter));
    doc.objects
        .insert(spatial.handle, ObjectType::Dictionary(spatial));
    let filters_handle = filters.handle;
    doc.objects
        .insert(filters_handle, ObjectType::Dictionary(filters));

    let mut insert = Insert::new("CLIPPED", Vector3::ZERO);
    insert.common.xdictionary_handle = Some(filters_handle);
    doc.add_entity(EntityType::Insert(insert)).unwrap();
    let scene = compile(&doc, &CompileOptions::default());
    assert!(!scene.strokes.is_empty());
    let max_x = scene
        .strokes
        .iter()
        .flat_map(|stroke| &stroke.points)
        .map(|point| point.x)
        .fold(0.0_f64, f64::max);
    assert!(max_x < 5.1, "clipped line reached x={max_x}");
    assert!(max_x > 4.0, "clipped line was too short, max x={max_x}");
}

#[test]
fn ttf_style_becomes_a_glyph_run() {
    let Some(path) = [
        r"C:\Windows\Fonts\arial.ttf",
        r"C:\Windows\Fonts\calibri.ttf",
    ]
    .into_iter()
    .map(std::path::PathBuf::from)
    .find(|path| path.is_file()) else {
        return;
    };
    let dir = path.parent().unwrap().to_path_buf();
    let file = path.file_name().unwrap().to_string_lossy().to_string();
    let mut doc = doc();
    doc.text_styles.get_mut("Standard").unwrap().font_file = file;
    doc.add_entity(EntityType::Text(
        Text::with_value("A", Vector3::ZERO).with_height(10.0),
    ))
    .unwrap();
    let mut options = CompileOptions::default();
    options.font_dirs.push(dir);
    let scene = compile(&doc, &options);
    assert_eq!(scene.glyphs.len(), 1);
    assert!(scene.strokes.is_empty());
    let image = match pollster::block_on(super::render_rgba(
        &doc,
        &View {
            target: Vector3::new(4.0, 4.0, 0.0),
            height: 20.0,
            ..View::default()
        },
        64,
        64,
        &options,
    )) {
        Err(super::RenderError::NoAdapter) => return,
        Err(error) => panic!("{error}"),
        Ok(image) => image,
    };
    let ink = image
        .pixels
        .chunks_exact(4)
        .filter(|pixel| pixel[0] > 200)
        .count();
    assert!(
        ink > 8,
        "true type glyph did not cover the frame, ink={ink}"
    );
}

fn layout(name: &str) -> CompileOptions {
    CompileOptions {
        space: DisplaySpace::Layout(name.to_string()),
        ..CompileOptions::default()
    }
}

fn model_window(center: Vector3, width: f64, height: f64, view_height: f64) -> Viewport {
    let mut viewport = Viewport::new();
    viewport.id = 2;
    viewport.center = center;
    viewport.width = width;
    viewport.height = height;
    viewport.view_height = view_height;
    viewport.view_target = Vector3::ZERO;
    viewport.view_center = Vector3::ZERO;
    viewport.view_direction = Vector3::new(0.0, 0.0, 1.0);
    viewport
}

fn two_point_strokes(scene: &super::Scene) -> Vec<&super::Stroke> {
    scene
        .strokes
        .iter()
        .filter(|stroke| stroke.points.len() == 2 && stroke.path == DrawPath::Dedicated)
        .collect()
}

#[test]
fn layout_draws_paper_geometry_and_a_scaled_viewport() {
    let mut doc = doc();
    doc.add_entity(EntityType::Line(Line::from_coords(
        0.0, 0.0, 0.0, 4.0, 0.0, 0.0,
    )))
    .unwrap();
    let mut paper = Line::from_coords(0.0, 30.0, 0.0, 6.0, 30.0, 0.0);
    paper.common.color = Color::RED;
    doc.add_paper_space_entity(EntityType::Line(paper)).unwrap();

    let model_only = compile(&doc, &CompileOptions::default());
    assert_eq!(model_only.strokes.len(), 1);
    assert!(model_only.strokes[0]
        .points
        .iter()
        .all(|point| point.y.abs() < 1e-6));

    let bare = compile(&doc, &layout("Layout1"));
    assert_eq!(two_point_strokes(&bare).len(), 1);
    assert!((two_point_strokes(&bare)[0].points[0].y - 30.0).abs() < 1e-6);

    let mut viewport = model_window(Vector3::ZERO, 30.0, 20.0, 10.0);
    viewport.view_center = Vector3::new(2.0, 0.0, 0.0);
    doc.add_entity_to_layout(EntityType::Viewport(Box::new(viewport)), "Layout1")
        .unwrap();
    let scene = compile(&doc, &layout("Layout1"));
    assert!(scene.fallbacks.is_empty());
    let window: Vec<_> = two_point_strokes(&scene)
        .into_iter()
        .filter(|stroke| stroke.points[0].y.abs() < 1e-6)
        .collect();
    assert_eq!(window.len(), 1);
    let start = window[0].points[0];
    let end = window[0].points[1];
    assert!((start.x + 4.0).abs() < 1e-6 && start.y.abs() < 1e-6);
    assert!((end.x - 4.0).abs() < 1e-6 && end.y.abs() < 1e-6);
    assert!(scene.strokes.iter().any(|stroke| stroke.points.len() == 5));
}

#[test]
fn viewport_twist_rotates_model_geometry_clockwise() {
    let mut doc = doc();
    doc.add_entity(EntityType::Line(Line::from_coords(
        0.0, 0.0, 0.0, 4.0, 0.0, 0.0,
    )))
    .unwrap();
    let mut viewport = model_window(Vector3::new(10.0, 20.0, 0.0), 40.0, 40.0, 20.0);
    viewport.twist_angle = std::f64::consts::FRAC_PI_2;
    doc.add_entity_to_layout(EntityType::Viewport(Box::new(viewport)), "Layout1")
        .unwrap();
    let scene = compile(&doc, &layout("Layout1"));
    let segment = two_point_strokes(&scene)
        .into_iter()
        .find(|stroke| (stroke.points[1] - stroke.points[0]).length() > 1.0)
        .expect("twisted segment");
    let start = segment.points[0];
    let end = segment.points[1];
    assert!((start.x - 10.0).abs() < 1e-5 && (start.y - 20.0).abs() < 1e-5);
    assert!((end.x - 10.0).abs() < 1e-5 && (end.y - 12.0).abs() < 1e-5);
}

#[test]
fn viewport_side_view_maps_world_z_onto_the_sheet() {
    let mut doc = doc();
    doc.add_entity(EntityType::Line(Line::from_coords(
        0.0, 0.0, 0.0, 0.0, 0.0, 4.0,
    )))
    .unwrap();
    let mut viewport = model_window(Vector3::new(5.0, 6.0, 0.0), 20.0, 20.0, 10.0);
    viewport.view_direction = Vector3::new(0.0, -1.0, 0.0);
    doc.add_entity_to_layout(EntityType::Viewport(Box::new(viewport)), "Layout1")
        .unwrap();
    let scene = compile(&doc, &layout("Layout1"));
    let segment = two_point_strokes(&scene)
        .into_iter()
        .find(|stroke| (stroke.points[1] - stroke.points[0]).length() > 1.0)
        .expect("side view");
    let start = segment.points[0];
    let end = segment.points[1];
    assert!((start.x - 5.0).abs() < 1e-5 && (start.y - 6.0).abs() < 1e-5);
    assert!((end.x - 5.0).abs() < 1e-5 && (end.y - 14.0).abs() < 1e-5);
}

#[test]
fn viewport_clips_model_geometry_to_the_window() {
    let mut doc = doc();
    doc.add_entity(EntityType::Line(Line::from_coords(
        -10.0, 0.0, 0.0, 10.0, 0.0, 0.0,
    )))
    .unwrap();
    doc.add_entity_to_layout(
        EntityType::Viewport(Box::new(model_window(Vector3::ZERO, 4.0, 4.0, 4.0))),
        "Layout1",
    )
    .unwrap();
    let scene = compile(&doc, &layout("Layout1"));
    let segment = two_point_strokes(&scene)
        .into_iter()
        .find(|stroke| stroke.points.iter().all(|point| point.y.abs() < 1e-6))
        .expect("clipped line");
    let xs = [segment.points[0].x, segment.points[1].x];
    assert!(xs.iter().any(|x| (*x + 2.0).abs() < 1e-5));
    assert!(xs.iter().any(|x| (*x - 2.0).abs() < 1e-5));
}

#[test]
fn viewport_clip_boundary_replaces_the_rectangle() {
    let mut doc = doc();
    doc.add_entity(EntityType::Line(Line::from_coords(
        -10.0, 0.0, 0.0, 10.0, 0.0, 0.0,
    )))
    .unwrap();
    let mut boundary = LwPolyline::new();
    boundary.is_closed = true;
    boundary.vertices = vec![
        LwVertex::new(Vector2::new(-1.0, -1.0)),
        LwVertex::new(Vector2::new(1.0, -1.0)),
        LwVertex::new(Vector2::new(1.0, 1.0)),
        LwVertex::new(Vector2::new(-1.0, 1.0)),
    ];
    let boundary_handle = doc
        .add_entity_to_layout(EntityType::LwPolyline(boundary), "Layout1")
        .unwrap();
    let mut viewport = model_window(Vector3::ZERO, 40.0, 40.0, 40.0);
    viewport.clip_boundary_handle = boundary_handle;
    viewport.status.non_rectangular_clipping = true;
    doc.add_entity_to_layout(EntityType::Viewport(Box::new(viewport)), "Layout1")
        .unwrap();
    let scene = compile(&doc, &layout("Layout1"));
    let segment = two_point_strokes(&scene)
        .into_iter()
        .find(|stroke| {
            stroke.points.iter().all(|point| point.y.abs() < 1e-6)
                && (stroke.points[1].x - stroke.points[0].x).abs() < 3.0
        })
        .expect("boundary clip");
    let xs = [segment.points[0].x, segment.points[1].x];
    assert!(xs.iter().any(|x| (*x + 1.0).abs() < 1e-4));
    assert!(xs.iter().any(|x| (*x - 1.0).abs() < 1e-4));
    assert!(scene.strokes.iter().all(|stroke| {
        stroke
            .points
            .iter()
            .all(|point| point.x.abs() < 2.0 && point.y.abs() < 2.0)
    }));
}

#[test]
fn viewport_freeze_hides_only_the_model_picture() {
    let mut doc = doc();
    let mut hidden = Layer::new("Hide");
    hidden.set_handle(doc.allocate_handle());
    doc.layers.add(hidden).unwrap();
    let hidden_handle = doc.layers.get("Hide").unwrap().handle;
    let mut hidden_line = Line::from_coords(0.0, 0.0, 0.0, 2.0, 0.0, 0.0);
    hidden_line.common.layer = "Hide".to_string();
    doc.add_entity(EntityType::Line(hidden_line)).unwrap();
    doc.add_entity(EntityType::Line(Line::from_coords(
        0.0, 3.0, 0.0, 2.0, 3.0, 0.0,
    )))
    .unwrap();
    let mut paper = Line::from_coords(0.0, 0.0, 0.0, 5.0, 0.0, 0.0);
    paper.common.layer = "Hide".to_string();
    doc.add_paper_space_entity(EntityType::Line(paper)).unwrap();
    let mut viewport = model_window(Vector3::ZERO, 40.0, 40.0, 40.0);
    viewport.frozen_layers = vec![hidden_handle];
    doc.add_entity_to_layout(EntityType::Viewport(Box::new(viewport)), "Layout1")
        .unwrap();

    let scene = compile(&doc, &layout("Layout1"));
    let horizontal: Vec<_> = two_point_strokes(&scene)
        .into_iter()
        .filter(|stroke| stroke.points.iter().all(|point| point.y.abs() < 1e-6))
        .collect();
    assert_eq!(horizontal.len(), 1);
    assert!((horizontal[0].points[1].x - horizontal[0].points[0].x).abs() - 5.0 < 1e-6);
    assert!(two_point_strokes(&scene).iter().any(|stroke| {
        stroke
            .points
            .iter()
            .all(|point| (point.y - 3.0).abs() < 1e-6)
    }));
    assert!(scene
        .skipped
        .iter()
        .any(|entry| { entry.reason.contains("frozen in this viewport") }));
}

#[test]
fn psltscale_keeps_paper_dash_lengths() {
    let mut doc = doc();
    add_dashed(&mut doc);
    doc.header.linetype_scale = 1.0;
    doc.header.paper_space_linetype_scaling = true;
    let mut line = Line::from_coords(0.0, 0.0, 0.0, 4.0, 0.0, 0.0);
    line.common.linetype = "DASHED".to_string();
    doc.add_entity(EntityType::Line(line)).unwrap();
    let mut viewport = model_window(Vector3::ZERO, 40.0, 20.0, 10.0);
    viewport.view_center = Vector3::new(2.0, 0.0, 0.0);
    doc.add_entity_to_layout(EntityType::Viewport(Box::new(viewport)), "Layout1")
        .unwrap();
    let scene = compile(&doc, &layout("Layout1"));
    let dashes: Vec<_> = scene
        .strokes
        .iter()
        .filter(|stroke| {
            !stroke.dot
                && stroke.points.len() == 2
                && stroke.points.iter().all(|point| point.y.abs() < 1e-6)
                && stroke.points[1].x > stroke.points[0].x
        })
        .collect();
    assert_eq!(dashes.len(), 4);
    for dash in dashes {
        let length = dash.points[1].x - dash.points[0].x;
        assert!((length - 1.0).abs() < 1e-5, "paper dash was {length}");
    }
}

#[test]
fn psltscale_off_scales_dashes_with_the_viewport() {
    let mut doc = doc();
    add_dashed(&mut doc);
    doc.header.linetype_scale = 1.0;
    doc.header.paper_space_linetype_scaling = false;
    let mut line = Line::from_coords(0.0, 0.0, 0.0, 4.0, 0.0, 0.0);
    line.common.linetype = "DASHED".to_string();
    doc.add_entity(EntityType::Line(line)).unwrap();
    let mut viewport = model_window(Vector3::ZERO, 40.0, 20.0, 10.0);
    viewport.view_center = Vector3::new(2.0, 0.0, 0.0);
    doc.add_entity_to_layout(EntityType::Viewport(Box::new(viewport)), "Layout1")
        .unwrap();
    let scene = compile(&doc, &layout("Layout1"));
    let dashes: Vec<_> = scene
        .strokes
        .iter()
        .filter(|stroke| {
            !stroke.dot
                && stroke.points.len() == 2
                && stroke.points.iter().all(|point| point.y.abs() < 1e-6)
                && (stroke.points[1].x - stroke.points[0].x).abs() > 1.5
        })
        .collect();
    assert_eq!(dashes.len(), 2);
    for dash in dashes {
        let length = (dash.points[1].x - dash.points[0].x).abs();
        assert!(
            (length - 2.0).abs() < 1e-5,
            "model dash was {length} paper units"
        );
    }
}

#[test]
fn sheet_lineweights_use_paper_units() {
    let mut doc = doc();
    doc.header.measurement = 1;
    doc.header.lineweight_display = true;
    let mut paper = Line::from_coords(0.0, 0.0, 0.0, 10.0, 0.0, 0.0);
    paper.common.line_weight = LineWeight::Value(50);
    doc.add_paper_space_entity(EntityType::Line(paper)).unwrap();
    let mut model = Line::from_coords(0.0, 1.0, 0.0, 2.0, 1.0, 0.0);
    model.common.line_weight = LineWeight::Value(25);
    doc.add_entity(EntityType::Line(model)).unwrap();
    doc.add_entity_to_layout(
        EntityType::Viewport(Box::new(model_window(Vector3::ZERO, 20.0, 20.0, 20.0))),
        "Layout1",
    )
    .unwrap();

    let scene = compile(&doc, &layout("Layout1"));
    assert!((scene.sheet_units_per_mm - 1.0).abs() < 1e-12);
    assert!(scene
        .strokes
        .iter()
        .any(|stroke| { stroke.style.sheet && (stroke.style.lineweight_mm - 0.50).abs() < 1e-9 }));
    assert!(scene
        .strokes
        .iter()
        .any(|stroke| { stroke.style.sheet && (stroke.style.lineweight_mm - 0.25).abs() < 1e-9 }));

    let model_scene = compile(&doc, &CompileOptions::default());
    assert_eq!(model_scene.sheet_units_per_mm, 0.0);
    assert!(model_scene.strokes.iter().all(|stroke| !stroke.style.sheet));
}

#[test]
fn english_sheets_convert_lineweight_millimetres() {
    let mut doc = doc();
    doc.header.measurement = 0;
    doc.header.lineweight_display = true;
    let mut paper = Line::from_coords(0.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    paper.common.line_weight = LineWeight::Value(25);
    doc.add_paper_space_entity(EntityType::Line(paper)).unwrap();
    let scene = compile(&doc, &layout("Layout1"));
    assert!((scene.sheet_units_per_mm - (1.0 / 25.4)).abs() < 1e-12);
    assert!(scene.strokes[0].style.sheet);
    assert!((scene.strokes[0].style.lineweight_mm - 0.25).abs() < 1e-9);
}

#[test]
fn off_and_paper_viewports_do_not_show_model_space() {
    let mut doc = doc();
    doc.add_entity(EntityType::Line(Line::from_coords(
        0.0, 0.0, 0.0, 8.0, 0.0, 0.0,
    )))
    .unwrap();
    doc.add_paper_space_entity(EntityType::Line(Line::from_coords(
        0.0, 40.0, 0.0, 4.0, 40.0, 0.0,
    )))
    .unwrap();
    let mut paper_viewport = model_window(Vector3::ZERO, 500.0, 500.0, 100.0);
    paper_viewport.id = 1;
    doc.add_entity_to_layout(EntityType::Viewport(Box::new(paper_viewport)), "Layout1")
        .unwrap();
    let mut off = model_window(Vector3::ZERO, 80.0, 80.0, 20.0);
    off.id = 3;
    off.status.is_on = false;
    off.status.is_off = true;
    doc.add_entity_to_layout(EntityType::Viewport(Box::new(off)), "Layout1")
        .unwrap();
    let scene = compile(&doc, &layout("Layout1"));
    assert!(!scene.strokes.is_empty());
    assert!(scene.strokes.iter().all(|stroke| {
        stroke
            .points
            .iter()
            .all(|point| (point.y - 40.0).abs() < 1e-6)
    }));
    assert!(scene
        .skipped
        .iter()
        .any(|entry| entry.reason == "paper-space viewport"));
    assert!(scene
        .skipped
        .iter()
        .any(|entry| entry.reason == "viewport is off"));
}

#[test]
fn missing_layout_compiles_nothing() {
    let doc = doc();
    let scene = compile(&doc, &layout("NotALayout"));
    assert!(scene.strokes.is_empty());
    assert!(scene.skipped.iter().any(|entry| entry.kind == "Layout"));
}

#[test]
fn view_border_draws_its_rectangle() {
    let mut doc = doc();
    let mut border = ViewBorder::new();
    border.min = [1.0, 2.0];
    border.max = [11.0, 8.0];
    doc.add_entity(EntityType::ViewBorder(border)).unwrap();
    let scene = compile(&doc, &CompileOptions::default());
    assert!(scene.fallbacks.is_empty());
    assert!(!super::FALLBACK_KINDS.contains(&"ViewBorder"));
    assert_eq!(scene.strokes.len(), 1);
    assert_eq!(scene.strokes[0].points.len(), 5);
    assert!((scene.strokes[0].points[0].x - 1.0).abs() < 1e-9);
    assert!((scene.strokes[0].points[0].y - 2.0).abs() < 1e-9);
    assert!(scene.strokes[0]
        .points
        .iter()
        .any(|point| { (point.x - 11.0).abs() < 1e-9 && (point.y - 8.0).abs() < 1e-9 }));
}

#[test]
fn section_symbol_draws_the_cut_line() {
    let mut doc = doc();
    let mut symbol = SectionSymbol::new();
    symbol.end_a = [0.0, 0.0];
    symbol.end_b = [10.0, 0.0];
    doc.add_entity(EntityType::SectionSymbol(symbol)).unwrap();
    let scene = compile(&doc, &CompileOptions::default());
    assert!(scene.fallbacks.is_empty());
    assert!(!super::FALLBACK_KINDS.contains(&"SectionSymbol"));
    assert_eq!(scene.strokes.len(), 1);
    assert!((scene.strokes[0].points[0].x).abs() < 1e-9);
    assert!((scene.strokes[0].points.last().unwrap().x - 10.0).abs() < 1e-9);
}

#[test]
fn section_symbol_broken_style_draws_stubs_and_arrows() {
    let mut doc = doc();
    let mut symbol = SectionSymbol::new();
    symbol.points = vec![
        SectionSymbolPoint {
            point: Vector3::new(0.0, 0.0, 0.0),
            bulge: 0.0,
            label: String::new(),
            label_offset: Vector3::ZERO,
            raw_flag_280: 0,
        },
        SectionSymbolPoint {
            point: Vector3::new(20.0, 0.0, 0.0),
            bulge: 0.0,
            label: String::new(),
            label_offset: Vector3::ZERO,
            raw_flag_280: 0,
        },
    ];
    doc.section_view_style = Some(SectionViewStyle {
        show_arrows: true,
        show_plane_line: false,
        show_end_lines: true,
        arrow_size: 1.0,
        arrow_extension: 0.0,
        label_height: 2.5,
        label_offset: 0.0,
        label_position: 0,
        arrow_position: 0,
        end_line_length: 2.0,
        end_line_overshoot: 0.0,
        arrow_start_handle: 0,
        arrow_end_handle: 0,
        arrow_is_default: true,
    });
    doc.add_entity(EntityType::SectionSymbol(symbol)).unwrap();
    let scene = compile(&doc, &CompileOptions::default());
    assert!(scene.fallbacks.is_empty());
    assert_eq!(scene.fills.len(), 2);
    let stubs: Vec<_> = scene
        .strokes
        .iter()
        .filter(|stroke| stroke.path == DrawPath::Dedicated)
        .collect();
    assert_eq!(stubs.len(), 2);
    for stub in stubs {
        let length = (stub.points.last().unwrap().x - stub.points[0].x).abs();
        assert!((length - 2.0).abs() < 1e-5, "stub length {length}");
    }
}

#[test]
fn section_object_draws_the_cutting_plane() {
    let mut doc = doc();
    doc.add_entity(EntityType::Extended(Box::new(ExtendedEntity {
        common: EntityCommon::default(),
        data: ExtendedEntityData::SectionObject(SectionObjectData {
            state: 0,
            flags: 0,
            name: "A".to_string(),
            vertical_direction: Vector3::new(0.0, 0.0, 1.0),
            top_height: 1.0,
            bottom_height: 0.0,
            indicator_alpha: 0,
            indicator_color: Color::WHITE,
            vertices: vec![
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(10.0, 0.0, 0.0),
                Vector3::new(10.0, 4.0, 0.0),
            ],
            back_line_vertices: vec![
                Vector3::new(0.0, 0.0, 1.0),
                Vector3::new(10.0, 0.0, 1.0),
                Vector3::new(10.0, 4.0, 1.0),
            ],
            settings_handle: Handle::NULL,
        }),
    })))
    .unwrap();
    let scene = compile(&doc, &CompileOptions::default());
    assert!(scene.fallbacks.is_empty());
    assert!(!super::FALLBACK_KINDS.contains(&"SectionObject"));
    assert!(scene.strokes.len() >= 5);
    assert!(scene.strokes.iter().any(|stroke| {
        stroke.points.len() == 4
            && stroke
                .points
                .iter()
                .any(|point| (point.x - 10.0).abs() < 1e-9 && (point.y - 4.0).abs() < 1e-9)
    }));
}

#[test]
fn sheet_lineweight_grows_when_zooming() {
    let mut doc = doc();
    doc.header.measurement = 1;
    doc.header.lineweight_display = true;
    let mut line = Line::from_coords(-30.0, 0.0, 0.0, 30.0, 0.0, 0.0);
    line.common.color = Color::RED;
    line.common.line_weight = LineWeight::Value(50);
    doc.add_paper_space_entity(EntityType::Line(line)).unwrap();
    let options = layout("Layout1");
    let config = super::RendererConfig {
        samples: 1,
        ..super::RendererConfig::default()
    };
    let render_at = |height: f64| {
        pollster::block_on(super::render_rgba_with(
            &doc,
            &View {
                target: Vector3::ZERO,
                height,
                ..View::default()
            },
            80,
            40,
            &options,
            &config,
        ))
    };
    let near = match render_at(4.0) {
        Err(super::RenderError::NoAdapter) => return,
        Err(error) => panic!("{error}"),
        Ok(image) => image,
    };
    let far = render_at(40.0).expect("adapter already opened");
    let near_rows = red_rows(&near);
    let far_rows = red_rows(&far);
    assert!(
        near_rows > far_rows,
        "sheet lineweight did not grow with zoom: near={near_rows} far={far_rows}"
    );
    assert!(
        (4..=7).contains(&near_rows),
        "0.50 mm on a 4-unit-tall, 40-pixel view should cover about 5 rows, found {near_rows}"
    );
}

fn solid_from(document: crate::entities::acis::SatDocument) -> crate::entities::Solid3D {
    let mut solid = crate::entities::Solid3D::new();
    solid.acis_data = crate::entities::AcisData::from_sat_document(&document);
    solid.common.color = Color::RED;
    solid
}

#[test]
fn shaded_box_has_outward_normals_and_shared_edges() {
    let mut doc = doc();
    doc.add_entity(EntityType::Solid3D(solid_from(
        crate::entities::acis::primitives::build_box([0.0, 0.0, 0.0], 10.0, 10.0, 10.0),
    )))
    .unwrap();
    let scene = compile(&doc, &CompileOptions::default());
    assert!(scene.fallbacks.is_empty());
    assert!(!super::FALLBACK_KINDS.contains(&"Solid3D"));
    let fill = &scene.fills[0];
    assert_eq!(fill.triangles.len(), fill.normals.len());
    assert!(
        fill.triangles.len() >= 36,
        "triangles {}",
        fill.triangles.len()
    );
    let top = fill
        .normals
        .iter()
        .zip(&fill.triangles)
        .find(|(_, point)| point.z > 4.9)
        .expect("top face");
    assert!(top.0.z > 0.9, "top normal {:?}", top.0);
    assert!(
        scene.strokes.len() >= 12,
        "boundary edges {}",
        scene.strokes.len()
    );
}

#[test]
fn cylinder_side_stays_on_the_surface_and_matches_the_cap() {
    let tess = super::tess::tessellate(&crate::entities::acis::primitives::build_cylinder(
        [0.0, 0.0, 0.0],
        5.0,
        12.0,
    ));
    assert!(!tess.triangles.is_empty());
    assert!(
        tess.edge_curves
            .iter()
            .any(|curve| matches!(curve, Curve::Elliptic { .. })),
        "a cylinder rim is an analytic circle"
    );
    let mut cap_tops = Vec::new();
    let mut side_tops = Vec::new();
    for corner in tess.triangles.chunks_exact(3) {
        let on_top = corner
            .iter()
            .filter(|point| (point.z - 12.0).abs() < 1e-6)
            .count();
        if on_top == 3 {
            cap_tops.extend(corner.iter().copied());
        } else if on_top > 0 {
            side_tops.extend(
                corner
                    .iter()
                    .filter(|point| (point.z - 12.0).abs() < 1e-6)
                    .copied(),
            );
        }
        for point in corner {
            let radial = (point.x * point.x + point.y * point.y).sqrt();
            let on_wall = (radial - 5.0).abs() < 0.05 && point.z >= -1e-6 && point.z <= 12.0 + 1e-6;
            let on_cap = point.z.abs() < 1e-5 || (point.z - 12.0).abs() < 1e-5;
            assert!(
                on_wall || on_cap && radial <= 5.05,
                "point off the cylinder: {point:?}"
            );
        }
    }
    assert!(!cap_tops.is_empty() && !side_tops.is_empty());
    for side in &side_tops {
        assert!(
            cap_tops.iter().any(|cap| (*cap - *side).length() < 1e-8),
            "side vertex {side:?} is not on the cap"
        );
    }
    for triangle in tess.triangles.chunks_exact(3).filter(|triangle| {
        triangle.iter().any(|point| point.z.abs() < 1e-5)
            && triangle.iter().any(|point| (point.z - 12.0).abs() < 1e-5)
    }) {
        let mut angles = triangle
            .iter()
            .map(|point| point.y.atan2(point.x))
            .collect::<Vec<_>>();
        angles.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
        let largest_gap = angles
            .windows(2)
            .map(|pair| pair[1] - pair[0])
            .chain(std::iter::once(
                angles[0] + std::f64::consts::TAU - angles[2],
            ))
            .fold(0.0_f64, f64::max);
        let covered_angle = std::f64::consts::TAU - largest_gap;
        assert!(
            covered_angle <= std::f64::consts::FRAC_PI_3,
            "cylinder wall triangle spans {covered_angle} radians: {triangle:?}"
        );
    }
}

#[test]
fn sphere_samples_lie_on_the_sphere() {
    let tess = super::tess::tessellate(&crate::entities::acis::primitives::build_sphere(
        [1.0, 2.0, 3.0],
        4.0,
    ));
    assert!(tess.triangles.len() > 200);
    for (point, normal) in tess.triangles.iter().zip(&tess.normals) {
        let radius = (*point - Vector3::new(1.0, 2.0, 3.0)).length();
        assert!((radius - 4.0).abs() < 1e-6, "radius {radius}");
        let expected = (*point - Vector3::new(1.0, 2.0, 3.0)).normalize();
        assert!(
            normal.dot(&expected) > 0.99,
            "normal {normal:?} expected {expected:?}"
        );
    }
}

#[test]
fn cone_reaches_its_apex_and_base() {
    let tess = super::tess::tessellate(&crate::entities::acis::primitives::build_cone(
        [0.0, 0.0, 0.0],
        4.0,
        9.0,
    ));
    assert!(tess
        .triangles
        .iter()
        .any(|point| (point.z - 9.0).abs() < 1e-5 && point.x.abs() < 1e-4 && point.y.abs() < 1e-4));
    assert!(tess.triangles.iter().any(|point| {
        point.z.abs() < 1e-5 && (point.x * point.x + point.y * point.y).sqrt() > 3.5
    }));
}

#[test]
fn torus_samples_satisfy_the_tube_equation() {
    let tess = super::tess::tessellate(&crate::entities::acis::primitives::build_torus(
        [0.0, 0.0, 0.0],
        6.0,
        1.5,
    ));
    assert!(tess.triangles.len() > 100);
    for point in &tess.triangles {
        let radial = (point.x * point.x + point.y * point.y).sqrt();
        let tube = ((radial - 6.0).powi(2) + point.z * point.z).sqrt();
        assert!((tube - 1.5).abs() < 1e-5, "tube radius {tube} at {point:?}");
    }
}

#[test]
fn bilinear_spline_patch_interpolates_its_corners() {
    use crate::entities::acis::{SatDocument, SatPointer, Sense, Sidedness};
    let mut sat = SatDocument::new_body();
    let control = [
        [0.0, 0.0, 0.0],
        [4.0, 0.0, 0.0],
        [0.0, 4.0, 0.0],
        [4.0, 4.0, 2.0],
    ];
    let knots = [(0.0, 2), (1.0, 2)];
    let surface = sat.add_spline_surface(
        false, false, 1, 1, false, false, &knots, &knots, &control, None, 0.0,
    );
    let base = sat.records.len() as i32;
    sat.add_face(
        SatPointer::NULL,
        SatPointer::NULL,
        SatPointer::new(base + 1),
        SatPointer::new(surface),
        Sense::Forward,
        Sidedness::Single,
    );
    sat.add_shell(SatPointer::new(base), SatPointer::new(base + 2));
    sat.add_lump(SatPointer::new(base + 1), SatPointer::new(0));
    if let Some(body) = sat.record_mut(0) {
        body.tokens[1] = crate::entities::acis::SatToken::Pointer(SatPointer::new(base + 2));
    }
    sat.complete_brep_links();
    let tess = super::tess::tessellate(&sat);
    let has = |x: f64, y: f64, z: f64| {
        tess.triangles.iter().any(|point| {
            (point.x - x).abs() < 1e-4 && (point.y - y).abs() < 1e-4 && (point.z - z).abs() < 1e-4
        })
    };
    assert!(has(0.0, 0.0, 0.0), "missing origin corner");
    assert!(has(4.0, 4.0, 2.0), "missing far corner");
}

#[test]
fn wireframe_style_draws_isolines_instead_of_faces() {
    let mut doc = doc();
    doc.add_entity(EntityType::Solid3D(solid_from(
        crate::entities::acis::primitives::build_sphere([0.0, 0.0, 0.0], 5.0),
    )))
    .unwrap();
    let mut options = CompileOptions::default();
    options.solid_style = SolidStyle::Wireframe;
    let scene = compile(&doc, &options);
    assert!(scene.fills.is_empty());
    assert!(scene.strokes.len() > 6, "isolines {}", scene.strokes.len());
    assert!(scene.fallbacks.is_empty());
}

#[test]
fn mesh_and_light_have_dedicated_pictures() {
    let mut doc = doc();
    let mut mesh = crate::entities::Mesh::new();
    mesh.common.color = Color::RED;
    mesh.vertices = vec![
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(2.0, 0.0, 0.0),
        Vector3::new(2.0, 2.0, 1.0),
        Vector3::new(0.0, 2.0, 1.0),
    ];
    mesh.faces.push(crate::entities::MeshFace::quad(0, 1, 2, 3));
    doc.add_entity(EntityType::Mesh(mesh)).unwrap();
    let mut light = crate::entities::Light::new();
    light.position = Vector3::new(5.0, 5.0, 5.0);
    light.target = Vector3::new(0.0, 0.0, 0.0);
    doc.add_entity(EntityType::Light(Box::new(light))).unwrap();
    let scene = compile(&doc, &CompileOptions::default());
    assert!(scene.fallbacks.is_empty());
    assert!(!super::FALLBACK_KINDS
        .iter()
        .any(|kind| { matches!(*kind, "Mesh" | "Light" | "Solid3D" | "Camera") }));
    assert_eq!(scene.fills[0].triangles.len(), 6);
    assert!(scene.fills[0]
        .normals
        .iter()
        .any(|normal| normal.z.abs() > 0.2));
    assert!(scene.strokes.len() >= 4);
}

#[test]
fn closer_face_wins_the_depth_test() {
    let mut doc = doc();
    let mut near = crate::entities::Mesh::new();
    near.common.color = Color::RED;
    near.vertices = vec![
        Vector3::new(-8.0, -8.0, 4.0),
        Vector3::new(8.0, -8.0, 4.0),
        Vector3::new(8.0, 8.0, 4.0),
        Vector3::new(-8.0, 8.0, 4.0),
    ];
    near.faces.push(crate::entities::MeshFace::quad(0, 1, 2, 3));
    let mut far = crate::entities::Mesh::new();
    far.common.color = Color::from_index(5);
    far.vertices = vec![
        Vector3::new(-8.0, -8.0, 0.0),
        Vector3::new(8.0, -8.0, 0.0),
        Vector3::new(8.0, 8.0, 0.0),
        Vector3::new(-8.0, 8.0, 0.0),
    ];
    far.faces.push(crate::entities::MeshFace::quad(0, 1, 2, 3));
    doc.add_entity(EntityType::Mesh(near)).unwrap();
    doc.add_entity(EntityType::Mesh(far)).unwrap();
    let image = match pollster::block_on(super::render_rgba(
        &doc,
        &View {
            target: Vector3::ZERO,
            height: 20.0,
            ..View::default()
        },
        48,
        48,
        &CompileOptions::default(),
    )) {
        Err(super::RenderError::NoAdapter) => return,
        Err(error) => panic!("{error}"),
        Ok(image) => image,
    };
    let center = ((24 * image.width + 24) * 4) as usize;
    let pixel = &image.pixels[center..center + 3];
    assert!(
        pixel[0] > pixel[2] + 40,
        "the higher face should cover the lower one, pixel {pixel:?}"
    );
}

#[test]
fn arc_text_characters_follow_the_circle() {
    let anchors = super::compile::arc_anchors(
        Vector3::ZERO,
        10.0,
        0.0,
        &[std::f64::consts::PI * 5.0, std::f64::consts::PI * 5.0],
        false,
    );
    assert_eq!(anchors.len(), 2);
    assert!((anchors[0].position.x - 10.0 * (std::f64::consts::FRAC_PI_4).cos()).abs() < 1e-6);
    assert!((anchors[0].position.y - 10.0 * (std::f64::consts::FRAC_PI_4).sin()).abs() < 1e-6);
    assert!((anchors[1].position.y - anchors[0].position.x).abs() < 1e-5);
}

#[test]
fn a_bmp_image_keeps_its_pixel_and_its_frame() {
    let dir = std::env::temp_dir().join(format!("acadrust-image-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("swatch.bmp");
    let mut bytes = vec![0u8; 54 + 4];
    bytes[0..2].copy_from_slice(b"BM");
    bytes[2..6].copy_from_slice(&58u32.to_le_bytes());
    bytes[10..14].copy_from_slice(&54u32.to_le_bytes());
    bytes[14..18].copy_from_slice(&40u32.to_le_bytes());
    bytes[18..22].copy_from_slice(&1u32.to_le_bytes());
    bytes[22..26].copy_from_slice(&1u32.to_le_bytes());
    bytes[26..28].copy_from_slice(&1u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&24u16.to_le_bytes());
    bytes[54] = 0;
    bytes[55] = 0;
    bytes[56] = 255;
    std::fs::write(&path, &bytes).unwrap();
    let mut doc = doc();
    let image = crate::entities::RasterImage::with_size(
        path.to_str().unwrap(),
        Vector3::ZERO,
        1.0,
        1.0,
        8.0,
        4.0,
    );
    doc.add_entity(EntityType::RasterImage(image)).unwrap();
    let mut options = CompileOptions::default();
    options.resource_dirs.push(dir.clone());
    let scene = compile(&doc, &options);
    assert!(scene.fallbacks.is_empty());
    assert_eq!(scene.pictures.len(), 1);
    assert_eq!(&scene.pictures[0].rgba[0..3], &[255, 0, 0]);
    assert!((scene.pictures[0].triangles[1].x - 8.0).abs() < 1e-6);
    let absent = crate::entities::RasterImage::new("missing-photo.png", Vector3::ZERO, 4.0, 4.0);
    let mut bare = CadDocument::new();
    bare.add_entity(EntityType::RasterImage(absent)).unwrap();
    let missing_scene = compile(&bare, &CompileOptions::default());
    assert!(missing_scene.pictures.is_empty());
    assert!(missing_scene
        .skipped
        .iter()
        .any(|entry| entry.reason.contains("was not found")));
    assert!(!missing_scene.strokes.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn pdf_underlay_draws_the_page_line_in_paper_coordinates() {
    let dir = std::env::temp_dir().join(format!("acadrust-pdf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("sheet.pdf");
    std::fs::write(
        &path,
        b"%PDF-1.4\n\
1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n\
2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj\n\
3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R >> endobj\n\
4 0 obj << /Length 28 >> stream\n\
10 10 m 40 10 l S\n\
endstream\n\
endobj\n\
trailer << /Root 1 0 R >>\n",
    )
    .unwrap();
    let mut doc = doc();
    let mut definition = crate::entities::UnderlayDefinition::pdf(path.to_str().unwrap(), "1");
    definition.handle = doc.allocate_handle();
    let handle = definition.handle;
    doc.objects.insert(
        handle,
        crate::objects::ObjectType::UnderlayDefinition(definition),
    );
    let mut underlay = crate::entities::Underlay::pdf();
    underlay.definition_handle = handle;
    underlay.insertion_point = Vector3::new(5.0, 6.0, 0.0);
    doc.add_entity(EntityType::Underlay(underlay)).unwrap();
    let mut options = CompileOptions::default();
    options.resource_dirs.push(dir.clone());
    let scene = compile(&doc, &options);
    assert!(scene.fallbacks.is_empty());
    let line = scene
        .strokes
        .iter()
        .find(|stroke| stroke.points.len() == 2)
        .expect("pdf line");
    assert!((line.points[0].x - 15.0).abs() < 1e-4 && (line.points[0].y - 16.0).abs() < 1e-4);
    assert!((line.points[1].x - 45.0).abs() < 1e-4 && (line.points[1].y - 16.0).abs() < 1e-4);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn geo_marker_draws_its_circle() {
    let mut doc = doc();
    doc.add_entity(EntityType::Extended(Box::new(
        crate::entities::ExtendedEntity {
            common: crate::entities::EntityCommon::default(),
            data: crate::entities::ExtendedEntityData::GeoPositionMarker(
                crate::entities::GeoPositionMarkerData {
                    class_version: 0,
                    position: Vector3::new(3.0, 4.0, 0.0),
                    radius: 2.0,
                    notes: String::new(),
                    landing_gap: 0.0,
                    mtext_visible: false,
                    text_alignment: 0,
                    enable_frame_text: false,
                    embedded_mtext: None,
                },
            ),
        },
    )))
    .unwrap();
    let scene = compile(&doc, &CompileOptions::default());
    assert!(scene.fallbacks.is_empty());
    assert!(scene.strokes[0].points.len() > 8);
    assert!(scene.strokes[0]
        .points
        .iter()
        .any(|point| { ((point.x - 5.0).abs() < 1e-4) && ((point.y - 4.0).abs() < 1e-4) }));
}

#[test]
fn dwf_line_is_placed_at_the_underlay_insertion() {
    let dir = std::env::temp_dir().join(format!("acadrust-dwf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("sheet.dwf");
    std::fs::write(&path, b"(DWF V06.00)\n(Line 1,2 4,2)\n").unwrap();
    let mut doc = doc();
    let mut definition = crate::entities::UnderlayDefinition::dwf(path.to_str().unwrap(), "");
    definition.handle = doc.allocate_handle();
    let handle = definition.handle;
    doc.objects.insert(
        handle,
        crate::objects::ObjectType::UnderlayDefinition(definition),
    );
    let mut underlay = crate::entities::Underlay::dwf();
    underlay.definition_handle = handle;
    underlay.insertion_point = Vector3::new(10.0, 20.0, 0.0);
    doc.add_entity(EntityType::Underlay(underlay)).unwrap();
    let mut options = CompileOptions::default();
    options.resource_dirs.push(dir.clone());
    let scene = compile(&doc, &options);
    assert!(scene.fallbacks.is_empty());
    let line = scene
        .strokes
        .iter()
        .find(|stroke| stroke.points.len() == 2)
        .expect("dwf line");
    assert!((line.points[0].x - 11.0).abs() < 1e-6 && (line.points[0].y - 22.0).abs() < 1e-6);
    assert!((line.points[1].x - 14.0).abs() < 1e-6 && (line.points[1].y - 22.0).abs() < 1e-6);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn point_cloud_draws_its_stored_extents() {
    let mut doc = doc();
    doc.add_entity(EntityType::Extended(Box::new(
        crate::entities::ExtendedEntity {
            common: crate::entities::EntityCommon::default(),
            data: crate::entities::ExtendedEntityData::PointCloud(
                crate::entities::PointCloudData {
                    class_version: 0,
                    origin: Vector3::ZERO,
                    saved_filename: String::new(),
                    source_files: Vec::new(),
                    extents_min: Vector3::new(1.0, 2.0, 3.0),
                    extents_max: Vector3::new(4.0, 5.0, 6.0),
                    point_count: 0,
                    ucs_name: String::new(),
                    ucs_origin: Vector3::ZERO,
                    ucs_x_direction: Vector3::UNIT_X,
                    ucs_y_direction: Vector3::UNIT_Y,
                    ucs_z_direction: Vector3::UNIT_Z,
                    definition_handle: Handle::NULL,
                    reactor_handle: Handle::NULL,
                    show_intensity: false,
                    intensity_scheme: 0,
                    minimum_intensity: 0.0,
                    maximum_intensity: 0.0,
                    low_intensity_threshold: 0.0,
                    high_intensity_threshold: 0.0,
                    show_clipping: false,
                    clippings: Vec::new(),
                },
            ),
        },
    )))
    .unwrap();
    let scene = compile(&doc, &CompileOptions::default());
    assert!(scene.fallbacks.is_empty());
    assert_eq!(scene.strokes.len(), 12);
    assert!(scene.strokes.iter().any(|stroke| {
        stroke.points.contains(&Vector3::new(1.0, 2.0, 3.0))
            && stroke.points.contains(&Vector3::new(4.0, 2.0, 3.0))
    }));
}

#[test]
fn coordination_model_draws_the_definition_box() {
    let mut doc = doc();
    let mut definition = crate::objects::ClassObject::new(
        crate::objects::ClassObjectData::NavisworksModelDefinition(
            crate::objects::NavisworksModelDefinition {
                extents_min: Vector3::ZERO,
                extents_max: Vector3::new(2.0, 2.0, 2.0),
                ..crate::objects::NavisworksModelDefinition::default()
            },
        ),
    );
    definition.handle = doc.allocate_handle();
    let handle = definition.handle;
    doc.objects
        .insert(handle, crate::objects::ObjectType::ClassObject(definition));
    doc.add_entity(EntityType::Extended(Box::new(
        crate::entities::ExtendedEntity {
            common: crate::entities::EntityCommon::default(),
            data: crate::entities::ExtendedEntityData::CoordinationModel(
                crate::entities::CoordinationModelData {
                    flags: 0,
                    definition_handle: handle,
                    transform: [
                        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0,
                        1.0,
                    ],
                    unit_factor: 1.0,
                },
            ),
        },
    )))
    .unwrap();
    let scene = compile(&doc, &CompileOptions::default());
    assert!(scene.fallbacks.is_empty());
    assert!(scene
        .strokes
        .iter()
        .any(|stroke| stroke.points.contains(&Vector3::new(2.0, 2.0, 2.0))));
}

#[test]
fn arch_profile_and_proxy_polyline_are_dedicated() {
    let mut doc = doc();
    doc.add_entity(EntityType::Extended(Box::new(
        crate::entities::ExtendedEntity {
            common: crate::entities::EntityCommon::default(),
            data: crate::entities::ExtendedEntityData::Arch(crate::classes::arch::ArchEntityData {
                dxf_name: "AEC_WALL".to_string(),
                cpp_class_name: String::new(),
                kind: crate::classes::arch::ArchKind::Wall,
                profiles: vec![vec![Vector3::ZERO, Vector3::new(4.0, 0.0, 0.0)]],
                dwg_type_code: 0,
                dwg_handle_bits: 0,
                dwg_source_version: None,
                raw_dwg_data: None,
                raw_dxf_codes: None,
                fields: Vec::new(),
                preview: Vec::new(),
            }),
        },
    )))
    .unwrap();
    let mut payload = Vec::new();
    payload.extend_from_slice(&2u32.to_le_bytes());
    for value in [0.0_f64, 1.0, 0.0, 6.0, 1.0, 0.0] {
        payload.extend_from_slice(&value.to_le_bytes());
    }
    let graphics = crate::entities::ProxyGraphics {
        records: vec![crate::entities::ProxyGraphicRecord::Unknown {
            record_type: 6,
            data: payload,
        }],
    };
    let mut unknown = crate::entities::UnknownEntity::new("CUSTOM");
    unknown.common.graphic_data = graphics.encode();
    doc.add_entity(EntityType::Unknown(unknown)).unwrap();
    let scene = compile(&doc, &CompileOptions::default());
    assert!(scene.fallbacks.is_empty());
    assert!(scene.strokes.iter().any(|stroke| {
        stroke.points.first() == Some(&Vector3::ZERO)
            && stroke.points.last() == Some(&Vector3::new(4.0, 0.0, 0.0))
    }));
    assert!(scene.strokes.iter().any(|stroke| {
        stroke.points.first() == Some(&Vector3::new(0.0, 1.0, 0.0))
            && stroke.points.last() == Some(&Vector3::new(6.0, 1.0, 0.0))
    }));
}
