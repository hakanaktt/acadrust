//! One spatially separated entity atlas in every supported DWG/DXF encoding.
//! cargo run --example entity_atlas -- [output directory]

use std::collections::BTreeMap;
use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::io::Cursor;
use std::path::{Path, PathBuf};

use acadrust::entities::acis::primitives;
use acadrust::entities::*;
use acadrust::objects::{
    Dictionary, ImageDefinition as ImageDef, ImageDefinitionReactor, ObjectType,
};
use acadrust::tables::{Layer, TextStyle, View};
use acadrust::types::{Color, DxfVersion, Handle, Vector2, Vector3};
use acadrust::{BlockRecord, CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};
use serde_json::{json, Value};

const VERSIONS: [DxfVersion; 8] = [
    DxfVersion::AC1012,
    DxfVersion::AC1014,
    DxfVersion::AC1015,
    DxfVersion::AC1018,
    DxfVersion::AC1021,
    DxfVersion::AC1024,
    DxfVersion::AC1027,
    DxfVersion::AC1032,
];
const COLUMNS: usize = 7;
const WIDTH: f64 = 150.0;
const HEIGHT: f64 = 100.0;

fn p(x: f64, y: f64) -> Vector3 {
    Vector3::new(x, y, 0.)
}
fn xy() -> Vec<Vector2> {
    vec![
        Vector2::new(20., 20.),
        Vector2::new(75., 20.),
        Vector2::new(65., 60.),
        Vector2::new(20., 55.),
    ]
}
fn polygon_sat() -> String {
    include_str!("entity_atlas_assets/region.sat").to_owned()
}

struct Sheet {
    doc: CadDocument,
    cases: Vec<Value>,
    assets: PathBuf,
}

