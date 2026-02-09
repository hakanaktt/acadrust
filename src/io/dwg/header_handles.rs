//! DWG header handles collection.
//!
//! The DWG file header section contains handle references to various
//! control objects (table controls, dictionary roots, default entries, etc.).
//! This collection stores those handles as the header is being read, and
//! later resolves them when building the `CadDocument`.
//!
//! Mirrors ACadSharp's `DwgHeaderHandlesCollection`.

use std::collections::HashMap;

/// Collection of named handle references found in the DWG file header.
///
/// Each handle is identified by a string name and stores an optional `u64`
/// handle value. After reading, the handles are used to seed the object
/// reader's processing queue and to link header variables to their
/// corresponding objects.
#[derive(Debug, Clone, Default)]
pub struct DwgHeaderHandlesCollection {
    handles: HashMap<String, Option<u64>>,
}

// Macro to generate getter/setter pairs for named handles.
macro_rules! handle_property {
    ($name:ident, $key:expr) => {
        pub fn $name(&self) -> Option<u64> {
            self.get_handle($key)
        }

        paste::paste! {
            pub fn [<set_ $name>](&mut self, value: Option<u64>) {
                self.set_handle($key, value);
            }
        }
    };
}

impl DwgHeaderHandlesCollection {
    /// Create a new empty handles collection.
    pub fn new() -> Self {
        Self {
            handles: HashMap::new(),
        }
    }

    /// Get a handle value by name.
    pub fn get_handle(&self, name: &str) -> Option<u64> {
        self.handles.get(name).copied().flatten()
    }

    /// Set a handle value by name.
    pub fn set_handle(&mut self, name: &str, value: Option<u64>) {
        self.handles.insert(name.to_string(), value);
    }

    /// Get all handles as a list (including `None` values).
    pub fn get_all_handles(&self) -> Vec<Option<u64>> {
        self.handles.values().copied().collect()
    }

    /// Get all non-None handle values.
    pub fn get_valid_handles(&self) -> Vec<u64> {
        self.handles
            .values()
            .filter_map(|v| *v)
            .collect()
    }

    /// Get the number of stored handles.
    pub fn len(&self) -> usize {
        self.handles.len()
    }

    /// Check if the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }

    // -----------------------------------------------------------------------
    // Named handle accessors
    //
    // These mirror the properties in ACadSharp's DwgHeaderHandlesCollection.
    // -----------------------------------------------------------------------

    // Current settings handles
    handle_property!(cmaterial, "CMATERIAL");
    handle_property!(clayer, "CLAYER");
    handle_property!(textstyle, "TEXTSTYLE");
    handle_property!(celtype, "CELTYPE");
    handle_property!(dimstyle, "DIMSTYLE");
    handle_property!(cmlstyle, "CMLSTYLE");

    // UCS handles
    handle_property!(ucsname_pspace, "UCSNAME_PSPACE");
    handle_property!(ucsname_mspace, "UCSNAME_MSPACE");
    handle_property!(pucsorthoref, "PUCSORTHOREF");
    handle_property!(pucsbase, "PUCSBASE");
    handle_property!(ucsorthoref, "UCSORTHOREF");
    handle_property!(ucsbase, "UCSBASE");

    // Dimension handles
    handle_property!(dimtxsty, "DIMTXSTY");
    handle_property!(dimldrblk, "DIMLDRBLK");
    handle_property!(dimblk, "DIMBLK");
    handle_property!(dimblk1, "DIMBLK1");
    handle_property!(dimblk2, "DIMBLK2");
    handle_property!(dimltype, "DIMLTYPE");
    handle_property!(dimltex1, "DIMLTEX1");
    handle_property!(dimltex2, "DIMLTEX2");

