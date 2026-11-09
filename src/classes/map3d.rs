//! Map 3D display-management and feature-source classes.
//!
//! These dictionary objects hold Map 3D's display maps, style libraries and
//! resource bookkeeping. They have no graphics; the reader keeps them as
//! raw objects and saves write them back unchanged. This catalog names them.

/// DXF name, C++ class and registering application of each class.
pub const OBJECT_CLASSES: &[(&str, &str, &str)] = &[
    ("DMMAPMANAGER", "AcMapDMMapManager", "AcMapDisplayManagement"),
    ("DMSTYLELIBRARY", "AcMapDMStyleLibrary", "AcMapDisplayManagement"),
    ("DMSTYLECATEGORY", "AcMapDMStyleCategory", "AcMapDisplayManagement"),
    ("DMMAP", "AcMapDMMap", "AcMapDisplayManagement"),
    ("DMBASEELEMENT", "AcMapDMBaseElement", "AcMapDisplayManagement"),
    ("DMDEFAULTSTYLE", "AcMapDMDefaultStyle", "AcMapDisplayManagement"),
    ("DMSTYLEREFERENCE", "AcMapDMStyleReference", "AcMapDisplayManagement"),
    ("DMSURROGATESTYLESETS", "AcMapDMSurrogateStyleSets", "AcMapDisplayManagement"),
    ("DMSTYLIZEDENTITIESTABLE", "AcMapDMStylizedEntitiesTable", "AcMapDisplayManagement"),
    ("MAPGWSUNDOOBJECT", "AcMapGWSUndoRecorder", "AcMapFeatureSourceManager"),
    ("MAPRESOURCEMANAGEROBJECT", "AcMapResourceManager", "AcMapSharedComponents"),
    ("MAPFSMRVOBJECT", "AcMapDMResourceVersionObject", "AcMapFeatureSourceManager"),
    ("MAPMETADATAOBJECT", "AcMapMetadataPersistObject", "AcMapFeatureSourceManager"),
];

/// C++ class name of a known Map 3D object class.
pub fn cpp_class_name(dxf_name: &str) -> Option<&'static str> {
    let upper = dxf_name.trim().to_ascii_uppercase();
    OBJECT_CLASSES
        .iter()
        .find(|(name, _, _)| *name == upper)
        .map(|(_, cpp, _)| *cpp)
}
