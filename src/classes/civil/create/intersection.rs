//! Intersections of two crossing alignments.

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
/// The arm word of each road (main road, other road).
const ROAD_ARMS: [i32; 2] = [2, 4];
/// The arm word of each region (NE, SE, SW, NW).
const REGION_ARMS: [i32; 4] = [2, 4, 3, 1];
const REGION_NAMES: [&str; 4] = ["NE - Quadrant", "SE - Quadrant", "SW - Quadrant", "NW - Quadrant"];
/// The settings the record ends with: secondary road grade rules (applied,
/// maximum grade 0.05, maximum change 0.04, distance rule 1, distance 50),
/// then the version word.
const TAIL: &str = "10010011010100110011001100110011001100110011001100110101001001111110001111011000101001010111001000111111000010111101010100100001111110100000001000000000000000000000000000000000000000000000000000100100101000000101000100001011111000000100";

/// One road of an intersection: its alignment and profile, the station of
/// the crossing and the direction of the alignment there.
struct Road {
    alignment: u64,
    profile: u64,
    station: f64,
    direction: Vector2,
}

/// A region between two arms: the arm it starts at (in) and the one it
/// ends at (out), counterclockwise, each an alignment and whether the arm
/// runs along rising stations.
struct Region {
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

/// The four regions of a crossing, NE, SE, SW and NW: between the main
/// road's forward arm and the other road's arm to its left, then on
/// counterclockwise. A region's angle is the difference of the headings of
/// its arms (each heading in [0, 2π)).
fn regions(main: Vector2, other: Vector2) -> [Region; 4] {
    let left = main.x * other.y - main.y * other.x > 0.0;
    let arm = |road: usize, forward: bool| {
        let d = if road == 0 { main } else { other };
        (road, forward, heading(if forward { d } else { Vector2::new(-d.x, -d.y) }))
    };
    let north = arm(1, left);
    let south = arm(1, !left);
    let (east, west) = (arm(0, true), arm(0, false));
    [(east, north), (south, east), (west, south), (north, west)].map(|(a, b)| Region {
        angle: (b.2 - a.2).rem_euclid(std::f64::consts::TAU),
        in_road: a.0,
        in_forward: a.1,
        out_road: b.0,
        out_forward: b.1,
    })
}

/// The intersection record (`AECC_INTERSECTION`, an entity in model
/// space):
///
/// ```text
/// BL 80, 0, 80, 80, 0, BL 0x10025F04, BL 0x4000836B, 2 bits 00, 16-byte
///   id, 2 bits 00, BL 1, BL 1, BL 2 roads, BL 1
/// per road: BD station, BL 1, 1, 1, 1, BL arm (2, 4)
/// per region: BL index (1-4), BL arm (2, 4, 3, 1), BD angle, B in arm
///   forward, B out arm forward
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
    let regions = regions(roads[0].direction, roads[1].direction);
    let mut refs = vec![soft_pointer(0), hard_pointer(style)];
    for r in roads {
        refs.extend([hard_pointer(r.alignment), soft_pointer(r.profile), soft_pointer(0), soft_pointer(0), soft_pointer(0), soft_pointer(0)]);
    }
    refs.push(soft_pointer(0));
    for g in &regions {
        refs.extend([soft_pointer(0), soft_pointer(roads[g.in_road].alignment), soft_pointer(roads[g.out_road].alignment), soft_pointer(0)]);
    }
    let mut strings = vec!["AeccImpIntersection".to_string(), String::new(), name.to_string(), String::new()];
    strings.extend(REGION_NAMES.iter().map(|s| s.to_string()));
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
            for v in [1, 1, 2, 1] {
                w.write_bit_long(v);
            }
            for (r, arm) in roads.iter().zip(ROAD_ARMS) {
                w.write_bit_double(r.station);
                for v in [1, 1, 1, 1, arm] {
                    w.write_bit_long(v);
                }
            }
            for (i, (g, arm)) in regions.iter().zip(REGION_ARMS).enumerate() {
                w.write_bit_long(i as i32 + 1);
                w.write_bit_long(arm);
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

/// Where two line pieces cross inside both, as (point, parameter on each).
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
    (inside(t) && inside(u)).then(|| (Vector2::new(a.0.x + t * r.x, a.0.y + t * r.y), t, u))
}

/// Adds the intersection `name` of the alignments `main` (the main road)
/// and `other`, where they cross, with their profiles `main_profile` and
/// `other_profile`, on `layer` (layer "0" when `None`), with the drawing's
/// default intersection style. The alignments must cross once, on line
/// elements and away from their ends (crossings, not tee junctions);
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
        let p = arch(doc, profile).filter(|p| p.dxf_name == "AECC_PROFILE").ok_or_else(|| invalid("a road profile is not a profile"))?;
        if !a.typed_handles().iter().any(|r| r.0 == profile.value()) || p.typed_handles().iter().all(|r| r.0 != al.value()) {
            return Err(invalid("a profile does not belong to its road"));
        }
    }
    let (la, lb) = (lines(doc, main).ok_or_else(|| invalid("the main road cannot be solved"))?, lines(doc, other).ok_or_else(|| invalid("the other road cannot be solved"))?);
    let mut found = Vec::new();
    for a in &la {
        for b in &lb {
            if let Some((p, t, u)) = crossing((a.1, a.2), (b.1, b.2)) {
                found.push((p, *a, *b, t, u));
            }
        }
    }
    let [(point, a, b, _, _)] = found[..] else {
        return Err(invalid("the roads must cross once, on line elements"));
    };
    if a.0.is_nan() || b.0.is_nan() {
        return Err(invalid("a spiral comes before the crossing"));
    }
    let road = |al: Handle, profile: Handle, (station, p, q): (f64, Vector2, Vector2)| Road {
        alignment: al.value(),
        profile: profile.value(),
        station: station + p.distance(&point),
        direction: Vector2::new(q.x - p.x, q.y - p.y),
    };
    let roads = [road(main, main_profile, a), road(other, other_profile, b)];
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
    }
}
