//! Table types and the generic [`Table`] container.
//!
//! Tables store named, reusable definitions that entities reference:
//!
//! | Type | Purpose |
//! |------|----------|
//! | [`Layer`] | Drawing layers (color, linetype, visibility) |
//! | [`LineType`] | Dash patterns |
//! | [`TextStyle`] | Font / text formatting |
//! | [`DimStyle`] | Dimension appearance |
//! | [`BlockRecord`] | Block definition registry |
//! | [`AppId`] | Application identifier (XData) |
//! | [`View`] | Named view configurations |
//! | [`VPort`] | Viewport configurations |
//! | [`Ucs`] | User coordinate systems |

use crate::types::Handle;
use indexmap::IndexMap;

/// Normalize a symbol-table name for case-insensitive lookup.
pub fn normalize_name(name: &str) -> String {
    name.to_uppercase()
}

/// Whether a stored lookup key still describes `name`.
///
/// `add_allow_duplicate` keys a second entry sharing a display name as
/// `NAME\0…`, so that suffixed form counts as matching too.
fn key_matches(key: &str, name: &str) -> bool {
    match key.strip_prefix(normalize_name(name).as_str()) {
        Some("") => true,
        Some(rest) => rest.starts_with('\u{0}'),
        None => false,
    }
}

pub mod appid;
pub mod block_record;
pub mod dimstyle;
pub mod layer;
pub mod linetype;
pub mod textstyle;
pub mod ucs;
pub mod view;
pub mod vport;
pub mod vx;

