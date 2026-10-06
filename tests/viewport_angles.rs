use acadrust::entities::Viewport;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter, EntityType, Vector3};
use std::io::Cursor;

fn assert_angle(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-12,
        "expected {expected}, got {actual}"
    );
}

fn viewport(doc: &CadDocument) -> &Viewport {
    doc.entities()
        .find_map(|entity| match entity {
            EntityType::Viewport(view) if view.width == 160.0 => Some(view),
            _ => None,
        })
        .unwrap()
}

fn drawing(snap: f64, twist: f64) -> CadDocument {
    let mut doc = CadDocument::new();
    let mut view = Viewport::new();
    view.id = 2;
    view.center = Vector3::new(100.0, 75.0, 0.0);
    view.width = 160.0;
    view.height = 100.0;
    view.snap_angle = snap;
    view.twist_angle = twist;
    doc.add_entity_to_layout(EntityType::Viewport(view), "Layout1")
        .unwrap();
    doc
}

#[test]
fn literal_dxf_viewport_angles_are_read_as_radians() {
    let doc = DxfReader::from_reader(Cursor::new(include_bytes!(
        "fixtures/viewports/reference-angle-units.dxf"
    )))
    .unwrap()
    .read()
    .unwrap();
    let view = viewport(&doc);
    assert_angle(view.snap_angle, -std::f64::consts::FRAC_PI_4);
    assert_angle(view.twist_angle, std::f64::consts::FRAC_PI_6);
}

#[test]
fn ascii_dxf_viewport_angles_are_written_as_degrees() {
    for (snap, twist, snap_degrees, twist_degrees) in [
        (
            -std::f64::consts::FRAC_PI_4,
            std::f64::consts::FRAC_PI_6,
            -45.0,
            30.0,
        ),
        (
            std::f64::consts::FRAC_PI_2,
            -std::f64::consts::PI,
            90.0,
            -180.0,
        ),
        (0.0, 0.0, 0.0, 0.0),
    ] {
        let doc = drawing(snap, twist);
        let text = String::from_utf8(DxfWriter::new(&doc).write_to_vec().unwrap()).unwrap();
        // Inspect the wire values directly: reading them with DxfReader would
        // let symmetric reader/writer unit mistakes conceal each other.
        let lines: Vec<_> = text.lines().collect();
        let start = lines
            .chunks_exact(2)
            .position(|pair| pair[0].trim() == "0" && pair[1].trim() == "VIEWPORT")
            .unwrap()
            * 2;
        let fields: Vec<_> = lines[start + 2..]
            .chunks_exact(2)
            .take_while(|pair| pair[0].trim() != "0")
            .collect();
        for (code, expected) in [("50", snap_degrees), ("51", twist_degrees)] {
            let values: Vec<f64> = fields
                .iter()
                .filter(|pair| pair[0].trim() == code)
                .map(|pair| pair[1].trim().parse().unwrap())
                .collect();
            assert_eq!(values.len(), 1, "VIEWPORT group {code}");
            assert_angle(values[0], expected);
        }
    }
}

#[test]
fn binary_dxf_viewport_angles_roundtrip_in_radians() {
    let doc = drawing(-std::f64::consts::FRAC_PI_4, std::f64::consts::FRAC_PI_6);
    let restored = DxfReader::from_reader(Cursor::new(
        DxfWriter::new_binary(&doc).write_to_vec().unwrap(),
    ))
    .unwrap()
    .read()
    .unwrap();
    assert_angle(viewport(&restored).snap_angle, -std::f64::consts::FRAC_PI_4);
    assert_angle(viewport(&restored).twist_angle, std::f64::consts::FRAC_PI_6);
}

#[test]
fn dwg_viewport_angles_remain_in_radians() {
    let doc = drawing(-std::f64::consts::FRAC_PI_4, std::f64::consts::FRAC_PI_6);
    let restored = DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(&doc).unwrap()))
        .read()
        .unwrap();
    assert_angle(viewport(&restored).snap_angle, -std::f64::consts::FRAC_PI_4);
    assert_angle(viewport(&restored).twist_angle, std::f64::consts::FRAC_PI_6);
}
