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
//! name is not in the record either (it lives in a sibling `AECC_COMPONENT`
//! record the application owns), so a written entity takes the name the
//! application gives it.
//!
//! The records alone adopt only as `AcDbZombieEntity`: live adoption needs
//! the stormwater model context the application writes around each entity
//! (an extension dictionary at the handle after the entity's, naming the
//! component records, plus drawing-level component-versions and schema
//! tables). The readers at the bottom of this file map that context;
//! emitting it is the documented follow-up (the component records' binary
//! payloads are the application's private save-state in an encoding the
//! corpus does not name).
//!
//! Provenance (D013): the grammar comes from the one drawing these
//! families are known from, the application's regional sample
//! `DE_Regenwassersteuerung_2027.dwg` — cited for provenance, not a
//! reference corpus.

use super::support::{bits, hard_pointer, invalid, put_entity, soft_pointer, Body, Ref};
use crate::entities::{EntityType, ExtendedEntityData};
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

// ---------------------------------------------------------------------------
// The adoption-context map
//
// A written stormwater record is *retained* by the application but adopts as
// `AcDbZombieEntity` in a bare drawing — even with the application's own
// record bytes. Live adoption requires the model context the application
// writes around each entity. Mapped from the one drawing these families are
// known from (provenance below); the readers here expose it so a writer
// round can be verified against it.
//
// Per entity (recorded handles of the pond; the storage and channel follow
// the same layout at their own handles):
//
// ```text
// entity AECC_POND            202D682, owner 1F (modelspace root)
//   extension dictionary      202D683  (= entity handle + 1; the entity's
//                             {ACAD_XDICTIONARY} hard-owner reference)
//     AcDbDictionary, owner = the entity, hard-owner flag set,
//     duplicate-clone flag 1, nine entries (the component family names):
//       AeccDbDrainageConnector                     -> AECC_DRAINAGE_CONNECTOR
//       AnalysisComponent_BasicAnalysisParameters   -> AECC_COMPONENT
//       AnalysisComponent_DepthAreaVolumeTable      -> AECC_COMPONENT
//       AnalysisComponent_OutletDetails             -> AECC_COMPONENT
//       ConnectionComponent                         -> AECC_COMPONENT
//       IdentityComponent                           -> AECC_COMPONENT
//       MeshComponent_2_5D                          -> AECC_SCHEMA_BASED_OBJECT
//       StageStorageDesignTable                     -> AECC_SCHEMA_BASED_OBJECT
//       VolumeGeometryComponent                     -> AECC_COMPONENT
//   component records, each owned by the dictionary (hard owner):
//     [BS version 0x04BE][component words][text stream][handle stream]
//     IdentityComponent texts: "", "1.0.0", the object's name ("RRB - 001")
//     — the entity's *name* lives here, not in the entity record (#641's
//     documented omission). ConnectionComponent texts: "1.0.0", and its
//     handle stream soft-points at the drainage connector. A storage's
//     dictionary drops the `MeshComponent_2_5D` entry (8 entries), a
//     channel's has neither mesh nor table nor connector entries and names
//     the channel's own component families instead (7 entries:
//     AnalysisComponent_FromBaseline, AnalysisComponent_HydraulicParameters,
//     ConnectionComponent, DesignComponent_Baseline,
//     DesignComponent_CrossSection, GeometryComponent_LongitudeLines,
//     IdentityComponent).
// ```
//
// Drawing level, two named-objects entries (recorded: the pond/storage/
// channel drawing's root dictionary):
//
// ```text
// AECC_ENTITY_COMPONENT_VERSIONS -> AcDbDictionary (owner = root, 3 entries)
//   AeccDbPond             -> AECC_ENTITY_COMPONENT_VERSIONS record
//   AeccDbUndergroundStorage -> same class
//   AeccDbChannel            -> same class
//   each record's text stream: "AeccImpEntityComponentVersions", "", "",
//   then four texts per component: the family name and three version texts
//   (recorded "1.1.0", "1.1.0", "1.0.0" for the pond's depth-area-volume
//   component; the three slots' meaning is not established)
// AECC_SCHEMA_ROOT -> AcDbDictionary (owner = root, 4 entries)
//   AeccMeshDataSchema_1_0_0, AeccStageStorageDesignRecordSchema_1_0_0,
//   AeccTriangleIndicesSchema_1_0_0, StageStorageDesignTableSchema_1_0_0
//   -> AECC_SCHEMA_BASED_OBJECT records describing each schema's fields
// ```
//
// What the readers deliberately do not decode: the component records'
// binary payloads after the version word are the application's private
// save-state (the analysis parameters' and volumes' numbers, GUID lists,
// mesh vertices) in an encoding this one drawing does not name — the
// identity component's payload is bit-identical across all three entities
// (it is entity-independent), but it is still unnamed bits, so a writer
// cannot synthesize it (D015). Mapping that encoding, and proving which
// subset of the context the application requires for live adoption, is the
// documented follow-up (#641).
//
// Provenance (D013): `C3D\Sample\DE_Regenwassersteuerung_2027.dwg`, the
// application's regional stormwater sample — the only known drawing with
// these families; cited for provenance, not a reference corpus.

