//! Subassembly cross-section shape.
//!
//! A subassembly body keeps the shape its code last generated as a point
//! list followed by a link list, in the subassembly's own offset/elevation
//! frame:
//!
//! ```text
//! BL  point count
//! point count x { BL code count, B, B, BL, BD offset, BD elevation }
//! BL  link count
//! link count x { BL code count, B, B, BL n, n x RL point index }
//! BL  area count
//! area count x { BL code count, B, B, BL n, n x RL link index, BL }
//! ```
//!
//! The codes themselves are strings, read in that order: every point's,
//! then every link's, then every area's.
//!
//! A body may store an EMPTY shape (all three counts 0) with the
//! parameter set right behind it, and a dynamic script's body keeps one
//! combined link/area list whose areas carry an auxiliary payload (see
//! [`mixed_at`]).
//!
//! The assembly places each shape: a subassembly's origin is a point of
//! the shape it hangs from, or the assembly's baseline point.

use super::bits::Bits;
use crate::types::{Vector2, Vector3};

/// Points, links and areas of a subassembly shape, with their codes.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SubassemblyShape {
    /// (offset, elevation) of each point.
    pub points: Vec<Vector2>,
    /// Point indices each link runs through.
    pub links: Vec<Vec<u32>>,
    /// Link indices bounding each area (shape), in order.
    pub areas: Vec<Vec<u32>>,
    /// Codes of each point, link and area.
    pub point_codes: Vec<Vec<String>>,
    pub link_codes: Vec<Vec<String>>,
    pub area_codes: Vec<Vec<String>>,
}

const MAX_COUNT: i32 = 100_000;

fn count(bits: &mut Bits) -> Option<usize> {
    let n = bits.bit_long()?;
    (0..=MAX_COUNT).contains(&n).then_some(n as usize)
}

/// `BL code count, B, B`; returns the code count.
fn header(bits: &mut Bits) -> Option<usize> {
    let codes = bits.bit_long()?;
    bits.bit()?;
    bits.bit()?;
    (0..64).contains(&codes).then_some(codes as usize)
}

/// `BL code count, B, B`; returns the code count and the two flag bits.
fn header_flags(bits: &mut Bits) -> Option<(usize, bool, bool)> {
    let codes = bits.bit_long()?;
    let first = bits.bit()?;
    let second = bits.bit()?;
    (0..64).contains(&codes).then_some((codes as usize, first, second))
}

/// A decoded body: the shape, the code count of every point, link and
/// area, and where the parameter set follows.
type Body = (SubassemblyShape, Vec<usize>, Vec<usize>, Vec<usize>, usize);
/// A [`Body`] whose parameter-set position is still unknown.
type BodyProbe = (SubassemblyShape, Vec<usize>, Vec<usize>, Vec<usize>, Option<usize>);

/// Shape plus the code count of every point, link and area.
/// Also returns where the body continues, `None` when the area list did not
/// read.
fn read_at(data: &[u8], pos: usize, end: usize) -> Option<BodyProbe> {
    let mut bits = Bits::new(data, pos, end);
    let point_count = count(&mut bits)?;
    let mut shape = SubassemblyShape::default();
    let mut point_codes = Vec::with_capacity(point_count.min(1024));
    for _ in 0..point_count {
        let (codes, first, second) = header_flags(&mut bits)?;
        point_codes.push(codes);
        // A dynamic script's origin point carries flags `(0, 0)` and a raw
        // 32-bit word (0x0202_0000 in every sample) where the code index
        // would be; the word is carried verbatim.
        if (first, second) == (false, false) {
            bits.bit_long()?;
        } else {
            (0..64).contains(&bits.bit_long()?).then_some(())?;
        }
        let x = bits.bit_double()?;
        let y = bits.bit_double()?;
        (x.abs() < 1e4 && y.abs() < 1e4).then_some(())?;
        shape.points.push(Vector2::new(x, y));
    }
    let link_count = count(&mut bits)?;
    let mut link_codes = Vec::with_capacity(link_count.min(1024));
    for _ in 0..link_count {
        link_codes.push(header(&mut bits)?);
        shape.links.push(indices(&mut bits, point_count)?);
    }
    // Areas follow; a body without any keeps an empty list.
    let mut area_codes = Vec::new();
    let mut after = None;
    let mut areas_bits = Bits::new(data, bits.pos, end);
    if let Some(area_count) = areas_bits.bit_long().and_then(|n| usize::try_from(n).ok()).filter(|n| *n <= MAX_COUNT as usize) {
        let mut areas = Vec::new();
        let mut codes = Vec::new();
        let parsed = (0..area_count).all(|_| {
            let Some(k) = header(&mut areas_bits) else { return false };
            let Some(links) = indices(&mut areas_bits, link_count) else { return false };
            if areas_bits.bit_long().is_none() {
                return false;
            }
            codes.push(k);
            areas.push(links);
            true
        });
        if parsed {
            shape.areas = areas;
            area_codes = codes;
            after = Some(areas_bits.pos);
        }
    }
    Some((shape, point_codes, link_codes, area_codes, after))
}

