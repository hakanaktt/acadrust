//! Semantic round-trip validation of the annotative
//! `AcDb*ObjectContextData` encoder.
//!
//! Strategy: the golden R2018 file `~/Downloads/0718-mbmdmc.dwg` carries 134
//! real `AcDbBlkRefObjectContextData` leaves (class 533). The reader models each
//! into an `ObjectContextData`.
//!
//! - `baseline_roundtrip_preserves_all_contexts` proves modeling did not
//!   regress the round-trip: a plain save/reload preserves every modeled leaf.
//! - `encoder_preserves_context_fields_and_scale_link` verifies that the field
//!   encoder preserves both the modeled payload and its annotation-scale link.
//!
//! Skips (does not fail) when the golden file is absent, so CI stays green.

use std::io::Cursor;

use acadrust::objects::{
    DimContext, DimSubtype, MTextColumns, MTextContext, ObjectContextData, ObjectContextKind,
    ObjectType, Scale,
};
use acadrust::types::{DxfVersion, Vector2, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, Handle};

const GOLDEN: &str = "/home/hakanseven/Downloads/0718-mbmdmc.dwg";

fn load_golden() -> Option<CadDocument> {
    if std::fs::metadata(GOLDEN).is_err() {
        eprintln!("SKIP: golden file {GOLDEN} not present");
        return None;
    }
    let mut reader = DwgReader::from_file(GOLDEN).expect("open golden");
    Some(reader.read().expect("read golden"))
}

fn roundtrip(doc: &CadDocument) -> CadDocument {
    let bytes = DwgWriter::write_to_vec(doc).expect("dwg write");
    let mut r = DwgReader::from_stream(Cursor::new(bytes));
    r.read().expect("dwg re-read")
}

#[test]
fn baseline_roundtrip_preserves_all_contexts() {
    let Some(doc) = load_golden() else { return };

    let ctx0 = doc.context_scales.len();
    let blk0 = blkref_fields(&doc);
    eprintln!(
        "loaded: context_scales={ctx0} modeled_blkref={}",
        blk0.len()
    );
    assert!(blk0.len() >= 134, "expected the 134 BLKREF contexts");

    let rt = roundtrip(&doc);
    let ctx1 = rt.context_scales.len();
    let blk1 = blkref_fields(&rt);

    assert_eq!(ctx0, ctx1, "context_scales count changed on round-trip");
    assert_eq!(blk0, blk1, "modeled BLKREF fields changed on round-trip");
}

/// A BLKREF leaf's decoded fields + the annotation scale it resolves to (the
/// interop-critical link), keyed by handle.
#[derive(Debug, Clone, PartialEq)]
struct BlkFields {
    class_version: i16,
    is_default: bool,
    scale_target: Handle,
    rotation: f64,
    insertion: [f64; 3],
    scale_factor: [f64; 3],
}

