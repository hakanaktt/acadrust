//! XRecord object - Extended record storage for arbitrary data

use crate::types::Handle;

/// Dictionary cloning behavior flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DictionaryCloningFlags {
    /// Not applicable
    #[default]
    NotApplicable = 0,
    /// Keep existing record
    KeepExisting = 1,
    /// Use clone
    UseClone = 2,
    /// XRef name-based cloning
    XrefName = 3,
    /// Name-based cloning
    Name = 4,
    /// Unmangle name
    UnmangleName = 5,
}

impl DictionaryCloningFlags {
    /// Create from DXF value
    pub fn from_value(value: i16) -> Self {
        match value {
            1 => DictionaryCloningFlags::KeepExisting,
            2 => DictionaryCloningFlags::UseClone,
            3 => DictionaryCloningFlags::XrefName,
            4 => DictionaryCloningFlags::Name,
            5 => DictionaryCloningFlags::UnmangleName,
            _ => DictionaryCloningFlags::NotApplicable,
        }
    }

    /// Convert to DXF value
    pub fn to_value(&self) -> i16 {
        *self as i16
    }

    /// Convert to DXF code (alias for to_value)
    pub fn to_code(&self) -> i16 {
        self.to_value()
    }
}

/// Group code value type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XRecordValueType {
    /// String value
    String,
    /// 3D point
    Point3D,
    /// Double value
    Double,
    /// Byte value
    Byte,
    /// 16-bit integer
    Int16,
    /// 32-bit integer
    Int32,
    /// 64-bit integer
    Int64,
    /// Handle value
    Handle,
    /// Object ID (handle reference)
    ObjectId,
    /// Boolean value
    Bool,
    /// Binary chunk
    Chunk,
    /// Unknown type
    Unknown,
}

impl XRecordValueType {
    /// Determine value type from DXF group code
    pub fn from_code(code: i32) -> Self {
        match code {
            // Handle references - check specific codes first
            5 | 105 => XRecordValueType::Handle,
            320..=329 | 480..=481 => XRecordValueType::Handle,
            330..=369 => XRecordValueType::ObjectId,
            // Strings (0-9 but not 5, plus 100-102, 300-309)
            0..=4 | 6..=9 | 100..=102 | 300..=309 => XRecordValueType::String,
            // 3D points
            10..=39 => XRecordValueType::Point3D,
            // Doubles
            40..=59 | 110..=149 | 210..=239 | 460..=469 => XRecordValueType::Double,
            // Bytes
            280..=289 => XRecordValueType::Byte,
            // 16-bit integers
            60..=79 | 170..=179 | 270..=279 => XRecordValueType::Int16,
            // 32-bit integers
            90..=99 | 420..=459 => XRecordValueType::Int32,
            // 64-bit integers
            160..=169 => XRecordValueType::Int64,
            // Booleans
            290..=299 => XRecordValueType::Bool,
            // Binary chunks
            310..=319 => XRecordValueType::Chunk,
            // Unknown
            _ => XRecordValueType::Unknown,
        }
    }

    /// Check if this type represents a handle/reference
    pub fn is_handle(&self) -> bool {
        matches!(self, XRecordValueType::Handle | XRecordValueType::ObjectId)
    }
}

/// XRecord entry value
#[derive(Debug, Clone, PartialEq)]
pub enum XRecordValue {
    /// String value
    String(String),
    /// Double value
    Double(f64),
    /// 16-bit integer
    Int16(i16),
    /// 32-bit integer
    Int32(i32),
    /// 64-bit integer
    Int64(i64),
    /// Byte value
    Byte(u8),
    /// Boolean value
    Bool(bool),
    /// Handle/Object reference
    Handle(Handle),
    /// 3D point (x, y, z)
    Point3D(f64, f64, f64),
    /// Binary data chunk
    Chunk(Vec<u8>),
}