/// `BL n, n x RL index`, every index below `limit`.
fn indices(bits: &mut Bits, limit: usize) -> Option<Vec<u32>> {
    let n = bits.bit_long()?;
    (1..=64).contains(&n).then_some(())?;
    let mut out = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let index = u32::try_from(bits.raw_long()?).ok()?;
        ((index as usize) < limit).then_some(())?;
        out.push(index);
    }
    Some(out)
}

/// Where the point list sits, read field by field. After the class header
/// (`BL 80, 0, 80, 80, 0, BL`), the 6-bit tag picks the form:
///
/// ```text
/// 010101 (keyed):  RC n, n x key (see alignment::class_keys), BL, 121 bits
/// 00xxxx (older):  20 bits, BL 1, BL n, n x key, BL, 121 bits
///                  or 20 bits, BL 0, BL 0, 128 bits
/// other (no keys): RC 0, 120 bits
/// ```
///
/// then 3BD origin, 9 BD (rotation), BD, 62 bits.
// ponytail: the 120/121/128, 20 and 62-bit runs are skipped by length
// (zero or constant in every sample; the 20 bits vary between drawings).
fn shape_at(data: &[u8], start: usize, end: usize) -> Option<usize> {
    let mut bits = Bits::new(data, start, end);
    let tag = super::alignment::class_header(&mut bits)?;
    if tag == super::alignment::KEYED {
        bits = Bits::new(data, start, end);
        super::alignment::class_keys(&mut bits)?;
        bits.pos += 121;
    } else if tag >> 4 == 0 {
        bits.skip(20)?;
        match bits.bit_long()? {
            1 => {
                for _ in 0..bits.bit_long()? {
                    super::alignment::key(&mut bits)?;
                }
                bits.bit_long()?;
                bits.pos += 121;
            }
            0 => {
                (bits.bit_long()? == 0).then_some(())?;
                bits.pos += 128;
            }
            _ => return None,
        }
    } else {
        (bits.bits_value(8)? == 0).then_some(())?;
        bits.pos += 120;
    }
    for _ in 0..13 {
        bits.bit_double()?;
    }
    Some(bits.pos + 62)
}

/// Where the codes start in a subassembly's texts: six texts after the
/// script name `Subassembly.<name>` (a stock script: three empty texts, the
/// resource module, an empty text; a package's: five empty texts).
fn codes_start(strings: &[String]) -> Option<usize> {
    // The first three texts are the class name and two empty texts; older
    // records name the script as their eighth text.
    strings.iter().enumerate().skip(3).find(|(_, s)| s.starts_with("Subassembly.")).map(|(i, _)| i + 6)
}

/// Shape, code counts and the parameter-set position of a body: the
/// standard form, or — when that refuses — the mixed link/area list of the
/// dynamic scripts ([`mixed_at`]).
fn body_at(data: &[u8], start: usize, end: usize) -> Option<Body> {
    if let Some((shape, points, links, areas, Some(after))) = read_at(data, start, end) {
        return Some((shape, points, links, areas, after));
    }
    mixed_at(data, start, end)
}

