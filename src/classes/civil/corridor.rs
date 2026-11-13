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
    // for the first group of each baseline: a head whose fields after its
    // BL 114 hold one bit more than the 66 of every other sample, and heads
    // with a field other than 0, 32 or 160 after the GUID.
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

/// The record read field by field:
///
/// ```text
/// head:     BL 80, 0, 80, 80, 0, BL, BL, 4 bits, RC n, n x key,
///           BL 0, 120 bits, BL 82, BL n, n x 4 bits, BL flag, [flag: GUID],
///           BL a, then by a
///             0:   BL m, m x 3BD
///             32:  BL, BL, RL, BL m, m x 3BD
///             160: 76 bits
///           BL 0, BL 32, RC, RC baselines (top 5 bits), 1 bit,
///           6 bits region count, BL record count (both of the first
///           baseline's first region)
/// flag:     set, each region block holds a GUID before its start and its
///           end station
/// baseline: [after the first: BL 0, BL 0, BL region count, BL record
///           count], regions, then B, GUID, BL n, n x feature line group,
///           B, B, GUID
/// region:   records (see [`station`]), then its block (see [`region`]),
///           which gives the next region's record count, 1 after the last;
///           the block's target entries hold as many lists as the region's
///           last applied subassembly has words before its -1, less 8 (10
///           of 18 and 7 of 15 in the samples)
/// ```
// ponytail: the 4, 120 and (a = 160) 76-bit runs, the first RC, the low 3
// bits of the second and the bit after it are skipped by length; they are
// the same in every sample.
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
    (bits.bit_long()? == 0).then_some(())?;
    bits.skip(120)?;
    (bits.bit_long()? == 82).then_some(())?;
    let styles = usize::try_from(bits.bit_long()?).ok()?;
    bits.skip(4 * styles)?;
    let guids = bits.bit_long()? != 0;
    if guids {
        bits.skip(128)?;
    }
    let corners = match bits.bit_long()? {
        0 => true,
        32 => {
            bits.bit_long()?;
            bits.bit_long()?;
            bits.raw_long()?;
            true
        }
        160 => {
            bits.skip(76)?;
            false
        }
        _ => return None,
    };
    if corners {
        let n = usize::try_from(bits.bit_long()?).ok().filter(|n| n * 6 <= bits.remaining())?;
        for _ in 0..3 * n {
            bits.bit_double()?;
        }
    }
    (bits.bit_long()? == 0 && bits.bit_long()? == 32).then_some(())?;
    bits.raw_short_byte()?;
    let baselines = bits.raw_short_byte()? >> 3;
    bits.skip(1)?;
    let mut regions = u32::from(bits.bits_value(6)?);
    let mut records = u32::try_from(bits.bit_long()?).ok()?;
    let mut out = Vec::new();
    for baseline in 0..baselines {
        if baseline > 0 {
            (bits.bit_long()? == 0 && bits.bit_long()? == 0).then_some(())?;
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

/// The assembly's groups of nodes `object, object, BL, BL, BL`; after a node
/// `BL 80` opens the group's next node (the rest of its first object head
/// follows), `BL 0` ends, any other value up to 79 opens the next group (1,
/// 2 and 3 in the samples).
fn assembly(bits: &mut Bits) -> Option<()> {
    loop {
        object(bits)?;
        loop {
            object(bits)?;
            for _ in 0..3 {
                bits.bit_long()?;
            }
            match bits.bit_long()? {
                80 => object_rest(bits)?,
                0 => return Some(()),
                1..=79 => break,
                _ => return None,
            }
        }
    }
}

/// One applied subassembly: `object, BL 8, object`, its shape, its parameter
/// set, then words until `-1`: each a count of target items, or
/// `0x8004xxxx` with no items. Returns the number of words.
///
/// ```text
/// shape:  BL n, n x { BL, BL, BD station, BD offset, BD elevation }
///         BL m, m x { BL code, B, B, BL k, k x RL point }
///         BL a, a x { BL code, B, B, BL k, k x RL link, BL }
/// ```
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

/// A target item: `BL type, BL 0`, then by the type's low byte
///
/// ```text
/// 0x15: object
/// 0x17: BL k, k x object, BL
/// ```
///
/// then `B, B choices, [BL n, n x BL], BL index`. Types carry flags above
/// the low byte (0x1015, 0x1017, 0x1117, 0x20001117 in the samples).
fn target(bits: &mut Bits) -> Option<()> {
    let kind = bits.bit_long()? & 0xFF;
    (bits.bit_long()? == 0).then_some(())?;
    match kind {
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
