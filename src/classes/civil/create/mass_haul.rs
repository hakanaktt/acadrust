//! Mass haul lines (`AECC_MASSHAULLINE`), written in the record form the
//! wizard's create path stores (mh_third.dwg, handle 0x6657; 2026-11-27):
//! the balance curve, its vertex tables and the head's free-haul distance —
//! with the derived display data EMPTY (polyline #2 holds 0 vertices; the
//! application draws the 166-vertex free-haul construction only when it
//! regenerates the display, which is the update path and is not written
//! here).

use super::sample_lines::arch;
use super::support::*;
use crate::entities::EntityCommon;
use crate::error::DxfError;
use crate::io::dwg::dwg_stream_writers::DwgMergedWriter;
use crate::types::Handle;
use crate::CadDocument;

/// The head word after `u1`: the application's create-path value (0x40809313;
/// the original and resaved records read 0x40008337 and 0x40800730 — the
/// word is per-record, unnamed, and not derivable from the curve, the free
/// haul or the display data, which are identical across the three). A
/// written line carries the create-path word; the application rewrites the
/// field on its first save (2026-11-27 adopt test).
const WORD_LINE: i32 = 0x4080_9313;
/// The run that closes the record before its texts (the same in the section
/// and sample line records).
const TAIL: &str = "0001011111000000100";
/// The record's first text (the class's import name).
const IMPS: &str = "AeccImpMassHaulLine";

fn head(w: &mut DwgMergedWriter, u2: i32) {
    for v in [80, 0, 80, 80, 0, 0x1002_5F04, u2] {
        w.write_bit_long(v);
    }
}

/// Writes the curve points and the vertex tables of the two display
/// polylines: polyline #1 is the curve itself (its table maps row `i` to
/// point `i`, the last row to point `n`), polyline #2 is empty (its table's
/// last row points at 0).
fn curve_and_tables(w: &mut DwgMergedWriter, curve: &[(f64, f64)]) {
    let n = curve.len();
    let points = |w: &mut DwgMergedWriter| {
        for (st, v) in curve {
            w.write_bit_double(*st);
            w.write_bit_double(*v);
            w.write_bit_double(0.0);
        }
    };
    points(w);
    // Table of polyline #1.
    w.write_bit_long(n as i32 - 1);
    w.write_bit_double(0.0);
    for j in 1..n as i32 - 1 {
        w.write_bit_long(j);
        w.write_bit_long(j);
    }
    w.write_bit_long(n as i32 - 1);
    w.write_bit_long(n as i32);
    // Polyline #1: the curve again.
    points(w);
    // Table of the (empty) polyline #2.
    w.write_bit_long(n as i32 - 1);
    w.write_bit_double(0.0);
    for j in 1..n as i32 - 1 {
        w.write_bit_long(j);
        w.write_bit_long(j);
    }
    w.write_bit_long(n as i32 - 1);
    w.write_bit_long(0);
}

