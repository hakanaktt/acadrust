/// Diagnostic tool for R2010+ DWG entity reading.
use acadrust::io::dwg::{DwgReader, DwgReaderConfiguration};

fn main() {
    let config = DwgReaderConfiguration { failsafe: true, ..Default::default() };
    for name in &["AC1015", "AC1018", "AC1024", "AC1027", "AC1032"] {
        let path = format!("reference_samples/sample_{}.dwg", name);
        println!("\n========== {} ==========", name);
        match DwgReader::from_file(&path) {
            Ok(reader) => {
                match reader.with_config(config.clone()).read() {
                    Ok(doc) => {
                        let entity_count: usize = doc.block_records.iter()
                            .map(|r| r.entities.len())
                            .sum();
                        println!("  total entities in block records: {}", entity_count);
                        println!("  block_records={}, layers={}", doc.block_records.len(), doc.layers.len());
                        for record in doc.block_records.iter() {
                            println!("    block '{}': {} entities, handle={:#X}, layout={:#X}, block_entity={:#X}",
                                record.name, record.entities.len(),
                                record.handle.value(),
                                record.layout.value(),
                                record.block_entity_handle.value());
                        }
                    }
                    Err(e) => println!("  READ ERROR: {}", e),
                }
            }
            Err(e) => println!("  OPEN ERROR: {}", e),
        }
    }
}
