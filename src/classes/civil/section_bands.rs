use super::alignment::{class_header, read_key};
use super::bits::Bits;

/// One label of a section data band group: its keys with their values,
/// in stored order, and the key ids it lists.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SectionDataBandLabel {
    pub keys: Vec<(u32, BandKeyValue)>,
    /// The label's run word, the BL between its property-set bytes and
    /// its key list. The corpus's edge and offset labels carry 0xA001;
    /// the offset-0 label between them (one key, the band number alone)
    /// carries 0xA003.
    pub run: u32,
    /// The key ids the label lists, in stored order (the corpus's edge
    /// and offset labels list the group's four; the offset-0 label lists
    /// its one).
    pub ids: Vec<u32>,
}

/// The value of a section data band label's key. Keys taking references
/// (`0x0F..`) or other list, text and color values are not read: the
/// corpus's records carry none, and the writer cannot emit them.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BandKeyValue {
    /// `0x01..` keys: the label's band number (`0x01015013`).
    Long(i32),
    /// `0x02..` keys: the labeled offset (`0x0202C012`).
    Double(f64),
    /// `0x03..` keys: the band edge marks (`0x0302C108`, `0x0302C109`).
    Byte(u8),
}

/// A section view data band's label group, by its fields.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SectionDataBandGroup {
    /// The labels, in stored order (the band's start edge, the labeled
    /// offsets, the end edge).
    pub labels: Vec<SectionDataBandLabel>,
    /// The settings' two words: 69 and the default dimension anchor
    /// option (`69, 1` in the corpus's records — see
    /// [`super::labels::LabelDimensionAnchor`]).
    pub words: (u32, u32),
    /// The settings' four doubles: the anchor value, the weeding and the
    /// two stagger line heights.
    pub doubles: [f64; 4],
    /// The label stagger (0 none, 1 both sides, 2 right, 3 left).
    pub stagger: u32,
    /// The band range: flag bits, start offset, end offset (`7, 0, 0` —
    /// an automatic range — in the corpus's records).
    pub range: (u32, f64, f64),
    /// The group's number in its view's bands (1: one band per view).
    pub band: i32,
    /// The 16 bytes the record carries after its class tag. The
    /// application does not keep them: across the corpus they are zeros
    /// and heap addresses — residual bytes the reader skips but carries,
    /// so a group rebuilt from a recorded drawing writes them back
    /// verbatim.
    #[cfg_attr(feature = "serde", serde(default))]
    pub guid: [u8; 16],
}

impl SectionDataBandGroup {
    /// The offsets the group's labels name (key `0x0202C012`), in record
    /// order.
    pub fn offsets(&self) -> Vec<f64> {
        self.labels
            .iter()
            .filter_map(|l| l.keys.iter().find_map(|(k, v)| match (k, v) {
                (0x0202_C012, BandKeyValue::Double(o)) => Some(*o),
                _ => None,
            }))
            .collect()
    }

    /// The band numbers the labels name (key `0x01015013`), in record
    /// order.
    pub fn band_numbers(&self) -> Vec<i32> {
        self.labels
            .iter()
            .filter_map(|l| l.keys.iter().find_map(|(k, v)| match (k, v) {
                (0x0101_5013, BandKeyValue::Long(b)) => Some(*b),
                _ => None,
            }))
            .collect()
    }
}

/// The record's header (see the module docs). The 16 residual bytes are
/// kept in `guid`.
fn header(bits: &mut Bits, guid: &mut [u8; 16]) -> Option<()> {
    match class_header(bits)? {
        0b01_0010 => {}
        _ => return None,
    }
    for byte in guid.iter_mut() {
        *byte = bits.bits_value(8)?;
    }
    for want in [80, 80, 0x04BE_0000] {
        (bits.bit_long()? == want).then_some(())?;
    }
    (bits.bit_long()? == 0x00FF_FFFF).then_some(())?;
    (bits.bits_value(3)? == 0b010).then_some(())
}

/// One label (see the module docs); nested labels and overrides are not
/// read — the corpus's records carry none.
fn label(bits: &mut Bits) -> Option<SectionDataBandLabel> {
    let mut keys = Vec::new();
    for _ in 0..bits.bit_long()? {
        let (key, value) = read_key(bits)?;
        let value = match value {
            super::alignment::KeyValue::Long(v) => BandKeyValue::Long(v),
            super::alignment::KeyValue::Double(v) => BandKeyValue::Double(v),
            super::alignment::KeyValue::Byte(v) => BandKeyValue::Byte(v),
            _ => return None,
        };
        keys.push((key, value));
    }
    (bits.bits_value(2)? == 0).then_some(())?;
    (bits.bits_value(8)? == 1).then_some(())?;
    let run = bits.bit_long()? as u32;
    let ids = (0..bits.bit_long()?).map(|_| bits.bit_long().map(|v| v as u32)).collect::<Option<Vec<_>>>()?;
    (bits.bit_long()? == 0).then_some(())?;
    (bits.bit_long()? == 0).then_some(())?;
    Some(SectionDataBandLabel { keys, run, ids })
}

