//! Civil 3D classes.
//!
//! Alignments, profiles, surfaces, corridors, and their labels and styles are
//! registered `AECC_*` classes built on the AEC base classes, so the DWG
//! reader keeps them in the Arch containers ([`ArchEntityData`] and
//! [`ArchObject`]) and same-version saves write the
//! native body back unchanged. This module names each class and reads the
//! geometry the class body stores.

mod alignment;
mod bits;
mod corridor;
mod labels;
mod profile;
mod sample_lines;
mod subassembly;
mod tin;

pub use alignment::{AlignmentElement, AlignmentElementKind, GeometryPoint};
pub use labels::{parse_format, property, LabelPart, LabelText};
pub use profile::{ProfileViewFrame, Pvi};
pub use sample_lines::SampleLines;
pub use subassembly::{AssemblyLayout, Attachment, SubassemblyShape};
pub use tin::{TinSurface, TinTriangle, TinVertex};

use super::arch::{ArchEntityData, ArchObject};
use crate::types::Vector2;

/// What a Civil 3D entity is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CivilKind {
    Alignment,
    /// Vertical alignment (profile) along an alignment.
    Profile,
    ProfileView,
    TinSurface,
    Corridor,
    Assembly,
    Subassembly,
    Parcel,
    SampleLineGroup,
    /// Label group or single label attached to another Civil 3D object.
    Label,
    /// A Civil 3D entity this crate does not give a narrower name.
    Other,
}

/// What a Civil 3D dictionary object is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CivilObjectKind {
    /// Object style (alignment style, surface style, profile view style, ...).
    Style,
    /// Label, text component, or table style.
    LabelStyle,
    /// Display representation of an entity class.
    DisplayRepresentation,
    /// Settings tree node.
    Settings,
    /// Fixed or floating profile geometry element owned by a profile.
    ProfileElement,
    Other,
}

/// DXF names of the Civil 3D entities with the C++ class registered for each.
pub const ENTITY_CLASSES: &[(&str, &str, CivilKind)] = &[
    ("AECC_ALIGNMENT", "AeccDbAlignment", CivilKind::Alignment),
    ("AECC_PROFILE", "AeccDbVAlignment", CivilKind::Profile),
    ("AECC_PROFILE_VIEW", "AeccDbGraphProfile", CivilKind::ProfileView),
    ("AECC_TIN_SURFACE", "AeccDbSurfaceTin", CivilKind::TinSurface),
    ("AECC_CORRIDOR", "AeccDbCorridor", CivilKind::Corridor),
    ("AECC_ASSEMBLY", "AeccDbAssembly", CivilKind::Assembly),
    ("AECC_SUBASSEMBLY", "AeccDbSubassembly", CivilKind::Subassembly),
    ("AECC_PARCEL", "AeccDbFace", CivilKind::Parcel),
    (
        "AECC_SAMPLE_LINE_GROUP",
        "AeccDbSampleLineGroup",
        CivilKind::SampleLineGroup,
    ),
    (
        "AECC_ALIGNMENT_STATION_LABEL_GROUP",
        "AeccDbAlignmentStationLabeling",
        CivilKind::Label,
    ),
    (
        "AECC_ALIGNMENT_MINOR_STATION_LABEL_GROUP",
        "AeccDbAlignmentMinorStationLabeling",
        CivilKind::Label,
    ),
    (
        "AECC_ALIGNMENT_GEOMPOINT_LABEL_GROUP",
        "AeccDbAlignmentGeomPointLabeling",
        CivilKind::Label,
    ),
    (
        "AECC_PROFILEDATABAND_LABELGROUP",
        "AeccDbProfileDataBandLabeling",
        CivilKind::Label,
    ),
    (
        "AECC_VALIGNMENT_LINE_LABEL_GROUP",
        "AeccDbVAlignmentLineLabeling",
        CivilKind::Label,
    ),
    (
        "AECC_VALIGNMENT_PVI_LABEL_GROUP",
        "AeccDbVAlignmentPVILabeling",
        CivilKind::Label,
    ),
    (
        "AECC_VALIGNMENT_SAGCURVE_LABEL_GROUP",
        "AeccDbVAlignmentSagCurveLabeling",
        CivilKind::Label,
    ),
    (
        "AECC_VALIGNMENT_CRESTCURVE_LABEL_GROUP",
        "AeccDbVAlignmentCrestCurveLabeling",
        CivilKind::Label,
    ),
    (
        "AECC_SAMPLELINE_LABELING",
        "AeccDbSampleLineLabeling",
        CivilKind::Label,
    ),
    (
        "AECC_STATION_ELEVATION_LABEL",
        "AeccDbStaElevLabel",
        CivilKind::Label,
    ),
];

/// Whether `dxf_name` is a Civil 3D class.
pub fn is_civil_name(dxf_name: &str) -> bool {
    dxf_name
        .trim()
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("AECC_"))
}

