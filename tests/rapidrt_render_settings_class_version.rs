//! The AcDbRenderSettings class version of a RAPIDRTRENDERSETTINGS record is
//! stored as is in every file version.
//!
//! The reference application writes 2 to DWG and DXF from R2004 to R2018, and
//! AutoCAD's own R2013 and R2018 drawings store 2. The readers used to add one
//! to the value read from R2013 (AC1027) and the writers to subtract one when
//! writing R2013, so a record read from R2013 carried 3 into every other
//! version and a record written to R2013 stored 1.
//!
//! The records are synthetic: a RAPIDRTRENDERSETTINGS built here.

use std::io::Cursor;

use acadrust::classes::DxfClass;
use acadrust::io::dwg::dwg_stream_readers::object_reader::class_object::read_class_object_data;
use acadrust::io::dwg::dwg_stream_readers::object_reader::DwgObjectReader;
use acadrust::io::dwg::dwg_stream_writers::object_writer::DwgObjectWriter;
use acadrust::objects::{
    ClassObject, ClassObjectData, ObjectType, RapidRtRenderSettings, RenderSettings,
};
use acadrust::{CadDocument, DxfReader, DxfVersion, DxfWriter, Handle};

/// The class version AutoCAD and the reference application store.
const CLASS_VERSION: i32 = 2;

const DWG_VERSIONS: [DxfVersion; 5] = [
    DxfVersion::AC1018,
    DxfVersion::AC1021,
    DxfVersion::AC1024,
    DxfVersion::AC1027,
    DxfVersion::AC1032,
];

const DXF_VERSIONS: [DxfVersion; 5] = DWG_VERSIONS;

fn settings(class_version: i32) -> RapidRtRenderSettings {
    RapidRtRenderSettings {
        base: RenderSettings {
            class_version,
            name: "RapidRT".into(),
            fog_enabled: false,
            fog_background_enabled: false,
            backfaces_enabled: true,
            environment_image_enabled: false,
            environment_image_filename: String::new(),
            description: "Render settings".into(),
            display_index: 3,
            has_predefined: true,
        },
        version: 1,
        render_target: 1,
        render_level: 5,
        render_time: 1,
        lighting_model: 1,
        filter_type: 1,
        filter_width: 2.0,
        filter_height: 2.0,
    }
}

/// A drawing of `version` with one RAPIDRTRENDERSETTINGS record.
fn document(version: DxfVersion, class_version: i32) -> (CadDocument, Handle) {
    let mut document = CadDocument::with_version(version);
    document.classes.add_or_update(DxfClass::new(
        "RAPIDRTRENDERSETTINGS",
        "AcDbRapidRTRenderSettings",
    ));
    let handle = document.allocate_handle();
    let owner = document.header.named_objects_dict_handle;
    document.objects.insert(
        handle,
        ObjectType::ClassObject(ClassObject {
            handle,
            owner,
            data: ClassObjectData::RapidRtRenderSettings(settings(class_version)),
            ..Default::default()
        }),
    );
    (document, handle)
}

/// The class version stored in the DWG record of `handle`, and the value the
/// DWG reader makes of that record.
fn dwg_stored_and_read(document: &CadDocument, handle: Handle) -> (i32, i32) {
    let (bytes, records, _, _) = DwgObjectWriter::new(document).unwrap().write();
    let offsets = records
        .into_iter()
        .map(|(h, offset)| (h, i64::from(offset)))
        .collect();
    let objects = DwgObjectReader::new(bytes, document.version, offsets).unwrap();
    let offset = objects
        .offset_for(handle.value())
        .expect("the record is written") as usize;

    let (type_code, mut reader) = objects.read_record_at(offset).unwrap();
    objects.read_common_non_entity_data(&mut reader, type_code);
    let stored = reader.read_bit_long();

    let (type_code, mut reader) = objects.read_record_at(offset).unwrap();
    objects.read_common_non_entity_data(&mut reader, type_code);
    let Some(ClassObjectData::RapidRtRenderSettings(read)) = read_class_object_data(
        &mut reader,
        "RAPIDRTRENDERSETTINGS",
        objects.version(),
        document.version,
    ) else {
        panic!("{:?}: not RapidRT render settings", document.version)
    };
    reader.check_error().unwrap();
    (stored, read.base.class_version)
}

