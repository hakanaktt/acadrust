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
mod cogo;
mod corridor;
mod labels;
mod network;
mod profile;
mod sample_lines;
mod style;
mod subassembly;
mod tin;
mod topology;

pub use alignment::{
    display_station, AlignmentCriteria, AlignmentElement, AlignmentType, OffsetParameters, OffsetRegion, WideningTransition, AlignmentElementKind, AlignmentPiece, DesignSpeed, GeometryPoint, SpiralShape,
    StationEquation,
};
pub use cogo::CogoPoint;
pub use labels::{parse_format, property, LabelPart, LabelText};
pub use network::PartField;
pub use profile::{CurveMinimumK, ProfileCriteria, ProfileLocking, ProfilePiece, ProfileViewFrame, Pvi};
pub use sample_lines::SampleLines;
pub use style::{DisplayComponent, Marker, ViewAxes, ViewAxis};
pub use subassembly::{AssemblyLayout, Attachment, SubassemblyShape};
pub use topology::{Topology, TopologyVertex};
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
    ("AECC_PROFILE_PROJECTION_LABEL", "AeccDbProfileProjectionLabel", CivilKind::Label),
    ("AECC_HORIZONTALGEOMETRYBAND_LABELGROUP", "AeccDbHorizontalGeometryBandLabeling", CivilKind::Label),
    ("AECC_VERTICALGEOMETRYBAND_LABELGROUP", "AeccDbVerticalGeometryBandLabeling", CivilKind::Label),
    ("AECC_SUPERELEVATIONBAND_LABELGROUP", "AeccDbSuperElevationBandLabeling", CivilKind::Label),
    ("AECC_ALIGNMENT_STAEQU_LABEL_GROUP", "AeccDbAlignmentStaEquLabeling", CivilKind::Label),
    ("AECC_ALIGNMENT_DESIGNSPEED_LABEL_GROUP", "AeccDbAlignmentDesignSpeedLabeling", CivilKind::Label),
    ("AECC_ALIGNMENT_SUPERELEVATION_LABEL_GROUP", "AeccDbAlignmentSuperelevationLabeling", CivilKind::Label),
    ("AECC_ALIGNMENT_VAGEOMPOINT_LABELING", "AeccDbAlignmentVAGeomPointLabeling", CivilKind::Label),
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
        let mut elements = alignment::elements(data, start, end);
        // A record of a kind this crate does not know that no known element
        // links to is a stray header read as a record.
        let known_links: Vec<i16> = elements
            .iter()
            .filter(|e| !matches!(e.kind, AlignmentElementKind::Other { .. }))
            .flat_map(|e| [e.previous, e.next])
            .collect();
        elements.retain(|e| !matches!(e.kind, AlignmentElementKind::Other { .. }) || known_links.contains(&e.id));
        Some(elements)
    }

    /// Solved centerline of a Civil 3D alignment, one polyline per run of
    /// elements this crate can solve.
    pub fn alignment_polylines(&self) -> Vec<Vec<Vector2>> {
        self.alignment_elements()
            .map(|elements| alignment::polylines(&elements))
            .unwrap_or_default()
    }

    /// Solved lines, curves and spirals of a Civil 3D alignment, each with
    /// its points.
    pub fn alignment_pieces(&self) -> Vec<(AlignmentPiece, Vec<Vector2>)> {
        self.alignment_elements()
            .map(|elements| alignment::pieces_by_kind(&elements))
            .unwrap_or_default()
    }

    /// Solved pieces of a Civil 3D alignment with the signed curvature each
    /// curve was solved with (0 for lines and spirals).
    pub fn alignment_pieces_with_curvature(&self) -> Vec<(AlignmentPiece, f64, Vec<Vector2>)> {
        self.alignment_elements()
            .map(|elements| alignment::pieces_with_curvature(&elements))
            .unwrap_or_default()
    }

    /// Station ranges a Civil 3D alignment hides (its masking).
    pub fn alignment_masked_ranges(&self) -> Vec<(f64, f64)> {
        let Some(first) = self.alignment_start_station() else {
            return Vec::new();
        };
        let length: f64 = self
            .alignment_polylines()
            .first()
            .map(|run| run.windows(2).map(|w| w[0].distance(&w[1])).sum())
            .unwrap_or(0.0);
        self.main_stream()
            .map(|(data, start, end)| alignment::masked_ranges(data, start, end, first, first + length))
            .unwrap_or_default()
    }

    /// Station equations of a Civil 3D alignment, in station order.
    pub fn alignment_station_equations(&self) -> Vec<StationEquation> {
        self.alignment_equations_and_speeds().0
    }

    /// Design speeds of a Civil 3D alignment, in station order.
    pub fn alignment_design_speeds(&self) -> Vec<DesignSpeed> {
        self.alignment_equations_and_speeds().1
    }

    fn alignment_equations_and_speeds(&self) -> (Vec<StationEquation>, Vec<DesignSpeed>) {
        let Some((first, last)) = self.alignment_station_range() else {
            return Default::default();
        };
        self.main_stream()
            .map(|(data, start, end)| alignment::equations_and_speeds(data, start, end, first, last))
            .unwrap_or_default()
    }

    /// First and last station of the first solved run.
    fn alignment_station_range(&self) -> Option<(f64, f64)> {
        let first = self.alignment_start_station()?;
        let length: f64 = self
            .alignment_polylines()
            .first()
            .map(|run| run.windows(2).map(|w| w[0].distance(&w[1])).sum())
            .unwrap_or(0.0);
        Some((first, first + length))
    }

    /// Centerline, offset, curb return or miscellaneous, and the bit after
    /// the type in the main stream.
    fn alignment_type_at(&self) -> Option<(AlignmentType, usize)> {
        let (first, last) = self.alignment_station_range()?;
        let (data, start, end) = self.main_stream()?;
        alignment::alignment_type(data, start, end, first, last)
    }

    /// Design Criteria and Constraint Editing settings of an alignment.
    pub fn alignment_criteria(&self) -> Option<AlignmentCriteria> {
        if self.civil_kind() != Some(CivilKind::Alignment) {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        alignment::criteria(data, start, end)
    }

    /// The design criteria file and the tables it names, as the Design
    /// Criteria tab shows them: file, minimum radius table, transition
    /// length table, attainment method. `None` without a criteria file.
    pub fn alignment_criteria_file(&self) -> Option<[String; 4]> {
        if self.civil_kind() != Some(CivilKind::Alignment) {
            return None;
        }
        let strings = self.decode().strings;
        // The file's path, then the same path under `<DATADIR>`, then the
        // three tables.
        let at = strings.iter().position(|s| s.contains("<DATADIR>"))?;
        let get = |i: usize| strings.get(i).cloned().unwrap_or_default();
        Some([get(at.checked_sub(1)?), get(at + 1), get(at + 2), get(at + 3)])
    }

    /// The design check set (`AECC_ALIGNMENT_DESIGN_CHECK_SET`) an
    /// alignment points to. Civil 3D sets it on save whether or not the
    /// check set is used. It is the last reference, before an offset's
    /// parent and a curb return's six input references.
    pub fn alignment_check_set(&self) -> Option<u64> {
        let after = match self.alignment_type()? {
            AlignmentType::Offset => 1,
            AlignmentType::CurbReturn => 6,
            _ => 0,
        };
        let handles = self.decode().handles;
        handles.len().checked_sub(after + 1).map(|i| handles[i]).filter(|h| *h != 0)
    }

    /// The parent alignment of an offset alignment (its last reference).
    pub fn alignment_offset_parent(&self) -> Option<u64> {
        (self.alignment_type()? == AlignmentType::Offset)
            .then(|| self.decode().handles.last().copied())
            .flatten()
            .filter(|h| *h != 0)
    }

    /// The two alignments a curb return connects: its last six references
    /// name them as A, A, B, B, A, B.
    pub fn alignment_connected_inputs(&self) -> Option<[u64; 2]> {
        if self.alignment_type()? != AlignmentType::CurbReturn {
            return None;
        }
        let handles = self.decode().handles;
        let tail = handles.get(handles.len().checked_sub(6)?..)?;
        (tail[0] == tail[1] && tail[2] == tail[3] && tail[4] == tail[0] && tail[5] == tail[2]).then(|| [tail[0], tail[2]])
    }

    /// The JSON string a connected alignment keeps its curve settings in
    /// (`CurveType`, `CurveParameters`), and the layout string of an
    /// alignment whose spiral groups are kept as separate elements.
    pub fn alignment_layout_json(&self) -> Option<String> {
        if self.civil_kind() != Some(CivilKind::Alignment) {
            return None;
        }
        self.decode().strings.into_iter().find(|s| s.trim_start().starts_with('{'))
    }

    /// What kind of alignment this is (Alignment Properties' type).
    pub fn alignment_type(&self) -> Option<AlignmentType> {
        self.alignment_type_at().map(|(t, _)| t)
    }

    /// Offset, side, locking, regions, widenings and parent anchors of an
    /// offset alignment.
    pub fn alignment_offset_parameters(&self) -> Option<OffsetParameters> {
        let (AlignmentType::Offset, pos) = self.alignment_type_at()? else {
            return None;
        };
        let (data, _, end) = self.main_stream()?;
        alignment::offset_parameters(data, pos, end)
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
        // Groups whose records vary in size (geometry points between
        // intervals) are only read through their record marker.
        self.main_stream()
            .map(|(data, start, end)| {
                if matches!(self.dxf_name.as_str(), "AECC_ALIGNMENT_STATION_LABEL_GROUP" | "AECC_ALIGNMENT_MINOR_STATION_LABEL_GROUP") {
                    if let Some(stations) = labels::group_stations(data, start, end) {
                        return stations;
                    }
                }
                // debt: other label group classes are searched for their
                // stations.
                let marked = labels::marked_stations(data, start, end);
                if marked.len() >= 2 { marked } else { labels::stations(data, start, end) }
            })
            .unwrap_or_default()
    }

    /// Geometry point kinds a geometry point label group
    /// (`AECC_ALIGNMENT_GEOMPOINT_LABEL_GROUP`) labels: `1` the alignment's
    /// beginning, `2` its end, `3` and up the geometry points between.
    pub fn label_geometry_point_kinds(&self) -> Vec<u8> {
        if self.dxf_name != "AECC_ALIGNMENT_GEOMPOINT_LABEL_GROUP" {
            return Vec::new();
        }
        self.main_stream()
            .map(|(data, start, end)| {
                // debt: groups whose labels do not read are searched.
                labels::group_point_kinds(data, start, end).unwrap_or_else(|| labels::geometry_point_kinds(data, start, end))
            })
            .unwrap_or_default()
    }

    /// Offset from the labelled point to where a dragged label sits, `None`
    /// for a label left in place.
    pub fn label_dragged_offset(&self) -> Option<Vector2> {
        if self.civil_kind() != Some(CivilKind::Label) {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        labels::dragged_offset(data, start, end)
    }

    /// Ids of the profile entities a profile label group
    /// (`AECC_VALIGNMENT_*_LABEL_GROUP`) labels; see
    /// [`Self::profile_entity_ids`].
    pub fn label_entity_ids(&self) -> Vec<i32> {
        if !self.dxf_name.starts_with("AECC_VALIGNMENT_") {
            return Vec::new();
        }
        self.main_stream()
            .map(|(data, start, end)| labels::labelled_entity_ids(data, start, end))
            .unwrap_or_default()
    }

    /// Every entity id a profile label group's records name, with its bit
    /// offset in the record; a PVI label names the tangents on both sides
    /// of its PVI.
    pub fn label_entity_refs(&self) -> Vec<(usize, i32)> {
        if !self.dxf_name.starts_with("AECC_VALIGNMENT_") {
            return Vec::new();
        }
        self.main_stream()
            .map(|(data, start, end)| labels::entity_refs(data, start, end))
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

    /// The profile in 3D over `alignment` (the alignment it belongs to),
    /// where both exist: a point at every profile vertex, every centerline
    /// vertex and the ends of the overlap, placed on the centerline at its
    /// station with the profile's elevation. Station equations are not
    /// applied.
    pub fn profile_over(&self, alignment: &ArchEntityData) -> Vec<Vec<crate::types::Vector3>> {
        let Some(first) = alignment.alignment_start_station() else {
            return Vec::new();
        };
        let Some(run) = alignment.alignment_polylines().into_iter().next() else {
            return Vec::new();
        };
        let mut along = vec![first];
        for w in run.windows(2) {
            along.push(along.last().unwrap() + w[0].distance(&w[1]));
        }
        // Linear interpolation of `values` over ascending `keys` at `k`.
        fn lerp<T: Copy + std::ops::Add<Output = T> + std::ops::Sub<Output = T> + std::ops::Mul<f64, Output = T>>(keys: &[f64], values: &[T], k: f64) -> T {
            let i = keys.partition_point(|s| *s <= k).clamp(1, keys.len() - 1);
            let (k0, k1) = (keys[i - 1], keys[i]);
            let t = if k1 > k0 { ((k - k0) / (k1 - k0)).clamp(0.0, 1.0) } else { 0.0 };
            values[i - 1] + (values[i] - values[i - 1]) * t
        }
        if run.len() < 2 {
            return Vec::new();
        }
        self.profile_polylines()
            .into_iter()
            .filter(|line| line.len() > 1 && line.windows(2).all(|w| w[0].x <= w[1].x))
            .filter_map(|line| {
                let stations: Vec<f64> = line.iter().map(|p| p.x).collect();
                let elevations: Vec<f64> = line.iter().map(|p| p.y).collect();
                let lo = stations[0].max(along[0]);
                let hi = stations[stations.len() - 1].min(*along.last().unwrap());
                if hi <= lo {
                    return None;
                }
                let mut at: Vec<f64> = stations.iter().chain(&along).copied().filter(|s| (lo..=hi).contains(s)).collect();
                at.extend([lo, hi]);
                at.sort_by(f64::total_cmp);
                at.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
                Some(
                    at.into_iter()
                        .map(|s| {
                            let xy = lerp(&along, &run, s);
                            crate::types::Vector3::new(xy.x, xy.y, lerp(&stations, &elevations, s))
                        })
                        .collect(),
                )
            })
            .collect()
    }

    /// Tangents and vertical curves of a Civil 3D profile in
    /// station/elevation.
    pub fn profile_pieces(&self) -> Vec<(ProfilePiece, Vec<Vector2>)> {
        if self.civil_kind() != Some(CivilKind::Profile) {
            return Vec::new();
        }
        self.main_stream()
            .map(|(data, start, end)| profile::pieces(data, start, end))
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

    /// Ids of a design profile's entities in link order, each marked `true`
    /// for a curve, with a fixed tangent's end station; profile label
    /// groups name the entities they label by these ids.
    pub fn profile_entity_ids(&self) -> Vec<(i32, bool, Option<f64>)> {
        if self.civil_kind() != Some(CivilKind::Profile) {
            return Vec::new();
        }
        self.main_stream()
            .map(|(data, start, end)| profile::entity_ids(data, start, end))
            .unwrap_or_default()
    }

    /// Design Criteria settings of a design profile.
    pub fn profile_criteria(&self) -> Option<ProfileCriteria> {
        if self.civil_kind() != Some(CivilKind::Profile) {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        profile::criteria(data, start, end)
    }

    /// Minimum K values design criteria give each parabolic vertical curve
    /// of a design profile.
    pub fn profile_curve_minimum_k(&self) -> Vec<CurveMinimumK> {
        if self.civil_kind() != Some(CivilKind::Profile) {
            return Vec::new();
        }
        self.main_stream()
            .map(|(data, start, end)| profile::curve_minimum_k(data, start, end))
            .unwrap_or_default()
    }

    /// The alignment's name: its fourth string, or for a data shortcut (the
    /// fourth string the source drawing) the name after `.dwg:` in the
    /// reference.
    pub fn alignment_name(&self) -> Option<String> {
        if self.civil_kind() != Some(CivilKind::Alignment) {
            return None;
        }
        let strings = self.decode().strings;
        if let Some(reference) = strings.iter().find_map(|s| s.rsplit_once(".dwg:")) {
            return Some(reference.1.to_string());
        }
        strings.get(3).filter(|s| !s.is_empty()).cloned()
    }

    /// The profile's name: the string after its label point names, which
    /// end with `PVI`.
    pub fn profile_name(&self) -> Option<String> {
        if self.civil_kind() != Some(CivilKind::Profile) {
            return None;
        }
        let strings = self.decode().strings;
        let at = strings.iter().position(|s| s == "PVI")?;
        strings.get(at + 1).cloned()
    }

    /// The design criteria file and its Minimum K Table, when a profile uses
    /// one.
    pub fn profile_criteria_file(&self) -> Option<[String; 2]> {
        if self.civil_kind() != Some(CivilKind::Profile) {
            return None;
        }
        let strings = self.decode().strings;
        let at = strings.iter().position(|s| s.contains("<DATADIR>"))?;
        Some([strings.get(at.checked_sub(1)?)?.clone(), strings.get(at + 1)?.clone()])
    }

    /// Placement and ranges of a Civil 3D profile view.
    pub fn profile_view_frame(&self) -> Option<ProfileViewFrame> {
        if self.civil_kind() != Some(CivilKind::ProfileView) {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        profile::view_frame_stored(data, start, end)
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
        let strings = self.decode().strings;
        let (data, start, end) = self.main_stream()?;
        subassembly::find(data, start, end, &strings)
    }

    /// Handle of the assembly a Civil 3D subassembly belongs to.
    pub fn subassembly_assembly(&self) -> Option<u64> {
        if self.civil_kind() != Some(CivilKind::Subassembly) {
            return None;
        }
        // Body handles: two nulls (one in Civil 3D 2013), then the owning
        // assembly.
        self.decode().handles.iter().take(3).skip(1).copied().find(|h| *h != 0)
    }

    /// Site container (`AECC_FEATURE_STYLE`) of a Civil 3D feature line
    /// (`AECC_FEATURE_LINE`). The record keeps no geometry: its points are
    /// an edge of the container's [`Topology`], see
    /// [`ArchObject::site_members`].
    pub fn feature_line_container(&self) -> Option<u64> {
        if self.dxf_name != "AECC_FEATURE_LINE" {
            return None;
        }
        // Body handles: a null, then the site container twice.
        self.decode().handles.iter().copied().find(|h| *h != 0)
    }

    /// Name and description of a feature line.
    pub fn feature_line_name(&self) -> Option<(String, String)> {
        if self.dxf_name != "AECC_FEATURE_LINE" {
            return None;
        }
        // The class name, the description twice, then the name; without a
        // description only the name.
        let strings = self.decode().strings;
        let texts: Vec<&String> = strings.iter().skip(1).filter(|s| !s.is_empty()).collect();
        Some(match texts.as_slice() {
            [description, _, name, ..] => ((*name).clone(), (*description).clone()),
            [name] => ((*name).clone(), String::new()),
            _ => (String::new(), String::new()),
        })
    }

    /// Name and description of a site or parcel (`AECC_PARCEL`).
    pub fn face_name(&self) -> Option<(String, String)> {
        if self.dxf_name != "AECC_PARCEL" {
            return None;
        }
        let strings = self.decode().strings;
        (strings.first().map(String::as_str) == Some("AeccImpFace"))
            .then(|| (strings.get(2).cloned().unwrap_or_default(), strings.get(3).cloned().unwrap_or_default()))
    }

    /// The site a parcel belongs to (its second reference); `None` for a
    /// site itself.
    pub fn parcel_site(&self) -> Option<u64> {
        if self.dxf_name != "AECC_PARCEL" {
            return None;
        }
        let handles = self.decode().handles;
        (handles.first() == Some(&0)).then(|| handles.get(1).copied()).flatten().filter(|h| *h != 0)
    }

    /// The `AECC_FEATURE_STYLE` record an alignment's geometry points are
    /// kept in: its site's container (owned by the site) or, for a siteless
    /// alignment, its companion (owned by the alignment registry). The
    /// alignment references it twice.
    pub fn alignment_container(&self) -> Option<u64> {
        if self.civil_kind() != Some(CivilKind::Alignment) {
            return None;
        }
        let handles = self.decode().handles;
        let (a, b) = (*handles.get(1)?, *handles.get(2)?);
        (a != 0 && a == b).then_some(a)
    }

    /// Number of the point a COGO point entity (`AECC_COGO_POINT`) shows;
    /// the drawing's point database ([`ArchObject::cogo_points`]) holds it.
    pub fn cogo_point_number(&self) -> Option<u32> {
        if self.dxf_name != "AECC_COGO_POINT" {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        cogo::entity_number(data, start, end)
    }

    /// Start and end of a Civil 3D pipe (`AECC_PIPE`), as stored.
    pub fn pipe_ends(&self) -> Option<(crate::types::Vector3, crate::types::Vector3)> {
        if self.dxf_name != "AECC_PIPE" {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        network::pipe_ends(data, start, end)
    }

    /// Insertion point of a Civil 3D structure (`AECC_STRUCTURE`), found
    /// inside the extents its cached preview records.
    // debt: located through the cache extents; no drawing at hand has a
    // structure to work out the record's field order from.
    pub fn structure_position(&self) -> Option<crate::types::Vector3> {
        if self.dxf_name != "AECC_STRUCTURE" {
            return None;
        }
        let mut min = [f64::INFINITY; 2];
        let mut max = [f64::NEG_INFINITY; 2];
        for p in self.profiles.iter().flatten() {
            min = [min[0].min(p.x), min[1].min(p.y)];
            max = [max[0].max(p.x), max[1].max(p.y)];
        }
        (min[0] <= max[0]).then_some(())?;
        let (data, start, end) = self.main_stream()?;
        network::structure_position(data, start, end, min, max)
    }

    /// Baseline point of a Civil 3D assembly and where each of its
    /// subassemblies attaches.
    pub fn assembly_layout(&self) -> Option<AssemblyLayout> {
        if self.civil_kind() != Some(CivilKind::Assembly) {
            return None;
        }
        let handles = self.decode().handles;
        let (data, start, end) = self.main_stream()?;
        subassembly::assembly_layout(data, start, end, &handles)
    }

    /// Feature lines of a Civil 3D corridor per baseline, each grouped by
    /// point code in the order of the corridor's code list.
    pub fn corridor_baselines(&self) -> Vec<Vec<Vec<Vec<(f64, crate::types::Vector3)>>>> {
        if self.civil_kind() != Some(CivilKind::Corridor) {
            return Vec::new();
        }
        self.main_stream()
            .map(|(data, start, end)| corridor::baselines(data, start, end))
            .unwrap_or_default()
    }

    /// Feature lines of a Civil 3D corridor grouped by point code, in the
    /// order of the corridor's code list (the feature line styles its body
    /// references follow the same order).
    pub fn corridor_feature_line_groups(&self) -> Vec<Vec<Vec<(f64, crate::types::Vector3)>>> {
        if self.civil_kind() != Some(CivilKind::Corridor) {
            return Vec::new();
        }
        self.main_stream()
            .map(|(data, start, end)| corridor::feature_line_groups(data, start, end))
            .unwrap_or_default()
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

    /// Points of the drawing's point database (`AECC_POINT_NODE`).
    pub fn cogo_points(&self) -> Vec<CogoPoint> {
        if self.dxf_name != "AECC_POINT_NODE" {
            return Vec::new();
        }
        let handles = self.decode().handles;
        self.main_stream()
            .map(|(data, start, end)| cogo::points(data, start, end, &handles))
            .unwrap_or_default()
    }

    /// Site topology a site container or alignment companion
    /// (`AECC_FEATURE_STYLE`) keeps: edges through nodes with the faces on
    /// their sides.
    pub fn topology(&self) -> Option<Topology> {
        if !self.dxf_name.eq_ignore_ascii_case("AECC_FEATURE_STYLE") {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        topology::topology(data, start, end)
    }

    /// Alignments and feature lines a site container (`AECC_FEATURE_STYLE`)
    /// lists, in the order of its topology's edges: the references before
    /// the site that owns it.
    pub fn site_members(&self) -> Vec<u64> {
        if !self.dxf_name.eq_ignore_ascii_case("AECC_FEATURE_STYLE") {
            return Vec::new();
        }
        let refs = self.decode().handles;
        let Some(site) = refs.iter().position(|h| *h == self.owner.value()) else {
            return Vec::new();
        };
        refs[..site].iter().copied().filter(|h| *h != 0).collect()
    }

    /// Profile Locking settings kept in a profile's
    /// `AECC_NOTIFICATIONINFOS` object (one of the objects the profile
    /// references).
    pub fn profile_locking(&self) -> Option<ProfileLocking> {
        if !self.dxf_name.eq_ignore_ascii_case("AECC_NOTIFICATIONINFOS") {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        profile::locking(data, start, end)
    }

    /// Content, height and offset of a label text component
    /// (`AECC_LABEL_TEXT_STYLE`).
    pub fn label_text(&self) -> Option<LabelText> {
        if self.dxf_name != "AECC_LABEL_TEXT_STYLE" && self.dxf_name != "AECC_LABEL_TEXT_PROPFIELDOBJECT_STYLE" {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        labels::text_component(data, start, end)
    }

    /// Display components of a Civil 3D object style, in the order the
    /// style class defines them: every component of the plan view, then the
    /// model view and the other view directions.
    ///
    /// `is_layer` tells layer handles apart; only styles with an unmapped
    /// block before their component lists need it.
    pub fn display_components(&self, is_layer: impl Fn(u64) -> bool) -> Vec<DisplayComponent> {
        if self.civil_kind() != Some(CivilObjectKind::Style) {
            return Vec::new();
        }
        let handles = self.decode().handles;
        self.main_stream()
            .map(|(data, start, end)| style::components(data, start, end, &handles, is_layer))
            .unwrap_or_default()
    }

    /// Plotted length and angle (radians, to the anchor's direction) of a
    /// Civil 3D label line component.
    pub fn label_line(&self) -> Option<(f64, f64)> {
        if self.dxf_name != "AECC_LABEL_RADIAL_LINE_STYLE" {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        labels::line_component(data, start, end)
    }

    /// Whether a label style component (text, line, tick, ...) is shown;
    /// `true` when the record does not say.
    pub fn label_component_visible(&self) -> bool {
        if !self.dxf_name.starts_with("AECC_LABEL_") {
            return true;
        }
        self.main_stream()
            .is_none_or(|(data, start, end)| labels::component_visible(data, start, end))
    }

    /// Plotted height of a Civil 3D profile view band.
    pub fn band_height(&self) -> Option<f64> {
        if !(self.dxf_name.starts_with("AECC_PROFILE_VIEW_") && self.dxf_name.ends_with("_BAND_STYLE")) {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        style::setting(data, start, end, style::BAND_HEIGHT).filter(|v| *v > 0.0)
    }

    /// Block and plotted size of a Civil 3D label tick style.
    pub fn label_tick(&self) -> Option<(u64, f64)> {
        if self.dxf_name != "AECC_LABEL_TICK_STYLE" {
            return None;
        }
        // The block is the style's last non-null reference.
        let block = self.decode().handles.iter().rev().copied().find(|h| *h != 0)?;
        let (data, start, end) = self.main_stream()?;
        Some((block, style::tick_size(data, start, end)?))
    }

    /// Axis intervals, tick sizes and text heights of a Civil 3D profile
    /// view style.
    pub fn profile_view_axes(&self) -> Option<ViewAxes> {
        if self.dxf_name != "AECC_PROFILE_VIEW_STYLE" {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        style::view_axes(data, start, end)
    }

    /// Point style of a Civil 3D marker style (`AECC_ROADWAYMARKER_STYLE`).
    pub fn marker(&self) -> Option<Marker> {
        if self.dxf_name != "AECC_ROADWAYMARKER_STYLE" && self.dxf_name != "AECC_POINT_STYLE" {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        style::marker(data, start, end)
    }

    /// Codes of a Civil 3D code set style (`AECC_ROADWAY_STYLE_SET`) with
    /// the style each one draws with: point codes map to marker styles, link
    /// codes to link styles and area codes to shape styles. Each code is
    /// four strings (code, name, description, blank) and five references
    /// (material, style, label style, feature line style, blank); the
    /// leading reference groups without strings are the defaults.
    pub fn code_set(&self) -> Vec<(String, u64)> {
        self.code_set_parts().1
    }

    /// Default styles of a Civil 3D code set style: for uncoded markers,
    /// links and shapes, then for codes the set does not list.
    pub fn code_set_defaults(&self) -> Vec<u64> {
        self.code_set_parts().0
    }

    fn code_set_parts(&self) -> (Vec<u64>, Vec<(String, u64)>) {
        if self.dxf_name != "AECC_ROADWAY_STYLE_SET" {
            return (Vec::new(), Vec::new());
        }
        let decoded = self.decode();
        let strings = &decoded.strings;
        let Some(first) = (4..strings.len().saturating_sub(1))
            .find(|&i| !strings[i].is_empty() && strings[i] == strings[i + 1])
        else {
            return (Vec::new(), Vec::new());
        };
        let codes: Vec<&String> = strings[first..].chunks(4).map(|c| &c[0]).collect();
        let groups: Vec<&[u64]> = decoded.handles.chunks(5).filter(|g| g.len() == 5).collect();
        let Some(defaults) = groups.len().checked_sub(codes.len()) else {
            return (Vec::new(), Vec::new());
        };
        let entries = codes
            .into_iter()
            .zip(&groups[defaults..])
            .map(|(code, group)| (code.clone(), group[1]))
            .collect();
        (groups[..defaults].iter().map(|g| g[1]).collect(), entries)
    }

    /// `(minor, major)` contour intervals of a Civil 3D surface style.
    pub fn surface_contour_intervals(&self) -> Option<(f64, f64)> {
        if self.dxf_name != "AECC_SURFACE_STYLE" {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        style::contour_intervals(data, start, end)
    }

    /// Number fields of a pipe network part definition
    /// (`AECC_NETWORK_PART_DEF`): sizes, thicknesses and the like.
    pub fn network_part_fields(&self) -> Vec<PartField> {
        if self.dxf_name != "AECC_NETWORK_PART_DEF" {
            return Vec::new();
        }
        let strings = self.decode().strings;
        self.main_stream()
            .map(|(data, start, end)| network::part_fields(data, start, end, &strings))
            .unwrap_or_default()
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
