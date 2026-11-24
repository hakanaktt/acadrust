//! Native DWG writers for database data/index helper objects.

use crate::io::dwg::dwg_reference_type::DwgReferenceType;
use crate::objects::{BreakPointReference, DataObject, DataObjectData};

use super::{common, DwgObjectWriter};

/// Whether a BREAKDATA point reference is written with a sub-entity path:
/// it was read with one (non-zero trailing BS) or one is set on the model.
fn has_subentity_path(reference: &BreakPointReference) -> bool {
    reference.trailing_version != 0
        || !reference.subentity_object.is_null()
        || reference.subentity_type != 0
        || reference.subentity_index != 0
        || reference.subentity_reserved != 0
}

impl<'a> DwgObjectWriter<'a> {
    pub(super) fn write_data_object(&mut self, object: &DataObject) {
        let type_code = match &object.data {
            DataObjectData::Dummy => common::OBJ_DUMMY,
            DataObjectData::LongTransaction => common::OBJ_LONG_TRANSACTION,
            _ => self.class_type_code(object.dxf_name(), 500),
        };
        self.write_common_non_entity_data(
            type_code,
            object.handle,
            object.owner,
            &object.reactors,
            &object.xdictionary_handle,
        );
        if matches!(&object.data, DataObjectData::BreakData(_)) {
            let owner = self
                .owner_overrides
                .get(&object.handle)
                .copied()
                .unwrap_or(object.owner);
            let reactors = if object.reactors.is_empty() {
                self.document
                    .reactors_by_handle
                    .get(&object.handle)
                    .cloned()
                    .unwrap_or_default()
            } else {
                object.reactors.clone()
            };
            let xdictionary = object.xdictionary_handle.or_else(|| {
                self.document
                    .xdic_by_handle
                    .get(&object.handle)
                    .copied()
                    .filter(|handle| self.document.objects.contains_key(handle))
            });
            let handles = self.writer.handle_mut();
            handles.reset();
            if owner.value() < object.handle.value() {
                let delta = object.handle.value() - owner.value();
                let count = ((64 - delta.leading_zeros() + 7) / 8) as u8;
                handles.write_byte(0xC0 | count);
                let bytes = delta.to_be_bytes();
                handles.write_bytes(&bytes[8 - count as usize..]);
            } else {
                handles.write_handle(DwgReferenceType::SoftPointer, owner.value());
            }
            for reactor in reactors {
                handles.write_handle(DwgReferenceType::SoftPointer, reactor.value());
            }
            if let Some(xdictionary) = xdictionary {
                handles.write_handle(DwgReferenceType::HardOwnership, xdictionary.value());
            } else if !self.version.r2004_plus() {
                handles.write_handle(DwgReferenceType::HardOwnership, 0);
            }
        }
        match &object.data {
            DataObjectData::BreakData(value) => {
                self.writer.write_bit_short(value.version);
                self.writer
                    .write_bit_long(value.point_references.len() as i32);
                for reference in &value.point_references {
                    self.writer.write_bit_short(reference.version);
                    if self.version.r2013_plus(self.dxf_version) {
                        self.writer.write_bit_short(reference.reserved);
                    } else {
                        self.writer.write_bit(reference.reserved != 0);
                    }
                    self.writer.write_bit_long(reference.reference_type);
                    self.writer.write_bit_short(reference.flags);
                    self.writer.write_bit_long(reference.identifier);
                    self.writer.write_3bit_double(reference.first_point);
                    self.writer.write_3bit_double(reference.second_point);
                    if has_subentity_path(reference) {
                        self.writer
                            .write_bit_short(reference.trailing_version.max(1));
                        self.writer.write_bit_long(reference.subentity_type);
                        self.writer.write_bit_long(reference.subentity_index);
                        self.writer.write_bit_long(reference.subentity_reserved);
                    } else {
                        self.writer.write_bit_short(0);
                    }
                }
                self.writer.write_handle(
                    DwgReferenceType::SoftPointer,
                    value.dimension_reference.value(),
                );
                for reference in &value.point_references {
                    self.writer.write_handle(
                        DwgReferenceType::SoftPointer,
                        reference.reserved_reference.value(),
                    );
                    if has_subentity_path(reference) {
                        self.writer.write_handle(
                            DwgReferenceType::SoftPointer,
                            reference.subentity_object.value(),
                        );
                    }
                }
            }
            DataObjectData::BreakPointRef => {}
            DataObjectData::CellStyleMap(value) => {
                self.writer.write_bit_long(value.cells.len() as i32);
                for cell in &value.cells {
                    self.write_named_table_cell_style(cell);
                }
            }
            DataObjectData::AcDsRecord
            | DataObjectData::AcDsSchema
            | DataObjectData::Dummy
            | DataObjectData::LongTransaction
            | DataObjectData::ObjectPointer => {}
            DataObjectData::IdBuffer(value) => {
                self.writer.write_byte(value.flags);
                self.writer.write_bit_long(value.object_ids.len() as i32);
                for handle in &value.object_ids {
                    self.writer
                        .write_handle(DwgReferenceType::SoftPointer, handle.value());
                }
            }
            DataObjectData::Index(value) => {
                self.writer.write_datetime(
                    value.last_updated_julian_day,
                    value.last_updated_milliseconds,
                );
            }
            DataObjectData::LayerIndex(value) => {
                self.writer.write_datetime(
                    value.last_updated_julian_day,
                    value.last_updated_milliseconds,
                );
                self.writer.write_bit_long(value.entries.len() as i32);
                for entry in &value.entries {
                    self.writer.write_bit_long(entry.layer_count);
                    self.writer.write_variable_text(&entry.name);
                    self.writer
                        .write_handle(DwgReferenceType::HardPointer, entry.id_buffer.value());
                }
            }
            DataObjectData::PartialViewingFilter(_) => {}
            DataObjectData::TableGeometry(value) => {
                self.writer.write_bit_long(value.rows);
                self.writer.write_bit_long(value.columns);
                self.writer.write_bit_long(value.cells.len() as i32);
                for cell in &value.cells {
                    self.writer.write_bit_long(cell.geometry_data_flag);
                    self.writer.write_bit_double(cell.width_with_gap);
                    self.writer.write_bit_double(cell.height_with_gap);
                    self.writer
                        .write_handle(DwgReferenceType::SoftPointer, cell.table_geometry.value());
                    self.writer.write_bit_long(cell.geometry.len() as i32);
                    for geometry in &cell.geometry {
                        self.writer.write_3bit_double(geometry.distance_to_top_left);
                        self.writer.write_3bit_double(geometry.distance_to_center);
                        self.writer.write_bit_double(geometry.width);
                        self.writer.write_bit_double(geometry.height);
                        self.writer.write_bit_double(geometry.outer_width);
                        self.writer.write_bit_double(geometry.outer_height);
                        self.writer.write_bit_long(geometry.flags);
                    }
                }
            }
        }
        self.register_object(object.handle);
    }
}

