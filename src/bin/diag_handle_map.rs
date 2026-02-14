/// Diagnostic: directly test the object writer to inspect handle maps.

use acadrust::document::CadDocument;
use acadrust::entities::*;
use acadrust::io::dwg::writer::object_writer::DwgObjectWriter;
use acadrust::types::DxfVersion;

fn main() {
    let versions = [
        (DxfVersion::AC1032, "AC1032"),
        (DxfVersion::AC1018, "AC1018"),
    ];

    for (version, label) in &versions {
        println!("\n═══════════════════ {} ═══════════════════", label);
        test_version(*version, label);
    }
}

fn test_version(version: DxfVersion, label: &str) {
    let mut doc = CadDocument::new();
    doc.version = version;

    let line = Line::from_coords(0.0, 0.0, 0.0, 10.0, 5.0, 0.0);
    let entity_handle = doc.add_entity(EntityType::Line(line)).unwrap();
    println!("Entity handle: {:#X}", entity_handle.value());

    // Print all block records and their handles
    for br in doc.block_records.iter() {
        let name = if br.is_model_space() { "*Model_Space" } else if br.is_paper_space() { "*Paper_Space" } else { "other" };
        println!("BlockRecord '{}': handle={:#X}, block_h={:#X}, end_h={:#X}",
            name, br.handle.value(),
            br.block_entity_handle.value(), br.block_end_handle.value());
    }

    let obj_writer = DwgObjectWriter::new(version, &doc);
    match obj_writer.write(&doc) {
        Ok((objects_data, handle_map)) => {
            println!("Objects data: {} bytes", objects_data.len());
            println!("Handle map entries: {}", handle_map.len());

            // Print all handles in the map
            println!("Handle map:");
            for (handle, offset) in &handle_map {
                println!("  Handle {:#X} → offset {}", handle, offset);
            }

            // Check if entity handle is present
            if handle_map.contains_key(&entity_handle.value()) {
                println!("✓ Entity handle {:#X} IS in handle map", entity_handle.value());
            } else {
                println!("✗ Entity handle {:#X} NOT in handle map!", entity_handle.value());
            }
        }
        Err(e) => {
            println!("Object writer FAILED: {}", e);
        }
    }
}
