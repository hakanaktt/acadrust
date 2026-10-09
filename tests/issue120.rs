use std::collections::HashMap;
use std::io::Cursor;

use acadrust::entities::{EntityType, Point, Viewport};
use acadrust::objects::ObjectType;
use acadrust::types::{DxfVersion, Handle};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};

#[derive(Clone, Copy)]
struct ExpectedViewport {
    handle: Handle,
    id: i16,
    is_on: bool,
    off_screen: bool,
}

fn add_viewport(
    document: &mut CadDocument,
    layout: &str,
    source_id: i16,
    id: i16,
    is_on: bool,
    off_screen: bool,
) -> ExpectedViewport {
    let mut viewport = Viewport::new();
    viewport.id = source_id;
    viewport.status.is_on = is_on;
    viewport.off_screen = off_screen;
    let handle = document
        .add_entity_to_layout(EntityType::Viewport(viewport), layout)
        .unwrap();
    ExpectedViewport {
        handle,
        id,
        is_on,
        off_screen,
    }
}

fn layout_block(document: &CadDocument, name: &str) -> Handle {
    document
        .objects
        .values()
        .find_map(|object| match object {
            ObjectType::Layout(layout) if layout.name == name => Some(layout.block_record),
            _ => None,
        })
        .unwrap()
}

fn overall_viewport(document: &CadDocument, layout: &str) -> ExpectedViewport {
    let owner = layout_block(document, layout);
    let handle = document
        .block_records
        .iter()
        .find(|block| block.handle == owner)
        .unwrap()
        .entity_handles[0];
    ExpectedViewport {
        handle,
        id: 1,
        is_on: true,
        off_screen: false,
    }
}

fn viewport(document: &CadDocument, handle: Handle) -> &Viewport {
    match document.get_entity(handle).unwrap() {
        EntityType::Viewport(viewport) => viewport,
        _ => panic!("expected viewport {handle}"),
    }
}

fn viewport_records(bytes: &[u8]) -> HashMap<Handle, HashMap<i16, String>> {
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    let lines: Vec<_> = text.lines().map(str::trim).collect();
    let pairs: Vec<_> = lines
        .chunks_exact(2)
        .map(|pair| (pair[0].parse::<i16>().unwrap(), pair[1]))
        .collect();
    let mut records = HashMap::new();
    for (index, pair) in pairs.iter().enumerate() {
        if *pair != (0, "VIEWPORT") {
            continue;
        }
        let record: HashMap<_, _> = pairs[index + 1..]
            .iter()
            .take_while(|(code, _)| *code != 0)
            .map(|(code, value)| (*code, value.to_string()))
            .collect();
        let handle = Handle::new(u64::from_str_radix(&record[&5], 16).unwrap());
        records.insert(handle, record);
    }
    records
}

fn assert_dxf_viewports(document: &CadDocument, expected: &[ExpectedViewport]) {
    let records = viewport_records(&DxfWriter::new(document).write_to_vec().unwrap());
    assert_eq!(records.len(), expected.len());
    for expected in expected {
        let record = &records[&expected.handle];
        let status = if !expected.is_on {
            0
        } else if expected.off_screen {
            -1
        } else {
            expected.id
        };
        assert_eq!(record[&69], expected.id.to_string(), "viewport ID");
        assert_eq!(record[&68], status.to_string(), "viewport status");
        assert_eq!(record[&67], "1", "paper-space flag");
        assert_eq!(
            record[&90].parse::<i32>().unwrap() & 0x8000 != 0,
            expected.is_on,
            "viewport on flag"
        );
    }

    for writer in [DxfWriter::new(document), DxfWriter::new_binary(document)] {
        let restored = DxfReader::from_reader(Cursor::new(writer.write_to_vec().unwrap()))
            .unwrap()
            .read()
            .unwrap();
        for expected in expected {
            let restored = viewport(&restored, expected.handle);
            assert_eq!(restored.id, expected.id);
            assert_eq!(restored.status.is_on, expected.is_on);
            assert_eq!(restored.off_screen, expected.off_screen);
        }
    }
}

#[test]
fn dxf_assigns_missing_ids_per_layout_without_colliding_with_explicit_ids() {
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    document.add_layout("Sheet A").unwrap();
    document.add_layout("Sheet B").unwrap();
    let mut expected = Vec::new();
    let mut missing = Vec::new();
    for layout in ["Layout1", "Sheet A", "Sheet B"] {
        if layout == "Layout1" {
            let overall = add_viewport(&mut document, layout, 0, 1, true, false);
            missing.push(overall.handle);
            expected.push(overall);
        } else {
            expected.push(overall_viewport(&document, layout));
        }
        expected.push(add_viewport(&mut document, layout, 4, 4, true, false));
        document
            .add_entity_to_layout(EntityType::Point(Point::new()), layout)
            .unwrap();
        for (id, is_on, off_screen) in [(2, false, false), (3, true, true), (5, true, false)] {
            let viewport = add_viewport(&mut document, layout, 0, id, is_on, off_screen);
            missing.push(viewport.handle);
            expected.push(viewport);
        }
    }

    assert_dxf_viewports(&document, &expected);
    for handle in missing {
        assert_eq!(viewport(&document, handle).id, 0, "writer mutated source");
    }
}

fn assert_dwg_roundtrip(version: DxfVersion) {
    let mut document = CadDocument::with_version(version);
    document.add_layout("Sheet A").unwrap();
    document.add_layout("Sheet B").unwrap();
    let mut expected = Vec::new();
    for layout in ["Layout1", "Sheet A", "Sheet B"] {
        if layout == "Layout1" {
            expected.push(add_viewport(&mut document, layout, 1, 1, true, false));
        } else {
            expected.push(overall_viewport(&document, layout));
        }
        document
            .add_entity_to_layout(EntityType::Point(Point::new()), layout)
            .unwrap();
        for (id, is_on) in [(2, true), (3, false), (4, true)] {
            expected.push(add_viewport(&mut document, layout, id, id, is_on, false));
        }
    }
    let bytes = DwgWriter::write_to_vec(&document).unwrap();
    let restored = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
    for layout in ["Layout1", "Sheet A", "Sheet B"] {
        let owner = layout_block(&restored, layout);
        let source_owner = layout_block(&document, layout);
        for expected in &expected {
            if viewport(&document, expected.handle).common.owner_handle != source_owner {
                continue;
            }
            let viewport = viewport(&restored, expected.handle);
            assert_eq!(viewport.common.owner_handle, owner, "{version:?} {layout}");
            assert_eq!(viewport.id, expected.id, "{version:?} {layout}");
            assert_eq!(
                viewport.status.is_on, expected.is_on,
                "{version:?} {layout}"
            );
        }
    }
    assert_dxf_viewports(&restored, &expected);
}

#[test]
fn modern_dwg_reconstructs_viewport_ids_and_dxf_status_for_every_layout() {
    for version in [
        DxfVersion::AC1018,
        DxfVersion::AC1021,
        DxfVersion::AC1024,
        DxfVersion::AC1027,
        DxfVersion::AC1032,
    ] {
        assert_dwg_roundtrip(version);
    }
}

#[test]
fn r2000_dwg_reconstructs_viewport_ids_and_dxf_status_for_every_layout() {
    assert_dwg_roundtrip(DxfVersion::AC1015);
}