/// Named-objects entry of the drawing-level component-versions table.
const COMPONENT_VERSIONS_ENTRY: &str = "AECC_ENTITY_COMPONENT_VERSIONS";
/// Named-objects entry of the drawing-level schema root.
const SCHEMA_ROOT_ENTRY: &str = "AECC_SCHEMA_ROOT";
/// Extension-dictionary key of the component record that carries the name.
const IDENTITY_COMPONENT: &str = "IdentityComponent";
/// Extension-dictionary key of the drainage connector record.
const DRAINAGE_CONNECTOR_ENTRY: &str = "AeccDbDrainageConnector";

/// One component record of a stormwater entity's model context.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StormWaterComponentContext {
    /// The family name the extension dictionary keys the record with
    /// (`IdentityComponent`, `ConnectionComponent`, ...).
    pub family: String,
    /// The record's handle.
    pub handle: crate::types::Handle,
    /// The object's name — only the identity component writes one (as the
    /// third of its three texts, after the empty prefix and the version).
    pub name: Option<String>,
    /// The version text the record writes (`1.0.0` and kin).
    pub version: Option<String>,
}

/// The model context the application writes around a stormwater control
/// entity: the entity's extension dictionary and the component records it
/// names. Read with [`stormwater_model_context`].
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StormWaterModelContext {
    /// The extension dictionary (recorded at the handle after the entity's
    /// own); its owner is the entity, its entries are hard owners.
    pub extension_dictionary: crate::types::Handle,
    /// The `AECC_COMPONENT` and `AECC_SCHEMA_BASED_OBJECT` records the
    /// dictionary names (a pond's: eight, a storage's: seven, a channel's:
    /// seven). Only the `AECC_COMPONENT` records write name/version texts.
    pub components: Vec<StormWaterComponentContext>,
    /// The drainage connector the dictionary names, when the entity has one
    /// (the recorded pond and storage do, the channel does not).
    pub connector: Option<crate::types::Handle>,
}

/// One class's entry of the drawing-level component-versions table.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StormWaterComponentVersions {
    /// The entity class the entry versions (`AeccDbPond`, ...).
    pub class: String,
    /// The `AECC_ENTITY_COMPONENT_VERSIONS` record's handle.
    pub record: crate::types::Handle,
    /// Per component: the family name and the three version texts the
    /// record writes. Empty when the record's text layout is not the
    /// recorded one (unmapped, not guessed).
    pub components: Vec<(String, [String; 3])>,
}

