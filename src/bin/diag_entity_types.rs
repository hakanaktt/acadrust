//! Diagnostic: test each entity type individually for DWG roundtrip.

use acadrust::entities::*;
use acadrust::entities::dimension::*;
use acadrust::io::dwg::writer::dwg_writer::DwgWriter;
use acadrust::io::dwg::DwgReader;
use acadrust::types::{DxfVersion, Vector3};
use acadrust::CadDocument;

fn test_entity(name: &str, _version: DxfVersion, doc: CadDocument) {
    let bytes = match DwgWriter::write(&doc) {
        Ok(b) => b,
        Err(e) => {
            println!("  WRITE_FAIL {name:<30} {e}");
            return;
        }
    };

    let reader = match DwgReader::from_reader(std::io::Cursor::new(bytes)) {
        Ok(r) => r,
        Err(e) => {
            println!("  OPEN_FAIL  {name:<30} {e}");
            return;
        }
    };

    match reader.read() {
        Ok(doc2) => {
            let count = doc2.entities().count();
            let block_count: usize = doc2.block_records.iter().map(|b| b.entities.len()).sum();
            if count > 0 || block_count > 0 {
                println!("  OK         {name:<30} entities={count} block={block_count}");
            } else {
                println!("  ZERO_ENT   {name:<30} (wrote OK but read 0 entities)");
            }
        }
        Err(e) => {
            println!("  READ_FAIL  {name:<30} {e}");
        }
    }
}

fn make_doc(version: DxfVersion, entity: EntityType) -> CadDocument {
    let mut doc = CadDocument::new();
    doc.version = version;
    doc.add_entity(entity);
    doc
}

fn main() {
    let versions = [
        DxfVersion::AC1015,
        DxfVersion::AC1018,
        DxfVersion::AC1024,
        DxfVersion::AC1027,
        DxfVersion::AC1032,
    ];

    for &version in &versions {
        println!("Testing entity types for {:?}", version);
        run_tests(version);
        println!();
    }

    // Test combined: minimal failing case
    println!("\nTesting combined (minimal fail case)");
    for &version in &[DxfVersion::AC1024, DxfVersion::AC1027, DxfVersion::AC1032] {
        // PASS: 5text + mtext + 5 dims (aligned→angular3pt) = 12 entities
        test_entity(&format!("5tm5d_{:?}", version), version, {
            let mut doc = CadDocument::new();
            doc.version = version;
            for (i, height) in [1.5, 2.0, 3.0, 4.0, 5.0].iter().enumerate() {
                let text = Text::with_value(&format!("H{:.1}", height), Vector3::new(0.0, (i as f64) * 10.0, 0.0)).with_height(*height);
                doc.add_entity(EntityType::Text(text));
            }
            let mut mtext = MText::new();
            mtext.value = "\\A1;Centered MText\\PWith multiple lines".to_string();
            mtext.insertion_point = Vector3::new(30.0, 0.0, 0.0);
            mtext.height = 2.5;
            mtext.rectangle_width = 25.0;
            doc.add_entity(EntityType::MText(mtext));
            doc.add_entity(EntityType::Dimension(Dimension::Aligned(DimensionAligned::new(Vector3::new(0.0, 60.0, 0.0), Vector3::new(20.0, 60.0, 0.0)))));
            doc.add_entity(EntityType::Dimension(Dimension::Linear(DimensionLinear::new(Vector3::new(0.0, 90.0, 0.0), Vector3::new(15.0, 98.0, 0.0)))));
            doc.add_entity(EntityType::Dimension(Dimension::Radius(DimensionRadius::new(Vector3::new(10.0, 125.0, 0.0), Vector3::new(18.0, 125.0, 0.0)))));
            doc.add_entity(EntityType::Dimension(Dimension::Diameter(DimensionDiameter::new(Vector3::new(40.0, 125.0, 0.0), Vector3::new(50.0, 125.0, 0.0)))));
            doc.add_entity(EntityType::Dimension(Dimension::Angular3Pt(DimensionAngular3Pt::new(Vector3::new(0.0, 150.0, 0.0), Vector3::new(10.0, 160.0, 0.0), Vector3::new(20.0, 150.0, 0.0)))));
            doc
        });
        // FAIL on AC1027/AC1032: 5text + mtext + 6 dims (add angular2ln) = 12 entities
        test_entity(&format!("5tm6d_{:?}", version), version, {
            let mut doc = CadDocument::new();
            doc.version = version;
            for (i, height) in [1.5, 2.0, 3.0, 4.0, 5.0].iter().enumerate() {
                let text = Text::with_value(&format!("H{:.1}", height), Vector3::new(0.0, (i as f64) * 10.0, 0.0)).with_height(*height);
                doc.add_entity(EntityType::Text(text));
            }
            let mut mtext = MText::new();
            mtext.value = "\\A1;Centered MText\\PWith multiple lines".to_string();
            mtext.insertion_point = Vector3::new(30.0, 0.0, 0.0);
            mtext.height = 2.5;
            mtext.rectangle_width = 25.0;
            doc.add_entity(EntityType::MText(mtext));
            doc.add_entity(EntityType::Dimension(Dimension::Aligned(DimensionAligned::new(Vector3::new(0.0, 60.0, 0.0), Vector3::new(20.0, 60.0, 0.0)))));
            doc.add_entity(EntityType::Dimension(Dimension::Linear(DimensionLinear::new(Vector3::new(0.0, 90.0, 0.0), Vector3::new(15.0, 98.0, 0.0)))));
            doc.add_entity(EntityType::Dimension(Dimension::Radius(DimensionRadius::new(Vector3::new(10.0, 125.0, 0.0), Vector3::new(18.0, 125.0, 0.0)))));
            doc.add_entity(EntityType::Dimension(Dimension::Diameter(DimensionDiameter::new(Vector3::new(40.0, 125.0, 0.0), Vector3::new(50.0, 125.0, 0.0)))));
            doc.add_entity(EntityType::Dimension(Dimension::Angular3Pt(DimensionAngular3Pt::new(Vector3::new(0.0, 150.0, 0.0), Vector3::new(10.0, 160.0, 0.0), Vector3::new(20.0, 150.0, 0.0)))));
            doc.add_entity(EntityType::Dimension(Dimension::Angular2Ln(DimensionAngular2Ln::new(Vector3::new(30.0, 150.0, 0.0), Vector3::new(40.0, 160.0, 0.0), Vector3::new(50.0, 150.0, 0.0)))));
            doc
        });
    }
}

