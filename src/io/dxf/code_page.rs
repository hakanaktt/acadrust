//! DXF code page ($DWGCODEPAGE) to encoding mapping.
//!
//! Exact legacy encodings, including OEM pages and Johab outside WHATWG.

use encoding_rs::Encoding;
use std::borrow::Cow;

mod johab;

/// A stateless encoding used by pre-Unicode CAD strings.
///
/// `Whatwg` is an `encoding_rs` row and nothing more: it may be only a
/// stand-in for the code page a drawing declared, so it cannot be asked what
/// that drawing could have held. `WhatwgExact` is the same row where it *is*
/// the code page (see [`code_page_is_faithful`]), and `Oem` and `Johab` are
/// exact tables. The two `Whatwg` spellings compare equal, because they
/// transcode bytes identically.
#[derive(Clone, Copy, Debug)]
pub enum LegacyEncoding {
    Whatwg(&'static Encoding),
    WhatwgExact(&'static Encoding),
    Oem(u16),
    Johab,
}

impl PartialEq for LegacyEncoding {
    fn eq(&self, other: &Self) -> bool {
        match (self.whatwg(), other.whatwg()) {
            (Some(a), Some(b)) => std::ptr::eq(a, b),
            (None, None) => match (self, other) {
                (Self::Oem(a), Self::Oem(b)) => a == b,
                (Self::Johab, Self::Johab) => true,
                _ => false,
            },
            _ => false,
        }
    }
}

impl Eq for LegacyEncoding {}

impl From<&'static Encoding> for LegacyEncoding {
    fn from(encoding: &'static Encoding) -> Self {
        Self::Whatwg(encoding)
    }
}

impl LegacyEncoding {
    /// The `encoding_rs` row, for the two spellings that are one.
    fn whatwg(self) -> Option<&'static Encoding> {
        match self {
            Self::Whatwg(e) | Self::WhatwgExact(e) => Some(e),
            _ => None,
        }
    }

    /// Whether this encoding is the code page itself, so that it can be asked
    /// which characters the drawing could not have held. False for a bare
    /// `encoding_rs` row, which may be a stand-in, or for a code page nobody
    /// named.
    pub fn is_faithful(self) -> bool {
        !matches!(self, Self::Whatwg(_))
    }

