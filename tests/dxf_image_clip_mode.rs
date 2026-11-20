//! An IMAGE's clip mode (DXF group 290: 0 = outside, 1 = inside) exists
//! from R2010 (AC1024) on, per the DXF reference's IMAGE group table and
//! the ODA spec's `clip_inverted` bit. Without it, an image clipped to
//! the inside of its boundary reads back clipping the outside.

use std::io::Cursor;

use acadrust::entities::{ClipMode, ClipType, EntityType, RasterImage};
use acadrust::{CadDocument, DxfReader, DxfVersion, DxfWriter, Handle, Vector2, Vector3};

/// One image with a closed polygonal clip boundary and the given mode.
fn image_document(version: DxfVersion, clip_mode: ClipMode) -> (CadDocument, Handle) {
    let mut document = CadDocument::with_version(version);
    let mut image = RasterImage::new("photo.png", Vector3::ZERO, 64.0, 32.0);
    image.clipping_enabled = true;
    image.clip_boundary.clip_type = ClipType::Polygonal;
    image.clip_boundary.clip_mode = clip_mode;
    image.clip_boundary.vertices = vec![
        Vector2::new(4.0, 4.0),
        Vector2::new(60.0, 4.0),
        Vector2::new(60.0, 28.0),
        Vector2::new(4.0, 28.0),
        Vector2::new(4.0, 4.0),
    ];
    let handle = document.add_entity(EntityType::RasterImage(image)).unwrap();
    (document, handle)
}

fn dxf_text(document: &CadDocument) -> String {
    String::from_utf8(DxfWriter::new(document).write_to_vec().unwrap()).unwrap()
}

fn dxf_round_trip(document: &CadDocument) -> CadDocument {
    DxfReader::from_reader(Cursor::new(dxf_text(document).into_bytes()))
        .unwrap()
        .read()
        .unwrap()
}

/// The (code, value) pairs of the first IMAGE entity in a DXF text.
fn image_groups(text: &str) -> Vec<(i32, String)> {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let start = lines
        .windows(2)
        .position(|w| w[0] == "0" && w[1] == "IMAGE")
        .expect("IMAGE entity");
    lines[start + 2..]
        .chunks_exact(2)
        .map(|pair| (pair[0].parse().unwrap(), pair[1].to_string()))
        .take_while(|(code, _)| *code != 0)
        .collect()
}

/// The single 290 group value of the first IMAGE, if written.
fn written_clip_mode(document: &CadDocument) -> Option<String> {
    image_groups(&dxf_text(document))
        .into_iter()
        .find(|(code, _)| *code == 290)
        .map(|(_, value)| value)
}

fn clip_mode_of(document: &CadDocument, image: Handle) -> ClipMode {
    match document.get_entity(image) {
        Some(EntityType::RasterImage(image)) => image.clip_boundary.clip_mode,
        other => panic!("IMAGE {image:?} missing: {other:?}"),
    }
}

/// An inside clip keeps its mode through DXF: the file carries `290 1`
/// (after the boundary, as the DXF reference orders it) and the reader
/// restores `ClipMode::Inside`.
#[test]
fn inside_clip_mode_is_written_and_round_trips() {
    for version in [DxfVersion::AC1024, DxfVersion::AC1032] {
        let (document, image) = image_document(version, ClipMode::Inside);
        assert_eq!(
            written_clip_mode(&document).as_deref(),
            Some("1"),
            "{version:?}"
        );
        let read = dxf_round_trip(&document);
        assert_eq!(clip_mode_of(&read, image), ClipMode::Inside, "{version:?}");
    }
}

/// An outside clip is written as `290 0`, not silently omitted.
#[test]
fn outside_clip_mode_is_written_as_zero() {
    let (document, image) = image_document(DxfVersion::AC1024, ClipMode::Outside);
    assert_eq!(written_clip_mode(&document).as_deref(), Some("0"));
    let read = dxf_round_trip(&document);
    assert_eq!(clip_mode_of(&read, image), ClipMode::Outside);
}

/// Before R2010 the group does not exist, so nothing is emitted and the
/// reader's default (outside) applies.
#[test]
fn clip_mode_group_absent_before_r2010() {
    for version in [DxfVersion::AC1015, DxfVersion::AC1021] {
        let (document, image) = image_document(version, ClipMode::Inside);
        assert_eq!(written_clip_mode(&document), None, "{version:?}");
        let read = dxf_round_trip(&document);
        assert_eq!(clip_mode_of(&read, image), ClipMode::Outside, "{version:?}");
    }
}

/// An image without a clip boundary writes the default mode.
#[test]
fn image_without_clipping_writes_default_mode() {
    let mut document = CadDocument::with_version(DxfVersion::AC1024);
    let image = RasterImage::new("photo.png", Vector3::ZERO, 64.0, 32.0);
    let image = document.add_entity(EntityType::RasterImage(image)).unwrap();
    assert_eq!(written_clip_mode(&document).as_deref(), Some("0"));
    let read = dxf_round_trip(&document);
    assert_eq!(clip_mode_of(&read, image), ClipMode::Outside);
}
