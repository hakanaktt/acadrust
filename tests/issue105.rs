use std::io::Cursor;

use acadrust::entities::{EntityType, Viewport};
use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DxfReader, DxfWriter};

fn read_single_viewport(bytes: Vec<u8>) -> Viewport {
    let document = DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap();
    let viewport = document
        .entities()
        .find_map(|entity| match entity {
            EntityType::Viewport(viewport) => Some(viewport.clone()),
            _ => None,
        })
        .expect("viewport missing");
    viewport
}

#[test]
fn dxf_status_is_preserved_independently_of_viewport_id() {
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    let mut viewport = Viewport::new();
    viewport.id = 7;
    viewport.dxf_status = Some(3);
    let handle = document
        .add_paper_space_entity(EntityType::Viewport(viewport))
        .unwrap();

    let bytes = DxfWriter::new(&document).write_to_vec().unwrap();
    let text = String::from_utf8(bytes.clone()).unwrap();
    let normalized = text.lines().map(str::trim).collect::<Vec<_>>().join("\n");
    assert!(normalized.contains("\n68\n3\n"), "group 68 was not written");
    assert!(normalized.contains("\n69\n7\n"), "group 69 was not written");

    let restored = read_single_viewport(bytes);
    assert_eq!(restored.common.handle, handle);
    assert_eq!(restored.id, 7);
    assert_eq!(restored.dxf_status, Some(3));
    assert!(!restored.off_screen);
}

#[test]
fn dxf_reader_preserves_group_68_when_it_precedes_or_follows_group_69() {
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    let mut viewport = Viewport::new();
    viewport.id = 7;
    viewport.dxf_status = Some(3);
    document
        .add_paper_space_entity(EntityType::Viewport(viewport))
        .unwrap();

    let bytes = DxfWriter::new(&document).write_to_vec().unwrap();
    let text = String::from_utf8(bytes)
        .unwrap()
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n");
    let original = "\n68\n3\n69\n7\n";
    let reordered = "\n69\n7\n68\n3\n";
    assert_eq!(text.matches(original).count(), 1);
    let reordered = text.replace(original, reordered);

    let restored = read_single_viewport(reordered.into_bytes());
    assert_eq!(restored.id, 7);
    assert_eq!(restored.dxf_status, Some(3));
    assert!(!restored.off_screen);
}
