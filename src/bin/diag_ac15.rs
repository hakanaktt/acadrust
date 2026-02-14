//! Diagnostic: Test AC1015 write + read roundtrip
use acadrust::document::CadDocument;
use acadrust::io::dwg::writer::dwg_writer::DwgWriter;
use acadrust::io::dwg::reader::dwg_reader::{DwgReader, DwgReaderConfiguration};
use acadrust::types::DxfVersion;

fn main() {
    // Create a minimal document
    let mut doc = CadDocument::new();
    doc.version = DxfVersion::AC1015;

    println!("Writing AC1015 DWG...");
    let bytes = match DwgWriter::write(&doc) {
        Ok(b) => {
            println!("  Written {} bytes", b.len());
            b
        }
        Err(e) => {
            eprintln!("  WRITE FAILED: {e:?}");
            return;
        }
    };

    // Dump first 0x80 bytes (file header)
    println!("\nFile header (first 0x61 = 97 bytes):");
    for (i, chunk) in bytes[..0x61.min(bytes.len())].chunks(16).enumerate() {
        print!("  {:04X}: ", i * 16);
        for b in chunk {
            print!("{:02X} ", b);
        }
        println!();
    }

    // Parse the file header manually to see section locators
    if bytes.len() >= 0x61 {
        let version_str = std::str::from_utf8(&bytes[0..6]).unwrap_or("???");
        println!("\nVersion string: {version_str}");

        let preview_addr = i32::from_le_bytes([bytes[0x0D], bytes[0x0E], bytes[0x0F], bytes[0x10]]);
        println!("Preview address: {preview_addr} (0x{preview_addr:X})");

        let code_page = u16::from_le_bytes([bytes[0x13], bytes[0x14]]);
        println!("Code page: {code_page}");

        let num_records = i32::from_le_bytes([bytes[0x15], bytes[0x16], bytes[0x17], bytes[0x18]]);
        println!("Number of section locator records: {num_records}");

        let mut offset = 0x19; // after num_records (note: the writer writes it as i16 at 0x15)
        // Actually the writer writes a i16 at 0x15, so num_records should be read as i16
        let num_records_i16 = i16::from_le_bytes([bytes[0x15], bytes[0x16]]);
        println!("Number of section locator records (i16): {num_records_i16}");

        // The section records start at offset 0x17 (after the 2-byte i16 count)
        let mut rec_offset = 0x17;
        for i in 0..num_records_i16 {
            if rec_offset + 9 > bytes.len() {
                println!("  Record {i}: TRUNCATED at offset {rec_offset}");
                break;
            }
            let number = bytes[rec_offset];
            let seeker = i32::from_le_bytes([
                bytes[rec_offset + 1],
                bytes[rec_offset + 2],
                bytes[rec_offset + 3],
                bytes[rec_offset + 4],
            ]);
            let size = i32::from_le_bytes([
                bytes[rec_offset + 5],
                bytes[rec_offset + 6],
                bytes[rec_offset + 7],
                bytes[rec_offset + 8],
            ]);
            println!(
                "  Record {i}: number={number}, seeker={seeker} (0x{seeker:X}), size={size} (0x{size:X})"
            );
            rec_offset += 9;
        }

        println!("\nTotal file size: {} (0x{:X})", bytes.len(), bytes.len());
    }

    // Now try to read it back (strict mode)
    println!("\nReading AC1015 DWG back (strict)...");
    let cursor = std::io::Cursor::new(bytes.clone());
    let config = DwgReaderConfiguration {
        failsafe: false,
        ..Default::default()
    };
    match DwgReader::from_reader(cursor) {
        Ok(reader) => match reader.with_config(config).read() {
            Ok(doc) => {
                println!("  READ OK (strict)! Version: {:?}", doc.version);
                println!("  Layers: {}", doc.layers.len());
            }
            Err(e) => {
                eprintln!("  READ FAILED (strict): {e}");
            }
        },
        Err(e) => {
            eprintln!("  from_reader FAILED: {e:?}");
        }
    }

    // Try failsafe mode too
    println!("\nReading AC1015 DWG back (failsafe)...");
    let cursor2 = std::io::Cursor::new(bytes.clone());
    let config2 = DwgReaderConfiguration {
        failsafe: true,
        ..Default::default()
    };
    match DwgReader::from_reader(cursor2) {
        Ok(reader) => match reader.with_config(config2).read() {
            Ok(doc) => {
                println!("  READ OK (failsafe)! Version: {:?}", doc.version);
                println!("  Layers: {}", doc.layers.len());
            }
            Err(e) => {
                eprintln!("  READ FAILED (failsafe): {e}");
            }
        },
        Err(e) => {
            eprintln!("  from_reader FAILED (failsafe): {e:?}");
        }
    }

    // Save to disk for inspection
    let path = "test_output/diag_ac15.dwg";
    let _ = std::fs::create_dir_all("test_output");
    std::fs::write(path, &bytes).unwrap();
    println!("\nSaved to {path}");
}
