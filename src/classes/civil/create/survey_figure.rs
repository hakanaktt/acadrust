//! Survey figures (`AECC_SVFIGURE`), written in the straight 2D record form
//! the application stores against a SQLite survey database (the 2026-11-27
//! svfig capture's app-written records: TESTFIG1, state1.dwg `0x2647`, and
//! LONG-NAME-CAPTURE-…, captures.dwg `0x2643`; the 2D record of CURVED-
//! CAPTURE-1 `0x2654` pins the same class stream — the arc lives only in
//! the application's object model, so a written figure is straight by
//! construction). The record:
//!
//! * the class stream opens with the 513-bit head the capture's four
//!   DB-route records share: `BL 80, 0, 80, 80, 0`, `BL 0x045F0210`,
//!   `BL(byte) 0x20`, three `BL 0`, `BL 0x0901`, the GUID marker `11`, the
//!   survey database's 128-bit id, and the per-figure block. The database
//!   id and the per-figure block (head bits [216,344) and [355,436)) are
//!   the application's own — the record carries them nowhere else (they
//!   appear in no network, style or face record), so the writer carries
//!   them from the drawing's own straight-2D figure and refuses a drawing
//!   that has none: the survey figures of a drawing belong to its survey
//!   database, and a second figure joins it rather than inventing one.
//! * the head closes with the straight form's 67-bit tail (eight `0x18`
//!   bytes at bit 436, then `001`), the 8-bit count word and a 2-bit gap.
//! * the traverse cache keeps `count = vertices + 1` entries of
//!   `[RD x][00][RD y][1000]` — z is absent (a planar figure) and the
//!   constant tag replaces the 3D form's `[00] + RD z + [00]` — the closed
//!   figure's closure repeat of the first vertex last, then the 17-bit
//!   trailer `01011111000000100`.
//! * the texts read `AeccImpSvFigure`, ``, ``, name, site (the 5-string
//!   form; the name is the fourth string — the mapping the live
//!   application confirmed for the DB dialect) and the handles read
//!   `[null, display style ×2, null, figure style, null, site face]` in
//!   the record's stream order. The record carries no survey-network
//!   reference: the network association lives in the survey database
//!   alone.
//! * the graphic-image section sketches the traverse's bounding box: one
//!   closed five-point rectangle at the figure's elevation (z 0 for the
//!   2D form) inset 0.1 drawing units from the low corner, under the
//!   `AeccDbSvFigure (AeccSurvey130)` schema word. The application's own
//!   corners are recomputed from its database and sit within the shared
//!   solved-point tolerance of the cache doubles without matching them bit
//!   for bit; the writer emits the cache bbox rounded to the nearest
//!   multiple of 256 ulps — the closest value the record's own data
//!   determines (the pins' corners match it within 0x100 ulps, most
//!   exactly).

use super::stormwater::graphic_image;
use super::support::*;
use crate::entities::{EntityCommon, EntityType, ExtendedEntityData};
use crate::error::DxfError;
use crate::io::dwg::dwg_stream_writers::DwgMergedWriter;
use crate::types::{Handle, Vector3};
use crate::CadDocument;

/// The head bits the writer carries from the drawing's own straight-2D
/// figure: the shared civil head, the schema word, the survey database's id
/// and the per-figure block ([`add_survey_figure`]).
const CONTEXT_BITS: usize = 436;
/// The 17-bit run that closes every traverse cache (the 2D and 3D forms').
const TAIL: &str = "01011111000000100";
/// The record's first text (the class's import name).
const IMPS: &str = "AeccImpSvFigure";
/// The graphic-image section's kind word (2D one-ring rectangle) and schema
/// word, as the capture's records keep them.
const GRAPHIC_KIND: u32 = 3;
const GRAPHIC_WORD: &str = "AeccDbSvFigure (AeccSurvey130)";
/// The graphic rectangle's inset from the bbox's low corner, in drawing
/// units (the capture's records: the origin sits 0.1 past the low corner).
const GRAPHIC_INSET: f64 = 0.1;

