//! Reading AutoCAD R10 (AC1006) drawings through the pre-R13 reader.
//!
//! The fixtures and the expected values come from CAD-Format-DWG-AC1006
//! (BSD-2-Clause), see `tests/fixtures/ac1006/README.md`.

use acadrust::entities::Dimension;
use acadrust::io::dwg::DwgReadOptions;
use acadrust::{CadDocument, DwgReader, DxfVersion, EntityType, Vector3};
use std::path::PathBuf;

fn path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ac1006")
        .join(name)
}

fn read(name: &str) -> CadDocument {
    DwgReader::from_file(path(name))
        .and_then(|mut reader| reader.read())
        .unwrap_or_else(|error| panic!("{name}: {error}"))
}

fn only(document: &CadDocument) -> &EntityType {
    let entities: Vec<_> = document.model_space_entities().collect();
    assert_eq!(entities.len(), 1, "{entities:?}");
    entities[0]
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
}

fn v(x: f64, y: f64, z: f64) -> Vector3 {
    Vector3::new(x, y, z)
}

const ALL: [&str; 32] = [
    "blank/BLANK.DWG",
    "entity/3dface/3DFACE1.DWG",
    "entity/3dface/3DFACE2.DWG",
    "entity/3dline/3DLINE1.DWG",
    "entity/arc/ARC1.DWG",
    "entity/circle/CIRCLE1.DWG",
    "entity/circle/CIRCLE2.DWG",
    "entity/dim/DIM1.DWG",
    "entity/insert/INSERT1.DWG",
    "entity/insert/INSERT2.DWG",
    "entity/jump/JUMP1.DWG",
    "entity/line/LINE1.DWG",
    "entity/line/LINE2.DWG",
    "entity/point/POINT1.DWG",
    "entity/polyline/PL1.DWG",
    "entity/seqend/PL1.DWG",
    "entity/shape/SHAPE1.DWG",
    "entity/solid/SOLID1.DWG",
    "entity/text/TEXT1.DWG",
    "entity/vertex/PL1.DWG",
    "header/dim_arrowhead_block1/DAB11.DWG",
    "header/dim_arrowhead_block1/DAB12.DWG",
    "header/dim_arrowhead_block2/DAB21.DWG",
    "header/dim_arrowhead_block2/DAB22.DWG",
    "header/dim_arrowhead_blocks_control/DS1.DWG",
    "header/dim_arrowhead_suppress/DAS1.DWG",
    "header/dim_text_between_ext_lines/DTBEL1.DWG",
    "header/dim_text_vertical_position_size/DTVPS1.DWG",
    "header/dim_tofl/DIMTOFL1.DWG",
    "header/handseed/HS1.DWG",
    "header/handseed/HS2.DWG",
    "header/surftype/ST1.DWG",
];

#[test]
fn every_sample_reads_as_an_r11_r12_document() {
    for name in ALL {
        let document = read(name);
        assert_eq!(document.version, DxfVersion::AC1009, "{name}");
    }
}

#[test]
fn failsafe_reports_the_record_stream_as_read() {
    for name in ALL {
        let outcome = DwgReader::from_file_with_options(path(name), DwgReadOptions::failsafe())
            .and_then(|mut reader| reader.read_with_stats())
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(outcome.stats.record_stream_read, "{name}");
    }
}

#[test]
fn header_variables() {
    let blank = read("blank/BLANK.DWG");
    assert_eq!(blank.header.model_space_limits_max.x, 12.0);
    assert_eq!(blank.header.model_space_limits_max.y, 9.0);
    assert_eq!(blank.header.linetype_scale, 1.0);
    assert_eq!(blank.header.surface_type, 6);
    assert!(!blank.header.dim_force_line_inside);
    assert!(blank.header.dim_arrow_block1.is_empty());
    assert!(blank
        .header
        .current_linetype_name
        .eq_ignore_ascii_case("BYLAYER"));
    assert_eq!(read("header/surftype/ST1.DWG").header.surface_type, 2);
    assert!(
        read("header/dim_tofl/DIMTOFL1.DWG")
            .header
            .dim_force_line_inside
    );
    assert_eq!(
        read("header/dim_arrowhead_block1/DAB11.DWG")
            .header
            .dim_arrow_block1,
        "DIM"
    );
    assert_eq!(
        read("header/dim_arrowhead_block1/DAB12.DWG")
            .header
            .dim_arrow_block1,
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ01234"
    );
}