impl XRecordValue {
    /// Get as string if this is a string value
    pub fn as_string(&self) -> Option<&str> {
        match self {
            XRecordValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Get as f64 if this is a double value
    pub fn as_double(&self) -> Option<f64> {
        match self {
            XRecordValue::Double(v) => Some(*v),
            _ => None,
        }
    }

    /// Get as i32 if this is an integer value
    pub fn as_i32(&self) -> Option<i32> {
        match self {
            XRecordValue::Int32(v) => Some(*v),
            XRecordValue::Int16(v) => Some(*v as i32),
            XRecordValue::Byte(v) => Some(*v as i32),
            _ => None,
        }
    }

    /// Get as handle if this is a handle value
    pub fn as_handle(&self) -> Option<Handle> {
        match self {
            XRecordValue::Handle(h) => Some(*h),
            _ => None,
        }
    }

    /// Get as bool if this is a boolean value
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            XRecordValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Get as 3D point if this is a point value
    pub fn as_point3d(&self) -> Option<(f64, f64, f64)> {
        match self {
            XRecordValue::Point3D(x, y, z) => Some((*x, *y, *z)),
            _ => None,
        }
    }
}

/// XRecord entry with group code and value
#[derive(Debug, Clone, PartialEq)]
pub struct XRecordEntry {
    /// DXF group code (1-369, except 5 and 105)
    pub code: i32,
    /// The stored value
    pub value: XRecordValue,
}

impl XRecordEntry {
    /// Create a new entry
    pub fn new(code: i32, value: XRecordValue) -> Self {
        Self { code, value }
    }

    /// Create a string entry
    pub fn string(code: i32, value: impl Into<String>) -> Self {
        Self::new(code, XRecordValue::String(value.into()))
    }

    /// Create a double entry
    pub fn double(code: i32, value: f64) -> Self {
        Self::new(code, XRecordValue::Double(value))
    }

    /// Create an i16 entry
    pub fn int16(code: i32, value: i16) -> Self {
        Self::new(code, XRecordValue::Int16(value))
    }

    /// Create an i32 entry
    pub fn int32(code: i32, value: i32) -> Self {
        Self::new(code, XRecordValue::Int32(value))
    }

    /// Create a handle entry
    pub fn handle(code: i32, value: Handle) -> Self {
        Self::new(code, XRecordValue::Handle(value))
    }

    /// Create a bool entry
    pub fn bool(code: i32, value: bool) -> Self {
        Self::new(code, XRecordValue::Bool(value))
    }

    /// Create a point entry
    pub fn point3d(x_code: i32, x: f64, y: f64, z: f64) -> Self {
        Self::new(x_code, XRecordValue::Point3D(x, y, z))
    }

    /// Get the value type for this entry
    pub fn value_type(&self) -> XRecordValueType {
        XRecordValueType::from_code(self.code)
    }

    /// Check if this entry contains a linked object reference
    pub fn has_linked_object(&self) -> bool {
        matches!(self.value, XRecordValue::Handle(_))
    }
}

/// XRecord object - stores arbitrary extended data
///
/// XRecords can store any DXF group code/value pairs and are commonly
/// used by applications to store custom data in DXF/DWG files.
///
/// # Example
/// ```ignore
/// use acadrust::objects::XRecord;
///
/// let mut xrecord = XRecord::new();
/// xrecord.add_string(1, "Custom Data");
/// xrecord.add_double(40, 3.14159);
/// xrecord.add_int32(90, 42);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct XRecord {
    /// Unique handle
    pub handle: Handle,
    /// Owner handle
    pub owner: Handle,
    /// Record name (optional, for named XRecords)
    pub name: String,
    /// Cloning behavior flags
    pub cloning_flags: DictionaryCloningFlags,
    /// Collection of data entries
    pub entries: Vec<XRecordEntry>,
}

impl XRecord {
    /// Object type name
    pub const OBJECT_TYPE: &'static str = "XRECORD";

