//! Stormwater control entities: pond (`AECC_POND`), underground storage
//! (`AECC_UNDERGROUNDSTORAGE`) and channel (`AECC_CHANNEL`).
//!
//! A record carries the placement origin, the drawn outline (the sections of
//! the stormwater graphic image, world coordinates) and the style
//! reference — nothing else (see the reading side in
//! [`super::super::stormwater`]). The writers emit exactly those fields:
//! the outline is closed by repeating its first point, the axes are the
//! identity frame, and the style is a null hard pointer unless one is
//! given. The application completes the rest when it adopts the drawing:
//! the hydraulic parameters (a pond's depth, freeboard and infiltration, a
//! channel's Manning coefficient and trapezoidal cross-section) are not
//! stored in such records at all — the application derives them — and the
//! name is not in the record either (it lives in a sibling
//! `AECC_COMPONENT` record the application owns), so a written entity takes
//! the name the application gives it.
//!
//! Provenance (D013): the grammar comes from the one drawing these
//! families are known from, the application's regional sample
//! `DE_Regenwassersteuerung_2027.dwg` — cited for provenance, not a
//! reference corpus.

use super::support::{bits, hard_pointer, invalid, put_entity, soft_pointer, Body, Ref};
use crate::io::dwg::dwg_reference_type::DwgReferenceType;
use crate::entities::EntityCommon;
use crate::error::DxfError;
use crate::types::Vector3;
use crate::CadDocument;

/// Schema word of the graphic image, per family (recorded).
const POND_IMAGE: &str = "AeccDbPond (AeccStormWaterControl130)";
const STORAGE_IMAGE: &str = "AeccDbUndergroundStorage (AeccStormWaterControl130)";
const CHANNEL_IMAGE: &str = "AeccDbChannel (AeccDrainageChannel130)";

/// Kind word of the graphic image: 3 for an area control, 8 for a channel.
const IMAGE_AREA: u32 = 3;
const IMAGE_CHANNEL: u32 = 8;

// The class-body flags after the shared head and schema word (recorded:
// a pond writes 0b0011, a storage or channel 0b0010).

/// Adds a pond at `origin` with the closed outline `outline` (three or
/// more finite points; the record repeats the first point at the end, as
/// the application writes it). The points' elevations are the drawn
/// outline's elevations (the application reads a pond's top elevation from
/// them). `style`, when given, is the hard-pointed `AECC_POND_STYLE`;
/// without one the record refers to no style and the application
/// substitutes its default. The layer must exist.
pub fn add_pond(doc: &mut CadDocument, origin: Vector3, outline: &[Vector3], style: Option<crate::types::Handle>, layer: Option<&str>) -> Result<crate::types::Handle, DxfError> {
    add_stormwater(
        doc,
        "AECC_POND",
        IMAGE_AREA,
        POND_IMAGE,
        "AeccImpPond",
        "0011",
        &[(0x0B00_1081, &[0x74A5_FEB3, 0x0357_16D9]), (0x100A_9100, &[]), (0x040A_9091, &[])],
        origin,
        outline,
        style,
        layer,
    )
}

/// Adds an underground storage; see [`add_pond`].
pub fn add_underground_storage(doc: &mut CadDocument, origin: Vector3, outline: &[Vector3], style: Option<crate::types::Handle>, layer: Option<&str>) -> Result<crate::types::Handle, DxfError> {
    add_stormwater(
        doc,
        "AECC_UNDERGROUNDSTORAGE",
        IMAGE_AREA,
        STORAGE_IMAGE,
        "AeccImpUndergroundStorage",
        "0010",
        &[(0x100A_91B1, &[]), (0x040A_9091, &[])],
        origin,
        outline,
        style,
        layer,
    )
}

/// Adds a channel; see [`add_pond`]. The outline is the channel's drawn
/// plan rectangle; the application reads the channel's run from a feature
/// line it associates the record with (a written record carries no such
/// reference — the application completes one when the drawing is adopted).
pub fn add_channel(doc: &mut CadDocument, origin: Vector3, outline: &[Vector3], style: Option<crate::types::Handle>, layer: Option<&str>) -> Result<crate::types::Handle, DxfError> {
    add_stormwater(
        doc,
        "AECC_CHANNEL",
        IMAGE_CHANNEL,
        CHANNEL_IMAGE,
        "AeccImpChannel",
        "0010",
        &[(0x100A_B001, &[]), (0x040A_B002, &[])],
        origin,
        outline,
        style,
        layer,
    )
}

