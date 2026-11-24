//! Stormwater control entities: pond (`AECC_POND`), underground storage
//! (`AECC_UNDERGROUNDSTORAGE`) and channel (`AECC_CHANNEL`) of the
//! application's stormwater planning tools.
//!
//! All three share the parcel-segment record head (upstream PR #591) and
//! schema word, and carry their shape in the entity's graphic-image section
//! (a "stormwater control" variant of the AEC graphic image, not a proxy
//! metafile):
//!
//! ```text
//! class body: BL 80, 0, 80, 80, 0, BL 0x1002_5F04, ... (unmapped save-state)
//! graphic:    [u32 self size][u32 kind][u32 sections - 8][u32 36]
//!             RD origin x y z, 3 x 3 RD placement axes,
//!             UTF-16 NUL-terminated schema word, pad to sections - 8,
//!             sections { u32 size, u32 6, u32 count, count x 3RD points } ...,
//!             u32 8, u32 21
//! ```
//!
//! The `kind` word is 3 for a pond or underground storage and 8 for a
//! channel; the schema word names the class (`AeccDbPond
//! (AeccStormWaterControl130)` and kin). The section points are the drawn
//! outline in world coordinates (a pond's contour rectangle at its top
//! elevation, an underground storage's bounding rectangle, a channel's
//! rectangular sheet profiles); the first point repeats at the end of a
//! closed outline. The placement axes are stored skewed for a rotated
//! storage; the bounding rectangle is what the record keeps (the application
//! derives the rotated boundary from it).
//!
//! Nothing else about the three is persisted in the records. The hydraulic
//! parameters the API exposes (a pond's depth, freeboard, porosity and
//! infiltration, a channel's Manning coefficient and trapezoidal
//! cross-section) appear nowhere in the entity record, its streams or any
//! other record of such a drawing — exhaustive bit scans; the API values of
//! the one drawing these families are known from are application defaults
//! (the channel's start/end elevations and slope are those of its baseline
//! feature line, to the last digit). The objects' names live in sibling
//! `AECC_COMPONENT` records' text streams, not in these records. There are
//! no writers upstream for the families; writing them is a follow-up.
//!
//! Provenance (D013): the only corpus drawing with these families is the
//! application's regional sample `DE_Regenwassersteuerung_2027.dwg`.

use super::bits::Bits;
use crate::types::Vector3;

/// Schema word the three records share with the parcel segments and the
/// sample line groups (upstream PR #591).
pub(super) const SCHEMA: u32 = 0x1002_5F04;

/// Which stormwater control entity a record holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum StormWaterKind {
    Pond,
    UndergroundStorage,
    Channel,
}

impl StormWaterKind {
    /// The graphic image's schema word prefix for the family.
    fn schema_word(self) -> &'static str {
        match self {
            StormWaterKind::Pond => "AeccDbPond (",
            StormWaterKind::UndergroundStorage => "AeccDbUndergroundStorage (",
            StormWaterKind::Channel => "AeccDbChannel (",
        }
    }

    /// The graphic image's kind word: 3 for an area-based control, 8 for a
    /// channel.
    fn graphic_kind(self) -> u32 {
        match self {
            StormWaterKind::Pond | StormWaterKind::UndergroundStorage => 3,
            StormWaterKind::Channel => 8,
        }
    }
}

/// What a stormwater control entity's record holds: its schema word, its
/// placement origin, the outline sections of its graphic image and the
/// style it hard-points at.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StormWater {
    pub kind: StormWaterKind,
    /// The graphic image's schema word, e.g. `AeccDbPond
    /// (AeccStormWaterControl130)`.
    pub description: String,
    /// Placement origin of the graphic image (a pond's is at its top
    /// elevation, a channel's at its downstream elevation).
    pub origin: Vector3,
    /// Outline points of each drawn section, world coordinates; a closed
    /// outline repeats its first point at the end.
    pub outlines: Vec<Vec<Vector3>>,
    /// Handle of the family style the record hard-points at
    /// (`AECC_POND_STYLE`, `AECC_UNDERGROUNDSTORAGE_STYLE` or
    /// `AECC_CHANNEL_STYLE`); 0 when the record names none.
    pub style: u64,
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    data.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn double_at(data: &[u8], at: usize) -> Option<f64> {
    let b = data.get(at..at + 8)?;
    if b.len() < 8 {
        return None;
    }
    let value = f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
    value.is_finite().then_some(value)
}

/// Reads the class body's gate: the shared head words and schema word.
pub(super) fn body_gate(bits: &mut Bits) -> Option<()> {
    for want in [80i32, 0, 80, 80, 0] {
        (bits.bit_long()? == want).then_some(())?;
    }
    (bits.bit_long()? as u32 == SCHEMA).then_some(())
}

