//! DXF class definitions (CLASSES section)
//!
//! Classes define custom object types registered in the DXF drawing.
//! Each class maps a DXF entity/object name to its C++ class name and
//! application that registered it.
//!
//! Corresponds to ACadSharp's `DxfClass` and `DxfClassCollection`.

use std::collections::HashMap;

/// Proxy capability flags for DXF class definitions.
///
/// These flags control what operations are allowed on proxy entities/objects
/// when the application that created them is not available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProxyFlags(pub u16);

impl ProxyFlags {
    pub const NONE: Self = Self(0);
    pub const ERASE_ALLOWED: Self = Self(1);
    pub const TRANSFORM_ALLOWED: Self = Self(2);
    pub const COLOR_CHANGE_ALLOWED: Self = Self(4);
    pub const LAYER_CHANGE_ALLOWED: Self = Self(8);
    pub const LINETYPE_CHANGE_ALLOWED: Self = Self(16);
    pub const LINETYPE_SCALE_CHANGE_ALLOWED: Self = Self(32);
    pub const VISIBILITY_CHANGE_ALLOWED: Self = Self(64);
    pub const CLONING_ALLOWED: Self = Self(128);
    pub const LINEWEIGHT_CHANGE_ALLOWED: Self = Self(256);
    pub const PLOT_STYLE_NAME_CHANGE_ALLOWED: Self = Self(512);
    pub const ALL_OPERATIONS_EXCEPT_CLONING: Self = Self(895);
    pub const ALL_OPERATIONS_ALLOWED: Self = Self(1023);
    pub const DISABLES_PROXY_WARNING_DIALOG: Self = Self(1024);
    pub const R13_FORMAT_PROXY: Self = Self(32768);

    /// Check if a specific flag is set
    pub fn contains(self, flag: Self) -> bool {
        (self.0 & flag.0) == flag.0
    }
}

impl Default for ProxyFlags {
    fn default() -> Self {
        Self::NONE
    }
}

impl From<u16> for ProxyFlags {
    fn from(val: u16) -> Self {
        Self(val)
    }
}

impl From<i32> for ProxyFlags {
    fn from(val: i32) -> Self {
        Self(val as u16)
    }
}

/// A single DXF class definition.
///
/// DXF group codes:
/// - 1: DXF class name (e.g. "MLEADERSTYLE")
/// - 2: C++ class name (e.g. "AcDbMLeaderStyle")
/// - 3: Application name (e.g. "ObjectDBX Classes")
/// - 90: Proxy capability flags
/// - 91: Instance count (informational)
/// - 280: Was-a-zombie flag
/// - 281: Is-an-entity flag (1 = can appear in ENTITIES/BLOCKS, 0 = OBJECTS only)
#[derive(Debug, Clone)]
pub struct DxfClass {
    /// DXF class name (group code 1) — e.g. "ACDBPLACEHOLDER"
    pub dxf_name: String,
    /// C++ class name (group code 2) — e.g. "AcDbPlaceHolder"
    pub cpp_class_name: String,
    /// Application name (group code 3) — e.g. "ObjectDBX Classes"
    pub application_name: String,
    /// Proxy capability flags (group code 90)
    pub proxy_flags: ProxyFlags,
    /// Instance count for this class in the drawing (group code 91)
    pub instance_count: i32,
    /// Was-a-zombie flag (group code 280) — true if class was a proxy
    pub was_zombie: bool,
    /// Is-an-entity flag (group code 281) — true if instances can appear in ENTITIES/BLOCKS
    pub is_an_entity: bool,
    /// Class number (assigned sequentially starting at 500)
    pub class_number: i16,
    /// Item class ID: 498 for entities, 499 for objects
    pub item_class_id: i16,
    /// DWG version that created this class (R2004+)
    pub dwg_version: i32,
    /// Maintenance release version (R2004+)
    pub maintenance_version: i16,
}

impl Default for DxfClass {
    fn default() -> Self {
        Self {
            dxf_name: String::new(),
            cpp_class_name: String::new(),
            application_name: "ObjectDBX Classes".to_string(),
            proxy_flags: ProxyFlags::NONE,
            instance_count: 0,
            was_zombie: false,
            is_an_entity: false,
            class_number: 0,
            item_class_id: 499,
            dwg_version: 0,
            maintenance_version: 0,
        }
    }
}

