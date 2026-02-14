/// Minimal roundtrip diagnostic: write 1 entity, read it back.
/// Traces every step to find where entities disappear.

use acadrust::document::CadDocument;
use acadrust::entities::*;
use acadrust::io::dwg::reader::dwg_reader::{DwgReader, DwgReaderConfiguration};
use acadrust::io::dwg::writer::dwg_writer::DwgWriter;
use acadrust::tables::TableEntry;
use acadrust::types::DxfVersion;
use std::io::Cursor;

fn main() {
    let versions = [
        (DxfVersion::AC1032, "AC1032"),
        (DxfVersion::AC1024, "AC1024"),
        (DxfVersion::AC1018, "AC1018"),
        (DxfVersion::AC1015, "AC1015"),
    ];

    for (version, label) in &versions {
        println!("\n═══════════════════ {} ═══════════════════", label);
        test_version(*version, label);
    }
}

fn test_version(version: DxfVersion, label: &str) {
    // 1. Create a minimal doc with 1 line entity
    let mut doc = CadDocument::new();
    doc.version = version;

    let line = Line::from_coords(0.0, 0.0, 0.0, 10.0, 5.0, 0.0);
    let entity_handle = doc.add_entity(EntityType::Line(line)).unwrap();

    let standalone_count = doc.entities().count();
    let block_count: usize = doc.block_records.iter().map(|b| b.entities.len()).sum();
    println!("1. Created doc: {} standalone entities, {} in blocks", standalone_count, block_count);
    println!("   Line entity handle: {:#X}", entity_handle.value());
    for br in doc.block_records.iter() {
        println!("   BlockRecord '{}': handle={:#X}, block_entity_h={:#X}, block_end_h={:#X}",
            br.name(), br.handle.value(),
            br.block_entity_handle.value(), br.block_end_handle.value());
    }

    // 2. Write to DWG
    let bytes = match DwgWriter::write(&doc) {
        Ok(b) => {
            println!("2. DwgWriter::write OK: {} bytes", b.len());
            b
        }
        Err(e) => {
            println!("2. DwgWriter::write FAILED: {}", e);
            return;
        }
    };

    // Inspect the version string at offset 0
    if bytes.len() > 6 {
        let ver = String::from_utf8_lossy(&bytes[0..6]);
        println!("   File version string: {}", ver);
    }

    // Save to disk for inspection
    let path = format!("test_output/diag_minimal_{}.dwg", label);
    std::fs::create_dir_all("test_output").ok();
    std::fs::write(&path, &bytes).ok();
    println!("   Saved to {}", path);

    // 3. Read back
    // First pass: failsafe=true to get what we can
    let cursor1 = Cursor::new(bytes.clone());
    let reader1 = match DwgReader::from_reader(cursor1) {
        Ok(r) => r.with_config(DwgReaderConfiguration {
            failsafe: true,
            keep_unknown_entities: true,
        }),
        Err(e) => {
            println!("3. DwgReader::from_reader FAILED: {}", e);
            return;
        }
    };

    match reader1.read() {
        Ok(readback) => {
            let rb_standalone = readback.entities().count();
            let rb_block: usize = readback
                .block_records
                .iter()
                .map(|b| b.entities.len())
                .sum();
            let rb_layers = readback.layers.len();
            let rb_blocks = readback.block_records.len();

            println!("3. DwgReader::read OK:");
            println!("   Standalone entities: {}", rb_standalone);
            println!("   Block entities: {}", rb_block);
            println!("   Layers: {}", rb_layers);
            println!("   Block records: {}", rb_blocks);

            for br in readback.block_records.iter() {
                println!("   Block '{}': {} entities, block_entity_h={:#X}, block_end_h={:#X}",
                    br.name(), br.entities.len(),
                    br.block_entity_handle.value(),
                    br.block_end_handle.value());
            }

            // Check entity types
            for e in readback.entities() {
                println!("   Entity: {:?} handle={:#X}",
                    entity_type_name(e), e.common().handle.value());
            }
        }
        Err(e) => {
            println!("3. DwgReader::read (failsafe) FAILED: {}", e);
        }
    }

    // Second pass: failsafe=false to see errors
    let cursor2 = Cursor::new(bytes);
    let reader2 = match DwgReader::from_reader(cursor2) {
        Ok(r) => r.with_config(DwgReaderConfiguration {
            failsafe: false,
            keep_unknown_entities: true,
        }),
        Err(e) => {
            println!("4. DwgReader::from_reader (strict) FAILED: {}", e);
            return;
        }
    };

    match reader2.read() {
        Ok(readback) => {
            let rb_standalone = readback.entities().count();
            let rb_block: usize = readback.block_records.iter().map(|b| b.entities.len()).sum();
            println!("4. Strict read OK: {} standalone, {} block entities", rb_standalone, rb_block);
        }
        Err(e) => {
            println!("4. Strict read FAILED: {}", e);
        }
    }
}

fn entity_type_name(e: &EntityType) -> &'static str {
    match e {
        EntityType::Line(_) => "Line",
        EntityType::Circle(_) => "Circle",
        EntityType::Arc(_) => "Arc",
        EntityType::Point(_) => "Point",
        EntityType::Ellipse(_) => "Ellipse",
        EntityType::Text(_) => "Text",
        EntityType::MText(_) => "MText",
        EntityType::LwPolyline(_) => "LwPolyline",
        EntityType::Spline(_) => "Spline",
        EntityType::Hatch(_) => "Hatch",
        EntityType::Dimension(_) => "Dimension",
        EntityType::Insert(_) => "Insert",
        EntityType::Solid(_) => "Solid",
        EntityType::Face3D(_) => "Face3D",
        EntityType::Block(_) => "Block",
        EntityType::BlockEnd(_) => "BlockEnd",
        _ => "Other",
    }
}
