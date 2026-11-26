//! Civil 3D classes.
//!
//! Alignments, profiles, surfaces, corridors, and their labels and styles are
//! registered `AECC_*` classes built on the AEC base classes, so the DWG
//! reader keeps them in the Arch containers ([`ArchEntityData`] and
//! [`ArchObject`]) and same-version saves write the
//! native body back unchanged. This module names each class and reads the
//! geometry the class body stores.

mod alignment;
pub mod create;
pub mod pkt;
mod bits;
mod cogo;
mod corridor;
mod grading;
mod intersection;
mod labels;
mod label_style;
mod network;
mod parameters;
mod profile;
mod sample_lines;
mod section;
mod stormwater;
mod style;
mod subassembly;
mod tin;
mod survey;
mod parcel_segments;
mod topology;
mod view_frames;

pub use alignment::{
    display_station, AlignmentAnchor, AlignmentCriteria, AlignmentElement, AlignmentType, OffsetParameters, OffsetRegion, WideningTransition, AlignmentElementKind, AlignmentPiece, BestFitPoint, DesignSpeed, GeometryPoint, SpiralShape,
    StationEquation, CriticalStationData, SuperelevationCurveData, SuperelevationView};
pub use cogo::{CogoPoint, PointGroupClassification, PointGroupOverrides, PropertyValue, UserProperty};
pub use create::{add_empty_tin_surface, add_tin_surface_with, add_assembly, add_pkt_subassembly, add_pkt_subassembly_on, PktSource, add_alignment, add_alignment_with_curves, add_offset_alignment, add_offset_alignment_with_widenings, Widening, add_alignment_of_kind, add_design_speeds, add_station_equations, add_superelevation, CriticalStation, CriticalStationKind, SuperelevationCurve, AlignmentKind, VertexCurve, add_corridor, add_corridor_with, add_cogo_point, create_cogo_point, point_creation_layer, NO_ELEVATION, add_intersection, add_point_group, add_point_group_query, add_profile, add_profile_entities, add_profile_with_curves, add_static_profile, add_surface_profile, ProfileEntity, VerticalCurve, add_profile_view, add_profile_view_with_bands, add_profile_view_with_range, BandKind, BandStagger, ProfileViewBand, add_sample_line, add_sample_line_through, add_sample_line_group, add_sections, add_section_view, add_site, add_alignment_on_site, add_feature_line, add_parcel, parcel_name, PARCEL_STYLE_NAME, PARCEL_NUMBER, add_feature_line_vertices, FeatureLineVertex, add_tin_surface, cogo_points, ensure_civil, feature_line_vertices, remove_cogo_point, set_cogo_point, set_cogo_point_name, set_point_group, set_point_group_overrides, set_point_group_query, set_point_group_styles, Comparison, PointAttribute, PointQuery, QueryConstant, QueryExpression, add_lot_line, add_pipe, add_pipe_network, add_pipe_of_family, add_structure, add_structure_of_family, connect_pipe, set_network_part_name, network_part_families, add_pressure_appurtenance, add_pressure_arc_pipe, add_pressure_cross, add_pressure_fitting, add_pressure_network, add_pressure_part, add_pressure_pipe, add_pressure_pipe_of_size, add_pressure_tee, connect_pressure_parts, pressure_part_descriptions, add_profile_view_style, add_profile_view_with_style, add_offset_profile, add_section_view_group, add_profile_view_style_with, set_feature_line_vertices, set_cogo_point_label_style, add_label_style, add_point_label_style, label_style, LabelStyle, add_intersection_with, add_intersection_with_curb_returns, IntersectionOffsets, CURB_RETURN_MARGIN, add_curb_return, add_curb_return_with, CurbReturnCurve, add_point_choice_property, add_point_property, add_point_property_classification, point_properties, set_cogo_point_property, add_pipe_of_size, network_pipe_sizes, add_structure_of_size, network_structure_size_fields, StructureSizeChoice, add_intersection_with_curb_return_profiles, CurbReturnProfiles, add_alignment_label_group, AlignmentGroupLabel, AlignmentLabelGroup, AlignmentLabelGroupKind, LabelKeyValue, LabelOverrideProperty, LabelOverrideValue};
pub use corridor::{CorridorBaseline, CorridorBoundary, CorridorDefinition, CorridorRegion, CorridorSurfaceDefinition, CurveStations, RegionFrequency, RegionTarget, TargetCurveStations, TargetKind};
pub use grading::{CriteriaValue, GradingCriteria, GradingEntity, GradingGroup, GradingGroupRecord, GradingGroupTree};
pub use intersection::IntersectionLayout;
pub use labels::{parse_format, property, LabelPart, LabelText};
pub use label_style::{LabelArrowComponent, LabelBlockComponent, LabelBorder, LabelDraggedState, LabelIteratorComponent, LabelLeader, LabelLineComponent, LabelStyleSettings, LabelTextComponent, LabelTickComponent};
pub use network::{PartField, PressurePipe, PressurePort};
pub use parameters::{Parameter, ParameterValue};
pub use profile::{CurveMinimumK, OffsetProfile, ProfileCriteria, ProfileLocking, ProfilePiece, ProfileViewFrame, Pvi};
pub use parcel_segments::ParcelSegmentGeometry;
pub use sample_lines::{SampleLines, SampleLineVertices, SectionViewGroupPlot};
pub use section::{MassHaulViewFrame, SectionGeometry, SectionViewFrame};
pub use style::{DisplayComponent, Marker, ProfileViewAxisStyle, ProfileViewAxisTitle, ProfileViewGrid, ProfileViewGridLines, ProfileViewStyleSettings, ProfileViewTicks, ProfileViewTitle, ViewAxes, ViewAxis};
pub use subassembly::{AssemblyLayout, Attachment, SubassemblyShape};
pub use survey::{SurveyFigure, SurveyFigureGeometry};
pub use view_frames::{MatchLine, MatchLineGeometry, SheetGeometry, ViewFrame, ViewFrameGeometry};
pub use topology::{Topology, TopologyVertex};
pub use tin::{TinBoundaryType, TinBreaklineType, TinDefinition, TinLines, TinOperation, TinSurface, TinTriangle, TinVertex};

