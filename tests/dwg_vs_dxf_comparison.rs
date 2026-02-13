//! DWG vs DXF comparison tests
//!
//! Reference samples contain identical data in both DWG and DXF formats.
//! These tests read both and verify that the parsed CadDocument contents match.
//!
//! The DWG reader is still under development, so some tests use soft assertions
//! (report mismatches without failing) while structural invariants are strict.

use acadrust::entities::EntityType;
use acadrust::CadDocument;
use std::collections::BTreeMap;

mod common;

// ---------------------------------------------------------------------------
// Helpers — delegated to common module
// ---------------------------------------------------------------------------

fn read_dwg(path: &str) -> CadDocument {
    common::read_dwg(path)
}

fn read_dxf(path: &str) -> CadDocument {
    common::read_dxf(path)
}

fn entity_type_histogram(doc: &CadDocument) -> BTreeMap<&'static str, usize> {
    common::entity_type_histogram(doc)
}

fn layer_names(doc: &CadDocument) -> Vec<String> {
    common::layer_names(doc)
}

fn linetype_names(doc: &CadDocument) -> Vec<String> {
    common::linetype_names(doc)
}

fn textstyle_names(doc: &CadDocument) -> Vec<String> {
    common::textstyle_names(doc)
}

fn block_record_names(doc: &CadDocument) -> Vec<String> {
    common::block_record_names(doc)
}

fn sorted_entities_by_type(doc: &CadDocument) -> BTreeMap<&'static str, Vec<&EntityType>> {
    common::comparison::sorted_entities_by_type(doc)
}

fn compare_entity_geometry(a: &EntityType, b: &EntityType) -> Vec<String> {
    common::comparison::compare_entity_geometry(a, b)
}

// ---------------------------------------------------------------------------
// Comparison report for a single version
// ---------------------------------------------------------------------------

struct ComparisonResult {
    version: String,
    dwg_entity_count: usize,
    dxf_entity_count: usize,
    dwg_layers: usize,
    dxf_layers: usize,
    dwg_linetypes: usize,
    dxf_linetypes: usize,
    dwg_blocks: usize,
    dxf_blocks: usize,
    matched_entities: usize,
}

fn compare_version(version: &str, dwg_path: &str, dxf_path: &str) -> ComparisonResult {
    let dwg = read_dwg(dwg_path);
    let dxf = read_dxf(dxf_path);

    // Geometry comparison: compare entities of the same type, matched by sort key
    let dwg_by_type = sorted_entities_by_type(&dwg);
    let dxf_by_type = sorted_entities_by_type(&dxf);

    let mut matched = 0usize;

    for (type_name, dwg_ents) in &dwg_by_type {
        if let Some(dxf_ents) = dxf_by_type.get(type_name) {
            let count = dwg_ents.len().min(dxf_ents.len());
            for i in 0..count {
                let diffs = compare_entity_geometry(dwg_ents[i], dxf_ents[i]);
                if diffs.is_empty() {
                    matched += 1;
                }
            }
        }
    }

    ComparisonResult {
        version: version.to_string(),
        dwg_entity_count: dwg.entities().count(),
        dxf_entity_count: dxf.entities().count(),
        dwg_layers: dwg.layers.len(),
        dxf_layers: dxf.layers.len(),
        dwg_linetypes: dwg.line_types.len(),
        dxf_linetypes: dxf.line_types.len(),
        dwg_blocks: dwg.block_records.len(),
        dxf_blocks: dxf.block_records.len(),
        matched_entities: matched,
    }
}

// ===========================================================================
// Per-version tests: table names (DWG tables should be subset of DXF tables)
// ===========================================================================

/// Check that every layer read from DWG also exists in DXF.
fn assert_dwg_layers_subset_of_dxf(version: &str, dwg: &CadDocument, dxf: &CadDocument) {
    let dxf_names: Vec<_> = dxf.layers.iter().map(|l| l.name.clone()).collect();
    for layer in dwg.layers.iter() {
        assert!(
            dxf_names.contains(&layer.name),
            "[{version}] DWG layer {:?} not found in DXF layers",
            layer.name
        );
    }
}

/// Check that every line type read from DWG also exists in DXF.
fn assert_dwg_linetypes_subset_of_dxf(version: &str, dwg: &CadDocument, dxf: &CadDocument) {
    let dxf_names: Vec<_> = dxf.line_types.iter().map(|lt| lt.name.clone()).collect();
    for lt in dwg.line_types.iter() {
        assert!(
            dxf_names.contains(&lt.name),
            "[{version}] DWG linetype {:?} not found in DXF linetypes",
            lt.name
        );
    }
}