#[cfg(test)]
mod tests {
    //! A BREAKDATA is written in AutoCAD's layout: a reference with a
    //! sub-entity path has a trailing BS of 1, the path's three BLs and,
    //! after its reserved pointer, the path's object; a static two-point
    //! reference has a trailing BS of 0 and only the reserved pointer.

    use super::*;
    use crate::io::dwg::dwg_stream_readers::merged_reader::DwgMergedReader;
    use crate::io::dwg::dwg_stream_readers::object_reader::DwgObjectReader;
    use crate::objects::{BreakData, BreakPointReference};
    use crate::types::{DxfVersion, Handle, Vector3};
    use crate::CadDocument;

    const VERSIONS: [DxfVersion; 4] = [
        DxfVersion::AC1015,
        DxfVersion::AC1018,
        DxfVersion::AC1024,
        DxfVersion::AC1032,
    ];

    /// References: object 0x201 (path set on the model only), static, object 0x202.
    fn break_data() -> DataObject {
        let with_path = |object: u64| BreakPointReference {
            identifier: -1,
            subentity_object: Handle::from(object),
            subentity_type: 2,
            ..BreakPointReference::default()
        };
        let mut object = DataObject::new(DataObjectData::BreakData(BreakData {
            dimension_reference: Handle::from(0x200u64),
            point_references: vec![
                with_path(0x201),
                BreakPointReference {
                    reference_type: 2,
                    first_point: Vector3::new(10.0, 20.0, 0.0),
                    second_point: Vector3::new(15.0, 20.0, 0.0),
                    ..BreakPointReference::default()
                },
                with_path(0x202),
            ],
            ..BreakData::default()
        }));
        object.handle = Handle::from(0x1000u64);
        object.owner = Handle::from(0x0cu64);
        object
    }

