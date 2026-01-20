//! Non-graphical objects (OBJECTS section)
//!
//! Objects are non-graphical elements in a DXF file, such as dictionaries,
//! layouts, groups, and other organizational structures.

mod dictionary_variable;
mod group;
mod image_definition;
mod mlinestyle;
mod multileader_style;
mod plot_settings;
mod scale;
mod sort_entities_table;
mod table_style;
mod xrecord;

pub use dictionary_variable::DictionaryVariable;
pub use group::Group;
pub use image_definition::{ImageDefinition, ImageDefinitionReactor, ResolutionUnit};
pub use mlinestyle::{MLineStyle, MLineStyleElement, MLineStyleFlags};
pub use multileader_style::{
    BlockContentConnectionType, LeaderContentType, LeaderDrawOrderType,
    LeaderLinePropertyOverrideFlags, MultiLeaderDrawOrderType, MultiLeaderPathType,
    MultiLeaderPropertyOverrideFlags, MultiLeaderStyle, TextAlignmentType, TextAngleType,
    TextAttachmentDirectionType, TextAttachmentType,
};
pub use plot_settings::{
    PaperMargin, PlotFlags, PlotPaperUnits, PlotRotation, PlotSettings, PlotType, PlotWindow,
    ScaledType, ShadePlotMode, ShadePlotResolutionLevel,
};
pub use scale::Scale;
pub use sort_entities_table::{SortEntsEntry, SortEntitiesTable};
pub use table_style::{
    CellAlignment, RowCellStyle, TableBorderPropertyFlags, TableBorderType, TableCellBorder,
    TableCellStylePropertyFlags, TableFlowDirection, TableStyle, TableStyleFlags,
};
pub use xrecord::{DictionaryCloningFlags, XRecord, XRecordEntry, XRecordValue, XRecordValueType};

use crate::types::Handle;

/// Dictionary object - stores key-value pairs of object handles
#[derive(Debug, Clone)]
pub struct Dictionary {
    /// Unique handle
    pub handle: Handle,
    /// Owner handle (soft pointer)
    pub owner: Handle,
    /// Dictionary entries (key -> handle)
    pub entries: Vec<(String, Handle)>,
    /// Duplicate record cloning flag
    pub duplicate_cloning: i16,
    /// Hard owner flag
    pub hard_owner: bool,
}

impl Dictionary {
    /// Create a new dictionary
    pub fn new() -> Self {
        Self {
            handle: Handle::NULL,
            owner: Handle::NULL,
            entries: Vec::new(),
            duplicate_cloning: 1,
            hard_owner: false,
        }
    }

    /// Add an entry to the dictionary
    pub fn add_entry(&mut self, key: impl Into<String>, handle: Handle) {
        self.entries.push((key.into(), handle));
    }

    /// Get a handle by key
    pub fn get(&self, key: &str) -> Option<Handle> {
        self.entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, h)| *h)
    }

    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the dictionary is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for Dictionary {
    fn default() -> Self {
        Self::new()
    }
}

/// Layout object - represents a layout (model space or paper space)
#[derive(Debug, Clone)]
pub struct Layout {
    /// Unique handle
    pub handle: Handle,
    /// Owner handle (soft pointer)
    pub owner: Handle,
    /// Layout name
    pub name: String,
    /// Layout flags
    pub flags: i16,
    /// Tab order
    pub tab_order: i16,
    /// Minimum limits
    pub min_limits: (f64, f64),
    /// Maximum limits
    pub max_limits: (f64, f64),
    /// Insertion base point
    pub insertion_base: (f64, f64, f64),
    /// Minimum extents
    pub min_extents: (f64, f64, f64),
    /// Maximum extents
    pub max_extents: (f64, f64, f64),
    /// Associated block record handle
    pub block_record: Handle,
    /// Viewport handle
    pub viewport: Handle,
}

impl Layout {
    /// Create a new layout
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            handle: Handle::NULL,
            owner: Handle::NULL,
            name: name.into(),
            flags: 0,
            tab_order: 0,
            min_limits: (0.0, 0.0),
            max_limits: (12.0, 9.0),
            insertion_base: (0.0, 0.0, 0.0),
            min_extents: (0.0, 0.0, 0.0),
            max_extents: (12.0, 9.0, 0.0),
            block_record: Handle::NULL,
            viewport: Handle::NULL,
        }
    }
}

/// Object types
#[derive(Debug, Clone)]
pub enum ObjectType {
    /// Dictionary object
    Dictionary(Dictionary),
    /// Layout object
    Layout(Layout),
    /// XRecord object - extended data storage
    XRecord(XRecord),
    /// Group object - named collection of entities
    Group(Group),
    /// MLineStyle object - multiline style definition
    MLineStyle(MLineStyle),
    /// ImageDefinition object - raster image definition
    ImageDefinition(ImageDefinition),
    /// PlotSettings object - plot configuration
    PlotSettings(PlotSettings),
    /// MultiLeaderStyle object - multileader style definition
    MultiLeaderStyle(MultiLeaderStyle),
    /// TableStyle object - table style definition
    TableStyle(TableStyle),
    /// Scale object - named scale definition
    Scale(Scale),
    /// SortEntitiesTable object - entity draw order
    SortEntitiesTable(SortEntitiesTable),
    /// DictionaryVariable object - named variable in dictionary
    DictionaryVariable(DictionaryVariable),
    /// Unknown object type (stored as raw data)
    Unknown {
        /// Object type name
        type_name: String,
        /// Object handle
        handle: Handle,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dictionary_creation() {
        let mut dict = Dictionary::new();
        assert!(dict.is_empty());

        dict.add_entry("KEY1", Handle::new(100));
        assert_eq!(dict.len(), 1);
        assert_eq!(dict.get("KEY1"), Some(Handle::new(100)));
        assert_eq!(dict.get("KEY2"), None);
    }

    #[test]
    fn test_layout_creation() {
        let layout = Layout::new("Layout1");
        assert_eq!(layout.name, "Layout1");
        assert_eq!(layout.tab_order, 0);
    }
}