/// The model context around a stormwater control entity: `None` when the
/// entity is not one of the three families or carries no extension
/// dictionary (a bare, context-less drawing).
pub fn stormwater_model_context(doc: &CadDocument, entity: crate::types::Handle) -> Option<StormWaterModelContext> {
    let xdict = match doc.get_entity(entity) {
        Some(EntityType::Extended(x)) => x.common.xdictionary_handle,
        _ => None,
    }?;
    let dictionary = match doc.objects.get(&xdict) {
        Some(crate::objects::ObjectType::Dictionary(d)) => d,
        _ => return None,
    };
    let is_stormwater = matches!(
        doc.get_entity(entity),
        Some(EntityType::Extended(x)) if matches!(&x.data, ExtendedEntityData::Arch(a)
            if matches!(a.dxf_name.as_str(), "AECC_POND" | "AECC_UNDERGROUNDSTORAGE" | "AECC_CHANNEL"))
    );
    if !is_stormwater {
        return None;
    }
    let mut context = StormWaterModelContext {
        extension_dictionary: xdict,
        components: Vec::new(),
        connector: None,
    };
    for (family, handle) in &dictionary.entries {
        if family == DRAINAGE_CONNECTOR_ENTRY {
            context.connector = Some(*handle);
            continue;
        }
        let (name, version) = match doc.objects.get(handle) {
            Some(crate::objects::ObjectType::Arch(a)) if a.dxf_name == "AECC_COMPONENT" => {
                match a.decode().strings.as_slice() {
                    [prefix, version, name, ..] if prefix.is_empty() => {
                        (Some(name.clone()), Some(version.clone()))
                    }
                    [version] => (None, Some(version.clone())),
                    _ => (None, None),
                }            }
            _ => (None, None),
        };
        context.components.push(StormWaterComponentContext {
            family: family.clone(),
            handle: *handle,
            name,
            version,
        });
    }
    Some(context)
}

/// The drawing-level component-versions table (the named-objects entry
/// `AECC_ENTITY_COMPONENT_VERSIONS`): one entry per stormwater entity class
/// the drawing uses. Empty when the drawing has no such table.
pub fn stormwater_component_versions(doc: &CadDocument) -> Vec<StormWaterComponentVersions> {
    let Some(table) = named_entry(doc, COMPONENT_VERSIONS_ENTRY) else {
        return Vec::new();
    };
    let Some(crate::objects::ObjectType::Dictionary(d)) = doc.objects.get(&table) else {
        return Vec::new();
    };
    d.entries
        .iter()
        .map(|(class, record)| {
            let components = match doc.objects.get(record) {
                Some(crate::objects::ObjectType::Arch(a)) => versions_of(a),
                _ => Vec::new(),
            };
            StormWaterComponentVersions {
                class: class.clone(),
                record: *record,
                components,
            }
        })
        .collect()
}

