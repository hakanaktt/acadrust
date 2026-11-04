//! Extended Data (XDATA) support
//!
//! Extended data is application-specific data that can be attached to entities.
//! It is stored in DXF files using group codes 1000-1071.

use crate::types::{Handle, Transform, Vector3};

/// Extended data value types
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum XDataValue {
    /// String value (group code 1000)
    String(String),
    /// Control string (group code 1002) - "{" or "}"
    ControlString(String),
    /// Layer name (group code 1003)
    LayerName(String),
    /// Binary data (group code 1004)
    BinaryData(Vec<u8>),
    /// Database handle (group code 1005)
    Handle(Handle),
    /// 3D point (group codes 1010, 1020, 1030)
    Point3D(Vector3),
    /// 3D position (group codes 1011, 1021, 1031)
    Position3D(Vector3),
    /// 3D displacement (group codes 1012, 1022, 1032)
    Displacement3D(Vector3),
    /// 3D direction (group codes 1013, 1023, 1033)
    Direction3D(Vector3),
    /// Real value (group code 1040)
    Real(f64),
    /// Distance (group code 1041)
    Distance(f64),
    /// Scale factor (group code 1042)
    ScaleFactor(f64),
    /// 16-bit integer (group code 1070)
    Integer16(i16),
    /// 32-bit integer (group code 1071)
    Integer32(i32),
}

/// Extended data record for a single application
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExtendedDataRecord {
    /// Application name (from group code 1001)
    pub application_name: String,
    /// Extended data values
    pub values: Vec<XDataValue>,
}

impl ExtendedDataRecord {
    /// Create a new extended data record
    pub fn new(application_name: impl Into<String>) -> Self {
        Self {
            application_name: application_name.into(),
            values: Vec::new(),
        }
    }

    /// Add a value to the extended data
    pub fn add_value(&mut self, value: XDataValue) {
        self.values.push(value);
    }

