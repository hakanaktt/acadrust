//! Corridor feature lines.
//!
//! A corridor body stores its feature lines as groups, one per point code
//! in the order of the corridor's code list:
//!
//! ```text
//! BL  segment count
//! segment count x { BL n, n x { BD station, BD x, BD y, BD z, B } }
//! ```
//!
//! Points are in world coordinates, one record per corridor station. A
//! corridor with several baselines keeps one such run of groups per
//! baseline, after that baseline's region and station records (see
//! [`walk`]).

use super::bits::Bits;
use crate::types::Vector3;

type Groups = Vec<Vec<Vec<(f64, Vector3)>>>;

/// One `4BD` station record, `None` past the end or when implausible.
fn record(bits: &mut Bits) -> Option<(f64, Vector3)> {
    let station = bits.bit_double()?;
    let point = Vector3::new(bits.bit_double()?, bits.bit_double()?, bits.bit_double()?);
    let plausible = station.is_finite()
        && station.abs() < 1e7
        && point.x.is_finite()
        && point.y.is_finite()
        && point.x.abs() < 1e9
        && point.y.abs() < 1e9
        && point.z.abs() < 1e5
        && (point.x.abs() > 1.0 || point.y.abs() > 1.0);
    plausible.then_some((station, point))
}

/// `BL n`, then `n` records each followed by a flag bit. Stations rise and
/// neighbors stay close, which rejects random bits that happen to parse.
fn segment(bits: &mut Bits) -> Option<Vec<(f64, Vector3)>> {
    let n = usize::try_from(bits.bit_long()?).ok()?;
    if n == 0 || n > 100_000 {
        return None;
    }
    let mut out: Vec<(f64, Vector3)> = Vec::with_capacity(n.min(4096));
    for _ in 0..n {
        let record = record(bits)?;
        bits.bit()?;
        if let Some(last) = out.last() {
            let step = record.0 - last.0;
            if !(0.0..=1000.0).contains(&step) || record.1.distance(&last.1) > step * 3.0 + 50.0 {
                return None;
            }
        }
        out.push(record);
    }
    Some(out)
}

/// `BL` segment count, then the segments.
fn group(bits: &mut Bits, first: bool) -> Option<Vec<Vec<(f64, Vector3)>>> {
    let n = usize::try_from(bits.bit_long()?).ok()?;
    if n == 0 || n > 1000 {
        return None;
    }
    let segments: Vec<_> = (0..n).map(|_| segment(bits)).collect::<Option<_>>()?;
    // A lone record is too weak to anchor the first group.
    (!first || segments.iter().map(Vec::len).sum::<usize>() >= 2).then_some(segments)
}

/// Feature lines of a corridor per baseline: each baseline's groups, one
/// per point code in the order of the corridor's code list, each group
/// holding that code's segments.
pub(super) fn baselines(data: &[u8], start: usize, end: usize) -> Vec<Groups> {
    if let Some(found) = walk(&mut Bits::new(data, start, end)) {
        return found;
    }
    // debt: records [`walk`] does not read are found by probing bit offsets
    // for the first group of each baseline: one in the samples, whose
    // surface (two link codes, older layout) is followed by the 32 right
    // after its BL 0, without the 2 bits before it.
    probe(data, start, end)
}

fn probe(data: &[u8], start: usize, end: usize) -> Vec<Groups> {
    let mut out = Vec::new();
    let mut from = start;
    while let Some(mut pos) = (from..end).find(|&pos| group(&mut Bits::new(data, pos, end), true).is_some()) {
        let mut groups = Vec::new();
        loop {
            let mut bits = Bits::new(data, pos, end);
            match group(&mut bits, groups.is_empty()) {
                Some(found) => {
                    groups.push(found);
                    pos = bits.pos;
                }
                None => break,
            }
        }
        out.push(groups);
        from = pos;
    }
    out
}

