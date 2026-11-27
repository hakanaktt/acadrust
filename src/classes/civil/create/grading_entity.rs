//! The minimal grading-entity writer: a record whose unmapped parts are
//! taken verbatim from a record the application wrote (the adopt-test
//! design, artifact 2026-11-26T06-dawn/gradtail/).
//!
//! ```text
//! the constant 132-bit schema head (the six words below, the lot lines'
//!   schema word 0x10025F04, three more words — the reader's class body
//!   of an AEC entity opens with it, no version word),
//! 137 verbatim bits of a source record (its head word and header body),
//! BS 0, BS 0, BD x, y, z (the anchor), BS 16, BS k, BD x, y, z (the
//!   record's bit-verbatim copy of the anchor),
//! k × (BS 1, BS 1, BS v_i; between the sections of a k = 2 record: the
//!   second point), BS 0,
//! the run (BS 0, BS 0 for the 744-bit form; BS flag, BS a, BS b with
//!   flag 2 ⇒ a == b otherwise),
//! '00' + 607 (0x25F), one 0 pad bit
//! handles: soft pointer 0, hard pointer parcel, hard ownership 0, hard
//!   pointer 0, hard pointer label style, soft pointer 0
//! ```
//!
//! The 137 opening bits, the section counters and the run have no
//! derivation from typed fields (the header body is not BS/BD-parseable;
//! the counters behave as creation/save-order counters) — so they go in
//! verbatim from [`GradingSkeleton`], the way the application's own clone
//! path keeps them, and the app is expected to recompute them on save
//! (it rewrote exactly these regions and kept the skeleton in the
//! recorded #602 clone experiment). The anchor and its copy are typed.

use super::super::parcel_segments::SCHEMA;
use super::support::*;
use crate::error::DxfError;
use crate::types::Handle;
use crate::CadDocument;

/// The verbatim parts of a source grading record: everything the writer
/// cannot derive. Extracted from a record the application wrote (the
/// class stream after the constant 132-bit schema head, bits [0, 137),
/// then the counters and the run).
#[derive(Debug, Clone, PartialEq)]
pub struct GradingSkeleton {
    /// The source record's 137 unmapped opening bits: its per-record head
    /// word ('00' + a 16-bit value) and its header body — the region the
    /// application rewrites on save. `'0'`/`'1'` characters, MSB first.
    pub head_bits: String,
    /// The record's section count (1, or 2 in the four 998-bit records).
    pub k: i16,
    /// One counter per section, in order.
    pub section_counters: Vec<i16>,
    /// The second point of a k = 2 record (differs from the anchor there).
    pub second_point: Option<[f64; 3]>,
    /// The run: `[0, 0, 0]` writes the 744-bit form (two 0 shorts after
    /// the terminator); otherwise flag, a, b with flag 1 or 2 and a == b
    /// when flag is 2, as every observed record stores.
    pub run: [i16; 3],
    /// The record's text stream, verbatim from the source record (two
    /// empty strings and one label-template string in every observed
    /// record; the application reads it after the fields).
    pub texts: Vec<String>,
}

impl GradingSkeleton {
    fn validate(&self) -> Result<(), DxfError> {
        if self.head_bits.len() != 137 || !self.head_bits.bytes().all(|b| b == b'0' || b == b'1') {
            return Err(invalid("the skeleton's head is not 137 bits"));
        }
        if !matches!(self.k, 1 | 2) || self.section_counters.len() != self.k as usize {
            return Err(invalid("a grading skeleton needs one counter per section (k = 1 or 2)"));
        }
        if (self.k == 2) != self.second_point.is_some() {
            return Err(invalid("a two-section grading skeleton needs its second point"));
        }
        let [flag, a, b] = self.run;
        if (flag, a, b) != (0, 0, 0) {
            if !matches!(flag, 1 | 2) || (flag == 2 && a != b) {
                return Err(invalid("the run is neither the 744-bit form nor flag a b with flag 1 or 2 (a == b when flag is 2)"));
            }
        }
        Ok(())
    }
}

