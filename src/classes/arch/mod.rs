//! ACAD Arch classes.
//!
//! Walls, doors, stairs, spaces, slabs, curtain walls, structural members, and
//! the style and display objects that belong to them are registered AEC
//! classes. Opaque native bodies are preserved on same-version DWG saves.
//! Supported typed edits update decoded native parameters and cached graphics
//! together. Unsupported layouts return [`ArchEditError`] and leave the entity
//! unchanged.
//!
//! The calling guide is
//! [src/classes/arch/API.md](https://github.com/hakanaktt/acadrust/blob/main/src/classes/arch/API.md).

mod continuous;
mod create;
mod decode;
mod display;
mod entity;
mod fields;
mod geometry;
mod host;
mod metrics;
mod object;
mod setters;

pub use continuous::{
    continuous_wall_outline, continuous_walls, set_continuous_wall, ContinuousWall,
};
pub use create::{door, wall, window};
pub use decode::{AecPlacement, DecodedAec};
pub use display::{
    curtain_wall_geometry, grid_geometry, member_geometry, opening_geometry, railing_geometry,
    stair_geometry, CurtainWallGeometry, GridGeometry, MemberGeometry, OpeningGeometry,
    RailingGeometry, StairGeometry,
};
pub use entity::{
    centered_wall_profile, reshape_rectangle, set_profile_bounds, set_wall, ArchEditError,
    ArchEntityData, ArchKind, StraightWallGeometry,
};
pub use fields::{NumberRole, PreviewRecord, StoredNumber};
pub use geometry::{decode_profiles, map_profile_points, polyline_from_type6};
pub use host::entities_anchored_to;
pub(crate) use host::{is_architecture_entity, objects_owned_by};
pub use metrics::{
    door_size, member_size, opening_size, stair_size, window_size, MemberSize, OpeningSize,
    StairSize,
};
pub use object::ArchObject;
pub use setters::{
    set_ceiling_grid, set_column_grid, set_curtain_wall, set_display_theme, set_door,
    set_mass_element, set_member, set_opening, set_railing, set_roof, set_roof_slab, set_slab,
    set_space, set_stair, set_window, set_window_assembly,
};

/// DXF names of the Arch entities found in the Arch sample
/// projects, with the C++ class ACAD registers for each.
pub const ENTITY_CLASSES: &[(&str, &str, ArchKind)] = &[
    ("AEC_WALL", "AecDbWall", ArchKind::Wall),
    ("AEC_DOOR", "AecDbDoor", ArchKind::Door),
    ("AEC_WINDOW", "AecDbWindow", ArchKind::Window),
    ("AEC_OPENING", "AecDbOpening", ArchKind::Opening),
    ("AEC_STAIR", "AecDbStair", ArchKind::Stair),
    ("AEC_RAILING", "AecDbRailing", ArchKind::Railing),
    ("AEC_SLAB", "AecDbSlab", ArchKind::Slab),
    ("AEC_ROOFSLAB", "AecDbRoofSlab", ArchKind::RoofSlab),
    ("AEC_ROOF", "AecDbRoof", ArchKind::Roof),
    ("AEC_SPACE", "AecDbSpace", ArchKind::Space),
    (
        "AEC_CURTAIN_WALL_LAYOUT",
        "AecDbCurtainWallLayout",
        ArchKind::CurtainWall,
    ),
    (
        "AEC_CURTAIN_WALL_UNIT",
        "AecDbCurtainWallUnit",
        ArchKind::CurtainWallUnit,
    ),
    (
        "AEC_WINDOW_ASSEMBLY",
        "AecDbWindowAssembly",
        ArchKind::WindowAssembly,
    ),
    ("AEC_COLUMN_GRID", "AecDbColumnGrid", ArchKind::ColumnGrid),
    (
        "AEC_CEILING_GRID",
        "AecDbCeilingGrid",
        ArchKind::CeilingGrid,
    ),
    ("AEC_POLYGON", "AecDbPolygon", ArchKind::Polygon),
    ("AEC_MASS_ELEM", "AecDbMassElem", ArchKind::MassElement),
    ("AEC_MASS_GROUP", "AecDbMassGroup", ArchKind::MassGroup),
    (
        "AEC_MVBLOCK_REF",
        "AecDbMvBlockRef",
        ArchKind::MultiViewBlock,
    ),
    (
        "AEC_MASKBLOCK_REF",
        "AecDbMaskBlockRef",
        ArchKind::MaskBlock,
    ),
    ("AEC_2D_SECTION", "AecDb2dSection", ArchKind::Section),
    (
        "AEC_BDG_SECTIONLINE",
        "AecDbBdgSectionLine",
        ArchKind::SectionLine,
    ),
    (
        "AEC_BDG_ELEVLINE",
        "AecDbBdgElevLine",
        ArchKind::ElevationLine,
    ),
    (
        "AEC_DIMENSION_GROUP",
        "AecDbDimensionGroup",
        ArchKind::Dimension,
    ),
    (
        "AEC_SCHEDULE_TABLE",
        "AecDbScheduleTable",
        ArchKind::Schedule,
    ),
    (
        "AEC_LAYOUT_CURVE",
        "AecDbLayoutCurve",
        ArchKind::LayoutCurve,
    ),
    (
        "AEC_LAYOUT_GRID2D",
        "AecDbLayoutGrid2d",
        ArchKind::LayoutGrid,
    ),
    (
        "AEC_LAYOUT_GRID3D",
        "AecDbLayoutGrid3d",
        ArchKind::LayoutGrid,
    ),
    ("AEC_ZONE", "AecDbZone", ArchKind::Zone),
    ("AEC_AREA", "AecDbArea", ArchKind::Area),
    ("AEC_AREAGROUP", "AecDbAreaGroup", ArchKind::AreaGroup),
    ("AEC_CAMERA", "AecDbCamera", ArchKind::Camera),
    ("AEC_SLICE", "AecDbSlice", ArchKind::Slice),
    ("AEC_ENT_REF", "AecDbEntRef", ArchKind::EntityReference),
    ("AEC_POINT", "AecDbPoint", ArchKind::Point),
    (
        "AEC_DISPLAYTHEME",
        "AecDbDisplayTheme",
        ArchKind::DisplayTheme,
    ),
    ("AECS_MEMBER", "AecsDbMember", ArchKind::StructuralMember),
];

/// Whether `dxf_name` is an ACAD Arch / MEP class.
///
/// Matches the AEC, structural, and building-systems classes stored in the
/// Arch sample drawings. Third-party classes are left untouched.
pub fn is_architecture_name(dxf_name: &str) -> bool {
    let name = dxf_name.trim();
    let upper = name.to_ascii_uppercase();
    upper.starts_with("AEC") || upper.starts_with("AAEC") || upper.starts_with("ABST_")
}

/// Entity kind for a known Arch DXF name.
pub fn kind_of(dxf_name: &str) -> Option<ArchKind> {
    let upper = dxf_name.trim().to_ascii_uppercase();
    ENTITY_CLASSES
        .iter()
        .find(|(name, _, _)| *name == upper)
        .map(|(_, _, kind)| *kind)
}

/// C++ class name registered for a known Arch entity, when this crate
/// knows it. Callers that have the drawing's CLASSES section should prefer
/// that entry.
pub fn cpp_class_name(dxf_name: &str) -> Option<&'static str> {
    let upper = dxf_name.trim().to_ascii_uppercase();
    ENTITY_CLASSES
        .iter()
        .find(|(name, _, _)| *name == upper)
        .map(|(_, cpp, _)| *cpp)
}