/// A corridor surface's head after its GUID:
///
/// ```text
/// B, B built, RC 0x20 or 0xA0
/// 3 bits n (link codes), n x B (the code is added as a breakline)
/// BL 0, BL m (feature line codes), m x B
/// BL overhang correction (0 none, 1 top links)
/// ```
// ponytail: the RC is read as 0x20 or 0xA0 (a file rebuilt by the program
// turns 0x20 into 0xA0 and leaves the rest alike); its bits are not named.
fn surface(bits: &mut Bits) -> Option<()> {
    bits.bit()?;
    bits.bit()?;
    matches!(bits.raw_short_byte()?, 0x20 | 0xA0).then_some(())?;
    let codes = usize::from(bits.bits_value(3)?);
    bits.skip(codes)?;
    // debt: this BL has no other value in the samples; what follows a
    // different one is not mapped.
    (bits.bit_long()? == 0).then_some(())?;
    let feature_codes = usize::try_from(bits.bit_long()?).ok().filter(|n| *n <= bits.remaining())?;
    bits.skip(feature_codes)?;
    bits.bit_long().map(|_| ())
}

/// The surfaces' boundaries and masks: `BL n` (one per surface that has
/// any), `n x { BL k, k x { BL kind, BL m, items } }`. Kinds with a flag
/// above the low 16 bits hold `m x 3BD` corners: 0x80000002 the corridor
/// extents (its outline in some files, none in others), 0x40000002 an
/// outside and 0x40000001 an inside boundary from a polygon, 0x40000004 a
/// mask. Kinds without flags, 2 outside and 1 inside, are drawn along
/// feature lines and hold `m` components:
///
/// ```text
/// BL, BL feature line, B reversed, BD start station, BD end station,
/// B start at the line's start, B end at its end, B, GUID
/// ```
fn outlines(bits: &mut Bits) -> Option<()> {
    for _ in 0..bits.bit_long()? {
        for _ in 0..bits.bit_long()? {
            let kind = bits.bit_long()? as u32;
            let m = usize::try_from(bits.bit_long()?).ok().filter(|m| m * 6 <= bits.remaining())?;
            if kind < 0x1_0000 {
                for _ in 0..m {
                    bits.bit_long()?;
                    bits.bit_long()?;
                    bits.bit()?;
                    bits.bit_double()?;
                    bits.bit_double()?;
                    bits.skip(3)?;
                    bits.skip(128)?;
                }
            } else {
                for _ in 0..3 * m {
                    bits.bit_double()?;
                }
            }
        }
    }
    Some(())
}

/// The record read field by field:
///
/// ```text
/// head:     BL 80, 0, 80, 80, 0, BL, BL, 4 bits, RC n, n x key,
///           2 bits, GUID (zero in newer files), 2 bits, RC 82, BL n,
///           n x { 2 bits, BL }, BL surfaces,
///           surfaces x { GUID, head (see [`surface`]) }, BL 0, outlines
///           (see [`outlines`]),
///           BL 0, 2 bits, RC 32, RC layout, RC baselines (top 5 bits), 1 bit,
///           6 bits region count, BL record count (both of the first
///           baseline's first region)
/// layout:   0x28: each region block holds a GUID before its start and its
///           end station; 0x48: it does not
/// baseline: [after the first: 2 bits, BL n, n x BD (none in newer files),
///           BL region count, BL record count], regions, then B, GUID,
///           BL n, n x feature line group, B, B, GUID
/// region:   records (see [`station`]), then its block (see [`region`]),
///           which gives the next region's record count, 1 after the last;
///           the block's target entries hold as many lists as the region's
///           last applied subassembly has words, less 8 (10
///           of 18 in the samples)
/// ```
// ponytail: the 4-bit run, the 2-bit runs around the head's GUID (01 before
// it, 01 or 11 after it), before the 32 (01 or 11) and opening each later
// baseline (00, 01, 10 or 11), the low 3 bits of the second RC and the bit
// after it are skipped by length.
fn walk(bits: &mut Bits) -> Option<Vec<Groups>> {
    for want in [80, 0, 80, 80, 0] {
        (bits.bit_long()? == want).then_some(())?;
    }
    bits.bit_long()?;
    bits.bit_long()?;
    bits.skip(4)?;
    for _ in 0..bits.bits_value(8)? {
        super::alignment::key(bits)?;
    }
    bits.skip(2 + 128 + 2)?;
    (bits.raw_short_byte()? == 82).then_some(())?;
    for _ in 0..bits.bit_long()? {
        bits.skip(2)?;
        bits.bit_long()?;
    }
    let surfaces = bits.bit_long()?;
    (0..=64).contains(&surfaces).then_some(())?;
    for _ in 0..surfaces {
        bits.skip(128)?;
        surface(bits)?;
    }
    // debt: this BL has no other value in the samples.
    (bits.bit_long()? == 0).then_some(())?;
    outlines(bits)?;
    (bits.bit_long()? == 0).then_some(())?;
    bits.skip(2)?;
    (bits.raw_short_byte()? == 32).then_some(())?;
    let guids = match bits.raw_short_byte()? {
        0x28 => true,
        0x48 => false,
        _ => return None,
    };
    let baselines = bits.raw_short_byte()? >> 3;
    bits.skip(1)?;
    let mut regions = u32::from(bits.bits_value(6)?);
    let mut records = u32::try_from(bits.bit_long()?).ok()?;
    let mut out = Vec::new();
    for baseline in 0..baselines {
        if baseline > 0 {
            bits.skip(2)?;
            for _ in 0..bits.bit_long()? {
                bits.bit_double()?;
            }
            regions = u32::try_from(bits.bit_long()?).ok()?;
            records = u32::try_from(bits.bit_long()?).ok()?;
        }
        for _ in 0..regions {
            let mut words = None;
            for index in 0..records {
                words = station(bits, index > 0)?.or(words);
            }
            records = region(bits, guids, words)?;
        }
        // A last baseline whose regions give no record count has nothing
        // built: no feature line groups follow.
        if records == 0 && baseline + 1 == baselines {
            out.push(Vec::new());
            break;
        }
        (records == 1).then_some(())?;
        bits.bit()?;
        bits.skip(128)?;
        let n = bits.bit_long()?;
        out.push((0..n).map(|_| fields_group(bits)).collect::<Option<Groups>>()?);
        bits.bit()?;
        bits.bit()?;
        bits.skip(128)?;
    }
    Some(out)
}