/// Adds a minimal grading entity: `skeleton` (verbatim from a source
/// record the application wrote) around the typed `anchor`. This is the
/// adopt-test writer, verified against the reference application end to
/// end (docs/civil-write.md §3p): opened read-only the record resolves as
/// a first-class `AeccDbGrading`; one application save rewrites exactly
/// the unnamed regions (the `BS 16` word 16→17, the second section
/// header 1→0, the counter, the run — 770 → 744 bits here) and keeps the
/// mapped skeleton bit for bit, re-pointing the record's null references
/// into its own tree.
///
/// Both references are therefore written null — `parcel` and
/// `label_style` must be [`Handle::NULL`]. Hard-pointing a fresh
/// drawing's objects has not been verified and wedged the reference
/// application once (2026-11-26: an open that never completed, the
/// session lost to it); the verified emission is the null-reference one.
///
/// The record carries no geometry (the solution lives in the site's
/// topology) and no graphic image; the application adds one on save.
/// `layer` defaults to `"0"`.
pub fn add_grading_minimal(doc: &mut CadDocument, parcel: Handle, label_style: Handle, skeleton: &GradingSkeleton, anchor: [f64; 3], layer: Option<&str>) -> Result<Handle, DxfError> {
    skeleton.validate()?;
    if parcel != Handle::NULL || label_style != Handle::NULL {
        return Err(invalid("the adopt-test writer writes null references (the application re-points them on save); non-null references are not verified"));
    }
    if !anchor.iter().all(|v| v.is_finite()) {
        return Err(invalid("the grading anchor needs finite coordinates"));
    }
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    let common = crate::entities::EntityCommon {
        handle: doc.allocate_handle(),
        owner_handle: doc.header.model_space_block_handle,
        layer: layer.into(),
        layer_handle: Some(layer_handle),
        ..crate::entities::EntityCommon::default()
    };
    let head_bits = skeleton.head_bits.clone();
    let k = skeleton.k;
    let counters = skeleton.section_counters.clone();
    let second = skeleton.second_point;
    let run = skeleton.run;
    let texts = skeleton.texts.clone();
    put_entity(doc, "AECC_GRADING", common, |w| {
        for word in [80i16, 256, 0xC76Au16 as i16, 80, 80, 0] {
            w.write_bit_short(word);
        }
        w.write_bit_long(SCHEMA as i32);
        for word in [80i16, 0x1540, 0xC1C0u16 as i16] {
            w.write_bit_short(word);
        }
        bits(w, &head_bits);
        w.write_bit_short(0);
        w.write_bit_short(0);
        for v in anchor {
            w.write_bit_double(v);
        }
        w.write_bit_short(16);
        w.write_bit_short(k);
        for v in anchor {
            w.write_bit_double(v);
        }
        for i in 0..k {
            w.write_bit_short(1);
            w.write_bit_short(1);
            w.write_bit_short(counters[i as usize]);
            if i + 1 < k {
                for v in second.unwrap_or(anchor) {
                    w.write_bit_double(v);
                }
            }
        }
        w.write_bit_short(0);
        // The run: the 744-bit form keeps two 0 shorts; otherwise flag, a, b.
        if run == [0, 0, 0] {
            w.write_bit_short(run[0]);
            w.write_bit_short(run[1]);
        } else {
            for v in run {
                w.write_bit_short(v);
            }
        }
        // The terminator: '00' + 607 (0x25F) little-endian, one 0 pad bit.
        w.write_2bits(0);
        w.write_byte(0x5F);
        w.write_byte(0x02);
        w.write_bit(false);
    }, Body {
        refs: vec![
            soft_pointer(0),
            hard_pointer(parcel.value()),
            Ref(crate::io::dwg::dwg_reference_type::DwgReferenceType::HardOwnership, 0),
            hard_pointer(0),
            hard_pointer(label_style.value()),
            soft_pointer(0),
        ],
        strings: texts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skeleton(head_bits: &str, k: i16, counters: &[i16], second: Option<[f64; 3]>, run: [i16; 3]) -> GradingSkeleton {
        GradingSkeleton {
            head_bits: head_bits.into(),
            k,
            section_counters: counters.to_vec(),
            second_point: second,
            run,
            texts: vec![String::new(), String::new(), "<[67326721(2449)]> : <[67112990(2449)]>".into()],
        }
    }

    #[test]
    fn rejects_a_bad_skeleton() {
        let bits = "0".repeat(137);
        assert!(skeleton("01", 1, &[139], None, [1, 1, 12]).validate().is_err());
        assert!(skeleton(&bits, 3, &[1, 2, 3], None, [1, 1, 12]).validate().is_err());
        assert!(skeleton(&bits, 1, &[], None, [1, 1, 12]).validate().is_err());
        assert!(skeleton(&bits, 2, &[1, 2], None, [1, 1, 12]).validate().is_err());
        assert!(skeleton(&bits, 1, &[1], None, [2, 1, 1]).validate().is_ok());
        assert!(skeleton(&bits, 1, &[1], None, [2, 1, 2]).validate().is_err());
        assert!(skeleton(&bits, 1, &[1], None, [2, 1, 3]).validate().is_err());
        assert!(skeleton(&bits, 1, &[1], None, [3, 1, 1]).validate().is_err());
    }

    #[test]
    fn refuses_non_null_references() {
        let mut doc = CadDocument::new();
        doc.version = crate::types::DxfVersion::AC1032;
        let skel = skeleton(
            "00010000000000000000000000000000000000000000000000000000000000000000010001100000000000000000000000000001001001000110100111100011110001010",
            1, &[139], None, [1, 1, 12],
        );
        let made = add_grading_minimal(&mut doc, crate::types::Handle::new(0x78), crate::types::Handle::NULL, &skel, [0.0; 3], None);
        assert!(made.is_err());
    }

    /// A written grading round-trips, decodes, and its class stream is
    /// the source record's bit for bit (the 13da record of Grading-3A.dwg:
    /// verbatim opening bits and counters, the record's own anchor).
    #[test]
    fn written_grading_round_trips() {
        let mut doc = CadDocument::new();
        doc.version = crate::types::DxfVersion::AC1032;
        super::super::support::ensure_civil(&mut doc).unwrap();
        let head_bits = "00010000000000000000000000000000000000000000000000000000000000000000010001100000000000000000000000000001001001000110100111100011110001010";
        let skel = skeleton(head_bits, 1, &[139], None, [1, 1, 12]);
        let anchor = [4233.012378452553, 3646.866181775694, 650.6339697023699];
        let made = add_grading_minimal(&mut doc, crate::types::Handle::NULL, crate::types::Handle::NULL, &skel, anchor, None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-grading-min-{}.dwg", std::process::id()));
        crate::io::dwg::DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = crate::io::dwg::DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let mut found = 0;
        for e in back.entities() {
            if let crate::entities::EntityType::Extended(x) = e {
                if let crate::entities::ExtendedEntityData::Arch(a) = &x.data {
                    if a.dxf_name == "AECC_GRADING" {
                        found += 1;
                        assert_eq!(x.common.handle, made);
                        // The references are null; the reader's grading
                        // decode needs non-null ones, so it is None.
                        assert!(a.grading().is_none());
                        // The class stream is the source record's bit for bit.
                        let bits = main_bits(a);
                        assert_eq!(bits.len(), 770);
                        assert_eq!(&bits[..269], &expected_stream(head_bits, anchor)[..269]);
                        assert_eq!(bits, expected_stream(head_bits, anchor));
                    }
                }
            }
        }
        assert_eq!(found, 1);
    }

    /// The class stream of an entity record as bits (the constant head
    /// search the round's probe uses).
    fn main_bits(data: &crate::classes::arch::ArchEntityData) -> String {
        let raw = data.raw_dwg_data.as_deref().unwrap();
        let blob: String = raw.iter().map(|b| format!("{b:08b}")).collect();
        let head = {
            let mut b = String::new();
            let bs = |v: i16, b: &mut String| match v {
                0 => b.push_str("10"),
                256 => b.push_str("11"),
                v if v > 0 && v < 256 => b.push_str(&format!("01{v:08b}")),
                v => {
                    b.push_str("00");
                    for x in v.to_le_bytes() {
                        b.push_str(&format!("{x:08b}"));
                    }
                }
            };
            for v in [80i16, 256, 0xC76Au16 as i16, 80, 80, 0] {
                bs(v, &mut b);
            }
            b.push_str("00");
            for x in crate::classes::civil::parcel_segments::SCHEMA.to_le_bytes() {
                b.push_str(&format!("{x:08b}"));
            }
            for v in [80i16, 0x1540, 0xC1C0u16 as i16] {
                bs(v, &mut b);
            }
            b
        };
        let at = blob.find(&head).unwrap();
        blob[at..at + 770].to_string()
    }

    /// The expected 770-bit class stream: the constant head, the 137
    /// verbatim bits, the grammar around the anchor.
    fn expected_stream(head_bits: &str, anchor: [f64; 3]) -> String {
        let mut b = String::new();
        let bs = |v: i16, b: &mut String| match v {
            0 => b.push_str("10"),
            256 => b.push_str("11"),
            v if v > 0 && v < 256 => b.push_str(&format!("01{v:08b}")),
            v => {
                b.push_str("00");
                for x in v.to_le_bytes() {
                    b.push_str(&format!("{x:08b}"));
                }
            }
        };
        let bd = |v: f64, b: &mut String| {
            b.push_str("00");
            for x in v.to_le_bytes() {
                b.push_str(&format!("{x:08b}"));
            }
        };
        for v in [80i16, 256, 0xC76Au16 as i16, 80, 80, 0] {
            bs(v, &mut b);
        }
        b.push_str("00");
        for x in crate::classes::civil::parcel_segments::SCHEMA.to_le_bytes() {
            b.push_str(&format!("{x:08b}"));
        }
        for v in [80i16, 0x1540, 0xC1C0u16 as i16] {
            bs(v, &mut b);
        }
        b.push_str(head_bits);
        bs(0, &mut b);
        bs(0, &mut b);
        for v in anchor {
            bd(v, &mut b);
        }
        bs(16, &mut b);
        bs(1, &mut b);
        for v in anchor {
            bd(v, &mut b);
        }
        bs(1, &mut b);
        bs(1, &mut b);
        bs(139, &mut b);
        bs(0, &mut b);
        bs(1, &mut b);
        bs(1, &mut b);
        bs(12, &mut b);
        // The terminator: '00' + 607 (0x25F) little-endian, one 0 pad bit.
        b.push_str("0001011111000000100");
        b
    }
}
