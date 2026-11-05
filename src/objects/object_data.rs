//! AutoCAD Map 3D Object Data: typed attribute tables whose records Map 3D
//! and Civil 3D show in an entity's properties.
//!
//! Three classes of application `"IRD"` hold it. `IRD_DSC_DICT`, under the
//! named-object dictionary's `IrdDscDictionary` key, lists every type and
//! table as an `IRD_DSC_RECORD`. An entity's `IRD_OBJ_RECORD` belongs to its
//! extension dictionary and points at its table. Descriptors and records keep
//! their payload as stored, the same bytes in DXF and DWG; [`ObjectDataTable`]
//! and [`ObjectDataValue`] read and build it.

use crate::document::CadDocument;
use crate::error::{DxfError, Result};
use crate::objects::{ClassObject, ClassObjectData, ObjectType};
use crate::types::Handle;

/// The key of the type and table dictionary in the named-object dictionary.
pub const OBJECT_DATA_DICTIONARY_KEY: &str = "IrdDscDictionary";

/// `IRD_DSC_DICT`: the drawing's types and tables.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ObjectDataDictionary {
    pub entries: Vec<(String, Handle)>,
    pub duplicate_cloning: i16,
    pub hard_owner: bool,
}

impl Default for ObjectDataDictionary {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            duplicate_cloning: 1,
            hard_owner: false,
        }
    }
}

/// `IRD_DSC_RECORD`: a built-in type or a table.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ObjectDataDescriptor {
    /// The descriptor's kind, which its payload also opens with.
    pub kind: i32,
    /// `<table>-_-ade` for a table, `<type>-_-Ird` for a built-in type.
    pub name: String,
    pub payload: Vec<u8>,
}

/// `IRD_OBJ_RECORD`: one entity's values for one table.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ObjectDataRecord {
    /// The table's `IRD_DSC_RECORD`.
    pub table: Handle,
    pub payload: Vec<u8>,
}

/// The type of a table's field.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ObjectDataType {
    Real,
    Integer,
    Character,
    /// A type this crate does not model, by its Map class name.
    Other(String),
}

impl ObjectDataType {
    fn class_name(&self) -> &str {
        match self {
            Self::Real => "ClassReal",
            Self::Integer => "ClassLong",
            Self::Character => "ClassChar",
            Self::Other(name) => name,
        }
    }