#[allow(clippy::too_many_arguments)]
fn add_stormwater(
    doc: &mut CadDocument,
    dxf: &str,
    image_kind: u32,
    image_word: &str,
    imp: &str,
    flags: &str,
    keys: &[(u32, &[i32])],
    origin: Vector3,
    outline: &[Vector3],
    style: Option<crate::types::Handle>,
    layer: Option<&str>,
) -> Result<crate::types::Handle, DxfError> {
    if outline.len() < 3
        || outline
            .iter()
            .any(|p| !p.x.is_finite() || !p.y.is_finite() || !p.z.is_finite())
        || !origin.x.is_finite()
        || !origin.y.is_finite()
        || !origin.z.is_finite()
    {
        return Err(invalid("a stormwater outline needs three or more finite points"));
    }
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    let mut points = outline.to_vec();
    if points[0] != points[points.len() - 1] {
        points.push(points[0]);
    }
    let common = EntityCommon {
        handle: doc.allocate_handle(),
        owner_handle: doc.header.model_space_block_handle,
        layer: layer.into(),
        layer_handle: Some(layer_handle),
        graphic_data: Some(graphic_image(image_kind, image_word, 1, origin, &[points])),
        ..EntityCommon::default()
    };
    let refs: Vec<Ref> = vec![hard_pointer(style.map(|h| h.value()).unwrap_or(0)), soft_pointer(0), Ref(DwgReferenceType::HardOwnership, 0)];
    let strings = vec![imp.to_string(), String::new(), String::new(), "1.0.0".to_string()];
    put_entity(
        doc,
        dxf,
        common,
        |w| {
            for v in [80, 0, 80, 80, 0, 0x1002_5F04, 80] {
                w.write_bit_long(v);
            }
            bits(w, flags);
            // The class keys (recorded schema words; see the module notes)
            // and their `BL 0` terminator. The application reads the key
            // list to its terminator — a body that ends earlier is refused.
            for (key, values) in keys {
                w.write_bit_long(*key as i32);
                match *values {
                    [] => {}
                    list => {
                        w.write_bit_long(list.len() as i32);
                        for v in list {
                            w.write_bit_long(*v);
                        }
                    }
                }
            }
            w.write_bit_long(0);
        },
        Body { refs, strings },
    )
}