    pub fn decode_without_bom_handling<'a>(self, bytes: &'a [u8]) -> (Cow<'a, str>, bool) {
        if let Some(encoding) = self.whatwg() {
            return encoding.decode_without_bom_handling(bytes);
        }
        let (text, _, errors) = self.decode(bytes);
        (text, errors)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Whatwg(encoding) | Self::WhatwgExact(encoding) => encoding.name(),
            Self::Oem(437) => "CP437",
            Self::Oem(850) => "CP850",
            Self::Oem(852) => "CP852",
            Self::Oem(855) => "CP855",
            Self::Oem(857) => "CP857",
            Self::Oem(860) => "CP860",
            Self::Oem(861) => "CP861",
            Self::Oem(863) => "CP863",
            Self::Oem(864) => "CP864",
            Self::Oem(865) => "CP865",
            Self::Oem(869) => "CP869",
            Self::Oem(_) => "OEM",
            Self::Johab => "JOHAB",
        }
    }

    /// Whether this encoding holds `ch`, so that [`encode_legacy_string`] writes
    /// it as bytes instead of a `\U+XXXX` escape.
    ///
    /// The bytes must also decode back to `ch`: WHATWG Shift_JIS encodes U+00A5
    /// to `0x5C` without an error, and that byte reads back as a backslash.
    pub fn can_encode(self, ch: char) -> bool {
        let mut buf = [0u8; 4];
        let text: &str = ch.encode_utf8(&mut buf);
        let (bytes, _, unmappable) = self.encode(text);
        !unmappable && self.decode(&bytes).0 == text
    }

    pub fn decode<'a>(self, bytes: &'a [u8]) -> (Cow<'a, str>, Self, bool) {
        match self {
            Self::Whatwg(encoding) | Self::WhatwgExact(encoding) => {
                let (text, _, errors) = encoding.decode(bytes);
                (text, self, errors)
            }
            Self::Oem(page) => {
                let table = oem_cp::code_table::DECODING_TABLE_CP_MAP.get(&page);
                let mut errors = false;
                // The OEM crate preserves a few undefined bytes as controls;
                // CAD text follows the original Microsoft mapping instead.
                let text = bytes
                    .iter()
                    .map(|&byte| {
                        let ch = if page == 864 && byte == 0x25 {
                            Some('\u{066A}')
                        } else if page == 864 && matches!(byte, 0x9B..=0x9C | 0x9F)
                            || page == 869 && matches!(byte, 0x80..=0x85 | 0x87 | 0x93..=0x94)
                        {
                            None
                        } else {
                            table.and_then(|table| table.decode_char_checked(byte))
                        };
                        ch.unwrap_or_else(|| {
                            errors = true;
                            '\u{FFFD}'
                        })
                    })
                    .collect::<String>();
                (Cow::Owned(text), self, errors)
            }
            Self::Johab => {
                let (text, errors) = johab::decode(bytes);
                (Cow::Owned(text), self, errors)
            }
        }
    }

    pub fn encode<'a>(self, text: &'a str) -> (Cow<'a, [u8]>, Self, bool) {
        if let Some(encoding) = self.whatwg() {
            let (bytes, _, errors) = encoding.encode(text);
            return (bytes, self, errors);
        }
        let mut errors = false;
        let mut bytes = Vec::with_capacity(text.len());
        for ch in text.chars() {
            match self {
                Self::Oem(page) => {
                    let byte = if page == 864 && ch == '\u{066A}' {
                        Some(0x25)
                    } else if page == 864 && (ch == '%' || matches!(ch as u32, 0x9B..=0x9C | 0x9F))
                        || page == 869 && matches!(ch as u32, 0x80..=0x85 | 0x87 | 0x93..=0x94)
                    {
                        None
                    } else if ch.is_ascii() {
                        Some(ch as u8)
                    } else {
                        oem_cp::code_table::ENCODING_TABLE_CP_MAP
                            .get(&page)
                            .and_then(|table| table.get(&ch))
                            .copied()
                    };
                    bytes.push(byte.unwrap_or_else(|| {
                        errors = true;
                        b'?'
                    }));
                }
                Self::Johab => {
                    if let Some(encoded) = johab::encode_char(ch) {
                        bytes.extend_from_slice(&encoded.0[..encoded.1]);
                    } else {
                        errors = true;
                        bytes.push(b'?');
                    }
                }
                Self::Whatwg(_) | Self::WhatwgExact(_) => unreachable!(),
            }
        }
        (Cow::Owned(bytes), self, errors)
    }
}

/// Get the legacy encoding for a DXF code page string.
///
/// Returns `None` for ASCII and UTF-8. Unknown names retain the historical
/// Windows-1252 fallback.
///
/// # Rules
/// - If the DXF version is AC1021 (ACAD 2007+) or later, UTF-8 is always
///   used regardless of $DWGCODEPAGE — callers should not call this function.
/// - Otherwise, the code page string (case-insensitive) is looked up in the
///   mapping table.
pub fn encoding_from_code_page(code_page: &str) -> Option<LegacyEncoding> {
    if matches!(
        code_page.to_ascii_lowercase().as_str(),
        "ascii" | "utf-8" | "utf8" | "unicode"
    ) {
        return None;
    }
    try_encoding_from_code_page(code_page).or(Some(encoding_rs::WINDOWS_1252.into()))
}

