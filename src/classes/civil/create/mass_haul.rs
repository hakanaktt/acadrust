//! Mass haul records (`AECC_MASSHAULLINE`, `AECC_GRAPH_MASS_HAUL_VIEW`):
//! the line's record is its balance curve plus a quantity tail the
//! application recomputes from the sample line group the record points at
//! (see [`super::super::section`]); the view's record is its graph layout
//! plus pointers to the line it draws. What a writer can do structurally is
//! therefore a *record clone*: take an app-written record's class stream
//! verbatim, re-point its references at the sample line group (and styles)
//! of the drawing being written, and let the application rebuild everything
//! the tail caches.

use super::support::*;
use crate::entities::{EntityCommon, EntityType, ExtendedEntityData};
use crate::error::DxfError;
use crate::io::dwg::dwg_reference_type::DwgReferenceType;
use crate::types::Handle;
use crate::CadDocument;

/// The class stream of an app-written mass haul line, ready to be written
/// into another drawing: the class body bits (verbatim, MSB first), the
/// class-body references with their kinds, the record's texts and the
/// graphic-image bytes.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MassHaulLineRecord {
    /// The class stream bits as `'0'`/`'1'` characters.
    pub bits: String,
    /// Class-body references in stream order (handle, kind byte).
    /// `[0]` is a null soft pointer, `[1]` the sample line group the
    /// quantities come from, `[2]` the mass haul line style.
    pub refs: Vec<(u64, u8)>,
    /// The record's texts.
    pub strings: Vec<String>,
    /// The record's graphic-image bytes, when it has them.
    pub graphic: Option<Vec<u8>>,
}

/// The class stream of an app-written mass haul view, like a
/// [`MassHaulLineRecord`]. The references: `[2]` the view style, `[3]` the
/// sample line group, `[4]` the mass haul line (soft) and `[10]` the line
/// again (hard); the other slots are null in the samples.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MassHaulViewRecord {
    pub bits: String,
    pub refs: Vec<(u64, u8)>,
    pub strings: Vec<String>,
    pub graphic: Option<Vec<u8>>,
}

/// One extracted class stream: the body bits, the references with their
/// kinds, the texts and the graphic-image bytes.
type ClassStream = (String, Vec<(u64, u8)>, Vec<String>, Option<Vec<u8>>);

fn class_stream(doc: &CadDocument, source: Handle, dxf: &str) -> Option<ClassStream> {
    let Some(EntityType::Extended(x)) = doc.get_entity(source) else { return None };
    let ExtendedEntityData::Arch(a) = &x.data else { return None };
    if a.dxf_name != dxf {
        return None;
    }
    let (data, start, end) = a.main_stream()?;
    let bits: String = (start..end).map(|i| if data[i / 8] >> (7 - i % 8) & 1 == 1 { '1' } else { '0' }).collect();
    Some((bits, a.typed_handles(), a.decode().strings, x.common.graphic_data.clone()))
}

/// Extracts the clonable record of the mass haul line `source`, if it is a
/// `AECC_MASSHAULLINE` whose balance curve this crate decodes (the
/// `mass_haul_points` gate — a record the reader finds no curve in is not
/// cloned either).
pub fn mass_haul_line_record(doc: &CadDocument, source: Handle) -> Option<MassHaulLineRecord> {
    let Some(EntityType::Extended(x)) = doc.get_entity(source) else { return None };
    let ExtendedEntityData::Arch(a) = &x.data else { return None };
    if a.dxf_name != "AECC_MASSHAULLINE" || a.mass_haul_points().is_empty() {
        return None;
    }
    let (bits, refs, strings, graphic) = class_stream(doc, source, "AECC_MASSHAULLINE")?;
    Some(MassHaulLineRecord { bits, refs, strings, graphic })
}

/// Extracts the clonable record of the mass haul view `source`, if it is a
/// `AECC_MASS_HAUL_VIEW` whose frame this crate decodes (the
/// `mass_haul_view_frame` gate).
pub fn mass_haul_view_record(doc: &CadDocument, source: Handle) -> Option<MassHaulViewRecord> {
    let Some(EntityType::Extended(x)) = doc.get_entity(source) else { return None };
    let ExtendedEntityData::Arch(a) = &x.data else { return None };
    if a.dxf_name != "AECC_MASS_HAUL_VIEW" || a.mass_haul_view_frame().is_none() {
        return None;
    }
    let (bits, refs, strings, graphic) = class_stream(doc, source, "AECC_MASS_HAUL_VIEW")?;
    Some(MassHaulViewRecord { bits, refs, strings, graphic })
}