    /// Create a new empty XRecord
    pub fn new() -> Self {
        Self {
            handle: Handle::NULL,
            owner: Handle::NULL,
            name: String::new(),
            cloning_flags: DictionaryCloningFlags::NotApplicable,
            entries: Vec::new(),
        }
    }

    /// Create a named XRecord
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::new()
        }
    }

    /// Add an entry to the record
    pub fn add_entry(&mut self, entry: XRecordEntry) {
        self.entries.push(entry);
    }

    /// Create and add a string entry
    pub fn add_string(&mut self, code: i32, value: impl Into<String>) {
        self.entries.push(XRecordEntry::string(code, value));
    }

    /// Create and add a double entry
    pub fn add_double(&mut self, code: i32, value: f64) {
        self.entries.push(XRecordEntry::double(code, value));
    }

    /// Create and add an i16 entry
    pub fn add_int16(&mut self, code: i32, value: i16) {
        self.entries.push(XRecordEntry::int16(code, value));
    }

    /// Create and add an i32 entry
    pub fn add_int32(&mut self, code: i32, value: i32) {
        self.entries.push(XRecordEntry::int32(code, value));
    }

    /// Create and add a handle entry
    pub fn add_handle(&mut self, code: i32, value: Handle) {
        self.entries.push(XRecordEntry::handle(code, value));
    }

    /// Create and add a bool entry
    pub fn add_bool(&mut self, code: i32, value: bool) {
        self.entries.push(XRecordEntry::bool(code, value));
    }

    /// Create and add a point entry
    pub fn add_point3d(&mut self, x_code: i32, x: f64, y: f64, z: f64) {
        self.entries.push(XRecordEntry::point3d(x_code, x, y, z));
    }

    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the record is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get an entry by index
    pub fn get(&self, index: usize) -> Option<&XRecordEntry> {
        self.entries.get(index)
    }

    /// Get all entries with a specific code
    pub fn get_by_code(&self, code: i32) -> Vec<&XRecordEntry> {
        self.entries.iter().filter(|e| e.code == code).collect()
    }

    /// Get the first entry with a specific code
    pub fn get_first_by_code(&self, code: i32) -> Option<&XRecordEntry> {
        self.entries.iter().find(|e| e.code == code)
    }

    /// Get all string values with a specific code
    pub fn get_strings(&self, code: i32) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|e| e.code == code)
            .filter_map(|e| e.value.as_string())
            .collect()
    }

    /// Get the first string value with a specific code
    pub fn get_string(&self, code: i32) -> Option<&str> {
        self.get_first_by_code(code)?.value.as_string()
    }

    /// Get the first double value with a specific code
    pub fn get_double(&self, code: i32) -> Option<f64> {
        self.get_first_by_code(code)?.value.as_double()
    }

    /// Get the first i32 value with a specific code
    pub fn get_i32(&self, code: i32) -> Option<i32> {
        self.get_first_by_code(code)?.value.as_i32()
    }

    /// Remove all entries with a specific code
    pub fn remove_by_code(&mut self, code: i32) {
        self.entries.retain(|e| e.code != code);
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Get all referenced handles
    pub fn get_references(&self) -> Vec<Handle> {
        self.entries
            .iter()
            .filter_map(|e| e.value.as_handle())
            .collect()
    }

    /// Iterate over entries
    pub fn iter(&self) -> impl Iterator<Item = &XRecordEntry> {
        self.entries.iter()
    }
}

impl Default for XRecord {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xrecord_creation() {
        let xrecord = XRecord::new();
        assert!(xrecord.is_empty());
        assert_eq!(xrecord.cloning_flags, DictionaryCloningFlags::NotApplicable);
    }

    #[test]
    fn test_xrecord_named() {
        let xrecord = XRecord::named("MyRecord");
        assert_eq!(xrecord.name, "MyRecord");
    }

    #[test]
    fn test_xrecord_add_entries() {
        let mut xrecord = XRecord::new();
        xrecord.add_string(1, "Test");
        xrecord.add_double(40, 3.14);
        xrecord.add_int32(90, 42);
        xrecord.add_bool(290, true);

        assert_eq!(xrecord.len(), 4);
    }

