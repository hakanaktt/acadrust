/// Reference roundtrip: read a reference DWG, write it back, read again.
/// This tests if our writer produces readable output from known-good data.

use acadrust::io::dwg::reader::dwg_reader::{DwgReader, DwgReaderConfiguration};
use acadrust::io::dwg::writer::dwg_writer::DwgWriter;
use std::io::Cursor;

fn main() {
    let samples = [
        ("ACadSharp-master/samples/sample_AC1018.dwg", "AC1018"),
        ("ACadSharp-master/samples/sample_AC1024.dwg", "AC1024"),
        ("ACadSharp-master/samples/sample_AC1032.dwg", "AC1032"),
    ];

    for (path, label) in &samples {
        println!("\n═══════════════════ {} ═══════════════════", label);

        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                println!("Cannot read {}: {}", path, e);
                continue;
            }
        };

        // 1. Read reference file
        let cursor = Cursor::new(bytes);
        let reader = match DwgReader::from_reader(cursor) {
            Ok(r) => r.with_config(DwgReaderConfiguration {
                failsafe: true,
                keep_unknown_entities: true,
            }),
            Err(e) => {
                println!("Failed to create reader: {}", e);
                continue;
            }
        };

        let doc = match reader.read() {
            Ok(d) => d,
            Err(e) => {
                println!("Read failed: {}", e);
                continue;
            }
        };

        let orig_entities = doc.entities().count();
        let orig_blocks: usize = doc.block_records.iter().map(|b| b.entities.len()).sum();
        println!("1. Original: {} standalone, {} block entities, {} layers, {} blocks",
            orig_entities, orig_blocks, doc.layers.len(), doc.block_records.len());

        // 2. Write back to DWG
        let bytes2 = match DwgWriter::write(&doc) {
            Ok(b) => {
                println!("2. Written: {} bytes", b.len());
                b
            }
            Err(e) => {
                println!("2. Write FAILED: {}", e);
                continue;
            }
        };

        // 3. Read back our written version
        let cursor2 = Cursor::new(bytes2.clone());
        let reader2 = match DwgReader::from_reader(cursor2) {
            Ok(r) => r.with_config(DwgReaderConfiguration {
                failsafe: true,
                keep_unknown_entities: true,
            }),
            Err(e) => {
                println!("3. Reader creation FAILED: {}", e);
                continue;
            }
        };

        match reader2.read() {
            Ok(doc2) => {
                let rt_entities = doc2.entities().count();
                let rt_blocks: usize = doc2.block_records.iter().map(|b| b.entities.len()).sum();
                println!("3. Roundtrip: {} standalone, {} block entities, {} layers, {} blocks",
                    rt_entities, rt_blocks, doc2.layers.len(), doc2.block_records.len());

                if rt_entities + rt_blocks > 0 {
                    println!("   ✓ ENTITIES SURVIVE ROUNDTRIP!");
                } else {
                    println!("   ✗ No entities after roundtrip");
                }
            }
            Err(e) => {
                println!("3. Read FAILED: {}", e);
            }
        }

        // 4. Strict read
        let cursor3 = Cursor::new(bytes2);
        let reader3 = match DwgReader::from_reader(cursor3) {
            Ok(r) => r.with_config(DwgReaderConfiguration {
                failsafe: false,
                keep_unknown_entities: true,
            }),
            Err(e) => {
                println!("4. Reader creation FAILED: {}", e);
                continue;
            }
        };

        match reader3.read() {
            Ok(doc3) => {
                let n = doc3.entities().count();
                let b: usize = doc3.block_records.iter().map(|b| b.entities.len()).sum();
                println!("4. Strict: {} standalone, {} block entities", n, b);
            }
            Err(e) => {
                println!("4. Strict FAILED: {}", e);
            }
        }
    }
}