fn refs_of(typed: &[(u64, u8)]) -> Vec<Ref> {
    typed.iter().map(|&(h, k)| Ref(match k {
        2 => DwgReferenceType::SoftOwnership,
        3 => DwgReferenceType::HardOwnership,
        4 => DwgReferenceType::SoftPointer,
        5 => DwgReferenceType::HardPointer,
        _ => DwgReferenceType::Undefined,
    }, h)).collect()
}

fn put_clone(doc: &mut CadDocument, dxf: &str, record_bits: &str, refs: Vec<Ref>, strings: Vec<String>, graphic: Option<Vec<u8>>) -> Result<Handle, DxfError> {
    let layer = doc.layers.get("0").map(|l| l.handle).ok_or_else(|| invalid("drawing has no layer 0"))?;
    let common = EntityCommon {
        handle: doc.allocate_handle(),
        owner_handle: doc.header.model_space_block_handle,
        layer: "0".into(),
        layer_handle: Some(layer),
        graphic_data: graphic,
        ..EntityCommon::default()
    };
    put_entity(doc, dxf, common, |w| bits(w, record_bits), Body { refs, strings })
}

/// Writes a mass haul line from `record` (a [`mass_haul_line_record`]
/// extraction, schema-gated there): the class stream verbatim, the sample
/// line group reference (slot 1) re-pointed at `sample_line_group` and the
/// style reference (slot 2) at `line_style` — `Handle::NULL` leaves the
/// style slot null, which the application replaces with its default. The
/// clone takes layer "0" and the drawing's model space.
pub fn add_mass_haul_line(doc: &mut CadDocument, record: &MassHaulLineRecord, sample_line_group: Handle, line_style: Handle) -> Result<Handle, DxfError> {
    if sample_line_group.value() == 0 {
        return Err(invalid("a mass haul line needs a sample line group"));
    }
    if record.refs.len() < 3 {
        return Err(invalid("the mass haul line record is missing its references"));
    }
    let mut refs = refs_of(&record.refs);
    refs[1] = hard_pointer(sample_line_group.value());
    refs[2] = hard_pointer(line_style.value());
    put_clone(doc, "AECC_MASSHAULLINE", &record.bits, refs, record.strings.clone(), record.graphic.clone())
}