/// Rounds a double's mantissa to the nearest multiple of 256 ulps (ties
/// up): the quantization the application's display path applies and the
/// closest value to its recomputed graphic corners that the record's own
/// cache doubles determine (see the [module](self) documentation).
fn quantize(value: f64) -> f64 {
    let b = value.to_bits();
    let rounded = if b & 0xFF >= 0x80 { (b & !0xFF) + 0x100 } else { b & !0xFF };
    f64::from_bits(rounded)
}

/// The drawing's straight-2D survey figure: the context donor whose head
/// the new figure's record carries ([`add_survey_figure`]). A figure is a
/// donor when the shipped reader decodes it as the 2D form — a z-absent
/// traverse (every vertex at z 0) and the flat one-ring bbox solid.
fn context_donor(doc: &CadDocument) -> Option<Handle> {
    for e in doc.entities() {
        let EntityType::Extended(x) = e else { continue };
        let ExtendedEntityData::Arch(a) = &x.data else { continue };
        if a.dxf_name != "AECC_SVFIGURE" {
            continue;
        }
        let Some(g) = a.survey_figure_geometry() else { continue };
        if g.vertices.iter().all(|v| v.z == 0.0) && g.min_elevation == g.max_elevation {
            return Some(x.common.handle);
        }
    }
    None
}

/// The first 436 bits of a straight-2D record's class stream: the shared
/// civil head, the schema word, the survey database's id and the
/// per-figure block the application owns (see the [module](self)
/// documentation). The class stream opens with the shared civil head the
/// shipped readers anchor on; a record without it has no context to
/// carry.
fn context_bits(doc: &CadDocument, donor: Handle) -> Option<String> {
    let EntityType::Extended(x) = doc.get_entity(donor)? else { return None };
    let ExtendedEntityData::Arch(a) = &x.data else { return None };
    let raw = a.raw_dwg_data.as_ref()?;
    let mut bit_string = String::with_capacity(raw.len() * 8);
    for byte in raw {
        for k in (0..8).rev() {
            bit_string.push(if (byte >> k) & 1 == 1 { '1' } else { '0' });
        }
    }
    // the shared civil head: BL 80, 0, 80, 80, 0
    let prefix = "0101010000100101010000010101000010";
    let start = bit_string.find(prefix)?;
    Some(bit_string[start..start + CONTEXT_BITS].to_string())
}