    fn from_class_name(name: String) -> Self {
        match name.as_str() {
            "ClassReal" => Self::Real,
            "ClassLong" => Self::Integer,
            "ClassChar" => Self::Character,
            _ => Self::Other(name),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ObjectDataField {
    pub name: String,
    pub data_type: ObjectDataType,
    pub description: String,
}

impl ObjectDataField {
    pub fn new(name: impl Into<String>, data_type: ObjectDataType) -> Self {
        Self {
            name: name.into(),
            data_type,
            description: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ObjectDataTable {
    pub name: String,
    pub description: String,
    pub fields: Vec<ObjectDataField>,
}

impl ObjectDataTable {
    pub fn new(name: impl Into<String>, fields: Vec<ObjectDataField>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            fields,
        }
    }
}

/// A record's value. Object Data has no null.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ObjectDataValue {
    Real(f64),
    Integer(i32),
    Character(String),
}

const KIND_TABLE: i32 = 0x3000;
const BUILT_IN_VERSION: &str = "Ird Class V 1.02";
const FIELD_MARKER: u32 = 0x8000_0000;

/// The types every table's fields derive from, as Map 3D writes them.
const BUILT_IN_TYPES: [(i32, &str, &str, &str); 5] = [
    (0x1501, "Class", "", "The base superclass"),
    (
        0x1701,
        "ClassNumeric",
        "Class",
        "The base class for the numerics",
    ),
    (0x1601, "ClassReal", "ClassNumeric", "The real value"),
    (0x1601, "ClassLong", "ClassNumeric", "The long value"),
    (0x1601, "ClassChar", "Class", "The character value"),
];

impl ObjectDataDescriptor {
    /// The descriptor of a table.
    pub fn table(table: &ObjectDataTable) -> Self {
        let mut payload = descriptor_header(
            KIND_TABLE,
            [&table.name, "ade", "Class", "Ird", &table.description, "1"],
        );
        for (index, field) in table.fields.iter().enumerate() {
            let count = if index == 0 {
                table.fields.len() as u32
            } else {
                0
            };
            payload.extend((FIELD_MARKER | count).to_le_bytes());
            payload.extend(0u16.to_le_bytes());
            for text in [
                &field.name,
                field.data_type.class_name(),
                "Ird",
                &field.description,
            ] {
                push_string(&mut payload, text);
            }
            payload.extend(0u16.to_le_bytes());
        }
        payload.extend(0u32.to_le_bytes());
        Self {
            kind: KIND_TABLE,
            name: format!("{}-_-ade", table.name),
            payload,
        }
    }

    fn built_in((kind, name, parent, description): (i32, &str, &str, &str)) -> Self {
        Self {
            kind,
            name: format!("{name}-_-Ird"),
            payload: {
                let mut payload = descriptor_header(
                    kind,
                    [name, "Ird", parent, "Ird", description, BUILT_IN_VERSION],
                );
                payload.extend(0u32.to_le_bytes());
                payload
            },
        }
    }

    /// The table this descriptor defines, or `None` for a built-in type or a
    /// layout this crate does not read, such as a field with a default value.
    pub fn as_table(&self) -> Option<ObjectDataTable> {
        if self.kind != KIND_TABLE {
            return None;
        }
        let mut reader = PayloadReader::new(&self.payload);
        reader.u16()?;
        reader.u16()?;
        let name = reader.string()?;
        for _ in 0..3 {
            reader.string()?;
        }
        let description = reader.string()?;
        reader.string()?;
        let mut fields = Vec::new();
        while reader.u32()? & FIELD_MARKER != 0 {
            reader.u16()?;
            let name = reader.string()?;
            let data_type = ObjectDataType::from_class_name(reader.string()?);
            reader.string()?;
            let description = reader.string()?;
            if reader.u16()? != 0 {
                return None;
            }
            fields.push(ObjectDataField {
                name,
                data_type,
                description,
            });
        }
        Some(ObjectDataTable {
            name,
            description,
            fields,
        })
    }
}

impl ObjectDataRecord {
    /// A record of `values` in the table's field order.
    pub fn new(
        table: Handle,
        fields: &[ObjectDataField],
        values: &[ObjectDataValue],
    ) -> Result<Self> {
        if fields.len() != values.len() {
            return Err(DxfError::Custom(format!(
                "an Object Data record needs {} values, not {}",
                fields.len(),
                values.len()
            )));
        }
        let mut payload = 0u16.to_le_bytes().to_vec();
        for (field, value) in fields.iter().zip(values) {
            match (&field.data_type, value) {
                (ObjectDataType::Real, ObjectDataValue::Real(v)) => payload.extend(v.to_le_bytes()),
                (ObjectDataType::Integer, ObjectDataValue::Integer(v)) => {
                    payload.extend(v.to_le_bytes())
                }
                (ObjectDataType::Character, ObjectDataValue::Character(v)) => {
                    push_string(&mut payload, v)
                }
                _ => {
                    return Err(DxfError::Custom(format!(
                        "Object Data field {} can't hold {value:?}",
                        field.name
                    )))
                }
            }
        }
        Ok(Self { table, payload })
    }

    /// The record's values, read with its table's fields.
    pub fn values(&self, fields: &[ObjectDataField]) -> Option<Vec<ObjectDataValue>> {
        let mut reader = PayloadReader::new(&self.payload);
        reader.u16()?;
        fields
            .iter()
            .map(|field| match field.data_type {
                ObjectDataType::Real => reader.f64().map(ObjectDataValue::Real),
                ObjectDataType::Integer => reader.i32().map(ObjectDataValue::Integer),
                ObjectDataType::Character => reader.string().map(ObjectDataValue::Character),
                ObjectDataType::Other(_) => None,
            })
            .collect()
    }
}

fn descriptor_header(kind: i32, strings: [&str; 6]) -> Vec<u8> {
    let mut payload = (kind as u16).to_le_bytes().to_vec();
    payload.extend(0u16.to_le_bytes());
    for text in strings {
        push_string(&mut payload, text);
    }
    payload
}

/// UTF-16, NUL-terminated, so a NUL of its own is dropped.
fn push_string(payload: &mut Vec<u8>, text: &str) {
    for unit in text.encode_utf16().filter(|&unit| unit != 0) {
        payload.extend(unit.to_le_bytes());
    }
    payload.extend(0u16.to_le_bytes());
}

struct PayloadReader<'a> {
    bytes: &'a [u8],
}

impl<'a> PayloadReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let (head, rest) = self.bytes.split_first_chunk::<N>()?;
        self.bytes = rest;
        Some(*head)
    }

    fn u16(&mut self) -> Option<u16> {
        self.take().map(u16::from_le_bytes)
    }

    fn u32(&mut self) -> Option<u32> {
        self.take().map(u32::from_le_bytes)
    }

    fn i32(&mut self) -> Option<i32> {
        self.take().map(i32::from_le_bytes)
    }

    fn f64(&mut self) -> Option<f64> {
        self.take().map(f64::from_le_bytes)
    }

    fn string(&mut self) -> Option<String> {
        let mut units = Vec::new();
        loop {
            match self.u16()? {
                0 => return String::from_utf16(&units).ok(),
                unit => units.push(unit),
            }
        }
    }
}

/// An Object Data record attached to an entity.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectDataAttachment {
    /// The `IRD_OBJ_RECORD`.
    pub record: Handle,
    /// Its table's `IRD_DSC_RECORD`.
    pub table: Handle,
    pub values: Vec<ObjectDataValue>,
}