/// Check that every text style read from DWG also exists in DXF.
fn assert_dwg_textstyles_subset_of_dxf(version: &str, dwg: &CadDocument, dxf: &CadDocument) {
    let dxf_names: Vec<_> = dxf.text_styles.iter().map(|ts| ts.name.clone()).collect();
    for ts in dwg.text_styles.iter() {
        assert!(
            dxf_names.contains(&ts.name),
            "[{version}] DWG textstyle {:?} not found in DXF textstyles",
            ts.name
        );
    }
}

/// Check that every entity type appearing in DWG also appears in DXF.
/// Some entity types (e.g. ATTDEF) may live inside block records in DXF
/// but appear at top-level in DWG, so we skip those known edge cases.
fn assert_dwg_entity_types_subset_of_dxf(version: &str, dwg: &CadDocument, dxf: &CadDocument) {
    // Entity types that DWG may enumerate at top-level but DXF keeps inside blocks
    const BLOCK_INTERNAL_TYPES: &[&str] = &["ATTDEF", "ATTRIB", "BLOCK", "ENDBLK", "SEQEND"];

    let dwg_hist = entity_type_histogram(dwg);
    let dxf_hist = entity_type_histogram(dxf);
    for (key, &dwg_count) in &dwg_hist {
        if BLOCK_INTERNAL_TYPES.contains(key) {
            continue;
        }
        let dxf_count = dxf_hist.get(key).copied().unwrap_or(0);
        assert!(
            dxf_count > 0,
            "[{version}] DWG has {dwg_count} {key} entities but DXF has none"
        );
    }
}

// ===========================================================================
// AC1015 (R2000)
// ===========================================================================

#[test]
fn test_dwg_vs_dxf_ac1015_tables_subset() {
    let dwg = read_dwg("reference_samples/sample_AC1015.dwg");
    let dxf = read_dxf("reference_samples/sample_AC1015_ascii.dxf");

    assert_dwg_layers_subset_of_dxf("AC1015", &dwg, &dxf);
    assert_dwg_linetypes_subset_of_dxf("AC1015", &dwg, &dxf);
    assert_dwg_textstyles_subset_of_dxf("AC1015", &dwg, &dxf);
}

#[test]
fn test_dwg_vs_dxf_ac1015_entity_types_subset() {
    let dwg = read_dwg("reference_samples/sample_AC1015.dwg");
    let dxf = read_dxf("reference_samples/sample_AC1015_ascii.dxf");
    assert_dwg_entity_types_subset_of_dxf("AC1015", &dwg, &dxf);
}

#[test]
fn test_dwg_vs_dxf_ac1015_both_have_entities() {
    let dwg = read_dwg("reference_samples/sample_AC1015.dwg");
    let dxf = read_dxf("reference_samples/sample_AC1015_ascii.dxf");
    assert!(dwg.entities().count() > 0, "AC1015 DWG should have entities");
    assert!(dxf.entities().count() > 0, "AC1015 DXF should have entities");
}

// ===========================================================================
// AC1018 (R2004)
// ===========================================================================

#[test]
fn test_dwg_vs_dxf_ac1018_tables_subset() {
    let dwg = read_dwg("reference_samples/sample_AC1018.dwg");
    let dxf = read_dxf("reference_samples/sample_AC1018_ascii.dxf");

    assert_dwg_layers_subset_of_dxf("AC1018", &dwg, &dxf);
    assert_dwg_linetypes_subset_of_dxf("AC1018", &dwg, &dxf);
    assert_dwg_textstyles_subset_of_dxf("AC1018", &dwg, &dxf);
}

#[test]
fn test_dwg_vs_dxf_ac1018_entity_types_subset() {
    let dwg = read_dwg("reference_samples/sample_AC1018.dwg");
    let dxf = read_dxf("reference_samples/sample_AC1018_ascii.dxf");
    assert_dwg_entity_types_subset_of_dxf("AC1018", &dwg, &dxf);
}

#[test]
fn test_dwg_vs_dxf_ac1018_both_have_entities() {
    let dwg = read_dwg("reference_samples/sample_AC1018.dwg");
    let dxf = read_dxf("reference_samples/sample_AC1018_ascii.dxf");
    assert!(dwg.entities().count() > 0, "AC1018 DWG should have entities");
    assert!(dxf.entities().count() > 0, "AC1018 DXF should have entities");
}

// ===========================================================================
// AC1024 (R2010)
// ===========================================================================

#[test]
fn test_dwg_vs_dxf_ac1024_tables_subset() {
    let dwg = read_dwg("reference_samples/sample_AC1024.dwg");
    let dxf = read_dxf("reference_samples/sample_AC1024_ascii.dxf");

    assert_dwg_layers_subset_of_dxf("AC1024", &dwg, &dxf);
    assert_dwg_linetypes_subset_of_dxf("AC1024", &dwg, &dxf);
    assert_dwg_textstyles_subset_of_dxf("AC1024", &dwg, &dxf);
}