impl Sheet {
    fn new(version: DxfVersion, assets: &Path) -> Self {
        let mut doc = CadDocument::with_version(version);
        for (name, color) in [("_ATLAS_LABELS", 7), ("_ATLAS_GRID", 8)] {
            let mut layer = Layer::new(name);
            layer.handle = doc.allocate_handle();
            layer.color = Color::Index(color);
            doc.layers.add(layer).unwrap();
        }
        Self {
            doc,
            cases: Vec::new(),
            assets: assets.to_path_buf(),
        }
    }
    fn text(&mut self, text: &str, at: Vector3, height: f64) {
        let mut e = Text::with_value(text, at).with_height(height);
        e.common.layer = "_ATLAS_LABELS".into();
        self.doc.add_entity(EntityType::Text(e)).unwrap();
    }
    fn slot(&mut self, name: &str, minimum: DxfVersion, note: &str) -> (String, Vector3, bool) {
        let i = self.cases.len();
        let layer = format!("E{:03}_{name}", i + 1);
        let origin = if name == "RAY" {
            p(0., -1200.)
        } else if name == "XLINE" {
            p(0., -1300.)
        } else {
            p(
                (i % COLUMNS) as f64 * WIDTH,
                -((i / COLUMNS) as f64) * HEIGHT,
            )
        };
        let included = self.doc.version >= minimum;
        let mut l = Layer::new(&layer);
        l.handle = self.doc.allocate_handle();
        l.color = Color::Index((i % 6 + 1) as u8);
        self.doc.layers.add(l).unwrap();
        self.text(&format!("{:03} {name}", i + 1), origin + p(5., 82.), 3.);
        if !included {
            self.text(
                &format!("Requires {}", minimum.as_str()),
                origin + p(5., 68.),
                2.5,
            );
        }
        for (a, b) in [
            (p(0., 0.), p(140., 0.)),
            (p(140., 0.), p(140., 94.)),
            (p(140., 94.), p(0., 94.)),
            (p(0., 94.), p(0., 0.)),
        ] {
            let mut e = Line::from_points(origin + a, origin + b);
            e.common.layer = "_ATLAS_GRID".into();
            self.doc.add_entity(EntityType::Line(e)).unwrap();
        }
        self.cases.push(json!({"id":i+1,"name":name,"layer":layer,"x":origin.x,"y":origin.y,"minimum_version":minimum.as_str(),"included":included,"note":note,"expected":[]}));
        (layer, origin, included)
    }
    fn add(&mut self, name: &str, minimum: DxfVersion, mut entity: EntityType) {
        let (layer, origin, included) = self.slot(name, minimum, "");
        if included {
            if !std::env::args().any(|arg| arg == "--untranslated") {
                entity.as_entity_mut().translate(origin);
            }
            entity.common_mut().layer = layer.clone();
            match &mut entity {
                EntityType::Polyline3D(polyline) => {
                    for vertex in &mut polyline.vertices {
                        vertex.layer = layer.clone();
                    }
                }
                EntityType::PolygonMesh(mesh) => {
                    for vertex in &mut mesh.vertices {
                        vertex.common.layer = layer.clone();
                    }
                }
                EntityType::PolyfaceMesh(mesh) => {
                    for vertex in &mut mesh.vertices {
                        vertex.common.layer = layer.clone();
                    }
                    for face in &mut mesh.faces {
                        face.common.layer = layer.clone();
                    }
                }
                EntityType::Leader(leader) => {
                    leader.creation_type = LeaderCreationType::NoAnnotation
                }
                EntityType::MultiLeader(leader) => {
                    leader.style_handle = self.doc.objects.iter().find_map(|(h, o)| {
                        matches!(o, ObjectType::MultiLeaderStyle(_)).then_some(*h)
                    });
                    leader.text_style_handle = Some(self.doc.header.current_text_style_handle);
                    leader.context.text_style_handle =
                        Some(self.doc.header.current_text_style_handle);
                }
                EntityType::Table(table) => {
                    table.table_style_handle =
                        self.doc.objects.iter().find_map(|(h, o)| {
                            matches!(o, ObjectType::TableStyle(_)).then_some(*h)
                        });
                }
                _ => {}
            }
            let kind = entity.as_entity().entity_type().to_owned();
            let handle = self.doc.add_entity(entity).unwrap();
            let entry = self.cases.last_mut().unwrap();
            entry["expected"] = json!([kind]);
            entry["handle"] = json!(format!("{:X}", handle.value()));
        }
    }
    fn dictionary(&mut self, name: &str) -> Handle {
        let root = self.doc.header.named_objects_dict_handle;
        let h = self.doc.allocate_handle();
        let mut d = Dictionary::new();
        d.handle = h;
        d.owner = root;
        self.doc.objects.insert(h, ObjectType::Dictionary(d));
        if let Some(ObjectType::Dictionary(d)) = self.doc.objects.get_mut(&root) {
            d.add_entry(name, h);
        }
        h
    }
    fn block(&mut self, name: &str, with_attribute: bool, array: bool) {
        let (layer, origin, _) = self.slot(
            name,
            DxfVersion::AC1012,
            "BLOCK/ENDBLK and ATTRIB/SEQEND are stored under their valid owners",
        );
        let block_name = format!("ATLAS_{name}");
        let mut block = BlockRecord::new(&block_name);
        block.handle = self.doc.allocate_handle();
        let owner = block.handle;
        block.block_entity_handle = self.doc.allocate_handle();
        block.block_end_handle = self.doc.allocate_handle();
        self.doc.block_records.add(block).unwrap();
        let mut circle = Circle::from_coords(0., 0., 0., 8.);
        circle.common.owner_handle = owner;
        circle.common.layer = layer.clone();
        self.doc.add_entity(EntityType::Circle(circle)).unwrap();
        let mut attdef = AttributeDefinition::new("ID".into(), "ID".into(), "DEF".into());
        attdef.common.owner_handle = owner;
        attdef.common.layer = layer.clone();
        attdef.insertion_point = p(-7., -15.);
        let mut attdef_handle = Handle::NULL;
        if with_attribute {
            attdef_handle = self
                .doc
                .add_entity(EntityType::AttributeDefinition(attdef))
                .unwrap();
        }
        let mut insert = Insert::new(&block_name, origin + p(30., 45.));
        insert.common.layer = layer.clone();
        if array {
            insert = insert.with_array(2, 2, 25., 25.);
        }
        if with_attribute {
            let mut attr = AttributeEntity::new("ID".into(), "ATTRIB".into());
            attr.insertion_point = origin + p(23., 30.);
            attr.alignment_point = attr.insertion_point;
            attr.common.layer = layer;
            attr.attdef_handle = attdef_handle;
            insert.attributes.push(attr);
        }
        let handle = self.doc.add_entity(EntityType::Insert(insert)).unwrap();
        self.cases.last_mut().unwrap()["expected"] =
            json!([if array { "MINSERT" } else { "INSERT" }]);
        self.cases.last_mut().unwrap()["handle"] = json!(format!("{:X}", handle.value()));
    }
}

