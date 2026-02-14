//! Diagnostic: attempt to read back all generated DWG test files.

use std::path::Path;

fn main() {
    let test_dir = Path::new("test_output").join("cad_validation");
    if !test_dir.exists() {
        eprintln!("test_output directory not found. Run generate_test_dwgs first.");
        std::process::exit(1);
    }

    let mut total = 0;
    let mut ok = 0;
    let mut fail = 0;

    let mut entries: Vec<_> = std::fs::read_dir(test_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map_or(false, |ext| ext == "dwg")
        })
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let name = path.file_name().unwrap().to_string_lossy();
        total += 1;

        match acadrust::io::dwg::DwgReader::from_file(&path) {
            Ok(mut reader) => match reader.read() {
                Ok(doc) => {
                    let entity_count = doc.entities().count();
                    let block_entities: usize = doc
                        .block_records
                        .iter()
                        .map(|br| br.entities.len())
                        .sum();
                    let layers = doc.layers.iter().count();
                    println!(
                        "  OK  {name:<50} entities={entity_count} block_ents={block_entities} layers={layers}"
                    );
                    ok += 1;
                }
                Err(e) => {
                    println!("  FAIL {name:<49} read_err={e}");
                    fail += 1;
                }
            },
            Err(e) => {
                println!("  FAIL {name:<49} open_err={e}");
                fail += 1;
            }
        }
    }

    println!("\n--- Summary ---");
    println!("Total: {total}, OK: {ok}, FAIL: {fail}");
}