/// Adds a closed straight survey figure `name` on survey site `site` over
/// `vertices` (the traverse's vertices in order, three or more) to the
/// drawing, referencing the site's face `site_face`, the figure's display
/// style `display_style` and the figure style `figure_style` — the three
/// references the record's handle stream keeps. The record is the straight
/// 2D form the application writes against its SQLite survey database, and
/// it carries that database's id and per-figure state from the drawing's
/// own straight-2D survey figure: a drawing without one has no survey
/// database for the figure to join (the writer does not invent one — see
/// the [module](self) documentation). `layer` (layer "0" when `None`)
/// must exist; the three references must be objects of the classes the
/// capture's records keep.
#[allow(clippy::too_many_arguments)]
pub fn add_survey_figure(
    doc: &mut CadDocument,
    site_face: Handle,
    display_style: Handle,
    figure_style: Handle,
    name: &str,
    site: &str,
    vertices: &[(f64, f64)],
    layer: Option<&str>,
) -> Result<Handle, DxfError> {
    let n = vertices.len();
    let finite = vertices.iter().all(|p| p.0.is_finite() && p.1.is_finite());
    if name.is_empty() || site.is_empty() || n < 3 || !finite {
        return Err(invalid(
            "a survey figure needs a name, a site and three or more finite vertices",
        ));
    }
    let is_parcel_face = match doc.get_entity(site_face) {
        Some(EntityType::Extended(x)) => matches!(&x.data, ExtendedEntityData::Arch(a) if a.dxf_name == "AECC_PARCEL"),
        _ => object(doc, site_face).is_some_and(|o| o.dxf_name == "AECC_PARCEL"),
    };
    if !is_parcel_face {
        return Err(invalid("the site face is not a parcel face record"));
    }
    object(doc, display_style)
        .filter(|o| o.dxf_name == "AECC_FEATURE_STYLE")
        .map(|_| ())
        .ok_or_else(|| invalid("the display style is not a feature style record"))?;
    object(doc, figure_style)
        .filter(|o| o.dxf_name == "AECC_SVFIGURE_STYLE")
        .map(|_| ())
        .ok_or_else(|| invalid("the figure style is not a survey figure style record"))?;
    let donor = context_donor(doc).ok_or_else(|| {
        invalid(
            "the drawing has no straight-2D survey figure to carry the record's survey-database context: a survey figure joins the drawing's own survey database, and the record keeps the database's id nowhere else",
        )
    })?;
    let context = context_bits(doc, donor)
        .ok_or_else(|| invalid("the drawing's survey figure record has no readable context head"))?;
    let handle = doc.allocate_handle();
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    let (min_x, min_y, max_x, max_y) = vertices.iter().fold(
        (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY),
        |(a, b, c, d), p| (a.min(p.0), b.min(p.1), c.max(p.0), d.max(p.1)),
    );
    let (lo_x, lo_y) = (quantize(min_x), quantize(min_y));
    let ring = vec![
        Vector3::new(lo_x, lo_y, 0.0),
        Vector3::new(quantize(max_x), lo_y, 0.0),
        Vector3::new(quantize(max_x), quantize(max_y), 0.0),
        Vector3::new(lo_x, quantize(max_y), 0.0),
        Vector3::new(lo_x, lo_y, 0.0),
    ];
    let graphic = graphic_image(
        GRAPHIC_KIND,
        GRAPHIC_WORD,
        2,
        Vector3::new(lo_x + GRAPHIC_INSET, lo_y + GRAPHIC_INSET, 0.0),
        &[ring],
    );
    let common = EntityCommon {
        handle,
        owner_handle: doc.header.model_space_block_handle,
        layer: layer.into(),
        layer_handle: Some(layer_handle),
        graphic_data: Some(graphic),
        ..EntityCommon::default()
    };
    put_entity(
        doc,
        "AECC_SVFIGURE",
        common,
        |w| {
            bits(w, &context);
            // the straight form's head tail: eight 0x18 bytes then `001`
            for _ in 0..8 {
                bits(w, "00011000");
            }
            bits(w, "001");
            // the count word: the traverse's entries including the closure
            w.write_byte(n as u8 + 1);
            bits(w, "00");
            let entry = |w: &mut DwgMergedWriter, p: (f64, f64)| {
                w.write_raw_double(p.0);
                bits(w, "00");
                w.write_raw_double(p.1);
                bits(w, "1000");
            };
            for p in vertices {
                entry(w, *p);
            }
            entry(w, vertices[0]); // the closure repeat of the first vertex
            bits(w, TAIL);
        },
        Body {
            refs: vec![
                soft_pointer(0),
                hard_pointer(display_style.value()),
                hard_pointer(display_style.value()),
                soft_pointer(0),
                hard_pointer(figure_style.value()),
                soft_pointer(0),
                hard_pointer(site_face.value()),
            ],
            strings: vec![
                IMPS.to_string(),
                String::new(),
                String::new(),
                name.to_string(),
                site.to_string(),
            ],
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::dwg::{DwgReader, DwgWriter};
    use crate::types::DxfVersion;

    /// The class streams of the capture's straight-2D pins, from the
    /// records' raw bytes at the shipped readers' anchor (the shared
    /// `BL 80, 0, 80, 80, 0` head) to the 17-bit trailer. TESTFIG1 =
    /// state1.dwg `0x2647`; LONG-NAME = captures.dwg `0x2643`; CURVED =
    /// captures.dwg `0x2654` (its class stream only — its graphic is the
    /// application's own curved geometry, extended by the arc).
    const TESTFIG1_STREAM: &str = include_str!("survey_figure_testfig1_stream.txt");
    const LONGNAME_STREAM: &str = include_str!("survey_figure_longname_stream.txt");
    const CURVED_STREAM: &str = include_str!("survey_figure_curved_stream.txt");
    /// The traverse vertices the pins' caches keep, bit for bit (the
    /// closure repeat last), as the capture's records store them.
    const TESTFIG1_CACHE_BITS: [[u64; 2]; 5] = [
        [0x40b343abe55ee4e1, 0x40b523b5a5c6a1b2],
        [0x40b35dab33a7c3c1, 0x40b56d183bdddc06],
        [0x40b3b03f94e1dff3, 0x40b52600baa9d0d2],
        [0x40b36c325796b30b, 0x40b4dfacea904730],
        [0x40b343abe55ee4e1, 0x40b523b5a5c6a1b2],
    ];
    const LONGNAME_CACHE_BITS: [[u64; 2]; 7] = [
        [0x40b3ac3a0f7d4409, 0x40b5d51c7a8743fd],
        [0x40b3e72fb29629bc, 0x40b60d196ff2cce4],
        [0x40b435cc8c0cb154, 0x40b5ca63deacd774],
        [0x40b3fc07d488e3dc, 0x40b57f579aa6c372],
        [0x40b3a0e94e120014, 0x40b58e3b8f9df834],
        [0x40b3daae049e2076, 0x40b5bfab424bdcca],
        [0x40b3ac3a0f7d4409, 0x40b5d51c7a8743fd],
    ];
    const CURVED_CACHE_BITS: [[u64; 2]; 4] = [
        [0x40b41aceaa0c7031, 0x40b70dc50a727c61],
        [0x40b48eebbbe00332, 0x40b731e77515371d],
        [0x40b4a354906d127b, 0x40b6fabcae7eb1b4],
        [0x40b41aceaa0c7031, 0x40b70dc50a727c61],
    ];

    fn vertices_of(bits: &[[u64; 2]], n: usize) -> Vec<(f64, f64)> {
        bits[..n].iter().map(|p| (f64::from_bits(p[0]), f64::from_bits(p[1]))).collect()
    }

    fn bit_string(raw: &[u8]) -> String {
        let mut s = String::with_capacity(raw.len() * 8);
        for byte in raw {
            for k in (0..8).rev() {
                s.push(if (byte >> k) & 1 == 1 { '1' } else { '0' });
            }
        }
        s
    }

    /// The written figure's class stream (from its shared-head anchor to
    /// the trailer).
    fn stream_of(doc: &CadDocument, h: Handle) -> String {
        let EntityType::Extended(x) = doc.get_entity(h).unwrap() else { panic!("entity") };
        let ExtendedEntityData::Arch(a) = &x.data else { panic!("arch") };
        let raw = a.raw_dwg_data.clone().unwrap();
        let bits = bit_string(&raw);
        let prefix = "0101010000100101010000010101000010";
        let start = bits.find(prefix).unwrap();
        let mut b = crate::classes::civil::bits::Bits::new(raw.as_slice(), start + 503, raw.len() * 8);
        let count = b.raw_short_byte().unwrap() as usize;
        let stream_end = start + 513 + count * 134 + 17;
        bits[start..stream_end].to_string()
    }

    /// A stub of each reference class, so a test can reference records of
    /// the record's classes without writing those families. The stub keeps
    /// the class's own record kind: the site face is an entity record, the
    /// styles are dictionary objects.
    fn stub_ref(doc: &mut CadDocument, dxf: &str) -> Handle {
        let handle = doc.allocate_handle();
        if dxf == "AECC_PARCEL" {
            let common = EntityCommon {
                handle,
                owner_handle: doc.header.model_space_block_handle,
                layer: "0".into(),
                layer_handle: doc.layers.get("0").map(|l| l.handle),
                ..EntityCommon::default()
            };
            put_entity(doc, dxf, common, |w| w.write_bit_long(0), Body { refs: vec![], strings: vec![] }).unwrap();
        } else {
            put_object(doc, dxf, handle, Handle::new(0), vec![], |w| w.write_bit_long(0), Body { refs: vec![], strings: vec![] }).unwrap();
        }
        handle
    }

    /// The fixture donor: the TESTFIG1 record itself (the capture's
    /// app-written straight-2D record), so the drawing carries a
    /// survey-database context the writer can join.
    fn write_donor(doc: &mut CadDocument, face: Handle, display: Handle, style: Handle) -> Handle {
        let handle = doc.allocate_handle();
        let vs = vertices_of(&TESTFIG1_CACHE_BITS, 5);
        let (min_x, min_y, max_x, max_y) = vs.iter().fold(
            (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY),
            |(a, b, c, d), p| (a.min(p.0), b.min(p.1), c.max(p.0), d.max(p.1)),
        );
        let (lo_x, lo_y) = (quantize(min_x), quantize(min_y));
        let ring = vec![
            Vector3::new(lo_x, lo_y, 0.0),
            Vector3::new(quantize(max_x), lo_y, 0.0),
            Vector3::new(quantize(max_x), quantize(max_y), 0.0),
            Vector3::new(lo_x, quantize(max_y), 0.0),
            Vector3::new(lo_x, lo_y, 0.0),
        ];
        let graphic = graphic_image(
            GRAPHIC_KIND,
            GRAPHIC_WORD,
            2,
            Vector3::new(lo_x + GRAPHIC_INSET, lo_y + GRAPHIC_INSET, 0.0),
            &[ring],
        );
        let common = EntityCommon {
            handle,
            owner_handle: doc.header.model_space_block_handle,
            layer: "0".into(),
            layer_handle: doc.layers.get("0").map(|l| l.handle),
            graphic_data: Some(graphic),
            ..EntityCommon::default()
        };
        put_entity(
            doc,
            "AECC_SVFIGURE",
            common,
            |w| bits(w, TESTFIG1_STREAM),
            Body {
                refs: vec![
                    soft_pointer(0),
                    hard_pointer(display.value()),
                    hard_pointer(display.value()),
                    soft_pointer(0),
                    hard_pointer(style.value()),
                    soft_pointer(0),
                    hard_pointer(face.value()),
                ],
                strings: vec![
                    IMPS.to_string(),
                    String::new(),
                    String::new(),
                    "TESTFIG1".to_string(),
                    "Survey Site".to_string(),
                ],
            },
        )
        .unwrap()
    }

    fn setup() -> (CadDocument, Handle, Handle, Handle) {
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let face = stub_ref(&mut doc, "AECC_PARCEL");
        let display = stub_ref(&mut doc, "AECC_FEATURE_STYLE");
        let style = stub_ref(&mut doc, "AECC_SVFIGURE_STYLE");
        write_donor(&mut doc, face, display, style);
        (doc, face, display, style)
    }

    #[test]
    fn refuses_a_drawing_without_a_context_donor() {
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let face = stub_ref(&mut doc, "AECC_PARCEL");
        let display = stub_ref(&mut doc, "AECC_FEATURE_STYLE");
        let style = stub_ref(&mut doc, "AECC_SVFIGURE_STYLE");
        let err = add_survey_figure(&mut doc, face, display, style, "F1", "Site", &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)], None).unwrap_err();
        assert!(err.to_string().contains("no straight-2D survey figure"), "{err}");
    }

    #[test]
    fn rejects_bad_inputs() {
        let (mut doc, face, display, style) = setup();
        let v = &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)];
        assert!(add_survey_figure(&mut doc, face, display, style, "", "S", v, None).is_err());
        assert!(add_survey_figure(&mut doc, face, display, style, "F", "", v, None).is_err());
        assert!(add_survey_figure(&mut doc, face, display, style, "F", "S", &[(0.0, 0.0), (10.0, 0.0)], None).is_err());
        assert!(add_survey_figure(&mut doc, face, display, style, "F", "S", &[(0.0, 0.0), (f64::NAN, 0.0), (1.0, 1.0)], None).is_err());
        assert!(add_survey_figure(&mut doc, Handle::new(0xdead), display, style, "F", "S", v, None).is_err());
        assert!(add_survey_figure(&mut doc, face, face, style, "F", "S", v, None).is_err());
        assert!(add_survey_figure(&mut doc, face, display, face, "F", "S", v, None).is_err());
        assert!(add_survey_figure(&mut doc, face, display, style, "F", "S", v, Some("nope")).is_err());
    }

    #[test]
    fn writes_the_pin_form_and_reads_it_back() {
        let (mut doc, face, display, style) = setup();
        let vs = vertices_of(&TESTFIG1_CACHE_BITS, 4);
        let h = add_survey_figure(&mut doc, face, display, style, "NEWFIG", "Survey Site", &vs, None).unwrap();
        // the record reads back through the shipped reader, in memory and
        // after a save/reopen
        let fig = |doc: &CadDocument| {
            let EntityType::Extended(x) = doc.get_entity(h).unwrap() else { panic!("entity") };
            let ExtendedEntityData::Arch(a) = &x.data else { panic!("arch") };
            (a.survey_figure().unwrap(), a.survey_figure_geometry().unwrap())
        };
        let (f, g) = fig(&doc);
        assert_eq!((f.name.as_str(), f.site.as_str()), ("NEWFIG", "Survey Site"));
        let expect: Vec<(f64, f64)> = TESTFIG1_CACHE_BITS.iter().take(5)
            .map(|p| (f64::from_bits(p[0]), f64::from_bits(p[1]))).collect();
        assert_eq!(g.vertices.iter().map(|v| (v.x, v.y)).collect::<Vec<_>>(), expect);
        assert_eq!((g.min_elevation, g.max_elevation), (0.0, 0.0));
        let path = std::env::temp_dir().join(format!("civil-svfig-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let (f2, g2) = fig(&back);
        assert_eq!((f2.name.as_str(), f2.site.as_str()), ("NEWFIG", "Survey Site"));
        assert_eq!(g2, g);
        // the handle stream keeps the record's references
        let EntityType::Extended(x) = back.get_entity(h).unwrap() else { panic!("entity") };
        let ExtendedEntityData::Arch(a) = &x.data else { panic!("arch") };
        let refs: Vec<u64> = a.typed_handles().iter().map(|r| r.0).collect();
        assert_eq!(refs, vec![0, display.value(), display.value(), 0, style.value(), 0, face.value()]);
        assert_eq!(a.decode().strings, vec![IMPS.to_string(), String::new(), String::new(), "NEWFIG".to_string(), "Survey Site".to_string()]);
    }

    #[test]
    fn the_longname_pin_reproduces_behind_the_carried_context() {
        // Writing LONG-NAME's traverse with the fixture donor (TESTFIG1)
        // reproduces the pin's class stream everywhere except the
        // app-owned per-figure block (head bits [355,436)), which the
        // record keeps nowhere else and the writer carries from the
        // drawing's own figure. The shared context head, the straight
        // tail, the count word, the entries and the trailer are exact.
        let (mut doc, face, display, style) = setup();
        let donor_context = TESTFIG1_STREAM[..CONTEXT_BITS].to_string();
        let vs = vertices_of(&LONGNAME_CACHE_BITS, 6);
        let h = add_survey_figure(&mut doc, face, display, style, "LONG-NAME-CAPTURE-ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789", "Survey Site", &vs, None).unwrap();
        let written = stream_of(&doc, h);
        assert_eq!(written.len(), LONGNAME_STREAM.len());
        assert_eq!(&written[..355], &LONGNAME_STREAM[..355], "the shared context head is the pin's");
        assert_eq!(&written[436..], &LONGNAME_STREAM[436..], "tail, count word, entries, trailer");
        assert_eq!(&written[355..436], &donor_context[355..436], "the carried per-figure block is the donor's");
        assert_ne!(&written[355..436], &LONGNAME_STREAM[355..436]);
        // the exact-bit accounting of the pin: every bit exact but the
        // carried block's bits where the donor's block differs from the
        // pin's (the two app-written blocks agree in most positions)
        let carried_diffs = donor_context[355..436].bytes()
            .zip(LONGNAME_STREAM[355..436].bytes())
            .filter(|(a, b)| a != b).count();
        let exact = written.bytes().zip(LONGNAME_STREAM.bytes()).filter(|(a, b)| a == b).count();
        assert_eq!(exact, written.len() - carried_diffs, "every bit exact but the carried block's differing ones");
        assert!(carried_diffs > 0, "the pins' per-figure blocks do differ");
    }

    #[test]
    fn the_testfig1_pin_reproduces_from_the_longname_fixture() {
        // The other direction: the donor carries LONG-NAME's context, the
        // writer writes TESTFIG1's traverse — again exact but the carried
        // block.
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let face = stub_ref(&mut doc, "AECC_PARCEL");
        let display = stub_ref(&mut doc, "AECC_FEATURE_STYLE");
        let style = stub_ref(&mut doc, "AECC_SVFIGURE_STYLE");
        // the donor is the LONG-NAME record itself
        let handle = doc.allocate_handle();
        let vs = vertices_of(&LONGNAME_CACHE_BITS, 7);
        let (min_x, min_y, max_x, max_y) = vs.iter().fold(
            (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY),
            |(a, b, c, d), p| (a.min(p.0), b.min(p.1), c.max(p.0), d.max(p.1)),
        );
        let (lo_x, lo_y) = (quantize(min_x), quantize(min_y));
        let ring = vec![
            Vector3::new(lo_x, lo_y, 0.0),
            Vector3::new(quantize(max_x), lo_y, 0.0),
            Vector3::new(quantize(max_x), quantize(max_y), 0.0),
            Vector3::new(lo_x, quantize(max_y), 0.0),
            Vector3::new(lo_x, lo_y, 0.0),
        ];
        let graphic = graphic_image(GRAPHIC_KIND, GRAPHIC_WORD, 2,
            Vector3::new(lo_x + GRAPHIC_INSET, lo_y + GRAPHIC_INSET, 0.0), &[ring]);
        let common = EntityCommon {
            handle,
            owner_handle: doc.header.model_space_block_handle,
            layer: "0".into(),
            layer_handle: doc.layers.get("0").map(|l| l.handle),
            graphic_data: Some(graphic),
            ..EntityCommon::default()
        };
        put_entity(&mut doc, "AECC_SVFIGURE", common,
            |w| bits(w, LONGNAME_STREAM),
            Body {
                refs: vec![
                    soft_pointer(0),
                    hard_pointer(display.value()),
                    hard_pointer(display.value()),
                    soft_pointer(0),
                    hard_pointer(style.value()),
                    soft_pointer(0),
                    hard_pointer(face.value()),
                ],
                strings: vec![
                    IMPS.to_string(),
                    String::new(),
                    String::new(),
                    "LONG-NAME-CAPTURE-ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".to_string(),
                    "Survey Site".to_string(),
                ],
            },
        ).unwrap();
        let vs = vertices_of(&TESTFIG1_CACHE_BITS, 4);
        let h = add_survey_figure(&mut doc, face, display, style, "TESTFIG1", "Survey Site", &vs, None).unwrap();
        let written = stream_of(&doc, h);
        assert_eq!(&written[..355], &TESTFIG1_STREAM[..355]);
        assert_eq!(&written[436..], &TESTFIG1_STREAM[436..]);
        assert_eq!(&written[355..436], &LONGNAME_STREAM[355..436]);
    }

    #[test]
    fn the_curved_pins_class_stream_is_the_same_2d_form() {
        // CURVED-CAPTURE-1's record is the same straight 2D form at the
        // class stream (its arc lives app-side only): the frame closes for
        // the 134-bit stride, its tags are `1000` and its context head is
        // the TESTFIG1 donor's (one survey database) — so the writer's
        // record form covers the capture's 2D records; the figure a writer
        // emits over CURVED's traverse is straight by construction.
        let frame = CURVED_STREAM.len() - 530;
        assert_eq!(frame % 134, 0);
        let count = frame / 134;
        assert_eq!(count, 4);
        for i in 0..count {
            let at = 513 + i * 134 + 130;
            assert_eq!(&CURVED_STREAM[at..at + 4], "1000", "entry {i} tag");
        }
        assert_eq!(CURVED_STREAM[..355], TESTFIG1_STREAM[..355]);
        // the traverse the shipped reader decodes from it: the entry
        // doubles decode from the stream's bits (RD = 8 bytes, LE, MSB bit
        // first) and match the capture's cache bit for bit
        let rd = |at: usize| {
            let mut bytes = [0u8; 8];
            for (k, byte) in bytes.iter_mut().enumerate() {
                *byte = u8::from_str_radix(&CURVED_STREAM[at + k * 8..at + k * 8 + 8], 2).unwrap();
            }
            f64::from_le_bytes(bytes)
        };
        for (i, p) in CURVED_CACHE_BITS.iter().enumerate() {
            let at = 513 + i * 134;
            assert_eq!((rd(at).to_bits(), rd(at + 66).to_bits()), (p[0], p[1]), "curved cache entry {i}");
        }
    }

    #[test]
    fn the_graphic_carries_the_schema_word_and_the_bbox_rectangle() {
        let (mut doc, face, display, style) = setup();
        let vs = vertices_of(&TESTFIG1_CACHE_BITS, 4);
        let h = add_survey_figure(&mut doc, face, display, style, "G1", "Survey Site", &vs, None).unwrap();
        let EntityType::Extended(x) = doc.get_entity(h).unwrap() else { panic!("entity") };
        let g = x.common.graphic_data.clone().unwrap();
        assert_eq!(g.len(), 316);
        let word: String = (0..30).map(|i| {
            let u = u16::from_le_bytes([g[112 + i * 2], g[113 + i * 2]]);
            char::from_u32(u as u32).unwrap()
        }).collect();
        assert_eq!(word, "AeccDbSvFigure (AeccSurvey130)");
        // the ring: the quantized bbox rectangle at z 0, closed
        let rd = |off: usize| f64::from_le_bytes(g[off..off + 8].try_into().unwrap());
        let (lo_x, lo_y) = (quantize(f64::from_bits(0x40b343abe55ee4e1)), quantize(f64::from_bits(0x40b4dfacea904730)));
        let (hi_x, hi_y) = (quantize(f64::from_bits(0x40b3b03f94e1dff3)), quantize(f64::from_bits(0x40b56d183bdddc06)));
        let expect = [
            (lo_x, lo_y), (hi_x, lo_y), (hi_x, hi_y), (lo_x, hi_y), (lo_x, lo_y),
        ];
        for (i, (ex, ey)) in expect.iter().enumerate() {
            let (x, y, z) = (rd(188 + i * 24), rd(196 + i * 24), rd(204 + i * 24));
            assert_eq!((x, y, z), (*ex, *ey, 0.0), "ring[{i}]");
        }
        // the origin sits GRAPHIC_INSET past the low corner
        assert_eq!((rd(16), rd(24), rd(32)), (lo_x + GRAPHIC_INSET, lo_y + GRAPHIC_INSET, 0.0));
    }

    #[test]
    fn the_quantizer_tracks_the_pins_corners() {
        // The app's own corners are recomputed from its database; the
        // record's cache doubles determine them only within the shared
        // solved-point tolerance. Over the pins' observed corners the
        // nearest-256-ulp rounding of the cache value lands within
        // 0x100 ulps of every corner: on CURVED all four are exact; on
        // TESTFIG1 two are exact (its other two sit 0x100 and 0xD0 ulps
        // away — the application recomputes them from its database, which
        // the record does not carry).
        let testfig1: [(u64, u64); 4] = [
            (0x40b343abe55ee4e1, 0x40b343abe55ee400),
            (0x40b3b03f94e1dff3, 0x40b3b03f94e1e000),
            (0x40b4dfacea904730, 0x40b4dfacea904800),
            (0x40b56d183bdddc00, 0x40b56d183bdddc00),
        ];
        let mut exact = 0;
        for (cache, app) in testfig1 {
            let q = quantize(f64::from_bits(cache)).to_bits();
            assert!(q.abs_diff(app) <= 0x100, "{q:#x} vs {app:#x}");
            exact += (q == app) as usize;
        }
        assert_eq!(exact, 2);
        // CURVED's four corners are all exactly determined
        let curved: [(u64, u64); 3] = [
            // CURVED's graphic corners over its own PIs (its fourth, the
            // max-y corner, is the application's arc extent — no quantized
            // PI determines it)
            (0x40b41aceaa0c7031, 0x40b41aceaa0c7000),
            (0x40b4a354906d127b, 0x40b4a354906d1200),
            (0x40b6fabcae7eb1b4, 0x40b6fabcae7eb200),
        ];
        for (cache, app) in curved {
            assert_eq!(quantize(f64::from_bits(cache)).to_bits(), app);
        }
    }
}