/// DXF lines of the document, without line ends.
fn dxf_lines(document: &CadDocument) -> Vec<String> {
    let text = String::from_utf8(DxfWriter::new(document).write_to_vec().unwrap()).unwrap();
    text.lines().map(str::to_string).collect()
}

/// Index of the value line of the group 90 that follows the
/// `100 AcDbRenderSettings` subclass marker.
fn dxf_class_version_line(lines: &[String]) -> usize {
    let marker = lines
        .windows(2)
        .position(|pair| pair[0].trim() == "100" && pair[1].trim() == "AcDbRenderSettings")
        .expect("the record is written with its AcDbRenderSettings subclass");
    assert_eq!(lines[marker + 2].trim(), "90");
    marker + 3
}

fn read_dxf(lines: &[String]) -> RapidRtRenderSettings {
    let text = lines.join("\n");
    let document = DxfReader::from_reader(Cursor::new(text.into_bytes()))
        .unwrap()
        .read()
        .unwrap();
    document
        .objects
        .values()
        .find_map(|object| match object {
            ObjectType::ClassObject(ClassObject {
                data: ClassObjectData::RapidRtRenderSettings(value),
                ..
            }) => Some(value.clone()),
            _ => None,
        })
        .expect("the RapidRT render settings are read")
}

#[test]
fn dwg_writer_stores_the_class_version_as_is() {
    for version in DWG_VERSIONS {
        let (document, handle) = document(version, CLASS_VERSION);
        let (stored, _) = dwg_stored_and_read(&document, handle);
        assert_eq!(stored, CLASS_VERSION, "{version:?}");
    }
}

#[test]
fn dwg_reader_takes_the_stored_class_version_as_is() {
    for version in DWG_VERSIONS {
        let (document, handle) = document(version, CLASS_VERSION);
        let (stored, read) = dwg_stored_and_read(&document, handle);
        assert_eq!(read, stored, "{version:?}");
    }
}

#[test]
fn dxf_writer_stores_the_class_version_as_is() {
    for version in DXF_VERSIONS {
        let (document, _) = document(version, CLASS_VERSION);
        let lines = dxf_lines(&document);
        let at = dxf_class_version_line(&lines);
        assert_eq!(lines[at].trim(), CLASS_VERSION.to_string(), "{version:?}");
    }
}

#[test]
fn dxf_reader_takes_the_stored_class_version_as_is() {
    for version in DXF_VERSIONS {
        let (document, _) = document(version, CLASS_VERSION);
        let mut lines = dxf_lines(&document);
        // The record as AutoCAD saves it, whatever the writer made of it.
        let at = dxf_class_version_line(&lines);
        lines[at] = CLASS_VERSION.to_string();
        let read = read_dxf(&lines);
        assert_eq!(read.base.class_version, CLASS_VERSION, "{version:?}");
        assert_eq!(read.base.name, "RapidRT", "{version:?}");
        assert_eq!(read.render_level, 5, "{version:?}");
    }
}

/// A record read from an R2013 DXF keeps its class version when it is saved
/// to DWG in another version.
#[test]
fn r2013_record_keeps_its_class_version_in_other_versions() {
    let (source, _) = document(DxfVersion::AC1027, CLASS_VERSION);
    let mut lines = dxf_lines(&source);
    let at = dxf_class_version_line(&lines);
    lines[at] = CLASS_VERSION.to_string();
    let read = read_dxf(&lines);

    for version in [DxfVersion::AC1021, DxfVersion::AC1032] {
        let (document, handle) = document(version, read.base.class_version);
        let (stored, _) = dwg_stored_and_read(&document, handle);
        assert_eq!(stored, CLASS_VERSION, "R2013 -> {version:?}");
    }
}
