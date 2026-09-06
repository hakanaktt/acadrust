//! Regression tests for issue #51 -- inconsistency in DXF file handling.
//!
//! Four defects made DXF handling inconsistent between read and write:
//!
//! 1. Dictionary per-entry hard ownership was not canonicalized on read.
//!    The DXF writer emits code 360 for every entry of a dictionary whose
//!    wide hard-owner flag (280) is set, and for well-known hard-owner NOD
//!    keys (ACAD_LAYOUT, ACAD_PLOTSTYLENAME, ACAD_FIELD, ...), but the
//!    reader only recorded `hard_owner_entries` for literal 360 codes. A
//!    file produced by another tool (350 codes) therefore produced a
//!    different model after one write--read cycle, and documents grown a
//!    new `hard_owner_entries` list on every cycle.
//!
//! 2. MLINESTYLE start/end angles were written in degrees but read raw.
//!    The model stores radians (the DWG stream stores radians and the DXF
//!    writer converts with `to_degrees`), so every read--write cycle
//!    multiplied the angle by 180/- (90.0 -- 5156.62 -- 295452.57).
//!
//! 3. The object-remap pass in `resolve_references()` updated dictionary
//!    entries and owners but not `DictionaryWithDefault::default_handle`,
//!    leaving the dictionary pointing at a handle that had been moved
//!    (issue #51 comment by Apicqq).
//!
//! 4. Defaults created by `CadDocument::new()` before parsing (Standard
//!    text style / dimstyle, ByLayer/ByBlock linetypes, ...) kept their
//!    sequential handles even when the file reused those handles for its
//!    own records and lacked the default entry -- the writer then emitted
//!    duplicate handles such as `#1B` (issue #51 comment by Apicqq).
//!
//! Together these broke the issue's core expectation: reading the same
//! unmodified file must yield equal documents, and read--write--read cycles
//! must stabilize instead of mutating the document.

use std::collections::HashMap;
use std::io::Cursor;

use acadrust::entities::{Circle, EntityType, Line};
use acadrust::objects::{
    Dictionary, DictionaryWithDefault, ObjectType, PlaceHolder, XRecord, XRecordEntry, XRecordValue,
};
use acadrust::types::{DxfVersion, Handle, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};

fn write_dxf(doc: &CadDocument) -> Vec<u8> {
    DxfWriter::new(doc).write_to_vec().unwrap()
}

fn read_dxf(bytes: &[u8]) -> CadDocument {
    DxfReader::from_reader(Cursor::new(bytes.to_vec()))
        .unwrap()
        .read()
        .unwrap()
}

/// A document with entities plus OBJECTS-section dictionaries that exercise
/// the hard-owner paths: the root NOD (wide flag clear, canonical keys) and
/// an extension-style dictionary with the wide hard-owner flag set.
fn sample_document(version: DxfVersion) -> CadDocument {
    let mut doc = CadDocument::with_version(version);
    doc.add_entity(EntityType::Line(Line::from_coords(
        0.0, 0.0, 0.0, 10.0, 10.0, 0.0,
    )))
    .unwrap();
    doc.add_entity(EntityType::Circle(Circle::from_coords(5.0, 5.0, 0.0, 2.0)))
        .unwrap();

    let nod_handle = doc.header.named_objects_dict_handle;
    let x1_handle = doc.allocate_handle();
    let x2_handle = doc.allocate_handle();
    let ext_handle = doc.allocate_handle();

    let mut x1 = XRecord::new();
    x1.handle = x1_handle;
    x1.owner = ext_handle;
    x1.entries.push(XRecordEntry {
        code: 1,
        value: XRecordValue::String("first".to_string()),
    });
    doc.objects.insert(x1_handle, ObjectType::XRecord(x1));

    let mut x2 = XRecord::new();
    x2.handle = x2_handle;
    x2.owner = ext_handle;
    x2.entries.push(XRecordEntry {
        code: 1,
        value: XRecordValue::String("second".to_string()),
    });
    doc.objects.insert(x2_handle, ObjectType::XRecord(x2));

    // Extension-style dictionary: wide hard-owner flag set (280 = 1). The
    // writer emits its entries with code 360; the reader must record that
    // ownership so the model survives a round-trip unchanged.
    let mut ext = Dictionary::new();
    ext.handle = ext_handle;
    ext.owner = nod_handle;
    ext.hard_owner = true;
    ext.add_entry("MY_XREC_ONE", x1_handle);
    ext.add_entry("MY_XREC_TWO", x2_handle);
    doc.objects.insert(ext_handle, ObjectType::Dictionary(ext));

    if let Some(ObjectType::Dictionary(nod)) = doc.objects.get_mut(&nod_handle) {
        nod.add_entry("MY_EXTENSION", ext_handle);
    }
    doc
}