/// Resolve an explicit legacy encoding without silently falling back for typos.
/// Returns `None` for unknown names, ASCII, and UTF-8.
pub fn try_encoding_from_code_page(code_page: &str) -> Option<LegacyEncoding> {
    let page = code_page.to_ascii_lowercase();
    let oem = match page.as_str() {
        "dos437" => Some(437),
        "dos850" => Some(850),
        "dos852" => Some(852),
        "dos855" => Some(855),
        "dos857" => Some(857),
        "dos860" => Some(860),
        "dos861" => Some(861),
        "dos863" => Some(863),
        "dos864" => Some(864),
        "dos865" => Some(865),
        "dos869" => Some(869),
        _ => None,
    };
    if let Some(page) = oem {
        return Some(LegacyEncoding::Oem(page));
    }
    if page == "johab" {
        return Some(LegacyEncoding::Johab);
    }
    let encoding = match page.as_str() {
        // Asian encodings
        "gb2312" | "ansi_936" => Some(encoding_rs::GBK),
        "big5" | "ansi_950" | "dos950" => Some(encoding_rs::BIG5),
        "korean" | "ansi_949" => Some(encoding_rs::EUC_KR),
        "ansi_932" | "dos932" => Some(encoding_rs::SHIFT_JIS),
        "mac-roman" => Some(encoding_rs::MACINTOSH),

        // DOS/OEM code pages
        "dos866" => Some(encoding_rs::IBM866),

        // Windows/ANSI code pages
        "ansi_874" => Some(encoding_rs::WINDOWS_874),
        "ansi_1250" => Some(encoding_rs::WINDOWS_1250),
        "ansi_1251" => Some(encoding_rs::WINDOWS_1251),
        "ansi_1252" => Some(encoding_rs::WINDOWS_1252),
        "ansi_1253" => Some(encoding_rs::WINDOWS_1253),
        "ansi_1254" => Some(encoding_rs::WINDOWS_1254),
        "ansi_1255" => Some(encoding_rs::WINDOWS_1255),
        "ansi_1256" => Some(encoding_rs::WINDOWS_1256),
        "ansi_1257" => Some(encoding_rs::WINDOWS_1257),
        "ansi_1258" => Some(encoding_rs::WINDOWS_1258),

        // ISO encodings
        "iso8859-1" | "iso_8859-1" => Some(encoding_rs::WINDOWS_1252),
        "iso8859-2" | "iso_8859-2" => Some(encoding_rs::ISO_8859_2),
        "iso8859-3" | "iso_8859-3" => Some(encoding_rs::ISO_8859_3),
        "iso8859-4" | "iso_8859-4" => Some(encoding_rs::ISO_8859_4),
        "iso8859-5" | "iso_8859-5" => Some(encoding_rs::ISO_8859_5),
        "iso8859-6" | "iso_8859-6" => Some(encoding_rs::ISO_8859_6),
        "iso8859-7" | "iso_8859-7" => Some(encoding_rs::ISO_8859_7),
        "iso8859-8" | "iso_8859-8" => Some(encoding_rs::ISO_8859_8),
        "iso8859-9" | "iso_8859-9" => Some(encoding_rs::WINDOWS_1254),
        "iso8859-10" | "iso_8859-10" => Some(encoding_rs::ISO_8859_10),
        "iso8859-13" | "iso_8859-13" => Some(encoding_rs::ISO_8859_13),
        "iso8859-14" | "iso_8859-14" => Some(encoding_rs::ISO_8859_14),
        "iso8859-15" | "iso_8859-15" => Some(encoding_rs::ISO_8859_15),

        // KOI8-R (Russian)
        "koi8-r" => Some(encoding_rs::KOI8_R),
        "koi8-u" => Some(encoding_rs::KOI8_U),

        // ASCII / UTF-8 / no fallback needed
        "ascii" | "utf-8" | "utf8" | "unicode" => None,

        _ => None,
    };
    encoding.map(|row| {
        if code_page_is_faithful(&page) {
            LegacyEncoding::WhatwgExact(row)
        } else {
            LegacyEncoding::Whatwg(row)
        }
    })
}

