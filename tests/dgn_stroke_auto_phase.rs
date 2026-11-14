//! A DGN stroke pattern's auto phase is a bit-long in DWG.
//!
//! AutoCAD writes the field after the iteration limit as a BL. It was read and
//! written as a BD, which takes the same two bits for 0 but none of the eight
//! bits that follow `01` for 1: every field after it was read from the wrong
//! place. In a drawing with the "Leader Line" line style, 20 stroke
//! components came back with no strokes, and AutoCAD discarded them as
//! improperly read once the drawing was saved.

use acadrust::io::dwg::dwg_stream_readers::bit_reader::DwgBitReader;
use acadrust::io::dwg::dwg_stream_readers::merged_reader::DwgMergedReader;
use acadrust::io::dwg::dwg_stream_readers::object_reader::dgn_linestyle::read_dgn_line_style_data;
use acadrust::io::dwg::dwg_stream_writers::bit_writer::DwgBitWriter;
use acadrust::io::dwg::DwgVersion;
use acadrust::objects::{
    DgnLineStyleData, DgnLineStyleObject, DgnLsComponentData, DgnLsComponentType, DgnLsPhaseMode,
    DgnLsStroke, DgnLsStrokePattern, ObjectType,
};
use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgReader, DwgWriter};
use std::io::Cursor;

const UID: [u8; 16] = [
    13, 97, 62, 115, 125, 183, 224, 17, 174, 89, 0, 48, 5, 139, 243, 190,
];

/// The "Leader Line - Stroke" pattern of the drawing it was found in: drawn once, auto phased,
/// a long dash and a short gap.
fn leader_line() -> DgnLsStrokePattern {
    let stroke = |is_dash, invert_at_origin, invert_at_end, length| DgnLsStroke {
        is_dash,
        bypass_corner: true,
        can_be_scaled: true,
        invert_at_origin,
        invert_at_end,
        length,
        start_width: 0.0,
        end_width: 0.0,
        width_mode: 0,
        cap_mode: 0,
    };
    DgnLsStrokePattern {
        has_iteration_limit: true,
        is_single_segment: false,
        iteration_limit: 0,
        auto_phase: 1.0,
        phase: 0.0,
        phase_mode: DgnLsPhaseMode::Centered,
        strokes: vec![
            stroke(true, false, true, 3.8446),
            stroke(true, true, false, 0.0001),
        ],
    }
}

/// `pattern`'s component data as AutoCAD writes it, with the auto phase as a BL.
fn autocad_bits(
    pattern: &DgnLsStrokePattern,
    version: DwgVersion,
    dxf_version: DxfVersion,
) -> (DwgMergedReader, i64) {
    let mut main = DwgBitWriter::new(version, dxf_version);
    let mut text = DwgBitWriter::new(version, dxf_version);
    let split_text = version.r2007_plus();
    // The name follows the component type in R2004 and older (#372).
    main.write_bit_long(1); // version
    main.write_bit_long(3); // stroke pattern component
    if split_text {
        text.write_variable_text("Leader Line - Stroke");
    } else {
        main.write_variable_text("Leader Line - Stroke");
    }
    main.write_bytes(&UID);
    main.write_bit_double(1.0); // scale
    main.write_bytes(&[1]); // property flags
    main.write_bit(pattern.has_iteration_limit);
    main.write_bit(pattern.is_single_segment);
    main.write_bit_long(pattern.iteration_limit);
    main.write_bit_long(pattern.auto_phase as i32);
    main.write_bit_double(pattern.phase);
    main.write_bit(true); // phase mode 2: centred
    main.write_bit(false);
    main.write_bit_long(pattern.strokes.len() as i32);
    for s in &pattern.strokes {
        main.write_bit(s.is_dash);
        main.write_bit(s.bypass_corner);
        main.write_bit(s.can_be_scaled);
        main.write_bit(s.invert_at_origin);
        main.write_bit(s.invert_at_end);
        main.write_bit_double(s.length);
        main.write_bit_double(s.start_width);
        main.write_bit_double(s.end_width);
        main.write_bit_long(s.width_mode);
        main.write_bit_long(s.cap_mode);
    }
    let end = main.position_in_bits();
    let mut reader = DwgMergedReader::from_readers(
        DwgBitReader::new(main.into_bytes(), version, dxf_version),
        split_text.then(|| DwgBitReader::new(text.into_bytes(), version, dxf_version)),
        None,
        dxf_version,
    );
    reader.set_main_data_end(end);
    (reader, end)
}

/// Reads `pattern` back from the bits AutoCAD would write for it.
fn read_autocad(pattern: &DgnLsStrokePattern) {
    for dxf_version in [DxfVersion::AC1018, DxfVersion::AC1024, DxfVersion::AC1032] {
        let version = DwgVersion::from_dxf_version(dxf_version).unwrap();
        let (mut reader, end) = autocad_bits(pattern, version, dxf_version);
        let data = read_dgn_line_style_data(&mut reader, "LSSTROKEPATTERNCOMPONENT")
            .expect("a stroke pattern component");
        let DgnLineStyleData::Component {
            description,
            component: DgnLsComponentData::Stroke(read),
            ..
        } = data
        else {
            panic!("{dxf_version:?}: not a stroke pattern: {data:?}");
        };
        assert_eq!(description, "Leader Line - Stroke", "{dxf_version:?}");
        assert_eq!(&read, pattern, "{dxf_version:?}");
        assert_eq!(
            reader.position_in_bits(),
            end,
            "{dxf_version:?}: data left over"
        );
    }
}

#[test]
fn a_stroke_pattern_written_by_autocad_reads_whole() {
    // As found: an auto phase of 1, whose BL takes eight bits more than a BD of 1.
    read_autocad(&leader_line());
}

#[test]
fn an_auto_phase_of_zero_reads_as_before() {
    // 0 takes the same two bits as a BL and as a BD: line styles without auto phase read the
    // same either way.
    read_autocad(&DgnLsStrokePattern {
        auto_phase: 0.0,
        ..leader_line()
    });
}

#[test]
fn a_large_auto_phase_reads_as_a_full_bit_long() {
    // Past a byte a BL is 32 bits, where a BD would read 64.
    read_autocad(&DgnLsStrokePattern {
        auto_phase: 300.0,
        ..leader_line()
    });
}

#[test]
fn an_auto_phased_stroke_pattern_survives_dwg_save() {
    for version in [DxfVersion::AC1024, DxfVersion::AC1032] {
        let mut doc = CadDocument::with_version(version);
        let handle = doc.allocate_handle();
        let data = DgnLineStyleData::Component {
            kind: DgnLsComponentType::Stroke,
            description: "Leader Line - Stroke".into(),
            version: 1,
            component_uid: UID,
            scale: 1.0,
            property_flags: 1,
            component: DgnLsComponentData::Stroke(leader_line()),
            properties: Vec::new(),
        };
        doc.objects.insert(
            handle,
            ObjectType::DgnLineStyle(DgnLineStyleObject {
                handle,
                owner: doc.header.named_objects_dict_handle,
                reactors: Vec::new(),
                xdictionary_handle: None,
                data,
            }),
        );
        let bytes = DwgWriter::write_to_vec(&doc).unwrap();
        let back = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
        match back.objects.get(&handle) {
            Some(ObjectType::DgnLineStyle(DgnLineStyleObject {
                data:
                    DgnLineStyleData::Component {
                        component: DgnLsComponentData::Stroke(pattern),
                        ..
                    },
                ..
            })) => assert_eq!(*pattern, leader_line(), "{version:?}"),
            other => panic!("{version:?}: stroke pattern lost, read {other:?}"),
        }
    }
}