impl CadDocument {
    /// The drawing's Object Data tables in the order they were defined, by
    /// their descriptor's handle.
    pub fn object_data_tables(&self) -> Vec<(Handle, ObjectDataTable)> {
        let Some(types) = self
            .named_objects_dictionary()
            .and_then(|root| root.get(OBJECT_DATA_DICTIONARY_KEY))
            .and_then(|handle| self.object_data_types(handle))
        else {
            return Vec::new();
        };
        types
            .entries
            .iter()
            .filter_map(|(_, handle)| Some((*handle, self.object_data_table(*handle)?)))
            .collect()
    }

    /// Defines a table, creating the type dictionary on first use, and
    /// returns its descriptor's handle.
    pub fn add_object_data_table(&mut self, table: &ObjectDataTable) -> Result<Handle> {
        if self
            .object_data_tables()
            .iter()
            .any(|(_, existing)| existing.name.eq_ignore_ascii_case(&table.name))
        {
            return Err(DxfError::Custom(format!(
                "Object Data table {} already exists",
                table.name
            )));
        }
        let dictionary = self.object_data_dictionary()?;
        Ok(self.add_object_data_descriptor(dictionary, ObjectDataDescriptor::table(table)))
    }

    /// Attaches a record of `values`, in the table's field order, to an
    /// entity and returns the record's handle.
    pub fn attach_object_data(
        &mut self,
        entity: Handle,
        table: Handle,
        values: &[ObjectDataValue],
    ) -> Result<Handle> {
        let fields = self
            .object_data_table(table)
            .ok_or(DxfError::ObjectNotFound(table.value()))?
            .fields;
        let record = ObjectDataRecord::new(table, &fields, values)?;
        if self.get_entity(entity).is_none() {
            return Err(DxfError::ObjectNotFound(entity.value()));
        }

        let xdictionary = self.ensure_extension_dictionary(entity);
        let handle = self.allocate_handle();
        let Some(ObjectType::Dictionary(dictionary)) = self.objects.get_mut(&xdictionary) else {
            return Err(DxfError::ObjectNotFound(xdictionary.value()));
        };
        dictionary.duplicate_cloning = 1;
        dictionary.add_entry(next_key(&dictionary.entries), handle);
        self.objects.insert(
            handle,
            ObjectType::ClassObject(ClassObject {
                handle,
                owner: xdictionary,
                reactors: vec![xdictionary],
                xdictionary_handle: None,
                data: ClassObjectData::ObjectDataRecord(record),
            }),
        );
        Ok(handle)
    }

    /// An entity's Object Data records. A record whose table has a field type
    /// this crate does not read is left out.
    pub fn object_data(&self, entity: Handle) -> Vec<ObjectDataAttachment> {
        let Some(ObjectType::Dictionary(xdictionary)) = self
            .get_entity(entity)
            .and_then(|entity| entity.common().xdictionary_handle)
            .and_then(|handle| self.objects.get(&handle))
        else {
            return Vec::new();
        };
        xdictionary
            .entries
            .iter()
            .filter_map(|(_, handle)| match self.objects.get(handle)? {
                ObjectType::ClassObject(ClassObject {
                    data: ClassObjectData::ObjectDataRecord(record),
                    ..
                }) => Some(ObjectDataAttachment {
                    record: *handle,
                    table: record.table,
                    values: record.values(&self.object_data_table(record.table)?.fields)?,
                }),
                _ => None,
            })
            .collect()
    }