    // Dictionary handles
    handle_property!(dictionary_layouts, "DICTIONARY_LAYOUTS");
    handle_property!(dictionary_plotsettings, "DICTIONARY_PLOTSETTINGS");
    handle_property!(dictionary_plotstyles, "DICTIONARY_PLOTSTYLES");
    handle_property!(dictionary_acad_group, "DICTIONARY_ACAD_GROUP");
    handle_property!(dictionary_acad_mlinestyle, "DICTIONARY_ACAD_MLINESTYLE");
    handle_property!(dictionary_named_objects, "DICTIONARY_NAMED_OBJECTS");
    handle_property!(dictionary_materials, "DICTIONARY_MATERIALS");
    handle_property!(dictionary_colors, "DICTIONARY_COLORS");
    handle_property!(dictionary_visualstyle, "DICTIONARY_VISUALSTYLE");

    // Block/layout handles
    handle_property!(cpsnid, "CPSNID");
    handle_property!(paper_space, "PAPER_SPACE");
    handle_property!(model_space, "MODEL_SPACE");
    handle_property!(bylayer, "BYLAYER");
    handle_property!(byblock, "BYBLOCK");
    handle_property!(continuous, "CONTINUOUS");

    // Table control object handles
    handle_property!(block_control_object, "BLOCK_CONTROL_OBJECT");
    handle_property!(layer_control_object, "LAYER_CONTROL_OBJECT");
    handle_property!(style_control_object, "STYLE_CONTROL_OBJECT");
    handle_property!(linetype_control_object, "LINETYPE_CONTROL_OBJECT");
    handle_property!(view_control_object, "VIEW_CONTROL_OBJECT");
    handle_property!(ucs_control_object, "UCS_CONTROL_OBJECT");
    handle_property!(vport_control_object, "VPORT_CONTROL_OBJECT");
    handle_property!(appid_control_object, "APPID_CONTROL_OBJECT");
    handle_property!(dimstyle_control_object, "DIMSTYLE_CONTROL_OBJECT");
    handle_property!(viewport_entity_header_control_object, "VIEWPORT_ENTITY_HEADER_CONTROL_OBJECT");

    // Visual style handles (R2007+)
    handle_property!(interfereobjvs, "INTERFEREOBJVS");
    handle_property!(interferevpvs, "INTERFEREVPVS");
    handle_property!(dragvs, "DRAGVS");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_collection_is_empty() {
        let handles = DwgHeaderHandlesCollection::new();
        assert!(handles.is_empty());
        assert_eq!(handles.len(), 0);
    }

    #[test]
    fn test_set_and_get_handle() {
        let mut handles = DwgHeaderHandlesCollection::new();
        handles.set_handle("CLAYER", Some(0x1A));
        assert_eq!(handles.get_handle("CLAYER"), Some(0x1A));
        assert_eq!(handles.clayer(), Some(0x1A));
    }

    #[test]
    fn test_none_handle() {
        let mut handles = DwgHeaderHandlesCollection::new();
        handles.set_handle("DIMBLK", None);
        assert_eq!(handles.get_handle("DIMBLK"), None);
        assert_eq!(handles.dimblk(), None);
    }

    #[test]
    fn test_named_accessors() {
        let mut handles = DwgHeaderHandlesCollection::new();
        handles.set_clayer(Some(0x10));
        handles.set_textstyle(Some(0x20));
        handles.set_block_control_object(Some(0x30));

        assert_eq!(handles.clayer(), Some(0x10));
        assert_eq!(handles.textstyle(), Some(0x20));
        assert_eq!(handles.block_control_object(), Some(0x30));
    }

    #[test]
    fn test_get_valid_handles() {
        let mut handles = DwgHeaderHandlesCollection::new();
        handles.set_handle("A", Some(1));
        handles.set_handle("B", None);
        handles.set_handle("C", Some(3));

        let valid = handles.get_valid_handles();
        assert_eq!(valid.len(), 2);
        assert!(valid.contains(&1));
        assert!(valid.contains(&3));
    }

    #[test]
    fn test_missing_handle_returns_none() {
        let handles = DwgHeaderHandlesCollection::new();
        assert_eq!(handles.get_handle("NONEXISTENT"), None);
        assert_eq!(handles.clayer(), None);
    }
}