use super::arch::{ArchEntityData, ArchObject};
pub(crate) use network::connector_count as network_connector_count;
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
    } else if name.contains("_STYLE") && name != "AECC_FEATURE_STYLE" {
        // (AECC_FEATURE_STYLE is a site container, no display components.)
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

    /// The triangulation a TIN surface's snapshot keeps; `None` without a
    /// snapshot. A snapshot of a surface with no triangulation decodes as a
    /// corner-only [`TinSurface`] (four vertices, no triangles).
    pub fn tin_snapshot(&self) -> Option<TinSurface> {
        if self.civil_kind() != Some(CivilKind::TinSurface) {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        tin::snapshot(data, start, end)
    }

    /// Triangulation stored by a Civil 3D TIN surface.
    ///
    /// `None` for other classes. A surface the application has not built
    /// keeps only its shell (the four bounding corners, no triangles —
    /// `Surface-2.dwg` "EG"); it decodes as a [`TinSurface`] with four
    /// vertices and no triangles, not as `None`.
    pub fn tin_surface(&self) -> Option<TinSurface> {
        if self.civil_kind() != Some(CivilKind::TinSurface) {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        tin::find(data, start, end)
    }

    /// The surface's name: the first non-empty string after the
    /// definitions' texts. The application stores a created-by name before
    /// them (2027 samples: "", "", "leightkadmin", then the definitions'
    /// names, the surface's name and its description — `Surface-2.dwg`'s
    /// empty shell reads "EG"); records this crate writes keep the name
    /// right after the texts, so it is read there as a fallback. A record
    /// whose definitions this grammar does not read names `None` — its
    /// text boundary is unknown.
    pub fn surface_name(&self) -> Option<String> {
        if self.civil_kind() != Some(CivilKind::TinSurface) {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        let texts = match tin::definitions(data, start, end) {
            Some(d) => d.list.iter().map(TinDefinition::text_count).sum::<usize>(),
            None => match tin::definitions_count(data, start, end) {
                None => 0,
                Some(_) => return None,
            },
        };
        let strings = self.decode().strings;
        strings
            .get(3 + texts)
            .filter(|s| !s.is_empty())
            .cloned()
            .or_else(|| strings.get(2 + texts).filter(|s| !s.is_empty()).cloned())
    }

    /// Definitions of a TIN surface in build order, with their
    /// texts (before the surface name) and source objects (the references
    /// after the hard owner), and whether the surface
    /// rebuilds itself when a source changes.
    pub fn tin_definitions(&self) -> Option<(Vec<TinDefinition>, bool)> {
        if self.civil_kind() != Some(CivilKind::TinSurface) {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        let found = tin::definitions(data, start, end)?;
        let strings = self.decode().strings;
        let refs = self.typed_handles();
        let mut text = 2;
        let mut source = refs.iter().position(|r| r.1 == 3)? + 1;
        let mut list = found.list;
        for d in &mut list {
            let n = d.text_count();
            d.texts = strings.get(text..text + n)?.to_vec();
            text += n;
            let n = d.source_count();
            d.sources = refs.get(source..source + n)?.iter().map(|r| r.0).collect();
            source += n;
        }
        Some((list, found.auto_rebuild))
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

    /// The superelevation curves of an alignment whose superelevation block
    /// has the plain form this crate writes (critical stations with their
    /// cross slopes, no curve settings); an empty list without a block,
    /// `None` for another form.
    pub fn alignment_superelevation(&self) -> Option<Vec<SuperelevationCurveData>> {
        let (first, last) = self.alignment_station_range()?;
        let (data, start, end) = self.main_stream()?;
        alignment::superelevation_curves(data, start, end, first, last)
    }

    /// [`Self::alignment_station_equations`] and
    /// [`Self::alignment_design_speeds`] with the identifiers of their
    /// anchors.
    #[allow(clippy::type_complexity)]
    pub(crate) fn alignment_timing_with_ids(&self) -> Option<(Vec<(StationEquation, [u8; 16])>, Vec<(DesignSpeed, [u8; 16])>)> {
        let (first, last) = self.alignment_station_range()?;
        let (data, start, end) = self.main_stream()?;
        alignment::equations_and_speeds_with_ids(data, start, end, first, last)
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
    /// name them as A, A, B, B, A, B; an intersection's curb return names
    /// A, A's offset alignment, B, B's offset alignment and the
    /// intersection last (see [`Self::alignment_intersection_curb_return`]).
    pub fn alignment_connected_inputs(&self) -> Option<[u64; 2]> {
        if self.alignment_type()? != AlignmentType::CurbReturn {
            return None;
        }
        let handles = self.decode().handles;
        let tail = handles.get(handles.len().checked_sub(6)?..)?;
        if tail[0] == tail[1] && tail[2] == tail[3] && tail[4] == tail[0] && tail[5] == tail[2] {
            return Some([tail[0], tail[2]]);
        }
        self.alignment_intersection_curb_return().map(|(roads, _, _)| roads)
    }

    /// An intersection's curb return: its two roads, the offset alignments
    /// it runs along and the intersection (its last five references).
    pub fn alignment_intersection_curb_return(&self) -> Option<([u64; 2], [u64; 2], u64)> {
        if self.alignment_type()? != AlignmentType::CurbReturn {
            return None;
        }
        let handles = self.decode().handles;
        let tail = handles.get(handles.len().checked_sub(5)?..)?;
        (tail[0] != tail[1] && tail[2] != tail[3] && tail[4] != tail[0] && tail[4] != tail[2] && tail.iter().all(|h| *h != 0)).then(|| ([tail[0], tail[2]], [tail[1], tail[3]], tail[4]))
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

    /// The track gauge of a rail alignment (0 for the other types).
    pub fn alignment_track_gauge(&self) -> Option<f64> {
        let (data, start, end) = self.main_stream()?;
        alignment::track_gauge(data, start, end)
    }

    /// What kind of alignment this is (Alignment Properties' type).
    pub fn alignment_type(&self) -> Option<AlignmentType> {
        self.alignment_type_at().map(|(t, _)| t)
    }

    /// The station anchors of an alignment (see [`AlignmentAnchor`]), in
    /// the order stored (by identifier).
    pub fn alignment_anchors(&self) -> Option<Vec<AlignmentAnchor>> {
        let (data, start, end) = self.main_stream()?;
        alignment::anchors(data, start, end)
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
        let projection = match self.dxf_name.as_str() {
            "AECC_STATION_ELEVATION_LABEL" => false,
            "AECC_PROFILE_PROJECTION_LABEL" => true,
            _ => return None,
        };
        let (data, start, end) = self.main_stream()?;
        labels::anchor(data, start, end, projection)
    }

    /// Stations a Civil 3D label group places labels at, in order.
    pub fn label_stations(&self) -> Vec<f64> {
        if self.civil_kind() != Some(CivilKind::Label) {
            return Vec::new();
        }
        self.main_stream()
            .and_then(|(data, start, end)| {
                if matches!(self.dxf_name.as_str(), "AECC_ALIGNMENT_STATION_LABEL_GROUP" | "AECC_ALIGNMENT_MINOR_STATION_LABEL_GROUP") {
                    labels::group_stations(data, start, end)
                } else {
                    labels::stations(data, start, end)
                }
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
            .and_then(|(data, start, end)| labels::group_point_kinds(data, start, end))
            .unwrap_or_default()
    }

    /// Offset from the labelled point to where a dragged label sits, `None`
    /// for a label left in place.
    pub fn label_dragged_offset(&self) -> Option<Vector2> {
        if self.civil_kind() != Some(CivilKind::Label) {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        labels::dragged_offset(data, start, end)?
    }

    /// Ids of the profile entities a profile label group
    /// (`AECC_VALIGNMENT_*_LABEL_GROUP`) labels; see
    /// [`Self::profile_entity_ids`].
    pub fn label_entity_ids(&self) -> Vec<i32> {
        if !self.dxf_name.starts_with("AECC_VALIGNMENT_") {
            return Vec::new();
        }
        self.main_stream()
            .and_then(|(data, start, end)| labels::labelled_entity_ids(data, start, end))
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
            .and_then(|(data, start, end)| labels::entity_refs(data, start, end))
            .unwrap_or_default()
    }

    /// The section view group plots of a sample line group that lists its
    /// lines (see [`SectionViewGroupPlot`]).
    pub fn section_view_group_plots(&self) -> Option<Vec<SectionViewGroupPlot>> {
        if self.civil_kind() != Some(CivilKind::SampleLineGroup) {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        sample_lines::group_plots(data, start, end)
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

    /// An offset profile's source, slope, offset and entity map (`None` for
    /// other profiles).
    pub fn profile_offset(&self) -> Option<OffsetProfile> {
        if self.civil_kind() != Some(CivilKind::Profile) {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        let mut o = profile::offset_profile(data, start, end)?;
        o.source = self.typed_handles().get(15)?.0;
        Some(o)
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
        // The application stores a created-by name before the alignment's
        // own (2027 samples: "AeccImpAlignment", "", "", "Administrator",
        // "Centerline (1)"); records this crate writes keep the name at
        // index 3. Prefer the first non-empty string from index 4, then 3.
        strings
            .get(4)
            .filter(|s| !s.is_empty())
            .cloned()
            .or_else(|| strings.get(3).filter(|s| !s.is_empty()).cloned())
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

    /// Input parameters of a subassembly with their current
    /// values, in the order the record stores them (integers, doubles,
    /// strings).
    pub fn subassembly_parameters(&self) -> Option<Vec<Parameter>> {
        if self.civil_kind() != Some(CivilKind::Subassembly) {
            return None;
        }
        let strings = self.decode().strings;
        let (data, start, end) = self.main_stream()?;
        subassembly::parameters(data, start, end, &strings)
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

    /// The feature line's edge in its site container's [`Topology`] (see
    /// [`Self::feature_line_container`]):
    ///
    /// ```text
    /// BL 80, BL 0, BL 80, BL 80, BL 0, BL 0x10025F04, 6 bits (010010,
    ///   a keyed class head without keys), 128 zero bits, BL 0, BL edge,
    ///   7 bits, BS 0x04BE
    /// ```
    pub fn feature_line_edge(&self) -> Option<i32> {
        if self.dxf_name != "AECC_FEATURE_LINE" {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        let mut bits = bits::Bits::new(data, start, end);
        for _ in 0..6 {
            bits.bit_long()?;
        }
        (bits.bits_value(6)? == 0b010010).then_some(())?;
        bits.skip(128)?;
        (bits.bit_long()? == 0).then_some(())?;
        bits.bit_long()
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

    /// A parcel's number and the face of its site's topology it covers
    /// (see [`Topology::face_polylines`]); `None` for a site.
    ///
    /// ```text
    /// class head with keys 0x0D00800A BL 0, 0x02008022 BD 0.1,
    ///   0x0200800B BD 1/24, BL 0, 121 zero bits
    /// BL number, BL 0, BL 0, BD 0, 3BD label location, BL 80, BL 1,
    /// 3BD label location, BL 2, BL n, n x BL face, BL 0, B 0, BS 0x04BE
    /// ```
    ///
    /// A parcel a feature line split lies over several faces (PD01); this
    /// gives the first, [`Self::parcel_faces`] all.
    pub fn parcel_face(&self) -> Option<(i32, i32)> {
        let (number, faces) = self.parcel_faces()?;
        Some((number, *faces.first()?))
    }

    /// A parcel's number and every face of its site's topology it covers
    /// (see [`Self::parcel_face`]).
    pub fn parcel_faces(&self) -> Option<(i32, Vec<i32>)> {
        self.parcel_site()?;
        let mut b = self.parcel_fields()?;
        let number = b.bit_long()?;
        b.bit_long()?;
        b.bit_long()?;
        for _ in 0..4 {
            b.bit_double()?;
        }
        (matches!(b.bit_long()?, 80 | 112) && b.bit_long()? == 1).then_some(())?;
        for _ in 0..3 {
            b.bit_double()?;
        }
        (b.bit_long()? == 2).then_some(())?;
        let n = b.bit_long()?;
        (1..=64).contains(&n).then_some(())?;
        Some((number, (0..n).map(|_| b.bit_long()).collect::<Option<_>>()?))
    }

    /// Where a parcel's own fields start: past the class header and its
    /// schema-word prefix. The header closes `BL 0x10025F04, 6 bits`; the
    /// word gates the prefix, the parcel fields proper are
    ///
    /// ```text
    /// BL number, BL 0, BL 0, BD 0, 3BD label location, BL 80 or 112,
    /// BL 1, 3BD label location (a copy), BL 2, BL n, n x BL face,
    /// BL 0, B 0, BS 0x04BE
    /// ```
    ///
    /// Prefixes on file (all 54 parcels of Labels-Parcels.dwg, plus the
    /// parcel drawings Parcel-1E/3A):
    ///
    /// * `0b01_0101` (the word this crate writes): 8-bit key count, keys
    ///   (see [`alignment::key`]), then a closing `BL` — 0 in fresh
    ///   records, 16 or 88 in re-saved ones — then 121 bits.
    /// * `0b01_0010` (Civil 3D 2013 parcels, no keys): 128 bits.
    /// * older words (`0..=7`): a raw 32-bit head word whose top byte
    ///   gates the prefix — `0x02`: four raw longs (the 2013 prefix behind
    ///   a head); `0x05`: 8-bit key count, keys, a closing `BL`, then the
    ///   same 121 bits the keyed word carries. A record of an older word
    ///   is admitted only with its whole body validated (label echo,
    ///   faces, tail, exact end); anything else reads `None`.
    fn parcel_fields(&self) -> Option<bits::Bits<'_>> {
        let (data, start, end) = self.main_stream()?;
        let at = parcel_fields_offset(data, start, end)?;
        let mut b = bits::Bits::new(data, start, end);
        b.pos = at;
        Some(b)
    }

    /// Where a parcel's area label sits (the label location after its
    /// number; see [`Self::parcel_face`]).
    pub fn parcel_label(&self) -> Option<Vector2> {
        self.parcel_site()?;
        let mut b = self.parcel_fields()?;
        for _ in 0..3 {
            b.bit_long()?;
        }
        b.bit_double()?;
        Some(Vector2::new(b.bit_double()?, b.bit_double()?))
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

    /// The `AECC_PARCEL_STYLE` a parcel references (its fourth body
    /// handle); `None` for a site, which stores no style.
    pub fn parcel_style(&self) -> Option<u64> {
        if self.dxf_name != "AECC_PARCEL" || self.parcel_site().is_none() {
            return None;
        }
        self.decode().handles.get(3).copied().filter(|h| *h != 0)
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

    /// Points of a Civil 3D section (`AECC_SECTION`, `AECC_SECTION_CORRIDOR`)
    /// in its (offset, elevation) frame, with the point pairs it links.
    pub fn section_geometry(&self) -> Option<SectionGeometry> {
        if !matches!(self.dxf_name.as_str(), "AECC_SECTION" | "AECC_SECTION_CORRIDOR") {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        section::geometry(data, start, end)
    }

    /// Frame of a Civil 3D section view (`AECC_GRAPH_SECTION_VIEW`): where it
    /// draws offsets and elevations.
    pub fn section_view_frame(&self) -> Option<SectionViewFrame> {
        if self.dxf_name != "AECC_GRAPH_SECTION_VIEW" {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        section::view_frame(data, start, end)
    }

    /// Balance curve of a Civil 3D mass haul line (`AECC_MASSHAULLINE`):
    /// (station, cumulative volume) pairs; place them with the frame of the
    /// mass haul view that shows the line.
    pub fn mass_haul_points(&self) -> Vec<crate::types::Vector2> {
        if self.dxf_name != "AECC_MASSHAULLINE" {
            return Vec::new();
        }
        self.main_stream()
            .and_then(|(data, start, end)| section::mass_haul_points(data, start, end))
            .unwrap_or_default()
    }

    /// Frame of a Civil 3D mass haul view (`AECC_MASS_HAUL_VIEW`): where it
    /// draws stations and volumes.
    pub fn mass_haul_view_frame(&self) -> Option<MassHaulViewFrame> {
        if self.dxf_name != "AECC_MASS_HAUL_VIEW" {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        section::mass_haul_view_frame(data, start, end)
    }


    /// Station and vertex sides of a sample line
    /// (`AECC_SAMPLE_LINE`): where it crosses its alignment and, per side
    /// (left first), its vertices as `(offset, station less the line's)`
    /// from the alignment outwards. The line's swath widths are its
    /// extreme offsets.
    pub fn sample_line_vertices(&self) -> Option<SampleLineVertices> {
        if self.dxf_name != "AECC_SAMPLE_LINE" {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        let (station, sides) = sample_lines::line_station_and_sides(data, start, end)?;
        Some(SampleLineVertices { station, left: sides[0].clone(), right: sides[1].clone() })
    }

    /// Insertion point of a Civil 3D pressure fitting or appurtenance
    /// (`AECC_FITTING`, `AECC_APPURTENANCE`), stored like a structure's.
    pub fn pressure_part_position(&self) -> Option<crate::types::Vector3> {
        if !matches!(self.dxf_name.as_str(), "AECC_FITTING" | "AECC_APPURTENANCE") {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        network::structure_at(data, start, end)
    }

    /// Rotation (radians, about Z) of a Civil 3D structure, pressure fitting
    /// or appurtenance (`AECC_STRUCTURE`, `AECC_FITTING`,
    /// `AECC_APPURTENANCE`).
    pub fn network_part_rotation(&self) -> Option<f64> {
        if !matches!(self.dxf_name.as_str(), "AECC_STRUCTURE" | "AECC_FITTING" | "AECC_APPURTENANCE") {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        network::part_rotation(data, start, end)
    }

    /// Ports of a pressure pipe, fitting or appurtenance
    /// (`AECC_PRESSUREPIPE`, `AECC_FITTING`, `AECC_APPURTENANCE`), in record
    /// order (a pipe's start, then its end). Port `k` connects to the part
    /// of reference `8 + k`.
    pub fn pressure_ports(&self) -> Option<Vec<PressurePort>> {
        let pipe = match self.dxf_name.as_str() {
            "AECC_PRESSUREPIPE" => true,
            "AECC_FITTING" | "AECC_APPURTENANCE" => false,
            _ => return None,
        };
        let (data, start, end) = self.main_stream()?;
        network::pressure_ports(data, start, end, pipe)
    }

    /// Ends, bulge and joint sizes of a Civil 3D pressure pipe
    /// (`AECC_PRESSUREPIPE`).
    pub fn pressure_pipe(&self) -> Option<PressurePipe> {
        if self.dxf_name != "AECC_PRESSUREPIPE" {
            return None;
        }
        let handles = self.decode().handles;
        let (data, start, end) = self.main_stream()?;
        network::pressure_pipe(data, start, end, &handles)
    }

    /// Road stations and region angles of a Civil 3D intersection
    /// (`AECC_INTERSECTION`).
    pub fn intersection_layout(&self) -> Option<IntersectionLayout> {
        if self.dxf_name != "AECC_INTERSECTION" {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        intersection::layout(data, start, end)
    }

    /// Start and end of a Civil 3D pipe (`AECC_PIPE`), as stored.
    pub fn pipe_ends(&self) -> Option<(crate::types::Vector3, crate::types::Vector3)> {
        if self.dxf_name != "AECC_PIPE" {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        network::pipe_ends(data, start, end)
    }

    /// The part definition (`AECC_NETWORK_PART_DEF`) a Civil 3D pipe or
    /// structure applies. Sizes the record does not keep itself live
    /// there: a pipe's `PipeInnerDiameter` (inches), a structure's
    /// `StructInnerDiameter`. A pipe's inverts are its ends lowered by
    /// half of its inner diameter in drawing units (inch / 12 per foot).
    pub fn pipe_part_definition(&self) -> Option<u64> {
        if !matches!(self.dxf_name.as_str(), "AECC_PIPE" | "AECC_STRUCTURE") {
            return None;
        }
        network::part_definition(&self.typed_handles())
    }

    /// A survey figure (`AECC_SVFIGURE`): its name and the name of the
    /// survey site it belongs to.
    pub fn survey_figure(&self) -> Option<survey::SurveyFigure> {
        if self.dxf_name != "AECC_SVFIGURE" {
            return None;
        }
        survey::figure(&self.decode().strings)
    }

    /// Geometry of a survey figure (`AECC_SVFIGURE`): the traverse vertices
    /// the record caches (within the shared solved-point tolerance of the
    /// live application's; see [`survey::SurveyFigureGeometry`]) and the
    /// elevation range its bbox solid stores (the live application's
    /// `MinElevation`/`MaxElevation`, bit for bit).
    pub fn survey_figure_geometry(&self) -> Option<survey::SurveyFigureGeometry> {
        if self.dxf_name != "AECC_SVFIGURE" {
            return None;
        }
        let graphic = super::arch::decode::entity_graphic_data(self)?;
        let (data, start, end) = self.main_stream()?;
        survey::geometry(&data, start, end, &graphic)
    }

    /// Name of a survey network (`AECC_SVNETWORK`).
    pub fn survey_network_name(&self) -> Option<String> {
        if self.dxf_name != "AECC_SVNETWORK" {
            return None;
        }
        survey::network_name(&self.decode().strings)
    }

    /// A sheet (`AECC_SHEET`) a plan production run creates: its label and
    /// its frame's view frame group.
    pub fn sheet(&self) -> Option<view_frames::Sheet> {
        if self.dxf_name != "AECC_SHEET" {
            return None;
        }
        let strings = self.decode().strings;
        view_frames::sheet(&strings, &self.typed_handles())
    }

    /// A superelevation diagram view (`AECC_SUPERELEVATION_DIAGRAM_VIEW`)
    /// of an alignment: its name and the alignment it shows.
    pub fn superelevation_view(&self) -> Option<alignment::SuperelevationView> {
        if self.dxf_name != "AECC_SUPERELEVATION_DIAGRAM_VIEW" {
            return None;
        }
        let strings = self.decode().strings;
        alignment::superelevation_view(&strings, &self.typed_handles())
    }

    /// Station of a Civil 3D sample line (`AECC_SAMPLE_LINE`).
    pub fn sample_line_station(&self) -> Option<f64> {
        if self.dxf_name != "AECC_SAMPLE_LINE" {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        sample_lines::sample_line_station(data, start, end)
    }

    /// A view frame (`AECC_VIEWFRAME`) of a plan production run: its name,
    /// its view frame group and the match lines on its sides.
    pub fn view_frame(&self) -> Option<view_frames::ViewFrame> {
        if self.dxf_name != "AECC_VIEWFRAME" {
            return None;
        }
        let strings = self.decode().strings;
        view_frames::view_frame(&strings, &self.typed_handles())
    }

    /// The frame rectangle a view frame (`AECC_VIEWFRAME`) record keeps in
    /// its graphic-image section: the four corners of the frame rotated to
    /// the alignment; see [`view_frames::ViewFrameGeometry`].
    pub fn view_frame_geometry(&self) -> Option<view_frames::ViewFrameGeometry> {
        if self.dxf_name != "AECC_VIEWFRAME" {
            return None;
        }
        let graphic = super::arch::decode::entity_graphic_data(self)?;
        let (data, start, end) = self.main_stream()?;
        view_frames::view_frame_geometry(data, start, end, &graphic)
    }

    /// The rectangles a sheet (`AECC_SHEET`) record keeps in its
    /// graphic-image section; see [`view_frames::SheetGeometry`].
    pub fn sheet_geometry(&self) -> Option<view_frames::SheetGeometry> {
        if self.dxf_name != "AECC_SHEET" {
            return None;
        }
        let graphic = super::arch::decode::entity_graphic_data(self)?;
        let (data, start, end) = self.main_stream()?;
        view_frames::sheet_geometry(data, start, end, &graphic)
    }

    /// A match line (`AECC_MATCHLINE`) between two view frames: its name,
    /// its view frame group and the frames on its sides.
    pub fn match_line(&self) -> Option<view_frames::MatchLine> {
        if self.dxf_name != "AECC_MATCHLINE" {
            return None;
        }
        let strings = self.decode().strings;
        view_frames::match_line(&strings, &self.typed_handles())
    }

    /// The rectangle a match line (`AECC_MATCHLINE`) record keeps in its
    /// graphic-image section: the closed four-corner walk whose bounds are
    /// the live API's `GeometricExtents`; see
    /// [`view_frames::MatchLineGeometry`].
    pub fn match_line_geometry(&self) -> Option<view_frames::MatchLineGeometry> {
        if self.dxf_name != "AECC_MATCHLINE" {
            return None;
        }
        let graphic = super::arch::decode::entity_graphic_data(self)?;
        let (data, start, end) = self.main_stream()?;
        view_frames::match_line_geometry(data, start, end, &graphic)
    }

    /// Number fields of the part data a Civil 3D pipe (`AECC_PIPE`) keeps
    /// itself: flow coefficients, minimum curve radius and the like.
    pub fn pipe_part_fields(&self) -> Vec<PartField> {
        if self.dxf_name != "AECC_PIPE" {
            return Vec::new();
        }
        let strings = self.decode().strings;
        self.main_stream()
            .map(|(data, start, end)| network::pipe_fields(data, start, end, &strings))
            .unwrap_or_default()
    }

    /// Insertion point of a Civil 3D structure (`AECC_STRUCTURE`), read by
    /// the record's fields.
    pub fn structure_position(&self) -> Option<crate::types::Vector3> {
        if self.dxf_name != "AECC_STRUCTURE" {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        network::structure_at(data, start, end)
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

    /// The point codes a corridor draws feature lines for, in the order of
    /// [`Self::corridor_baselines`]' groups; empty when the record is not
    /// read.
    pub fn corridor_point_codes(&self) -> Vec<String> {
        if self.civil_kind() != Some(CivilKind::Corridor) {
            return Vec::new();
        }
        let strings = self.decode().strings;
        self.main_stream()
            .and_then(|(data, start, end)| corridor::point_codes(data, start, end, &strings))
            .unwrap_or_default()
    }

    /// What a corridor defines: settings, surfaces, baselines with
    /// their regions, frequencies, targets and added stations. Names and
    /// references are filled for a corridor that holds no built station.
    pub fn corridor_definition(&self) -> Option<CorridorDefinition> {
        if self.civil_kind() != Some(CivilKind::Corridor) {
            return None;
        }
        let refs: Vec<u64> = self.typed_handles().iter().map(|r| r.0).collect();
        let strings = self.decode().strings;
        let (data, start, end) = self.main_stream()?;
        corridor::definition(data, start, end, &strings, &refs)
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

    /// Geometry and style of a stormwater control entity (`AECC_POND`,
    /// `AECC_UNDERGROUNDSTORAGE` or `AECC_CHANNEL`); see
    /// [`stormwater::StormWater`]. The hydraulic parameters the application
    /// shows for these are not persisted in the record — see the module
    /// notes.
    pub fn stormwater(&self) -> Option<stormwater::StormWater> {
        let kind = match self.dxf_name.as_str() {
            "AECC_POND" => stormwater::StormWaterKind::Pond,
            "AECC_UNDERGROUNDSTORAGE" => stormwater::StormWaterKind::UndergroundStorage,
            "AECC_CHANNEL" => stormwater::StormWaterKind::Channel,
            _ => return None,
        };
        let (data, start, end) = self.main_stream()?;
        let graphic = super::arch::decode::entity_graphic_data(self)?;
        let handles = self.decode().handles;
        stormwater::entity(kind, data, start, end, &graphic, &handles)
    }

    /// Raw record and the bit range of its class body.
    fn main_stream(&self) -> Option<(&[u8], usize, usize)> {
        let reader = super::arch::decode::open_entity(self).ok()?;
        let start = usize::try_from(reader.position_in_bits()).ok()?;
        let len = usize::try_from(reader.main_remaining_bits()).ok()?;
        Some((self.raw_dwg_data.as_deref()?, start, start + len))
    }

    /// Geometry a Civil 3D parcel segment (`AECC_PARCEL_SEGMENT`, the
    /// application's lot line) record holds; see
    /// [`parcel_segments::ParcelSegmentGeometry`].
    pub fn parcel_segment_geometry(&self) -> Option<ParcelSegmentGeometry> {
        if self.dxf_name != "AECC_PARCEL_SEGMENT" {
            return None;
        }
        let graphic = super::arch::decode::entity_graphic_data(self)?;
        let (data, start, end) = self.main_stream()?;
        parcel_segments::geometry(&data, start, end, &graphic)
    }

    /// The references a grading entity (`AECC_GRADING`) record
    /// carries; see [`grading::GradingEntity`].
    pub fn grading(&self) -> Option<GradingEntity> {
        if self.dxf_name != "AECC_GRADING" {
            return None;
        }
        let handles = self.decode().handles;
        let (data, start, end) = self.main_stream()?;
        grading::entity(data, start, end, &handles)
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
        let decoded = self.decode();
        self.main_stream()
            .map(|(data, start, end)| cogo::points(data, start, end, &decoded.handles, &decoded.strings))
            .unwrap_or_default()
    }

    /// User-defined point property an `AECC_USER_DEFINED_ATTRIBUTE` record
    /// defines.
    pub fn user_property(&self) -> Option<UserProperty> {
        if self.dxf_name != "AECC_USER_DEFINED_ATTRIBUTE" {
            return None;
        }
        let strings = self.decode().strings;
        let (data, start, end) = self.main_stream()?;
        cogo::user_property(data, start, end, &strings)
    }

    /// What a point group (`AECC_POINT_PG`) of the recent form overrides
    /// in its points.
    pub fn point_group_overrides(&self) -> Option<PointGroupOverrides> {
        if self.dxf_name != "AECC_POINT_PG" {
            return None;
        }
        let d = self.decode();
        let (data, start, end) = self.main_stream()?;
        cogo::group_overrides(data, start, end, &d.handles, &d.strings)
    }

    /// Numbers of the points a Civil 3D point group (`AECC_POINT_PG`) holds.
    pub fn point_group_numbers(&self) -> Vec<u32> {
        if self.dxf_name != "AECC_POINT_PG" {
            return Vec::new();
        }
        self.main_stream()
            .and_then(|(data, start, end)| cogo::group_numbers(data, start, end))
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

    /// The feature-line topology of a site container
    /// (`AECC_FEATURE_STYLE`): the topology the application keeps for the
    /// container's feature lines once it extended the container (a site
    /// that gained members after it was first written) — one edge per
    /// feature line, not split where lines cross, unlike [`Self::topology`]
    /// on such a container, whose first section the parcels' faces hang
    /// off.
    ///
    /// Not wired yet: the bytes between the container's first section and
    /// the feature-line section carry the site's grading data, and no
    /// grammar for them is mapped (on the whole tutorial corpus the parcel
    /// grammars account for none of that span — 2026-11-25 sweep of all
    /// 163 drawings). Until that walk is mapped this reads through
    /// [`Self::topology`], whose first section is the feature lines' own
    /// on every container this crate writes and on the containers the
    /// application never extended.
    pub fn feature_line_topology(&self) -> Option<Topology> {
        if !self.dxf_name.eq_ignore_ascii_case("AECC_FEATURE_STYLE") {
            return None;
        }
        self.topology()
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

    /// Target and projection settings of a Civil 3D grading criteria
    /// (`AECC_GRADING_CRITERIA`).
    pub fn grading_criteria(&self) -> Option<GradingCriteria> {
        if self.dxf_name != "AECC_GRADING_CRITERIA" {
            return None;
        }
        let handles = self.decode().handles;
        let (data, start, end) = self.main_stream()?;
        grading::criteria(data, start, end, &handles)
    }

    /// A grading group record (`AECC_GRADING_GROUP`): a site's
    /// root group or a named group of gradings; see
    /// [`grading::GradingGroupRecord`].
    pub fn grading_group(&self) -> Option<GradingGroupRecord> {
        if self.dxf_name != "AECC_GRADING_GROUP" {
            return None;
        }
        let decoded = self.decode();
        let (data, start, end) = self.main_stream()?;
        grading::group_record(data, start, end, &decoded.strings, &decoded.handles)
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
        // A label style (the collector) is not a component and has no
        // component visibility.
        if !self.dxf_name.starts_with("AECC_LABEL_") || self.dxf_name == "AECC_LABEL_COLLECTOR_STYLE" {
            return true;
        }
        self.main_stream()
            .and_then(|(data, start, end)| labels::component_visible(data, start, end))
            .unwrap_or(true)
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

    /// The settings of a profile view style: the graph's title,
    /// exaggeration and direction, its grid and its four axes (see
    /// [`ProfileViewStyleSettings`]); text styles as handles.
    pub fn profile_view_style_settings(&self) -> Option<ProfileViewStyleSettings> {
        if self.dxf_name != "AECC_PROFILE_VIEW_STYLE" {
            return None;
        }
        let (data, start, end) = self.main_stream()?;
        style::view_style_settings(data, start, end, &self.decode().handles)
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
    /// codes to link styles and area codes to shape styles. The record's
    /// class data lists its default and code entries (see
    /// `style::code_set_entries`); each has five references (material,
    /// style, label style, feature line style, blank) and each code four
    /// strings (code, name, description, blank).
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
        let Some((defaults, codes)) = self.main_stream().and_then(|(data, start, end)| style::code_set_entries(data, start, end)) else {
            return (Vec::new(), Vec::new());
        };
        let decoded = self.decode();
        // The groups follow the owned children (a collection's styles).
        let refs: Vec<u64> = self.typed_handles().into_iter().skip_while(|r| r.1 == 2).map(|r| r.0).collect();
        let groups: Vec<&[u64]> = refs.chunks(5).filter(|g| g.len() == 5).collect();
        let strings = &decoded.strings;
        let Some(first) = strings.len().checked_sub(4 * codes) else {
            return (Vec::new(), Vec::new());
        };
        if groups.len() < defaults + codes {
            return (Vec::new(), Vec::new());
        }
        let entries = strings[first..].chunks(4).zip(&groups[defaults..]).map(|(c, group)| (c[0].clone(), group[1])).collect();
        (groups[..defaults].iter().map(|g| g[1]).collect(), entries)
    }

    /// The class data fields of a label collector (`collector`) or text
    /// component style, the index of its own reference and its texts
    /// after the key texts.
    pub(in crate::classes::civil) fn label_fields(&self, collector: bool) -> Option<(label_style::LabelFields, usize, usize)> {
        let iterator = match self.dxf_name.as_str() {
            "AECC_LABEL_COLLECTOR_STYLE" if collector => None,
            "AECC_LABEL_TEXT_STYLE" if !collector => None,
            "AECC_LABEL_TEXT_ITERATOR_STYLE" if !collector => Some(false),
            "AECC_LABEL_TEXT_ITERATOR_CURVE_OR_SPIRAL_STYLE" if !collector => Some(true),
            _ => return None,
        };
        let (data, start, end) = self.main_stream()?;
        let (class, own_ref, text_keys) = style::class_layout(data, start, end)?;
        let fields = match iterator {
            Some(curve) => label_style::LabelFields::read_iterator(data, class, end, curve)?,
            None => label_style::LabelFields::read(data, class, end, collector)?,
        };
        Some((fields, own_ref, text_keys))
    }

    /// Settings of a label style (`AECC_LABEL_COLLECTOR_STYLE`):
    /// visibility, layer, text style, behavior, plan readability, leader
    /// and dragged state, each `Some` when the style overrides the label
    /// style defaults.
    pub fn label_style_settings(&self) -> Option<LabelStyleSettings> {
        let (fields, own, _) = self.label_fields(true)?;
        let refs: Vec<u64> = self.typed_handles().into_iter().skip(own).map(|r| r.0).collect();
        Some(fields.settings(&refs))
    }

    /// Text components a label style lists (their handles), in order.
    pub fn label_style_components(&self) -> Vec<u64> {
        let Some((_, own, _)) = self.label_fields(true) else {
            return Vec::new();
        };
        let refs: Vec<u64> = self.typed_handles().into_iter().skip(own + label_style::BASE_REFS).map(|r| r.0).collect();
        // Three lists of the components and a closing null reference.
        let n = refs.len().saturating_sub(1) / 3;
        refs[..n].to_vec()
    }

    /// The fields of a line (`line`) or block component and where its
    /// references and texts start.
    pub(in crate::classes::civil) fn label_component_fields(&self, line: bool) -> Option<(label_style::LabelFields, usize, usize)> {
        (self.dxf_name == if line { "AECC_LABEL_RADIAL_LINE_STYLE" } else { "AECC_LABEL_BLOCK_STYLE" }).then_some(())?;
        let (data, start, end) = self.main_stream()?;
        let (class, own_ref, text_keys) = style::class_layout(data, start, end)?;
        Some((label_style::LabelFields::read_component(data, class, end, line)?, own_ref, text_keys))
    }

    /// Id and name (under a text id) of a label style component.
    fn label_component_head(&self) -> Option<(u32, Option<String>)> {
        let (data, start, end) = self.main_stream()?;
        let mut b = bits::Bits::new(data, start, end);
        b.bit_short()?;
        let id = b.bit_long()? as u32;
        Some((id, (id & 0x8000_0000 != 0).then(|| self.decode().strings.first().cloned()).flatten()))
    }

    /// The fields of a tick (`tick`) or direction arrow component.
    pub(in crate::classes::civil) fn label_mark_fields(&self, tick: bool) -> Option<(label_style::LabelFields, usize, usize)> {
        (self.dxf_name == if tick { "AECC_LABEL_TICK_STYLE" } else { "AECC_LABEL_VECTOR_ARROW_STYLE" }).then_some(())?;
        let (data, start, end) = self.main_stream()?;
        let (class, own_ref, text_keys) = style::class_layout(data, start, end)?;
        Some((label_style::LabelFields::read_mark(data, class, end, tick)?, own_ref, text_keys))
    }

    /// A label style's tick component (`AECC_LABEL_TICK_STYLE`).
    pub fn label_tick_component(&self) -> Option<LabelTickComponent> {
        let (fields, own, _) = self.label_mark_fields(true)?;
        let refs: Vec<u64> = self.typed_handles().into_iter().skip(own).map(|r| r.0).collect();
        let (id, name) = self.label_component_head()?;
        fields.tick_component(id, name, &refs)
    }

    /// A label style's direction arrow component
    /// (`AECC_LABEL_VECTOR_ARROW_STYLE`).
    pub fn label_arrow_component(&self) -> Option<LabelArrowComponent> {
        let (fields, _, _) = self.label_mark_fields(false)?;
        let (id, name) = self.label_component_head()?;
        fields.arrow_component(id, name)
    }

    /// A label style's line component (`AECC_LABEL_RADIAL_LINE_STYLE`).
    pub fn label_line_component(&self) -> Option<LabelLineComponent> {
        let (fields, own, _) = self.label_component_fields(true)?;
        let refs: Vec<u64> = self.typed_handles().into_iter().skip(own).map(|r| r.0).collect();
        let (id, name) = self.label_component_head()?;
        fields.line_component(id, name, &refs)
    }

    /// A label style's block component (`AECC_LABEL_BLOCK_STYLE`).
    pub fn label_block_component(&self) -> Option<LabelBlockComponent> {
        let (fields, own, _) = self.label_component_fields(false)?;
        let refs: Vec<u64> = self.typed_handles().into_iter().skip(own).map(|r| r.0).collect();
        let (id, name) = self.label_component_head()?;
        fields.block_component(id, name, &refs)
    }

    /// A label style's text iterator component
    /// (`AECC_LABEL_TEXT_ITERATOR_STYLE`,
    /// `AECC_LABEL_TEXT_ITERATOR_CURVE_OR_SPIRAL_STYLE`).
    pub fn label_iterator_component(&self) -> Option<LabelIteratorComponent> {
        self.dxf_name.starts_with("AECC_LABEL_TEXT_ITERATOR").then_some(())?;
        let (iterates, spiral_contents) = self.label_fields(false)?.0.iterator()?;
        Some(LabelIteratorComponent { text: self.label_text_component()?, iterates, spiral_contents })
    }

    /// A label style's text component (`AECC_LABEL_TEXT_STYLE`; the text
    /// settings of an iterator).
    pub fn label_text_component(&self) -> Option<LabelTextComponent> {
        let (fields, own, text_keys) = self.label_fields(false)?;
        let refs: Vec<u64> = self.typed_handles().into_iter().skip(own).map(|r| r.0).collect();
        let (data, start, end) = self.main_stream()?;
        let mut b = bits::Bits::new(data, start, end);
        b.bit_short()?;
        let id = b.bit_long()? as u32;
        let strings = self.decode().strings;
        let text_id = |id: u32| id & 0x8000_0000 != 0;
        let own_texts = if text_id(id) { 2 } else { 1 };
        let name = text_id(id).then(|| strings.first().cloned()).flatten();
        let anchor = fields.text_component(id, None, &refs, None)?.anchor_component;
        let anchor_name = text_id(anchor).then(|| strings.get(own_texts + text_keys).cloned()).flatten();
        fields.text_component(id, name, &refs, anchor_name)
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

/// Where a parcel's own fields start in its class body (see
/// [`ArchEntityData::parcel_fields`]): the schema word after the class
/// header gates the prefix.
fn parcel_fields_offset(data: &[u8], start: usize, end: usize) -> Option<usize> {
    let mut b = bits::Bits::new(data, start, end);
    match alignment::class_header(&mut b)? {
        alignment::KEYED => {
            for _ in 0..b.bits_value(8)? {
                alignment::key(&mut b)?;
            }
            b.bit_long()?;
            b.pos += 121;
            Some(b.pos)
        }
        // Civil 3D 2013: no keys.
        0b01_0010 => Some(b.pos + 128),
        // Older words open their prefix with a raw 32-bit head word whose
        // top byte gates what follows: `0x02` carries the 2013 prefix (four
        // raw longs) behind the head; `0x05` carries a keyed block — 8-bit
        // key count, keys (see [`alignment::key`]), a closing `BL`, then
        // the same 121 bits the keyed word carries. Either way the body
        // must validate to the record's exact end.
        _ => {
            let kind = b.raw_long()? as u32 >> 24;
            let at = match kind {
                0x02 => b.pos + 128,
                0x05 => {
                    for _ in 0..b.bits_value(8)? {
                        alignment::key(&mut b)?;
                    }
                    b.bit_long()?;
                    b.pos + 121
                }
                _ => return None,
            };
            parcel_body_ok(data, at, end).map(|_| at)
        }
    }
}

/// Whether a full parcel body parses at `at` (see
/// [`ArchEntityData::parcel_fields`] for the grammar): the label location
/// twice over, a face list, and the closing word to the record's exact end.
fn parcel_body_ok(data: &[u8], at: usize, end: usize) -> Option<()> {
    let mut b = bits::Bits::new(data, at, end);
    b.bit_long()?;
    b.bit_long()?;
    b.bit_long()?;
    (b.bit_double()? == 0.0).then_some(())?;
    let (x, y, z) = (b.bit_double()?, b.bit_double()?, b.bit_double()?);
    (x.is_finite() && y.is_finite()).then_some(())?;
    (matches!(b.bit_long()?, 80 | 112) && b.bit_long()? == 1).then_some(())?;
    (b.bit_double()? == x && b.bit_double()? == y && b.bit_double()? == z).then_some(())?;
    (b.bit_long()? == 2).then_some(())?;
    let n = b.bit_long()?;
    (1..=64).contains(&n).then_some(())?;
    for _ in 0..n {
        b.bit_long()?;
    }
    (b.bit_long()? == 0).then_some(())?;
    (b.bit()? == false).then_some(())?;
    (b.bit_short()? == 0x04BE).then_some(())?;
    (b.remaining() == 0).then_some(())
}

#[cfg(test)]
mod parcel_word_tests {
    use super::*;

    fn record(hex: &str) -> Vec<u8> {
        (0..hex.len() / 2)
            .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
            .collect()
    }

    /// Word 3, head 0x02040810: four raw longs, body at +160 (the shape of
    /// the Labels-Parcels word-2 records), parcel 25 at (100.5, -2.5).
    const WORD3_HEAD02: (&str, usize) = (
        "542541508045f02100c4020100b8ca4000390a4000394a4000398a4000119a8000000000020594000000000000001302541010000000000081650000000000000004c0902404e804000085f020",
        613,
    );

    /// Word 4, head 0x05040840: a ref key and a date key, closing `BL` 16,
    /// 121 bits, body at +303 (the Labels-Parcels re-save form).
    const WORD4_HEAD05: (&str, usize) = (
        "542541508045f02101100201014081010000407c40005800000000000000011000000000000000000000000000000023350000000000040b28000000000000002604a82020000000000102ca000000000000000981204809d00800010be040",
        756,
    );

    /// Word 0, head 0x050408f8: four keys (ref, long, date, ref), body at
    /// +373 (Parcel-1E's re-saved STANDARD parcels).
    const WORD0_HEAD05_4KEYS: (&str, usize) = (
        "542541508045f021003e0201014101010000403e00000607c4000580000000000000000048000044400000000000000000000000000000008cd40000000000102ca0000000000000009812a0808000000000040b28000000000000002604812027402000042f8100",
        826,
    );

    /// Word 2 with an unmapped head word: not admitted.
    const WORD2_HEAD_UNKNOWN: (&str, usize) = (
        "542541508045f0210090020100519a8000000000020594000000000000001302541010000000000081650000000000000004c0902404e804000085f020",
        485,
    );

    #[test]
    fn an_older_word_with_a_type_02_head_reads_the_2013_prefix_behind_it() {
        let (hex, bits) = WORD3_HEAD02;
        let data = record(hex);
        assert_eq!(parcel_fields_offset(&data, 0, bits), Some(74 + 160));
    }

    #[test]
    fn an_older_word_with_a_type_05_head_reads_keys_then_121_bits() {
        let (hex, bits) = WORD4_HEAD05;
        let data = record(hex);
        assert_eq!(parcel_fields_offset(&data, 0, bits), Some(74 + 303));
    }

    #[test]
    fn a_type_05_head_with_four_keys_lands_past_them() {
        let (hex, bits) = WORD0_HEAD05_4KEYS;
        let data = record(hex);
        assert_eq!(parcel_fields_offset(&data, 0, bits), Some(74 + 373));
    }

    #[test]
    fn an_unmapped_head_word_reads_none() {
        let (hex, bits) = WORD2_HEAD_UNKNOWN;
        let data = record(hex);
        assert_eq!(parcel_fields_offset(&data, 0, bits), None);
    }

    #[test]
    fn a_head_whose_body_fails_validation_reads_none() {
        // The keyed form parses, but the 0x04BE closing word of the body is
        // broken, so the record is not admitted.
        let (hex, bits) = WORD4_HEAD05;
        let mut data = record(hex);
        let last = data.len() - 1;
        data[last] ^= 0x10;
        assert_eq!(parcel_fields_offset(&data, 0, bits), None);
    }
}
