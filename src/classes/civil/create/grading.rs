//! Grading entities (`AECC_GRADING`): a record of this class carries no
//! geometry and no style of its own — only its schema head, two class-body
//! hard pointers (the parcel it grades from and a label style) and an
//! unmapped bit-coded tail (see [`super::super::grading::GradingEntity`]).
//! What a writer can do structurally is therefore a *record clone*: take an
//! app-written record's class stream verbatim, re-point its two references
//! at a parcel (and label style) of the drawing being written, and let the
//! application rebuild everything its tail caches.

use super::support::*;
use super::super::grading;
use crate::entities::{EntityCommon, EntityType, ExtendedEntityData};
use crate::error::DxfError;
use crate::io::dwg::dwg_reference_type::DwgReferenceType;
use crate::types::Handle;
use crate::CadDocument;

/// The class stream of an app-written grading record, ready to be written
/// into another drawing: the class body bits (verbatim, MSB first), the
/// class-body references with their kinds, the record's texts and the
/// graphic-image bytes.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GradingRecord {
    /// The class stream bits as `'0'`/`'1'` characters.
    pub bits: String,
    /// Class-body references in stream order (handle, kind byte).
    pub refs: Vec<(u64, u8)>,
    /// The record's texts.
    pub strings: Vec<String>,
    /// The record's graphic-image bytes, when it has them.
    pub graphic: Option<Vec<u8>>,
}

/// Extracts the clonable record of the grading entity `source`, if it is a
/// `AECC_GRADING` whose schema head this crate knows (the [`grading`]
/// grammar gate — a record the reader refuses is not cloned either).
pub fn grading_record(doc: &CadDocument, source: Handle) -> Option<GradingRecord> {
    let Some(EntityType::Extended(x)) = doc.get_entity(source) else { return None };
    let ExtendedEntityData::Arch(a) = &x.data else { return None };
    if a.dxf_name != "AECC_GRADING" {
        return None;
    }
    let handles: Vec<u64> = a.decode().handles;
    let (data, start, end) = a.main_stream()?;
    // The gate: only a record that decodes as a grading (constant schema
    // head, parcel and label style references present) is cloned.
    grading::entity(data, start, end, &handles)?;
    let bits: String = (start..end).map(|i| if data[i / 8] >> (7 - i % 8) & 1 == 1 { '1' } else { '0' }).collect();
    Some(GradingRecord { bits, refs: a.typed_handles(), strings: a.decode().strings, graphic: x.common.graphic_data.clone() })
}

