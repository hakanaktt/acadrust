//! Native DWG readers for database data/index helper objects.

use crate::entities::CellContentGeometry;
use crate::io::dwg::dwg_stream_readers::merged_reader::DwgMergedReader;
use crate::objects::{
    BreakData, BreakPointReference, CellStyleMap, DataObjectData, IdBuffer, Index, LayerIndex,
    LayerIndexEntry, PartialViewingFilter, TableGeometry, TableGeometryCell,
};
use crate::types::Handle;

fn count(value: i32, reader: &DwgMergedReader) -> usize {
    reader.checked_count(i64::from(value)) as usize
}

pub fn read_data_object_data(
    reader: &mut DwgMergedReader,
    dxf_name: &str,
) -> Option<DataObjectData> {
    let data = match dxf_name.to_uppercase().as_str() {
        "BREAKDATA" => {
            // Main stream: BS version, BL count, then per reference its
            // fields through the trailing BS and, when that is non-zero, the
            // sub-entity path's type and index and one reserved BL. Handle
            // stream: the dimension, then per reference a reserved soft
            // pointer and, when it has a path, the sub-entity object.
            let version = reader.read_bit_short();
            let mut point_references = Vec::new();
            for _ in 0..count(reader.read_bit_long(), reader) {
                let mut reference = BreakPointReference {
                    version: reader.read_bit_short(),
                    reserved: if reader.dxf_version() >= crate::types::DxfVersion::AC1027 {
                        reader.read_bit_short()
                    } else {
                        reader.read_bit() as i16
                    },
                    reference_type: reader.read_bit_long(),
                    flags: reader.read_bit_short(),
                    identifier: reader.read_bit_long(),
                    first_point: reader.read_3bit_double(),
                    second_point: reader.read_3bit_double(),
                    trailing_version: reader.read_bit_short(),
                    ..BreakPointReference::default()
                };
                if reference.trailing_version != 0 {
                    reference.subentity_type = reader.read_bit_long();
                    reference.subentity_index = reader.read_bit_long();
                    reference.subentity_reserved = reader.read_bit_long();
                }
                point_references.push(reference);
            }
            let dimension_reference = Handle::from(reader.read_handle());
            for reference in &mut point_references {
                reference.reserved_reference = Handle::from(reader.read_handle());
                if reference.trailing_version != 0 {
                    reference.subentity_object = Handle::from(reader.read_handle());
                }
            }
            DataObjectData::BreakData(BreakData {
                version,
                dimension_reference,
                point_references,
                ..BreakData::default()
            })
        }
        "BREAKPOINTREF" => DataObjectData::BreakPointRef,
        "CELLSTYLEMAP" => {
            let mut cells = Vec::new();
            for _ in 0..count(reader.read_bit_long(), reader) {
                cells.push(super::objects::read_named_table_cell_style(reader));
            }
            DataObjectData::CellStyleMap(CellStyleMap { cells })
        }
        "ACDSRECORD" => DataObjectData::AcDsRecord,
        "ACDSSCHEMA" => DataObjectData::AcDsSchema,
        "IDBUFFER" => {
            let flags = reader.read_byte();
            let mut object_ids = Vec::new();
            for _ in 0..count(reader.read_bit_long(), reader) {
                object_ids.push(Handle::from(reader.read_handle()));
            }
            DataObjectData::IdBuffer(IdBuffer { flags, object_ids })
        }
        "INDEX" => DataObjectData::Index(Index {
            last_updated_julian_day: reader.read_bit_long(),
            last_updated_milliseconds: reader.read_bit_long(),
        }),
        "LAYER_INDEX" => {
            let last_updated_julian_day = reader.read_bit_long();
            let last_updated_milliseconds = reader.read_bit_long();
            let mut entries = Vec::new();
            for _ in 0..count(reader.read_bit_long(), reader) {
                entries.push(LayerIndexEntry {
                    layer_count: reader.read_bit_long(),
                    name: reader.read_variable_text(),
                    id_buffer: Handle::from(reader.read_handle()),
                });
            }
            DataObjectData::LayerIndex(LayerIndex {
                last_updated_julian_day,
                last_updated_milliseconds,
                entries,
            })
        }
        "PARTIAL_VIEWING_FILTER" => DataObjectData::PartialViewingFilter(PartialViewingFilter),
        "LONG_TRANSACTION" => DataObjectData::LongTransaction,
        "TABLEGEOMETRY" => {
            let rows = reader.read_bit_long();
            let columns = reader.read_bit_long();
            let mut cells = Vec::new();
            for _ in 0..count(reader.read_bit_long(), reader) {
                let geometry_data_flag = reader.read_bit_long();
                let width_with_gap = reader.read_bit_double();
                let height_with_gap = reader.read_bit_double();
                let table_geometry = Handle::from(reader.read_handle());
                let mut geometry = Vec::new();
                for _ in 0..count(reader.read_bit_long(), reader) {
                    geometry.push(CellContentGeometry {
                        distance_to_top_left: reader.read_3bit_double(),
                        distance_to_center: reader.read_3bit_double(),
                        width: reader.read_bit_double(),
                        height: reader.read_bit_double(),
                        outer_width: reader.read_bit_double(),
                        outer_height: reader.read_bit_double(),
                        flags: reader.read_bit_long(),
                    });
                }
                cells.push(TableGeometryCell {
                    geometry_data_flag,
                    width_with_gap,
                    height_with_gap,
                    table_geometry,
                    geometry,
                });
            }
            DataObjectData::TableGeometry(TableGeometry {
                rows,
                columns,
                cells,
            })
        }
        _ => return None,
    };
    Some(data)
}