#[test]
fn dxf_read_is_deterministic() {
    // The literal symptom from the issue: reading the same unmodified file
    // twice (and more times) must produce equal documents.
    for version in [DxfVersion::AC1015, DxfVersion::AC1032] {
        let bytes = write_dxf(&sample_document(version));
        let first = read_dxf(&bytes);
        for i in 0..25 {
            let other = read_dxf(&bytes);
            assert_eq!(
                first, other,
                "{version:?}: read iteration {i} differs from the first read"
            );
        }
    }
}

#[test]
fn hard_owner_dictionary_survives_roundtrip() {
    let doc1 = read_dxf(&write_dxf(&sample_document(DxfVersion::AC1032)));
    let doc2 = read_dxf(&write_dxf(&doc1));
    let doc3 = read_dxf(&write_dxf(&doc2));

    // The wide hard-owner flag must keep every entry marked hard-owned.
    for (handle, object) in doc1.objects.iter() {
        if let ObjectType::Dictionary(dict) = object {
            if !dict.hard_owner {
                continue;
            }
            let rt = doc2.objects.get(handle).expect("dictionary lost");
            if let ObjectType::Dictionary(rt_dict) = rt {
                assert_eq!(dict.entries, rt_dict.entries, "entries changed");
                for (key, _) in &dict.entries {
                    assert!(
                        rt_dict.is_entry_hard_owner(key),
                        "entry {key:?} lost its hard-owner marking"
                    );
                }
            }
        }
    }

    assert_eq!(
        doc1.objects, doc2.objects,
        "OBJECTS section changed after one DXF round-trip"
    );
    assert_eq!(doc2, doc3, "round-trip cycle 2 != cycle 1");
}

#[test]
fn nod_soft_entries_stay_soft_on_roundtrip() {
    // BricsCAD's own DXF export writes every NOD entry except ACAD_FIELD as
    // a soft pointer (350) - including ACAD_LAYOUT and ACAD_PLOTSTYLENAME.
    // The reader must preserve that ownership mode so the round-trip is
    // stable (issue #51).
    let bytes = write_dxf(&sample_document(DxfVersion::AC1032));
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert!(
        text.contains("  3\r\nACAD_LAYOUT\r\n350\r\n"),
        "expected soft 350 entries in output"
    );

    let doc = read_dxf(&bytes);
    let nod_handle = doc.header.named_objects_dict_handle;
    if let Some(ObjectType::Dictionary(nod)) = doc.objects.get(&nod_handle) {
        assert!(
            !nod.is_entry_hard_owner("ACAD_LAYOUT"),
            "ACAD_LAYOUT must stay a soft pointer"
        );
        assert!(
            !nod.is_entry_hard_owner("ACAD_PLOTSTYLENAME"),
            "ACAD_PLOTSTYLENAME must stay a soft pointer"
        );
        assert!(
            !nod.is_entry_hard_owner("ACAD_GROUP"),
            "ACAD_GROUP stays a soft pointer"
        );
    } else {
        panic!("root NOD missing");
    }
}

#[test]
fn acad_field_entry_is_canonicalized_on_read() {
    // ACAD_FIELD is the one NOD key the writer always emits as a hard owner
    // (360); a file that carries it as 350 must still read back as
    // hard-owned so the next write is identical (issue #63).
    let mut doc = sample_document(DxfVersion::AC1032);
    let nod_handle = doc.header.named_objects_dict_handle;
    let xrec = doc
        .objects
        .iter()
        .find_map(|(h, o)| matches!(o, ObjectType::XRecord(_)).then_some(*h))
        .expect("sample has an XRecord");
    if let Some(ObjectType::Dictionary(nod)) = doc.objects.get_mut(&nod_handle) {
        nod.add_entry("ACAD_FIELD", xrec);
    }

    let bytes = write_dxf(&doc);
    let text = String::from_utf8(bytes.clone()).unwrap();
    let patched = text.replace(
        "  3\r\nACAD_FIELD\r\n360\r\n",
        "  3\r\nACAD_FIELD\r\n350\r\n",
    );
    assert_ne!(text, patched, "expected a hard-owner ACAD_FIELD in output");

    let doc = read_dxf(patched.as_bytes());
    let nod_handle = doc.header.named_objects_dict_handle;
    if let Some(ObjectType::Dictionary(nod)) = doc.objects.get(&nod_handle) {
        assert!(
            nod.is_entry_hard_owner("ACAD_FIELD"),
            "ACAD_FIELD must be canonicalized to hard-owner on read"
        );
    } else {
        panic!("root NOD missing");
    }
}