    /// Get the number of values
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Check if the record is empty
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// Extended data collection for an entity
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExtendedData {
    /// Extended data records, keyed by application name
    records: Vec<ExtendedDataRecord>,
    /// Raw DWG EED blobs captured during DWG read, used for verbatim
    /// round-trip. Each tuple is (app_handle_value, raw_bytes).
    /// This field is ignored by the DXF reader/writer.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub raw_dwg_eed: Vec<(u64, Vec<u8>)>,
}

impl ExtendedData {
    /// Create a new extended data collection
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            raw_dwg_eed: Vec::new(),
        }
    }

    /// Add a record to the collection
    pub fn add_record(&mut self, record: ExtendedDataRecord) {
        self.records.push(record);
    }

    /// Insert a record, replacing the existing record for the same application.
    ///
    /// XDATA permits only one logical payload per registered application on an
    /// entity. Keeping this invariant here prevents callers from accidentally
    /// writing ambiguous duplicate records when metadata is edited repeatedly.
    pub fn upsert_record(&mut self, record: ExtendedDataRecord) {
        if let Some(existing) = self
            .records
            .iter_mut()
            .find(|existing| existing.application_name == record.application_name)
        {
            *existing = record;
        } else {
            self.records.push(record);
        }
    }

    // Update the modeled fields while retaining unrelated values owned by the app.
    pub(crate) fn merge_modeled_record(&mut self, replacement: &ExtendedDataRecord) {
        let mut record = self
            .remove_record(&replacement.application_name)
            .unwrap_or_else(|| ExtendedDataRecord::new(&replacement.application_name));
        let labeled_group = matches!(replacement.values.get(1), Some(XDataValue::ControlString(value)) if value == "{");
        if labeled_group {
            let start = record
                .values
                .iter()
                .position(|value| Some(value) == replacement.values.first());
            if let Some(start) = start {
                let mut depth = 0;
                let end =
                    record
                        .values
                        .iter()
                        .enumerate()
                        .skip(start + 1)
                        .find_map(|(index, value)| {
                            if let XDataValue::ControlString(value) = value {
                                if value == "{" {
                                    depth += 1;
                                }
                                if value == "}" {
                                    depth -= 1;
                                    if depth == 0 {
                                        return Some(index + 1);
                                    }
                                }
                            }
                            None
                        });
                if let Some(end) = end {
                    record.values.splice(start..end, replacement.values.clone());
                } else {
                    record.values.extend(replacement.values.clone());
                }
            } else {
                record.values.extend(replacement.values.clone());
            }
        } else {
            let mut cursor = 0;
            for value in &replacement.values {
                if let Some(offset) = record.values[cursor..]
                    .iter()
                    .position(|old| std::mem::discriminant(old) == std::mem::discriminant(value))
                {
                    cursor += offset;
                    record.values[cursor] = value.clone();
                    cursor += 1;
                } else {
                    record.values.push(value.clone());
                    cursor = record.values.len();
                }
            }
        }
        self.add_record(record);
    }

    /// Remove and return the record owned by `application_name`.
    pub fn remove_record(&mut self, application_name: &str) -> Option<ExtendedDataRecord> {
        let index = self
            .records
            .iter()
            .position(|record| record.application_name == application_name)?;
        Some(self.records.remove(index))
    }

    /// Get all records
    pub fn records(&self) -> &[ExtendedDataRecord] {
        &self.records
    }

    pub(crate) fn records_mut(&mut self) -> &mut [ExtendedDataRecord] {
        &mut self.records
    }

    /// Get a record by application name
    pub fn get_record(&self, application_name: &str) -> Option<&ExtendedDataRecord> {
        self.records
            .iter()
            .find(|r| r.application_name == application_name)
    }

    /// Get the number of records
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Check if the collection is empty
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Clear all records
    pub fn clear(&mut self) {
        self.records.clear();
    }

    /// Carry the world-space values along when their entity is transformed,
    /// as AutoCAD does: a 1011 position takes the whole transform, a 1012
    /// displacement and a 1013 direction its linear part (the direction
    /// stays a unit vector), and a 1041 distance and 1042 scale factor its
    /// scale. Other values are left alone. Returns whether anything changed;
    /// the DWG writer then writes the edited records instead of the blobs
    /// read from the file. A caller that transforms an entity itself drops
    /// the entity's verbatim `raw_record` when this returns true, as
    /// `EntityType::translate` does.
    pub fn apply_transform(&mut self, transform: &Transform) -> bool {
        let scale = transform.matrix.to_matrix3().determinant().abs().cbrt();
        let mut changed = false;
        for record in &mut self.records {
            for value in &mut record.values {
                let new = match value {
                    XDataValue::Position3D(p) => XDataValue::Position3D(transform.apply(*p)),
                    XDataValue::Displacement3D(d) => {
                        XDataValue::Displacement3D(transform.apply_rotation(*d))
                    }
                    XDataValue::Direction3D(d) => {
                        let moved = transform.apply_rotation(*d);
                        if moved.length() > f64::EPSILON {
                            XDataValue::Direction3D(moved.normalize())
                        } else {
                            continue;
                        }
                    }
                    XDataValue::Distance(v) => XDataValue::Distance(*v * scale),
                    XDataValue::ScaleFactor(v) => XDataValue::ScaleFactor(*v * scale),
                    _ => continue,
                };
                if new != *value {
                    *value = new;
                    changed = true;
                }
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_with_world_xdata() -> crate::entities::EntityType {
        let mut line = crate::entities::Line::from_points(
            Vector3::new(100.0, 100.0, 0.0),
            Vector3::new(110.0, 100.0, 0.0),
        );
        let mut record = ExtendedDataRecord::new("MYAPP");
        record.add_value(XDataValue::String("tag".into()));
        record.add_value(XDataValue::Point3D(Vector3::new(1.0, 2.0, 0.0)));
        record.add_value(XDataValue::Position3D(Vector3::new(100.0, 100.0, 0.0)));
        record.add_value(XDataValue::Displacement3D(Vector3::new(1.0, 0.0, 0.0)));
        record.add_value(XDataValue::Direction3D(Vector3::new(1.0, 0.0, 0.0)));
        record.add_value(XDataValue::Distance(5.0));
        record.add_value(XDataValue::ScaleFactor(1.0));
        record.add_value(XDataValue::Real(5.0));
        line.common.extended_data.add_record(record);
        crate::entities::EntityType::Line(line)
    }

    fn values(entity: &crate::entities::EntityType) -> Vec<XDataValue> {
        entity.common().extended_data.records()[0].values.clone()
    }

    fn close(a: Vector3, b: Vector3) -> bool {
        (a - b).length() < 1e-9
    }

    #[test]
    fn move_carries_world_positions_only() {
        let mut entity = line_with_world_xdata();
        entity.translate(Vector3::new(50.0, 0.0, 0.0));
        let v = values(&entity);
        assert_eq!(v[1], XDataValue::Point3D(Vector3::new(1.0, 2.0, 0.0)));
        assert_eq!(
            v[2],
            XDataValue::Position3D(Vector3::new(150.0, 100.0, 0.0))
        );
        assert_eq!(
            v[3],
            XDataValue::Displacement3D(Vector3::new(1.0, 0.0, 0.0))
        );
        assert_eq!(v[5], XDataValue::Distance(5.0));
    }

    #[test]
    fn rotate_and_scale_carry_vectors_and_distances() {
        let mut entity = line_with_world_xdata();
        entity.rotate(Vector3::new(100.0, 100.0, 0.0), std::f64::consts::FRAC_PI_2);
        entity.apply_scaling_with_origin(
            Vector3::new(2.0, 2.0, 2.0),
            Vector3::new(100.0, 100.0, 0.0),
        );
        let v = values(&entity);
        let XDataValue::Position3D(p) = v[2] else {
            panic!()
        };
        assert!(close(p, Vector3::new(100.0, 100.0, 0.0)));
        let XDataValue::Displacement3D(d) = v[3] else {
            panic!()
        };
        assert!(close(d, Vector3::new(0.0, 2.0, 0.0)), "{d:?}");
        let XDataValue::Direction3D(d) = v[4] else {
            panic!()
        };
        assert!(close(d, Vector3::new(0.0, 1.0, 0.0)), "{d:?}");
        let XDataValue::Distance(d) = v[5] else {
            panic!()
        };
        assert!((d - 10.0).abs() < 1e-9);
        let XDataValue::ScaleFactor(f) = v[6] else {
            panic!()
        };
        assert!((f - 2.0).abs() < 1e-9);
        assert_eq!(v[7], XDataValue::Real(5.0));
    }

    #[test]
    fn mirror_carries_world_positions() {
        let mut entity = line_with_world_xdata();
        entity.mirror(Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 1.0, 0.0));
        let XDataValue::Position3D(p) = values(&entity)[2] else {
            panic!()
        };
        assert!(close(p, Vector3::new(-100.0, 100.0, 0.0)), "{p:?}");
    }

    #[test]
    fn test_xdata_record_creation() {
        let mut record = ExtendedDataRecord::new("MYAPP");
        assert_eq!(record.application_name, "MYAPP");
        assert!(record.is_empty());

        record.add_value(XDataValue::String("test".to_string()));
        assert_eq!(record.len(), 1);
    }

    #[test]
    fn test_xdata_collection() {
        let mut xdata = ExtendedData::new();
        assert!(xdata.is_empty());

        let mut record = ExtendedDataRecord::new("APP1");
        record.add_value(XDataValue::Real(3.14));
        xdata.add_record(record);

        assert_eq!(xdata.len(), 1);
        assert!(xdata.get_record("APP1").is_some());
        assert!(xdata.get_record("APP2").is_none());
    }
}
