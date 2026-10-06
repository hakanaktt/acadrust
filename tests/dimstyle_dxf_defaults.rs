//! DIMSTYLE groups a DXF leaves out use the format's built-in values.
//!
//! Writers may drop DIMSTYLE groups equal to the built-in imperial defaults.
//! Reading those entries from the metric `DimStyle::new` values turned DIMZIN
//! 0 into 8 (so `1.000` became `1`), DIMTIH/DIMTOH on into off (vertical
//! dimensions read sideways) and DIMDEC 4 into 2.

use std::io::Cursor;

use acadrust::tables::DimStyle;
use acadrust::{CadDocument, DxfReader, DxfWriter};

/// Groups writers may omit when they hold the built-in value, by DXF code.
const OMITTED_GROUPS: &[&str] = &[
    "42", "43", "73", "74", "77", "78", "147", "171", "173", "271", "272", "274", "283", "284",
];

/// Write `doc`, then drop the listed groups from the DIMSTYLE entry called `name`.
fn dxf_without_groups(doc: &CadDocument, name: &str) -> Vec<u8> {
    let text = String::from_utf8(DxfWriter::new(doc).write_to_vec().expect("write")).expect("utf8");
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<&str> = Vec::with_capacity(lines.len());
    let mut in_entry = false;
    let mut i = 0;
    while i + 1 < lines.len() {
        let (code, value) = (lines[i].trim(), lines[i + 1]);
        if code == "0" {
            in_entry = false;
        }
        if code == "2"
            && value.trim() == name
            && matches!(out.last(), Some(v) if v.trim() == "DIMSTYLE" || v.trim().starts_with("AcDb"))
        {
            in_entry = true;
        }
        if in_entry && OMITTED_GROUPS.contains(&code) {
            i += 2;
            continue;
        }
        out.push(lines[i]);
        out.push(value);
        i += 2;
    }
    out.extend_from_slice(&lines[i..]);
    out.join("\r\n").into_bytes()
}

#[test]
fn omitted_dimstyle_groups_read_as_format_defaults() {
    let mut doc = CadDocument::new();
    let mut sparse = DimStyle::new("Sparse");
    // Values that differ from both the metric constructor and the format's
    // defaults, so the test would notice if the groups were not removed.
    sparse.dimzin = 12;
    sparse.dimtih = false;
    sparse.dimdec = 1;
    doc.dim_styles.add(sparse).expect("add style");

    let loaded = DxfReader::from_reader(Cursor::new(dxf_without_groups(&doc, "Sparse")))
        .expect("reader")
        .read()
        .expect("read");
    let style = loaded.dim_styles.get("Sparse").expect("Sparse survives");

    assert_eq!(style.dimzin, 0, "DIMZIN keeps trailing zeros");
    assert_eq!(style.dimtzin, 0, "DIMTZIN keeps trailing zeros");
    assert!(style.dimtih, "DIMTIH: inside text is horizontal");
    assert!(style.dimtoh, "DIMTOH: outside text is horizontal");
    assert_eq!(style.dimdec, 4);
    assert_eq!(style.dimtdec, 4);
    assert_eq!(style.dimtad, 0, "text centred on the line");
    assert!(!style.dimsah);
    assert_eq!(style.dimtolj, 1);
    assert_eq!(style.dimaltd, 2);
    assert_eq!(style.dimexo, 0.0625);
    assert_eq!(style.dimdli, 0.38);
    assert_eq!(style.dimgap, 0.09);
}

#[test]
fn explicit_dimstyle_groups_still_win() {
    let mut doc = CadDocument::new();
    let mut explicit = DimStyle::new("Explicit");
    explicit.dimzin = 8;
    explicit.dimtih = false;
    explicit.dimdec = 3;
    doc.dim_styles.add(explicit).expect("add style");

    let bytes = DxfWriter::new(&doc).write_to_vec().expect("write");
    let loaded = DxfReader::from_reader(Cursor::new(bytes))
        .expect("reader")
        .read()
        .expect("read");
    let style = loaded
        .dim_styles
        .get("Explicit")
        .expect("Explicit survives");

    assert_eq!(style.dimzin, 8);
    assert!(!style.dimtih);
    assert_eq!(style.dimdec, 3);
}

#[test]
fn new_styles_keep_their_metric_defaults() {
    let fresh = DimStyle::new("Fresh");
    assert_eq!(fresh.dimzin, 8);
    assert!(!fresh.dimtih);
    let defaults = DimStyle::dxf_defaults("Fresh");
    assert_eq!(defaults.name, "Fresh");
    assert_eq!(defaults.dimtxt, fresh.dimtxt, "shared values stay shared");
}