/// Extract the code-51/52 values of the OBJECTS-section MLINESTYLE record
/// with the given handle (skipping the identically-named CLASSES entry and
/// any other MLINESTYLE records).
fn mlinestyle_angle_codes(bytes: &[u8], handle: &str) -> (f64, f64) {
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    let lines: Vec<&str> = text.split("\r\n").collect();
    let read_code = |code: &str| -> f64 {
        for i in 0..lines.len().saturating_sub(2) {
            // The OBJECTS record is the one followed by its handle (code 5);
            // the CLASSES entry is followed by its C++ class name (code 2).
            if lines[i] == "MLINESTYLE" && lines[i + 1].trim() == "5" {
                if !lines[i + 2].trim().eq_ignore_ascii_case(handle) {
                    continue;
                }
                let mut j = i;
                while j < lines.len() - 1 {
                    if lines[j].trim() == code {
                        return lines[j + 1].trim().parse::<f64>().unwrap_or(f64::NAN);
                    }
                    j += 1;
                }
            }
        }
        f64::NAN
    };
    (read_code("51"), read_code("52"))
}

#[test]
fn mlinestyle_angles_stable_across_roundtrips() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut style = acadrust::objects::MLineStyle::new("TESTSTYLE");
    style.handle = doc.allocate_handle();
    style.start_angle = std::f64::consts::FRAC_PI_2; // 90 degrees
    style.end_angle = std::f64::consts::FRAC_PI_4; // 45 degrees
    let style_handle = style.handle;
    doc.objects
        .insert(style_handle, ObjectType::MLineStyle(style));

    // DXF carries degrees.
    let bytes = write_dxf(&doc);
    let (start_deg, end_deg) =
        mlinestyle_angle_codes(&bytes, &format!("{:X}", style_handle.value()));
    assert!(
        (start_deg - 90.0).abs() < 1e-9,
        "start angle must be degrees, got {start_deg}"
    );
    assert!(
        (end_deg - 45.0).abs() < 1e-9,
        "end angle must be degrees, got {end_deg}"
    );

    // The model carries radians and must not drift across cycles.
    let doc1 = read_dxf(&bytes);
    let doc2 = read_dxf(&write_dxf(&doc1));
    let doc3 = read_dxf(&write_dxf(&doc2));

    let get_angle = |doc: &CadDocument| -> (f64, f64) {
        match doc.objects.get(&style_handle) {
            Some(ObjectType::MLineStyle(style)) => (style.start_angle, style.end_angle),
            _ => panic!("MLineStyle lost"),
        }
    };
    let a = get_angle(&doc1);
    let b = get_angle(&doc2);
    let c = get_angle(&doc3);

    assert!(
        (a.0 - std::f64::consts::FRAC_PI_2).abs() < 1e-9,
        "start angle drifted: {:?}",
        a
    );
    assert!(
        (a.1 - std::f64::consts::FRAC_PI_4).abs() < 1e-9,
        "end angle drifted: {:?}",
        a
    );
    assert_eq!(a, b, "angles changed after one round-trip");
    assert_eq!(b, c, "angles changed after two round-trips");
}

#[test]
fn dwg_to_dxf_pipeline_is_stable() {
    // The reporter's workflow: DWG -> DXF (via DwgReader + DxfWriter), then
    // read the produced DXF repeatedly and round-trip it.
    let doc = sample_document(DxfVersion::AC1032);
    let dwg_bytes = DwgWriter::write_to_vec(&doc).unwrap();
    let from_dwg = DwgReader::from_stream(Cursor::new(dwg_bytes))
        .read()
        .unwrap();

    let dxf_bytes = write_dxf(&from_dwg);
    let a = read_dxf(&dxf_bytes);
    let b = read_dxf(&dxf_bytes);
    assert_eq!(a, b, "two reads of the same generated DXF differ");

    let doc2 = read_dxf(&write_dxf(&a));
    assert_eq!(a.objects, doc2.objects, "OBJECTS changed across round-trip");
}