pub use appid::AppId;
pub use block_record::BlockRecord;
pub use dimstyle::DimStyle;
pub use layer::{Layer, LayerFlags};
pub use linetype::{LineType, LineTypeComplexContent, LineTypeComplexData, LineTypeElement};
pub use textstyle::{TextGenerationFlags, TextStyle};
pub use ucs::Ucs;
pub use view::View;
pub use vport::VPort;
pub use vx::VxTableRecord;

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
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
        let name = normalize_name(entry.name());
        if self.entries.contains_key(&name) {
            return Err(format!("Entry '{}' already exists in table", entry.name()));
        }
        self.entries.insert(name, entry);
        Ok(())
    }

    /// Add or replace an entry in the table (parsed data wins over defaults)
    pub fn add_or_replace(&mut self, entry: T) {
        let name = normalize_name(entry.name());
        self.entries.insert(name, entry);
    }

    /// Add an entry while preserving existing entries with the same display
    /// name. This is needed for VPORT tables, where tiled model-space
    /// viewports can all be named "*Active".
    pub fn add_allow_duplicate(&mut self, entry: T) {
        let name = normalize_name(entry.name());
        if !self.entries.contains_key(&name) {
            self.entries.insert(name, entry);
            return;
        }

        let handle = entry.handle();
        let mut key = if handle.is_valid() {
            format!("{}\u{0}{:X}", name, handle.value())
        } else {
            format!("{}\u{0}{}", name, self.entries.len())
        };
        let mut n = 1usize;
        while self.entries.contains_key(&key) {
            key = format!("{}\u{0}{}-{}", name, handle.value(), n);
            n += 1;
        }
        self.entries.insert(key, entry);
    }

    /// Get an entry by name (case-insensitive)
    pub fn get(&self, name: &str) -> Option<&T> {
        self.entries.get(&normalize_name(name))
    }

    /// Get a mutable entry by name (case-insensitive)
    pub fn get_mut(&mut self, name: &str) -> Option<&mut T> {
        self.entries.get_mut(&normalize_name(name))
    }

    /// Remove an entry by name (case-insensitive)
    pub fn remove(&mut self, name: &str) -> Option<T> {
        self.entries.shift_remove(&normalize_name(name))
    }

    /// Check if an entry exists (case-insensitive)
    pub fn contains(&self, name: &str) -> bool {
        self.entries.contains_key(&normalize_name(name))
    }

    /// Rename an entry without changing its handle or table position.
    pub fn rename(&mut self, old_name: &str, new_name: impl Into<String>) -> Result<(), String> {
        let new_name = new_name.into();
        let old_key = normalize_name(old_name);
        let new_key = normalize_name(&new_name);
        let Some(index) = self.entries.get_index_of(&old_key) else {
            return Err(format!("Entry '{old_name}' does not exist in table"));
        };
        if old_key != new_key && self.entries.contains_key(&new_key) {
            return Err(format!("Entry '{new_name}' already exists in table"));
        }
        if old_key == new_key {
            self.entries[index].set_name(new_name);
            return Ok(());
        }
        let (_, _, mut entry) = self.entries.shift_remove_full(&old_key).unwrap();
        entry.set_name(new_name);
        let replaced = self.entries.shift_insert(index, new_key, entry);
        debug_assert!(replaced.is_none());
        Ok(())
    }

    /// Whether any entry was renamed in place and so is no longer reachable
    /// under its own name. See [`resync_keys`](Self::resync_keys).
    pub fn has_stale_keys(&self) -> bool {
        self.entries
            .iter()
            .any(|(key, entry)| !key_matches(key, entry.name()))
    }

    /// Re-key any entry whose name was changed in place, e.g. through
    /// [`iter_mut`](Self::iter_mut) or [`get_mut`](Self::get_mut).
    ///
    /// Lookups are keyed by the normalized name captured at insertion, so
    /// assigning `entry.name` directly leaves the entry reachable only under
    /// its old name. Every name-based resolution then misses: the DWG/DXF
    /// writers turn an unresolved entity layer into a NULL hard pointer and
    /// produce an invalid drawing. A rename colliding with another
    /// entry keeps the suffixed form `add_allow_duplicate` uses, so no entry is
    /// ever dropped.
    ///
    /// Order is preserved; this is a no-op when every key already agrees.
    /// Returns one `(old_key, new_name)` pair per re-keyed entry, so a caller
    /// can update name-based references to the entries that moved.
    pub fn resync_keys(&mut self) -> Vec<(String, String)> {
        let stale: Vec<usize> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, (key, entry))| !key_matches(key, entry.name()))
            .map(|(index, _)| index)
            .collect();
        if stale.is_empty() {
            return Vec::new();
        }
        let mut renames = Vec::with_capacity(stale.len());
        // Re-insert at the original index so table order — which DWG readers
        // rely on for the symbol-table record sequence — is untouched.
        for index in stale.iter().copied() {
            let (key, _) = self.entries.get_index(index).expect("index in range");
            let key = key.clone();
            let (_, _, entry) = self.entries.shift_remove_full(&key).expect("key present");
            let new_name = entry.name().to_string();
            let name = normalize_name(&new_name);
            let target = if self.entries.contains_key(&name) {
                let handle = entry.handle();
                let mut candidate = format!("{}\u{0}{:X}", name, handle.value());
                let mut n = 1usize;
                while self.entries.contains_key(&candidate) {
                    candidate = format!("{}\u{0}{:X}-{}", name, handle.value(), n);
                    n += 1;
                }
                candidate
            } else {
                name
            };
            self.entries.shift_insert(index, target, entry);
            renames.push((key, new_name));
        }
        renames
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
    fn test_table_allow_duplicate_entries() {
        let mut table = Table::new();
        let entry1 = MockEntry {
            handle: Handle::new(1),
            name: "*Active".to_string(),
        };
        let entry2 = MockEntry {
            handle: Handle::new(2),
            name: "*Active".to_string(),
        };

        table.add_allow_duplicate(entry1);
        table.add_allow_duplicate(entry2);

        assert_eq!(table.len(), 2);
        assert_eq!(
            table.names().collect::<Vec<_>>(),
            vec!["*Active", "*Active"]
        );
        assert_eq!(table.get("*active").unwrap().handle(), Handle::new(1));
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

    #[test]
    fn test_table_rename_preserves_entry_and_order() {
        let mut table = Table::new();
        for (handle, name) in [(1, "First"), (2, "Test"), (3, "Last")] {
            table
                .add(MockEntry {
                    handle: Handle::new(handle),
                    name: name.to_string(),
                })
                .unwrap();
        }

        table.rename("Test", "Renamed").unwrap();

        assert_eq!(
            table.names().collect::<Vec<_>>(),
            vec!["First", "Renamed", "Last"]
        );
        assert_eq!(table.get("renamed").unwrap().handle(), Handle::new(2));
    }

    #[test]
    fn test_table_rename_allows_unicode_case_change() {
        let mut table = Table::new();
        table
            .add(MockEntry {
                handle: Handle::new(1),
                name: "é".to_string(),
            })
            .unwrap();

        table.rename("é", "É").unwrap();

        assert_eq!(table.get("é").unwrap().name(), "É");
    }

    #[test]
    fn test_table_rename_errors_do_not_mutate() {
        let mut table = Table::new();
        for (handle, name) in [(1, "First"), (2, "Second")] {
            table
                .add(MockEntry {
                    handle: Handle::new(handle),
                    name: name.to_string(),
                })
                .unwrap();
        }

        assert!(table.rename("First", "Second").is_err());
        assert!(table.rename("Missing", "Renamed").is_err());
        assert_eq!(table.names().collect::<Vec<_>>(), vec!["First", "Second"]);
    }
}