/// The drawing-level schema root (the named-objects entry
/// `AECC_SCHEMA_ROOT`): the schema records the application's stormwater
/// model data refers to, keyed by schema name. Empty without the entry.
pub fn stormwater_schema_root(doc: &CadDocument) -> Vec<(String, crate::types::Handle)> {
    named_entry(doc, SCHEMA_ROOT_ENTRY)
        .and_then(|h| match doc.objects.get(&h) {
            Some(crate::objects::ObjectType::Dictionary(d)) => Some(d.entries.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

/// The named-objects dictionary entry `key`, `None` without one.
fn named_entry(doc: &CadDocument, key: &str) -> Option<crate::types::Handle> {
    doc.named_objects_dictionary().and_then(|d| d.get(key))
}

/// The versioned component names of an `AECC_ENTITY_COMPONENT_VERSIONS`
/// record: the text stream runs the class prefix, two empty strings, then
/// four texts per component. A record of any other layout is unmapped.
fn versions_of(a: &crate::classes::arch::ArchObject) -> Vec<(String, [String; 3])> {
    let strings = a.decode().strings;
    let Some(rest) = strings.strip_prefix(["AeccImpEntityComponentVersions".to_string(), String::new(), String::new()].as_slice()) else {
        return Vec::new();
    };
    if rest.len() % 4 != 0 {
        return Vec::new();
    }
    rest.chunks_exact(4)
        .map(|c| (c[0].clone(), [c[1].clone(), c[2].clone(), c[3].clone()]))
        .collect()
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

    // -- the adoption-context readers -------------------------------------

    use crate::classes::civil::create::support::{put_object, put_plain_object, soft_pointer};
    use crate::objects::{Dictionary, ObjectType};
    use crate::types::Handle;

    /// Writes one `AECC_COMPONENT` record of the recorded layout: the
    /// version word, the recorded `BD 1.0` word, and the recorded texts.
    /// The record's unnamed payload bits (identity: entity-independent,
    /// connection: per-record) are deliberately omitted — they are not
    /// synthesizable (see the module's context map).
    fn put_component(doc: &mut CadDocument, handle: Handle, owner: Handle, strings: Vec<String>, refs: Vec<Ref>) {
        put_object(doc, "AECC_COMPONENT", handle, owner, vec![], |w| {
            w.write_bit_double(1.0);
        }, Body { refs, strings }).unwrap();
    }

    /// Writes the recorded model context of one entity: the extension
    /// dictionary at the next handle with the family entries, and the
    /// component records it names. Returns the dictionary's handle.
    fn put_context(doc: &mut CadDocument, entity: Handle, entries: &[(&str, Vec<String>)], connector: Option<Handle>) -> Handle {
        let xdict = doc.allocate_handle();
        let mut d = Dictionary::new();
        d.handle = xdict;
        d.owner = entity;
        d.hard_owner = true;
        d.duplicate_cloning = 1;
        for (family, texts) in entries {
            let h = doc.allocate_handle();
            let refs = if *family == "ConnectionComponent" {
                connector.map(|c| vec![soft_pointer(c.value())]).unwrap_or_default()
            } else {
                vec![]
            };
            put_component(doc, h, xdict, texts.clone(), refs);
            d.add_entry(*family, h);
        }
        if let Some(c) = connector {
            let h = doc.allocate_handle();
            put_object(doc, "AECC_DRAINAGE_CONNECTOR", h, xdict, vec![], |w| {
                w.write_bit_double(1.0);
            }, Body { refs: vec![], strings: vec![] }).unwrap();
            d.add_entry("AeccDbDrainageConnector", h);
        }
        doc.objects.insert(xdict, ObjectType::Dictionary(d));
        match doc.get_entity_mut(entity) {
            Some(EntityType::Extended(x)) => x.common.xdictionary_handle = Some(xdict),
            _ => panic!("entity missing"),
        }
        xdict
    }

    /// The recorded pond component set (family, texts).
    fn pond_components(name: &str) -> Vec<(&'static str, Vec<String>)> {
        vec![
            ("IdentityComponent", vec!["".into(), "1.0.0".into(), name.into()]),
            ("ConnectionComponent", vec!["1.0.0".into()]),
            ("AnalysisComponent_DepthAreaVolumeTable", vec!["".into(), "1.1.0".into()]),
            ("AnalysisComponent_OutletDetails", vec!["Auslass - (1)".into(), "1.0.0".into()]),
            ("AnalysisComponent_BasicAnalysisParameters", vec!["1.0.0".into()]),
            ("VolumeGeometryComponent", vec!["1.0.0".into()]),
        ]
    }

    #[test]
    fn maps_the_model_context_round_trip() {
        let mut doc = drawing();
        let pond = add_pond(&mut doc, Vector3::new(495600.609_566_662_8, 574_734.747_287_734_6, 476.7), &outline(476.7), None, None).unwrap();
        let storage = add_underground_storage(&mut doc, Vector3::new(1.0, 2.0, 475.0), &outline(475.0), None, None).unwrap();
        let channel = add_channel(&mut doc, Vector3::new(5.0, 6.0, 473.9), &outline(473.9), None, None).unwrap();

        let pond_xdict = put_context(&mut doc, pond, &pond_components("RRB - 001"), None);
        let storage_xdict = put_context(&mut doc, storage, &pond_components("Speicher - 001"), None);
        let mut channel_components = pond_components("Kanal - 001");
        channel_components[1] = ("ConnectionComponent", vec!["1.0.0".into()]);
        let channel_xdict = put_context(&mut doc, channel, &channel_components, None);

        // The drawing-level tables under the named-objects dictionary.
        let nod = doc.header.named_objects_dict_handle;
        let versions_dict = doc.allocate_handle();
        let schema_dict = doc.allocate_handle();
        for (key, class, handle, owner) in [
            ("AECC_ENTITY_COMPONENT_VERSIONS", "AECC_ENTITY_COMPONENT_VERSIONS", versions_dict, nod),
            ("AECC_SCHEMA_ROOT", "AECC_SCHEMA_BASED_OBJECT", schema_dict, nod),
        ] {
            let mut d = Dictionary::new();
            d.handle = handle;
            d.owner = owner;
            doc.objects.insert(handle, ObjectType::Dictionary(d));
            match doc.objects.get_mut(&nod) {
                Some(ObjectType::Dictionary(n)) => n.add_entry(key, handle),
                _ => panic!("no named objects dictionary"),
            }
        }
        // One recorded versions record per class: the prefix, two empties,
        // then four texts per component.
        let mut versions = Dictionary::new();
        versions.handle = versions_dict;
        versions.owner = nod;
        for (class, count) in [("AeccDbPond", 10usize), ("AeccDbUndergroundStorage", 8), ("AeccDbChannel", 8)] {
            let h = doc.allocate_handle();
            let mut strings = vec!["AeccImpEntityComponentVersions".to_string(), String::new(), String::new()];
            for i in 0..count {
                strings.push(format!("Component{i}"));
                strings.push("1.0.0".into());
                strings.push("1.0.0".into());
                strings.push("1.0.0".into());
            }
            put_plain_object(&mut doc, "AECC_ENTITY_COMPONENT_VERSIONS", h, versions_dict, vec![], |w| {
                w.write_bit_short(80);
                w.write_bit(true);
                w.write_bit(false);
            }, Body { refs: vec![], strings }).unwrap();
            versions.add_entry(class, h);
        }
        doc.objects.insert(versions_dict, ObjectType::Dictionary(versions));
        let mut schemas = Dictionary::new();
        schemas.handle = schema_dict;
        schemas.owner = nod;
        for schema in [
            "AeccMeshDataSchema_1_0_0",
            "AeccStageStorageDesignRecordSchema_1_0_0",
            "AeccTriangleIndicesSchema_1_0_0",
            "StageStorageDesignTableSchema_1_0_0",
        ] {
            let h = doc.allocate_handle();
            put_plain_object(&mut doc, "AECC_SCHEMA_BASED_OBJECT", h, schema_dict, vec![], |w| {
                w.write_bit_short(80);
                w.write_bit(false);
            }, Body { refs: vec![], strings: vec![schema.split("_1_0_0").next().unwrap().to_string()] }).unwrap();
            schemas.add_entry(schema, h);
        }
        doc.objects.insert(schema_dict, ObjectType::Dictionary(schemas));

        let path = std::env::temp_dir().join(format!("sw_ctx_{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);

        // The pond's context: dictionary, entries, the name, the version.
        let ctx = stormwater_model_context(&back, pond).expect("pond context missing");
        assert_eq!(ctx.extension_dictionary, pond_xdict);
        assert_eq!(ctx.components.len(), 6);
        let identity = ctx.components.iter().find(|c| c.family == "IdentityComponent").unwrap();
        assert_eq!(identity.name.as_deref(), Some("RRB - 001"));
        assert_eq!(identity.version.as_deref(), Some("1.0.0"));
        let connection = ctx.components.iter().find(|c| c.family == "ConnectionComponent").unwrap();
        assert_eq!(connection.name, None);
        assert_eq!(connection.version.as_deref(), Some("1.0.0"));

        let storage_ctx = stormwater_model_context(&back, storage).unwrap();
        assert_eq!(storage_ctx.components.iter().find(|c| c.family == "IdentityComponent").unwrap().name.as_deref(), Some("Speicher - 001"));
        let channel_ctx = stormwater_model_context(&back, channel).unwrap();
        assert_eq!(channel_ctx.components.iter().find(|c| c.family == "IdentityComponent").unwrap().name.as_deref(), Some("Kanal - 001"));

        // The drawing-level tables.
        let versions = stormwater_component_versions(&back);
        assert_eq!(versions.len(), 3);
        let pond_versions = versions.iter().find(|v| v.class == "AeccDbPond").unwrap();
        assert_eq!(pond_versions.components.len(), 10);
        assert_eq!(pond_versions.components[0].0, "Component0");
        assert_eq!(pond_versions.components[0].1, ["1.0.0".to_string(), "1.0.0".into(), "1.0.0".into()]);
        assert!(versions.iter().all(|v| !v.components.is_empty()));

        let schemas = stormwater_schema_root(&back);
        assert_eq!(schemas.len(), 4);
        assert!(schemas.iter().any(|(k, _)| k == "AeccMeshDataSchema_1_0_0"));

        // The context did not disturb the records: all three still decode.
        for (handle, kind) in [(pond, "Pond"), (storage, "UndergroundStorage"), (channel, "Channel")] {
            let data = arch_of(&back, handle);
            let sw = data.stormwater().expect(kind);
            assert_eq!(sw.description.split(' ').next().unwrap(), format!("AeccDb{kind}"));
        }
    }

    #[test]
    fn a_bare_entity_has_no_context() {
        let mut doc = drawing();
        let pond = add_pond(&mut doc, Vector3::new(0.0, 0.0, 1.0), &outline(1.0), None, None).unwrap();
        let path = std::env::temp_dir().join(format!("sw_bare_{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(stormwater_model_context(&back, pond), None);
        assert!(stormwater_component_versions(&back).is_empty());
        assert!(stormwater_schema_root(&back).is_empty());
    }

    #[test]
    fn refuses_foreign_versions_layout() {
        let mut doc = drawing();
        // The recorded layout: the named-objects entry points at a table
        // dictionary, the per-class records at its entries.
        let nod = doc.header.named_objects_dict_handle;
        let table = doc.allocate_handle();
        let mut d = Dictionary::new();
        d.handle = table;
        d.owner = nod;
        doc.objects.insert(table, ObjectType::Dictionary(d));
        match doc.objects.get_mut(&nod) {
            Some(ObjectType::Dictionary(n)) => n.add_entry("AECC_ENTITY_COMPONENT_VERSIONS", table),
            _ => panic!("no named objects dictionary"),
        }
        let h = doc.allocate_handle();
        // A record whose text stream is not the recorded four-per-component
        // layout: unmapped, so the entry reads with no components.
        put_plain_object(&mut doc, "AECC_ENTITY_COMPONENT_VERSIONS", h, table, vec![], |w| {
            w.write_bit_short(80);
            w.write_bit(true);
            w.write_bit(false);
        }, Body { refs: vec![], strings: vec!["AeccImpEntityComponentVersions".into(), "x".into()] }).unwrap();
        match doc.objects.get_mut(&table) {
            Some(ObjectType::Dictionary(t)) => t.add_entry("AeccDbPond", h),
            _ => panic!("versions table missing"),
        }
        let versions = stormwater_component_versions(&doc);
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].class, "AeccDbPond");
        assert!(versions[0].components.is_empty());
    }
}