fn blkref_fields(doc: &CadDocument) -> std::collections::BTreeMap<Handle, BlkFields> {
    doc.objects
        .iter()
        .filter_map(|(h, o)| match o {
            ObjectType::ObjectContextData(c) => match &c.kind {
                ObjectContextKind::BlkRef {
                    rotation,
                    insertion,
                    scale_factor,
                } => Some((
                    *h,
                    BlkFields {
                        class_version: c.class_version,
                        is_default: c.is_default,
                        scale_target: c.scale,
                        rotation: *rotation,
                        insertion: [insertion.x, insertion.y, insertion.z],
                        scale_factor: [scale_factor.x, scale_factor.y, scale_factor.z],
                    },
                )),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[test]
fn encoder_preserves_context_fields_and_scale_link() {
    let Some(doc) = load_golden() else { return };

    // Ground truth: the fields AutoCAD stored + which SCALE each leaf points at.
    let original = blkref_fields(&doc);
    assert!(original.len() >= 134, "expected the 134 BLKREF contexts");

    // Recognised context objects are always encoded from their modeled fields.
    let rt = roundtrip(&doc);
    let produced = blkref_fields(&rt);

    assert_eq!(
        original.len(),
        produced.len(),
        "leaf count changed after encode"
    );

    let mut mism = 0usize;
    for (h, orig) in &original {
        match produced.get(h) {
            None => {
                eprintln!("leaf {h:?} vanished");
                mism += 1;
            }
            Some(got) if got != orig => {
                if mism < 3 {
                    eprintln!("leaf {h:?} FIELD MISMATCH:\n  orig {orig:?}\n  got  {got:?}");
                }
                mism += 1;
            }
            _ => {}
        }
    }
    assert_eq!(
        mism, 0,
        "{mism} BLKREF leaves lost fields/scale-link after field-encoding"
    );

    // The re-encode must be a stable semantic fixed point.
    let twice = blkref_fields(&roundtrip(&rt));
    assert_eq!(produced, twice, "field encoder is not semantically stable");
}

// ── Synthesis path (what OCS uses to CREATE contexts from scratch) ──────────

/// Build a fresh R2018 document carrying one annotative context leaf of `kind`
/// that references a synthesized SCALE, round-trip it, and return the reloaded
/// leaf's decoded fields (scale target + kind). Exercises the whole create
/// path: class registration → object insertion → DWG write → read-back.
fn synth_roundtrip_leaf(
    class_name: &str,
    kind: ObjectContextKind,
) -> (Handle, ObjectContextKind, i16, bool) {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    doc.register_object_context_class(class_name);

    let scale_h = doc.allocate_handle();
    let mut scale = Scale::new("1:50", 1.0, 50.0);
    scale.handle = scale_h;
    doc.objects.insert(scale_h, ObjectType::Scale(scale));

    let owner_h = doc.allocate_handle();
    let leaf_h = doc.allocate_handle();
    let ctx = ObjectContextData {
        handle: leaf_h,
        owner_handle: owner_h,
        reactors: vec![owner_h],
        xdictionary_handle: None,
        class_version: 3,
        is_default: true,
        scale: scale_h,
        kind,
    };
    doc.objects
        .insert(leaf_h, ObjectType::ObjectContextData(ctx));

    let rt = roundtrip(&doc);
    let (h, c) = rt
        .objects
        .iter()
        .find_map(|(h, o)| match o {
            ObjectType::ObjectContextData(c) => Some((*h, c.clone())),
            _ => None,
        })
        .expect("context leaf survived round-trip");
    // The scale link must resolve back to a SCALE object in the reloaded doc.
    assert!(
        matches!(rt.objects.get(&c.scale), Some(ObjectType::Scale(_))),
        "leaf scale handle {:?} must resolve to a SCALE",
        c.scale
    );
    assert!(
        rt.context_scales.contains_key(&h),
        "leaf must be registered in context_scales"
    );
    (h, c.kind, c.class_version, c.is_default)
}

#[test]
fn synth_blkref_context_roundtrips() {
    let kind = ObjectContextKind::BlkRef {
        rotation: 0.75,
        insertion: Vector3::new(123.5, -42.25, 7.0),
        scale_factor: Vector3::new(2.0, 3.0, 4.0),
    };
    let (_h, got, cv, def) =
        synth_roundtrip_leaf("ACDB_BLKREFOBJECTCONTEXTDATA_CLASS", kind.clone());
    assert_eq!(cv, 3);
    assert!(def);
    assert_eq!(
        got, kind,
        "BLKREF fields did not survive synthesis round-trip"
    );
}

#[test]
fn synth_text_context_roundtrips() {
    let kind = ObjectContextKind::Text {
        horizontal_mode: 1,
        rotation: 0.5,
        insertion: Vector2::new(10.0, 20.0),
        alignment: Vector2::new(-3.0, 4.0),
    };
    let (_h, got, cv, def) = synth_roundtrip_leaf("ACDB_TEXTOBJECTCONTEXTDATA_CLASS", kind.clone());
    assert_eq!(cv, 3);
    assert!(def);
    assert_eq!(
        got, kind,
        "TEXT fields did not survive synthesis round-trip"
    );
}

#[test]
fn synth_mtext_context_roundtrips() {
    // Non-columned MTEXT (the common single-representation case).
    let kind = ObjectContextKind::MText(MTextContext {
        attachment: 2,
        x_axis_dir: Vector3::new(1.0, 0.0, 0.0),
        insertion: Vector3::new(7306.97, 12427.8, 0.0),
        rect_width: 39.26,
        rect_height: 17.6,
        extents_width: 1.146,
        extents_height: 0.0975,
        column_type: 0,
        columns: None,
    });
    let (_h, got, _cv, _def) =
        synth_roundtrip_leaf("ACDB_MTEXTOBJECTCONTEXTDATA_CLASS", kind.clone());
    assert_eq!(
        got, kind,
        "MTEXT (no columns) fields did not survive round-trip"
    );
}

#[test]
fn synth_mtext_columns_roundtrips() {
    // Dynamic columns with explicit heights — exercises the column branch.
    let kind = ObjectContextKind::MText(MTextContext {
        attachment: 1,
        x_axis_dir: Vector3::new(0.7071, 0.7071, 0.0),
        insertion: Vector3::new(1.0, 2.0, 3.0),
        rect_width: 100.0,
        rect_height: 50.0,
        extents_width: 90.0,
        extents_height: 45.0,
        column_type: 2,
        columns: Some(MTextColumns {
            num_heights: 3,
            width: 30.0,
            gutter: 5.0,
            auto_height: false,
            flow_reversed: true,
            heights: vec![10.0, 20.0, 15.0],
        }),
    });
    let (_h, got, _cv, _def) =
        synth_roundtrip_leaf("ACDB_MTEXTOBJECTCONTEXTDATA_CLASS", kind.clone());
    assert_eq!(
        got, kind,
        "MTEXT (columns) fields did not survive round-trip"
    );
}

fn dim_ctx(subtype: DimSubtype) -> DimContext {
    DimContext {
        def_pt: Vector2::new(11.5, 22.5),
        is_def_textloc: true,
        text_rotation: 0.25,
        block: Handle::from(0x2Au64),
        b293: false,
        dimtofl: true,
        dimosxd: false,
        dimatfit: true,
        dimtix: false,
        dimtmove: true,
        override_code: 3,
        has_arrow2: true,
        flip_arrow2: false,
        flip_arrow1: true,
        subtype,
    }
}

#[test]
fn synth_aligned_dim_context_roundtrips() {
    let kind = ObjectContextKind::Dim(dim_ctx(DimSubtype::Aligned {
        dimline_pt: Vector3::new(1.0, 2.0, 3.0),
    }));
    let (_h, got, _cv, _def) =
        synth_roundtrip_leaf("ACDB_ALDIMOBJECTCONTEXTDATA_CLASS", kind.clone());
    assert_eq!(got, kind, "aligned-dim fields did not survive round-trip");
    // The dimension's block hard-pointer must survive too.
    if let ObjectContextKind::Dim(d) = &got {
        assert_eq!(d.block, Handle::from(0x2Au64), "dim block handle lost");
    }
}

#[test]
fn synth_ordinate_dim_context_roundtrips() {
    // Two-point subtype — exercises the multi-3BD path.
    let kind = ObjectContextKind::Dim(dim_ctx(DimSubtype::Ordinate {
        feature_location_pt: Vector3::new(4.0, 5.0, 6.0),
        leader_endpt: Vector3::new(7.0, 8.0, 9.0),
    }));
    let (_h, got, _cv, _def) =
        synth_roundtrip_leaf("ACDB_ORDDIMOBJECTCONTEXTDATA_CLASS", kind.clone());
    assert_eq!(got, kind, "ordinate-dim fields did not survive round-trip");
}