fn build(version: DxfVersion, assets: &Path) -> Sheet {
    use DxfVersion::*;
    let mut s = Sheet::new(version, assets);
    s.add(
        "POINT",
        AC1012,
        EntityType::Point(Point::from_coords(45., 40., 0.)),
    );
    s.add(
        "LINE",
        AC1012,
        EntityType::Line(Line::from_points(p(20., 20.), p(90., 60.))),
    );
    s.add(
        "CIRCLE",
        AC1012,
        EntityType::Circle(Circle::from_coords(50., 40., 0., 22.)),
    );
    s.add(
        "ARC",
        AC1012,
        EntityType::Arc(Arc::from_coords(50., 40., 0., 25., 0., PI * 1.5)),
    );
    s.add(
        "ELLIPSE",
        AC1012,
        EntityType::Ellipse(Ellipse::from_center_axes(p(55., 40.), p(35., 0.), 0.5)),
    );
    let mut lw = LwPolyline::from_points(xy());
    lw.close();
    lw.vertices[0].bulge = 0.4;
    s.add("LWPOLYLINE", AC1014, EntityType::LwPolyline(lw));
    let mut poly = Polyline::new();
    for v in [p(20., 20.), p(60., 30.), Vector3::new(90., 65., 15.)] {
        poly.add_vertex(Vertex3D::new(v));
    }
    s.add("POLYLINE", AC1012, EntityType::Polyline(poly));
    let mut pl = Polyline2D::new();
    for v in [p(20., 20.), p(60., 30.), p(90., 65.)] {
        pl.add_vertex(Vertex2D::new(v));
    }
    s.add("POLYLINE2D", AC1012, EntityType::Polyline2D(pl));
    s.add(
        "POLYLINE3D",
        AC1012,
        EntityType::Polyline3D(Polyline3D::from_points(vec![
            p(20., 20.),
            Vector3::new(60., 30., 20.),
            p(90., 65.),
        ])),
    );
    s.add(
        "SPLINE",
        AC1012,
        EntityType::Spline(Spline::from_control_points(
            3,
            vec![p(20., 20.), p(35., 65.), p(75., 15.), p(100., 55.)],
        )),
    );
    let mut helix = Helix::new();
    helix.axis_base_point = p(50., 40.);
    helix.start_point = p(70., 40.);
    helix.radius = 20.;
    helix.turns = 3.;
    helix.turn_height = 5.;
    let points = (0..=72)
        .map(|i| {
            let t = i as f64 / 72. * 3. * TAU;
            Vector3::new(50. + 20. * t.cos(), 40. + 20. * t.sin(), t / TAU * 5.)
        })
        .collect();
    helix.spline = Spline::from_control_points(1, points);
    s.add("HELIX", AC1021, EntityType::Helix(helix));
    s.add(
        "TEXT",
        AC1012,
        EntityType::Text(Text::with_value("CAD 012345", p(12., 40.)).with_height(7.)),
    );
    let mut mt = MText::with_value("Text\\P{\\C1;Red} and {\\C3;green}", p(12., 65.));
    mt.height = 4.;
    mt.rectangle_width = 110.;
    s.add("MTEXT", AC1012, EntityType::MText(mt));
    let mut ad = AttributeDefinition::constant("TAG", "ATTDEF");
    ad.insertion_point = p(20., 40.);
    ad.height = 5.;
    s.add("ATTDEF", AC1012, EntityType::AttributeDefinition(ad));
    s.block("INSERT", false, false);
    s.block("MINSERT", false, true);
    s.block("ATTRIB", true, false);
    let mut d = DimensionLinear::new(p(20., 25.), p(90., 25.));
    d.definition_point = p(20., 55.);
    s.add(
        "DIM_LINEAR",
        AC1012,
        EntityType::Dimension(Dimension::Linear(d)),
    );
    let mut d = DimensionAligned::new(p(20., 20.), p(80., 40.));
    d.set_offset(20.);
    s.add(
        "DIM_ALIGNED",
        AC1012,
        EntityType::Dimension(Dimension::Aligned(d)),
    );
    s.add(
        "DIM_RADIUS",
        AC1012,
        EntityType::Dimension(Dimension::Radius(DimensionRadius::new(
            p(50., 35.),
            p(75., 55.),
        ))),
    );
    s.add(
        "DIM_DIAMETER",
        AC1012,
        EntityType::Dimension(Dimension::Diameter(DimensionDiameter::new(
            p(25., 30.),
            p(85., 60.),
        ))),
    );
    s.add(
        "DIM_ANGULAR2",
        AC1012,
        EntityType::Dimension(Dimension::Angular2Ln(DimensionAngular2Ln::new(
            p(30., 25.),
            p(95., 25.),
            p(65., 65.),
        ))),
    );
    s.add(
        "DIM_ANGULAR3",
        AC1012,
        EntityType::Dimension(Dimension::Angular3Pt(DimensionAngular3Pt::new(
            p(30., 25.),
            p(95., 25.),
            p(65., 65.),
        ))),
    );
    s.add(
        "DIM_ORDINATE",
        AC1012,
        EntityType::Dimension(Dimension::Ordinate(DimensionOrdinate::new(
            p(35., 25.),
            p(85., 60.),
            true,
        ))),
    );
    let d = DimensionArc {
        center_point: p(45., 30.),
        first_extension_point: p(75., 30.),
        second_extension_point: p(45., 60.),
        definition_point: p(82., 67.),
        arc_end_parameter: FRAC_PI_2,
        ..Default::default()
    };
    s.add("DIM_ARC", AC1018, EntityType::Dimension(Dimension::Arc(d)));
    let d = DimensionLargeRadial {
        definition_point: p(20., 25.),
        chord_point: p(100., 60.),
        override_center: p(40., 30.),
        jog_point: p(65., 50.),
        jog_angle: PI / 4.,
        ..Default::default()
    };
    s.add(
        "DIM_JOGGED",
        AC1018,
        EntityType::Dimension(Dimension::LargeRadial(d)),
    );
    s.add(
        "SOLID",
        AC1012,
        EntityType::Solid(Solid::new(
            p(20., 20.),
            p(85., 20.),
            p(20., 60.),
            p(70., 65.),
        )),
    );
    s.add(
        "3DFACE",
        AC1012,
        EntityType::Face3D(Face3D::new(
            p(20., 20.),
            p(85., 20.),
            Vector3::new(70., 65., 15.),
            p(20., 60.),
        )),
    );
    for (name, pattern) in [("HATCH_SOLID", false), ("HATCH_PATTERN", true)] {
        let mut hatch = Hatch::solid();
        let mut path = BoundaryPath::new();
        path.add_edge(BoundaryEdge::Polyline(PolylineEdge::new(xy(), true)));
        hatch.add_path(path);
        if pattern {
            hatch.is_solid = false;
            hatch.pattern.name = "ANSI31".into();
            hatch.pattern_scale = 3.;
            hatch.pattern.add_line(HatchPatternLine {
                angle: PI / 4.,
                base_point: Vector2::new(0., 0.),
                offset: Vector2::new(-3., 3.),
                dash_lengths: vec![],
            });
        }
        s.add(name, AC1012, EntityType::Hatch(hatch));
    }
    s.add(
        "LEADER",
        AC1012,
        EntityType::Leader(Leader::from_vertices(vec![
            p(20., 20.),
            p(50., 55.),
            p(90., 55.),
        ])),
    );
    s.add(
        "MULTILEADER",
        AC1021,
        EntityType::MultiLeader(MultiLeader::with_text(
            "Leader",
            p(70., 55.),
            vec![p(20., 20.), p(50., 55.)],
        )),
    );
    s.add(
        "MLINE",
        AC1012,
        EntityType::MLine(MLine::from_points(&[p(20., 20.), p(65., 20.), p(95., 65.)])),
    );
    s.add(
        "TOLERANCE",
        AC1012,
        EntityType::Tolerance(Tolerance::with_text(p(15., 40.), "{\\Fgdt;p}%%v0.5")),
    );
    let mut pf = PolyfaceMesh::new();
    let a = pf.add_vertex_xyz(20., 20., 0.);
    let b = pf.add_vertex_xyz(90., 20., 0.);
    let c = pf.add_vertex_xyz(55., 65., 20.);
    pf.add_triangle(a, b, c);
    s.add("POLYFACE", AC1012, EntityType::PolyfaceMesh(pf));
    let mut pm = PolygonMeshEntity::new();
    pm.m_vertex_count = 2;
    pm.n_vertex_count = 2;
    pm.vertices = vec![
        p(20., 20.),
        p(20., 60.),
        p(80., 20.),
        Vector3::new(80., 60., 20.),
    ]
    .into_iter()
    .map(PolygonMeshVertex::at)
    .collect();
    s.add("POLYGON_MESH", AC1012, EntityType::PolygonMesh(pm));
    s.add(
        "MESH",
        AC1024,
        EntityType::Mesh(Mesh::from_triangles(
            vec![p(20., 20.), p(90., 20.), Vector3::new(55., 65., 20.)],
            &[(0, 1, 2)],
        )),
    );
    s.add(
        "WIPEOUT",
        AC1014,
        EntityType::Wipeout(Wipeout::rectangular(p(25., 25.), 60., 35.)),
    );
    let mut table = Table::new(p(15., 65.), 3, 2);
    for column in &mut table.columns {
        column.width = 45.;
    }
    for row in &mut table.rows {
        row.height = 13.;
    }
    for (i, row) in table.rows.iter_mut().enumerate() {
        row.cells[0] = TableCell::text(&format!("Row {i}"));
        row.cells[1] = TableCell::text(&format!("{}", i * 10));
    }
    s.add("TABLE", AC1018, EntityType::Table(table));
    for (name, sat) in [
        (
            "3DSOLID_BOX",
            primitives::build_box([50., 40., 10.], 45., 30., 20.),
        ),
        (
            "3DSOLID_CYLINDER",
            primitives::build_cylinder([50., 40., 0.], 20., 30.),
        ),
        (
            "3DSOLID_CONE",
            primitives::build_cone([50., 40., 0.], 20., 30.),
        ),
        (
            "3DSOLID_SPHERE",
            primitives::build_sphere([50., 40., 20.], 20.),
        ),
        (
            "3DSOLID_TORUS",
            primitives::build_torus([50., 40., 10.], 22., 6.),
        ),
        (
            "3DSOLID_WEDGE",
            primitives::build_wedge([20., 20., 0.], 60., 35., 25.),
        ),
        (
            "3DSOLID_PYRAMID",
            primitives::build_pyramid([50., 40., 0.], 45., 30.),
        ),
    ] {
        s.add(
            name,
            AC1012,
            EntityType::Solid3D(Solid3D::from_sat(&sat.to_sat_string())),
        );
    }
    s.add(
        "REGION",
        AC1012,
        EntityType::Region(Region::from_sat(&polygon_sat())),
    );
    s.add(
        "BODY",
        AC1012,
        EntityType::Body(Body::from_sat(&polygon_sat())),
    );
    for kind in [SurfaceKind::Generic, SurfaceKind::Plane, SurfaceKind::Nurb] {
        let mut surface = Surface::new(kind);
        surface.acis_data = Region::from_sat(&polygon_sat()).acis_data;
        s.add(
            &format!("SURFACE_{kind:?}").to_uppercase(),
            AC1021,
            EntityType::Surface(surface),
        );
    }
    for (name, kind) in [("LIGHT_POINT", 2), ("LIGHT_SPOT", 3), ("LIGHT_DISTANT", 1)] {
        let mut light = Light::new();
        light.name = name.into();
        light.position = Vector3::new(40., 30., 20.);
        light.target = p(75., 60.);
        light.light_type = kind;
        light.plot_glyph = true;
        light.hotspot_angle = PI / 6.;
        light.falloff_angle = PI / 4.;
        s.add(name, AC1021, EntityType::Light(light));
    }
    let mut style = TextStyle::new("ATLAS_SHAPES");
    style.handle = s.doc.allocate_handle();
    style.is_shape_file = true;
    style.font_file = s.assets.join("ltypeshp.shx").to_string_lossy().into_owned();
    let sh = style.handle;
    s.doc.text_styles.add(style).unwrap();
    let mut shape = Shape::with_number(p(45., 40.), 132, 15.);
    shape.shape_name = "BOX".into();
    shape.style_name = "ATLAS_SHAPES".into();
    shape.style_handle = Some(sh);
    s.add("SHAPE", AC1012, EntityType::Shape(shape));
    let (layer, origin, included) = s.slot("IMAGE", AC1014, "Uses assets/checker.bmp");
    if included {
        let dictionary = s.dictionary("ACAD_IMAGE_DICT");
        let mut def = ImageDef::new(s.assets.join("checker.bmp").to_string_lossy());
        def.handle = s.doc.allocate_handle();
        def.owner = dictionary;
        def.size_in_pixels = (64, 64);
        def.pixel_size = (1., 1.);
        def.is_loaded = true;
        let dh = def.handle;
        s.doc.objects.insert(dh, ObjectType::ImageDefinition(def));
        if let Some(ObjectType::Dictionary(d)) = s.doc.objects.get_mut(&dictionary) {
            d.add_entry("CHECKER", dh);
        }
        let mut image = RasterImage::with_size(
            &s.assets.join("checker.bmp").to_string_lossy(),
            origin + p(25., 20.),
            64.,
            64.,
            60.,
            50.,
        );
        image.common.layer = layer;
        image.definition_handle = Some(dh);
        let ih = s.doc.allocate_handle();
        image.common.handle = ih;
        let mut reactor = ImageDefinitionReactor::new(ih);
        reactor.handle = s.doc.allocate_handle();
        reactor.owner = ih;
        image.definition_reactor_handle = Some(reactor.handle);
        s.doc
            .objects
            .insert(reactor.handle, ObjectType::ImageDefinitionReactor(reactor));
        s.doc.add_entity(EntityType::RasterImage(image)).unwrap();
        s.cases.last_mut().unwrap()["expected"] = json!(["IMAGE"]);
    }
    for (name, kind, file, minimum) in [
        ("PDFUNDERLAY", UnderlayType::Pdf, "reference.pdf", AC1024),
        ("DWFUNDERLAY", UnderlayType::Dwf, "reference.dwf", AC1021),
        ("DGNUNDERLAY", UnderlayType::Dgn, "reference.dgn", AC1021),
    ] {
        let (layer, origin, included) = s.slot(
            name,
            minimum,
            "External dependency recorded in assets manifest",
        );
        if included {
            let dictionary = s.dictionary(&format!(
                "ACAD_{}DEFINITIONS",
                name.trim_end_matches("UNDERLAY")
            ));
            let mut def = UnderlayDefinition::new(kind);
            def.handle = s.doc.allocate_handle();
            def.owner_handle = dictionary;
            def.file_path = s.assets.join(file).to_string_lossy().into();
            def.page_name = if kind == UnderlayType::Pdf {
                "1"
            } else {
                "Model"
            }
            .into();
            let dh = def.handle;
            s.doc
                .objects
                .insert(dh, ObjectType::UnderlayDefinition(def));
            if let Some(ObjectType::Dictionary(d)) = s.doc.objects.get_mut(&dictionary) {
                d.add_entry("REFERENCE", dh);
            }
            let mut u = Underlay::at_point(kind, origin + p(20., 20.));
            u.set_scale(0.2);
            u.definition_handle = dh;
            u.common.layer = layer;
            s.doc.add_entity(EntityType::Underlay(u)).unwrap();
            s.cases.last_mut().unwrap()["expected"] = json!([name]);
        }
    }
    let (layer, origin, included) = s.slot("CAMERA", AC1021, "Linked to a named view");
    if included {
        let mut view = View::new("ATLAS_CAMERA");
        view.handle = s.doc.allocate_handle();
        view.target = origin + p(50., 40.);
        view.direction = Vector3::new(20., -30., 30.);
        view.height = 35.;
        view.width = 50.;
        view.perspective = true;
        let vh = view.handle;
        s.doc.views.add(view).unwrap();
        let mut common = EntityCommon::default();
        common.layer = layer;
        s.doc
            .add_entity(EntityType::Extended(ExtendedEntity {
                common,
                data: ExtendedEntityData::Camera { view_handle: vh },
            }))
            .unwrap();
        s.cases.last_mut().unwrap()["expected"] = json!(["CAMERA"]);
    }
    s.add(
        "SECTIONOBJECT",
        AC1021,
        EntityType::Extended(ExtendedEntity {
            common: Default::default(),
            data: ExtendedEntityData::SectionObject(SectionObjectData {
                state: 1,
                flags: 0,
                name: "Atlas section".into(),
                vertical_direction: Vector3::UNIT_Z,
                top_height: 20.,
                bottom_height: 0.,
                indicator_alpha: 50,
                indicator_color: Color::Index(3),
                vertices: vec![p(20., 25.), p(90., 60.)],
                back_line_vertices: vec![],
                settings_handle: Handle::NULL,
            }),
        }),
    );
    s.add(
        "RTEXT",
        AC1014,
        EntityType::Extended(ExtendedEntity {
            common: Default::default(),
            data: ExtendedEntityData::RemoteText(RemoteTextData {
                position: p(20., 40.),
                normal: Vector3::UNIT_Z,
                rotation: 0.,
                height: 5.,
                style_handle: s.doc.header.current_text_style_handle,
                style_name: "Standard".into(),
                flags: 0,
                text: "Remote text".into(),
            }),
        }),
    );
    s.add(
        "POSITIONMARKER",
        AC1027,
        EntityType::Extended(ExtendedEntity {
            common: Default::default(),
            data: ExtendedEntityData::GeoPositionMarker(GeoPositionMarkerData {
                class_version: 0,
                position: p(50., 40.),
                radius: 10.,
                notes: "Marker".into(),
                landing_gap: 2.,
                mtext_visible: false,
                text_alignment: 0,
                enable_frame_text: false,
                embedded_mtext: None,
            }),
        }),
    );
    s.add(
        "ARCALIGNEDTEXT",
        AC1014,
        EntityType::Extended(ExtendedEntity {
            common: Default::default(),
            data: ExtendedEntityData::ArcAlignedText(ArcAlignedTextData {
                text: "Arc text".into(),
                font_name: "txt.shx".into(),
                big_font_name: String::new(),
                style_name: "Standard".into(),
                center: p(55., 30.),
                radius: 25.,
                x_scale: 1.,
                text_size: 4.,
                character_spacing: 1.,
                offset_from_arc: 2.,
                right_offset: 0.,
                left_offset: 0.,
                start_angle: 0.,
                end_angle: PI,
                reverse: false,
                text_direction: 0,
                alignment: 1,
                text_position: 1,
                bold: false,
                italic: false,
                underlined: false,
                character_set: 0,
                pitch_and_family: 0,
                is_shx: true,
                text_color: 7,
                normal: Vector3::UNIT_Z,
                wizard_flag: false,
                arc_handle: Handle::NULL,
            }),
        }),
    );
    let (layer, _, _) = s.slot("VIEWPORT", AC1012, "Located on Layout1 in paper space");
    let mut overall = Viewport::new();
    overall.id = 1;
    overall.common.owner_handle = s.doc.header.paper_space_block_handle;
    s.doc.add_entity(EntityType::Viewport(overall)).unwrap();
    let mut viewport = Viewport::with_size(p(100., 80.), 160., 110.);
    viewport.id = 2;
    viewport.common.layer = layer;
    viewport.common.owner_handle = s.doc.header.paper_space_block_handle;
    viewport.view_height = 1100.;
    viewport.view_target = p(500., -400.);
    s.doc.add_entity(EntityType::Viewport(viewport)).unwrap();
    s.cases.last_mut().unwrap()["expected"] = json!(["VIEWPORT"]);
    s.add(
        "RAY",
        AC1012,
        EntityType::Ray(Ray::new(p(20., 35.), Vector3::UNIT_X)),
    );
    s.add(
        "XLINE",
        AC1012,
        EntityType::XLine(XLine::new(p(50., 35.), Vector3::UNIT_X)),
    );
    s.text(
        &format!("ACADRUST ENTITY ATLAS / {}", version.as_str()),
        p(0., 125.),
        8.,
    );
    if let Some(v) = s.doc.vports.get_mut("*Active") {
        v.view_height = 1600.;
        v.view_center = Vector2::new(500., -580.);
    }
    s
}