#[test]
fn objects_maps_are_content_comparable() {
    // Sanity check for the helpers used above: a HashMap<Handle, ObjectType>
    // compares by content, so this must hold trivially.
    let mut m1: HashMap<Handle, ObjectType> = HashMap::new();
    let mut m2: HashMap<Handle, ObjectType> = HashMap::new();
    let mut d = Dictionary::new();
    d.handle = Handle::new(1);
    m1.insert(Handle::new(1), ObjectType::Dictionary(d.clone()));
    m2.insert(Handle::new(1), ObjectType::Dictionary(d));
    assert_eq!(m1, m2);
}

#[test]
#[test]
fn remaps_dictionary_with_default_handle() {
    // Issue #51 comment (Apicqq): resolve_references() remapped objects and
    // dictionary entries, but left DictionaryWithDefault's default (code
    // 340) pointing at the old handle - ezdxf's audit then reported a
    // "default object points to a handle that no longer exists".
    let mut document = CadDocument::new();
    let colliding = Handle::new(0x100);

    let mut line = Line::from_points(Vector3::ZERO, Vector3::UNIT_X);
    line.common.handle = colliding;
    document.add_entity(EntityType::Line(line)).unwrap();

    let mut placeholder = PlaceHolder::new();
    placeholder.handle = colliding;
    placeholder.owner = colliding;
    document
        .objects
        .insert(colliding, ObjectType::PlaceHolder(placeholder));

    let dwd_handle = Handle::new(0x200);
    let mut dwd = DictionaryWithDefault::new();
    dwd.handle = dwd_handle;
    dwd.owner = Handle::new(0x0C);
    dwd.entries.push(("Normal".to_string(), colliding));
    dwd.default_handle = colliding;
    document
        .objects
        .insert(dwd_handle, ObjectType::DictionaryWithDefault(dwd));

    document.resolve_references();

    // The placeholder must have been moved off the colliding handle, and
    // the dictionary's default must follow it.
    match document.objects.get(&dwd_handle) {
        Some(ObjectType::DictionaryWithDefault(d)) => {
            assert_ne!(
                d.default_handle, colliding,
                "default_handle was not remapped with its object"
            );
            assert_eq!(d.entries[0].1, d.default_handle);
            assert!(
                matches!(
                    document.objects.get(&d.default_handle),
                    Some(ObjectType::PlaceHolder(_))
                ),
                "default_handle points at a handle that no longer exists"
            );
        }
        other => panic!("dictionary with default lost: {:?}", other.is_some()),
    }
}

#[test]
fn surviving_default_entries_are_rehandled_on_read() {
    // Issue #51 comment (Apicqq): a file without a "Standard" dimstyle that
    // reuses the default Standard handle for *Paper_Space made the writer
    // emit two records with handle #1B - ezdxf's audit reported a
    // duplicate handle.
    let mut doc = CadDocument::new();
    let standard_handle = doc.dim_styles.get("Standard").unwrap().handle;
    assert!(doc.dim_styles.remove("Standard").is_some());
    if let Some(br) = doc.block_records.get_mut("*Paper_Space") {
        br.handle = standard_handle;
    }
    doc.header.paper_space_block_handle = standard_handle;

    let bytes = write_dxf(&doc);
    let rt = read_dxf(&bytes);

    // The file-sourced *Paper_Space handle must be preserved.
    let paper_space = rt
        .block_records
        .get("*Paper_Space")
        .expect("*Paper_Space lost")
        .handle;
    assert_eq!(
        paper_space, standard_handle,
        "file handle must be preserved"
    );

    // The surviving default Standard dimstyle must have been moved off the
    // colliding handle.
    let standard = rt
        .dim_styles
        .get("Standard")
        .expect("default Standard dimstyle missing");
    assert_ne!(
        standard.handle, paper_space,
        "default Standard dimstyle kept the colliding handle"
    );

    // Writing the read document must not emit two records with the same handle.
    let text = String::from_utf8(write_dxf(&rt)).unwrap();
    let needle = format!("\r\n  5\r\n{:X}\r\n", standard_handle.value());
    assert_eq!(
        text.matches(&needle).count(),
        1,
        "duplicate handle {:#X} in round-tripped output",
        standard_handle.value()
    );
}

