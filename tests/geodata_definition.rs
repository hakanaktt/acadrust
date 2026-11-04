//! GEODATA coordinate-system definitions longer than one DXF group.
//!
//! AutoCAD writes the definition (a MapGuide XML dictionary, several kB with
//! line breaks) as 303 chunks of 250 characters and a final 301, with line
//! breaks as `^J`. The DXF reader must join every chunk in order, and the
//! writer must split the same way, so a definition survives DXF and DWG
//! round trips byte for byte.

use std::io::Cursor;

use acadrust::objects::{GeoData, ObjectType};
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};

const GEO_KEY: &str = "ACAD_GEOGRAPHICDATA";
/// The longest string AutoCAD reads in one DXF group (R2007 and later).
const DXF_STRING_MAX: usize = 2049;

/// A multi-line definition of about 6 kB, with a tab and a non-ASCII name.
fn long_definition() -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-16\" standalone=\"no\" ?>\n\
         <Dictionary version=\"1.0\" xmlns=\"http://www.osgeo.org/mapguide/coordinatesystem\">\n",
    );
    for i in 0..60 {
        xml.push_str(&format!(
            "  <ParameterValue>\n\t<OperationParameterId>Paramètre {i}</OperationParameterId>\n    \
             <Value uom=\"degree\">{}</Value>\n  </ParameterValue>\n",
            5.387_638_888_888_89 + f64::from(i)
        ));
    }
    xml.push_str("</Dictionary>\n");
    xml
}

fn document_with(definition: &str) -> CadDocument {
    let mut doc = CadDocument::new();
    doc.version = DxfVersion::AC1032;
    let ms = doc
        .block_records
        .get("*Model_Space")
        .expect("model space")
        .handle;
    let dict = doc.ensure_extension_dictionary(ms);
    let handle = doc.allocate_handle();
    let mut geo = GeoData::new();
    geo.handle = handle;
    geo.owner = dict;
    geo.reactors = vec![dict];
    geo.version = 3;
    geo.host_block = ms;
    geo.coordinate_type = 2;
    geo.design_point = Vector3::new(155_000.0, 463_000.0, 0.0);
    geo.coordinate_system_definition = definition.to_string();
    doc.objects.insert(handle, ObjectType::GeoData(geo));
    if let Some(ObjectType::Dictionary(d)) = doc.objects.get_mut(&dict) {
        d.add_entry(GEO_KEY, handle);
    }
    doc
}

fn definition_of(doc: &CadDocument) -> String {
    doc.objects
        .values()
        .find_map(|o| match o {
            ObjectType::GeoData(g) => Some(g.coordinate_system_definition.clone()),
            _ => None,
        })
        .expect("a GEODATA")
}

fn dxf_text(doc: &CadDocument) -> String {
    let bytes = DxfWriter::new(doc).write_to_vec().expect("write DXF");
    String::from_utf8(bytes).expect("UTF-8 DXF")
}

fn read_dxf(text: &str) -> CadDocument {
    DxfReader::from_reader(Cursor::new(text.as_bytes().to_vec()))
        .expect("DXF reader")
        .read()
        .expect("read DXF")
}

fn read_dwg(bytes: Vec<u8>) -> CadDocument {
    DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("read DWG")
}

/// The (code, value) pairs of the GEODATA object in a DXF text.
fn geodata_pairs(text: &str) -> Vec<(i32, String)> {
    let lines: Vec<&str> = text.lines().map(|l| l.trim_end_matches('\r')).collect();
    let mut pairs = Vec::new();
    let mut inside = false;
    for pair in lines.chunks(2) {
        let [code, value] = pair else { break };
        let code: i32 = code.trim().parse().expect("group code");
        if code == 0 {
            if inside {
                break;
            }
            inside = *value == "GEODATA";
            continue;
        }
        if inside {
            pairs.push((code, value.to_string()));
        }
    }
    pairs
}

/// Replace the GEODATA's 303/301 groups in `text` with `chunks` (all 303 but
/// the last, which is 301).
fn with_chunks(text: &str, chunks: &[String]) -> String {
    let lines: Vec<&str> = text.lines().map(|l| l.trim_end_matches('\r')).collect();
    let mut out = Vec::new();
    let mut inside = false;
    let mut written = false;
    for pair in lines.chunks(2) {
        let [code, value] = pair else { break };
        let c: i32 = code.trim().parse().expect("group code");
        if c == 0 {
            inside = *value == "GEODATA";
        }
        if inside && (c == 303 || c == 301) {
            if !written {
                for (k, chunk) in chunks.iter().enumerate() {
                    out.push(if k + 1 == chunks.len() { "301" } else { "303" }.to_string());
                    out.push(chunk.clone());
                }
                written = true;
            }
            continue;
        }
        out.push(code.to_string());
        out.push(value.to_string());
    }
    out.join("\r\n") + "\r\n"
}