/// Writes a grading entity from `record` (a [`grading_record`] extraction,
/// schema-gated there): the class stream verbatim, the record's references
/// with slot 1 (the parcel it grades from) re-pointed at `parcel` and slot
/// 4 (the label style) at `label_style` — the two pointers the #594
/// grammar maps; the other references keep the cloned values (they are
/// null in the samples). The clone takes layer "0" and the drawing's model
/// space.
pub fn add_grading(doc: &mut CadDocument, record: &GradingRecord, parcel: Handle, label_style: Handle) -> Result<Handle, DxfError> {
    if parcel.value() == 0 {
        return Err(invalid("a grading needs a parcel"));
    }
    let mut refs: Vec<Ref> = record.refs.iter().map(|&(h, k)| Ref(match k {
        2 => DwgReferenceType::SoftOwnership,
        3 => DwgReferenceType::HardOwnership,
        4 => DwgReferenceType::SoftPointer,
        5 => DwgReferenceType::HardPointer,
        _ => DwgReferenceType::Undefined,
    }, h)).collect();
    if refs.len() < 5 {
        return Err(invalid("the grading record is missing its references"));
    }
    refs[1] = hard_pointer(parcel.value());
    // The label style pointer: the caller's, or the drawing's default point
    // label style (an `AECC_LABEL_COLLECTOR_STYLE`, the class the slot
    // carries in the samples) when the drawing has none of its own.
    let label_style = if label_style.value() != 0 {
        label_style
    } else {
        super::labels::ensure_label_styles(doc).ok();
        Handle::new(super::labels::default_label_style(doc, super::labels::POINT_LABELS).unwrap_or(0))
    };
    if label_style.value() == 0 {
        return Err(invalid("the drawing has no label style to point the grading at"));
    }
    refs[4] = hard_pointer(label_style.value());
    let layer = doc.layers.get("0").map(|l| l.handle).ok_or_else(|| invalid("drawing has no layer 0"))?;
    let common = EntityCommon {
        handle: doc.allocate_handle(),
        owner_handle: doc.header.model_space_block_handle,
        layer: "0".into(),
        layer_handle: Some(layer),
        graphic_data: record.graphic.clone(),
        ..EntityCommon::default()
    };
    put_entity(doc, "AECC_GRADING", common, |w| bits(w, &record.bits), Body { refs, strings: record.strings.clone() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::dwg::{DwgReader, DwgWriter};
    use std::path::Path;

    /// The first `AECC_GRADING` of a drawing.
    fn first_grading(doc: &CadDocument) -> Option<Handle> {
        doc.entities().find_map(|e| match e {
            EntityType::Extended(x) => match &x.data {
                ExtendedEntityData::Arch(a) if a.dxf_name == "AECC_GRADING" => Some(x.common.handle),
                _ => None,
            },
            _ => None,
        })
    }

    #[test]
    fn clones_a_grading_record_into_a_new_drawing() {
        let path = Path::new("C:\\Program Files\\Autodesk\\AutoCAD 2027\\C3D\\Help\\Civil Tutorials\\Drawings\\Grading-3A.dwg");
        if !path.exists() {
            return;
        }
        let source = DwgReader::from_file(path).unwrap().read_with_stats().unwrap().document;
        let record = grading_record(&source, first_grading(&source).unwrap()).unwrap();
        // The schema head is the constant one of the #594 grammar.
        assert!(record.bits.starts_with("1010"));
        assert!(record.refs.len() >= 5);

        // A drawing this crate writes: a site with a parcel. The clone's
        // pointers go to them.
        let mut doc = CadDocument::new();
        doc.version = crate::types::DxfVersion::AC1032;
        crate::classes::civil::add_alignment(&mut doc, "AL-CL", &[crate::types::Vector2::new(0.0, -50.0), crate::types::Vector2::new(100.0, -50.0)], None).unwrap();
        let site = crate::classes::civil::add_site(&mut doc, "GW", "").unwrap();
        let parcel = crate::classes::civil::add_parcel(&mut doc, site, &[crate::types::Vector2::new(0.0, 0.0), crate::types::Vector2::new(100.0, 0.0), crate::types::Vector2::new(100.0, 50.0), crate::types::Vector2::new(0.0, 50.0)], None).unwrap();
        let label_style = Handle::new(match doc.get_entity(parcel) {
            Some(EntityType::Extended(x)) => match &x.data {
                ExtendedEntityData::Arch(a) => a.typed_handles().get(4).map(|r| r.0).unwrap_or(0),
                _ => 0,
            },
            _ => 0,
        });
        assert_ne!(label_style.value(), 0, "the parcel has no label style");
        let grading = add_grading(&mut doc, &record, parcel, label_style).unwrap();

        // The written record decodes as a grading of that parcel, and its
        // class stream is the source's bit for bit.
        let read = match doc.get_entity(grading) {
            Some(EntityType::Extended(x)) => match &x.data {
                ExtendedEntityData::Arch(a) => {
                    let handles: Vec<u64> = a.decode().handles;
                    let (data, start, end) = a.main_stream().unwrap();
                    let g = grading::entity(data, start, end, &handles).unwrap();
                    let bits: String = (start..end).map(|i| if data[i / 8] >> (7 - i % 8) & 1 == 1 { '1' } else { '0' }).collect();
                    (g, bits)
                }
                _ => panic!("not arch"),
            },
            _ => panic!("missing"),
        };
        assert_eq!(read.0.parcel, parcel.value());
        assert_eq!(read.0.label_style, label_style.value());
        assert_eq!(read.1, record.bits);

        // A file round-trip keeps it.
        let out = std::env::temp_dir().join("gw_clone_roundtrip.dwg");
        DwgWriter::write_to_file(&out, &doc).unwrap();
        let back = DwgReader::from_file(&out).unwrap().read_with_stats().unwrap().document;
        let handle = first_grading(&back).unwrap();
        let g = match back.get_entity(handle) {
            Some(EntityType::Extended(x)) => match &x.data {
                ExtendedEntityData::Arch(a) => {
                    let handles: Vec<u64> = a.decode().handles;
                    let (data, start, end) = a.main_stream().unwrap();
                    grading::entity(data, start, end, &handles).unwrap()
                }
                _ => panic!("not arch"),
            },
            _ => panic!("missing"),
        };
        assert_eq!(g.parcel, parcel.value());
        assert_eq!(g.label_style, label_style.value());
        let _ = std::fs::remove_file(&out);
    }

    #[test]
    fn a_foreign_record_is_not_cloned() {
        // An entity of another class: no record.
        let mut doc = CadDocument::new();
        doc.version = crate::types::DxfVersion::AC1032;
        let line = crate::entities::EntityType::Line(crate::entities::Line { common: EntityCommon { handle: doc.allocate_handle(), layer: "0".into(), ..EntityCommon::default() }, start: crate::types::Vector3::default(), end: crate::types::Vector3::new(1.0, 0.0, 0.0), thickness: 0.0, normal: crate::types::Vector3::new(0.0, 0.0, 1.0) });
        let handle = doc.add_entity(line).unwrap();
        assert_eq!(grading_record(&doc, handle), None);
    }
}