impl DxfClass {
    /// Create a new DXF class definition
    pub fn new(dxf_name: impl Into<String>, cpp_class_name: impl Into<String>) -> Self {
        Self {
            dxf_name: dxf_name.into(),
            cpp_class_name: cpp_class_name.into(),
            application_name: "ObjectDBX Classes".to_string(),
            proxy_flags: ProxyFlags::NONE,
            instance_count: 0,
            was_zombie: false,
            is_an_entity: false,
            class_number: 0,
            item_class_id: 499, // default to object
            dwg_version: 0,
            maintenance_version: 0,
        }
    }

    /// Create a class for an entity type (can appear in ENTITIES/BLOCKS)
    pub fn new_entity(dxf_name: impl Into<String>, cpp_class_name: impl Into<String>) -> Self {
        let mut class = Self::new(dxf_name, cpp_class_name);
        class.is_an_entity = true;
        class.item_class_id = 498;
        class
    }
}

/// Collection of DXF class definitions, keyed by DXF name (case-insensitive).
///
/// Corresponds to ACadSharp's `DxfClassCollection`.
#[derive(Debug, Clone)]
pub struct DxfClassCollection {
    entries: Vec<DxfClass>,
    name_index: HashMap<String, usize>,
}

impl DxfClassCollection {
    /// Create an empty class collection
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            name_index: HashMap::new(),
        }
    }

    /// Add a class. If a class with the same DXF name already exists,
    /// only its instance count is updated (matching ACadSharp behavior).
    pub fn add_or_update(&mut self, mut class: DxfClass) {
        let key = class.dxf_name.to_uppercase();
        if let Some(&idx) = self.name_index.get(&key) {
            self.entries[idx].instance_count = class.instance_count;
        } else {
            if class.class_number < 500 {
                class.class_number = 500 + self.entries.len() as i16;
            }
            let idx = self.entries.len();
            self.name_index.insert(key, idx);
            self.entries.push(class);
        }
    }

    /// Get a class by its DXF name (case-insensitive)
    pub fn get_by_name(&self, dxf_name: &str) -> Option<&DxfClass> {
        let key = dxf_name.to_uppercase();
        self.name_index.get(&key).map(|&idx| &self.entries[idx])
    }

    /// Check if a class with the given DXF name exists
    pub fn contains(&self, dxf_name: &str) -> bool {
        self.name_index.contains_key(&dxf_name.to_uppercase())
    }

    /// Number of class definitions
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the collection is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over all class definitions
    pub fn iter(&self) -> impl Iterator<Item = &DxfClass> {
        self.entries.iter()
    }

    /// Clear all class definitions
    pub fn clear(&mut self) {
        self.entries.clear();
        self.name_index.clear();
    }

    /// Populate with default class definitions that AutoCAD expects.
    ///
    /// This mirrors ACadSharp's `DxfClassCollection.UpdateDxfClasses()`.
    pub fn update_defaults(&mut self) {
        let defaults = default_classes();
        for class in defaults {
            if !self.contains(&class.dxf_name) {
                self.add_or_update(class);
            }
        }
    }
}

