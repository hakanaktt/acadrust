//! A standalone vertical-curve label group, written from its fields.
//!
//! The corpus's curve label groups (`AECC_VALIGNMENT_SAGCURVE_LABEL_GROUP`,
//! `AECC_VALIGNMENT_CRESTCURVE_LABEL_GROUP`) are keyed-label-grammar
//! records (see `super::super::labels`) whose label list is empty: the
//! group's labels live in its references (the profile's curves through
//! its label style) and in the class's own run the record carries after
//! its settings — the cached per-curve label data the application
//! recomputes on edit. The record form:
//!
//! ```text
//! header (the shared class header: 80, 0, 80, 80, 0, 0x10025F04, tag
//!         0b010010, 16 zero bytes, 80, 80, 0x04BE0000, 0x00FFFFFF, 010)
//! BL 0           (no keyed labels)
//! settings       (the label set's reference keys, two words, four
//!                 doubles, the stagger — see [`CurveLabelGroup`])
//! class run      (the recorded form's 125-bit run, verbatim)
//! B, BS 0x04BE   (close)
//! references: soft 0, soft 0, the profile (soft), the label style
//!   (hard), the view (soft)
//! texts: the class's `AeccImp*` name, four empty texts
//! ```
//!
//! The writer's constants are the corpus's create form (the #690/#708
//! precedent): the settings of the Sections-Project-Objects groups
//! (`69, 1`, `0.125, 0, 1/12, 1/6`, the label-set key `0x0F00B413`) and
//! that drawing's recorded 125-bit class run (profile 2's sag group —
//! the shortest of the drawing's four forms; the application recomputes
//! the run's cached values on edit). A group rebuilt from a recorded
//! drawing carries the drawing's own recorded run instead ([the round
//! pin proves the run rides the record bit for bit](super::super::labels)).

use super::support::*;
use crate::classes::civil::labels::LabelGroupRecord;
use crate::entities::EntityCommon;
use crate::error::DxfError;
use crate::types::Handle;
use crate::CadDocument;

/// Which vertical curve a group labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CurveKind {
    /// A sag (valley) curve: `AECC_VALIGNMENT_SAGCURVE_LABEL_GROUP`.
    Sag,
    /// A crest (summit) curve: `AECC_VALIGNMENT_CRESTCURVE_LABEL_GROUP`.
    Crest,
}

impl CurveKind {
    /// The record's dxf name.
    pub fn dxf(self) -> &'static str {
        match self {
            Self::Sag => "AECC_VALIGNMENT_SAGCURVE_LABEL_GROUP",
            Self::Crest => "AECC_VALIGNMENT_CRESTCURVE_LABEL_GROUP",
        }
    }

    /// The implementation class name the record's first text carries (the
    /// object enabler resolves it at load; the RX class name alone falls
    /// back to a zombie — the #713 lesson).
    pub fn class_text(self) -> &'static str {
        match self {
            Self::Sag => "AeccImpVAlignmentSagCurveLabeling",
            Self::Crest => "AeccImpVAlignmentCrestCurveLabeling",
        }
    }
}

/// The class's own run: its cached bits (whole bytes, then the run's
/// final partial byte, high-aligned) and their count.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ClassRun {
    pub bytes: Vec<u8>,
    pub bits: usize,
}

/// The class run the created groups get: the corpus's recorded 125-bit
/// form (Sections-Project-Objects, profile 2's sag group; see the module
/// docs).
pub fn default_class_run() -> ClassRun {
    ClassRun {
        bytes: vec![
            0xf0, 0x11, 0x56, 0x6b, 0x7a, 0x62, 0x09, 0x36, 0x80, 0xa8, 0x2a, 0x00, 0x00, 0x05, 0xf0, 0x20,
        ],
        bits: 125,
    }
}

/// A standalone vertical-curve label group, by its fields.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CurveLabelGroup {
    /// Which curve the group labels.
    pub kind: CurveKind,
    /// The settings' reference keys (keys that take their value from the
    /// handle stream — the profile label set; the corpus records carry
    /// the one key `0x0F00B413`).
    pub keys: Vec<(u32, crate::classes::civil::KeyValue)>,
    /// The settings' two words (`69, 1` in the corpus records).
    pub words: (u32, u32),
    /// The settings' four doubles: the default dimension anchor value,
    /// the weeding, and the two stagger line heights (`0.125, 0, 1/12,
    /// 1/6` in the corpus records).
    pub doubles: [f64; 4],
    /// The label stagger (0 none, 1 both sides, 2 right, 3 left).
    pub stagger: u32,
    /// The class's own run (see [`LabelGroupRecord`]); created groups get
    /// [`DEFAULT_CLASS_RUN`], a group rebuilt from a recorded drawing the
    /// drawing's recorded run.
    pub class_run: ClassRun,
}