#[test]
fn a_long_definition_is_written_as_303_chunks_and_a_final_301() {
    let definition = long_definition();
    assert!(definition.len() > 3 * DXF_STRING_MAX);
    let text = dxf_text(&document_with(&definition));
    let chunks: Vec<(i32, String)> = geodata_pairs(&text)
        .into_iter()
        .filter(|(c, _)| *c == 301 || *c == 303)
        .collect();
    assert!(chunks.len() > 1, "split into chunks");
    let (last, rest) = chunks.split_last().expect("chunks");
    assert_eq!(last.0, 301, "the final chunk is 301");
    assert!(rest.iter().all(|(c, _)| *c == 303), "the others are 303");
    for (_, value) in &chunks {
        assert!(value.len() <= DXF_STRING_MAX, "{} bytes", value.len());
        let decoded = value.replace("^J", "\n").replace("^I", "\t");
        assert!(decoded.chars().count() <= 250, "AutoCAD's 250 characters");
        assert!(!value.contains("\\P"), "line breaks are ^J, not MTEXT \\P");
    }
    let joined: String = chunks.iter().map(|(_, v)| v.as_str()).collect();
    assert_eq!(joined, definition.replace('\n', "^J").replace('\t', "^I"));
}

#[test]
fn a_long_definition_survives_dxf_byte_for_byte() {
    let definition = long_definition();
    let once = read_dxf(&dxf_text(&document_with(&definition)));
    assert_eq!(definition_of(&once), definition);
    // And again: what was read writes the same groups.
    let twice = read_dxf(&dxf_text(&once));
    assert_eq!(definition_of(&twice), definition);
}

#[test]
fn a_long_definition_survives_dwg_2018_byte_for_byte() {
    let definition = long_definition();
    let doc = document_with(&definition);
    let from_dwg = read_dwg(DwgWriter::write_to_vec(&doc).expect("write DWG"));
    assert_eq!(definition_of(&from_dwg), definition);
    // DWG → DXF → DWG keeps it too.
    let from_dxf = read_dxf(&dxf_text(&from_dwg));
    assert_eq!(definition_of(&from_dxf), definition);
    let again = read_dwg(DwgWriter::write_to_vec(&from_dxf).expect("write DWG"));
    assert_eq!(definition_of(&again), definition);
}

#[test]
fn a_short_definition_is_one_301_group() {
    let text = dxf_text(&document_with("<Dictionary/>"));
    let chunks: Vec<(i32, String)> = geodata_pairs(&text)
        .into_iter()
        .filter(|(c, _)| *c == 301 || *c == 303)
        .collect();
    assert_eq!(chunks, vec![(301, "<Dictionary/>".to_string())]);
}

#[test]
fn every_chunk_is_read_in_order() {
    // AutoCAD's layout: 303 chunks of 250 characters, then the 301.
    let definition = long_definition();
    let text = dxf_text(&document_with("x"));
    let chars: Vec<char> = definition.chars().collect();
    let chunks: Vec<String> = chars
        .chunks(250)
        .map(|c| {
            c.iter()
                .collect::<String>()
                .replace('\n', "^J")
                .replace('\t', "^I")
        })
        .collect();
    let doc = read_dxf(&with_chunks(&text, &chunks));
    assert_eq!(definition_of(&doc), definition);
}

#[test]
fn an_escape_cut_between_two_chunks_is_joined() {
    // libdxfrw splits the escaped text every 255 bytes, cutting a ^J in two.
    let definition = format!(
        "{}\n{}\t{}",
        "a".repeat(254),
        "b".repeat(253),
        "c".repeat(10)
    );
    let escaped = definition.replace('\n', "^J").replace('\t', "^I");
    let chunks: Vec<String> = escaped
        .as_bytes()
        .chunks(255)
        .map(|c| String::from_utf8(c.to_vec()).expect("ASCII"))
        .collect();
    assert!(chunks[0].ends_with('^') && chunks[1].starts_with('J'));
    assert!(chunks[1].ends_with('^') && chunks[2].starts_with('I'));
    let doc = read_dxf(&with_chunks(&dxf_text(&document_with("x")), &chunks));
    assert_eq!(definition_of(&doc), definition);
}

#[test]
fn a_long_definition_survives_binary_dxf_byte_for_byte() {
    let definition = long_definition();
    let bytes = DxfWriter::new_binary(&document_with(&definition))
        .write_to_vec()
        .expect("write binary DXF");
    let doc = DxfReader::from_reader(Cursor::new(bytes))
        .expect("DXF reader")
        .read()
        .expect("read binary DXF");
    assert_eq!(definition_of(&doc), definition);
}

#[test]
fn a_caret_at_a_chunk_boundary_survives_the_codecs_own_round_trip() {
    // A literal caret on the 250th character, followed by a character the
    // reader would otherwise join with it into a control character.
    for next in ['J', ' ', 'M', 'I'] {
        let definition = format!("{}^{next}{}", "a".repeat(249), "b".repeat(300));
        let text = dxf_text(&document_with(&definition));
        for (code, value) in geodata_pairs(&text) {
            if code == 303 {
                // Escaped, a literal caret is "^ ".
                assert!(
                    !value.ends_with("^ ") && !value.ends_with('^'),
                    "a 303 chunk ends on a caret: {:?}",
                    &value[value.len().saturating_sub(4)..]
                );
            }
        }
        assert_eq!(
            definition_of(&read_dxf(&text)),
            definition,
            "caret then {next:?}"
        );
    }
}