/// Entity kind for a Civil 3D DXF name. Unlisted `AECC_*` entities are
/// [`CivilKind::Other`]; non-Civil names return `None`.
pub fn kind_of(dxf_name: &str) -> Option<CivilKind> {
    if !is_civil_name(dxf_name) {
        return None;
    }
    let upper = dxf_name.trim().to_ascii_uppercase();
    Some(
        ENTITY_CLASSES
            .iter()
            .find(|(name, _, _)| *name == upper)
            .map_or(CivilKind::Other, |(_, _, kind)| *kind),
    )
}

/// C++ class name registered for a known Civil 3D entity.
pub fn cpp_class_name(dxf_name: &str) -> Option<&'static str> {
    let upper = dxf_name.trim().to_ascii_uppercase();
    ENTITY_CLASSES
        .iter()
        .find(|(name, _, _)| *name == upper)
        .map(|(_, cpp, _)| *cpp)
}

/// Object kind for a Civil 3D dictionary-object DXF name; `None` for
/// non-Civil names.
pub fn object_kind_of(dxf_name: &str) -> Option<CivilObjectKind> {
    if !is_civil_name(dxf_name) {
        return None;
    }
    let name = dxf_name.trim().to_ascii_uppercase();
    let kind = if name.starts_with("AECC_DISP_REP_") {
        CivilObjectKind::DisplayRepresentation
    } else if name.starts_with("AECC_DB_FIXED_") || name.starts_with("AECC_DB_FLOAT_") {
        CivilObjectKind::ProfileElement
    } else if name.ends_with("SETTINGS_NODE") {
        CivilObjectKind::Settings
    } else if name.starts_with("AECC_LABEL_") || name.ends_with("_TABLE_STYLE") {
        CivilObjectKind::LabelStyle
    } else if name.contains("_STYLE") {
        CivilObjectKind::Style
    } else {
        CivilObjectKind::Other
    };
    Some(kind)
}

impl ArchEntityData {
    /// Civil 3D kind of this entity, `None` for Arch entities.
    pub fn civil_kind(&self) -> Option<CivilKind> {
        kind_of(&self.dxf_name)
    }

