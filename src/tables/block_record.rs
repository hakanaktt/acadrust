//! Block record table entry

use super::TableEntry;
use crate::entities::EntityType;
use crate::types::Handle;

/// Block record flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockFlags {
    /// Block is anonymous
    pub anonymous: bool,
    /// Block has attributes
    pub has_attributes: bool,
    /// Block is external reference (xref)
    pub is_xref: bool,
    /// Block is xref overlay
    pub is_xref_overlay: bool,
    /// Block is from external reference
    pub is_external: bool,
}

impl BlockFlags {
    /// Create default block flags
    pub fn new() -> Self {
        BlockFlags {
            anonymous: false,
            has_attributes: false,
            is_xref: false,
            is_xref_overlay: false,
            is_external: false,
        }
    }
}

impl Default for BlockFlags {
    fn default() -> Self {
        Self::new()
    }
}

/// A block record table entry
#[derive(Debug, Clone)]
pub struct BlockRecord {
    /// Unique handle for the block record table entry
    pub handle: Handle,
    /// Handle for the BLOCK entity
    pub block_entity_handle: Handle,
    /// Handle for the ENDBLK entity
    pub block_end_handle: Handle,
    /// Block name
    pub name: String,
    /// Block flags
    pub flags: BlockFlags,
    /// Layout handle (if this block is a layout)
    pub layout: Handle,
    /// Units for block scaling
    pub units: i16,
    /// Explodability flag
    pub explodable: bool,
    /// Can scale uniformly
    pub scale_uniformly: bool,
    /// Entities owned by this block
    pub entities: Vec<EntityType>,
}

impl BlockRecord {
    /// Create a new block record
    pub fn new(name: impl Into<String>) -> Self {
        BlockRecord {
            handle: Handle::NULL,
            block_entity_handle: Handle::NULL,
            block_end_handle: Handle::NULL,
            name: name.into(),
            flags: BlockFlags::new(),
            layout: Handle::NULL,
            units: 0,
            explodable: true,
            scale_uniformly: false,
            entities: Vec::new(),
        }
    }

    /// Create the model space block record
    pub fn model_space() -> Self {
        BlockRecord {
            handle: Handle::NULL,
            block_entity_handle: Handle::NULL,
            block_end_handle: Handle::NULL,
            name: "*Model_Space".to_string(),
            flags: BlockFlags::new(),
            layout: Handle::NULL,
            units: 0,
            explodable: true,
            scale_uniformly: false,
            entities: Vec::new(),
        }
    }

    /// Create the paper space block record
    pub fn paper_space() -> Self {
        BlockRecord {
            handle: Handle::NULL,
            block_entity_handle: Handle::NULL,
            block_end_handle: Handle::NULL,
            name: "*Paper_Space".to_string(),
            flags: BlockFlags::new(),
            layout: Handle::NULL,
            units: 0,
            explodable: true,
            scale_uniformly: false,
            entities: Vec::new(),
        }
    }

    /// Check if this is a model space block
    pub fn is_model_space(&self) -> bool {
        self.name == "*Model_Space"
    }

    /// Check if this is a paper space block
    pub fn is_paper_space(&self) -> bool {
        self.name.starts_with("*Paper_Space")
    }

    /// Check if this is a layout block
    pub fn is_layout(&self) -> bool {
        !self.layout.is_null()
    }

    /// Check if this block is anonymous
    pub fn is_anonymous(&self) -> bool {
        self.flags.anonymous || self.name.starts_with('*')
    }
}

impl TableEntry for BlockRecord {
    fn handle(&self) -> Handle {
        self.handle
    }

    fn set_handle(&mut self, handle: Handle) {
        self.handle = handle;
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn set_name(&mut self, name: String) {
        self.name = name;
    }

    fn is_standard(&self) -> bool {
        self.is_model_space() || self.is_paper_space()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_record_creation() {
        let block = BlockRecord::new("MyBlock");
        assert_eq!(block.name, "MyBlock");
        assert!(block.explodable);
    }

    #[test]
    fn test_model_space() {
        let block = BlockRecord::model_space();
        assert!(block.is_model_space());
        assert!(block.is_standard());
        assert!(!block.is_paper_space());
    }

    #[test]
    fn test_paper_space() {
        let block = BlockRecord::paper_space();
        assert!(block.is_paper_space());
        assert!(block.is_standard());
        assert!(!block.is_model_space());
    }
}