/// `BL 80, BL 80, BL 0x04BE0000`, the head of each nested object.
fn object(bits: &mut Bits) -> Option<()> {
    (bits.bit_long()? == 80).then_some(())?;
    object_rest(bits)
}

fn object_rest(bits: &mut Bits) -> Option<()> {
    (bits.bit_long()? == 80 && bits.bit_long()? == 0x04BE_0000).then_some(())
}

/// One station: `[after a region's first: B], BD station, GUID, BL, BL, BL,
/// BL`, assembly groups (see [`assembly`]), `BL n`, `n x` applied
/// subassembly (see [`body`]), `BL 19`. Returns the last body's word count.
fn station(bits: &mut Bits, after_first: bool) -> Option<Option<u32>> {
    if after_first {
        bits.bit()?;
    }
    bits.bit_double()?;
    bits.skip(128)?;
    for _ in 0..4 {
        bits.bit_long()?;
    }
    assembly(bits)?;
    let mut words = None;
    for _ in 0..bits.bit_long()? {
        words = Some(body(bits)?);
    }
    (bits.bit_long()? == 19).then_some(words)
}

/// The assembly's groups of nodes `object, object, BL kind, BD, BD`; after a node
/// `BL 80` opens the group's next node (the rest of its first object head
/// follows), `BL 0` ends, any other value up to 79 opens the next group (1,
/// 2 and 3 in the samples).
fn assembly(bits: &mut Bits) -> Option<()> {
    loop {
        object(bits)?;
        loop {
            object(bits)?;
            bits.bit_long()?;
            bits.bit_double()?;
            bits.bit_double()?;
            match bits.bit_long()? {
                80 => object_rest(bits)?,
                0 => return Some(()),
                1..=79 => break,
                _ => return None,
            }
        }
    }
}

/// Words of an applied subassembly before its end.
const WORDS: u32 = 18;

