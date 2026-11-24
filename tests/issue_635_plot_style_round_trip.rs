//! A plot style table read from a file and saved again comes back the way it
//! was read: the same bytes when nothing was edited, and the stored text with
//! only the edited values changed when something was (#635).

use acadrust::plot_style::PlotStyleTable;
use std::io::{Read, Write};

const CTB_NAME: &str = "sample-LW0.ctb";

/// A colour table laid out the way plotting programs save one: 255 styles,
/// each with its keys in the stored order, ACI 1-9, 209 and 251 plotting
/// black with lineweight 4 on ACI 7, the rest object colour with linetype 31.
/// Built here so the test ships no file saved by another program.
fn ctb_text() -> String {
    let mut text = String::from(concat!(
        "description=\"\n",
        "aci_table_available=TRUE\n",
        "scale_factor=1.0\n",
        "apply_factor=FALSE\n",
        "custom_lineweight_display_units=0\n",
        "aci_table{\n",
    ));
    for i in 0..255 {
        text += &format!(" {i}=\"Color_{}\n", i + 1);
    }
    text += "}\nplot_style{\n";
    for i in 0..255 {
        let aci = i + 1;
        let black = (1..=9).contains(&aci) || aci == 209 || aci == 251;
        let (color, policy) = if black { (-1023410176i64, 5) } else { (-1006632961, 1) };
        text += &format!(
            concat!(
                " {i}{{\n",
                "  name=\"Color_{aci}\n",
                "  localized_name=\"Color_{aci}\n",
                "  description=\"\n",
                "  color={color}\n",
                "  mode_color={color}\n",
                "  color_policy={policy}\n",
                "  physical_pen_number=0\n",
                "  virtual_pen_number=0\n",
                "  screen=100\n",
                "  linepattern_size=0.5\n",
                "  linetype=31\n",
                "  adaptive_linetype=TRUE\n",
                "  lineweight={lw}\n",
                "  fill_style=73\n",
                "  end_style=4\n",
                "  join_style=5\n",
                " }}\n",
            ),
            i = i,
            aci = aci,
            color = color,
            policy = policy,
            lw = if aci == 7 { 4 } else { 0 },
        );
    }
    text += "}\ncustom_lineweight_table{\n";
    for (i, w) in [0.0, 0.05, 0.09, 0.1, 0.13, 0.15, 0.18, 0.2, 0.25, 0.3]
        .iter()
        .enumerate()
    {
        text += &format!(" {i}={w:?}\n");
    }
    text += "}\n";
    text
}

fn ctb() -> Vec<u8> {
    table_file("CTB", &ctb_text())
}

/// The stored text of a table: past the 60-byte header, inflated, NUL dropped.
fn stored_text(file: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    flate2::read::ZlibDecoder::new(&file[60..])
        .read_to_end(&mut out)
        .expect("inflate");
    assert_eq!(out.pop(), Some(0), "the stored text ends in a NUL");
    out
}

fn text_of(file: &[u8]) -> String {
    String::from_utf8(stored_text(file)).unwrap()
}

fn le32(file: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(file[at..at + 4].try_into().unwrap())
}

fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for byte in bytes {
        a = (a + u32::from(*byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

/// A table file around `text`, tagged `tag` (CTB, STB or TTB) in the header.
fn table_file(tag: &str, text: &str) -> Vec<u8> {
    let mut body = text.as_bytes().to_vec();
    body.push(0);
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(6));
    encoder.write_all(&body).unwrap();
    let stream = encoder.finish().unwrap();
    let mut file = format!("PIAFILEVERSION_2.0,{tag}VER1,compress\r\npmzlibcodec").into_bytes();
    file.extend_from_slice(&adler32(&stream).to_le_bytes());
    file.extend_from_slice(&(body.len() as u32).to_le_bytes());
    file.extend_from_slice(&(stream.len() as u32).to_le_bytes());
    file.extend_from_slice(&stream);
    file
}

/// A named table with its styles in the order the file has them (Black
/// before Normal) and object colour stored as a packed value.
const STB_TEXT: &str = concat!(
    "description=\"\n",
    "aci_table_available=FALSE\n",
    "scale_factor=1.0\n",
    "apply_factor=FALSE\n",
    "custom_lineweight_display_units=0\n",
    "plot_style{\n",
    " 0{\n",
    "  name=\"Black\n",
    "  localized_name=\"Black\n",
    "  description=\"\n",
    "  color=-1023410176\n",
    "  mode_color=-1023410176\n",
    "  color_policy=5\n",
    "  lineweight=8\n",
    " }\n",
    " 1{\n",
    "  name=\"Normal\n",
    "  localized_name=\"Normal\n",
    "  description=\"\n",
    "  color=-1006632961\n",
    "  mode_color=-1006632961\n",
    "  color_policy=1\n",
    "  lineweight=0\n",
    " }\n",
    "}\n",
    "custom_lineweight_table{\n",
    " 0=0.0\n",
    " 1=0.05\n",
    "}\n",
);

#[test]
fn ctb_sample_text_rewrite_is_byte_identical() {
    let file = ctb();
    let table = PlotStyleTable::from_bytes(CTB_NAME, &file).unwrap();
    assert!(
        table.to_text() == text_of(&file),
        "text differs from the file"
    );
}

#[test]
fn ctb_sample_unedited_bytes_are_returned_as_read() {
    let file = ctb();
    let table = PlotStyleTable::from_bytes(CTB_NAME, &file).unwrap();
    assert!(table.to_bytes().unwrap() == file, "bytes differ");
}

#[test]
fn ctb_edit_aci_7_lineweight_then_revert_is_text_identical() {
    let file = ctb();
    let mut table = PlotStyleTable::from_bytes(CTB_NAME, &file).unwrap();
    let original = table.aci_entries[7].lineweight;
    assert_eq!(original, 4);
    table.aci_entries[7].lineweight = 8;
    let edited = table.to_text();
    assert!(edited != text_of(&file));
    assert!(edited.contains("  lineweight=8\n"));
    table.aci_entries[7].lineweight = original;
    assert!(table.to_text() == text_of(&file), "reverted text differs");
    // the edit survives a save and a reload
    table.aci_entries[7].lineweight = 8;
    let reloaded = PlotStyleTable::from_bytes(CTB_NAME, &table.to_bytes().unwrap()).unwrap();
    assert_eq!(reloaded.aci_entries[7].lineweight, 8);
    assert!(reloaded.to_text() == edited);
}

#[test]
fn ctb_recompressed_header_is_consistent() {
    let file = ctb();
    let mut table = PlotStyleTable::from_bytes(CTB_NAME, &file).unwrap();
    table.aci_entries[7].lineweight = 8;
    let bytes = table.to_bytes().unwrap();
    assert_eq!(&bytes[..48], &file[..48]);
    let stream = &bytes[60..];
    assert_eq!(le32(&bytes, 48), adler32(stream));
    assert_eq!(le32(&bytes, 56) as usize, stream.len());
    assert_eq!(le32(&bytes, 52) as usize, stored_text(&bytes).len() + 1);
    // one digit swapped for another: the text is as long as the file's
    assert_eq!(le32(&bytes, 52), le32(&file, 52));
    assert!(table.load_warnings.is_empty());
    assert!(PlotStyleTable::from_bytes(CTB_NAME, &bytes)
        .unwrap()
        .load_warnings
        .is_empty());
}

/// Passes before the change too: reading the sample is not what is broken.
#[test]
fn ctb_sample_entries_resolve() {
    let file = ctb();
    let table = PlotStyleTable::from_bytes(CTB_NAME, &file).unwrap();
    for aci in (1..=9).chain([209, 251]) {
        let entry = &table.aci_entries[aci];
        assert_eq!(entry.color_policy, 5, "ACI {aci}");
        assert_eq!(table.resolve_color(aci as u8), Some([0.0; 3]), "ACI {aci}");
    }
    assert_eq!(table.aci_entries[7].lineweight, 4);
    for aci in [10, 100, 255] {
        let entry = &table.aci_entries[aci];
        assert_eq!(table.resolve_color(aci as u8), None, "ACI {aci}");
        assert_eq!(table.resolve_screening(aci as u8), 1.0, "ACI {aci}");
        assert_eq!((entry.linetype, entry.color_policy), (31, 1), "ACI {aci}");
    }
}

/// Passes before the change too: a table built in memory keeps numeric order.
#[test]
fn ctb_built_from_scratch_keeps_numeric_order() {
    let text = PlotStyleTable::identity("new.ctb").to_text();
    assert!(text.contains("plot_style{\n 0{\n"));
}

#[test]
fn stb_unedited_bytes_are_returned_as_read() {
    let file = table_file("STB", STB_TEXT);
    let table = PlotStyleTable::from_bytes("named.stb", &file).unwrap();
    assert!(table.is_stb);
    assert_eq!(table.to_text(), STB_TEXT);
    assert!(table.to_bytes().unwrap() == file, "bytes differ");
}

#[test]
fn stb_edit_changes_only_the_edited_line() {
    let file = table_file("STB", STB_TEXT);
    let mut table = PlotStyleTable::from_bytes("named.stb", &file).unwrap();
    table.named_entries.get_mut("Black").unwrap().lineweight = 9;
    let expected = STB_TEXT.replacen("  lineweight=8\n", "  lineweight=9\n", 1);
    assert_eq!(table.to_text(), expected);
    let reloaded = PlotStyleTable::from_bytes("named.stb", &table.to_bytes().unwrap()).unwrap();
    assert_eq!(reloaded.named_entries["Black"].lineweight, 9);
    assert_eq!(reloaded.to_text(), expected);
}

#[test]
fn an_edited_table_keeps_the_header_tag_it_was_read_with() {
    let file = table_file("TTB", STB_TEXT);
    let mut table = PlotStyleTable::from_bytes("named.stb", &file).unwrap();
    table.named_entries.get_mut("Black").unwrap().lineweight = 9;
    let bytes = table.to_bytes().unwrap();
    assert_eq!(&bytes[..48], &file[..48]);
}