/// A dynamic script's body: the points, then ONE list whose count covers
/// links and areas. An element is `BL code count, B, B`; flags `10` mark a
/// link (standard payload), flags `01` an area whose payload is a raw
/// 32-bit word (bytes `0x0D 0x02 <element index> 0x00` and `0x09 0x02 0 0`
/// in the samples, carried verbatim) plus an auxiliary payload this crate
/// does not decode, skipped by pinned length:
///
/// ```text
/// every area but the last:  word, 48 bits
/// a 4-area body:            first word, 220 bits — areas 2 and 3, and
///                           their code texts, sit inside the residual —
///                           then the last: word, 48 bits
/// trailing BL closes the list
/// ```
///
/// An area's link indices sit in its residual, so every area reads back
/// with an empty index list next to its codes.
// ponytail: the 48- and 220-bit residuals are skipped by length (constant
// in every sample of their form); read them field by field once they vary.
fn mixed_at(data: &[u8], start: usize, end: usize) -> Option<Body> {
    let mut bits = Bits::new(data, start, end);
    let point_count = count(&mut bits)?;
    let mut shape = SubassemblyShape::default();
    let mut point_codes = Vec::with_capacity(point_count.min(1024));
    for _ in 0..point_count {
        let (codes, first, second) = header_flags(&mut bits)?;
        (first && !second).then_some(())?;
        point_codes.push(codes);
        (0..64).contains(&bits.bit_long()?).then_some(())?;
        let x = bits.bit_double()?;
        let y = bits.bit_double()?;
        (x.abs() < 1e4 && y.abs() < 1e4).then_some(())?;
        shape.points.push(Vector2::new(x, y));
    }
    let element_count = count(&mut bits)?;
    let mut link_codes = Vec::with_capacity(element_count.min(1024));
    let mut links = 0usize;
    let first = loop {
        let (codes, first, second) = header_flags(&mut bits)?;
        if first && !second {
            link_codes.push(codes);
            shape.links.push(indices(&mut bits, point_count)?);
            links += 1;
        } else {
            (!first && second).then_some(())?;
            break (codes, bits.pos);
        }
    };
    let (first_codes, first_word) = first;
    let area_count = element_count.checked_sub(links)?;
    // Every area: word, 48-bit residual; after the last one the list's
    // trailing BL.
    let areas_m1 = |data: &[u8]| -> Option<(usize, Vec<usize>, usize)> {
        let mut bits = Bits::new(data, first_word, end);
        let mut codes_list = vec![first_codes];
        let mut codes = first_codes;
        (bits.bits_value(2)? == 0).then_some(())?;
        bits.raw_long()?;
        bits.skip(48)?;
        for _ in 1..area_count {
            let (c, first, second) = header_flags(&mut bits)?;
            (!first && second).then_some(())?;
            codes_list.push(c);
            codes += c;
            (bits.bits_value(2)? == 0).then_some(())?;
            bits.raw_long()?;
            bits.skip(48)?;
        }
        bits.bit_long()?;
        Some((codes, codes_list, bits.pos))
    };
    // A 4-area body: the first area's residual is 220 bits (it hides areas
    // 2 and 3), then the last area reads like an [`areas_m1`] one.
    let areas_m2 = |data: &[u8]| -> Option<(usize, Vec<usize>, usize)> {
        (area_count == 4).then_some(())?;
        let mut bits = Bits::new(data, first_word, end);
        (bits.bits_value(2)? == 0).then_some(())?;
        bits.raw_long()?;
        bits.skip(220)?;
        let (codes_last, first, second) = header_flags(&mut bits)?;
        (!first && second).then_some(())?;
        let codes_list = vec![first_codes, codes_last];
        (bits.bits_value(2)? == 0).then_some(())?;
        bits.raw_long()?;
        bits.skip(48)?;
        bits.bit_long()?;
        Some((first_codes + codes_last, codes_list, bits.pos))
    };
    let (_, area_codes, after) = areas_m1(data).or_else(|| areas_m2(data))?;
    shape.areas = vec![Vec::new(); area_codes.len()];
    Some((shape, point_codes, link_codes, area_codes, after))
}

/// Shape from the point list, with codes from `strings`, the record's text
/// stream (see `codes_start`).
pub(super) fn find(data: &[u8], start: usize, end: usize, strings: &[String]) -> Option<SubassemblyShape> {
    // A header in none of the forms of [`shape_at`] gives no shape.
    let (mut shape, points, links, areas, _) = body_at(data, shape_at(data, start, end)?, end)?;
    let first = codes_start(strings);
    let mut codes = first.map(|i| strings[i.min(strings.len())..].iter());
    let mut take = |counts: &[usize]| -> Vec<Vec<String>> {
        counts
            .iter()
            .map(|&k| match codes.as_mut() {
                Some(iter) => iter.by_ref().take(k).cloned().collect(),
                None => Vec::new(),
            })
            .collect()
    };
    shape.point_codes = take(&points);
    shape.link_codes = take(&links);
    shape.area_codes = take(&areas);
    Some(shape)
}