/// Builds the stormwater graphic image from its fields:
///
/// ```text
/// [u32 self size][u32 kind][u32 sections - 8][u32 36]
/// RD origin x y z, 3 x 3 RD identity axes,
/// UTF-16 NUL-terminated schema word,
/// u32 size (=12+24n), u32 6, u32 n, n x (RD x,y,z), u32 8, u32 21
/// ```
pub(super) fn graphic_image(kind: u32, word: &str, nul_units: usize, origin: Vector3, rings: &[Vec<Vector3>]) -> Vec<u8> {
    let mut blob = Vec::new();
    let u32_at = |blob: &mut Vec<u8>, v: u32| blob.extend_from_slice(&v.to_le_bytes());
    let double_at = |blob: &mut Vec<u8>, v: f64| blob.extend_from_slice(&v.to_le_bytes());
    u32_at(&mut blob, 0); // self size, patched below
    u32_at(&mut blob, kind);
    let text_units = word.encode_utf16().count() + nul_units;
    let sections_at = 16 + 96 + text_units * 2;
    u32_at(&mut blob, (sections_at - 8) as u32);
    u32_at(&mut blob, 36);
    double_at(&mut blob, origin.x);
    double_at(&mut blob, origin.y);
    double_at(&mut blob, origin.z);
    for axis in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] {
        double_at(&mut blob, axis[0]);
        double_at(&mut blob, axis[1]);
        double_at(&mut blob, axis[2]);
    }
    for unit in word.encode_utf16() {
        blob.extend_from_slice(&unit.to_le_bytes());
    }
    for _ in 0..nul_units {
        blob.extend_from_slice(&[0, 0]);
    }
    debug_assert_eq!(blob.len(), sections_at);
    for ring in rings {
        let size = (12 + ring.len() * 24) as u32;
        u32_at(&mut blob, size);
        u32_at(&mut blob, 6);
        u32_at(&mut blob, ring.len() as u32);
        for p in ring {
            double_at(&mut blob, p.x);
            double_at(&mut blob, p.y);
            double_at(&mut blob, p.z);
        }
    }
    u32_at(&mut blob, 8);
    u32_at(&mut blob, 21);
    let total = blob.len() as u32;
    blob[0..4].copy_from_slice(&total.to_le_bytes());
    blob
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classes::civil::create::support::register;
    use crate::entities::{EntityType, ExtendedEntityData};
    use crate::io::dwg::{DwgReader, DwgWriter};
    use crate::types::DxfVersion;

    fn drawing() -> CadDocument {
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        doc
    }

    fn outline(z: f64) -> Vec<Vector3> {
        vec![
            Vector3::new(10.0, 20.0, z),
            Vector3::new(50.0, 20.0, z),
            Vector3::new(50.0, 60.0, z),
            Vector3::new(10.0, 60.0, z),
        ]
    }

    fn arch_of(doc: &CadDocument, handle: crate::types::Handle) -> crate::classes::arch::ArchEntityData {
        match doc.get_entity(handle) {
            Some(EntityType::Extended(x)) => match &x.data {
                ExtendedEntityData::Arch(a) => a.clone(),
                _ => panic!("not an arch entity"),
            },
            _ => panic!("entity missing"),
        }
    }

    #[test]
    fn writes_pond_storage_channel_round_trip() {
        let mut doc = drawing();
        let origin = Vector3::new(495600.609_566_662_8, 574_734.747_287_734_6, 476.7);
        let pond = add_pond(&mut doc, origin, &outline(476.7), None, None).unwrap();
        let storage = add_underground_storage(&mut doc, Vector3::new(1.0, 2.0, 475.0), &outline(475.0), None, None).unwrap();
        let channel = add_channel(&mut doc, Vector3::new(5.0, 6.0, 473.935_558_722_156_77), &outline(473.935_558_722_156_77), None, None).unwrap();

        let path = std::env::temp_dir().join(format!("sw_write_{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        // The file header's two maintenance bytes must carry the reference
        // application's own value (0): a non-zero byte makes the
        // application report the drawing as written by a newer release.
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[0..6], b"AC1032");
        assert_eq!(bytes[0x0B], 0, "maintenance byte 0x0B must match the application's files");
        assert_eq!(bytes[0x12], 0, "maintenance byte 0x12 must match the application's files");
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);

        let pond_data = arch_of(&back, pond);
        let sw = pond_data.stormwater().expect("pond does not decode");
        assert_eq!(sw.kind, crate::classes::civil::stormwater::StormWaterKind::Pond);
        assert_eq!(sw.description, POND_IMAGE);
        assert_eq!(sw.origin, origin);
        assert_eq!(sw.style, 0);
        assert_eq!(sw.outlines.len(), 1);
        assert_eq!(sw.outlines[0].len(), 5);
        assert_eq!(sw.outlines[0][0].x, 10.0);
        assert_eq!(sw.outlines[0][0].z, 476.7);
        assert_eq!(sw.outlines[0][4], sw.outlines[0][0]);

        let storage_data = arch_of(&back, storage);
        let sw = storage_data.stormwater().expect("storage does not decode");
        assert_eq!(sw.kind, crate::classes::civil::stormwater::StormWaterKind::UndergroundStorage);
        assert_eq!(sw.description, STORAGE_IMAGE);
        assert_eq!(sw.origin.z, 475.0);

        let channel_data = arch_of(&back, channel);
        let sw = channel_data.stormwater().expect("channel does not decode");
        assert_eq!(sw.kind, crate::classes::civil::stormwater::StormWaterKind::Channel);
        assert_eq!(sw.description, CHANNEL_IMAGE);
        assert_eq!(sw.origin.z, 473.935_558_722_156_77);
    }

    #[test]
    fn writes_a_given_style_handle() {
        let mut doc = drawing();
        let style = doc.allocate_handle();
        let pond = add_pond(&mut doc, Vector3::new(0.0, 0.0, 100.0), &outline(100.0), Some(style), None).unwrap();
        let data = arch_of(&doc, pond);
        let sw = data.stormwater().unwrap();
        assert_eq!(sw.style, style.value());
        assert_eq!(data.typed_handles().first().copied(), Some((style.value(), 5)));
    }

    #[test]
    fn refuses_short_and_nonfinite_outlines() {
        let mut doc = drawing();
        assert!(add_pond(&mut doc, Vector3::new(0.0, 0.0, 0.0), &outline(0.0)[..2], None, None).is_err());
        let mut nan = outline(0.0);
        nan[1].y = f64::NAN;
        assert!(add_pond(&mut doc, Vector3::new(0.0, 0.0, 0.0), &nan, None, None).is_err());
        assert!(add_pond(&mut doc, Vector3::new(0.0, 0.0, 0.0), &outline(0.0), None, Some("no-such-layer")).is_err());
        // the class registers even on refusals
        assert!(register(&mut doc, "AECC_POND").is_ok());
    }

    #[test]
    fn closes_an_open_outline_once() {
        let mut doc = drawing();
        let mut closed = outline(1.0);
        closed.push(closed[0]);
        let handle = add_pond(&mut doc, Vector3::new(0.0, 0.0, 1.0), &closed, None, None).unwrap();
        let data = arch_of(&doc, handle);
        let sw = data.stormwater().unwrap();
        // the closed input is stored as-is, not closed twice
        assert_eq!(sw.outlines[0].len(), 5);
    }
}
