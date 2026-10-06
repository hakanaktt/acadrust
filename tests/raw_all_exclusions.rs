use acadrust::entities::{EntityType, Helix, Line};
use acadrust::objects::{DictionaryCloningFlags, ObjectType, XRecord};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfVersion, Handle};
use std::ffi::OsString;
use std::io::Cursor;

struct RawEnvironment {
    all: Option<OsString>,
    exclude: Option<OsString>,
}

impl RawEnvironment {
    fn new() -> Self {
        let saved = Self {
            all: std::env::var_os("ACADRUST_RAW_ALL"),
            exclude: std::env::var_os("ACADRUST_RAW_EXCLUDE"),
        };
        std::env::remove_var("ACADRUST_RAW_ALL");
        std::env::remove_var("ACADRUST_RAW_EXCLUDE");
        saved
    }
}

impl Drop for RawEnvironment {
    fn drop(&mut self) {
        for (name, value) in [
            ("ACADRUST_RAW_ALL", &self.all),
            ("ACADRUST_RAW_EXCLUDE", &self.exclude),
        ] {
            if let Some(value) = value {
                std::env::set_var(name, value);
            } else {
                std::env::remove_var(name);
            }
        }
    }
}

fn roundtrip(document: &CadDocument) -> CadDocument {
    let bytes = DwgWriter::write_to_vec(document).unwrap();
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

fn assert_markers(
    document: &CadDocument,
    lines: &[Handle],
    helix: Handle,
    line_x: f64,
    radius: f64,
) {
    for &handle in lines {
        let Some(EntityType::Line(line)) = document.get_entity(handle) else {
            panic!("missing line {handle:?}");
        };
        assert_eq!(line.end.x, line_x);
    }
    let Some(EntityType::Helix(helix)) = document.get_entity(helix) else {
        panic!("missing helix");
    };
    assert_eq!(helix.radius, radius);
}

// One test per binary keeps environment changes isolated from other tests.
#[test]
fn exclusions_force_serialization_in_serial_and_parallel_writers() {
    let _environment = RawEnvironment::new();
    for (version, line_count) in [
        (DxfVersion::AC1018, 1),
        (DxfVersion::AC1021, 1),
        (DxfVersion::AC1032, 1_024),
    ] {
        std::env::remove_var("ACADRUST_RAW_ALL");
        std::env::remove_var("ACADRUST_RAW_EXCLUDE");
        let mut source = CadDocument::with_version(version);
        let lines: Vec<_> = (0..line_count)
            .map(|_| {
                source
                    .add_entity(EntityType::Line(Line::from_coords(0., 0., 0., 1., 1., 0.)))
                    .unwrap()
            })
            .collect();
        let mut helix = Helix::new();
        helix.radius = 2.;
        let helix_handle = source.add_entity(EntityType::Helix(Box::new(helix))).unwrap();
        let mut xrecord = XRecord::new();
        xrecord.handle = source.allocate_handle();
        xrecord.add_string(1, "source");
        let xrecord_handle = xrecord.handle;
        source
            .objects
            .insert(xrecord_handle, ObjectType::XRecord(xrecord));
        std::env::set_var("ACADRUST_RAW_ALL", "1");
        let mut captured = roundtrip(&source);

        // Deliberately retain the source cache after adding semantic markers.
        // This makes the selected writer path observable without a binary fixture:
        // raw passthrough emits the old values, serialization emits the markers.
        for handle in lines.iter().copied().chain([helix_handle]) {
            let raw = captured
                .get_entity(handle)
                .unwrap()
                .common()
                .raw_record
                .clone();
            assert!(raw.is_some());
            let entity = captured.get_entity_mut(handle).unwrap();
            match entity {
                EntityType::Line(line) => line.end.x = 9.,
                EntityType::Helix(helix) => helix.radius = 7.,
                _ => unreachable!(),
            }
            entity.common_mut().raw_record = raw;
        }
        let Some(ObjectType::XRecord(xrecord)) = captured.objects.get_mut(&xrecord_handle) else {
            panic!("missing xrecord");
        };
        assert!(xrecord.raw_dwg_data.is_some());
        xrecord.cloning_flags = DictionaryCloningFlags::KeepExisting;
        xrecord.entries_complete = false;

        for (exclude, line_x, radius) in [
            ("", 1., 2.),
            ("19", 9., 2.),
            (" heLix ", 1., 7.),
            ("19, HELIX", 9., 7.),
            (" entities ", 9., 7.),
        ] {
            std::env::set_var("ACADRUST_RAW_EXCLUDE", exclude);
            assert_markers(&roundtrip(&captured), &lines, helix_handle, line_x, radius);
        }
        for (exclude, expected) in [
            ("", DictionaryCloningFlags::NotApplicable),
            ("79", DictionaryCloningFlags::KeepExisting),
            ("objects", DictionaryCloningFlags::KeepExisting),
        ] {
            std::env::set_var("ACADRUST_RAW_EXCLUDE", exclude);
            let actual = roundtrip(&captured);
            let Some(ObjectType::XRecord(xrecord)) = actual.objects.get(&xrecord_handle) else {
                panic!("missing xrecord");
            };
            assert_eq!(xrecord.cloning_flags, expected);
        }

        // EXCLUDE alone must not override the normal entity cache policy.
        std::env::remove_var("ACADRUST_RAW_ALL");
        std::env::set_var("ACADRUST_RAW_EXCLUDE", "ENTITIES");
        assert_markers(&roundtrip(&captured), &lines, helix_handle, 1., 2.);
    }
}
