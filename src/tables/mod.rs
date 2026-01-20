//! CAD table types and management

use crate::types::Handle;
use indexmap::IndexMap;

pub mod layer;
pub mod linetype;
pub mod textstyle;
pub mod block_record;
pub mod dimstyle;
pub mod appid;
pub mod view;
pub mod vport;
pub mod ucs;

pub use layer::{Layer, LayerFlags};
pub use linetype::{LineType, LineTypeElement};
pub use textstyle::{TextStyle, TextGenerationFlags};
pub use block_record::BlockRecord;
pub use dimstyle::DimStyle;
pub use appid::AppId;
pub use view::View;
pub use vport::VPort;
pub use ucs::Ucs;

/// Base trait for all table entries
pub trait TableEntry {
    /// Get the entry's unique handle
    fn handle(&self) -> Handle;

    /// Set the entry's handle
    fn set_handle(&mut self, handle: Handle);

    /// Get the entry's name
    fn name(&self) -> &str;

    /// Set the entry's name
    fn set_name(&mut self, name: String);

    /// Check if this is a standard/default entry
    fn is_standard(&self) -> bool {
        false
    }
}

/// Generic table for storing named entries
#[derive(Debug, Clone)]
pub struct Table<T: TableEntry> {
    /// Entries stored by name (case-insensitive)
    entries: IndexMap<String, T>,
    /// Table handle
    handle: Handle,
}

impl<T: TableEntry> Table<T> {
    /// Create a new empty table
    pub fn new() -> Self {
        Table {
            entries: IndexMap::new(),
            handle: Handle::NULL,
        }
    }

    /// Create a table with a specific handle
    pub fn with_handle(handle: Handle) -> Self {
        Table {
            entries: IndexMap::new(),
            handle,
        }
    }

    /// Get the table's handle
    pub fn handle(&self) -> Handle {
        self.handle
    }

    /// Set the table's handle
    pub fn set_handle(&mut self, handle: Handle) {
        self.handle = handle;
    }

    /// Add an entry to the table
    pub fn add(&mut self, entry: T) -> Result<(), String> {
        let name = entry.name().to_uppercase();
        if self.entries.contains_key(&name) {
            return Err(format!("Entry '{}' already exists in table", entry.name()));
        }
        self.entries.insert(name, entry);
        Ok(())
    }

    /// Get an entry by name (case-insensitive)
    pub fn get(&self, name: &str) -> Option<&T> {
        self.entries.get(&name.to_uppercase())
    }

    /// Get a mutable entry by name (case-insensitive)
    pub fn get_mut(&mut self, name: &str) -> Option<&mut T> {
        self.entries.get_mut(&name.to_uppercase())
    }

    /// Remove an entry by name (case-insensitive)
    pub fn remove(&mut self, name: &str) -> Option<T> {
        self.entries.shift_remove(&name.to_uppercase())
    }

    /// Check if an entry exists (case-insensitive)
    pub fn contains(&self, name: &str) -> bool {
        self.entries.contains_key(&name.to_uppercase())
    }

    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the table is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over all entries
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.entries.values()
    }

    /// Iterate over all entries mutably
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.entries.values_mut()
    }

    /// Get all entry names
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.values().map(|e| e.name())
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl<T: TableEntry> Default for Table<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock table entry for testing
    #[derive(Debug, Clone)]
    struct MockEntry {
        handle: Handle,
        name: String,
    }

    impl TableEntry for MockEntry {
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
    }

    #[test]
    fn test_table_add_and_get() {
        let mut table = Table::new();
        let entry = MockEntry {
            handle: Handle::new(1),
            name: "Test".to_string(),
        };
        
        assert!(table.add(entry).is_ok());
        assert!(table.contains("Test"));
        assert!(table.contains("test")); // Case-insensitive
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn test_table_duplicate_entry() {
        let mut table = Table::new();
        let entry1 = MockEntry {
            handle: Handle::new(1),
            name: "Test".to_string(),
        };
        let entry2 = MockEntry {
            handle: Handle::new(2),
            name: "test".to_string(), // Same name, different case
        };
        
        assert!(table.add(entry1).is_ok());
        assert!(table.add(entry2).is_err()); // Should fail
    }

    #[test]
    fn test_table_remove() {
        let mut table = Table::new();
        let entry = MockEntry {
            handle: Handle::new(1),
            name: "Test".to_string(),
        };
        
        table.add(entry).unwrap();
        assert_eq!(table.len(), 1);
        
        let removed = table.remove("test");
        assert!(removed.is_some());
        assert_eq!(table.len(), 0);
    }
}

