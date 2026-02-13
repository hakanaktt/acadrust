//! Generates DXF files with all entity types for every supported version.
//! Files are written to `test_output/` so you can open them in your CAD application.
//!
//! Run with:  cargo test --test cad_roundtrip_output -- --nocapture --ignored

mod common;

use acadrust::entities::EntityType;
use acadrust::{CadDocument, DxfWriter};
use std::path::Path;

// ---------------------------------------------------------------------------
// Document with every entity type laid out in a visible grid
// ---------------------------------------------------------------------------

/// Re-export from shared common builders
fn create_all_entities_document() -> CadDocument {
    common::builders::create_all_entities_document()
}
// ---------------------------------------------------------------------------
// Helpers — delegated to common module
// ---------------------------------------------------------------------------

fn read_back(path: &str) -> CadDocument {
    common::read_dxf(path)
}

fn entity_type_counts(doc: &CadDocument) -> std::collections::BTreeMap<String, usize> {
    common::entity_type_counts(doc)
}

// Use common::ALL_VERSIONS for version iteration

// ---------------------------------------------------------------------------
// Main test — writes all versions, reads back, reports results
// ---------------------------------------------------------------------------

#[test]
#[ignore] // run explicitly: cargo test --test cad_roundtrip_output -- --ignored --nocapture
fn generate_all_version_dxf_files() {
    let out_dir = Path::new("test_output");
    std::fs::create_dir_all(out_dir).expect("Failed to create test_output/");

    let mut summary = Vec::new();

    for (ver, name) in &common::ALL_VERSIONS {
        // --- ASCII ---
        let ascii_name = format!("all_entities_{}_ascii.dxf", name);
        let ascii_path = out_dir.join(&ascii_name);
        {
            let mut doc = create_all_entities_document();
            doc.version = *ver;
            let wrote = doc.entity_count();
            match DxfWriter::new(doc).write_to_file(ascii_path.to_str().unwrap()) {
                Ok(_) => {
                    let rdoc = read_back(ascii_path.to_str().unwrap());
                    let read_total = rdoc.entity_count();
                    let unknown: usize = rdoc.entities()
                        .filter(|e| matches!(e, EntityType::Unknown(_)))
                        .count();
                    let sz = std::fs::metadata(&ascii_path).unwrap().len();

                    summary.push(format!(
                        "  {:<12} ASCII   {:>6} bytes   wrote {:>2}  read {:>2}  unknown {:>2}  {}",
                        name, sz, wrote, read_total, unknown,
                        if read_total == wrote && unknown == 0 { "PERFECT" }
                        else if unknown == 0 { "OK" }
                        else { "PARTIAL" }
                    ));
                }
                Err(e) => {
                    summary.push(format!("  {:<12} ASCII   SKIPPED (file locked: {})", name, e));
                }
            }
        }

        // --- Binary ---
        let bin_name = format!("all_entities_{}_binary.dxf", name);
        let bin_path = out_dir.join(&bin_name);
        {
            let mut doc = create_all_entities_document();
            doc.version = *ver;
            let wrote = doc.entity_count();
            match DxfWriter::new_binary(doc).write_to_file(bin_path.to_str().unwrap()) {
                Ok(_) => {
                    let rdoc = read_back(bin_path.to_str().unwrap());
                    let read_total = rdoc.entity_count();
                    let unknown: usize = rdoc.entities()
                        .filter(|e| matches!(e, EntityType::Unknown(_)))
                        .count();
                    let sz = std::fs::metadata(&bin_path).unwrap().len();

                    summary.push(format!(
                        "  {:<12} Binary  {:>6} bytes   wrote {:>2}  read {:>2}  unknown {:>2}  {}",
                        name, sz, wrote, read_total, unknown,
                        if read_total == wrote && unknown == 0 { "PERFECT" }
                        else if unknown == 0 { "OK" }
                        else { "PARTIAL" }
                    ));
                }
                Err(e) => {
                    summary.push(format!("  {:<12} Binary  SKIPPED (file locked: {})", name, e));
                }
            }
        }
    }

    // --- Detailed report for 2018 ASCII (most complete version) ---
    println!("\n===== DXF Round-Trip Test Output =====");
    println!("Files written to: test_output/\n");

    println!("--- Version Summary ---");
    for line in &summary {
        println!("{}", line);
    }

    // Detailed entity breakdown for 2018 ASCII
    let detail_path = out_dir.join("all_entities_2018_ascii.dxf");
    let orig = create_all_entities_document();
    let rdoc = read_back(detail_path.to_str().unwrap());
    let orig_counts = entity_type_counts(&orig);
    let read_counts = entity_type_counts(&rdoc);

    println!("\n--- Entity Detail (2018 ASCII) ---");
    println!("  {:<35} {:>6} {:>6}", "Type", "Wrote", "Read");
    let mut all_keys: Vec<_> = orig_counts.keys().chain(read_counts.keys()).cloned().collect();
    all_keys.sort();
    all_keys.dedup();
    for key in &all_keys {
        let w = orig_counts.get(key).copied().unwrap_or(0);
        let r = read_counts.get(key).copied().unwrap_or(0);
        let mark = if w == r { "OK" } else { "DIFF" };
        println!("  {:<35} {:>6} {:>6}  {}", key, w, r, mark);
    }

    println!("\n--- Files for CAD testing ---");
    for (_, name) in &common::ALL_VERSIONS {
        println!("  test_output/all_entities_{}_ascii.dxf", name);
        println!("  test_output/all_entities_{}_binary.dxf", name);
    }
    println!("\nTotal: {} files generated", common::ALL_VERSIONS.len() * 2);
}