/// Writes a mass haul view from `record` (a [`mass_haul_view_record`]
/// extraction): the view style (slot 2) re-pointed at `view_style`
/// (`Handle::NULL` leaves it null), the sample line group (slot 3) at
/// `sample_line_group` and both line references (slots 4 and 10) at `line`
/// — the handle [`add_mass_haul_line`] returned.
pub fn add_mass_haul_view(doc: &mut CadDocument, record: &MassHaulViewRecord, line: Handle, sample_line_group: Handle, view_style: Handle) -> Result<Handle, DxfError> {
    if line.value() == 0 {
        return Err(invalid("a mass haul view needs a mass haul line"));
    }
    if sample_line_group.value() == 0 {
        return Err(invalid("a mass haul view needs a sample line group"));
    }
    if record.refs.len() < 11 {
        return Err(invalid("the mass haul view record is missing its references"));
    }
    let mut refs = refs_of(&record.refs);
    refs[2] = hard_pointer(view_style.value());
    refs[3] = hard_pointer(sample_line_group.value());
    refs[4] = soft_pointer(line.value());
    refs[10] = hard_pointer(line.value());
    put_clone(doc, "AECC_MASS_HAUL_VIEW", &record.bits, refs, record.strings.clone(), record.graphic.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::dwg::{DwgReader, DwgWriter};
    use std::path::Path;

    fn first_of(doc: &CadDocument, dxf: &str) -> Option<Handle> {
        doc.entities().find_map(|e| match e {
            EntityType::Extended(x) => match &x.data {
                ExtendedEntityData::Arch(a) if a.dxf_name == dxf => Some(x.common.handle),
                _ => None,
            },
            _ => None,
        })
    }

    #[test]
    fn clones_mass_haul_records_into_a_new_drawing() {
        let path = Path::new("C:\\Program Files\\Autodesk\\AutoCAD 2027\\C3D\\Help\\Civil Tutorials\\Drawings\\Mass Haul-2.dwg");
        if !path.exists() {
            return;
        }
        let source = DwgReader::from_file(path).unwrap().read_with_stats().unwrap().document;
        let source_line = first_of(&source, "AECC_MASSHAULLINE").unwrap();
        let source_view = first_of(&source, "AECC_MASS_HAUL_VIEW").unwrap();
        let line_record = mass_haul_line_record(&source, source_line).unwrap();
        let view_record = mass_haul_view_record(&source, source_view).unwrap();
        let source_points = {
            let x = match source.get_entity(source_line) {
                Some(EntityType::Extended(x)) => x,
                _ => panic!("missing"),
            };
            let ExtendedEntityData::Arch(a) = &x.data else { panic!("not arch") };
            a.mass_haul_points()
        };
        assert!(source_points.len() >= 2);

        // A drawing this crate writes: an alignment long enough for the
        // curve's station span and a sample line group on it. The clones'
        // pointers go to the group.
        let mut doc = CadDocument::new();
        doc.version = crate::types::DxfVersion::AC1032;
        let alignment = crate::classes::civil::add_alignment(&mut doc, "AL-CL", &[crate::types::Vector2::new(0.0, -50.0), crate::types::Vector2::new(1700.0, -50.0)], None).unwrap();
        let group = crate::classes::civil::add_sample_line_group(&mut doc, alignment, "SLG-1", None).unwrap();
        crate::classes::civil::add_sample_line(&mut doc, group, "SL-1", 100.0, 10.0, 10.0, None).unwrap();

        let line = add_mass_haul_line(&mut doc, &line_record, group, Handle::NULL).unwrap();
        let view = add_mass_haul_view(&mut doc, &view_record, line, group, Handle::NULL).unwrap();
        assert_ne!(line.value(), 0);

        // The written line decodes with the source's curve, bit for bit,
        // and points at the new group.
        let read_line = match doc.get_entity(line) {
            Some(EntityType::Extended(x)) => match &x.data {
                ExtendedEntityData::Arch(a) => a,
                _ => panic!("not arch"),
            },
            _ => panic!("missing"),
        };
        assert_eq!(read_line.mass_haul_points(), source_points);
        let (data, start, end) = read_line.main_stream().unwrap();
        let written_bits: String = (start..end).map(|i| if data[i / 8] >> (7 - i % 8) & 1 == 1 { '1' } else { '0' }).collect();
        assert_eq!(written_bits, line_record.bits);
        assert_eq!(read_line.typed_handles()[1].0, group.value());
        assert_eq!(read_line.typed_handles()[2].0, 0);

        // The written view decodes with the source's frame and points at
        // the new line and group.
        let read_view = match doc.get_entity(view) {
            Some(EntityType::Extended(x)) => match &x.data {
                ExtendedEntityData::Arch(a) => a,
                _ => panic!("not arch"),
            },
            _ => panic!("missing"),
        };
        assert_eq!(read_view.mass_haul_view_frame(), {
            let x = match source.get_entity(source_view) {
                Some(EntityType::Extended(x)) => x,
                _ => panic!("missing"),
            };
            let ExtendedEntityData::Arch(a) = &x.data else { panic!("not arch") };
            a.mass_haul_view_frame()
        });
        assert_eq!(read_view.typed_handles()[4].0, line.value());
        assert_eq!(read_view.typed_handles()[10].0, line.value());
        assert_eq!(read_view.typed_handles()[3].0, group.value());

        // A file round-trip keeps both.
        let out = std::env::temp_dir().join("mhw_clone_roundtrip.dwg");
        DwgWriter::write_to_file(&out, &doc).unwrap();
        let back = DwgReader::from_file(&out).unwrap().read_with_stats().unwrap().document;
        let back_line = first_of(&back, "AECC_MASSHAULLINE").unwrap();
        let back_view = first_of(&back, "AECC_MASS_HAUL_VIEW").unwrap();
        let a = match back.get_entity(back_line) {
            Some(EntityType::Extended(x)) => match &x.data {
                ExtendedEntityData::Arch(a) => a,
                _ => panic!("not arch"),
            },
            _ => panic!("missing"),
        };
        assert_eq!(a.mass_haul_points(), source_points);
        assert_eq!(a.typed_handles()[1].0, group.value());
        let a = match back.get_entity(back_view) {
            Some(EntityType::Extended(x)) => match &x.data {
                ExtendedEntityData::Arch(a) => a,
                _ => panic!("not arch"),
            },
            _ => panic!("missing"),
        };
        assert_eq!(a.mass_haul_view_frame().unwrap().origin, read_view.mass_haul_view_frame().unwrap().origin);
        assert_eq!(a.typed_handles()[10].0, back_line.value());
        let _ = std::fs::remove_file(&out);
    }

    #[test]
    fn a_foreign_record_is_not_cloned() {
        // An entity of another class: no record.
        let mut doc = CadDocument::new();
        doc.version = crate::types::DxfVersion::AC1032;
        let line = crate::entities::EntityType::Line(crate::entities::Line { common: EntityCommon { handle: doc.allocate_handle(), layer: "0".into(), ..EntityCommon::default() }, start: crate::types::Vector3::default(), end: crate::types::Vector3::new(1.0, 0.0, 0.0), thickness: 0.0, normal: crate::types::Vector3::new(0.0, 0.0, 1.0) });
        let handle = doc.add_entity(line).unwrap();
        assert_eq!(mass_haul_line_record(&doc, handle), None);
        assert_eq!(mass_haul_view_record(&doc, handle), None);
    }
}