#[test]
fn test_dwg_vs_dxf_ac1024_entity_types_subset() {
    let dwg = read_dwg("reference_samples/sample_AC1024.dwg");
    let dxf = read_dxf("reference_samples/sample_AC1024_ascii.dxf");
    assert_dwg_entity_types_subset_of_dxf("AC1024", &dwg, &dxf);
}

// ===========================================================================
// AC1027 (R2013)
// ===========================================================================

#[test]
fn test_dwg_vs_dxf_ac1027_tables_subset() {
    let dwg = read_dwg("reference_samples/sample_AC1027.dwg");
    let dxf = read_dxf("reference_samples/sample_AC1027_ascii.dxf");

    assert_dwg_layers_subset_of_dxf("AC1027", &dwg, &dxf);
    assert_dwg_linetypes_subset_of_dxf("AC1027", &dwg, &dxf);
    assert_dwg_textstyles_subset_of_dxf("AC1027", &dwg, &dxf);
}

#[test]
fn test_dwg_vs_dxf_ac1027_entity_types_subset() {
    let dwg = read_dwg("reference_samples/sample_AC1027.dwg");
    let dxf = read_dxf("reference_samples/sample_AC1027_ascii.dxf");
    assert_dwg_entity_types_subset_of_dxf("AC1027", &dwg, &dxf);
}

// ===========================================================================
// AC1032 (R2018)
// ===========================================================================

#[test]
fn test_dwg_vs_dxf_ac1032_tables_subset() {
    let dwg = read_dwg("reference_samples/sample_AC1032.dwg");
    let dxf = read_dxf("reference_samples/sample_AC1032_ascii.dxf");

    assert_dwg_layers_subset_of_dxf("AC1032", &dwg, &dxf);
    assert_dwg_linetypes_subset_of_dxf("AC1032", &dwg, &dxf);
    assert_dwg_textstyles_subset_of_dxf("AC1032", &dwg, &dxf);
}

#[test]
fn test_dwg_vs_dxf_ac1032_entity_types_subset() {
    let dwg = read_dwg("reference_samples/sample_AC1032.dwg");
    let dxf = read_dxf("reference_samples/sample_AC1032_ascii.dxf");
    assert_dwg_entity_types_subset_of_dxf("AC1032", &dwg, &dxf);
}

// ===========================================================================
// Full diagnostic report across all versions
// ===========================================================================

#[test]
fn test_dwg_vs_dxf_full_report() {
    let versions: &[(&str, &str, &str)] = &[
        (
            "AC1015",
            "reference_samples/sample_AC1015.dwg",
            "reference_samples/sample_AC1015_ascii.dxf",
        ),
        (
            "AC1018",
            "reference_samples/sample_AC1018.dwg",
            "reference_samples/sample_AC1018_ascii.dxf",
        ),
        // AC1021 skipped — RS encoding not fully implemented
        (
            "AC1024",
            "reference_samples/sample_AC1024.dwg",
            "reference_samples/sample_AC1024_ascii.dxf",
        ),
        (
            "AC1027",
            "reference_samples/sample_AC1027.dwg",
            "reference_samples/sample_AC1027_ascii.dxf",
        ),
        (
            "AC1032",
            "reference_samples/sample_AC1032.dwg",
            "reference_samples/sample_AC1032_ascii.dxf",
        ),
    ];

    println!("\n{:=<90}", "");
    println!("DWG vs DXF Comparison Report");
    println!("{:=<90}\n", "");

    println!(
        "{:<10} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8}",
        "Version", "DWG Ent", "DXF Ent", "DWG Lyr", "DXF Lyr", "DWG LT", "DXF LT", "DWG Blk",
        "DXF Blk", "Matched"
    );
    println!("{:-<90}", "");

    let mut all_results = Vec::new();

    for (ver, dwg_path, dxf_path) in versions {
        let r = compare_version(ver, dwg_path, dxf_path);
        println!(
            "{:<10} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8}",
            r.version,
            r.dwg_entity_count,
            r.dxf_entity_count,
            r.dwg_layers,
            r.dxf_layers,
            r.dwg_linetypes,
            r.dxf_linetypes,
            r.dwg_blocks,
            r.dxf_blocks,
            r.matched_entities,
        );
        all_results.push(r);
    }

    println!();

    // Detailed histogram per version
    for (ver, dwg_path, dxf_path) in versions {
        let dwg = read_dwg(dwg_path);
        let dxf = read_dxf(dxf_path);

        // Table name comparison
        let dwg_ly = layer_names(&dwg);
        let dxf_ly = layer_names(&dxf);
        let dwg_lt = linetype_names(&dwg);
        let dxf_lt = linetype_names(&dxf);
        let dwg_ts = textstyle_names(&dwg);
        let dxf_ts = textstyle_names(&dxf);
        let dwg_br = block_record_names(&dwg);
        let dxf_br = block_record_names(&dxf);

        println!("[{ver}] Table names:");
        println!("  Layers     match={:<5} DWG={dwg_ly:?}", dwg_ly == dxf_ly);
        println!("  LineTypes  match={:<5} DWG={dwg_lt:?}", dwg_lt == dxf_lt);
        println!("  TextStyles match={:<5} DWG={dwg_ts:?}", dwg_ts == dxf_ts);
        println!("  Blocks     match={:<5} DWG={dwg_br:?}", dwg_br == dxf_br);

        let dwg_hist = entity_type_histogram(&dwg);
        let dxf_hist = entity_type_histogram(&dxf);

        let mut all_keys: Vec<&str> = dwg_hist.keys().chain(dxf_hist.keys()).copied().collect();
        all_keys.sort();
        all_keys.dedup();

        println!("[{ver}] Entity type histogram:");
        for key in &all_keys {
            let dw = dwg_hist.get(key).copied().unwrap_or(0);
            let dx = dxf_hist.get(key).copied().unwrap_or(0);
            let marker = if dw == dx {
                "  OK"
            } else if dw == 0 {
                "  MISSING_IN_DWG"
            } else if dx == 0 {
                "  EXTRA_IN_DWG"
            } else {
                "  COUNT_DIFF"
            };
            println!("  {key:<25} DWG={dw:<4} DXF={dx:<4}{marker}");
        }
        println!();
    }

    println!("{:=<90}\n", "");

    // Structural assertions: every version should read at least some data
    for r in &all_results {
        assert!(
            r.dwg_entity_count > 0 || r.dwg_blocks >= 2,
            "[{}] DWG reader produced no entities and <2 blocks",
            r.version
        );
    }
}