/// One applied subassembly: `object, BL 8, object`, its shape, its parameter
/// set, then 18 words, each a count of target items or `0x8004xxxx` with no
/// items, and `BL -1` (0 when the shape and parameter set are empty).
/// Returns the number of words.
///
/// ```text
/// shape:  BL n, n x { BL, BL, BD station, BD offset, BD elevation }
///         BL m, m x { BL code, B, B, BL k, k x RL point }
///         BL a, a x { BL code, B, B, BL k, k x RL link, BL }
/// ```
// ponytail: the word list is read as 18 words and its end; every sample
// (all versions, 39084 lists) has 18. A list ending earlier with -1 is still
// read.
fn body(bits: &mut Bits) -> Option<u32> {
    object(bits)?;
    (bits.bit_long()? == 8).then_some(())?;
    object(bits)?;
    for _ in 0..bits.bit_long()? {
        bits.bit_long()?;
        bits.bit_long()?;
        for _ in 0..3 {
            bits.bit_double()?;
        }
    }
    for areas in [false, true] {
        for _ in 0..bits.bit_long()? {
            bits.bit_long()?;
            bits.bit()?;
            bits.bit()?;
            for _ in 0..bits.bit_long()? {
                bits.raw_long()?;
            }
            if areas {
                bits.bit_long()?;
            }
        }
    }
    super::parameters::read_with(bits, &mut || Some(String::new()))?;
    let mut words = 0;
    loop {
        if words == WORDS {
            return matches!(bits.bit_long()?, -1 | 0).then_some(words);
        }
        let count = bits.bit_long()?;
        match count {
            -1 => return Some(words),
            0.. => {
                for _ in 0..count {
                    target(bits)?;
                }
            }
            // ponytail: the low half of the word is skipped (0 or 4 in the
            // samples).
            _ => ((count as u32) >> 16 == 0x8004).then_some(())?,
        }
        words += 1;
    }
}

/// A target item: `BL type, BL 0`, then by the type's low byte, less its
/// 0x20 flag
///
/// ```text
/// 0x12: BL value
/// 0x13: BD value
/// 0x14: nothing (a text in the text stream)
/// 0x15: object
/// 0x17: BL k, k x object, BL
/// ```
///
/// then `B, B choices, [BL n, n x BL], BL index`. Types carry flags above
/// the low byte (0x1015, 0x1017, 0x1117, 0x20001117, 0x1033 in the samples).
fn target(bits: &mut Bits) -> Option<()> {
    let kind = bits.bit_long()? & 0xDF;
    (bits.bit_long()? == 0).then_some(())?;
    match kind {
        0x12 => {
            bits.bit_long()?;
        }
        0x13 => {
            bits.bit_double()?;
        }
        0x14 => {}
        0x15 => object(bits)?,
        0x17 => {
            for _ in 0..bits.bit_long()? {
                object(bits)?;
            }
            bits.bit_long()?;
        }
        _ => return None,
    }
    bits.bit()?;
    if bits.bit()? {
        for _ in 0..bits.bit_long()? {
            bits.bit_long()?;
        }
    }
    bits.bit_long().map(|_| ())
}

/// A region's block after its stations, returning the record count it gives
/// for what follows:
///
/// ```text
/// B, GUID, BL, BL, BL, BL, assembly groups, [GUID], BD start station,
/// [GUID], BD end station, 2 bits, BD, BD, BL, BD, BD, BD, GUID, BL, BL,
/// BL, BL n, n x `lists` target lists, BL m, m x BD station, B, B, B,
/// BL count
/// ```
// ponytail: the 2-bit run is skipped by length (00 in every sample).
fn region(bits: &mut Bits, guids: bool, words: Option<u32>) -> Option<u32> {
    bits.bit()?;
    bits.skip(128)?;
    for _ in 0..4 {
        bits.bit_long()?;
    }
    assembly(bits)?;
    for _ in 0..2 {
        if guids {
            bits.skip(128)?;
        }
        bits.bit_double()?;
    }
    bits.skip(2)?;
    bits.bit_double()?;
    bits.bit_double()?;
    bits.bit_long()?;
    for _ in 0..3 {
        bits.bit_double()?;
    }
    bits.skip(128)?;
    for _ in 0..3 {
        bits.bit_long()?;
    }
    let entries = bits.bit_long()?;
    let lists = match entries {
        0 => 0,
        _ => words?.checked_sub(8)?,
    };
    for _ in 0..entries {
        for _ in 0..lists {
            for _ in 0..bits.bit_long()? {
                target(bits)?;
            }
        }
    }
    for _ in 0..bits.bit_long()? {
        bits.bit_double()?;
    }
    bits.skip(3)?;
    u32::try_from(bits.bit_long()?).ok()
}