fn checker(path: &Path) {
    let stride = 64 * 3;
    let mut bytes = vec![0u8; 54 + stride * 64];
    let len = bytes.len() as u32;
    bytes[..2].copy_from_slice(b"BM");
    bytes[2..6].copy_from_slice(&len.to_le_bytes());
    bytes[10..14].copy_from_slice(&54u32.to_le_bytes());
    bytes[14..18].copy_from_slice(&40u32.to_le_bytes());
    bytes[18..22].copy_from_slice(&64u32.to_le_bytes());
    bytes[22..26].copy_from_slice(&64u32.to_le_bytes());
    bytes[26..28].copy_from_slice(&1u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&24u16.to_le_bytes());
    for y in 0..64 {
        for x in 0..64 {
            let color = if (x / 8 + y / 8) % 2 == 0 {
                [180, 80, 20]
            } else {
                [40, 200, 240]
            };
            let at = 54 + y * stride + x * 3;
            bytes[at..at + 3].copy_from_slice(&color);
        }
    }
    std::fs::write(path, bytes).unwrap();
}

fn counts(doc: &CadDocument) -> BTreeMap<String, Vec<String>> {
    let mut result: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for e in doc.entities() {
        if e.common().layer.starts_with('E') {
            result
                .entry(e.common().layer.clone())
                .or_default()
                .push(e.as_entity().entity_type().into());
        }
    }
    result
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let minimal = std::env::args().any(|arg| arg == "--minimal");
    let exact_case = std::env::args().any(|arg| arg == "--exact-case");
    let case = std::env::args().find_map(|arg| arg.strip_prefix("--case=").map(str::to_owned));
    let only_version =
        std::env::args().find_map(|arg| arg.strip_prefix("--version=").map(str::to_owned));
    let exclusions: Vec<_> = std::env::args()
        .filter_map(|arg| arg.strip_prefix("--exclude=").map(str::to_owned))
        .collect();
    let output = std::env::args()
        .nth(1)
        .unwrap_or("target/entity-atlas".into());
    std::fs::create_dir_all(&output)?;
    let output = std::fs::canonicalize(output)?;
    let output = PathBuf::from(
        output
            .to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_owned(),
    );
    let assets = output.join("assets");
    std::fs::create_dir_all(&assets)?;
    checker(&assets.join("checker.bmp"));
    DwgWriter::write_to_file(
        output.join("validation-control.dwg"),
        &CadDocument::with_version(DxfVersion::AC1015),
    )?;
    let mut manifest = vec![];
    for version in VERSIONS {
        if only_version
            .as_ref()
            .is_some_and(|name| name != version.as_str())
        {
            continue;
        }
        let mut s = if minimal {
            let mut sheet = Sheet::new(version, &assets);
            sheet.add(
                "LINE",
                DxfVersion::AC1012,
                EntityType::Line(Line::from_points(p(20., 20.), p(90., 60.))),
            );
            sheet
        } else {
            build(version, &assets)
        };
        for excluded in &exclusions {
            let layers: Vec<_> = s
                .cases
                .iter()
                .filter(|item| {
                    item["name"]
                        .as_str()
                        .is_some_and(|name| name.starts_with(excluded))
                })
                .filter_map(|item| item["layer"].as_str().map(str::to_owned))
                .collect();
            let removed: Vec<_> = s
                .doc
                .entities()
                .filter(|entity| layers.contains(&entity.common().layer))
                .map(|entity| entity.common().handle)
                .collect();
            for handle in removed {
                s.doc.remove_entity(handle);
            }
            s.cases
                .retain(|item| !layers.iter().any(|layer| item["layer"] == *layer));
        }
        if let Some(case) = &case {
            let selected: Vec<String> = s
                .cases
                .iter()
                .filter(|item| {
                    item["name"].as_str().is_some_and(|name| {
                        if exact_case {
                            name == case
                        } else {
                            name.starts_with(case)
                        }
                    })
                })
                .filter_map(|item| item["layer"].as_str().map(str::to_owned))
                .collect();
            if selected.is_empty() {
                return Err(format!("Unknown case: {case}").into());
            }
            let removed: Vec<Handle> = s
                .doc
                .entities()
                .filter(|entity| {
                    !selected.contains(&entity.common().layer)
                        && !(case == "VIEWPORT"
                            && matches!(entity, EntityType::Viewport(viewport) if viewport.id == 1))
                })
                .map(|entity| entity.common().handle)
                .collect();
            for handle in removed {
                s.doc.remove_entity(handle);
            }
            let live: std::collections::HashSet<Handle> = s
                .doc
                .entities()
                .map(|entity| entity.common().handle)
                .collect();
            for block in s.doc.block_records.iter_mut() {
                block.entity_handles.retain(|handle| live.contains(handle));
            }
            if case != "SHAPE" {
                s.doc.text_styles.remove("ATLAS_SHAPES");
            }
            s.cases.retain(|item| {
                item["layer"]
                    .as_str()
                    .is_some_and(|layer| selected.iter().any(|value| value == layer))
            });
        }
        let directory = output.join(version.as_str());
        std::fs::create_dir_all(&directory)?;
        for format in ["dwg", "dxf_ascii", "dxf_binary"] {
            if minimal && format != "dwg" {
                continue;
            }
            let file = directory.join(format!(
                "entity_atlas_{}_{}.{}",
                version.as_str(),
                format,
                if format == "dwg" { "dwg" } else { "dxf" }
            ));
            let bytes = match format {
                "dwg" => DwgWriter::write_to_vec(&s.doc)?,
                "dxf_binary" => DxfWriter::new_binary(&s.doc).write_to_vec()?,
                _ => DxfWriter::new(&s.doc).write_to_vec()?,
            };
            std::fs::write(&file, &bytes)?;
            let read = if format == "dwg" {
                DwgReader::from_stream(Cursor::new(bytes.clone())).read()
            } else {
                DxfReader::from_reader(Cursor::new(bytes.clone())).and_then(|r| r.read())
            };
            let readback = match read {
                Ok(doc) => {
                    json!({"ok":true,"entities":doc.entity_count(),"by_layer":counts(&doc),"notifications":doc.notifications.iter().map(|n|n.message.clone()).collect::<Vec<_>>()})
                }
                Err(e) => json!({"ok":false,"error":e.to_string()}),
            };
            let item = json!({"version":version.as_str(),"format":format,"file":file.to_string_lossy(),"bytes":bytes.len(),"cases":s.cases,"source_by_layer":counts(&s.doc),"readback":readback});
            std::fs::write(
                file.with_extension("manifest.json"),
                serde_json::to_vec_pretty(&item)?,
            )?;
            manifest.push(item);
            println!("{}: {} bytes", file.display(), bytes.len());
        }
    }
    std::fs::write(
        output.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    std::fs::write(output.join("coverage-notes.md"),"# Entity atlas\n\nEach file contains the same numbered grid. E### layers hold test entities; _ATLAS layers hold labels and frames. RAY and XLINE occupy separate rows below the grid. VIEWPORT is on Layout1. Version-ineligible cases retain their label but contain no entity.\n\nBLOCK, ENDBLK, ATTRIB, VERTEX and SEQEND are exercised through valid INSERT/polyline ownership. Generic, plane and NURB surface cases use a planar ACIS sheet. Native type checks do not certify arbitrary geometry or editing history.\n\nNot synthesized: extruded, lofted, revolved and swept surfaces (require native construction fixtures), OLEFRAME/OLE2FRAME (requires an embedded application payload), point clouds and coordination models (requires indexed external data), model-documentation SECTIONLINE/DRAWINGVIEW (requires a complete view-representation graph), third-party registered/proxy/unknown entities and dynamic-block internals (require an existing class payload or authoring graph), pre-R13 REPEAT/ENDREP/LOAD/JUMP (outside supported output versions). These are coverage exclusions, not passing tests.\n\nUnderlay paths are listed in the per-file manifests. A missing dependency is reported separately from a missing entity.\n")?;
    Ok(())
}