/// Input parameters a subassembly stores after its shape, texts taken after
/// the shape's codes. A mixed-form body (see [`mixed_at`]) hides some of
/// its area codes inside the first area's residual; their count follows
/// from the text stream, which keeps exactly one unused text after the
/// parameter set.
pub(super) fn parameters(data: &[u8], start: usize, end: usize, strings: &[String]) -> Option<Vec<super::parameters::Parameter>> {
    let first = codes_start(strings)?;
    let landing = shape_at(data, start, end)?;
    let (points, links, areas, after, hidden) = match read_at(data, landing, end) {
        Some((_, points, links, areas, Some(after))) => (points, links, areas, after, false),
        _ => {
            let (_, points, links, areas, after) = mixed_at(data, landing, end)?;
            (points, links, areas, after, true)
        }
    };
    let codes: usize = points.iter().chain(&links).chain(&areas).sum();
    let mut base = first + codes;
    let mut bits = Bits::new(data, after, end);
    if hidden {
        let consumed = super::parameters::text_count(&mut bits)?;
        base += strings.len().checked_sub(1)?.checked_sub(consumed)?.checked_sub(base)?;
        bits = Bits::new(data, after, end);
    }
    let mut texts = strings.get(base..)?.iter();
    super::parameters::read(&mut bits, &mut texts)
}

/// Where a subassembly sits in its assembly.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Attachment {
    pub subassembly: u64,
    /// Subassembly it hangs from, 0 for the assembly baseline.
    pub parent: u64,
    /// Index into the parent shape's points; its origin is that point.
    pub point: u32,
}

/// Baseline point and subassembly tree of an assembly.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AssemblyLayout {
    /// Drawing position of the baseline point.
    pub origin: Vector3,
    pub attachments: Vec<Attachment>,
}

/// An attachment record (`v` is 70 in Civil 3D 2013 drawings, 80 from
/// 2018). Returns `v`.
fn attachment_tag(bits: &mut Bits) -> Option<i16> {
    let v = bits.bit_short()?;
    (matches!(v, 70 | 80) && bits.bit_short()? == v && bits.bit_short()? == 0).then_some(())?;
    // `B 1, BL 240` from 2018; Civil 3D 2013 writes `B 0` and `00`
    // followed by the byte 0x40 in the same place.
    let set = bits.bit()?;
    let code = (u8::from(bits.bit()?) << 1) | u8::from(bits.bit()?);
    let byte = bits.raw_short_byte()?;
    (matches!((set, code, byte), (true, 1, 240) | (false, 0, 0x40))).then_some(())?;
    let flags = [bits.bit()?, bits.bit()?, bits.bit()?, bits.bit()?, bits.bit()?];
    (flags == [false, false, true, false, false]).then_some(v)
}

/// Parent point indices in subassembly order, read after the baseline
/// point: 160 bits, then lists until an empty one, each `BL n` and `n`
/// nodes (two attachment records, `BL point`, `BL`, `BL`), then 36 bits to
/// the end of the body.
// ponytail: the 160 and 36 bits are skipped by length (the same length in
// every assembly seen); read them field by field once a drawing varies them.
fn attachment_points(data: &[u8], pos: usize, end: usize) -> Option<Vec<u32>> {
    let mut bits = Bits::new(data, pos, end);
    (bits.remaining() >= 160).then_some(())?;
    bits.pos += 160;
    let mut points = Vec::new();
    loop {
        let n = bits.bit_long()?;
        if n == 0 {
            break;
        }
        for _ in 0..n {
            attachment_tag(&mut bits)?;
            attachment_tag(&mut bits)?;
            points.push(u32::try_from(bits.bit_long()?).ok()?);
            bits.bit_long()?;
            bits.bit_long()?;
        }
    }
    (bits.remaining() == 36).then_some(points)
}