/// Resolve the compact code-page index stored in DWG file metadata.
pub fn dwg_code_page_name(index: u16) -> &'static str {
    match index {
        1 => "ASCII",
        2 => "ISO8859-1",
        3 => "ISO8859-2",
        4 => "ISO8859-3",
        5 => "ISO8859-4",
        6 => "ISO8859-5",
        7 => "ISO8859-6",
        8 => "ISO8859-7",
        9 => "ISO8859-8",
        10 => "ISO8859-9",
        11 => "DOS437",
        12 => "DOS850",
        13 => "DOS852",
        14 => "DOS855",
        15 => "DOS857",
        16 => "DOS860",
        17 => "DOS861",
        18 => "DOS863",
        19 => "DOS864",
        20 => "DOS865",
        21 => "DOS869",
        22 | 38 => "ANSI_932",
        23 => "MAC-ROMAN",
        24 | 41 => "BIG5",
        25 | 40 => "KOREAN",
        26 | 42 => "JOHAB",
        27 => "DOS866",
        28 => "ANSI_1250",
        29 => "ANSI_1251",
        30 => "ANSI_1252",
        31 | 39 => "GB2312",
        32 => "ANSI_1253",
        33 => "ANSI_1254",
        34 => "ANSI_1255",
        35 => "ANSI_1256",
        36 => "ANSI_1257",
        37 => "ANSI_874",
        43 => "UTF-8",
        44 => "ANSI_1258",
        _ => "ANSI_1252",
    }
}

/// Whether `index` is one of the compact DWG code page indices this crate maps.
///
/// Anything else is decoded with the ANSI_1252 fallback by
/// [`dwg_code_page_name`], so callers can use this to tell a declared 1252
/// file from a guessed one.
pub fn dwg_code_page_is_known(index: u16) -> bool {
    matches!(index, 1..=44)
}

/// Convert a DXF `$DWGCODEPAGE` name to the compact DWG metadata index.
pub fn dwg_code_page_index(code_page: &str) -> u16 {
    match code_page.to_ascii_lowercase().as_str() {
        "ascii" => 1,
        "iso8859-1" | "iso_8859-1" => 2,
        "iso8859-2" | "iso_8859-2" => 3,
        "iso8859-3" | "iso_8859-3" => 4,
        "iso8859-4" | "iso_8859-4" => 5,
        "iso8859-5" | "iso_8859-5" => 6,
        "iso8859-6" | "iso_8859-6" => 7,
        "iso8859-7" | "iso_8859-7" => 8,
        "iso8859-8" | "iso_8859-8" => 9,
        "iso8859-9" | "iso_8859-9" => 10,
        "dos437" => 11,
        "dos850" => 12,
        "dos852" => 13,
        "dos855" => 14,
        "dos857" => 15,
        "dos860" => 16,
        "dos861" => 17,
        "dos863" => 18,
        "dos864" => 19,
        "dos865" => 20,
        "dos869" => 21,
        "ansi_932" | "dos932" => 22,
        "mac-roman" => 23,
        "big5" | "ansi_950" | "dos950" => 24,
        "korean" | "ansi_949" => 25,
        "johab" => 26,
        "dos866" => 27,
        "ansi_1250" | "ansi1250" => 28,
        "ansi_1251" | "ansi1251" => 29,
        "ansi_1252" | "ansi1252" => 30,
        "gb2312" | "ansi_936" => 31,
        "ansi_1253" | "ansi1253" => 32,
        "ansi_1254" | "ansi1254" => 33,
        "ansi_1255" | "ansi1255" => 34,
        "ansi_1256" | "ansi1256" => 35,
        "ansi_1257" | "ansi1257" => 36,
        "ansi_874" => 37,
        "utf-8" | "utf8" | "unicode" => 43,
        "ansi_1258" | "ansi1258" => 44,
        _ => 30,
    }
}

pub fn encoding_from_dwg_code_page(index: u16) -> LegacyEncoding {
    let encoding = encoding_from_code_page(dwg_code_page_name(index))
        .unwrap_or(encoding_rs::WINDOWS_1252.into());
    // An index outside the table lands on the `ANSI_1252` default name, which
    // is a stand-in for whatever this drawing actually declared.
    match encoding {
        LegacyEncoding::WhatwgExact(row) if !(1..=44).contains(&index) => {
            LegacyEncoding::Whatwg(row)
        }
        other => other,
    }
}