#[test]
fn lines() {
    let EntityType::Line(line) = only(&read("entity/line/LINE1.DWG")).clone() else {
        panic!("LINE1 is not a line");
    };
    assert_eq!((line.start, line.end), (v(0., 0., 0.), v(1., 1., 0.)));
    let EntityType::Line(line) = only(&read("entity/line/LINE2.DWG")).clone() else {
        panic!("LINE2 is not a line");
    };
    assert_eq!((line.start, line.end), (v(0., 0., 1.), v(1., 1., 1.)));
    // 3DLINE with a one-byte linetype index (0 = CONTINUOUS).
    let EntityType::Line(line) = only(&read("entity/3dline/3DLINE1.DWG")).clone() else {
        panic!("3DLINE1 is not a line");
    };
    assert_eq!((line.start, line.end), (v(0., 0., 0.), v(1., 1., 1.)));
    assert!(line.common.linetype.eq_ignore_ascii_case("CONTINUOUS"));
}

#[test]
fn circles_and_arcs() {
    let EntityType::Circle(circle) = only(&read("entity/circle/CIRCLE1.DWG")).clone() else {
        panic!("CIRCLE1 is not a circle");
    };
    assert_eq!((circle.center, circle.radius), (v(1., 1., 0.), 1.0));
    // CIRCLE2 stores an elevation of 1.
    let EntityType::Circle(circle) = only(&read("entity/circle/CIRCLE2.DWG")).clone() else {
        panic!("CIRCLE2 is not a circle");
    };
    assert_eq!((circle.center, circle.radius), (v(1., 1., 1.), 1.0));
    let EntityType::Arc(arc) = only(&read("entity/arc/ARC1.DWG")).clone() else {
        panic!("ARC1 is not an arc");
    };
    assert_eq!(arc.center, v(9., -8., 0.));
    close(arc.radius, 12.0415945787923);
    close(arc.start_angle, 1.48765509490646);
    close(arc.end_angle, 2.41495031290807);
}

#[test]
fn points_solids_and_faces() {
    let EntityType::Point(point) = only(&read("entity/point/POINT1.DWG")).clone() else {
        panic!("POINT1 is not a point");
    };
    assert_eq!(point.location, v(1., 1., 0.));
    let EntityType::Solid(solid) = only(&read("entity/solid/SOLID1.DWG")).clone() else {
        panic!("SOLID1 is not a solid");
    };
    assert_eq!(
        [
            solid.first_corner,
            solid.second_corner,
            solid.third_corner,
            solid.fourth_corner
        ],
        [v(0., 0., 0.), v(0., 1., 0.), v(1., 1., 0.), v(1., 0., 0.)]
    );
    for (name, z) in [
        ("entity/3dface/3DFACE1.DWG", [0., 0., 0., 0.]),
        ("entity/3dface/3DFACE2.DWG", [0., 1., 2., 3.]),
    ] {
        let EntityType::Face3D(face) = only(&read(name)).clone() else {
            panic!("{name} is not a 3DFACE");
        };
        assert_eq!(
            [
                face.first_corner,
                face.second_corner,
                face.third_corner,
                face.fourth_corner
            ],
            [
                v(0., 0., z[0]),
                v(1., 1., z[1]),
                v(2., 2., z[2]),
                v(3., 3., z[3])
            ],
            "{name}"
        );
    }
}

#[test]
fn text_and_shape() {
    let EntityType::Text(text) = only(&read("entity/text/TEXT1.DWG")).clone() else {
        panic!("TEXT1 is not a text");
    };
    assert_eq!(text.value, "FOO");
    assert_eq!(text.insertion_point, v(1., 1., 0.));
    close(text.height, 0.2);
    let EntityType::Shape(shape) = only(&read("entity/shape/SHAPE1.DWG")).clone() else {
        panic!("SHAPE1 is not a shape");
    };
    assert_eq!(shape.insertion_point, v(5., 5., 0.));
    assert_eq!(shape.size, 1.0);
    assert_eq!(shape.shape_number, 131);
}