    #[test]
    fn test_xrecord_get_values() {
        let mut xrecord = XRecord::new();
        xrecord.add_string(1, "Hello");
        xrecord.add_double(40, 2.5);
        xrecord.add_int32(90, 100);

        assert_eq!(xrecord.get_string(1), Some("Hello"));
        assert_eq!(xrecord.get_double(40), Some(2.5));
        assert_eq!(xrecord.get_i32(90), Some(100));
        assert_eq!(xrecord.get_string(999), None);
    }

    #[test]
    fn test_xrecord_get_by_code() {
        let mut xrecord = XRecord::new();
        xrecord.add_string(1, "First");
        xrecord.add_string(1, "Second");
        xrecord.add_string(2, "Other");

        let code_1 = xrecord.get_by_code(1);
        assert_eq!(code_1.len(), 2);

        let strings = xrecord.get_strings(1);
        assert_eq!(strings, vec!["First", "Second"]);
    }

    #[test]
    fn test_xrecord_remove_by_code() {
        let mut xrecord = XRecord::new();
        xrecord.add_string(1, "Keep");
        xrecord.add_double(40, 1.0);
        xrecord.add_double(40, 2.0);

        xrecord.remove_by_code(40);
        assert_eq!(xrecord.len(), 1);
        assert_eq!(xrecord.get_string(1), Some("Keep"));
    }

    #[test]
    fn test_xrecord_entry_types() {
        let entry = XRecordEntry::string(1, "test");
        assert_eq!(entry.value_type(), XRecordValueType::String);

        let entry = XRecordEntry::double(40, 1.0);
        assert_eq!(entry.value_type(), XRecordValueType::Double);

        let entry = XRecordEntry::int32(90, 42);
        assert_eq!(entry.value_type(), XRecordValueType::Int32);

        let entry = XRecordEntry::handle(330, Handle::new(100));
        assert_eq!(entry.value_type(), XRecordValueType::ObjectId);
        assert!(entry.has_linked_object());
    }

    #[test]
    fn test_xrecord_point3d() {
        let mut xrecord = XRecord::new();
        xrecord.add_point3d(10, 1.0, 2.0, 3.0);

        let entry = xrecord.get(0).unwrap();
        assert_eq!(entry.value.as_point3d(), Some((1.0, 2.0, 3.0)));
    }

    #[test]
    fn test_xrecord_get_references() {
        let mut xrecord = XRecord::new();
        xrecord.add_handle(330, Handle::new(100));
        xrecord.add_string(1, "text");
        xrecord.add_handle(340, Handle::new(200));

        let refs = xrecord.get_references();
        assert_eq!(refs.len(), 2);
        assert!(refs.contains(&Handle::new(100)));
        assert!(refs.contains(&Handle::new(200)));
    }

    #[test]
    fn test_cloning_flags() {
        assert_eq!(DictionaryCloningFlags::from_value(0), DictionaryCloningFlags::NotApplicable);
        assert_eq!(DictionaryCloningFlags::from_value(1), DictionaryCloningFlags::KeepExisting);
        assert_eq!(DictionaryCloningFlags::from_value(2), DictionaryCloningFlags::UseClone);
        assert_eq!(DictionaryCloningFlags::KeepExisting.to_value(), 1);
    }

    #[test]
    fn test_value_type_from_code() {
        assert_eq!(XRecordValueType::from_code(1), XRecordValueType::String);
        assert_eq!(XRecordValueType::from_code(10), XRecordValueType::Point3D);
        assert_eq!(XRecordValueType::from_code(40), XRecordValueType::Double);
        assert_eq!(XRecordValueType::from_code(90), XRecordValueType::Int32);
        assert_eq!(XRecordValueType::from_code(290), XRecordValueType::Bool);
        assert_eq!(XRecordValueType::from_code(330), XRecordValueType::ObjectId);
    }
}