/// Whether the `encoding_rs` row [`encoding_from_code_page`] picks for
/// `code_page` has that code page's repertoire.
///
/// Only a faithful row can answer "could this drawing have held this
/// character?". The rest are approximations, and asking them yields an answer
/// about a different character set: CP932 maps neither `U+00A5` nor `U+203E`,
/// where WHATWG Shift_JIS does; the WHATWG `Big5` row is a superset of the
/// code page it stands in for (`Big5` is HKSCS, CP950 is not), while the
/// `EUC-KR` row *is* CP949, so `ANSI_949` and `KOREAN` are faithful. An
/// unrecognised name reaches the windows-1252 default and is a stand-in for
/// whatever the drawing really declared; so is a spelling the table does not
/// match (`ansi1252`), even though a matched spelling of the same code page
/// would be faithful. The OEM pages and Johab are exact tables of their own, so they are on
/// the list.
pub fn code_page_is_faithful(code_page: &str) -> bool {
    matches!(
        code_page.to_ascii_lowercase().as_str(),
        // Windows/ANSI rows: WHATWG windows-125x is CP125x except in the
        // unassigned C1 slots U+0081..U+009F, which WHATWG maps to C1
        // controls and Microsoft leaves undefined; no glyph, no drawing
        // content. ANSI_1255 is the one real exception and is not on this
        // list: WHATWG maps U+05BA at 0xCA where CP1255 leaves it undefined.
        "ansi_1250"
            | "ansi_1251"
            | "ansi_1252"
            | "ansi_1253"
            | "ansi_1254"
            | "ansi_1256"
            | "ansi_1257"
            | "ansi_1258"
            | "ansi_874"
            // encoding_rs' ibm866 is CP866 and its euc-kr is CP949, measured
            // against Microsoft's CP866.TXT and CP949.TXT with no difference
            // in either direction.
            | "dos866"
            | "korean"
            | "ansi_949"
            // ISO rows that map to their own `encoding_rs` row. ISO8859-1 and
            // ISO8859-9 are absent on purpose: they borrow windows-1252 and
            // windows-1254.
            | "iso8859-2"
            | "iso_8859-2"
            | "iso8859-3"
            | "iso_8859-3"
            | "iso8859-4"
            | "iso_8859-4"
            | "iso8859-5"
            | "iso_8859-5"
            | "iso8859-6"
            | "iso_8859-6"
            | "iso8859-7"
            | "iso_8859-7"
            | "iso8859-8"
            | "iso_8859-8"
            | "iso8859-10"
            | "iso_8859-10"
            | "iso8859-13"
            | "iso_8859-13"
            | "iso8859-14"
            | "iso_8859-14"
            | "iso8859-15"
            | "iso_8859-15"
            | "koi8-r"
            // GBK is CP936's row here, but it is not CP936: the two agree on
            // 23907 of GBK's 24085 BMP scalars. An escape naming one of the
            // 178 GBK-only scalars is transport this gate calls content.
            // Accepted in the original gate work: the DWG index table and
            // ACadSharp both treat ANSI_936 as the CP936 identity.
            | "gb2312"
            | "ansi_936"
            // Exact tables of their own (`Oem`, `Johab`), not a stand-in.
            | "dos437"
            | "dos850"
            | "dos852"
            | "dos855"
            | "dos857"
            | "dos860"
            | "dos861"
            | "dos863"
            | "dos864"
            | "dos865"
            | "dos869"
            | "johab"
    )
}

/// Encode a string to a legacy (pre-UTF-16) DWG code page.
///
/// Characters the code page cannot represent are emitted as ACAD CIF
/// `\U+XXXX` escapes (astral-plane characters as a surrogate pair of
/// escapes) instead of `encoding_rs`'s HTML `&#NNNNN;` references, which
/// no CAD application understands. Well-formed CIF escapes round-trip
/// through [`decode_cif_escapes`].
pub fn encode_legacy_string(text: &str, encoding: impl Into<LegacyEncoding>) -> Vec<u8> {
    let encoding = encoding.into();
    let (encoded, _, unmappable) = encoding.encode(text);
    if !unmappable && encoding.decode(&encoded).0 == text {
        return encoded.into_owned();
    }
    let mut out = Vec::with_capacity(text.len() + 6);
    let mut buf = [0u8; 4];
    for ch in text.chars() {
        // Every DWG code page is stateless, so per-char encoding is safe.
        let (bytes, _, _) = encoding.encode(ch.encode_utf8(&mut buf));
        if !encoding.can_encode(ch) {
            let mut units = [0u16; 2];
            for unit in ch.encode_utf16(&mut units) {
                out.extend_from_slice(format!("\\U+{:04X}", unit).as_bytes());
            }
        } else {
            out.extend_from_slice(&bytes);
        }
    }
    out
}

