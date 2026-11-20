//! Intersections of two alignments: crossings and tee junctions.

use super::default_styles::{ensure_intersection_styles, KEY_INTERSECTION_STYLES};
use super::sample_lines::arch;
use super::site::default_style_of;
use super::support::*;
use crate::classes::civil::AlignmentPiece;
use crate::entities::EntityCommon;
use crate::error::DxfError;
use crate::io::dwg::dwg_stream_writers::DwgMergedWriter;
use crate::types::{Handle, Vector2};
use crate::CadDocument;

/// The class word after the class head (0x4000836B or 0x4000836C in the
/// samples, by session).
const WORD: i32 = 0x4000_836B;
/// The id after the class word (a byte 0x80, then fifteen zero bytes).
const ID: [u8; 16] = [0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
/// The arm word of each road (main road, other road): a crossing's and a
/// tee junction's.
const ROAD_ARMS: [i32; 2] = [2, 4];
const TEE_ROAD_ARMS: [i32; 2] = [2, 2];
/// The word of each region (NE, SE, SW, NW).
const REGION_ARMS: [i32; 4] = [2, 4, 3, 1];
const REGION_NAMES: [&str; 4] = ["NE - Quadrant", "SE - Quadrant", "SW - Quadrant", "NW - Quadrant"];
/// The settings the record ends with: secondary road grade rules (applied,
/// maximum grade 0.05, maximum change 0.04, distance rule 1, distance 50),
/// then the version word.
const TAIL: &str = "10010011010100110011001100110011001100110011001100110101001001111110001111011000101001010111001000111111000010111101010100100001111110100000001000000000000000000000000000000000000000000000000000100100101000000101000100001011111000000100";

/// One road of an intersection: its alignment and profile, the station of
/// the crossing and the direction of the alignment there; a tee
/// junction's second road `ends` there (its arm runs back along it) or
/// starts there.
struct Road {
    alignment: u64,
    profile: u64,
    station: f64,
    direction: Vector2,
    ends: Option<bool>,
}

/// A region between two arms: the arm it starts at (in) and the one it
/// ends at (out), counterclockwise, each an alignment and whether the arm
/// runs along rising stations; `quadrant` indexes `REGION_NAMES`.
struct Region {
    quadrant: usize,
    angle: f64,
    in_road: usize,
    in_forward: bool,
    out_road: usize,
    out_forward: bool,
}

fn heading(v: Vector2) -> f64 {
    let a = v.y.atan2(v.x);
    if a < 0.0 {
        a + std::f64::consts::TAU
    } else {
        a
    }
}

/// The regions of an intersection: between consecutive road arms going
/// counterclockwise (a crossing's four; a tee junction's two beside the
/// second road's single arm). Each is named by the compass quadrant its
/// bisector points into (NE between east and north, NW, SW, SE), the
/// regions listed in NE, SE, SW, NW order; its angle is the difference of
/// the headings of its arms (each heading in [0, 2π)). WI01, WI11, WI21,
/// WI48, WI51 (crossings, the first road heading east or north-east),
/// WI42, WI44 to WI47 (tee junctions on either side, the second road
/// starting or ending there or the first road ending there).
fn regions(roads: &[Road; 2]) -> Vec<Region> {
    let arm = |road: usize, forward: bool| {
        let d = roads[road].direction;
        (road, forward, heading(if forward { d } else { Vector2::new(-d.x, -d.y) }))
    };
    let mut arms = vec![arm(0, true), arm(0, false)];
    match roads[1].ends {
        Some(ends) => arms.push(arm(1, !ends)),
        None => arms.extend([arm(1, true), arm(1, false)]),
    }
    arms.sort_by(|a, b| a.2.total_cmp(&b.2));
    let mut out: Vec<Region> = (0..arms.len())
        .map(|i| (arms[i], arms[(i + 1) % arms.len()]))
        .filter(|(a, b)| roads[1].ends.is_none() || a.0 == 1 || b.0 == 1)
        .map(|(a, b)| {
            let angle = (b.2 - a.2).rem_euclid(std::f64::consts::TAU);
            let bisector = (a.2 + angle / 2.0).rem_euclid(std::f64::consts::TAU);
            // East to north NE, then NW, SW, SE (indexes of REGION_NAMES).
            let quadrant = [0, 3, 2, 1][((bisector / std::f64::consts::FRAC_PI_2) as usize).min(3)];
            Region { quadrant, angle, in_road: a.0, in_forward: a.1, out_road: b.0, out_forward: b.1 }
        })
        .collect();
    out.sort_by_key(|r| r.quadrant);
    out
}

/// The intersection record (`AECC_INTERSECTION`, an entity in model
/// space):
///
/// ```text
/// BL 80, 0, 80, 80, 0, BL 0x10025F04, BL 0x4000836B, 2 bits 00, 16-byte
///   id, 2 bits 00, BL kind (1 crossing, 2 tee junction), BL 1, BL 2
///   roads, BL 1
/// per road: BD station, BL 1, 1, 1, 1, BL arm (crossing 2, 4; tee 2, 2)
/// per region (crossing four, tee two): BL index (1 up), BL quadrant
///   (NE 2, SE 4, SW 3, NW 1), BD angle, B in arm forward, B out arm
///   forward
/// TAIL
/// references: soft 0, style (hard), per road {alignment (hard), profile,
///   left offset alignment and profile, right offset alignment and
///   profile (soft; offsets written null)}, soft 0, per region {soft 0,
///   in alignment, out alignment, soft 0}
/// texts: "AeccImpIntersection", "", name, "", the region names, the two
///   assembly set files
/// ```
///
/// Written with the samples' roads the record equals theirs bit for bit
/// apart from the class word.
fn write_intersection(doc: &mut CadDocument, common: EntityCommon, name: &str, style: u64, roads: &[Road; 2], assembly_sets: [&str; 2]) -> Result<Handle, DxfError> {
    let tee = roads[1].ends;
    let regions = regions(roads);
    let mut refs = vec![soft_pointer(0), hard_pointer(style)];
    for r in roads {
        refs.extend([hard_pointer(r.alignment), soft_pointer(r.profile), soft_pointer(0), soft_pointer(0), soft_pointer(0), soft_pointer(0)]);
    }
    refs.push(soft_pointer(0));
    for g in &regions {
        refs.extend([soft_pointer(0), soft_pointer(roads[g.in_road].alignment), soft_pointer(roads[g.out_road].alignment), soft_pointer(0)]);
    }
    let mut strings = vec!["AeccImpIntersection".to_string(), String::new(), name.to_string(), String::new()];
    strings.extend(regions.iter().map(|g| REGION_NAMES[g.quadrant].to_string()));
    strings.extend(assembly_sets.iter().map(|s| s.to_string()));
    put_entity(
        doc,
        "AECC_INTERSECTION",
        common,
        |w: &mut DwgMergedWriter| {
            for v in [80, 0, 80, 80, 0, 0x1002_5F04, WORD] {
                w.write_bit_long(v);
            }
            bits(w, "00");
            for b in ID {
                w.write_byte(b);
            }
            bits(w, "00");
            for v in [if tee.is_some() { 2 } else { 1 }, 1, 2, 1] {
                w.write_bit_long(v);
            }
            for (r, arm) in roads.iter().zip(if tee.is_some() { TEE_ROAD_ARMS } else { ROAD_ARMS }) {
                w.write_bit_double(r.station);
                for v in [1, 1, 1, 1, arm] {
                    w.write_bit_long(v);
                }
            }
            for (i, g) in regions.iter().enumerate() {
                w.write_bit_long(i as i32 + 1);
                w.write_bit_long(REGION_ARMS[g.quadrant]);
                w.write_bit_double(g.angle);
                w.write_bit(g.in_forward);
                w.write_bit(g.out_forward);
            }
            bits(w, TAIL);
        },
        Body { refs, strings },
    )
}

/// The line pieces of an alignment with the station each starts at (NaN
/// after a spiral: its length is not solved exactly).
fn lines(doc: &CadDocument, alignment: Handle) -> Option<Vec<(f64, Vector2, Vector2)>> {
    let a = arch(doc, alignment).filter(|a| a.dxf_name == "AECC_ALIGNMENT")?;
    let mut station = a.alignment_start_station()?;
    let mut out = Vec::new();
    for (kind, curvature, points) in a.alignment_pieces_with_curvature() {
        let (p, q) = (*points.first()?, *points.last()?);
        let chord = p.distance(&q);
        station += match kind {
            AlignmentPiece::Line => {
                out.push((station, p, q));
                chord
            }
            // ponytail: the minor arc of the chord; arcs of more than a
            // half turn would need the stored sweep.
            AlignmentPiece::Curve if curvature != 0.0 => 2.0 * (chord * curvature.abs() / 2.0).min(1.0).asin() / curvature.abs(),
            _ => f64::NAN,
        };
    }
    Some(out)
}

/// Where two line pieces meet, as (point, parameter on each): inside the
/// first, and inside the second or at one of its ends.
fn crossing(a: (Vector2, Vector2), b: (Vector2, Vector2)) -> Option<(Vector2, f64, f64)> {
    let (r, s) = (Vector2::new(a.1.x - a.0.x, a.1.y - a.0.y), Vector2::new(b.1.x - b.0.x, b.1.y - b.0.y));
    let den = r.x * s.y - r.y * s.x;
    if den.abs() < 1e-12 * (r.x.hypot(r.y) * s.x.hypot(s.y)) {
        return None;
    }
    let (qx, qy) = (b.0.x - a.0.x, b.0.y - a.0.y);
    let t = (qx * s.y - qy * s.x) / den;
    let u = (qx * r.y - qy * r.x) / den;
    let inside = |v: f64| v > 1e-9 && v < 1.0 - 1e-9;
    (inside(t) && u > -1e-9 && u < 1.0 + 1e-9).then(|| (Vector2::new(a.0.x + t * r.x, a.0.y + t * r.y), t, u.clamp(0.0, 1.0)))
}

/// Adds the intersection `name` of the alignments `main` (the main road)
/// and `other`, where they cross, with their profiles `main_profile` and
/// `other_profile` (null for none: the application makes a tee junction
/// only without profiles), on `layer` (layer "0" when `None`), with the
/// drawing's default intersection style. The alignments must meet once,
/// on line elements: crossing away from their ends, or one ending (or
/// starting) on the other (a tee junction; the through road is made the
/// main road, as the application does);
/// `assembly_sets` are the two assembly set files the record names (the
/// file and its form under the content folder, both supplied by the
/// caller). Only the intersection is written: offset alignments, curb
/// returns and corridor regions are not (the record's references to them
/// are null).
#[allow(clippy::too_many_arguments)]
pub fn add_intersection(doc: &mut CadDocument, name: &str, main: Handle, main_profile: Handle, other: Handle, other_profile: Handle, assembly_sets: [&str; 2], layer: Option<&str>) -> Result<Handle, DxfError> {
    if name.is_empty() {
        return Err(invalid("an intersection needs a name"));
    }
    if main == other {
        return Err(invalid("an intersection needs two alignments"));
    }
    for (al, profile) in [(main, main_profile), (other, other_profile)] {
        let a = arch(doc, al).filter(|a| a.dxf_name == "AECC_ALIGNMENT").ok_or_else(|| invalid("a road is not an alignment"))?;
        if profile.is_null() {
            continue;
        }
        let p = arch(doc, profile).filter(|p| p.dxf_name == "AECC_PROFILE").ok_or_else(|| invalid("a road profile is not a profile"))?;
        if !a.typed_handles().iter().any(|r| r.0 == profile.value()) || p.typed_handles().iter().all(|r| r.0 != al.value()) {
            return Err(invalid("a profile does not belong to its road"));
        }
    }
    let (la, lb) = (lines(doc, main).ok_or_else(|| invalid("the main road cannot be solved"))?, lines(doc, other).ok_or_else(|| invalid("the other road cannot be solved"))?);
    // Where a road ends on the other's line: the road's first piece's
    // start or last piece's end; the through road is the main road.
    let meets = |la: &[(f64, Vector2, Vector2)], lb: &[(f64, Vector2, Vector2)]| {
        let mut found = Vec::new();
        for a in la {
            for (j, b) in lb.iter().enumerate() {
                if let Some((p, _, u)) = crossing((a.1, a.2), (b.1, b.2)) {
                    let end = if u > 1e-9 && u < 1.0 - 1e-9 {
                        None
                    } else if u <= 1e-9 && j == 0 {
                        Some(false)
                    } else if u >= 1.0 - 1e-9 && j + 1 == lb.len() {
                        Some(true)
                    } else {
                        continue;
                    };
                    found.push((p, *a, *b, end));
                }
            }
        }
        found
    };
    let (mut main, mut main_profile, mut other, mut other_profile) = (main, main_profile, other, other_profile);
    let mut found = meets(&la, &lb);
    if found.is_empty() {
        found = meets(&lb, &la);
        (main, main_profile, other, other_profile) = (other, other_profile, main, main_profile);
    }
    let [(point, a, b, ends)] = found[..] else {
        return Err(invalid("the roads must meet once, on line elements"));
    };
    if ends.is_some() && !(main_profile.is_null() && other_profile.is_null()) {
        return Err(invalid("a tee junction is written without profiles"));
    }
    if a.0.is_nan() || b.0.is_nan() {
        return Err(invalid("a spiral comes before the crossing"));
    }
    let road = |al: Handle, profile: Handle, (station, p, q): (f64, Vector2, Vector2)| Road {
        alignment: al.value(),
        profile: profile.value(),
        station: station + p.distance(&point),
        direction: Vector2::new(q.x - p.x, q.y - p.y),
        ends: None,
    };
    let roads = [road(main, main_profile, a), Road { ends, ..road(other, other_profile, b) }];
    ensure_intersection_styles(doc)?;
    let style = default_style_of(doc, KEY_INTERSECTION_STYLES).ok_or_else(|| invalid("drawing has no intersection styles"))?;
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    let handle = doc.allocate_handle();
    let common = EntityCommon { handle, owner_handle: doc.header.model_space_block_handle, layer: layer.into(), layer_handle: Some(layer_handle), ..EntityCommon::default() };
    write_intersection(doc, common, name, style, &roads, assembly_sets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classes::civil::{add_alignment, add_alignment_with_curves, add_profile, VertexCurve};
    use crate::io::dwg::{DwgReader, DwgWriter};
    use crate::types::DxfVersion;

    #[test]
    fn crossings_get_their_stations_and_regions() {
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let a = add_alignment_with_curves(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0), v(200.0, 100.0)], &[VertexCurve::Arc(50.0)], None).unwrap();
        let b = add_alignment(&mut doc, "AL2", &[v(150.0, 100.0), v(250.0, 0.0)], None).unwrap();
        let c = add_alignment(&mut doc, "AL3", &[v(0.0, 10.0), v(220.0, 10.0)], None).unwrap();
        let d = add_alignment(&mut doc, "AL4", &[v(0.0, 150.0), v(300.0, 150.0)], None).unwrap();
        let pa = add_profile(&mut doc, a, "PA", &[v(0.0, 10.0), v(200.0, 12.0)], &[], None).unwrap();
        let pb = add_profile(&mut doc, b, "PB", &[v(0.0, 11.0), v(141.0, 11.0)], &[], None).unwrap();
        let pc = add_profile(&mut doc, c, "PC", &[v(0.0, 11.0), v(220.0, 11.0)], &[], None).unwrap();
        let pd = add_profile(&mut doc, d, "PD", &[v(0.0, 11.0), v(300.0, 11.0)], &[], None).unwrap();
        assert!(add_intersection(&mut doc, "", a, pa, b, pb, ["", ""], None).is_err());
        assert!(add_intersection(&mut doc, "X", a, pb, b, pa, ["", ""], None).is_err());
        // AL2 and AL4 do not meet; AL1 and AL3 cross on the curve.
        assert!(add_intersection(&mut doc, "X", b, pb, d, pd, ["", ""], None).is_err());
        assert!(add_intersection(&mut doc, "X", a, pa, c, pc, ["", ""], None).is_err());
        let i = add_intersection(&mut doc, "INT1", a, pa, b, pb, ["sets.xml", "<dir>/sets.xml"], None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-intersections-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let x = arch(&back, i).unwrap();
        let layout = x.intersection_layout().unwrap();
        // (175, 75): 100 along the first line, the arc, then the second line.
        let arc = 50.0 * std::f64::consts::FRAC_PI_4;
        let tangent = 50.0 * (std::f64::consts::FRAC_PI_8).tan();
        let along = 75.0_f64.hypot(75.0) - 100.0_f64.hypot(100.0) + (100.0_f64.hypot(100.0) - tangent);
        assert!((layout.road_stations[0] - (100.0 - tangent + arc + along)).abs() < 1e-9, "{:?}", layout);
        assert!((layout.road_stations[1] - 25.0_f64.hypot(25.0)).abs() < 1e-9);
        assert_eq!(layout.region_angles.len(), 4);
        assert!(layout.region_angles.iter().all(|a| (a - std::f64::consts::FRAC_PI_2).abs() < 1e-12));
        let refs: Vec<u64> = x.typed_handles().iter().map(|r| r.0).collect();
        // The drawing gets the default intersection style, which draws one
        // of the marker styles it gets with it.
        let style = default_style_of(&back, KEY_INTERSECTION_STYLES).unwrap();
        let marker = object(&back, Handle::new(style)).unwrap().decode().handles[0];
        assert_eq!(object(&back, Handle::new(marker)).unwrap().dxf_name, "AECC_ROADWAYMARKER_STYLE");
        assert_eq!(&refs[1..4], &[style, a.value(), pa.value()]);
        assert_eq!(x.decode().strings[2], "INT1");
        assert_eq!(x.decode().strings[9], "<dir>/sets.xml");
        assert!(!layout.tee);
    }

    #[test]
    fn tee_junctions_get_two_regions_named_by_quadrant() {
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let a = add_alignment(&mut doc, "Road A", &[v(0.0, 0.0), v(200.0, 0.0)], None).unwrap();
        // Road B starts on Road A and runs south.
        let b = add_alignment(&mut doc, "Road B", &[v(100.0, 0.0), v(100.0, -100.0)], None).unwrap();
        let pa = add_profile(&mut doc, a, "PA", &[v(0.0, 10.0), v(200.0, 12.0)], &[], None).unwrap();
        assert!(add_intersection(&mut doc, "T", a, pa, b, Handle::NULL, ["", ""], None).is_err());
        // Given as the second road, the through road is still made main.
        let i = add_intersection(&mut doc, "T", b, Handle::NULL, a, Handle::NULL, ["", ""], None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-tee-junctions-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let x = arch(&back, i).unwrap();
        let layout = x.intersection_layout().unwrap();
        assert!(layout.tee);
        assert_eq!(layout.road_stations, vec![100.0, 0.0]);
        assert_eq!(layout.region_angles.len(), 2);
        assert!(layout.region_angles.iter().all(|a| (a - std::f64::consts::FRAC_PI_2).abs() < 1e-12));
        assert_eq!(&x.decode().strings[4..6], &["SE - Quadrant", "SW - Quadrant"]);
        assert_eq!(x.typed_handles()[2].0, a.value());
    }
}
