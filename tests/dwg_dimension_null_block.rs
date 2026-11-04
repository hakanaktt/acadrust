//! A DIMENSION without a geometry block keeps an empty block name through a
//! DWG write and read, so a later DXF save writes no group 2.
//!
//! The DWG builder named the null block handle `*U0`, and the DXF writer then
//! wrote `2 *U0`, a reference to a block that does not exist.

use acadrust::entities::{Dimension, DimensionAligned, EntityType};
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfWriter};
use std::io::Cursor;

#[test]
fn dimension_without_block_stays_without_block_through_dwg() {
    for version in [DxfVersion::AC1015, DxfVersion::AC1032] {
        let mut doc = CadDocument::with_version(version);
        let dim = DimensionAligned::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(10.0, 0.0, 0.0));
        assert!(dim.base.block_name.is_empty());
        let handle = doc
            .add_entity(EntityType::Dimension(Box::new(Dimension::Aligned(dim))))
            .unwrap();
        let bytes = DwgWriter::write_to_vec(&doc).expect("write dwg");
        let back = DwgReader::from_stream(Cursor::new(bytes))
            .read()
            .expect("read dwg");
        let name = match back.get_entity(handle) {
            Some(EntityType::Dimension(d)) => d.base().block_name.clone(),
            other => panic!("{version:?}: dimension lost: {other:?}"),
        };
        assert_eq!(name, "", "{version:?}");
        let text = String::from_utf8(DxfWriter::new(&back).write_to_vec().unwrap()).unwrap();
        assert!(
            !text.lines().any(|line| line.trim() == "*U0"),
            "{version:?}: DXF references a missing block *U0"
        );
    }
}