/// Decode ACAD CIF `\U+XXXX` escapes (exactly four hex digits) into
/// Unicode characters.
///
/// A high-surrogate escape followed by a low-surrogate escape combines into
/// a scalar value. Malformed or unterminated escapes are left as literal
/// text, and invalid code points are dropped — matching the MTEXT
/// formatter's behavior for the same escapes.
pub fn decode_cif_escapes(text: &str) -> String {
    decode_cif(text, None)
}

/// Decode ACAD CIF `\U+XXXX` escapes in text read from a drawing whose code
/// page is `encoding`.
///
/// [`encode_legacy_string`] escapes a character only when the code page cannot
/// hold it, so an escape naming a character the page *can* hold was never
/// written as transport: it is the text itself, and it is copied through as it
/// stands (lower-case hex included). Surrogate pairs are judged as the one
/// scalar they name. UTF-8 holds every character, so nothing in it was ever
/// written as transport and its escapes are all text.
pub fn decode_cif_escapes_in(text: &str, encoding: impl Into<LegacyEncoding>) -> String {
    let encoding = encoding.into();
    if encoding == LegacyEncoding::Whatwg(encoding_rs::UTF_8) {
        return text.to_string();
    }
    decode_cif(text, Some(encoding))
}

/// [`decode_legacy_escapes`] for text read from a drawing whose code page is
/// `encoding`: MIF escapes decode as before, CIF escapes by
/// [`decode_cif_escapes_in`].
pub fn decode_legacy_escapes_in(text: &str, encoding: impl Into<LegacyEncoding>) -> String {
    decode_cif_escapes_in(&decode_mif_escapes(text), encoding)
}