/// Reads the graphic image: schema word, origin and outline sections.
///
/// `kind` gates the image's own kind word and schema word so a foreign
/// graphic cannot pass.
pub(super) fn graphic(
    kind: StormWaterKind,
    blob: &[u8],
) -> Option<(String, Vector3, Vec<Vec<Vector3>>)> {
    let size = u32_at(blob, 0)? as usize;
    (u32_at(blob, 4)? == kind.graphic_kind()).then_some(())?;
    if size != blob.len() {
        return None;
    }
    // The placement is a fixed 112-byte run: the origin doubles, then three
    // placement axes (which the application stores skewed for a rotated
    // storage — not an orthonormal frame, so no rigidity is required).
    let origin = Vector3::new(
        double_at(blob, 16)?,
        double_at(blob, 24)?,
        double_at(blob, 32)?,
    );
    // The schema word follows the placement, NUL-terminated UTF-16.
    let text_at = 16 + 24 + 3 * 24;
    let mut end = text_at;
    while end + 2 <= size {
        if blob[end] == 0 && blob[end + 1] == 0 {
            break;
        }
        end += 2;
    }
    let units: Vec<u16> = (text_at..end)
        .step_by(2)
        .filter_map(|at| Some(u16::from_le_bytes([*blob.get(at)?, *blob.get(at + 1)?])))
        .collect();
    let description = String::from_utf16(&units).ok()?;
    (description.starts_with(kind.schema_word())).then_some(())?;
    // The outline sections start 8 past the header word.
    let mut at = u32_at(blob, 8)? as usize + 8;
    let mut outlines = Vec::new();
    while at + 12 <= size {
        let count = u32_at(blob, at + 8)?;
        (u32_at(blob, at + 4)? == 6 && count > 0).then_some(())?;
        let count = count as usize;
        let bytes = 12 + count * 24;
        (u32_at(blob, at)? == bytes as u32).then_some(())?;
        if at + bytes > size {
            break;
        }
        let mut points = Vec::with_capacity(count);
        for index in 0..count {
            let base = at + 12 + index * 24;
            points.push(Vector3::new(
                double_at(blob, base)?,
                double_at(blob, base + 8)?,
                double_at(blob, base + 16)?,
            ));
        }
        outlines.push(points);
        at += bytes;
    }
    (!outlines.is_empty()).then_some(())?;
    // The section list closes with the words 8 and 21.
    (at + 8 <= size && u32_at(blob, at)? == 8 && u32_at(blob, at + 4)? == 21).then_some(())?;
    Some((description, origin, outlines))
}

