/// Diagnostic tool: reads DWG files and reports what the reader finds.
///
/// Usage:
///     cargo run --bin diag_dwg_reader -- <path_to_dwg>
///     cargo run --bin diag_dwg_reader -- test_output/cad_validation/  (reads all .dwg in dir)

use acadrust::io::dwg::reader::dwg_reader::{DwgReader, DwgReaderConfiguration};
use std::env;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: diag_dwg_reader <path_to_dwg_or_directory>");
        std::process::exit(1);
    }

    let path = PathBuf::from(&args[1]);
    let mut files = Vec::new();

    if path.is_dir() {
        for entry in fs::read_dir(&path).expect("Failed to read directory") {
            let entry = entry.unwrap();
            let p = entry.path();
            if p.extension().map(|e| e == "dwg").unwrap_or(false) {
                files.push(p);
            }
        }
        files.sort();
    } else {
        files.push(path);
    }

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║          acadrust DWG Reader Diagnostic                     ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    let mut success = 0;
    let mut failed = 0;

    for file in &files {
        diagnose_dwg(file, &mut success, &mut failed);
    }

    println!();
    println!("════════════════════════════════════════════════════════════════");
    println!("Results: {} succeeded, {} failed out of {} total", success, failed, files.len());
}

fn diagnose_dwg(path: &Path, success: &mut usize, failed: &mut usize) {
    let filename = path.file_name().unwrap().to_string_lossy();
    let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    
    print!("{:<45} {:>8} bytes  ", filename, size);

    // Read raw bytes and inspect header
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            println!("❌ Cannot read file: {}", e);
            *failed += 1;
            return;
        }
    };

    // Check version string (first 6 bytes)
    if bytes.len() < 6 {
        println!("❌ File too small ({} bytes)", bytes.len());
        *failed += 1;
        return;
    }
    let version_str = String::from_utf8_lossy(&bytes[0..6]);
    print!("[{}] ", version_str);

    // Try to read with our reader (strict mode first, then failsafe)
    let cursor = Cursor::new(bytes.clone());
    let reader = match DwgReader::from_reader(cursor) {
        Ok(r) => r,
        Err(e) => {
            println!("❌ Cannot create reader: {}", e);
            *failed += 1;
            return;
        }
    };
    match reader.read() {
        Ok(doc) => {
            let standalone_count = doc.entities().count();
            let layer_count = doc.layers.len();
            // Also count entities inside block records
            let mut block_entity_count = 0usize;
            for br in doc.block_records.iter() {
                block_entity_count += br.entities.len();
            }
            let total_entities = standalone_count + block_entity_count;
            println!("✅ {} entities ({}+{} blk), {} layers, version={:?}",
                total_entities, standalone_count, block_entity_count, layer_count, doc.version);
            *success += 1;
        }
        Err(e) => {
            let err_str = format!("{}", e);
            // Truncate long errors
            let display = if err_str.len() > 80 {
                format!("{}...", &err_str[..80])
            } else {
                err_str
            };
            println!("❌ {}", display);
            *failed += 1;
        }
    }
}