    /// Triangulation stored by a Civil 3D TIN surface.
    ///
    /// `None` for other classes or when the body has no readable
    /// triangulation.
    pub fn tin_surface(&self) -> Option<TinSurface> {
        if self.civil_kind() != Some(CivilKind::TinSurface) {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        tin::find(data, start, end)
    }

    /// Design elements of a Civil 3D alignment.
    pub fn alignment_elements(&self) -> Option<Vec<AlignmentElement>> {
        if self.civil_kind() != Some(CivilKind::Alignment) {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        Some(alignment::elements(data, start, end))
    }

    /// Solved centerline of a Civil 3D alignment, one polyline per run of
    /// elements this crate can solve.
    pub fn alignment_polylines(&self) -> Vec<Vec<Vector2>> {
        self.alignment_elements()
            .map(|elements| alignment::polylines(&elements))
            .unwrap_or_default()
    }

    /// Geometry points (TS, SC, PC, ...) along the first solved run of a
    /// Civil 3D alignment.
    pub fn alignment_geometry_points(&self) -> Vec<GeometryPoint> {
        self.alignment_elements()
            .map(|elements| alignment::geometry_points(&elements))
            .unwrap_or_default()
    }

    /// Station at the start of a Civil 3D alignment.
    pub fn alignment_start_station(&self) -> Option<f64> {
        if self.civil_kind() != Some(CivilKind::Alignment) {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        Some(alignment::start_station(data, start, end))
    }

    /// Name template a Civil 3D sample line group gives its sample lines.
    pub fn sample_line_name_template(&self) -> Option<Vec<LabelPart>> {
        if self.civil_kind() != Some(CivilKind::SampleLineGroup) {
            return None;
        }
        self.decode()
            .strings
            .iter()
            .find(|s| s.contains("<["))
            .map(|s| parse_format(s))
    }

    /// Drawing position a single Civil 3D label is anchored at.
    pub fn label_anchor(&self) -> Option<Vector2> {
        if self.civil_kind() != Some(CivilKind::Label) {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        labels::anchor(data, start, end)
    }

    /// Stations a Civil 3D label group places labels at, in order.
    pub fn label_stations(&self) -> Vec<f64> {
        if self.civil_kind() != Some(CivilKind::Label) {
            return Vec::new();
        }
        self.main_stream()
            .map(|(data, start, end)| labels::stations(data, start, end))
            .unwrap_or_default()
    }

    /// Sample lines a Civil 3D sample line group generates.
    pub fn sample_lines(&self) -> Option<SampleLines> {
        if self.civil_kind() != Some(CivilKind::SampleLineGroup) {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        sample_lines::sample_lines(data, start, end)
    }

    /// Station/elevation polylines of a Civil 3D profile.
    pub fn profile_polylines(&self) -> Vec<Vec<Vector2>> {
        if self.civil_kind() != Some(CivilKind::Profile) {
            return Vec::new();
        }
        self.main_stream()
            .map(|(data, start, end)| profile::polylines(data, start, end))
            .unwrap_or_default()
    }

    /// PVIs of a Civil 3D design profile.
    pub fn profile_pvis(&self) -> Vec<Pvi> {
        if self.civil_kind() != Some(CivilKind::Profile) {
            return Vec::new();
        }
        self.main_stream()
            .map(|(data, start, end)| profile::pvis(data, start, end))
            .unwrap_or_default()
    }

    /// Placement and ranges of a Civil 3D profile view.
    pub fn profile_view_frame(&self) -> Option<ProfileViewFrame> {
        if self.civil_kind() != Some(CivilKind::ProfileView) {
            return None;
        }
        let frame = self.profiles.first()?;
        let min = Vector2::new(
            frame.iter().map(|p| p.x).fold(f64::INFINITY, f64::min),
            frame.iter().map(|p| p.y).fold(f64::INFINITY, f64::min),
        );
        let max = Vector2::new(
            frame.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max),
            frame.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max),
        );
        let (data, start, end) = self.main_stream()?;
        profile::view_frame(data, start, end, min, max)
    }

    /// Baseline and feature lines of a Civil 3D corridor as (station, world
    /// point) runs.
    pub fn corridor_feature_lines(&self) -> Vec<Vec<(f64, crate::types::Vector3)>> {
        if self.civil_kind() != Some(CivilKind::Corridor) {
            return Vec::new();
        }
        self.main_stream()
            .map(|(data, start, end)| corridor::feature_lines(data, start, end))
            .unwrap_or_default()
    }

    /// Cross-section shape a Civil 3D subassembly last generated, in its
    /// own (offset, elevation) frame.
    pub fn subassembly_shape(&self) -> Option<SubassemblyShape> {
        if self.civil_kind() != Some(CivilKind::Subassembly) {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        subassembly::find(data, start, end)
    }

    /// Handle of the assembly a Civil 3D subassembly belongs to.
    pub fn subassembly_assembly(&self) -> Option<u64> {
        if self.civil_kind() != Some(CivilKind::Subassembly) {
            return None;
        }
        // Body handles: two nulls, then the owning assembly.
        self.decode().handles.get(2).copied().filter(|h| *h != 0)
    }

    /// Baseline point of a Civil 3D assembly and where each of its
    /// subassemblies attaches.
    pub fn assembly_layout(&self) -> Option<AssemblyLayout> {
        if self.civil_kind() != Some(CivilKind::Assembly) {
            return None;
        }
        let mut min = Vector2::new(f64::INFINITY, f64::INFINITY);
        let mut max = Vector2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
        for p in self.profiles.iter().flatten() {
            min = Vector2::new(min.x.min(p.x), min.y.min(p.y));
            max = Vector2::new(max.x.max(p.x), max.y.max(p.y));
        }
        let handles = self.decode().handles;
        let (data, start, end) = self.main_stream()?;
        subassembly::assembly_layout(data, start, end, &handles, (min, max))
    }

    /// Raw record and the bit range of its class body.
    fn main_stream(&self) -> Option<(&[u8], usize, usize)> {
        let reader = super::arch::decode::open_entity(self).ok()?;
        let start = usize::try_from(reader.position_in_bits()).ok()?;
        let len = usize::try_from(reader.main_remaining_bits()).ok()?;
        Some((self.raw_dwg_data.as_deref()?, start, start + len))
    }
}

impl ArchObject {
    /// Civil 3D kind of this dictionary object, `None` for Arch objects.
    pub fn civil_kind(&self) -> Option<CivilObjectKind> {
        object_kind_of(&self.dxf_name)
    }

    /// Content, height and offset of a label text component
    /// (`AECC_LABEL_TEXT_STYLE`).
    pub fn label_text(&self) -> Option<LabelText> {
        if self.dxf_name != "AECC_LABEL_TEXT_STYLE" {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        labels::text_component(data, start, end)
    }

    /// UTF-16 strings the record stores inline (label formats and the
    /// like), in record order.
    pub fn inline_strings(&self) -> Vec<String> {
        self.main_stream()
            .map(|(data, start, end)| labels::inline_strings(data, start, end))
            .unwrap_or_default()
    }

    /// Raw record and the bit range of its class body.
    fn main_stream(&self) -> Option<(&[u8], usize, usize)> {
        let objects = crate::io::dwg::dwg_stream_readers::object_reader::DwgObjectReader::new(
            Vec::new(),
            self.raw_dwg_version.unwrap_or(crate::types::DxfVersion::AC1032),
            Default::default(),
        )
        .ok()?;
        let raw = self.raw_dwg_data.as_deref()?;
        let (type_code, mut reader) = objects.read_raw_record(raw, self.raw_dwg_handle_bits).ok()?;
        reader.set_ref_handle(self.handle.value());
        objects.read_common_non_entity_data(&mut reader, type_code);
        let start = usize::try_from(reader.position_in_bits()).ok()?;
        let len = usize::try_from(reader.main_remaining_bits()).ok()?;
        Some((raw, start, start + len))
    }
}