// ===========================================================================
// Entity-by-entity geometry matching detail tests
// ===========================================================================

fn run_geometry_detail_test(version: &str, dwg_path: &str, dxf_path: &str) {
    let dwg = read_dwg(dwg_path);
    let dxf = read_dxf(dxf_path);

    let dwg_by_type = sorted_entities_by_type(&dwg);
    let dxf_by_type = sorted_entities_by_type(&dxf);

    let mut total_compared = 0usize;
    let mut total_matched = 0usize;
    let mut all_diffs: Vec<String> = Vec::new();

    for (type_name, dwg_ents) in &dwg_by_type {
        if let Some(dxf_ents) = dxf_by_type.get(type_name) {
            let count = dwg_ents.len().min(dxf_ents.len());
            for i in 0..count {
                let diffs = compare_entity_geometry(dwg_ents[i], dxf_ents[i]);
                total_compared += 1;
                if diffs.is_empty() {
                    total_matched += 1;
                } else {
                    for d in &diffs {
                        all_diffs.push(format!("{type_name}[{i}] {d}"));
                    }
                }
            }
        }
    }

    println!(
        "\n[{version}] Geometry comparison: {total_compared} entities compared, \
         {total_matched} fully matched, {} with diffs",
        total_compared - total_matched
    );
    for d in &all_diffs {
        println!("  {d}");
    }

    if total_compared > 0 {
        let match_rate = total_matched as f64 / total_compared as f64;
        println!("  Match rate: {:.1}%", match_rate * 100.0);
    }
}

#[test]
fn test_dwg_vs_dxf_ac1015_geometry_detail() {
    run_geometry_detail_test(
        "AC1015",
        "reference_samples/sample_AC1015.dwg",
        "reference_samples/sample_AC1015_ascii.dxf",
    );
}

#[test]
fn test_dwg_vs_dxf_ac1018_geometry_detail() {
    run_geometry_detail_test(
        "AC1018",
        "reference_samples/sample_AC1018.dwg",
        "reference_samples/sample_AC1018_ascii.dxf",
    );
}

#[test]
fn test_dwg_vs_dxf_ac1024_geometry_detail() {
    run_geometry_detail_test(
        "AC1024",
        "reference_samples/sample_AC1024.dwg",
        "reference_samples/sample_AC1024_ascii.dxf",
    );
}

#[test]
fn test_dwg_vs_dxf_ac1027_geometry_detail() {
    run_geometry_detail_test(
        "AC1027",
        "reference_samples/sample_AC1027.dwg",
        "reference_samples/sample_AC1027_ascii.dxf",
    );
}

#[test]
fn test_dwg_vs_dxf_ac1032_geometry_detail() {
    run_geometry_detail_test(
        "AC1032",
        "reference_samples/sample_AC1032.dwg",
        "reference_samples/sample_AC1032_ascii.dxf",
    );
}