/// Reads a stormwater control entity: `data`/`start`/`end` is its class
/// stream, `blob` its graphic image, `handles` its class-body references in
/// stream order (the first is the style hard pointer).
pub(super) fn entity(
    kind: StormWaterKind,
    data: &[u8],
    start: usize,
    end: usize,
    blob: &[u8],
    handles: &[u64],
) -> Option<StormWater> {
    let mut bits = Bits::new(data, start, end);
    body_gate(&mut bits)?;
    let (description, origin, outlines) = graphic(kind, blob)?;
    let style = handles.first().copied().unwrap_or(0);
    Some(StormWater {
        kind,
        description,
        origin,
        outlines,
        style,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Appends a NUL-terminated UTF-16 string.
    fn push_text(blob: &mut Vec<u8>, text: &str) {
        for unit in text.encode_utf16() {
            blob.extend_from_slice(&unit.to_le_bytes());
        }
        blob.extend_from_slice(&[0, 0]);
    }

    fn push_f64(blob: &mut Vec<u8>, value: f64) {
        blob.extend_from_slice(&value.to_le_bytes());
    }

    fn push_u32(blob: &mut Vec<u8>, value: u32) {
        blob.extend_from_slice(&value.to_le_bytes());
    }

    fn point_bytes(points: &[(f64, f64, f64)]) -> Vec<u8> {
        let mut out = Vec::new();
        for (x, y, z) in points {
            push_f64(&mut out, *x);
            push_f64(&mut out, *y);
            push_f64(&mut out, *z);
        }
        out
    }

    /// A graphic image in the recorded layout, from the recorded field
    /// values of the DE_Regenwassersteuerung_2027.dwg pond (a Sample
    /// drawing — see the module provenance note).
    fn pond_blob() -> Vec<u8> {
        let points = [
            (495600.088_177_815_84, 574_734.225_898_887_6, 476.7),
            (495_639.713_730_186_9, 574_734.225_898_887_6, 476.7),
            (495_639.713_730_186_9, 574_771.722_210_898_8, 476.7),
            (495600.088_177_815_84, 574_771.722_210_898_8, 476.7),
            (495600.088_177_815_84, 574_734.225_898_887_6, 476.7),
        ];
        // header (16) + origin (24) + axes (72) + text + section (12 + 120) + tail (8)
        let text_bytes = "AeccDbPond (AeccStormWaterControl130)"
            .encode_utf16()
            .count()
            * 2
            + 2;
        let section_words = 12 + points.len() * 24;
        let mut blob = Vec::with_capacity(16 + 96 + text_bytes + 4 + section_words + 8);
        push_u32(&mut blob, 0); // self size, patched below
        push_u32(&mut blob, 3);
        // The header word points 8 past the section list's first word
        // (recorded: header 180, sections at 188, right after the text).
        let sections_at = 16 + 96 + text_bytes;
        push_u32(&mut blob, (sections_at - 8) as u32);
        push_u32(&mut blob, 36);
        push_f64(&mut blob, 495600.609_566_662_8);
        push_f64(&mut blob, 574_734.747_287_734_6);
        push_f64(&mut blob, 476.7);
        for axis in [
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            [1.042_777_693_975_949_7, 1.0, 0.0],
        ] {
            push_f64(&mut blob, axis[0]);
            push_f64(&mut blob, axis[1]);
            push_f64(&mut blob, axis[2]);
        }
        push_text(&mut blob, "AeccDbPond (AeccStormWaterControl130)");
        debug_assert_eq!(blob.len(), sections_at);
        push_u32(&mut blob, section_words as u32);
        push_u32(&mut blob, 6);
        push_u32(&mut blob, points.len() as u32);
        blob.extend(point_bytes(&points));
        push_u32(&mut blob, 8);
        push_u32(&mut blob, 21);
        let size = blob.len() as u32;
        blob[0..4].copy_from_slice(&size.to_le_bytes());
        blob
    }

    /// The class body head: the five shared words and the schema word.
    fn body_bytes() -> Vec<u8> {
        let mut bits = String::new();
        let mut word = |value: u32, bits: &mut String| {
            bits.push_str("00");
            for b in value.to_le_bytes() {
                bits.push_str(&format!("{b:08b}"));
            }
        };
        for v in [80u32, 0, 80, 80, 0] {
            if v == 0 {
                bits.push_str("10");
            } else {
                word(v, &mut bits);
            }
        }
        word(SCHEMA, &mut bits);
        bits.as_bytes()
            .iter()
            .filter(|b| **b == b'0' || **b == b'1')
            .enumerate()
            .fold(Vec::new(), |mut out, (index, b)| {
                let bit = (b - b'0') as u8;
                let byte = index / 8;
                if byte >= out.len() {
                    out.push(0);
                }
                out[byte] |= bit << (7 - index % 8);
                out
            })
    }

    #[test]
    fn reads_pond_outline_and_style() {
        let kind = StormWaterKind::Pond;
        let (description, origin, outlines) = graphic(kind, &pond_blob()).unwrap();
        assert_eq!(description, "AeccDbPond (AeccStormWaterControl130)");
        assert_eq!(origin.x, 495600.609_566_662_8);
        assert_eq!(origin.y, 574_734.747_287_734_6);
        assert_eq!(origin.z, 476.7);
        assert_eq!(outlines.len(), 1);
        assert_eq!(outlines[0].len(), 5);
        // The drawn contour rectangle, closed, at the pond's top elevation.
        assert_eq!(outlines[0][0].x, 495600.088_177_815_84);
        assert_eq!(outlines[0][0].y, 574_734.225_898_887_6);
        assert_eq!(outlines[0][0].z, 476.7);
        assert_eq!(outlines[0][4], outlines[0][0]);
        assert_eq!(outlines[0][2].x, 495_639.713_730_186_9);
        assert_eq!(outlines[0][2].y, 574_771.722_210_898_8);

        let body = body_bytes();
        let storm = entity(
            kind,
            &body,
            0,
            body.len() * 8,
            &pond_blob(),
            &[0x202D_6CE, 0, 0],
        )
        .unwrap();
        assert_eq!(storm.kind, StormWaterKind::Pond);
        assert_eq!(storm.style, 0x202D_6CE);
        assert_eq!(storm.description, "AeccDbPond (AeccStormWaterControl130)");
        assert_eq!(storm.outlines.len(), 1);
    }

    #[test]
    fn reads_channel_sections() {
        // The channel's graphic image holds six sections: the channel bed
        // rectangle and its top rectangle at start elevation + depth, then
        // four vertical edges — recorded point values.
        let bed = [
            (
                495547.713_075_912_03,
                574_756.835_598_150_7,
                473.935_558_722_156_77,
            ),
            (
                495601.720_267_238_4,
                574_756.835_598_150_7,
                473.935_558_722_156_77,
            ),
            (
                495601.720_267_238_4,
                574_889.316_167_097,
                473.935_558_722_156_77,
            ),
            (
                495547.713_075_912_03,
                574_889.316_167_097,
                473.935_558_722_156_77,
            ),
            (
                495547.713_075_912_03,
                574_756.835_598_150_7,
                473.935_558_722_156_77,
            ),
        ];
        let top = [
            (
                495601.720_267_238_4,
                574_889.316_167_097,
                477.557_180_586_777_2,
            ),
            (
                495601.720_267_238_4,
                574_756.835_598_150_7,
                477.557_180_586_777_2,
            ),
            (
                495547.713_075_912_03,
                574_756.835_598_150_7,
                477.557_180_586_777_2,
            ),
            (
                495547.713_075_912_03,
                574_889.316_167_097,
                477.557_180_586_777_2,
            ),
            (
                495601.720_267_238_4,
                574_889.316_167_097,
                477.557_180_586_777_2,
            ),
        ];
        let edge = [
            (
                495601.720_267_238_4,
                574_889.316_167_097,
                477.557_180_586_777_2,
            ),
            (
                495601.720_267_238_4,
                574_889.316_167_097,
                473.935_558_722_156_77,
            ),
        ];
        let text_bytes = "AeccDbChannel (AeccDrainageChannel130)"
            .encode_utf16()
            .count()
            * 2
            + 2;
        let mut blob = Vec::new();
        push_u32(&mut blob, 0);
        push_u32(&mut blob, 8);
        // The channel image pads the text with one more NUL unit before the
        // section list (recorded: header 184, sections at 192, text to 190).
        let sections_at = 16 + 96 + text_bytes + 2;
        push_u32(&mut blob, (sections_at - 8) as u32);
        push_u32(&mut blob, 36);
        push_f64(&mut blob, 495548.405_475_800_86);
        push_f64(&mut blob, 574_757.527_998_039_5);
        push_f64(&mut blob, 473.935_558_722_156_77);
        for axis in [
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            [1.384_799_777_598_788_8, 1.0, 0.0],
        ] {
            push_f64(&mut blob, axis[0]);
            push_f64(&mut blob, axis[1]);
            push_f64(&mut blob, axis[2]);
        }
        push_text(&mut blob, "AeccDbChannel (AeccDrainageChannel130)");
        blob.extend_from_slice(&[0, 0]); // the channel image pads the text with one more unit
        debug_assert_eq!(blob.len(), sections_at);
        for (count, points) in [
            (5usize, &bed[..]),
            (5, &top[..]),
            (2, &edge[..]),
            (2, &edge[..]),
            (2, &edge[..]),
            (2, &edge[..]),
        ] {
            push_u32(&mut blob, (12 + count * 24) as u32);
            push_u32(&mut blob, 6);
            push_u32(&mut blob, count as u32);
            blob.extend(point_bytes(points));
        }
        push_u32(&mut blob, 8);
        push_u32(&mut blob, 21);
        let size = blob.len() as u32;
        blob[0..4].copy_from_slice(&size.to_le_bytes());

        let (description, origin, outlines) = graphic(StormWaterKind::Channel, &blob).unwrap();
        assert_eq!(description, "AeccDbChannel (AeccDrainageChannel130)");
        assert_eq!(origin.z, 473.935_558_722_156_77);
        assert_eq!(outlines.len(), 6);
        assert_eq!(outlines[0].len(), 5);
        assert_eq!(outlines[1].len(), 5);
        assert_eq!(outlines[2].len(), 2);
        // The bed rectangle sits at the downstream elevation, the top
        // rectangle at start elevation + depth, to the last digit.
        assert_eq!(outlines[0][0].z, 473.935_558_722_156_77);
        assert_eq!(outlines[1][0].z, 477.557_180_586_777_2);
        assert_eq!(outlines[1][0].z - outlines[0][0].z, 3.621_621_864_620_408_4);

        let body = body_bytes();
        let storm = entity(
            StormWaterKind::Channel,
            &body,
            0,
            body.len() * 8,
            &blob,
            &[0x202A_A2C, 0],
        )
        .unwrap();
        assert_eq!(storm.style, 0x202A_A2C);
    }

    #[test]
    fn refuses_other_families() {
        // A pond image under the channel kind: the kind word and the schema
        // word both disagree.
        let mut blob = pond_blob();
        blob[4..8].copy_from_slice(&8u32.to_le_bytes());
        assert!(graphic(StormWaterKind::Channel, &blob).is_none());
        assert!(graphic(StormWaterKind::UndergroundStorage, &pond_blob()).is_none());

        // A truncated section list is refused, not misread.
        let mut cut = pond_blob();
        cut.truncate(cut.len() - 9);
        assert!(graphic(StormWaterKind::Pond, &cut).is_none());
    }

    #[test]
    fn refuses_foreign_body() {
        let mut body = body_bytes();
        body[1] ^= 0xff;
        assert!(entity(
            StormWaterKind::Pond,
            &body,
            0,
            body.len() * 8,
            &pond_blob(),
            &[0x202D_6CE]
        )
        .is_none());
    }
}