impl Default for DxfClassCollection {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> IntoIterator for &'a DxfClassCollection {
    type Item = &'a DxfClass;
    type IntoIter = std::slice::Iter<'a, DxfClass>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

/// Build the set of default DXF classes that AutoCAD registers.
///
/// Each class gets `proxy_flags = AllOperationsAllowed (1023)` unless otherwise noted.
fn default_classes() -> Vec<DxfClass> {
    let mut classes = Vec::new();

    // Entity classes (item_class_id = 498)
    for &(dxf, cpp) in &[
        ("MESH", "AcDbSubDMesh"),
        ("ACAD_TABLE", "AcDbTable"),
        ("WIPEOUT", "AcDbWipeout"),
        ("IMAGE", "AcDbRasterImage"),
        ("PDFUNDERLAY", "AcDbPdfReference"),
        ("DWFUNDERLAY", "AcDbDwfReference"),
        ("DGNUNDERLAY", "AcDbDgnReference"),
        ("MULTILEADER", "AcDbMLeader"),
        ("OLE2FRAME", "AcDbOle2Frame"),
        ("MLINE", "AcDbMline"),
    ] {
        let mut c = DxfClass::new_entity(dxf, cpp);
        c.proxy_flags = ProxyFlags::ALL_OPERATIONS_ALLOWED;
        classes.push(c);
    }

    // Object classes (item_class_id = 499)
    for &(dxf, cpp) in &[
        ("DICTIONARYWDFLT", "AcDbDictionaryWithDefault"),
        ("ACDBPLACEHOLDER", "AcDbPlaceHolder"),
        ("LAYOUT", "AcDbLayout"),
        ("DICTIONARYVAR", "AcDbDictionaryVar"),
        ("TABLESTYLE", "AcDbTableStyle"),
        ("MATERIAL", "AcDbMaterial"),
        ("VISUALSTYLE", "AcDbVisualStyle"),
        ("SCALE", "AcDbScale"),
        ("MLEADERSTYLE", "AcDbMLeaderStyle"),
        ("CELLSTYLEMAP", "AcDbCellStyleMap"),
        ("XRECORD", "AcDbXrecord"),
        ("ACDB_XRECORD_CLASS", "AcDbXrecord"),
        ("SORTENTSTABLE", "AcDbSortentsTable"),
        ("WIPEOUTVARIABLES", "AcDbWipeoutVariables"),
        ("DIMASSOC", "AcDbDimAssoc"),
        ("TABLECONTENT", "AcDbTableContent"),
        ("TABLEGEOMETRY", "AcDbTableGeometry"),
        ("RASTERVARIABLES", "AcDbRasterVariables"),
        ("IMAGEDEF", "AcDbRasterImageDef"),
        ("IMAGEDEF_REACTOR", "AcDbRasterImageDefReactor"),
        ("DBCOLOR", "AcDbColor"),
        ("GEODATA", "AcDbGeoData"),
        ("PDFDEFINITION", "AcDbPdfDefinition"),
        ("DWFDEFINITION", "AcDbDwfDefinition"),
        ("DGNDEFINITION", "AcDbDgnDefinition"),
        ("SPATIALFILTER", "AcDbSpatialFilter"),
        ("PLOTSETTINGS", "AcDbPlotSettings"),
        ("GROUP", "AcDbGroup"),
        ("MLINESTYLE", "AcDbMlineStyle"),
    ] {
        let mut c = DxfClass::new(dxf, cpp);
        c.proxy_flags = ProxyFlags::ALL_OPERATIONS_ALLOWED;
        classes.push(c);
    }

    classes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dxf_class_creation() {
        let class = DxfClass::new("MLEADERSTYLE", "AcDbMLeaderStyle");
        assert_eq!(class.dxf_name, "MLEADERSTYLE");
        assert_eq!(class.cpp_class_name, "AcDbMLeaderStyle");
        assert!(!class.is_an_entity);
        assert_eq!(class.item_class_id, 499);
    }

    #[test]
    fn test_entity_class() {
        let class = DxfClass::new_entity("MESH", "AcDbSubDMesh");
        assert!(class.is_an_entity);
        assert_eq!(class.item_class_id, 498);
    }

    #[test]
    fn test_collection_add_or_update() {
        let mut coll = DxfClassCollection::new();

        let mut c = DxfClass::new("XRECORD", "AcDbXrecord");
        c.instance_count = 5;
        coll.add_or_update(c);
        assert_eq!(coll.len(), 1);
        assert_eq!(coll.get_by_name("XRECORD").unwrap().instance_count, 5);
        assert_eq!(coll.get_by_name("XRECORD").unwrap().class_number, 500);

        // Update instance count
        let mut c2 = DxfClass::new("xrecord", "AcDbXrecord");
        c2.instance_count = 10;
        coll.add_or_update(c2);
        assert_eq!(coll.len(), 1);
        assert_eq!(coll.get_by_name("XRECORD").unwrap().instance_count, 10);
    }

    #[test]
    fn test_collection_defaults() {
        let mut coll = DxfClassCollection::new();
        coll.update_defaults();
        assert!(coll.len() > 20);
        assert!(coll.contains("MESH"));
        assert!(coll.contains("LAYOUT"));
        assert!(coll.contains("MLEADERSTYLE"));
    }

    #[test]
    fn test_proxy_flags() {
        let flags = ProxyFlags::ALL_OPERATIONS_ALLOWED;
        assert!(flags.contains(ProxyFlags::ERASE_ALLOWED));
        assert!(flags.contains(ProxyFlags::TRANSFORM_ALLOWED));
        assert!(flags.contains(ProxyFlags::CLONING_ALLOWED));
    }
}