#[test]
fn inserts() {
    for (name, z) in [
        ("entity/insert/INSERT1.DWG", 0.0),
        ("entity/insert/INSERT2.DWG", 1.0),
    ] {
        let document = read(name);
        let inserts: Vec<_> = document
            .model_space_entities()
            .filter_map(|entity| match entity {
                EntityType::Insert(insert) => Some(insert.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(inserts.len(), 1, "{name}");
        let insert = &inserts[0];
        assert_eq!(insert.insert_point, v(2., 2., z), "{name}");
        assert_eq!(
            (insert.x_scale(), insert.y_scale(), insert.z_scale()),
            (0.5, 0.5, 0.5),
            "{name}"
        );
        close(insert.rotation, 0.5235987755982988);
        // The block definition holds the one LINE that is also in model
        // space before it was turned into a block.
        assert_eq!(
            document.entities_in_block(&insert.block_name).count(),
            1,
            "{name}"
        );
    }
}

#[test]
fn polylines_including_one_reached_through_a_jump() {
    let EntityType::Polyline2D(polyline) = only(&read("entity/polyline/PL1.DWG")).clone() else {
        panic!("PL1 is not a 2D polyline");
    };
    let locations: Vec<_> = polyline.vertices.iter().map(|v| v.location).collect();
    assert_eq!(locations, [v(0., 0., 0.), v(1., 1., 0.)]);
    // JUMP1 starts its entity section with a JUMP into the extra section,
    // where the POLYLINE header is; its five vertices follow in the entity
    // section.
    let EntityType::Polyline2D(polyline) = only(&read("entity/jump/JUMP1.DWG")).clone() else {
        panic!("JUMP1 is not a 2D polyline");
    };
    assert_eq!(polyline.vertices.len(), 5);
}

#[test]
fn dimension_and_its_block() {
    let document = read("entity/dim/DIM1.DWG");
    let dimensions: Vec<_> = document
        .model_space_entities()
        .filter_map(|entity| match entity {
            EntityType::Dimension(dimension) => Some(dimension.as_ref().clone()),
            _ => None,
        })
        .collect();
    assert_eq!(dimensions.len(), 1);
    assert_eq!(document.model_space_entities().count(), 2);
    let Dimension::Linear(linear) = &dimensions[0] else {
        panic!("DIM1 is not a linear dimension: {:?}", dimensions[0]);
    };
    assert_eq!(linear.first_point, v(0., 0., 0.));
    assert_eq!(linear.second_point, v(1., 1., 1.));
    assert_eq!(linear.definition_point.x, 1.0);
    close(linear.definition_point.y, -0.0969879154919123);
    assert_eq!(linear.base.text_middle_point.x, 1.9);
    // The dimension block draws its lines with linetype byte 0xff (BYBLOCK).
    let lines: Vec<_> = document
        .entities_in_block(&linear.base.block_name)
        .filter_map(|entity| match entity {
            EntityType::Line(line) => Some(line.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(lines.len(), 4);
    for line in lines {
        assert!(line.common.linetype.eq_ignore_ascii_case("BYBLOCK"));
    }
}

/// Turn a 158-variable sample into the later R10 form with 160 variables:
/// the APPID table locator and `$WORLDVIEW` are inserted at 0x512, as the
/// CAD-Format-DWG-AC1006 description documents for that form
/// (`if: num_header_vars == 160`). Every file address behind 0x512 moves by
/// the 12 inserted bytes. One APPID row is appended at the end of the file.
fn with_appid_header(name: &str, appid: &str) -> Vec<u8> {
    const AT: usize = 0x512;
    const SHIFT: u32 = 12;
    let source = std::fs::read(path(name)).unwrap();
    assert_eq!(u16::from_le_bytes([source[0x11], source[0x12]]), 158);
    let mut data = source[..AT].to_vec();
    data.extend_from_slice(&[0; 12]);
    data.extend_from_slice(&source[AT..]);
    data[0x11..0x13].copy_from_slice(&160u16.to_le_bytes());
    let mut shift = |offset: usize| {
        let value = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
        if value as usize >= AT {
            data[offset..offset + 4].copy_from_slice(&(value + SHIFT).to_le_bytes());
        }
    };
    // Entity, block and extra section addresses.
    for offset in [0x14, 0x18, 0x1c, 0x24] {
        shift(offset);
    }
    // Addresses of the seven table locators.
    for locator in [0x2c, 0x36, 0x40, 0x4a, 0x54, 0x3ef, 0x500] {
        shift(locator + 6);
    }
    // APPID locator: 33-byte rows, one row at the end of the file.
    let address = data.len() as u32;
    data[AT..AT + 2].copy_from_slice(&33u16.to_le_bytes());
    data[AT + 2..AT + 4].copy_from_slice(&1u16.to_le_bytes());
    data[AT + 6..AT + 10].copy_from_slice(&address.to_le_bytes());
    // $WORLDVIEW = 1
    data[AT + 10..AT + 12].copy_from_slice(&1u16.to_le_bytes());
    let mut row = vec![0u8; 33];
    row[1..1 + appid.len()].copy_from_slice(appid.as_bytes());
    data.extend_from_slice(&row);
    data
}

fn read_bytes(bytes: Vec<u8>) -> CadDocument {
    DwgReader::from_stream(std::io::Cursor::new(bytes))
        .read()
        .unwrap()
}

#[test]
fn header_with_appid_table_and_worldview() {
    let document = read_bytes(with_appid_header("entity/line/LINE1.DWG", "R10APP"));
    assert!(document.header.world_view);
    assert!(document.app_ids.get("R10APP").is_some());
    let EntityType::Line(line) = only(&document).clone() else {
        panic!("LINE1 is not a line");
    };
    assert_eq!((line.start, line.end), (v(0., 0., 0.), v(1., 1., 0.)));
    // Block and table addresses move with the header.
    let document = read_bytes(with_appid_header("entity/insert/INSERT1.DWG", "R10APP"));
    let insert = document
        .model_space_entities()
        .find_map(|entity| match entity {
            EntityType::Insert(insert) => Some(insert.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(insert.insert_point, v(2., 2., 0.));
    assert_eq!(document.entities_in_block(&insert.block_name).count(), 1);
}
