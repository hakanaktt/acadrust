use std::io::Cursor;

use acadrust::entities::{EntityType, Line};
use acadrust::tables::Layer;
use acadrust::types::{DxfVersion, Transparency};
use acadrust::{CadDocument, DxfReader, DxfWriter};

#[test]
fn layer_transparency_zero_is_explicit_opaque_but_entity_zero_remains_by_layer() {
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    let mut layer = Layer::new("RAW_ZERO");
    layer.transparency = Transparency::Explicit(1);
    document.layers.add(layer).unwrap();

    let mut line = Line::new();
    line.common.layer = "RAW_ZERO".to_string();
    line.common.transparency = Transparency::ByLayer;
    document.add_entity(EntityType::Line(line)).unwrap();

    let bytes = DxfWriter::new(&document).write_to_vec().unwrap();
    let mut lines: Vec<String> = String::from_utf8(bytes)
        .unwrap()
        .lines()
        .map(|line| line.trim().to_string())
        .collect();
    let transparency_xdata = lines
        .windows(4)
        .position(|window| window == ["1001", "AcCmTransparency", "1071", "33554686"])
        .expect("layer transparency XDATA");
    lines[transparency_xdata + 3] = "0".to_string();

    let restored = DxfReader::from_reader(Cursor::new(lines.join("\n").into_bytes()))
        .unwrap()
        .read()
        .unwrap();
    let restored_layer = restored.layers.get("RAW_ZERO").unwrap();
    assert_eq!(restored_layer.transparency, Transparency::OPAQUE);
    let restored_line = restored
        .entities()
        .find_map(|entity| match entity {
            EntityType::Line(line) => Some(line),
            _ => None,
        })
        .unwrap();
    assert_eq!(restored_line.common.transparency, Transparency::ByLayer);
}

#[test]
fn layer_transparency_decoder_distinguishes_defaults_and_unknown_methods() {
    assert_eq!(
        Transparency::from_layer_alpha_value(0),
        Some(Transparency::OPAQUE)
    );
    assert_eq!(
        Transparency::from_layer_alpha_value(0x0200_00DF),
        Some(Transparency::Explicit(32))
    );
    assert_eq!(
        Transparency::from_layer_alpha_value(0x0300_00DF),
        Some(Transparency::Explicit(32))
    );
    assert_eq!(
        Transparency::from_layer_alpha_value(0x0400_00DF),
        None,
        "unsupported methods must not become ByLayer"
    );
}