/// Adds a mass haul line `name` over `curve` (station, cumulative volume;
/// stations strictly increasing, two or more points) to the sample line
/// group `group`, with the line style `style`, the free-haul distance
/// `free_haul` and the record GUID `guid` (the application assigns
/// sequential GUIDs; a caller passes one, say from its own counter). The
/// record is the create-path form: the free-haul display construction is
/// not written, the application draws it when it regenerates the display.
/// `group` must be a sample line group and `style` a mass haul line style
/// this crate reads; `layer` (layer "0" when `None`) must exist.
pub fn add_mass_haul_line(
    doc: &mut CadDocument,
    group: Handle,
    style: Handle,
    name: &str,
    free_haul: f64,
    curve: &[(f64, f64)],
    guid: [u8; 16],
    layer: Option<&str>,
) -> Result<Handle, DxfError> {
    let n = curve.len();
    let sorted = curve.windows(2).all(|w| w[0].0 < w[1].0);
    let finite = curve.iter().all(|p| p.0.is_finite() && p.1.is_finite());
    if name.is_empty() || n < 2 || !sorted || !finite || !free_haul.is_finite() || free_haul < 0.0 {
        return Err(invalid("a mass haul line needs a name, a free haul of at least 0 and two or more points over increasing stations"));
    }
    let group = arch(doc, group).filter(|a| a.dxf_name == "AECC_SAMPLE_LINE_GROUP").map(|_| group).ok_or_else(|| invalid("the group is not a sample line group"))?;
    object(doc, style).filter(|o| o.dxf_name == "AECC_MASS_HAUL_LINE_STYLE").ok_or_else(|| invalid("the style is not a mass haul line style"))?;
    let handle = doc.allocate_handle();
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    let common = EntityCommon {
        handle,
        owner_handle: doc.header.model_space_block_handle,
        layer: layer.into(),
        layer_handle: Some(layer_handle),
        ..EntityCommon::default()
    };
    let (lo, hi) = curve.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| (a.min(p.1), b.max(p.1)));
    put_entity(
        doc,
        "AECC_MASSHAULLINE",
        common,
        |w| {
            head(w, WORD_LINE);
            bits(w, "00");
            w.write_bit_long(0);
            bits(w, &"0".repeat(128)); // GUID 1: the create path's all-zero form
            bits(w, "11");
            for byte in guid {
                w.write_byte(byte); // GUID 2
            }
            bits(w, &"0".repeat(127));
            w.write_bit_double(free_haul);
            w.write_bit_long(1);
            w.write_bit_long(0);
            w.write_bit_long(n as i32);
            curve_and_tables(w, curve);
            for _ in 0..13 {
                w.write_bit_double(0.0);
            }
            w.write_bit_double(lo);
            w.write_bit_double(hi);
            bits(w, TAIL);
        },
        Body {
            refs: vec![soft_pointer(0), soft_pointer(group.value()), soft_pointer(style.value())],
            strings: vec![IMPS.to_string(), String::new(), name.to_string(), String::new()],
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classes::civil::bits::Bits;
    use crate::io::dwg::{DwgReader, DwgWriter};
    use crate::types::DxfVersion;

    const GUID: [u8; 16] = [0x72, 0x72, 0x0d, 0xc0, 0x23, 0xec, 0x4d, 0x3a, 0xe3, 0xbc, 0x76, 0x5a, 0x84, 0xe3, 0xd1, 0xb5];
    fn curve() -> Vec<(f64, f64)> {
        vec![(0.0, 0.0), (25.0, -641.4001390494934), (50.0, 538.2354845344805), (1210.0, 271308.3549908486), (1658.6262214244061, -89055.10911632466)]
    }

    /// A stub mass haul line style: the style's record itself is not
    /// written yet (the create sample's style is a 435-byte display
    /// component record); a test only needs an object of the class to
    /// reference.
    fn stub_style(doc: &mut CadDocument) -> Handle {
        let handle = doc.allocate_handle();
        put_object(doc, "AECC_MASS_HAUL_LINE_STYLE", handle, Handle::new(0), vec![], |w| w.write_bit_long(0), Body { refs: vec![], strings: vec![] }).unwrap();
        handle
    }

    fn setup() -> (CadDocument, Handle, Handle) {
        let v = crate::types::Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0)], None).unwrap();
        let g = crate::classes::civil::add_sample_line_group(&mut doc, al, "SLG1", None).unwrap();
        let s = stub_style(&mut doc);
        (doc, g, s)
    }

    #[test]
    fn writes_the_create_path_record_and_reads_it_back() {
        let (mut doc, g, s) = setup();
        let cv = curve();
        let h = add_mass_haul_line(&mut doc, g, s, "Total Volume (1)", 500.0, &cv, GUID, None).unwrap();
        // The record reads back through the shipped reader, in memory and
        // after a save/reopen.
        let line = |doc: &CadDocument| arch(doc, h).unwrap().mass_haul_points();
        assert_eq!(line(&doc).iter().map(|p| (p.x, p.y)).collect::<Vec<_>>(), cv);
        let path = std::env::temp_dir().join(format!("civil-mass-haul-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let a = arch(&back, h).unwrap();
        assert_eq!(a.dxf_name, "AECC_MASSHAULLINE");
        assert_eq!(a.cpp_class_name, "AeccDbMassHaulLine");
        assert_eq!(line(&back), line(&doc));
        assert_eq!(a.decode().strings[2], "Total Volume (1)");
        let refs: Vec<u64> = a.typed_handles().iter().map(|r| r.0).collect();
        assert!(refs.contains(&g.value()) && refs.contains(&s.value()));
    }

    #[test]
    fn the_record_carries_the_create_path_form() {
        // Bit-walk of the written class stream: the head words, the
        // all-zero GUID 1, the free haul, the two tables with polyline #2
        // empty (last row 0) and the trailer of 13 zero doubles and the
        // curve's volume range.
        let (mut doc, g, s) = setup();
        let cv = curve();
        let h = add_mass_haul_line(&mut doc, g, s, "MHL", 500.0, &cv, GUID, None).unwrap();
        let raw = arch(&doc, h).unwrap().raw_dwg_data.clone().unwrap();
        let mut best = None;
        for start in 0..raw.len() * 8 - 100 {
            let mut b = Bits::new(&raw, start, raw.len() * 8);
            let ok = [80, 0, 80, 80, 0].iter().all(|w| b.bit_long() == Some(*w))
                && b.bit_long() == Some(0x1002_5F04)
                && b.bit_long() == Some(0x4080_9313)
                && b.bits_value(2) == Some(0)
                && b.bit_long() == Some(0);
            if ok {
                best = Some(start);
                break;
            }
        }
        let start = best.unwrap();
        let mut b = Bits::new(&raw, start, raw.len() * 8);
        for w in [80, 0, 80, 80, 0] {
            assert_eq!(b.bit_long().unwrap(), w);
        }
        assert_eq!(b.bit_long().unwrap(), 0x1002_5F04, "u1");
        assert_eq!(b.bit_long().unwrap(), 0x4080_9313, "the create-path head word");
        b.skip(2).unwrap();
        assert_eq!(b.bit_long().unwrap(), 0);
        b.skip(128).unwrap(); // GUID 1, all zeros
        b.skip(2 + 128 + 127).unwrap();
        assert_eq!(b.bit_double().unwrap(), 500.0);
        assert_eq!((b.bit_long().unwrap(), b.bit_long().unwrap()), (1, 0));
        let n = cv.len();
        assert_eq!(b.bit_long().unwrap(), n as i32);
        let point = |b: &mut Bits| (b.bit_double().unwrap(), b.bit_double().unwrap());
        for p in &cv {
            assert_eq!(point(&mut b), *p);
            assert_eq!(b.bit_double().unwrap(), 0.0);
        }
        // Table 1, polyline #1 (the curve again), table 2 with the last
        // row pointing at 0.
        let table = |b: &mut Bits| {
            assert_eq!(b.bit_long().unwrap(), n as i32 - 1);
            assert_eq!(b.bit_double().unwrap(), 0.0);
            for j in 1..n as i32 - 1 {
                assert_eq!((b.bit_long().unwrap(), b.bit_long().unwrap()), (j, j));
            }
            (b.bit_long().unwrap(), b.bit_long().unwrap())
        };
        assert_eq!(table(&mut b), (n as i32 - 1, n as i32));
        for p in &cv {
            assert_eq!(point(&mut b), *p);
            assert_eq!(b.bit_double().unwrap(), 0.0);
        }
        assert_eq!(table(&mut b), (n as i32 - 1, 0));
        for _ in 0..13 {
            assert_eq!(b.bit_double().unwrap(), 0.0);
        }
        let (lo, hi) = cv.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, c), p| (a.min(p.1), c.max(p.1)));
        assert_eq!((b.bit_double().unwrap(), b.bit_double().unwrap()), (lo, hi));
    }

    #[test]
    fn rejects_bad_inputs() {
        let (mut doc, g, s) = setup();
        assert!(add_mass_haul_line(&mut doc, g, s, "", 500.0, &curve(), GUID, None).is_err());
        assert!(add_mass_haul_line(&mut doc, g, s, "M", 500.0, &[(0.0, 0.0)], GUID, None).is_err());
        assert!(add_mass_haul_line(&mut doc, g, s, "M", 500.0, &[(0.0, 0.0), (0.0, 1.0)], GUID, None).is_err());
        assert!(add_mass_haul_line(&mut doc, g, s, "M", -1.0, &curve(), GUID, None).is_err());
        assert!(add_mass_haul_line(&mut doc, g, s, "M", 500.0, &[(0.0, 0.0), (f64::NAN, 1.0)], GUID, None).is_err());
        assert!(add_mass_haul_line(&mut doc, Handle::new(0xdead), s, "M", 500.0, &curve(), GUID, None).is_err());
        assert!(add_mass_haul_line(&mut doc, g, g, "M", 500.0, &curve(), GUID, None).is_err());
        assert!(add_mass_haul_line(&mut doc, g, s, "M", 500.0, &curve(), GUID, Some("nope")).is_err());
    }
}