/// The settings after the labels (the standard label settings with no
/// keys of its own), the range and the band number.
fn settings(bits: &mut Bits) -> Option<((u32, u32), [f64; 4], u32, (u32, f64, f64), i32)> {
    (bits.bit_long()? == 0).then_some(())?;
    let words = (bits.bit_long()? as u32, bits.bit_long()? as u32);
    // The standard label settings' doubles: the anchor value and the
    // weeding before the stagger, the two stagger line heights after it.
    let mut doubles = [0.0; 4];
    doubles[0] = bits.bit_double()?;
    doubles[1] = bits.bit_double()?;
    let stagger = bits.bit_long()? as u32;
    doubles[2] = bits.bit_double()?;
    doubles[3] = bits.bit_double()?;
    let range = (bits.bits_value(3)? as u32, bits.bit_double()?, bits.bit_double()?);
    let band = bits.bit_long()?;
    Some((words, doubles, stagger, range, band))
}

/// The group's record, by its fields.
pub(super) fn group(data: &[u8], start: usize, end: usize) -> Option<SectionDataBandGroup> {
    let mut bits = Bits::new(data, start, end);
    let mut guid = [0; 16];
    header(&mut bits, &mut guid)?;
    let labels = (0..bits.bit_long()?).map(|_| label(&mut bits)).collect::<Option<Vec<_>>>()?;
    let (words, doubles, stagger, range, band) = settings(&mut bits)?;
    (bits.bit()? == false).then_some(())?;
    (bits.bit_short()? == 0x04BE && bits.remaining() == 0).then_some(())?;
    Some(SectionDataBandGroup { labels, words, doubles, stagger, range, band, guid })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bits as written, padded to whole bytes.
    fn bytes(bits: &str) -> Vec<u8> {
        let mut bits = bits.to_string();
        while bits.len() % 8 != 0 {
            bits.push('0');
        }
        bits.as_bytes()
            .chunks(8)
            .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 2).unwrap())
            .collect()
    }

    fn raw(le: &[u8]) -> String {
        le.iter().map(|b| format!("{b:08b}")).collect()
    }

    fn bl(v: u32) -> String {
        match v {
            0 => "10".into(),
            1..=255 => format!("01{}", raw(&[v as u8])),
            _ => format!("00{}", raw(&v.to_le_bytes())),
        }
    }

    fn bd(v: f64) -> String {
        if v == 0.0 {
            return "10".into();
        }
        format!("00{}", raw(&v.to_le_bytes()))
    }

    /// A key of each value kind: BL, BD and byte keys.
    fn band_number() -> String {
        bl(0x0101_5013) + "01" + &raw(&[1])
    }

    fn offset(o: f64) -> String {
        bl(0x0202_C012) + &bd(o)
    }

    fn edge(mark: u32) -> String {
        bl(mark) + &raw(&[1])
    }

    /// An edge or offset label: keys, the 44-bit run 0xA001, the group's
    /// four key ids, no nested labels or overrides.
    fn label(keys: &[String]) -> String {
        label_of(keys, 0xA001, &[0x0101_5013, 0x0202_C012, 0x0302_C108, 0x0302_C109])
    }

    /// A label with its own run word and key ids.
    fn label_of(keys: &[String], run: u32, ids: &[u32]) -> String {
        let mut s = bl(keys.len() as u32) + &keys.concat();
        s += &("0000000001".to_string() + &bl(run));
        s += &bl(ids.len() as u32);
        for id in ids {
            s += &bl(*id);
        }
        s + &bl(0) + &bl(0)
    }

    fn header(guid: &[u8]) -> String {
        let mut s: String = [80u32, 0, 80, 80, 0, 0x1002_5F04].into_iter().map(bl).collect();
        s += "010010";
        s += &guid.iter().map(|b| format!("{b:08b}")).collect::<String>();
        s + &bl(80) + &bl(80) + &bl(0x04BE_0000) + &bl(0xFFFFFF) + "010"
    }

    fn range() -> String {
        "111".to_string() + &bd(0.0) + &bd(0.0)
    }

    /// The corpus's form: the start edge, the offsets, the end edge, then
    /// the settings (with `setting_keys` keys), the automatic range, the
    /// band number and the close.
    fn record(guid: &[u8], offsets: &[f64], band: i32, setting_keys: &[u32]) -> (Vec<u8>, usize) {
        let mut s = header(guid) + &bl(3 + offsets.len() as u32);
        s += &label(&[edge(0x0302_C108), band_number()]);
        s += &label_of(&[band_number()], 0xA003, &[0x0101_5013]);
        for o in offsets {
            s += &label(&[offset(*o), band_number()]);
        }
        s += &label(&[edge(0x0302_C109), band_number()]);
        s += &bl(setting_keys.len() as u32);
        for k in setting_keys {
            s += &bl(*k);
        }
        s += &(bl(69) + &bl(1) + &bd(0.0) + &bd(0.0) + &bl(0) + &bd(0.0) + &bd(1.0 / 6.0));
        s += &(range() + &bl(band as u32) + "0" + "00" + &raw(&0x04BEu16.to_le_bytes()));
        (bytes(&s), s.len())
    }

    #[test]
    fn the_corpus_form_reads_by_its_fields() {
        let guid = [0xAB; 16];
        let (data, end) = record(&guid, &[-290.0, -10.0, 10.0, 290.0], 1, &[]);
        let g = group(&data, 0, end).unwrap();
        assert_eq!(g.labels.len(), 7);
        assert_eq!(g.words, (69, 1));
        assert_eq!(g.doubles, [0.0, 0.0, 0.0, 1.0 / 6.0]);
        assert_eq!(g.stagger, 0);
        assert_eq!(g.range, (7, 0.0, 0.0));
        assert_eq!(g.band, 1);
        assert_eq!(g.guid, guid);
        // The edges carry their marks, the middle labels their offsets,
        // and every label its band number and the four key ids.
        assert_eq!(g.labels[0].keys[0], (0x0302_C108, BandKeyValue::Byte(1)));
        assert_eq!(g.labels[0].keys[1], (0x0101_5013, BandKeyValue::Long(1)));
        assert_eq!(g.offsets(), vec![-290.0, -10.0, 10.0, 290.0]);
        assert_eq!(g.band_numbers(), vec![1; 7]);
        assert_eq!(g.labels[6].keys[0], (0x0302_C109, BandKeyValue::Byte(1)));
        // The offset-0 label between the offsets: the band number alone,
        // run 0xA003, its one key id.
        assert_eq!(g.labels[1].keys, vec![(0x0101_5013, BandKeyValue::Long(1))]);
        assert_eq!(g.labels[1].run, 0xA003);
        assert_eq!(g.labels[1].ids, vec![0x0101_5013]);
        assert!(g.labels.iter().enumerate().all(|(i, l)| i == 1 || (l.ids == [0x0101_5013, 0x0202_C012, 0x0302_C108, 0x0302_C109] && l.run == 0xA001)));
    }

    #[test]
    fn unmapped_forms_are_not_read() {
        // A group whose settings open with keys (a label set in the
        // record) is not read.
        let (data, end) = record(&[0; 16], &[10.0], 1, &[0x0F00_B401]);
        assert_eq!(group(&data, 0, end), None);
        // A run word is carried as recorded, not pinned to a constant.
        let (data, end) = record(&[0; 16], &[], 1, &[]);
        let g = group(&data, 0, end).unwrap();
        assert_eq!(g.labels[1].run, 0xA003);
        // A label with a nested label is not read.
        let mut s = header(&[0; 16]) + &bl(1);
        s += &(bl(1) + &band_number() + "00" + "00000001" + &bl(0xA001) + &bl(1) + &bl(0x0101_5013) + &bl(1) + &band_number() + &bl(0) + &bl(0));
        s += &(bl(0) + &bl(69) + &bl(1) + &bd(0.0) + &bd(0.0) + &bl(0) + &bd(0.0) + &bd(1.0 / 6.0));
        s += &(range() + &bl(1) + "0" + "00" + &raw(&0x04BEu16.to_le_bytes()));
        assert!(group(&bytes(&s), 0, s.len()).is_none());
        // A record that does not close where its fields end is not read.
        let (data, end) = record(&[0; 16], &[10.0], 1, &[]);
        assert_eq!(group(&data, 0, end - 1), None);
    }

    #[test]
    fn residual_bytes_are_carried_verbatim() {
        // The corpus's records: one with zeros past its class tag, one
        // with an address-like run. Both read, both carry their bytes.
        let zeros = [0; 16];
        let (data, end) = record(&zeros, &[10.0], 1, &[]);
        assert_eq!(group(&data, 0, end).unwrap().guid, zeros);
        let run: [u8; 16] = [0x9C, 0x80, 0x17, 0x00, 0x11, 0x00, 0x19, 0x40, 0, 0, 0, 0, 0, 0, 0, 0];
        let (data, end) = record(&run, &[10.0], 1, &[]);
        assert_eq!(group(&data, 0, end).unwrap().guid, run);
    }
}