fn decode_cif(text: &str, held_by: Option<LegacyEncoding>) -> String {
    if !text.contains("\\U+") {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let hex_at = |start: usize| -> Option<u16> {
        if start + 4 > len {
            return None;
        }
        let hex: String = chars[start..start + 4].iter().collect();
        u16::from_str_radix(&hex, 16).ok()
    };
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < len {
        if chars[i] == '\\' && i + 7 <= len && chars[i + 1] == 'U' && chars[i + 2] == '+' {
            if let Some(unit) = hex_at(i + 3) {
                let start = i;
                i += 7;
                let mut units = [unit, 0];
                let mut count = 1usize;
                // Combine a following low-surrogate escape into one scalar.
                if (0xD800..0xDC00).contains(&unit)
                    && i + 7 <= len
                    && chars[i] == '\\'
                    && chars[i + 1] == 'U'
                    && chars[i + 2] == '+'
                {
                    if let Some(low) = hex_at(i + 3) {
                        if (0xDC00..0xE000).contains(&low) {
                            units[1] = low;
                            count = 2;
                            i += 7;
                        }
                    }
                }
                if let Some(Ok(ch)) = std::char::decode_utf16(units[..count].iter().copied()).next()
                {
                    if held_by.is_some_and(|e| e.is_faithful() && e.can_encode(ch)) {
                        out.extend(&chars[start..i]);
                    } else {
                        out.push(ch);
                    }
                }
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Decode ACAD MIF `\M+nxxyy` escapes into Unicode characters.
///
/// The digit `n` selects the legacy double-byte code page: 1 is Shift-JIS,
/// 2 is Big5, 3 is EUC-KR, 4 is Johab, and 5 is GB2312/GBK. The following
/// four hexadecimal digits are decoded as the two bytes in that code page.
/// Malformed escapes and byte sequences that the selected encoding cannot
/// decode are left as literal text.
pub fn decode_mif_escapes(text: &str) -> String {
    if !text.contains("\\M+") {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < len {
        if i + 8 <= len && chars[i] == '\\' && chars[i + 1] == 'M' && chars[i + 2] == '+' {
            let encoding = match chars[i + 3] {
                '1' => Some(encoding_rs::SHIFT_JIS.into()),
                '2' => Some(encoding_rs::BIG5.into()),
                '3' => Some(encoding_rs::EUC_KR.into()),
                '4' => Some(LegacyEncoding::Johab),
                '5' => Some(encoding_rs::GBK.into()),
                _ => None,
            };
            if let Some(encoding) = encoding {
                let hex: String = chars[i + 4..i + 8].iter().collect();
                if let Ok(value) = u16::from_str_radix(&hex, 16) {
                    let bytes = value.to_be_bytes();
                    let (decoded, _, had_errors) = encoding.decode(&bytes);
                    if !had_errors {
                        out.push_str(&decoded);
                        i += 8;
                        continue;
                    }
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

pub fn has_legacy_escape(text: &str) -> bool {
    text.contains("\\U+") || text.contains("\\M+")
}

/// Decode both legacy ACAD escape formats found in pre-Unicode strings.
pub fn decode_legacy_escapes(text: &str) -> String {
    decode_cif_escapes(&decode_mif_escapes(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ansi_1252() {
        let enc = encoding_from_code_page("ANSI_1252");
        assert_eq!(enc, Some(encoding_rs::WINDOWS_1252.into()));
    }

    #[test]
    fn test_case_insensitive() {
        assert_eq!(
            encoding_from_code_page("ansi_1251"),
            encoding_from_code_page("ANSI_1251")
        );
    }

    #[test]
    fn test_ascii_returns_none() {
        assert_eq!(encoding_from_code_page("ASCII"), None);
    }

    #[test]
    fn test_utf8_returns_none() {
        assert_eq!(encoding_from_code_page("UTF-8"), None);
    }

    #[test]
    fn test_unknown_returns_windows1252() {
        let enc = encoding_from_code_page("SOMETHING_UNKNOWN");
        assert_eq!(enc, Some(encoding_rs::WINDOWS_1252.into()));
    }

    #[test]
    fn test_asian_encodings() {
        assert_eq!(
            encoding_from_code_page("GB2312"),
            Some(encoding_rs::GBK.into())
        );
        assert_eq!(
            encoding_from_code_page("BIG5"),
            Some(encoding_rs::BIG5.into())
        );
        assert_eq!(
            encoding_from_code_page("ANSI_932"),
            Some(encoding_rs::SHIFT_JIS.into())
        );
        assert_eq!(
            encoding_from_code_page("KOREAN"),
            Some(encoding_rs::EUC_KR.into())
        );
    }

    #[test]
    fn unsupported_dwg_pages_use_the_declared_fallback_encoding() {
        for page in [
            "KOI8-R",
            "KOI8-U",
            "ISO8859-10",
            "ISO8859-13",
            "ISO8859-14",
            "ISO8859-15",
        ] {
            let index = dwg_code_page_index(page);
            assert_eq!(index, 30);
            assert_eq!(
                encoding_from_dwg_code_page(index),
                encoding_rs::WINDOWS_1252.into()
            );
        }
    }

    #[test]
    fn test_decode_cif_escapes() {
        assert_eq!(decode_cif_escapes("ab\\U+4E2Dcd"), "ab中cd");
        assert_eq!(decode_cif_escapes("\\U+0041\\U+0042"), "AB");
        // Exactly four hex digits: a fifth character stays literal.
        assert_eq!(decode_cif_escapes("\\U+00412"), "A2");
        // Malformed escapes stay literal.
        assert_eq!(decode_cif_escapes("\\U+GGGG"), "\\U+GGGG");
        assert_eq!(decode_cif_escapes("\\U+4E"), "\\U+4E");
        assert_eq!(decode_cif_escapes("no escapes here"), "no escapes here");
        // Invalid code points are dropped.
        assert_eq!(decode_cif_escapes("\\U+D800"), "");
        // Surrogate pair combines into a scalar.
        assert_eq!(decode_cif_escapes("\\U+D83D\\U+DE00"), "😀");
    }

    #[test]
    fn test_decode_mif_escapes() {
        assert_eq!(decode_mif_escapes(r"\M+5BCFE\M+5BAC5"), "件号");
        assert_eq!(decode_legacy_escapes(r"\M+5BCFE\U+0021"), "件!");
        assert_eq!(decode_mif_escapes(r"\M+9BCFE"), r"\M+9BCFE");
        assert_eq!(decode_mif_escapes(r"\M+5GGGG"), r"\M+5GGGG");
        // Shift-JIS as SolidWorks writes it, mixed with literal text.
        assert_eq!(
            decode_mif_escapes("\\M+18B5A术\\M+19776\\M+18B81"),
            "技术要求"
        );
        assert_eq!(decode_mif_escapes("\\M+48861"), "\u{AC00}");
    }

    #[test]
    fn test_gated_cif_decode_follows_the_code_page() {
        let w1252 = encoding_from_code_page("ANSI_1252").unwrap();
        // Held by the page: content, copied as written (hex case included).
        assert_eq!(decode_cif_escapes_in(r"94\U+00B0", w1252), r"94\U+00B0");
        assert_eq!(decode_cif_escapes_in(r"94\U+00b0", w1252), r"94\U+00b0");
        assert_eq!(
            decode_cif_escapes_in(r"\U+0041\U+0042", w1252),
            r"\U+0041\U+0042"
        );
        // Not held: transport escape, decoded, surrogate pairs as one scalar.
        assert_eq!(decode_cif_escapes_in(r"\U+2205 x", w1252), "\u{2205} x");
        assert_eq!(decode_cif_escapes_in(r"\U+D83D\U+DE00", w1252), "\u{1F600}");
        assert_eq!(
            decode_cif_escapes_in(r"\U+D83D\U+DE00", encoding_rs::SHIFT_JIS),
            "\u{1F600}"
        );
        // The same character is held by one page and not by another.
        assert_eq!(decode_cif_escapes_in(r"\U+0411", w1252), "\u{411}");
        assert_eq!(
            decode_cif_escapes_in(r"\U+0411", encoding_from_code_page("ANSI_1251").unwrap()),
            r"\U+0411"
        );
        // A bare row names no code page, so it asks nothing.
        assert_eq!(decode_cif_escapes_in(r"94\U+00B0", encoding_rs::WINDOWS_1252), "94\u{B0}");
        // UTF-8 holds every character, so its escapes are text.
        assert_eq!(
            decode_cif_escapes_in(r"\U+0041", encoding_rs::UTF_8),
            r"\U+0041"
        );
        // An OEM page is asked too.
        assert_eq!(
            decode_cif_escapes_in(r"\U+00E9", LegacyEncoding::Oem(437)),
            r"\U+00E9"
        );
    }

    #[test]
    fn test_encode_legacy_string_cif_escapes() {
        // Mappable chars encode directly.
        assert_eq!(encode_legacy_string("AB", encoding_rs::WINDOWS_1252), b"AB");
        // Unmappable chars become CIF escapes, not HTML references.
        let encoded = encode_legacy_string("中", encoding_rs::WINDOWS_1252);
        assert_eq!(String::from_utf8(encoded).unwrap(), "\\U+4E2D");
        // Astral-plane chars become a surrogate pair of escapes.
        let encoded = encode_legacy_string("😀", encoding_rs::WINDOWS_1252);
        assert_eq!(String::from_utf8(encoded).unwrap(), "\\U+D83D\\U+DE00");
        // GBK encodes Chinese directly with no escapes.
        assert_eq!(encode_legacy_string("中", encoding_rs::GBK), &[0xD6, 0xD0]);
        // Round-trip through the decoder.
        let encoded = encode_legacy_string("a中b😀c", encoding_rs::WINDOWS_1252);
        let text = String::from_utf8(encoded).unwrap();
        assert_eq!(decode_cif_escapes(&text), "a中b😀c");
    }
}
