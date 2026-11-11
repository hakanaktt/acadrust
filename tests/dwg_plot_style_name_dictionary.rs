//! ACAD_PLOTSTYLENAME is a dictionary with a default (ACDBDICTIONARYWDFLT) in
//! every DWG AutoCAD and ODA write. Third-party DXF files store it as a plain
//! DICTIONARY; the DWG writer promotes it, with `Normal` (else the first
//! entry) as the default.

use acadrust::objects::{Dictionary, ObjectType, PlaceHolder};
use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter, Handle};
use std::io::Cursor;

const VERSIONS: [DxfVersion; 6] = [
    DxfVersion::AC1015,
    DxfVersion::AC1018,
    DxfVersion::AC1021,
    DxfVersion::AC1024,
    DxfVersion::AC1027,
    DxfVersion::AC1032,
];

/// A document whose ACAD_PLOTSTYLENAME is a plain dictionary holding one
/// placeholder per name in `names`, and whose class table has no
/// ACDBDICTIONARYWDFLT, as in a DXF file written by another program.
fn plain_plot_style_names(version: DxfVersion, names: &[&str]) -> CadDocument {
    let mut doc = CadDocument::with_version(version);
    let dict_handle = doc.header.acad_plotstylename_dict_handle;
    let old: Vec<Handle> = match doc.objects.get(&dict_handle) {
        Some(ObjectType::DictionaryWithDefault(dict)) => {
            dict.entries.iter().map(|(_, handle)| *handle).collect()
        }
        other => panic!("new document: {other:?}"),
    };
    for handle in old {
        doc.objects.remove(&handle);
    }
    let mut dict = Dictionary::new();
    dict.handle = dict_handle;
    dict.owner = doc.header.named_objects_dict_handle;
    for name in names {
        let mut placeholder = PlaceHolder::new();
        placeholder.handle = doc.allocate_handle();
        placeholder.owner = dict_handle;
        dict.add_entry(*name, placeholder.handle);
        doc.objects
            .insert(placeholder.handle, ObjectType::PlaceHolder(placeholder));
    }
    doc.objects
        .insert(dict_handle, ObjectType::Dictionary(dict));
    let kept: Vec<_> = doc
        .classes
        .iter()
        .filter(|class| class.dxf_name != "ACDBDICTIONARYWDFLT")
        .cloned()
        .collect();
    doc.classes.clear();
    for class in kept {
        doc.classes.push_preserving(class);
    }
    doc
}

fn read_back(doc: &CadDocument) -> CadDocument {
    let dwg = DwgWriter::write_to_vec(doc).unwrap();
    DwgReader::from_stream(Cursor::new(dwg)).read().unwrap()
}

/// The object the named object dictionary lists as ACAD_PLOTSTYLENAME.
fn plot_style_names(doc: &CadDocument) -> &ObjectType {
    let handle = doc
        .named_objects_dictionary()
        .and_then(|root| root.get("ACAD_PLOTSTYLENAME"))
        .expect("ACAD_PLOTSTYLENAME in the named object dictionary");
    doc.objects.get(&handle).expect("ACAD_PLOTSTYLENAME object")
}

/// Entry names and default entry name of a dictionary with default, or the
/// object type when it is something else.
fn names_and_default(doc: &CadDocument) -> (Vec<String>, String) {
    match plot_style_names(doc) {
        ObjectType::DictionaryWithDefault(dict) => {
            let names = dict.entries.iter().map(|(name, _)| name.clone()).collect();
            let default = dict
                .entries
                .iter()
                .find(|(_, handle)| *handle == dict.default_handle)
                .map(|(name, _)| name.clone())
                .unwrap_or_else(|| format!("{:?}", dict.default_handle));
            (names, default)
        }
        other => panic!("ACAD_PLOTSTYLENAME read back as {other:?}"),
    }
}

#[test]
fn plain_dictionary_is_written_with_normal_as_default() {
    for version in VERSIONS {
        let doc = plain_plot_style_names(version, &["Style 1", "Normal"]);
        let read = read_back(&doc);
        assert_eq!(
            names_and_default(&read),
            (
                vec!["Style 1".to_string(), "Normal".to_string()],
                "Normal".to_string()
            ),
            "{version:?}"
        );
        assert!(
            read.classes.contains("ACDBDICTIONARYWDFLT"),
            "{version:?}: class table"
        );
        // The writer works on its own copy.
        assert!(matches!(plot_style_names(&doc), ObjectType::Dictionary(_)));
    }
}

#[test]
fn without_normal_the_first_entry_is_the_default() {
    for version in [DxfVersion::AC1015, DxfVersion::AC1021, DxfVersion::AC1032] {
        let doc = plain_plot_style_names(version, &["Style 1", "Style 2"]);
        let read = read_back(&doc);
        assert_eq!(
            names_and_default(&read),
            (
                vec!["Style 1".to_string(), "Style 2".to_string()],
                "Style 1".to_string()
            ),
            "{version:?}"
        );
    }
}

#[test]
fn plain_dictionary_from_dxf_is_written_with_default() {
    for version in [DxfVersion::AC1015, DxfVersion::AC1018, DxfVersion::AC1021] {
        let doc = plain_plot_style_names(version, &["Normal"]);
        let dxf = DxfWriter::new(&doc).write_to_vec().unwrap();
        let read = DxfReader::from_reader(Cursor::new(dxf))
            .unwrap()
            .read()
            .unwrap();
        assert!(
            matches!(plot_style_names(&read), ObjectType::Dictionary(_)),
            "{version:?}: DXF keeps the plain dictionary"
        );
        let read = read_back(&read);
        assert_eq!(
            names_and_default(&read),
            (vec!["Normal".to_string()], "Normal".to_string()),
            "{version:?}"
        );
    }
}

#[test]
fn dictionary_with_default_is_left_as_it_is() {
    for version in VERSIONS {
        let doc = CadDocument::with_version(version);
        let read = read_back(&doc);
        assert_eq!(
            names_and_default(&read),
            (vec!["Normal".to_string()], "Normal".to_string()),
            "{version:?}"
        );
    }
}