#[test]
fn visualstyle_record_carries_code_291() {
    // Issue #51 comment (Apicqq): round-tripped files failed to load because
    // a written VISUALSTYLE was missing group code 291. Every version the
    // writer supports must emit it inside the VISUALSTYLE record.
    for version in [
        DxfVersion::AC1015,
        DxfVersion::AC1018,
        DxfVersion::AC1021,
        DxfVersion::AC1024,
        DxfVersion::AC1027,
        DxfVersion::AC1032,
    ] {
        let mut doc = CadDocument::with_version(version);
        let mut style = acadrust::objects::VisualStyle::new();
        style.handle = doc.allocate_handle();
        doc.objects
            .insert(style.handle, ObjectType::VisualStyle(style));

        let text = String::from_utf8(write_dxf(&doc)).unwrap();
        let lines: Vec<&str> = text.split("\r\n").collect();
        let mut found = false;
        for i in 0..lines.len().saturating_sub(1) {
            if lines[i] == "VISUALSTYLE" && lines[i + 1].trim() == "5" {
                let record_end = lines[i..]
                    .iter()
                    .position(|l| *l == "ENDSEC" || (*l == "DICTIONARY" || *l == "XRECORD"))
                    .unwrap_or(lines.len() - i);
                found = lines[i..i + record_end]
                    .windows(2)
                    .any(|w| w[0].trim() == "291");
                break;
            }
        }
        assert!(
            found,
            "{version:?}: VISUALSTYLE record is missing group code 291"
        );
    }
}

#[test]
fn handle_less_records_receive_handles_on_read() {
    // Issue #51 comment (Apicqq): files without handle records (R12 and
    // earlier) left table entries and objects with NULL handles, so the
    // writer emitted invalid `5 / 0` handle records. resolve_references()
    // must assign handles to everything that arrived without one.
    let mut doc = CadDocument::with_version(DxfVersion::AC1012);
    doc.add_entity(EntityType::Line(Line::from_coords(
        0.0, 0.0, 0.0, 1.0, 1.0, 0.0,
    )))
    .unwrap();

    let mut layer = acadrust::tables::Layer::new("NULL_LAYER");
    layer.handle = Handle::NULL;
    doc.layers.add(layer).unwrap();
    let mut style = acadrust::tables::TextStyle::new("NULL_STYLE");
    style.handle = Handle::NULL;
    doc.text_styles.add(style).unwrap();
    let mut dimstyle = acadrust::tables::DimStyle::new("NULL_DIM");
    dimstyle.handle = Handle::NULL;
    doc.dim_styles.add(dimstyle).unwrap();
    let mut vport = acadrust::tables::VPort::new("NULL_VPORT");
    vport.handle = Handle::NULL;
    doc.vports.add(vport).unwrap();
    let mut appid = acadrust::tables::AppId::new("NULL_APPID");
    appid.handle = Handle::NULL;
    doc.app_ids.add(appid).unwrap();

    let mut xrecord = XRecord::new();
    xrecord.handle = Handle::NULL;
    doc.objects
        .insert(Handle::NULL, ObjectType::XRecord(xrecord));
    // A dictionary whose map key is valid but whose record handle is NULL.
    let mut dict = Dictionary::new();
    dict.handle = Handle::NULL;
    doc.objects
        .insert(Handle::new(0x7001), ObjectType::Dictionary(dict));

    // The reader runs resolve_references() after parsing; do the same.
    doc.resolve_references();

    for entry in doc.layers.iter() {
        assert!(
            !entry.handle.is_null(),
            "layer {:?} still has no handle",
            entry.name
        );
    }
    for entry in doc.text_styles.iter() {
        assert!(
            !entry.handle.is_null(),
            "text style {:?} still has no handle",
            entry.name
        );
    }
    for entry in doc.dim_styles.iter() {
        assert!(
            !entry.handle.is_null(),
            "dim style {:?} still has no handle",
            entry.name
        );
    }
    for entry in doc.vports.iter() {
        assert!(
            !entry.handle.is_null(),
            "vport {:?} still has no handle",
            entry.name
        );
    }
    for entry in doc.app_ids.iter() {
        assert!(
            !entry.handle.is_null(),
            "appid {:?} still has no handle",
            entry.name
        );
    }
    for (handle, _) in doc.objects.iter() {
        assert!(
            !handle.is_null(),
            "object still stored under the NULL handle key"
        );
    }

    // The written output must not contain a single handle-valued 0.
    let output = String::from_utf8(write_dxf(&doc)).unwrap();
    let lines: Vec<&str> = output.split("\r\n").collect();
    let mut invalid = Vec::new();
    let mut current_record = String::new();
    for i in 0..lines.len().saturating_sub(1) {
        if lines[i] == "  0" {
            current_record = lines.get(i + 1).copied().unwrap_or("?").to_string();
        }
        let code = lines[i].trim();
        if matches!(code, "5" | "105" | "340" | "350" | "360")
            && lines.get(i + 1).map(|v| v.trim()) == Some("0")
        {
            invalid.push(format!("{} in {}", code, current_record));
        }
    }
    assert!(
        invalid.is_empty(),
        "invalid handle value 0 written: {:?}",
        invalid
    );
}