fn run_tests(version: DxfVersion) {

    test_entity("Line", version, make_doc(version, EntityType::Line(Line {
        common: EntityCommon::default(),
        start: Vector3::new(0.0, 0.0, 0.0),
        end: Vector3::new(10.0, 10.0, 0.0),
        thickness: 0.0,
        normal: Vector3::UNIT_Z,
    })));

    test_entity("Circle", version, make_doc(version, EntityType::Circle(Circle {
        common: EntityCommon::default(),
        center: Vector3::new(5.0, 5.0, 0.0),
        radius: 5.0,
        thickness: 0.0,
        normal: Vector3::UNIT_Z,
    })));

    test_entity("Arc", version, make_doc(version, EntityType::Arc(Arc {
        common: EntityCommon::default(),
        center: Vector3::new(5.0, 5.0, 0.0),
        radius: 5.0,
        start_angle: 0.0,
        end_angle: 1.5708,
        thickness: 0.0,
        normal: Vector3::UNIT_Z,
    })));

    test_entity("Point", version, make_doc(version, EntityType::Point(Point {
        common: EntityCommon::default(),
        location: Vector3::new(5.0, 5.0, 0.0),
        thickness: 0.0,
        normal: Vector3::UNIT_Z,
    })));

    test_entity("Text", version, make_doc(version, EntityType::Text(Text {
        common: EntityCommon::default(),
        value: "Hello".to_string(),
        insertion_point: Vector3::new(0.0, 0.0, 0.0),
        height: 2.5,
        ..Default::default()
    })));

    test_entity("MText", version, make_doc(version, EntityType::MText(MText {
        common: EntityCommon::default(),
        value: "Hello MText".to_string(),
        insertion_point: Vector3::new(0.0, 0.0, 0.0),
        height: 2.5,
        rectangle_width: 50.0,
        ..Default::default()
    })));

    test_entity("Ellipse", version, make_doc(version, EntityType::Ellipse(Ellipse {
        common: EntityCommon::default(),
        center: Vector3::new(5.0, 5.0, 0.0),
        major_axis: Vector3::new(10.0, 0.0, 0.0),
        minor_axis_ratio: 0.5,
        start_parameter: 0.0,
        end_parameter: std::f64::consts::TAU,
        normal: Vector3::UNIT_Z,
    })));

    test_entity("Ray", version, make_doc(version, EntityType::Ray(Ray {
        common: EntityCommon::default(),
        base_point: Vector3::new(0.0, 0.0, 0.0),
        direction: Vector3::new(1.0, 1.0, 0.0),
    })));

    test_entity("XLine", version, make_doc(version, EntityType::XLine(XLine {
        common: EntityCommon::default(),
        base_point: Vector3::new(0.0, 0.0, 0.0),
        direction: Vector3::new(1.0, 0.0, 0.0),
    })));

    test_entity("LwPolyline", version, {
        let mut doc = CadDocument::new();
        doc.version = version;
        let mut lwp = LwPolyline::default();
        lwp.vertices.push(LwVertex { location: acadrust::types::Vector2::new(0.0, 0.0), bulge: 0.0, start_width: 0.0, end_width: 0.0 });
        lwp.vertices.push(LwVertex { location: acadrust::types::Vector2::new(10.0, 0.0), bulge: 0.0, start_width: 0.0, end_width: 0.0 });
        lwp.vertices.push(LwVertex { location: acadrust::types::Vector2::new(10.0, 10.0), bulge: 0.0, start_width: 0.0, end_width: 0.0 });
        lwp.is_closed = true;
        doc.add_entity(EntityType::LwPolyline(lwp));
        doc
    });

    test_entity("Spline", version, {
        let mut doc = CadDocument::new();
        doc.version = version;
        let mut spline = Spline::default();
        spline.degree = 3;
        spline.control_points = vec![
            Vector3::ZERO, Vector3::new(5.0, 10.0, 0.0),
            Vector3::new(10.0, 0.0, 0.0), Vector3::new(15.0, 10.0, 0.0),
        ];
        spline.knots = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
        doc.add_entity(EntityType::Spline(spline));
        doc
    });

    test_entity("Insert", version, make_doc(version, EntityType::Insert(Insert {
        common: EntityCommon::default(),
        block_name: "*Model_Space".to_string(),
        insert_point: Vector3::ZERO,
        x_scale: 1.0,
        y_scale: 1.0,
        z_scale: 1.0,
        rotation: 0.0,
        normal: Vector3::UNIT_Z,
        column_count: 1,
        row_count: 1,
        column_spacing: 0.0,
        row_spacing: 0.0,
        attributes: vec![],
    })));

    test_entity("Viewport", version, make_doc(version, EntityType::Viewport(Viewport {
        common: EntityCommon::default(),
        center: Vector3::new(5.0, 5.0, 0.0),
        width: 10.0,
        height: 10.0,
        ..Default::default()
    })));

    test_entity("DimLinear", version, {
        let mut doc = CadDocument::new();
        doc.version = version;
        let mut dim = DimensionLinear::default();
        dim.base.definition_point = Vector3::new(10.0, 0.0, 0.0);
        dim.base.text_middle_point = Vector3::new(5.0, 5.0, 0.0);
        dim.definition_point = Vector3::new(10.0, 5.0, 0.0);
        doc.add_entity(EntityType::Dimension(Dimension::Linear(dim)));
        doc
    });

    test_entity("Leader", version, {
        let mut doc = CadDocument::new();
        doc.version = version;
        let mut leader = Leader::default();
        leader.vertices = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(5.0, 5.0, 0.0),
            Vector3::new(10.0, 5.0, 0.0),
        ];
        doc.add_entity(EntityType::Leader(leader));
        doc
    });

    test_entity("Tolerance", version, {
        let mut doc = CadDocument::new();
        doc.version = version;
        let tol = Tolerance::default();
        doc.add_entity(EntityType::Tolerance(tol));
        doc
    });

    test_entity("Shape", version, {
        let mut doc = CadDocument::new();
        doc.version = version;
        let mut shape = Shape::default();
        shape.size = 1.0;
        doc.add_entity(EntityType::Shape(shape));
        doc
    });

    test_entity("Hatch", version, {
        let mut doc = CadDocument::new();
        doc.version = version;
        let hatch = Hatch::default();
        doc.add_entity(EntityType::Hatch(hatch));
        doc
    });

    test_entity("Face3D", version, make_doc(version, EntityType::Face3D(Face3D {
        common: EntityCommon::default(),
        first_corner: Vector3::new(0.0, 0.0, 0.0),
        second_corner: Vector3::new(10.0, 0.0, 0.0),
        third_corner: Vector3::new(10.0, 10.0, 0.0),
        fourth_corner: Vector3::new(0.0, 10.0, 0.0),
        invisible_edges: InvisibleEdgeFlags::NONE,
    })));

    test_entity("Solid", version, make_doc(version, EntityType::Solid(Solid {
        common: EntityCommon::default(),
        first_corner: Vector3::new(0.0, 0.0, 0.0),
        second_corner: Vector3::new(10.0, 0.0, 0.0),
        third_corner: Vector3::new(0.0, 10.0, 0.0),
        fourth_corner: Vector3::new(10.0, 10.0, 0.0),
        thickness: 0.0,
        normal: Vector3::UNIT_Z,
    })));

    // Additional dimension types
    test_entity("DimAligned", version, {
        let mut doc = CadDocument::new();
        doc.version = version;
        let dim = DimensionAligned::new(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(10.0, 5.0, 0.0),
        );
        doc.add_entity(EntityType::Dimension(Dimension::Aligned(dim)));
        doc
    });

    test_entity("DimRadius", version, {
        let mut doc = CadDocument::new();
        doc.version = version;
        let dim = DimensionRadius::new(
            Vector3::new(5.0, 5.0, 0.0),
            Vector3::new(10.0, 5.0, 0.0),
        );
        doc.add_entity(EntityType::Dimension(Dimension::Radius(dim)));
        doc
    });

    test_entity("DimDiameter", version, {
        let mut doc = CadDocument::new();
        doc.version = version;
        let dim = DimensionDiameter::new(
            Vector3::new(5.0, 5.0, 0.0),
            Vector3::new(12.0, 5.0, 0.0),
        );
        doc.add_entity(EntityType::Dimension(Dimension::Diameter(dim)));
        doc
    });

    test_entity("DimAngular3Pt", version, {
        let mut doc = CadDocument::new();
        doc.version = version;
        let dim = DimensionAngular3Pt::new(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(5.0, 5.0, 0.0),
            Vector3::new(10.0, 0.0, 0.0),
        );
        doc.add_entity(EntityType::Dimension(Dimension::Angular3Pt(dim)));
        doc
    });

    test_entity("DimOrdinate", version, {
        let mut doc = CadDocument::new();
        doc.version = version;
        let dim = DimensionOrdinate::x_ordinate(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(5.0, 5.0, 0.0),
        );
        doc.add_entity(EntityType::Dimension(Dimension::Ordinate(dim)));
        doc
    });

    test_entity("DimAngular2Ln", version, {
        let mut doc = CadDocument::new();
        doc.version = version;
        let dim = DimensionAngular2Ln::new(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(5.0, 5.0, 0.0),
            Vector3::new(10.0, 0.0, 0.0),
        );
        doc.add_entity(EntityType::Dimension(Dimension::Angular2Ln(dim)));
        doc
    });

    test_entity("Wipeout", version, {
        let mut doc = CadDocument::new();
        doc.version = version;
        let wipeout = Wipeout::rectangular(Vector3::new(0.0, 0.0, 0.0), 10.0, 8.0);
        doc.add_entity(EntityType::Wipeout(wipeout));
        doc
    });
}