#[cfg(test)]
mod tests {
    use super::read_data_object_data;
    use crate::io::dwg::dwg_reference_type::DwgReferenceType::SoftPointer;
    use crate::io::dwg::dwg_stream_readers::merged_reader::DwgMergedReader;
    use crate::io::dwg::dwg_stream_writers::merged_writer::DwgMergedWriter;
    use crate::io::dwg::dwg_version::DwgVersion;
    use crate::objects::DataObjectData;
    use crate::types::{DxfVersion, Vector3};

    /// A BREAKDATA as AutoCAD writes it when a reference to an object is
    /// followed by a static two-point break (`reference_type` 2): the static
    /// reference ends at a trailing BS of 0, with no sub-entity path in the
    /// main stream and only the reserved pointer in the handle stream. Seen
    /// in R2010 drawings; R2004 has the same record layout and its
    /// two-stream framing lets the test build the streams directly.
    #[test]
    fn static_point_reference_has_no_subentity_path() {
        let dxf = DxfVersion::AC1018;
        let mut w = DwgMergedWriter::new(DwgVersion::AC18, dxf);
        let first = Vector3::new(10.0, 20.0, 0.0);
        let second = Vector3::new(15.0, 20.0, 0.0);
        w.write_bit_short(0); // BREAKDATA version
        w.write_bit_long(3); // references: object, static, object
        for (reference_type, identifier, points, trailing) in [
            (0, -1, (Vector3::ZERO, Vector3::ZERO), 1),
            (2, 0, (first, second), 0),
            (0, -1, (Vector3::ZERO, Vector3::ZERO), 1),
        ] {
            w.write_bit_short(0); // version
            w.write_bit(true); // reserved (a bit before R2013)
            w.write_bit_long(reference_type);
            w.write_bit_short(0); // flags
            w.write_bit_long(identifier);
            w.write_3bit_double(points.0);
            w.write_3bit_double(points.1);
            w.write_bit_short(trailing);
            if trailing != 0 {
                w.write_bit_long(2); // sub-entity type
                w.write_bit_long(0); // sub-entity index
                w.write_bit_long(0); // reserved
            }
        }
        w.write_handle(SoftPointer, 0x100); // dimension
        w.write_handle(SoftPointer, 0); // first: reserved pointer
        w.write_handle(SoftPointer, 0x101); // first: object
        w.write_handle(SoftPointer, 0); // static: reserved pointer only
        w.write_handle(SoftPointer, 0); // third: reserved pointer
        w.write_handle(SoftPointer, 0x102); // third: object

        let mut reader = DwgMergedReader::new(w.merge(), dxf, w.handle_start_bits());
        let data = read_data_object_data(&mut reader, "BREAKDATA").expect("BREAKDATA");
        reader.check_error().expect("the record has enough bits");
        let DataObjectData::BreakData(data) = data else {
            panic!("BreakData expected");
        };
        assert_eq!(data.dimension_reference.value(), 0x100);
        let refs = &data.point_references;
        assert_eq!(refs.len(), 3);
        assert_eq!(refs[0].subentity_object.value(), 0x101);
        assert_eq!(refs[0].subentity_type, 2);

        assert_eq!(refs[1].reference_type, 2);
        assert_eq!((refs[1].first_point, refs[1].second_point), (first, second));
        assert_eq!(refs[1].trailing_version, 0);
        assert!(refs[1].subentity_object.is_null());
        assert_eq!((refs[1].subentity_type, refs[1].subentity_index), (0, 0));

        // The reference after the static one is still in step.
        assert_eq!((refs[2].reference_type, refs[2].identifier), (0, -1));
        assert_eq!(refs[2].trailing_version, 1);
        assert_eq!(refs[2].subentity_type, 2);
        assert_eq!(refs[2].subentity_object.value(), 0x102);
    }
}