    /// Writes `object` in `version` and returns a reader at its body.
    fn written_body(version: DxfVersion, object: &DataObject) -> DwgMergedReader {
        let document = CadDocument::with_version(version);
        let mut writer = DwgObjectWriter::new(&document).expect("writer");
        writer.write_data_object(object);
        let offset = writer.handle_map()[0].1 as usize;
        let reader = DwgObjectReader::new(writer.output().to_vec(), version, Default::default())
            .expect("object reader");
        let (kind, mut record) = reader.read_record_at(offset).expect("record");
        reader.read_common_non_entity_data(&mut record, kind);
        record
    }

    /// Reads one reference's main-stream fields up to its trailing BS.
    fn reference_head(r: &mut DwgMergedReader, version: DxfVersion) -> (i32, Vector3, i16) {
        r.read_bit_short(); // version
        if version >= DxfVersion::AC1027 {
            r.read_bit_short();
        } else {
            r.read_bit();
        }
        let reference_type = r.read_bit_long();
        r.read_bit_short(); // flags
        r.read_bit_long(); // identifier
        let first = r.read_3bit_double();
        r.read_3bit_double();
        (reference_type, first, r.read_bit_short())
    }

    #[test]
    fn static_point_reference_is_written_without_subentity_path() {
        for version in VERSIONS {
            let mut r = written_body(version, &break_data());
            assert_eq!(r.read_bit_short(), 0, "{version:?}: version");
            assert_eq!(r.read_bit_long(), 3, "{version:?}: count");

            let (kind, _, trailing) = reference_head(&mut r, version);
            assert_eq!((kind, trailing), (0, 1), "{version:?}: first reference");
            assert_eq!(
                [r.read_bit_long(), r.read_bit_long(), r.read_bit_long()],
                [2, 0, 0],
                "{version:?}: first path"
            );

            let (kind, first, trailing) = reference_head(&mut r, version);
            assert_eq!(kind, 2, "{version:?}: static reference type");
            assert_eq!(first, Vector3::new(10.0, 20.0, 0.0), "{version:?}");
            assert_eq!(trailing, 0, "{version:?}: static reference has no path");

            let (kind, first, trailing) = reference_head(&mut r, version);
            assert_eq!(
                (kind, first, trailing),
                (0, Vector3::ZERO, 1),
                "{version:?}: third"
            );
            assert_eq!(
                [r.read_bit_long(), r.read_bit_long(), r.read_bit_long()],
                [2, 0, 0],
                "{version:?}: third path"
            );
            r.check_error().unwrap();
            assert_eq!(r.main_remaining_bits(), 0, "{version:?}: data left over");

            assert_eq!(r.read_handle(), 0x200, "{version:?}: dimension");
            assert_eq!(
                [
                    r.read_handle(),
                    r.read_handle(),
                    r.read_handle(),
                    r.read_handle(),
                    r.read_handle()
                ],
                [0, 0x201, 0, 0, 0x202],
                "{version:?}: reserved pointers and path objects"
            );
            r.check_error().unwrap();
        }
    }
}