impl CurveLabelGroup {
    /// The corpus's create form: the Sections-Project-Objects groups'
    /// settings and the recorded class run.
    pub fn new(kind: CurveKind) -> Self {
        Self {
            kind,
            keys: vec![(0x0F00_B413, crate::classes::civil::KeyValue::None)],
            words: (69, 1),
            doubles: [0.125, 0.0, 1.0 / 12.0, 1.0 / 6.0],
            stagger: 0,
            class_run: default_class_run(),
        }
    }

    /// The group's record, ready to encode.
    fn record(&self) -> LabelGroupRecord {
        LabelGroupRecord {
            tag: 0b01_0010,
            tag_bits: None,
            guid: [0; 16],
            word: 0x00FF_FFFF,
            bits: 0b010,
            class_word: 0x1002_5F04,
            labels: Vec::new(),
            settings: crate::classes::civil::labels::LabelGroupSettings {
                keys: self.keys.clone(),
                words: self.words,
                doubles: self.doubles,
                stagger: self.stagger,
            },
            residual: self.class_run.bytes.clone(),
            residual_bits: self.class_run.bits,
            close: false,
        }
    }
}

/// Writes a standalone vertical-curve label group for `profile`, labeled
/// with `style`, in `view`, on `layer` (the drawing's default when
/// `None`). The record is [`CurveLabelGroup`]; the references are the
/// samples': soft 0, soft 0, the profile (soft), the style (hard), the
/// view (soft).
///
/// Refuses an unknown layer, a profile that is not an `AECC_PROFILE` and
/// a class run wider than its bytes.
pub fn add_valignment_curve_label_group(
    doc: &mut CadDocument,
    profile: Handle,
    style: Handle,
    view: Handle,
    layer: Option<&str>,
    group: &CurveLabelGroup,
) -> Result<Handle, DxfError> {
    let is_profile = arch_of(doc, profile).is_some_and(|name| crate::classes::civil::kind_of(&name) == Some(crate::classes::civil::CivilKind::Profile));
    if !is_profile {
        return Err(invalid("the curve label group's profile is not an AECC_PROFILE"));
    }
    if !(arch_of(doc, style).is_some() || object(doc, style).is_some())
        || !(arch_of(doc, view).is_some() || object(doc, view).is_some())
    {
        return Err(invalid("the curve label group's style and view must be civil records"));
    }
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    if group.class_run.bits > group.class_run.bytes.len() * 8 {
        return Err(invalid("the class run's bit count exceeds its bytes"));
    }
    let owner = doc.header.model_space_block_handle;
    let handle = doc.allocate_handle();
    let common = EntityCommon {
        handle,
        owner_handle: owner,
        layer: layer.into(),
        layer_handle: Some(layer_handle),
        ..EntityCommon::default()
    };
    let refs = vec![soft_pointer(0), soft_pointer(0), soft_pointer(profile.value()), hard_pointer(style.value()), soft_pointer(view.value())];
    let strings = vec![group.kind.class_text().to_string(), String::new(), String::new(), String::new(), String::new()];
    let record = group.record();
    put_entity(
        doc,
        group.kind.dxf(),
        common,
        |w| crate::classes::civil::labels::write_record(w, &record),
        Body { refs, strings },
    )?;
    Ok(handle)
}

