//! Roundtrip all sample DWG files in every supported version (2004+).
//!
//! For each .dwg in tests/roundtrip/, reads it, then writes it back in
//! AC1018 (2004), AC1021 (2007), AC1024 (2010), AC1027 (2013), AC1032 (2018).
//! The output files are saved as `<stem>_rt_<version>.dwg`.
//!
//! Usage:
//!   cargo run --example gen_all_versions_rt

use std::io::Cursor;
use std::path::{Path, PathBuf};

use acadrust::types::DxfVersion;
use acadrust::{DwgReader, DwgWriter, DxfReader, DxfWriter};

const VERSIONS: &[(DxfVersion, &str)] = &[
    (DxfVersion::AC1018, "AC1018"),
    (DxfVersion::AC1021, "AC1021"),
    (DxfVersion::AC1024, "AC1024"),
    (DxfVersion::AC1027, "AC1027"),
    (DxfVersion::AC1032, "AC1032"),
];

fn main() {
    let dir = Path::new("tests/roundtrip");
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("cannot read tests/roundtrip/")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension().map(|e| e == "dwg").unwrap_or(false)
                && !p.file_name().unwrap().to_string_lossy().contains("_rt")
        })
        .collect();
    files.sort();

    println!("Found {} source files in {}", files.len(), dir.display());
    println!("{}", "=".repeat(72));

    let mut total_ok = 0usize;
    let mut total_err = 0usize;

    for src in &files {
        let stem = src.file_stem().unwrap().to_string_lossy();
        println!("\n── {} ──", stem);

        // Read once
        let data = std::fs::read(src).expect("read source");
        let mut reader = DwgReader::from_stream(Cursor::new(&data));
        let mut doc = match reader.read() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("  ERROR reading {}: {}", src.display(), e);
                total_err += VERSIONS.len();
                continue;
            }
        };
        let orig_version = doc.version;
        println!(
            "  Read OK  (version={:?}, entities={})",
            orig_version,
            doc.entities().count()
        );

        for &(ver, ver_str) in VERSIONS {
            let out_name = format!("{}_rt_{}.dwg", stem, ver_str);
            let out_path = dir.join(&out_name);

            doc.version = ver;
            let result = DwgWriter::write_to_vec(&doc);
            match result {
                Ok(bytes) => {
                    let mut written = false;
                    for attempt in 0..5 {
                        match std::fs::write(&out_path, &bytes) {
                            Ok(_) => {
                                written = true;
                                break;
                            }
                            Err(_e) if attempt < 4 => {
                                std::thread::sleep(std::time::Duration::from_millis(200));
                                continue;
                            }
                            Err(e) => {
                                println!("  {} → WRITE FAILED after retries: {}", ver_str, e);
                                total_err += 1;
                            }
                        }
                    }
                    if !written {
                        continue;
                    }
                    // Verify: re-read
                    let mut r2 = DwgReader::from_stream(Cursor::new(&bytes));
                    match r2.read() {
                        Ok(doc2) => {
                            let ent_count = doc2.entities().count();
                            println!(
                                "  {} → {} bytes, read-back OK ({} entities)",
                                ver_str,
                                bytes.len(),
                                ent_count
                            );
                            total_ok += 1;
                        }
                        Err(e) => {
                            println!(
                                "  {} → {} bytes, READ-BACK FAILED: {}",
                                ver_str,
                                bytes.len(),
                                e
                            );
                            total_err += 1;
                        }
                    }
                }
                Err(e) => {
                    println!("  {} → WRITE FAILED: {}", ver_str, e);
                    total_err += 1;
                }
            }
        }
        // ── ASCII DXF roundtrip for each version ──
        for &(ver, ver_str) in VERSIONS {
            let out_name = format!("{}_rt_{}.dxf", stem, ver_str);
            let out_path = dir.join(&out_name);

            doc.version = ver;
            let writer = DxfWriter::new(&doc);
            match writer.write_to_vec() {
                Ok(bytes) => {
                    // Retry write in case of file locking (antivirus etc.)
                    let mut written = false;
                    for attempt in 0..5 {
                        match std::fs::write(&out_path, &bytes) {
                            Ok(_) => {
                                written = true;
                                break;
                            }
                            Err(_e) if attempt < 4 => {
                                std::thread::sleep(std::time::Duration::from_millis(200));
                                continue;
                            }
                            Err(e) => {
                                println!("  {} DXF → WRITE FAILED after retries: {}", ver_str, e);
                                total_err += 1;
                            }
                        }
                    }
                    if !written {
                        continue;
                    }
                    // Verify: re-read
                    match DxfReader::from_reader(Cursor::new(bytes.clone())) {
                        Ok(r2) => match r2.read() {
                            Ok(doc2) => {
                                let ent_count = doc2.entities().count();
                                println!(
                                    "  {} DXF → {} bytes, read-back OK ({} entities)",
                                    ver_str,
                                    bytes.len(),
                                    ent_count
                                );
                                total_ok += 1;
                            }
                            Err(e) => {
                                println!(
                                    "  {} DXF → {} bytes, READ-BACK FAILED: {}",
                                    ver_str,
                                    bytes.len(),
                                    e
                                );
                                total_err += 1;
                            }
                        },
                        Err(e) => {
                            println!(
                                "  {} DXF → {} bytes, READER INIT FAILED: {}",
                                ver_str,
                                bytes.len(),
                                e
                            );
                            total_err += 1;
                        }
                    }
                }
                Err(e) => {
                    println!("  {} DXF → WRITE FAILED: {}", ver_str, e);
                    total_err += 1;
                }
            }
        }

        // Restore original version
        doc.version = orig_version;
    }

    println!("\n{}", "=".repeat(72));
    println!("Results: {} OK, {} FAILED", total_ok, total_err);
    if total_err > 0 {
        std::process::exit(1);
    }
}
