//! Issue #216: the CIF escape gate only means something when the encoding is
//! the drawing's code page. For a stand-in row every well-formed escape
//! decodes, as it did before the gate.

use acadrust::io::dxf::code_page::{
    code_page_is_faithful, decode_cif_escapes_in, encoding_from_code_page,
    encoding_from_dwg_code_page,
};

#[test]
fn issue_216_stand_in_names_are_not_faithful() {
    for name in ["ISO8859-1", "ISO8859-9", "ansi1252", "SOMETHING_UNKNOWN", "ANSI_1255", "ANSI_932"] {
        assert!(!code_page_is_faithful(name), "{name} is a stand-in");
        assert!(!encoding_from_code_page(name).unwrap().is_faithful(), "{name}");
    }
    for name in ["ANSI_1252", "ANSI_1251", "DOS866", "KOREAN", "ANSI_949", "DOS437", "JOHAB"] {
        assert!(code_page_is_faithful(name), "{name} is the code page");
        assert!(encoding_from_code_page(name).unwrap().is_faithful(), "{name}");
    }
}

#[test]
fn issue_216_a_stand_in_row_decodes_every_escape() {
    // windows-1252 holds the euro sign, ISO-8859-1 does not.
    let iso = encoding_from_code_page("ISO8859-1").unwrap();
    assert_eq!(decode_cif_escapes_in("\\U+20AC", iso), "\u{20AC}");
    // WHATWG windows-1255 maps U+05BA, CP1255 does not.
    let hebrew = encoding_from_code_page("ANSI_1255").unwrap();
    assert_eq!(decode_cif_escapes_in("\\U+05BA", hebrew), "\u{05BA}");
    // Control: a faithful page keeps a representable escape literal.
    let ansi = encoding_from_code_page("ANSI_1252").unwrap();
    assert_eq!(decode_cif_escapes_in("94\\U+00B0", ansi), "94\\U+00B0");
}

#[test]
fn issue_216_an_index_outside_the_table_names_no_code_page() {
    for index in [0u16, 45, 99, 65535] {
        let page = encoding_from_dwg_code_page(index);
        assert!(!page.is_faithful(), "index {index}");
        assert_eq!(decode_cif_escapes_in("94\\U+00B0", page), "94\u{B0}", "index {index}");
    }
    assert!(encoding_from_dwg_code_page(30).is_faithful(), "30 is ANSI_1252");
}

#[test]
fn issue_216_a_bare_row_names_no_code_page() {
    let bare = acadrust::io::dxf::code_page::LegacyEncoding::from(encoding_rs::WINDOWS_1252);
    assert!(!bare.is_faithful());
    assert_eq!(decode_cif_escapes_in("94\\U+00B0", bare), "94\u{B0}");
}