/// The dxf name of the entity at `handle`, when it is an Arch entity.
fn arch_of(doc: &CadDocument, handle: Handle) -> Option<String> {
    match doc.get_entity(handle) {
        Some(crate::entities::EntityType::Extended(e)) => match &e.data {
            crate::entities::ExtendedEntityData::Arch(a) => Some(a.dxf_name.clone()),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classes::civil::{add_profile, add_profile_view, ensure_civil};
    use crate::io::dwg::{DwgReader, DwgWriter};
    use crate::types::{DxfVersion, Vector2};

    fn drawing() -> CadDocument {
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        ensure_civil(&mut doc).unwrap();
        doc
    }

    fn roundtrip(doc: &CadDocument, tag: &str) -> CadDocument {
        let path = std::env::temp_dir().join(format!("rt_valign_curve_{tag}_{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        back
    }

    fn entity(doc: &CadDocument, h: Handle) -> crate::classes::arch::ArchEntityData {
        match doc.get_entity(h) {
            Some(crate::entities::EntityType::Extended(e)) => match &e.data {
                crate::entities::ExtendedEntityData::Arch(a) => a.clone(),
                _ => panic!("not arch"),
            },
            _ => panic!("missing"),
        }
    }

    /// A created curve label group round-trips its fields and decodes as
    /// the same form the oracle records carry.
    #[test]
    fn a_created_curve_label_group_round_trips() {
        let mut doc = drawing();
        let al = crate::classes::civil::add_alignment(&mut doc, "AL-C", &[Vector2::new(0.0, 0.0), Vector2::new(600.0, 0.0)], None).unwrap();
        let prof = add_profile(&mut doc, al, "PF-C", &[Vector2::new(0.0, 600.0), Vector2::new(200.0, 640.0), Vector2::new(600.0, 636.0)], &[0.0], None).unwrap();
        let view = add_profile_view(&mut doc, al, "PV-C", Vector2::new(0.0, 0.0), None).unwrap();
        let style = crate::classes::civil::add_point_label_style(&mut doc, "CURVE LABEL", |_| {}).unwrap();
        let group = CurveLabelGroup::new(CurveKind::Sag);
        let h = add_valignment_curve_label_group(&mut doc, prof, style, view, None, &group).unwrap();
        let back = roundtrip(&doc, "sag");
        let e = entity(&back, h);
        assert_eq!(e.dxf_name, "AECC_VALIGNMENT_SAGCURVE_LABEL_GROUP");
        assert_eq!(e.cpp_class_name, "AeccDbVAlignmentSagCurveLabeling");
        // The decoded record carries the written fields.
        let record = e.label_group_record().expect("the group decodes");
        assert!(record.labels.is_empty());
        assert_eq!(record.settings.words, (69, 1));
        assert_eq!(record.settings.keys, vec![(0x0F00_B413, crate::classes::civil::KeyValue::None)]);
        assert_eq!(record.residual_bits, default_class_run().bits);
        assert_eq!(record.residual, default_class_run().bytes);
        // References: soft 0, soft 0, profile (soft), style (hard), view (soft).
        let refs = e.typed_handles();
        assert_eq!(refs.len(), 5);
        assert_eq!(refs[0], (0, 4));
        assert_eq!(refs[1], (0, 4));
        assert_eq!(refs[2], (prof.value(), 4));
        assert_eq!(refs[3], (style.value(), 5));
        assert_eq!(refs[4], (view.value(), 4));
        assert_eq!(e.decode().strings[0], "AeccImpVAlignmentSagCurveLabeling");
    }

    /// The crest form and a named layer round-trip too.
    #[test]
    fn a_crest_group_on_a_named_layer_round_trips() {
        let mut doc = drawing();
        let _ = doc.layers.add(crate::tables::Layer::new("C-PROF-TEXT"));
        let al = crate::classes::civil::add_alignment(&mut doc, "AL-D", &[Vector2::new(0.0, 0.0), Vector2::new(600.0, 0.0)], None).unwrap();
        let prof = add_profile(&mut doc, al, "PF-D", &[Vector2::new(0.0, 600.0), Vector2::new(200.0, 640.0), Vector2::new(600.0, 636.0)], &[0.0], None).unwrap();
        let view = add_profile_view(&mut doc, al, "PV-D", Vector2::new(0.0, 0.0), None).unwrap();
        let style = crate::classes::civil::add_point_label_style(&mut doc, "CURVE LABEL", |_| {}).unwrap();
        let mut group = CurveLabelGroup::new(CurveKind::Crest);
        group.class_run.bytes[0] ^= 0xFF; // a run is carried verbatim, whatever it holds
        let h = add_valignment_curve_label_group(&mut doc, prof, style, view, Some("C-PROF-TEXT"), &group).unwrap();
        let back = roundtrip(&doc, "crest");
        let e = entity(&back, h);
        assert_eq!(e.dxf_name, "AECC_VALIGNMENT_CRESTCURVE_LABEL_GROUP");
        assert_eq!(e.decode().strings[0], "AeccImpVAlignmentCrestCurveLabeling");
        let record = e.label_group_record().unwrap();
        assert_eq!(record.residual, group.class_run.bytes);
        let common = match back.get_entity(h) {
            Some(crate::entities::EntityType::Extended(e)) => e.common.clone(),
            _ => panic!("missing"),
        };
        assert_eq!(common.layer, "C-PROF-TEXT");
    }

    /// The writer refuses a non-profile, an unknown layer and an
    /// over-long class run.
    #[test]
    fn the_writer_refuses_out_of_form_groups() {
        let mut doc = drawing();
        let al = crate::classes::civil::add_alignment(&mut doc, "AL-R", &[Vector2::new(0.0, 0.0), Vector2::new(600.0, 0.0)], None).unwrap();
        let prof = add_profile(&mut doc, al, "PF-R", &[Vector2::new(0.0, 600.0), Vector2::new(200.0, 640.0), Vector2::new(600.0, 636.0)], &[0.0], None).unwrap();
        let view = add_profile_view(&mut doc, al, "PV-R", Vector2::new(0.0, 0.0), None).unwrap();
        let style = crate::classes::civil::add_point_label_style(&mut doc, "CURVE LABEL", |_| {}).unwrap();
        let group = CurveLabelGroup::new(CurveKind::Sag);
        // An alignment is not a profile.
        assert!(add_valignment_curve_label_group(&mut doc, al, style, view, None, &group).is_err());
        assert!(add_valignment_curve_label_group(&mut doc, prof, style, view, Some("NO-SUCH"), &group).is_err());
        let mut long = CurveLabelGroup::new(CurveKind::Crest);
        long.class_run.bits = long.class_run.bytes.len() * 8 + 1;
        assert!(add_valignment_curve_label_group(&mut doc, prof, style, view, None, &long).is_err());
    }
}