    fn object_data_table(&self, descriptor: Handle) -> Option<ObjectDataTable> {
        match self.objects.get(&descriptor)? {
            ObjectType::ClassObject(ClassObject {
                data: ClassObjectData::ObjectDataDescriptor(descriptor),
                ..
            }) => descriptor.as_table(),
            _ => None,
        }
    }

    fn object_data_types(&self, handle: Handle) -> Option<&ObjectDataDictionary> {
        match self.objects.get(&handle)? {
            ObjectType::ClassObject(ClassObject {
                data: ClassObjectData::ObjectDataDictionary(types),
                ..
            }) => Some(types),
            _ => None,
        }
    }

    fn object_data_dictionary(&mut self) -> Result<Handle> {
        let root = self.ensure_named_objects_dictionary();
        if let Some(handle) = self
            .named_objects_dictionary()
            .and_then(|root| root.get(OBJECT_DATA_DICTIONARY_KEY))
        {
            return match self.object_data_types(handle) {
                Some(_) => Ok(handle),
                None => Err(DxfError::Custom(format!(
                    "{OBJECT_DATA_DICTIONARY_KEY} is not an Object Data dictionary"
                ))),
            };
        }
        let handle = self.allocate_handle();
        self.objects.insert(
            handle,
            ObjectType::ClassObject(ClassObject {
                handle,
                owner: root,
                reactors: vec![root],
                xdictionary_handle: None,
                data: ClassObjectData::ObjectDataDictionary(ObjectDataDictionary::default()),
            }),
        );
        if let Some(root) = self.named_objects_dictionary_mut() {
            root.add_entry(OBJECT_DATA_DICTIONARY_KEY, handle);
        }
        for descriptor in built_in_descriptors() {
            self.add_object_data_descriptor(handle, descriptor);
        }
        Ok(handle)
    }

    fn add_object_data_descriptor(
        &mut self,
        dictionary: Handle,
        descriptor: ObjectDataDescriptor,
    ) -> Handle {
        let handle = self.allocate_handle();
        self.objects.insert(
            handle,
            ObjectType::ClassObject(ClassObject {
                handle,
                owner: dictionary,
                reactors: vec![dictionary],
                xdictionary_handle: None,
                data: ClassObjectData::ObjectDataDescriptor(descriptor),
            }),
        );
        if let Some(ObjectType::ClassObject(ClassObject {
            data: ClassObjectData::ObjectDataDictionary(types),
            ..
        })) = self.objects.get_mut(&dictionary)
        {
            let key = next_key(&types.entries);
            types.entries.push((key, handle));
        }
        handle
    }
}

/// The types every table's fields derive from, which Map 3D adds to a drawing
/// with its first table.
pub(crate) fn built_in_descriptors() -> impl Iterator<Item = ObjectDataDescriptor> {
    BUILT_IN_TYPES
        .into_iter()
        .map(ObjectDataDescriptor::built_in)
}