/// A feature line group read by its counts, without the probe's checks.
fn fields_group(bits: &mut Bits) -> Option<Vec<Vec<(f64, Vector3)>>> {
    let n = bits.bit_long()?;
    (0..n)
        .map(|_| {
            let n = usize::try_from(bits.bit_long()?).ok().filter(|n| n * 9 <= bits.remaining())?;
            (0..n)
                .map(|_| {
                    let station = bits.bit_double()?;
                    let point = Vector3::new(bits.bit_double()?, bits.bit_double()?, bits.bit_double()?);
                    bits.bit()?;
                    Some((station, point))
                })
                .collect()
        })
        .collect()
}

/// Feature line groups of every baseline, one after another.
pub(super) fn feature_line_groups(data: &[u8], start: usize, end: usize) -> Vec<Vec<Vec<(f64, Vector3)>>> {
    baselines(data, start, end).into_iter().flatten().collect()
}

/// Every feature line segment of a corridor.
pub(super) fn feature_lines(data: &[u8], start: usize, end: usize) -> Vec<Vec<(f64, Vector3)>> {
    feature_line_groups(data, start, end).into_iter().flatten().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits_of(s: &str) -> Vec<u8> {
        let s: Vec<u8> = s.bytes().filter(|b| *b == b'0' || *b == b'1').collect();
        s.chunks(8).map(|c| c.iter().enumerate().fold(0u8, |a, (i, b)| a | ((b - b'0') << (7 - i)))).collect()
    }

    #[test]
    fn reads_a_boundary_along_feature_lines() {
        // One list with one outside boundary (kind 2) of two components:
        // {BL 0, BL 0, B 0, BD 0, BD 0, B 1, B 1, B 0, GUID} and the same
        // on feature line 1, reversed.
        let zeros = "0".repeat(128);
        let s = format!(
            "0100000001 0100000001 0100000010 0100000010 10 10 0 10 10 110 {zeros} 10 0100000001 1 10 10 110 {zeros}"
        );
        let b = bits_of(&s);
        let len = s.bytes().filter(|c| *c == b'0' || *c == b'1').count();
        let mut bits = Bits::new(&b, 0, len);
        assert_eq!(outlines(&mut bits), Some(()));
        assert_eq!(bits.pos, len);
    }

    #[test]
    fn reads_a_surface_with_two_link_codes_and_an_outline() {
        // B, B built, RC 0x20; two link codes, both breaklines; BL 0, one
        // feature line code; BL 0 overhang; then BL 0 and one list with one
        // extents outline of one corner.
        let s = "0 1 00100000 010 1 1 10 0100000001 1 10 10 01 00000001 01 00000001
                 00 00000010 00000000 00000000 10000000 01 00000001 10 10 10";
        let b = bits_of(s);
        let len = s.bytes().filter(|c| *c == b'0' || *c == b'1').count();
        let mut bits = Bits::new(&b, 0, len);
        assert_eq!(surface(&mut bits), Some(()));
        assert_eq!(bits.bit_long(), Some(0));
        assert_eq!(outlines(&mut bits), Some(()));
        assert_eq!(bits.pos, len);
    }

    #[test]
    fn reads_assembly_groups_and_a_target() {
        let bl80 = "01 01010000";
        let rest = "00 00000000 00000000 10111110 00000100";
        let object = format!("{bl80} {bl80} {rest}");
        let zeros = "10 10 10";
        // Group 1 with two nodes (BL 80 continues it), BL 1, group 2 with
        // one node, BL 0; then a 0x1017 item with one object, BL 0, B, no
        // choices, BL index 3.
        let s = format!(
            "{object} {object} {zeros} {bl80} {bl80} {rest} {object} {zeros} 01 00000001
             {object} {object} {zeros} 10
             00 00010111 00010000 00000000 00000000 10 01 00000001 {object} 10 0 0 01 00000011"
        );
        let b = bits_of(&s);
        let len = s.bytes().filter(|b| *b == b'0' || *b == b'1').count();
        let mut bits = Bits::new(&b, 0, len);
        assert_eq!(assembly(&mut bits), Some(()));
        assert_eq!(target(&mut bits), Some(()));
        assert_eq!(bits.pos, len);
    }
}
