//! The corpus sweep of the VALIGNMENT label-group records: every record
//! the lossless grammar admits (see `LabelGroupRecord`) must re-encode
//! bit for bit at its recorded position; every refusal is named (file,
//! dxf name) — the corpus round-trip pin of the standalone VAlignment
//! curve label-group round.
//!
//! Oracle: the Civil Tutorials Drawings corpus (D013) — every
//! `AECC_VALIGNMENT_*_LABEL_GROUP` record across its drawings.
//! Runs when `ACADRUST_CIVIL_TUTORIALS_DIR` is set.

use acadrust::entities::{EntityType, ExtendedEntityData};
use acadrust::DwgReader;

#[test]
fn corpus_valignment_label_groups_round_trip() {
    let Some(dir) = std::env::var_os("ACADRUST_CIVIL_TUTORIALS_DIR") else {
        println!("ACADRUST_CIVIL_TUTORIALS_DIR not set — nothing to sweep");
        return;
    };
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("corpus directory")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "dwg"))
        .collect();
    files.sort();
    let (mut admitted, mut refused, mut exact, mut files_with) = (0usize, 0usize, 0usize, 0usize);
    for path in &files {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };
        let mut reader = match DwgReader::from_file(path) {
            Ok(r) => r,
            Err(e) => {
                refused += 1;
                eprintln!("REFUSAL {name}: open failed: {e}");
                continue;
            }
        };
        let doc = match reader.read() {
            Ok(d) => d,
            Err(e) => {
                refused += 1;
                eprintln!("REFUSAL {name}: parse failed: {e}");
                continue;
            }
        };
        let mut file_admitted = 0usize;
        for e in doc.entities() {
            let EntityType::Extended(x) = e else { continue };
            let ExtendedEntityData::Arch(a) = &x.data else { continue };
            if !a.dxf_name.starts_with("AECC_VALIGNMENT_") || !a.dxf_name.ends_with("_LABEL_GROUP") {
                continue;
            }
            match a.label_group_record_at() {
                Some((record, start, end)) => {
                    admitted += 1;
                    file_admitted += 1;
                    // Re-encode; the record's bits must match the raw
                    // class body bit for bit at its recorded position.
                    let out = record.encoded();
                    let raw = a.raw_dwg_data.as_deref().expect("raw body");
                    let nbits = end - start;
                    assert!(out.len() * 8 >= nbits, "{name} {}: re-encode too short", a.dxf_name);
                    let mut mismatches = 0usize;
                    for i in 0..nbits {
                        let got = (out[i / 8] >> (7 - (i % 8))) & 1;
                        let at = start + i;
                        let want = (raw[at / 8] >> (7 - (at % 8))) & 1;
                        if got != want {
                            mismatches += 1;
                            if mismatches <= 3 {
                                eprintln!("DIFF {name} {} handle {:X}: bit {i}", a.dxf_name, x.common.handle.value());
                            }
                        }
                    }
                    assert_eq!(mismatches, 0, "{name} {}: {mismatches} bit diffs", a.dxf_name);
                    exact += 1;
                }
                None => {
                    refused += 1;
                    eprintln!("REFUSAL {name} {} handle {:X}: the grammar does not map the record", a.dxf_name, x.common.handle.value());
                }
            }
        }
        if file_admitted > 0 {
            files_with += 1;
            println!("{name}: {file_admitted} records, all exact");
        }
    }
    println!(
        "VALIGNMENT label groups: {admitted} admitted, {exact} round-trip bit-exact, {refused} refusals (named above), {files_with}/{} files carry admitted records",
        files.len()
    );
    assert_eq!(exact, admitted, "every admitted record must round-trip bit-exact");
}

#[test]
fn corpus_valignment_label_groups_by_class() {
    let Some(dir) = std::env::var_os("ACADRUST_CIVIL_TUTORIALS_DIR") else {
        return;
    };
    let mut counts: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
    for entry in std::fs::read_dir(&dir).expect("corpus directory").filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().is_none_or(|x| x != "dwg") {
            continue;
        }
        let Ok(mut reader) = DwgReader::from_file(&path) else { continue };
        let Ok(doc) = reader.read() else { continue };
        for e in doc.entities() {
            let EntityType::Extended(x) = e else { continue };
            let ExtendedEntityData::Arch(a) = &x.data else { continue };
            if !a.dxf_name.starts_with("AECC_VALIGNMENT_") || !a.dxf_name.ends_with("_LABEL_GROUP") {
                continue;
            }
            let entry = counts.entry(a.dxf_name.clone()).or_default();
            match a.label_group_record() {
                Some(_) => entry.0 += 1,
                None => entry.1 += 1,
            }
        }
    }
    for (dxf, (ok, refused)) in &counts {
        println!("{dxf}: {ok} admitted, {refused} refused");
    }
}
