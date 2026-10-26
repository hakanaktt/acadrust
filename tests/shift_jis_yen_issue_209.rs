//! WHATWG Shift_JIS encodes U+00A5 (yen) to 0x5C and U+203E (overline) to
//! 0x7E without reporting an error, and decodes those bytes back to a
//! backslash and a tilde. `encode_legacy_string` only escapes what the encoder
//! calls unmappable, so it can write a bare backslash for drawing content, and
//! the gated CIF decoder then reads that backslash as the start of an escape.
//!
//! Public API only.

use acadrust::io::dxf::code_page::{decode_cif_escapes_in, encode_legacy_string};

fn round_trip(text: &str) -> String {
    let enc = encoding_rs::SHIFT_JIS;
    let bytes = encode_legacy_string(text, enc);
    let (on_disk, _, _) = enc.decode(&bytes);
    decode_cif_escapes_in(&on_disk, enc)
}

#[test]
fn the_shift_jis_writer_does_not_turn_a_yen_sign_into_a_backslash() {
    let bytes = encode_legacy_string("\u{A5}", encoding_rs::SHIFT_JIS);
    assert_ne!(
        bytes, b"\\",
        "U+00A5 was written as a bare 0x5C, which reads back as a backslash"
    );
}

#[test]
fn a_yen_sign_survives_a_write_and_read() {
    assert_eq!(round_trip("\u{A5}"), "\u{A5}");
}

#[test]
fn a_yen_sign_before_something_escape_shaped_stays_text() {
    // U+00E9 is not in Shift_JIS, so the gate would decode the lookalike.
    let authored = "\u{A5}U+00E9";
    assert_eq!(round_trip(authored), authored);
}

#[test]
fn every_character_shift_jis_does_not_round_trip_is_escaped() {
    for ch in ['\u{A5}', '\u{203E}', 'A', '\u{3042}', '\u{4E2D}'] {
        let s = ch.to_string();
        assert_eq!(
            round_trip(&s),
            s,
            "U+{:04X} did not survive a write/read through Shift_JIS",
            ch as u32
        );
    }
}