/// Map keys a dictionary's entries `*A1`, `*A2`, and so on, and doesn't
/// reuse a deleted entry's key.
fn next_key(entries: &[(String, Handle)]) -> String {
    let last = entries
        .iter()
        .filter_map(|(key, _)| key.strip_prefix("*A")?.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    format!("*A{}", last + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect()
    }

    /// Civil 3D 2027's DXF payloads for its built-in types.
    const BUILT_IN_PAYLOADS: [&str; 5] = [
        "0115000043006C0061007300730000004900720064000000000049007200640000005400680065002000620061007300650020007300750070006500720063006C006100730073000000490072006400200043006C0061007300730020005600200031002E0030003200000000000000",
        "0117000043006C006100730073004E0075006D0065007200690063000000490072006400000043006C006100730073000000490072006400000054006800650020006200610073006500200063006C00610073007300200066006F007200200074006800650020006E0075006D00650072006900630073000000490072006400200043006C0061007300730020005600200031002E0030003200000000000000",
        "0116000043006C006100730073005200650061006C000000490072006400000043006C006100730073004E0075006D0065007200690063000000490072006400000054006800650020007200650061006C002000760061006C00750065000000490072006400200043006C0061007300730020005600200031002E0030003200000000000000",
        "0116000043006C006100730073004C006F006E0067000000490072006400000043006C006100730073004E0075006D0065007200690063000000490072006400000054006800650020006C006F006E0067002000760061006C00750065000000490072006400200043006C0061007300730020005600200031002E0030003200000000000000",
        "0116000043006C0061007300730043006800610072000000490072006400000043006C006100730073000000490072006400000054006800650020006300680061007200610063007400650072002000760061006C00750065000000490072006400200043006C0061007300730020005600200031002E0030003200000000000000",
    ];

    fn test_table() -> ObjectDataTable {
        let field = |name: &str, data_type, description: &str| ObjectDataField {
            description: description.into(),
            ..ObjectDataField::new(name, data_type)
        };
        ObjectDataTable::new(
            "TEST",
            vec![
                field("VOLTAGE", ObjectDataType::Real, "the voltage"),
                field("CIRCUITS", ObjectDataType::Integer, "the circuit"),
                field("OWNER", ObjectDataType::Character, "the owner"),
                field("ENERGIZED", ObjectDataType::Integer, "is it energized"),
            ],
        )
    }

    #[test]
    fn built_in_types_match_map() {
        for (built_in, payload) in BUILT_IN_TYPES.into_iter().zip(BUILT_IN_PAYLOADS) {
            assert_eq!(
                ObjectDataDescriptor::built_in(built_in).payload,
                hex(payload)
            );
        }
    }

    /// A table Civil 3D 2027 defined and wrote to DXF.
    #[test]
    fn table_matches_map() {
        let descriptor = ObjectDataDescriptor::table(&test_table());
        assert_eq!(descriptor.name, "TEST-_-ade");
        assert_eq!(
            descriptor.payload,
            hex(concat!(
                "0030000054004500530054000000610064006500000043006C0061007300730000004900720064000000000031000000",
                "04000080000056004F004C005400410047004500000043006C006100730073005200650061006C000000490072006400",
                "0000740068006500200076006F006C007400610067006500000000000000008000004300490052004300550049005400",
                "5300000043006C006100730073004C006F006E0067000000490072006400000074006800650020006300690072006300",
                "750069007400000000000000008000004F0057004E0045005200000043006C0061007300730043006800610072000000",
                "490072006400000074006800650020006F0077006E00650072000000000000000080000045004E004500520047004900",
                "5A0045004400000043006C006100730073004C006F006E00670000004900720064000000690073002000690074002000",
                "65006E0065007200670069007A00650064000000000000000000",
            ))
        );
        assert_eq!(descriptor.as_table(), Some(test_table()));
    }

    #[test]
    fn record_matches_map() {
        let table = test_table();
        let values = vec![
            ObjectDataValue::Real(230.0),
            ObjectDataValue::Integer(1),
            ObjectDataValue::Character("hello".into()),
            ObjectDataValue::Integer(1),
        ];
        let record = ObjectDataRecord::new(Handle::new(0x91), &table.fields, &values).unwrap();
        assert_eq!(
            record.payload,
            hex("00000000000000C06C4001000000680065006C006C006F00000001000000")
        );
        assert_eq!(record.values(&table.fields), Some(values));
    }

    #[test]
    fn record_rejects_wrong_values() {
        let table = test_table();
        assert!(ObjectDataRecord::new(Handle::NULL, &table.fields, &[]).is_err());
        let values = vec![
            ObjectDataValue::Integer(230),
            ObjectDataValue::Integer(1),
            ObjectDataValue::Character(String::new()),
            ObjectDataValue::Integer(1),
        ];
        assert!(ObjectDataRecord::new(Handle::NULL, &table.fields, &values).is_err());
    }
}

#[cfg(test)]
mod key_tests {
    use super::*;

    #[test]
    fn next_key_skips_deleted_keys() {
        let entries = vec![
            ("*A2".to_string(), Handle::new(1)),
            ("*A3".to_string(), Handle::new(2)),
        ];
        assert_eq!(next_key(&entries), "*A4");
        assert_eq!(next_key(&[]), "*A1");
    }
}