#[test]
fn dictionary_with_default_record_matches_bricscad_form() {
    // Issue #51 BricsCAD audit: BricsCAD's own DXF export does NOT write the
    // hard-owner flag (280) in ACDBDICTIONARYWDFLT records - the record goes
    // straight from the AcDbDictionary subclass to the cloning flag (281).
    // Writing 280 made BricsCAD's audit reject the plot style references.
    let doc = CadDocument::with_version(DxfVersion::AC1032);
    let text = String::from_utf8(write_dxf(&doc)).unwrap();
    let lines: Vec<&str> = text.split("\r\n").collect();
    for i in 1..lines.len() - 2 {
        // Record start: the class name must follow a group-code-0 line (the
        // CLASSES section also carries the name as a code-1 value).
        if lines[i] == "ACDBDICTIONARYWDFLT" && lines[i - 1] == "  0" {
            // Walk the record to the AcDbDictionary subclass marker; the
            // very next code must be the 281 cloning flag (no 280).
            let mut j = i + 1;
            while j < lines.len() - 1 && lines[j] != "AcDbDictionary" {
                j += 1;
            }
            assert_eq!(
                lines[j + 1].trim(),
                "281",
                "ACDBDICTIONARYWDFLT must not carry group code 280"
            );
        }
    }
}

#[test]
fn acad_regapp_record_is_written_first() {
    // Issue #51 BricsCAD audit: "RegApp ACAD has invalid index 1". CAD
    // applications map XDATA applications by table index and require the
    // ACAD record at index 0, regardless of the document's internal order.
    for version in [DxfVersion::AC1015, DxfVersion::AC1032] {
        let doc = sample_document(version);
        let text = String::from_utf8(write_dxf(&doc)).unwrap();
        let lines: Vec<&str> = text.split("\r\n").collect();
        let mut first_appid_name = None;
        for i in 0..lines.len() - 9 {
            if lines[i] == "  0" && lines[i + 1] == "APPID" {
                for j in i + 2..(i + 20).min(lines.len() - 1) {
                    if lines[j] == "  2" {
                        first_appid_name = Some(lines[j + 1].to_string());
                        break;
                    }
                }
                break;
            }
        }
        assert_eq!(
            first_appid_name.as_deref(),
            Some("ACAD"),
            "{version:?}: ACAD must be the first APPID record"
        );
    }
}

#[test]
fn unrestorable_assoc_objects_are_not_written() {
    // Issue #51 BricsCAD audit: the associative-framework objects written
    // from raw records could not be restored by CAD applications, which
    // audit-skipped them and left dangling dictionary entries behind. The
    // writer must drop them together with every entry pointing at them.
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    doc.add_entity(EntityType::Line(Line::from_coords(
        0.0, 0.0, 0.0, 1.0, 1.0, 0.0,
    )))
    .unwrap();

    let nod_handle = doc.header.named_objects_dict_handle;
    let assoc_handle = doc.allocate_handle();
    let mut assoc =
        acadrust::objects::AssociativeObject::new("ACDBASSOCVARIABLE", "AcDbAssocVariable");
    assoc.handle = assoc_handle;
    assoc.owner = nod_handle;
    doc.objects
        .insert(assoc_handle, ObjectType::Associative(assoc));

    if let Some(ObjectType::Dictionary(nod)) = doc.objects.get_mut(&nod_handle) {
        nod.add_entry("ACDBASSOC2DCONSTRAINTGROUP", assoc_handle);
    }

    let text = String::from_utf8(write_dxf(&doc)).unwrap();
    assert!(
        !text.contains("\r\nACDBASSOCVARIABLE\r\n  5\r\n"),
        "unrestorable assoc object must not be written"
    );
    assert!(
        !text.contains("  3\r\nACDBASSOC2DCONSTRAINTGROUP\r\n"),
        "dictionary entries pointing at dropped assoc objects must be filtered"
    );
}