/// The baseline point is a `3BD` after the class header
/// (`BL 80, 0, 80, 80, 0`, two BLs, a flag BL): with flag 64 after the
/// date-modified block (`BL, BL, BL 0x1600101F`, 64-bit FILETIME) and 131
/// bits; with flag 128 after 98 bits.
// ponytail: the 131 and 98 bits are skipped by length (constant in every
// assembly seen); read them field by field once a drawing varies them.
pub(super) fn assembly_layout(data: &[u8], start: usize, end: usize, handles: &[u64]) -> Option<AssemblyLayout> {
    let mut bits = Bits::new(data, start, end);
    for want in [80, 0, 80, 80, 0] {
        (bits.bit_long()? == want).then_some(())?;
    }
    bits.bit_long()?;
    bits.bit_long()?;
    match bits.bit_long()? {
        64 => {
            bits.bit_long()?;
            bits.bit_long()?;
            (bits.bit_long()? == 0x1600_101F).then_some(())?;
            bits.pos += 64 + 131;
        }
        128 => bits.pos += 98,
        _ => return None,
    }
    let origin = Vector3::new(bits.bit_double()?, bits.bit_double()?, bits.bit_double()?);
    [origin.x, origin.y, origin.z].iter().all(|v| v.is_finite()).then_some(())?;
    // Body handles: two nulls (one in Civil 3D 2013), the baseline style,
    // (subassembly, parent) pairs, then the drawing's Civil root.
    let style = handles.iter().take(3).position(|h| *h != 0)?;
    let pairs = handles.get(style + 1..handles.len().checked_sub(1)?)?;
    let points = attachment_points(data, bits.pos, end)?;
    if pairs.len() % 2 != 0 || pairs.len() / 2 != points.len() {
        return None;
    }
    let attachments = pairs
        .chunks(2)
        .zip(points)
        .map(|(pair, point)| Attachment {
            subassembly: pair[0],
            parent: pair[1],
            point,
        })
        .collect();
    Some(AssemblyLayout { origin, attachments })
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::parameters::ParameterValue;

    fn bits_of(s: &str) -> Vec<u8> {
        let s: Vec<u8> = s.bytes().filter(|b| *b == b'0' || *b == b'1').collect();
        s.chunks(8).map(|c| c.iter().enumerate().fold(0u8, |a, (i, b)| a | ((b - b'0') << (7 - i)))).collect()
    }

    #[test]
    fn finds_the_point_list_after_an_older_header() {
        // Class header, tag 000101, 20 bits, BL 1, BL 2, two keys
        // (0x0404D01E, 0x0404D020), BL, 121 bits, 13 BD, 62 bits.
        let keyed = "01010100001001010100000101010000100000000100010111110000001000010000 000101 10111000011100000000 \n             0100000001 0100000010 0000011110110100000000010000000100 \n             0000100000110100000000010000000100 0100000000 0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000100101010010110101101101 \n             10101001101010011010100101 00001000010101000001010100000000000000000000001011111000000100";
        // Class header, tag 000100, 20 bits, BL 0, BL 0, 128 bits, 13 BD,
        // 62 bits.
        let bare = "01010100001001010100000101010000100000000100010111110000001000010000 000100 00000000000010000000 \n             0100000000 10 00001010000000000000000000000000111111111111111111111111111111110000000100000000000000000000000000000000000000000000000000000000 \n             10101001101010011010100101 00001000010101000001010100000000000000000000001011111000000100";
        for (s, at) in [(keyed, 401), (bare, 322)] {
            let b = bits_of(s);
            assert_eq!(shape_at(&b, 0, b.len() * 8), Some(at));
        }
        // Another value where the form holds BL 1 or BL 0 is not this form.
        let other = bare.replacen("0100000000 10", "0100000010 10", 1);
        let b = bits_of(&other);
        assert_eq!(shape_at(&b, 0, b.len() * 8), None);
    }

    /// The class header, origin and rotation of the `bare` fixture, whose
    /// point list would land at 322.
    const PREAMBLE: &str = "01010100001001010100000101010000100000000100010111110000001000010000 000100 00000000000010000000 \n             0100000000 10 00001010000000000000000000000000111111111111111111111111111111110000000100000000000000000000000000000000000000000000000000000000 \n             10101001101010011010100101 00001000010101000001010100000000000000000000001011111000000100";

    /// One integer parameter `ParName` = 1 (type 0x1012, no choices).
    const PARAMS: &str = "10 10 011010 01 00000001 00 00010010 00010000 00000000 00000000 10 01 00000001 0 0 10 10 10";

    /// A dynamic script's origin point: flags `00`, then a BL-carried raw
    /// 32-bit word (`00`, bytes 00 00 02 02 little-endian = 0x0202_0000 in
    /// the samples) where the code index would be.
    const WORD_POINT: &str = "10 00 00 00000000 00000000 00000010 00000010 10 10";

    fn two_points() -> String {
        // Two points, one code each, (0, 0) and (1, 0).
        "01 00000010 01 00000001 10 10 10 10 01 00000001 10 10 01 10".into()
    }

    fn texts(extra: &[&str]) -> Vec<String> {
        // The class name, two empties, the script, five empties — then the
        // caller's codes and parameter texts.
        ["AeccDbSubassembly", "", "", "Subassembly.Test", "", "", "", "", ""]
            .iter()
            .chain(extra)
            .map(|s| s.to_string())
            .collect()
    }

    #[test]
    fn reads_an_empty_shape_with_parameters() {
        // The no-shape form: three empty counts, then the parameter set.
        let mut s = PREAMBLE.to_string();
        s.push_str(" 10 10 10 ");
        s.push_str(PARAMS);
        let b = bits_of(&s);
        let strings = texts(&["ParName", "ParDisplay", "ParDesc", "", "END"]);
        let shape = find(&b, 0, b.len() * 8, &strings).unwrap();
        assert_eq!((shape.points.len(), shape.links.len(), shape.areas.len()), (0, 0, 0));
        let params = parameters(&b, 0, b.len() * 8, &strings).unwrap();
        assert_eq!(params.len(), 1);
        assert_eq!((params[0].name.as_str(), params[0].value.clone()), ("ParName", ParameterValue::Integer(1)));
    }

    #[test]
    fn reads_the_word_point() {
        // A ConditionalCutOrFill-style body: the first point carries the
        // raw word, the second and the link are standard.
        let mut s = PREAMBLE.to_string();
        s.push_str(" 01 00000010 ");
        s.push_str(WORD_POINT);
        s.push_str(" 01 00000001 10 10 01 10 ");
        s.push_str("01 00000001 01 00000001 10 01 00000010 00000000 00000000 00000000 00000000 00000001 00000000 00000000 00000000 ");
        s.push_str("10 ");
        s.push_str(PARAMS);
        let b = bits_of(&s);
        let strings = texts(&["P1", "L0", "ParName", "ParDisplay", "ParDesc", "", "END"]);
        let shape = find(&b, 0, b.len() * 8, &strings).unwrap();
        assert_eq!(shape.points.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>(), [(0.0, 0.0), (1.0, 0.0)]);
        assert_eq!(shape.links, vec![vec![0, 1]]);
        let params = parameters(&b, 0, b.len() * 8, &strings).unwrap();
        assert_eq!(params[0].name, "ParName");
    }

    /// Residual payload after an area's word, from DaylightBasin 1BA4
    /// (Assembly-2a.dwg): 48 bits, skipped verbatim.
    const BASIN_RESIDUAL: &str = "000000000000000000000101000000000000000000000000";
    /// The same, from the last of Basin 1BA4's three areas.
    const BASIN_LAST_RESIDUAL: &str = "000000000000000000000111000000000000000000000000";
    /// The 220-bit residual of DaylightStandard 1B49's (Corridor-3b.dwg)
    /// first area — areas 2 and 3, and their code texts, sit inside it.
    const STANDARD_RESIDUAL: &str = "0000000000000000000000010000000000000000000000000100000011100100000010000000000000000000000000000000000000001000000000000000000000000001000000111001000000100000001000000000000000000000000000000011000000000000000000000000";
    /// The last area's residual of the same record.
    const STANDARD_LAST_RESIDUAL: &str = "000000000000000000000100000000000000000000000000";
    /// An area's word: `BL(raw)`, bytes 0x0D 0x02 <element index> 0x00.
    fn area_word(index: u32) -> String {
        format!("00 00001101 00000010 {index:08b} 00000000")
    }

    #[test]
    fn reads_the_mixed_link_area_list() {
        // Basin-style: one list of a link and two areas, every area's
        // header visible, each word followed by 48 residual bits.
        let mut s = PREAMBLE.to_string();
        s.push_str(&two_points());
        s.push_str(" 01 00000011 ");
        s.push_str("01 00000001 10 01 00000010 00000000 00000000 00000000 00000000 00000001 00000000 00000000 00000000 ");
        s.push_str("01 00000001 01 ");
        s.push_str(&area_word(4));
        s.push(' ');
        s.push_str(BASIN_RESIDUAL);
        s.push_str(" 01 00000010 01 ");
        s.push_str(&area_word(6));
        s.push(' ');
        s.push_str(BASIN_LAST_RESIDUAL);
        s.push_str(" 10 ");
        s.push_str(PARAMS);
        let b = bits_of(&s);
        let strings = texts(&["P1Code", "P2Code", "L1Code", "A1Code", "A2Code1", "A2Code2", "ParName", "ParDisplay", "ParDesc", "", "END"]);
        let shape = find(&b, 0, b.len() * 8, &strings).unwrap();
        assert_eq!(shape.points.len(), 2);
        assert_eq!(shape.links, vec![vec![0, 1]]);
        // The areas' link indices sit in the skipped residuals.
        assert_eq!(shape.areas, vec![Vec::<u32>::new(), Vec::new()]);
        assert_eq!(shape.area_codes, vec![vec!["A1Code".to_string()], vec!["A2Code1".to_string(), "A2Code2".to_string()]]);
        let params = parameters(&b, 0, b.len() * 8, &strings).unwrap();
        assert_eq!(params[0].name, "ParName");
    }

    #[test]
    fn reads_the_four_area_body() {
        // Standard-style: 4 areas, the first one's residual hides areas 2
        // and 3; their code texts are counted from the text stream, which
        // keeps one unused text behind the parameters.
        let mut s = PREAMBLE.to_string();
        s.push_str(&two_points());
        s.push_str(" 01 00000100 ");
        s.push_str("01 00000001 01 ");
        s.push_str(&area_word(0));
        s.push(' ');
        s.push_str(STANDARD_RESIDUAL);
        s.push_str(" 01 00000010 01 ");
        s.push_str(&area_word(3));
        s.push(' ');
        s.push_str(STANDARD_LAST_RESIDUAL);
        s.push_str(" 10 ");
        s.push_str(PARAMS);
        let b = bits_of(&s);
        let strings = texts(&[
            "P1Code", "P2Code", "A1Code", "HidCode1", "HidCode2", "A4Code1", "A4Code2", "ParName", "ParDisplay", "ParDesc", "", "END",
        ]);
        let shape = find(&b, 0, b.len() * 8, &strings).unwrap();
        assert_eq!(shape.points.len(), 2);
        assert_eq!(shape.areas, vec![Vec::<u32>::new(), Vec::new()]);
        let params = parameters(&b, 0, b.len() * 8, &strings).unwrap();
        // The parameters start behind the hidden code texts: the first one
        // is the text two behind the last visible area's codes.
        assert_eq!(params[0].name, "ParName");
    }

    #[test]
    fn refuses_a_body_whose_residual_does_not_fit() {
        // One bit short in the last area's residual: neither form reads.
        let mut s = PREAMBLE.to_string();
        s.push_str(&two_points());
        s.push_str(" 01 00000011 ");
        s.push_str("01 00000001 10 01 00000010 00000000 00000000 00000000 00000000 00000001 00000000 00000000 00000000 ");
        s.push_str("01 00000001 01 ");
        s.push_str(&area_word(4));
        s.push(' ');
        s.push_str(BASIN_RESIDUAL);
        s.push_str(" 01 00000010 01 ");
        s.push_str(&area_word(6));
        s.push(' ');
        s.push_str(&BASIN_LAST_RESIDUAL[..47]);
        // The body ends one bit into the last residual — no parameter set
        // follows, so the missing bit is past `end`.
        let b = bits_of(&s);
        let strings = texts(&["P1Code", "P2Code", "L1Code", "A1Code", "A2Code1", "A2Code2", "ParName", "ParDisplay", "ParDesc", "", "END"]);
        assert_eq!(find(&b, 0, b.len() * 8, &strings), None);
        assert_eq!(parameters(&b, 0, b.len() * 8, &strings), None);
    }
}
