//! Intersections of two alignments: crossings and tee junctions.

use super::assoc::{add_action, ActionBody, Dependency};
use super::default_styles::{ensure_intersection_label_style, ensure_intersection_styles, KEY_INTERSECTION_STYLES};
use super::profile::VerticalCurve;
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

/// The location label's record: constant (WT40, WT41, WI42, WI44, WI11).
const LABEL: (usize, &str) = (647, "542541508045f02104800000000000000000000000000000001505400000be043fffffc012030020018008054808f36a001ea2a0350aaaaaaaaaaab6a7e2aaaaaaaaaaae29f8000000000000b281417c08");
/// The profile locking action body's bits after its version word
/// (constant in every sample).
const LOCKING: &str = "00010000001101000000111010101001010100";
/// The orders of the locking action's and the intersection action's
/// dependencies on the intersection.
const LOCKING_ORDER: i32 = 0x1_0100;
const INTERSECTION_ORDER: i32 = 0x1_0000;
/// The design speed the application gives an intersection's offset
/// alignments.
const OFFSET_SPEED: f64 = 60.0;

/// The widths of the offset alignments an intersection gets on the left
/// and right of each road (the application's default is 12 for each).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IntersectionOffsets {
    pub main_left: f64,
    pub main_right: f64,
    pub other_left: f64,
    pub other_right: f64,
}

impl Default for IntersectionOffsets {
    fn default() -> Self {
        IntersectionOffsets { main_left: 12.0, main_right: 12.0, other_left: 12.0, other_right: 12.0 }
    }
}

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
    /// Its left and right offset alignments (0 for none).
    offsets: [u64; 2],
    /// Its curb returns by region (0 for none; filled in the first road).
    curb_returns: [u64; 4],
    /// Its left and right offset profiles (0 for none).
    offset_profiles: [u64; 2],
    /// The curb return profiles by region (0 for none; first road).
    curb_return_profiles: [u64; 4],
}

/// The curb return profiles the command makes with its curb return
/// profiles setting on (WC61), and what they need: the grade of the
/// roads' offset profiles from the roads' profiles and how far each curb
/// return runs along the incoming and the outgoing lane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurbReturnProfiles {
    /// Grade of the offset profiles away from their road (the
    /// application's default -0.02).
    pub slope: f64,
    /// Length of the curb return's line along the incoming lane (25).
    pub incoming: f64,
    /// Length of its line along the outgoing lane (25).
    pub outgoing: f64,
}

impl Default for CurbReturnProfiles {
    fn default() -> Self {
        CurbReturnProfiles { slope: -0.02, incoming: 25.0, outgoing: 25.0 }
    }
}

/// The styles the command gives what it makes when the drawing has them
/// by name (WC40, WC61: "Offsets" for the offset alignments, "Intersection
/// Basic" for the curb returns and the profiles); otherwise the drawing's
/// default ones, as the application uses in a drawing without them (WI01).
struct CommandStyles {
    offsets: Option<u64>,
    curb_returns: Option<u64>,
    profiles: Option<u64>,
}

impl CommandStyles {
    fn of(doc: &CadDocument) -> Self {
        use super::site::named_style;
        use super::tree::{KEY_ALIGNMENT_STYLES, KEY_PROFILE_STYLES};
        CommandStyles {
            offsets: named_style(doc, KEY_ALIGNMENT_STYLES, "Offsets"),
            curb_returns: named_style(doc, KEY_ALIGNMENT_STYLES, "Intersection Basic"),
            profiles: named_style(doc, KEY_PROFILE_STYLES, "Intersection Basic"),
        }
    }
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
///   profile (soft; offset profiles written null)}, soft 0, per region {soft 0,
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
        refs.extend([hard_pointer(r.alignment), soft_pointer(r.profile), soft_pointer(r.offsets[0]), soft_pointer(r.offset_profiles[0]), soft_pointer(r.offsets[1]), soft_pointer(r.offset_profiles[1])]);
    }
    // Per region: its curb return alignment and profile, in and out roads
    // (WC40), and a closing soft 0.
    for (i, g) in regions.iter().enumerate() {
        refs.extend([soft_pointer(roads[0].curb_returns[i]), soft_pointer(roads[0].curb_return_profiles[i]), soft_pointer(roads[g.in_road].alignment), soft_pointer(roads[g.out_road].alignment)]);
    }
    refs.push(soft_pointer(0));
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

/// The order of a curb return action's dependency on its intersection
/// (WC40).
const CURB_RETURN_ORDER: i32 = 0x2_0000;
/// The order of a curb return profile action's dependency on its
/// intersection (WC61).
const CURB_RETURN_PROFILE_ORDER: i32 = 0x3_0000;

/// A profile's elevation at `station` (the entity there).
fn elevation_at(doc: &CadDocument, profile: Handle, station: f64) -> Option<f64> {
    let a = arch(doc, profile)?;
    let (data, start, end) = a.main_stream()?;
    let points = super::super::profile::entity_points(data, start, end)?;
    let tol = 1e-9 * station.abs().max(1.0);
    points.iter().find(|(_, _, p, _)| station >= p[0].x - tol && station <= p[2].x + tol).map(|(.., curve)| curve.elevation_at(station))
}

/// The command's grade rule for the other road (the record's settings):
/// beside the locked stretch its grade changes by at most this much and
/// stays within this grade, up to the intersection's extents along it
/// (its distance rule; the record's distance 50 is not used by that rule:
/// WC85 radius 15 puts the PVI at 52, WC86 lines 10 at 47).
const GRADE_CHANGE: f64 = 0.04;
const GRADE_MAX: f64 = 0.05;

/// The grade the rule gives beside the locked stretch: `outside` limited to
/// [`GRADE_CHANGE`] from the stretch's `inside` grade, then to
/// [`GRADE_MAX`] (WC76: -0.06 becomes -0.05; WC77: -0.055 within the change
/// becomes -0.05). `None` when `outside` already keeps both, or when a
/// grade is not finite (a PVI sharing the stretch end's station).
fn ruled_grade(inside: f64, outside: f64) -> Option<f64> {
    if !inside.is_finite() || !outside.is_finite() {
        return None;
    }
    let mut g = outside;
    if (g - inside).abs() > GRADE_CHANGE + 1e-12 {
        g = inside + GRADE_CHANGE * (g - inside).signum();
    }
    if g.abs() > GRADE_MAX + 1e-12 {
        g = GRADE_MAX * g.signum();
    }
    ((g - outside).abs() > 1e-12).then_some(g)
}

/// A PVI of the profile being locked: its point, its curve (with the
/// stations the curve covers) and whether the locking adds it.
type LockPvi = (Vector2, Option<(VerticalCurve, f64, f64)>, bool);

/// The profile locking: the other road's profile keeps its PVIs outside
/// the locked stretch and takes new PVIs at the crossing (the main road's
/// elevation) and, with offset profiles (`slope`), where it crosses the
/// main road's offset lines (the main road's elevation there raised by
/// `slope` times the offset). A tee junction's other road is locked only
/// with offset profiles, at the junction and the one offset line it
/// crosses (WC67; WC66 and WC74 without them are not changed).
///
/// - A PVI at a new PVI's station is kept unless it has the same
///   elevation (WC68: the crossing's PVI then follows it, a tangent of no
///   length between them); curves that reach the locked stretch are
///   dropped, the others kept (WC68, WC69).
/// - With offset profiles the grade rule holds on each side: where the
///   grade beyond the stretch breaks [`ruled_grade`], a PVI at the
///   intersection's `extents` on that side (the curb returns' farthest
///   lines on the road) takes the ruled grade and the PVIs before it are
///   dropped (WC67, WC70, WC72, WC73, WC76 to WC78, WC81 to WC86; WC61,
///   WC69, WC71 within it). Without a PVI beyond it the new PVI ends the
///   profile, past its old end (WC79, WC80).
///
/// A profile whose curves cannot be read is left as it is.
fn lock_other_profile(doc: &mut CadDocument, roads: &[Road; 2], widths: &[[f64; 2]; 2], ruled: Option<(f64, (f64, f64))>) -> Result<(), DxfError> {
    let slope = ruled.map(|r| r.0);
    use crate::classes::civil::ProfilePiece;
    let (m, x) = (&roads[0], &roads[1]);
    if x.ends.is_some() && slope.is_none() {
        return Ok(());
    }
    let other = Handle::new(x.profile);
    let Some(a) = arch(doc, other) else { return Ok(()) };
    let pvis = a.profile_pvis();
    let Some(pieces) = a.main_stream().and_then(|(data, start, end)| super::super::profile::entity_points(data, start, end)) else { return Ok(()) };
    if pvis.len() < 2 {
        return Ok(());
    }
    // Each PVI's curve and the stations it covers.
    let mut curves: Vec<Option<(VerticalCurve, f64, f64)>> = Vec::with_capacity(pvis.len());
    for p in &pvis {
        if p.curve_length == 0.0 {
            curves.push(None);
            continue;
        }
        let tol = 1e-9 * p.point.x.abs().max(1.0);
        let Some((_, piece, ends, curve)) = pieces.iter().find(|(_, k, e, _)| *k != ProfilePiece::Tangent && e[0].x <= p.point.x + tol && e[2].x >= p.point.x - tol) else { return Ok(()) };
        let (lo, hi) = (ends[0].x, ends[2].x);
        let c = match piece {
            ProfilePiece::Parabola => VerticalCurve::Parabola(hi - lo),
            ProfilePiece::AsymmetricParabola => {
                let pvi = curve.asymmetric_pvi().unwrap_or(p.point.x);
                VerticalCurve::Asymmetric { before: pvi - lo, after: hi - pvi }
            }
            ProfilePiece::Circular => VerticalCurve::CircularLength(hi - lo),
            ProfilePiece::Tangent => VerticalCurve::None,
        };
        curves.push(Some((c, lo, hi)));
    }
    let (first, last) = (pvis[0].point.x, pvis[pvis.len() - 1].point.x);
    let unit = |v: Vector2| v * (1.0 / v.length());
    let (d, e) = (unit(m.direction), unit(x.direction));
    let main_z = |t: f64| elevation_at(doc, Handle::new(m.profile), m.station + t).ok_or_else(|| invalid("the main road's profile does not reach the crossing"));
    let mut new = vec![Vector2::new(x.station, main_z(0.0)?)];
    if let Some(slope) = slope {
        // Where the other road meets the main road's offset line `q` off
        // its centre line: `e u - d t = q`; a tee's road meets one.
        let left = Vector2::new(-d.y, d.x);
        for q in [left * widths[0][0], left * -widths[0][1]] {
            let den = e.cross(&d);
            let (u, t) = (q.cross(&d) / den, q.cross(&e) / den);
            let station = x.station + u;
            if station >= first - 1e-9 && station <= last + 1e-9 {
                new.push(Vector2::new(station, main_z(t)? + slope * q.length()));
            }
        }
    }
    new.sort_by(|a, b| a.x.total_cmp(&b.x));
    let (lo, hi) = (new[0].x, new[new.len() - 1].x);
    let tol = 1e-9 * hi.abs().max(1.0);
    let mut all: Vec<LockPvi> = pvis
        .iter()
        .zip(&curves)
        .filter(|(p, _)| p.point.x <= lo + tol || p.point.x >= hi - tol)
        .filter(|(p, _)| !new.iter().any(|n| (n.x - p.point.x).abs() <= tol && (n.y - p.point.y).abs() <= tol))
        .map(|(p, c)| (p.point, c.filter(|c| c.2 < lo - tol || c.1 > hi + tol), false))
        .chain(new.iter().map(|n| (*n, None, true)))
        .collect();
    let order = |all: &mut Vec<LockPvi>| all.sort_by(|a, b| a.0.x.total_cmp(&b.0.x).then(a.2.cmp(&b.2)));
    order(&mut all);
    if let Some((_, extents)) = ruled {
        let grade = |a: Vector2, b: Vector2| (b.y - a.y) / (b.x - a.x);
        // Beyond the stretch's last new PVI.
        if let Some(i) = all.iter().rposition(|p| p.2).filter(|i| *i > 0 && i + 1 < all.len()) {
            let (end, g_in, g_out) = (all[i].0, grade(all[i - 1].0, all[i].0), grade(all[i].0, all[i + 1].0));
            let (at, d) = (extents.1, extents.1 - end.x);
            if let Some(g) = ruled_grade(g_in, g_out).filter(|_| d > tol) {
                all.retain(|p| p.2 || p.0.x <= end.x + tol || p.0.x > at + tol);
                for p in all.iter_mut().filter(|p| p.1.is_some_and(|c| c.1 < at + tol)) {
                    p.1 = None;
                }
                all.push((Vector2::new(at, end.y + g * d), None, false));
                order(&mut all);
            }
        }
        // Before its first.
        if let Some(i) = all.iter().position(|p| p.2).filter(|i| *i > 0 && i + 1 < all.len()) {
            let (start, g_before, g_after) = (all[i].0, grade(all[i - 1].0, all[i].0), grade(all[i].0, all[i + 1].0));
            let (at, d) = (extents.0, start.x - extents.0);
            if let Some(g) = ruled_grade(g_after, g_before).filter(|_| d > tol) {
                all.retain(|p| p.2 || p.0.x >= start.x - tol || p.0.x < at - tol);
                for p in all.iter_mut().filter(|p| p.1.is_some_and(|c| c.2 > at - tol)) {
                    p.1 = None;
                }
                all.push((Vector2::new(at, start.y - g * d), None, false));
                order(&mut all);
            }
        }
    }
    let points: Vec<Vector2> = all.iter().map(|p| p.0).collect();
    let curves: Vec<VerticalCurve> = all[1..all.len() - 1].iter().map(|p| p.1.map_or(VerticalCurve::None, |c| c.0)).collect();
    super::profile::set_profile_pvis(doc, other, &points, &curves)
}

/// A curb return's place: the corner where its offset lines meet, the in
/// and out arms' directions, the offset alignments (road, side: 0 left, 1
/// right) it runs along, and the stations its lines take on them.
struct CurbReturnPlace {
    corner: Vector2,
    arms: [Vector2; 2],
    sides: [(usize, usize); 2],
    spans: [((usize, usize), f64, f64); 2],
    /// The arc's tangent length and its sweep.
    tangent: f64,
    sweep: f64,
    /// The arc's radius and the lines' lengths (shortened on a road that
    /// ends within them).
    radius: f64,
    lines: [f64; 2],
}

/// A road's station at a point near the crossing (its line there).
fn station_on(road: &Road, point: Vector2, p: Vector2) -> f64 {
    let d = road.direction * (1.0 / road.direction.length());
    road.station + (p - point).dot(&d)
}

/// The curb return of each region of a crossing or tee junction: it runs toward the
/// crossing along the incoming arm's offset line on the region's side
/// (the road's left offset when the arm runs along the road, else its
/// right) and away along the outgoing arm's (the road's right when that
/// arm runs along the road), a unit of line before and after the arc.
///
/// Where a road's alignment (its `ranges` of stations) ends before the
/// arc's end on it the radius is halved until the arc fits, then each line
/// is halved until it fits (WC82, WC83: radius 12.5 on a road 30 and 33
/// long, lines 3.125 and 6.25; WC81, WC84: lines 1.5625 and 12.5).
fn curb_returns(roads: &[Road; 2], point: Vector2, widths: &[[f64; 2]; 2], radius: f64, lines: [f64; 2], ranges: [(f64, f64); 2]) -> Result<Vec<CurbReturnPlace>, DxfError> {
    let mut out = Vec::new();
    for g in regions(roads) {
        let arm = |road: usize, forward: bool| {
            let d = roads[road].direction * (1.0 / roads[road].direction.length());
            if forward { d } else { d * -1.0 }
        };
        let (a_in, a_out) = (arm(g.in_road, g.in_forward), arm(g.out_road, g.out_forward));
        let side_in = (g.in_road, if g.in_forward { 0 } else { 1 });
        let side_out = (g.out_road, if g.out_forward { 1 } else { 0 });
        // The offset lines: the region lies left of the in arm and right
        // of the out arm.
        let left = |v: Vector2| Vector2::new(-v.y, v.x);
        let (p_in, p_out) = (point + left(a_in) * widths[side_in.0][side_in.1], point - left(a_out) * widths[side_out.0][side_out.1]);
        let den = a_in.cross(&a_out);
        if den.abs() < 1e-12 {
            return Err(invalid("the roads are parallel"));
        }
        let corner = p_in + a_in * ((p_out - p_in).cross(&a_out) / den);
        let sweep = (a_in * -1.0).dot(&a_out).clamp(-1.0, 1.0).acos();
        let arms = [(side_in, a_in), (side_out, a_out)];
        let fits = |(side, a): ((usize, usize), Vector2), along: f64| {
            let s = station_on(&roads[side.0], point, corner + a * along);
            let (lo, hi) = ranges[side.0];
            s >= lo - 1e-9 && s <= hi + 1e-9
        };
        let (mut radius, mut lines) = (radius, lines);
        for _ in 0..64 {
            if arms.iter().all(|arm| fits(*arm, radius * (sweep / 2.0).tan())) {
                break;
            }
            radius /= 2.0;
        }
        let tangent = radius * (sweep / 2.0).tan();
        for (line, arm) in lines.iter_mut().zip(arms) {
            for _ in 0..64 {
                if fits(arm, tangent + *line) {
                    break;
                }
                *line /= 2.0;
            }
        }
        let span = |side: (usize, usize), a: Vector2, line: f64| {
            let (s0, s1) = (station_on(&roads[side.0], point, corner + a * tangent), station_on(&roads[side.0], point, corner + a * (tangent + line)));
            (side, s0.min(s1), s0.max(s1))
        };
        out.push(CurbReturnPlace { corner, arms: [a_in, a_out], sides: [side_in, side_out], spans: [span(side_in, a_in, lines[0]), span(side_out, a_out, lines[1])], tangent, sweep, radius, lines });
    }
    Ok(out)
}

/// The line pieces of an alignment with the station each starts at (NaN
/// after a spiral: its length is not solved exactly).
pub(super) fn lines(doc: &CadDocument, alignment: Handle) -> Option<Vec<(f64, Vector2, Vector2)>> {
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
/// `other_profile` (null for none), on `layer` (layer "0" when `None`), with the
/// drawing's default intersection style. The alignments must meet once,
/// on line elements: crossing away from their ends, or one ending (or
/// starting) on the other (a tee junction; the through road is made the
/// main road, as the application does);
/// `assembly_sets` are the two assembly set files the record names (the
/// file and its form under the content folder, both supplied by the
/// caller). Only the intersection is written: offset alignments, curb
/// returns and corridor regions are not (the record's references to them
/// are null). [`add_intersection_with`] writes the offset alignments too.
#[allow(clippy::too_many_arguments)]
pub fn add_intersection(doc: &mut CadDocument, name: &str, main: Handle, main_profile: Handle, other: Handle, other_profile: Handle, assembly_sets: [&str; 2], layer: Option<&str>) -> Result<Handle, DxfError> {
    add_intersection_with(doc, name, main, main_profile, other, other_profile, assembly_sets, None, layer)
}

/// [`add_intersection`] with what the application's command writes with
/// it when `offsets` is given (its curb returns and corridors turned
/// off): an offset alignment on each side of each road over the road's
/// whole length, named `<road>-Left-<width>` and `<road>-Right-<width>`
/// (three decimals), with the application's design speed 60, each kept
/// by an offset alignment action (`AECC_ASSOC_OFFSET_ALIGNMENT_ACTION_BODY`;
/// the road read, the offset written); a profile locking action
/// (`AECC_ASSOC_PVILOCKING_MULTIALIGNINTER_ACTION_BODY`: the intersection,
/// the roads and the main road's profile read, the other road's profile
/// written; without profiles the intersection and the other road read;
/// at a crossing the other road's profile, when it is tangents only, is
/// written again with a PVI at the crossing at the main road's elevation,
/// as the command's locking leaves it: WC62);
/// the intersection action (`AECC_ASSOC_INTERSECTION_ACTION_BODY`: the roads
/// read, the intersection written); and the location label
/// (`AECC_INTERSECTION_LOCATION_LABEL`) with the drawing's default
/// intersection label style. The roads must be siteless alignments of
/// lines and arcs this crate writes.
#[allow(clippy::too_many_arguments)]
pub fn add_intersection_with(doc: &mut CadDocument, name: &str, main: Handle, main_profile: Handle, other: Handle, other_profile: Handle, assembly_sets: [&str; 2], offsets: Option<IntersectionOffsets>, layer: Option<&str>) -> Result<Handle, DxfError> {
    build_intersection(doc, name, main, main_profile, other, other_profile, assembly_sets, offsets, None, layer)
}

/// [`add_intersection_with`] with the curb returns the command makes in
/// every region of a crossing (its curb return alignments setting on,
/// profiles and corridors off; sample WC40): a curb return alignment
/// named `<intersection> - <region name>` of one unit along the incoming
/// road's offset alignment, a free arc of `radius` and one unit along the
/// outgoing road's offset alignment, kept by an associative action that
/// reads both offset alignments and the intersection. The offset
/// alignments then run only over the curb returns' stations on their road
/// and a thousandth of that span beyond, see [`CURB_RETURN_MARGIN`] (the command's intersection-extents
/// option); both offsets of a road run over every curb return line on the
/// road and the crossing station, cut to the road's ends (tee junctions,
/// WC50 to WC52).
#[allow(clippy::too_many_arguments)]
pub fn add_intersection_with_curb_returns(doc: &mut CadDocument, name: &str, main: Handle, main_profile: Handle, other: Handle, other_profile: Handle, assembly_sets: [&str; 2], offsets: IntersectionOffsets, radius: f64, layer: Option<&str>) -> Result<Handle, DxfError> {
    if !(radius.is_finite() && radius > 0.0) {
        return Err(invalid("a curb return radius must be positive"));
    }
    build_intersection(doc, name, main, main_profile, other, other_profile, assembly_sets, Some(offsets), Some((radius, None)), layer)
}

/// [`add_intersection_with_curb_returns`] with the command's curb return
/// profiles setting on as well (WC61: roads (0, 0)-(200, 0) and (100,
/// -100)-(100, 100) with profiles, widths 12, radius 25):
///
/// - The curb returns' lines run `profiles.incoming` and
///   `profiles.outgoing` along the lanes (25 by default); the offset
///   alignments run over those lines as before (124.248 long in WC61).
/// - The other road's profile is written again as fixed tangents through
///   its PVIs outside the crossing and new ones where it crosses the main
///   road's offset lines and the main road (elevations of the main road's
///   offset profiles and of its profile there), see
///   [`add_intersection_with`].
/// - Every offset alignment gets an offset profile from its road's
///   profile, `profiles.slope` times its width away from it, named
///   `<offset alignment> - <slope>%` (three decimals), see
///   [`super::add_offset_profile`]; the intersection names them.
/// - Every curb return gets a curb return profile `<curb return> -
///   Profile` of fixed tangents through the incoming lane's offset profile
///   at the curb return's start and at its arc's start and the outgoing
///   lane's at the arc's end and at its end (the command's joining
///   tangent rule), kept by an action
///   (`AECC_ASSOC_CURBRETURN_VALIGNMENT_ACTION_BODY`: both offset profiles,
///   the curb return and the intersection (order `0x30000`) read, the
///   profile written); the intersection names each region's profile.
///
/// Both roads need profiles of tangents; only a crossing is written.
#[allow(clippy::too_many_arguments)]
pub fn add_intersection_with_curb_return_profiles(doc: &mut CadDocument, name: &str, main: Handle, main_profile: Handle, other: Handle, other_profile: Handle, assembly_sets: [&str; 2], offsets: IntersectionOffsets, radius: f64, profiles: CurbReturnProfiles, layer: Option<&str>) -> Result<Handle, DxfError> {
    if !(radius.is_finite() && radius > 0.0) {
        return Err(invalid("a curb return radius must be positive"));
    }
    if !profiles.slope.is_finite() || [profiles.incoming, profiles.outgoing].iter().any(|l| !(l.is_finite() && *l > 0.0)) {
        return Err(invalid("curb return profiles need a finite slope and positive line lengths"));
    }
    if main_profile.is_null() || other_profile.is_null() {
        return Err(invalid("curb return profiles need both roads' profiles"));
    }
    build_intersection(doc, name, main, main_profile, other, other_profile, assembly_sets, Some(offsets), Some((radius, Some(profiles))), layer)
}

/// How far, as a share of the span of the curb returns' lines on it, an
/// offset alignment of an intersection with curb returns runs past those
/// lines at either end: lines over 62 to 63 and 137 to 138 give 61.924 to
/// 138.076; over radii 10 to 50 and widths 10 to 20 the offset is always
/// the lines' span stretched by a thousandth at each end.
pub const CURB_RETURN_MARGIN: f64 = 0.001;

#[allow(clippy::too_many_arguments)]
fn build_intersection(doc: &mut CadDocument, name: &str, main: Handle, main_profile: Handle, other: Handle, other_profile: Handle, assembly_sets: [&str; 2], offsets: Option<IntersectionOffsets>, curbs: Option<(f64, Option<CurbReturnProfiles>)>, layer: Option<&str>) -> Result<Handle, DxfError> {
    let curb_radius = curbs.map(|c| c.0);
    let curb_profiles = curbs.and_then(|c| c.1);
    let curb_lines = curb_profiles.map_or([1.0; 2], |p| [p.incoming, p.outgoing]);
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
    if super::site::site_container_of(doc, main) != super::site::site_container_of(doc, other) {
        return Err(invalid("the roads must lie on the same site, or both on none"));
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
    if a.0.is_nan() || b.0.is_nan() {
        return Err(invalid("a spiral comes before the crossing"));
    }
    let road = |al: Handle, profile: Handle, (station, p, q): (f64, Vector2, Vector2)| Road {
        alignment: al.value(),
        profile: profile.value(),
        station: station + p.distance(&point),
        direction: Vector2::new(q.x - p.x, q.y - p.y),
        ends: None,
        offsets: [0, 0],
        curb_returns: [0; 4],
        offset_profiles: [0; 2],
        curb_return_profiles: [0; 4],
    };
    let mut roads = [road(main, main_profile, a), Road { ends, ..road(other, other_profile, b) }];
    if let Some(o) = offsets {
        if main_profile.is_null() != other_profile.is_null() {
            return Err(invalid("an intersection with offset alignments takes both profiles or neither"));
        }
        if [o.main_left, o.main_right, o.other_left, o.other_right].iter().any(|w| !(w.is_finite() && *w > 0.0)) {
            return Err(invalid("offset widths must be positive"));
        }
        let label = ensure_intersection_label_style(doc)?;
        let styles = CommandStyles::of(doc);
        let widths = [[o.main_left, o.main_right], [o.other_left, o.other_right]];
        // The curb returns: per region its geometry, and the stations its
        // lines take on each offset alignment (road, side).
        let returns = match curb_radius {
            Some(radius) => {
                let range = |k: usize| arch(doc, Handle::new(roads[k].alignment)).and_then(|a| Some((a.alignment_start_station()?, super::corridor::alignment_length(a)))).map_or((f64::NEG_INFINITY, f64::INFINITY), |(s, l)| (s, s + l));
                Some(curb_returns(&roads, point, &widths, radius, curb_lines, [range(0), range(1)])?)
            }
            None => None,
        };
        for (k, r) in roads.iter_mut().enumerate() {
            let parent = Handle::new(r.alignment);
            let road_name = arch(doc, parent).and_then(|a| a.alignment_name()).unwrap_or_default();
            for (i, (side, width)) in [("Left", -widths[k][0]), ("Right", widths[k][1])].into_iter().enumerate() {
                // Both offsets of a road run over the stations of every
                // curb return line on the road and the crossing (WC50: a
                // tee's through road's right offset as its left).
                let range = returns.as_ref().map(|c| {
                    let s: Vec<f64> = c.iter().flat_map(|g| g.spans.iter().filter(|x| x.0 .0 == k).flat_map(|x| [x.1, x.2])).chain([r.station]).collect();
                    let (lo, hi) = (s.iter().copied().fold(f64::INFINITY, f64::min), s.iter().copied().fold(f64::NEG_INFINITY, f64::max));
                    let margin = (hi - lo) * CURB_RETURN_MARGIN;
                    (lo - margin, hi + margin)
                });
                let h = super::alignment::add_kept_offset_alignment(doc, parent, &format!("{road_name}-{side}-{:.3}", width.abs()), width, range, OFFSET_SPEED, styles.offsets, layer)?;
                add_action(doc, ActionBody { dxf: "AECC_ASSOC_OFFSET_ALIGNMENT_ACTION_BODY", refs: vec![h], tail: "" }, &[Dependency { object: parent, writes: false, order: 0 }, Dependency { object: h, writes: true, order: 0 }])?;
                r.offsets[i] = h.value();
            }
        }
        if let Some(c) = &returns {
            for g in 0..c.len() {
                roads[0].curb_returns[g] = doc.allocate_handle().value();
            }
        }
        // The profile locking reshapes the other road's profile at a
        // crossing (WC61, WC62).
        if roads[0].profile != 0 && roads[1].profile != 0 {
            // The intersection's extents along the other road: its curb
            // returns' farthest lines on it.
            let extents = returns.as_ref().map(|c| {
                let s: Vec<f64> = c.iter().flat_map(|g| g.spans.iter().filter(|x| x.0 .0 == 1).flat_map(|x| [x.1, x.2])).collect();
                (s.iter().copied().fold(f64::INFINITY, f64::min), s.iter().copied().fold(f64::NEG_INFINITY, f64::max))
            });
            lock_other_profile(doc, &roads, &widths, curb_profiles.map(|p| p.slope).zip(extents))?;
        }
        let handle = write_record(doc, name, &roads, assembly_sets, layer)?;
        let read = |h: u64| Dependency { object: Handle::new(h), writes: false, order: 0 };
        let wrote = |h: u64| Dependency { object: Handle::new(h), writes: true, order: 0 };
        let [m, x] = &roads;
        // The locking action without profiles depends on the intersection
        // and the other road only (WT44).
        let (refs, deps) = if m.profile != 0 {
            ([x.profile, handle.value(), 0, m.alignment, 0, 0, m.profile, 0], vec![Dependency { order: LOCKING_ORDER, ..read(handle.value()) }, read(x.alignment), wrote(x.profile), read(m.alignment), read(m.profile)])
        } else {
            ([0, handle.value(), 0, 0, 0, 0, 0, 0], vec![Dependency { order: LOCKING_ORDER, ..read(handle.value()) }, read(x.alignment)])
        };
        add_action(doc, ActionBody { dxf: "AECC_ASSOC_PVILOCKING_MULTIALIGNINTER_ACTION_BODY", refs: refs.map(Handle::new).to_vec(), tail: LOCKING }, &deps)?;
        add_action(doc, ActionBody { dxf: "AECC_ASSOC_INTERSECTION_ACTION_BODY", refs: vec![handle], tail: "" }, &[read(m.alignment), read(x.alignment), Dependency { order: INTERSECTION_ORDER, ..wrote(handle.value()) }])?;
        if let Some(c) = &returns {
            let regions = regions(&roads);
            // The curb return profiles' handles and identifiers: each curb
            // return lists its profile.
            let profile_ids: Vec<Option<(Handle, [u8; 16])>> = (0..c.len())
                .map(|_| {
                    curb_profiles.map(|_| {
                        let h = doc.allocate_handle();
                        (h, super::profile::new_identifier(h.value()))
                    })
                })
                .collect();
            for (g, (cr, region)) in c.iter().zip(&regions).enumerate() {
                let h = Handle::new(roads[0].curb_returns[g]);
                let road = |i: usize| Handle::new(roads[i].alignment);
                let offset = |(k, i): (usize, usize)| Handle::new(roads[k].offsets[i]);
                let stations = |i: usize, p: Vector2| station_on(&roads[[region.in_road, region.out_road][i]], point, p);
                super::alignment::add_intersection_curb_return(
                    doc,
                    h,
                    &format!("{name} - {}", REGION_NAMES[region.quadrant]),
                    [road(region.in_road), road(region.out_road)],
                    [offset(cr.sides[0]), offset(cr.sides[1])],
                    handle,
                    g as i32 + 1,
                    cr.corner,
                    cr.arms,
                    &stations,
                    region.in_forward,
                    region.out_forward,
                    cr.radius,
                    cr.lines,
                    profile_ids[g],
                    styles.curb_returns,
                    layer,
                )?;
                add_action(
                    doc,
                    ActionBody { dxf: "AECC_ASSOC_CURBRETURN_ALIGNMENT_ACTION_BODY", refs: vec![h], tail: "" },
                    &[read(offset(cr.sides[0]).value()), read(offset(cr.sides[1]).value()), Dependency { order: CURB_RETURN_ORDER, ..read(handle.value()) }, wrote(h.value())],
                )?;
            }
            // The offset profiles come after the curb returns (WC65: their
            // edges in the site's container follow the curb returns').
            if let Some(p) = curb_profiles {
                for k in 0..2 {
                    for i in 0..2 {
                        let offset = Handle::new(roads[k].offsets[i]);
                        let offset_name = arch(doc, offset).and_then(|a| a.alignment_name()).unwrap_or_default();
                        let h = super::profile::add_offset_profile_styled(doc, Handle::new(roads[k].profile), offset, &format!("{offset_name} - {:.3}%", p.slope * 100.0), p.slope, styles.profiles, layer)?;
                        roads[k].offset_profiles[i] = h.value();
                    }
                }
            }
            if let Some(p) = curb_profiles {
                for (g, (cr, region)) in c.iter().zip(&regions).enumerate() {
                    let h = Handle::new(roads[0].curb_returns[g]);
                    let lane = |i: usize| {
                        let (k, side) = cr.sides[i];
                        (k, side, Handle::new(roads[k].offset_profiles[side]))
                    };
                    // The lanes' offset profiles at the curb return's start,
                    // its arc's ends and its end.
                    let z = |i: usize, along: f64| -> Result<f64, DxfError> {
                        let (k, side, _) = lane(i);
                        let station = station_on(&roads[k], point, cr.corner + cr.arms[i] * (cr.tangent + along));
                        Ok(elevation_at(doc, Handle::new(roads[k].profile), station).ok_or_else(|| invalid("a road profile does not reach the curb return"))? + p.slope * widths[k][side])
                    };
                    let ([incoming, outgoing], arc) = (cr.lines, cr.radius * cr.sweep);
                    let pvis = [
                        Vector2::new(0.0, z(0, incoming)?),
                        Vector2::new(incoming, z(0, 0.0)?),
                        Vector2::new(incoming + arc, z(1, 0.0)?),
                        Vector2::new(incoming + arc + outgoing, z(1, outgoing)?),
                    ];
                    let cr_name = format!("{name} - {} - Profile", REGION_NAMES[region.quadrant]);
                    let (incoming, outgoing) = (lane(0).2, lane(1).2);
                    let (ph, pid) = profile_ids[g].ok_or_else(|| invalid("no curb return profile handle"))?;
                    let profile = super::profile::write_curb_return_profile(doc, ph, &pid, h, &cr_name, &pvis, incoming, outgoing, styles.profiles, layer)?;
                    add_action(
                        doc,
                        ActionBody { dxf: "AECC_ASSOC_CURBRETURN_VALIGNMENT_ACTION_BODY", refs: vec![profile], tail: "" },
                        &[read(incoming.value()), read(outgoing.value()), read(h.value()), wrote(profile.value()), Dependency { order: CURB_RETURN_PROFILE_ORDER, ..read(handle.value()) }],
                    )?;
                    roads[0].curb_return_profiles[g] = profile.value();
                }
                // The record again, naming the curb return profiles.
                let common = match doc.get_entity(handle) {
                    Some(crate::entities::EntityType::Extended(x)) => x.common.clone(),
                    _ => return Err(invalid("the intersection was not written")),
                };
                let style = arch(doc, handle).and_then(|a| a.typed_handles().get(1).map(|r| r.0)).unwrap_or(0);
                write_intersection(doc, common, name, style, &roads, assembly_sets)?;
            }
        }
        write_location_label(doc, handle, label, layer)?;
        return Ok(handle);
    }
    write_record(doc, name, &roads, assembly_sets, layer)
}

/// The intersection record with the drawing's default intersection style.
fn write_record(doc: &mut CadDocument, name: &str, roads: &[Road; 2], assembly_sets: [&str; 2], layer: Option<&str>) -> Result<Handle, DxfError> {
    ensure_intersection_styles(doc)?;
    let style = default_style_of(doc, KEY_INTERSECTION_STYLES).ok_or_else(|| invalid("drawing has no intersection styles"))?;
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    let handle = doc.allocate_handle();
    let common = EntityCommon { handle, owner_handle: doc.header.model_space_block_handle, layer: layer.into(), layer_handle: Some(layer_handle), ..EntityCommon::default() };
    write_intersection(doc, common, name, style, roads, assembly_sets)
}

/// The intersection's location label (an entity in model space): the
/// constant record [`LABEL`]; references soft 0, soft 0, the intersection
/// (soft), the label style and the intersection (hard); texts
/// "AeccImpIntersectionLocationLabel" and three empty ones.
fn write_location_label(doc: &mut CadDocument, intersection: Handle, style: u64, layer: Option<&str>) -> Result<Handle, DxfError> {
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    let handle = doc.allocate_handle();
    let common = EntityCommon { handle, owner_handle: doc.header.model_space_block_handle, layer: layer.into(), layer_handle: Some(layer_handle), ..EntityCommon::default() };
    let refs = vec![soft_pointer(0), soft_pointer(0), soft_pointer(intersection.value()), hard_pointer(style), hard_pointer(intersection.value())];
    let strings = ["AeccImpIntersectionLocationLabel", "", "", ""].map(String::from).to_vec();
    put_entity(doc, "AECC_INTERSECTION_LOCATION_LABEL", common, |w| super::templates::run(w, LABEL.0, LABEL.1), Body { refs, strings })
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
        // With offset alignments it takes both profiles or neither.
        assert!(add_intersection_with(&mut doc, "T", a, pa, b, Handle::NULL, ["", ""], Some(IntersectionOffsets::default()), None).is_err());
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

    #[test]
    fn intersections_with_offsets_get_their_actions_and_label() {
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let a = add_alignment(&mut doc, "Road A", &[v(0.0, 0.0), v(200.0, 0.0)], None).unwrap();
        let b = add_alignment(&mut doc, "Road B", &[v(100.0, 0.0), v(100.0, 100.0)], None).unwrap();
        let pa = add_profile(&mut doc, a, "PA", &[v(0.0, 10.0), v(200.0, 12.0)], &[], None).unwrap();
        let pb = add_profile(&mut doc, b, "PB", &[v(0.0, 11.0), v(100.0, 11.0)], &[], None).unwrap();
        let o = IntersectionOffsets { main_left: 10.0, ..IntersectionOffsets::default() };
        let i = add_intersection_with(&mut doc, "T", a, pa, b, pb, ["", ""], Some(o), None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-intersection-offsets-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let refs: Vec<u64> = arch(&back, i).unwrap().typed_handles().iter().map(|r| r.0).collect();
        // The main road's left offset alignment, its right one, then the
        // other road's.
        let left = arch(&back, Handle::new(refs[4])).unwrap();
        assert_eq!(left.alignment_name(), Some("Road A-Left-10.000".into()));
        assert_eq!(left.alignment_offset_parameters().unwrap().offset, -10.0);
        assert_eq!(left.alignment_design_speeds()[0].speed, OFFSET_SPEED);
        assert_eq!(arch(&back, Handle::new(refs[12])).unwrap().alignment_name(), Some("Road B-Right-12.000".into()));
        // The intersection action's and the locking action's dependencies,
        // lower order first; the parent road is depended on by both offset
        // actions, the locking action and the intersection action.
        let reactors = |h: Handle| back.get_entity(h).unwrap().common().reactors.clone();
        let order = |d: &Handle| object(&back, *d).unwrap().main_stream().map(|(data, s, e)| {
            let mut b = crate::classes::civil::bits::Bits::new(data, s, e);
            b.bit_short();
            b.bit_long();
            b.skip(4);
            b.bit_long().unwrap()
        });
        assert_eq!(reactors(i).iter().map(order).collect::<Vec<_>>(), vec![Some(INTERSECTION_ORDER), Some(LOCKING_ORDER)]);
        assert_eq!(reactors(a).len(), 4);
        let labels = back.entities().filter(|e| matches!(e, crate::entities::EntityType::Extended(x) if matches!(&x.data, crate::entities::ExtendedEntityData::Arch(l) if l.dxf_name == "AECC_INTERSECTION_LOCATION_LABEL"))).count();
        assert_eq!(labels, 1);
    }

    #[test]
    fn crossings_get_curb_returns_in_every_region() {
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let a = add_alignment(&mut doc, "Road A", &[v(0.0, 0.0), v(200.0, 0.0)], None).unwrap();
        let b = add_alignment(&mut doc, "Road B", &[v(100.0, -100.0), v(100.0, 100.0)], None).unwrap();
        let t = add_alignment(&mut doc, "Road T", &[v(150.0, 0.0), v(150.0, 100.0)], None).unwrap();
        // A tee junction gets its curb returns too (in a copy, so the
        // drawing below holds the crossing alone).
        assert!(add_intersection_with_curb_returns(&mut doc.clone(), "T", a, Handle::NULL, t, Handle::NULL, ["", ""], IntersectionOffsets::default(), 25.0, None).is_ok());
        let i = add_intersection_with_curb_returns(&mut doc, "I", a, Handle::NULL, b, Handle::NULL, ["", ""], IntersectionOffsets::default(), 25.0, None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-intersection-curb-returns-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let refs: Vec<u64> = arch(&back, i).unwrap().typed_handles().iter().map(|r| r.0).collect();
        // Per region: curb return, its profile (none), in road, out road.
        let ne = arch(&back, Handle::new(refs[14])).unwrap();
        assert_eq!(ne.alignment_name(), Some("I - NE - Quadrant".into()));
        assert_eq!(ne.alignment_type(), Some(crate::classes::civil::AlignmentType::CurbReturn));
        // NE runs west along Road A's left offset into Road B's right one.
        assert_eq!(ne.alignment_intersection_curb_return(), Some(([a.value(), b.value()], [refs[4], refs[12]], i.value())));
        assert_eq!(ne.alignment_connected_inputs(), Some([a.value(), b.value()]));
        let e = ne.alignment_elements().unwrap();
        assert_eq!(e.len(), 3);
        // The offset alignments run over the curb returns' stations and
        // the margin: 62 - 0.076 to 138 + 0.076.
        let left = arch(&back, Handle::new(refs[4])).unwrap().alignment_offset_parameters().unwrap();
        let margin = 76.0 * CURB_RETURN_MARGIN;
        assert!((left.regions[0].parent_start - (62.0 - margin)).abs() < 1e-9 && (left.regions[0].parent_end - (138.0 + margin)).abs() < 1e-9);
    }

    #[test]
    fn curb_return_profiles_join_the_offset_profiles() {
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let a = add_alignment(&mut doc, "Road A", &[v(0.0, 0.0), v(200.0, 0.0)], None).unwrap();
        let b = add_alignment(&mut doc, "Road B", &[v(100.0, -100.0), v(100.0, 100.0)], None).unwrap();
        let pa = add_profile(&mut doc, a, "PA", &[v(0.0, 10.0), v(200.0, 12.0)], &[], None).unwrap();
        let pb = add_profile(&mut doc, b, "PB", &[v(0.0, 11.0), v(200.0, 11.0)], &[], None).unwrap();
        assert!(add_intersection_with_curb_return_profiles(&mut doc, "I", a, Handle::NULL, b, pb, ["", ""], IntersectionOffsets::default(), 25.0, CurbReturnProfiles::default(), None).is_err());
        let i = add_intersection_with_curb_return_profiles(&mut doc, "I", a, pa, b, pb, ["", ""], IntersectionOffsets::default(), 25.0, CurbReturnProfiles::default(), None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-intersection-curb-return-profiles-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let refs: Vec<u64> = arch(&back, i).unwrap().typed_handles().iter().map(|r| r.0).collect();
        // The other road's profile meets the main road and its offset
        // profiles across the crossing.
        let pvis: Vec<(f64, f64)> = arch(&back, pb).unwrap().profile_pvis().iter().map(|p| (p.point.x, p.point.y)).collect();
        assert_eq!(pvis.len(), 5);
        for (got, want) in pvis.iter().zip([(0.0, 11.0), (88.0, 10.76), (100.0, 11.0), (112.0, 10.76), (200.0, 11.0)]) {
            assert!((got.0 - want.0).abs() < 1e-9 && (got.1 - want.1).abs() < 1e-9, "{got:?} {want:?}");
        }
        // Each road's offsets carry offset profiles 2 % below the road.
        let left = arch(&back, Handle::new(refs[5])).unwrap();
        assert_eq!(left.profile_name(), Some("Road A-Left-12.000 - -2.000%".into()));
        assert!((left.profile_offset().unwrap().slope + 0.02).abs() < 1e-12);
        // The NE curb return's profile: 25 along Road A's left lane, the
        // arc, 25 along Road B's right lane.
        let ne = arch(&back, Handle::new(refs[15])).unwrap();
        assert_eq!(ne.profile_name(), Some("I - NE - Quadrant - Profile".into()));
        let pvis: Vec<(f64, f64)> = ne.profile_pvis().iter().map(|p| (p.point.x, p.point.y)).collect();
        let arc = 25.0 * std::f64::consts::FRAC_PI_2;
        for (got, want) in pvis.iter().zip([(0.0, 11.38), (25.0, 11.13), (25.0 + arc, 10.76 + 0.24 * 25.0 / 88.0 - 0.24), (50.0 + arc, 10.76 + 0.24 * 50.0 / 88.0 - 0.24)]) {
            assert!((got.0 - want.0).abs() < 1e-9 && (got.1 - want.1).abs() < 1e-9, "{got:?} {want:?}");
        }
    }

    #[test]
    fn roads_on_a_site_join_it_with_their_offsets_and_curb_returns() {
        use crate::classes::civil::{add_alignment_on_site, add_site};
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let s = add_site(&mut doc, "Site 1", "").unwrap();
        let a = add_alignment_on_site(&mut doc, s, "Road A", &[v(0.0, 0.0), v(200.0, 0.0)], &[], None).unwrap();
        let b = add_alignment_on_site(&mut doc, s, "Road B", &[v(100.0, -100.0), v(100.0, 100.0)], &[], None).unwrap();
        let pa = add_profile(&mut doc, a, "PA", &[v(0.0, 10.0), v(200.0, 12.0)], &[], None).unwrap();
        let pb = add_profile(&mut doc, b, "PB", &[v(0.0, 11.0), v(200.0, 11.0)], &[], None).unwrap();
        // A siteless road does not meet roads on a site.
        let c = add_alignment(&mut doc, "Road C", &[v(150.0, -100.0), v(150.0, 100.0)], None).unwrap();
        assert!(add_intersection_with_curb_returns(&mut doc, "X", a, Handle::NULL, c, Handle::NULL, ["", ""], IntersectionOffsets::default(), 25.0, None).is_err());
        add_intersection_with_curb_return_profiles(&mut doc, "I", a, pa, b, pb, ["", ""], IntersectionOffsets::default(), 25.0, CurbReturnProfiles::default(), None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-intersection-on-site-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        // The roads, their profiles, four offsets, four curb returns and
        // eight profiles, listed in hash order (id 8 first).
        let container = arch(&back, s).unwrap().typed_handles()[5].0;
        let t = object(&back, Handle::new(container)).unwrap().topology().unwrap();
        assert_eq!(t.edge_ids.len(), 20);
        assert_eq!(&t.edge_ids[..3], &[8, 16, 1]);
        // Eight parcels: four squares, four corner regions labelled on
        // their corner's bisector.
        let mut labels: Vec<(i32, Vector2)> = back
            .entities()
            .filter_map(|e| match e {
                crate::entities::EntityType::Extended(x) => match &x.data {
                    crate::entities::ExtendedEntityData::Arch(p) if p.parcel_site() == Some(s.value()) => p.parcel_face().map(|f| f.0).zip(p.parcel_label()),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        labels.sort_by_key(|l| l.0);
        assert_eq!(labels.len(), 8);
        let at = |i: usize, x: f64, y: f64| (labels[i].1.x - x).abs() < 1e-9 && (labels[i].1.y - y).abs() < 1e-9;
        let d = 12.0 + 12.5 * (std::f64::consts::SQRT_2 - 1.0) / std::f64::consts::SQRT_2;
        assert!(at(0, 94.0, -6.0) && at(3, 106.0, 6.0));
        assert!(at(4, 100.0 + d, d), "{:?}", labels[4]);
    }

    #[test]
    fn tee_junctions_lock_the_other_profile_by_the_grade_rule() {
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let a = add_alignment(&mut doc, "Road A", &[v(0.0, 0.0), v(200.0, 0.0)], None).unwrap();
        let b = add_alignment(&mut doc, "Road B", &[v(100.0, 0.0), v(100.0, 100.0)], None).unwrap();
        let pa = add_profile(&mut doc, a, "PA", &[v(0.0, 10.0), v(200.0, 12.0)], &[], None).unwrap();
        let pb = add_profile(&mut doc, b, "PB", &[v(0.0, 11.0), v(100.0, 13.0)], &[], None).unwrap();
        add_intersection_with_curb_return_profiles(&mut doc, "T", a, pa, b, pb, ["", ""], IntersectionOffsets::default(), 25.0, CurbReturnProfiles::default(), None).unwrap();
        // The junction, the left offset line, then 2 % (the grade -2 % at
        // the offset line changed by 4 %) for 50 before the profile's end.
        let pvis: Vec<(f64, f64)> = arch(&doc, pb).unwrap().profile_pvis().iter().map(|p| (p.point.x, p.point.y)).collect();
        assert_eq!(pvis.len(), 4);
        for (got, want) in pvis.iter().zip([(0.0, 11.0), (12.0, 10.76), (62.0, 11.76), (100.0, 13.0)]) {
            assert!((got.0 - want.0).abs() < 1e-9 && (got.1 - want.1).abs() < 1e-9, "{got:?} {want:?}");
        }
    }

    #[test]
    fn the_grade_rule_keeps_its_maximum_grade_and_short_roads_halve_the_curb_returns() {
        let v = Vector2::new;
        let pvis_of = |b_end: f64, pb_end: f64| {
            let mut doc = CadDocument::new();
            doc.version = DxfVersion::AC1032;
            let a = add_alignment(&mut doc, "Road A", &[v(0.0, 0.0), v(200.0, 0.0)], None).unwrap();
            let b = add_alignment(&mut doc, "Road B", &[v(100.0, 0.0), v(100.0, b_end)], None).unwrap();
            let pa = add_profile(&mut doc, a, "PA", &[v(0.0, 10.0), v(200.0, 12.0)], &[], None).unwrap();
            let pb = add_profile(&mut doc, b, "PB", &[v(0.0, 11.0), v(b_end, pb_end)], &[], None).unwrap();
            add_intersection_with_curb_return_profiles(&mut doc, "T", a, pa, b, pb, ["", ""], IntersectionOffsets::default(), 25.0, CurbReturnProfiles::default(), None).unwrap();
            arch(&doc, pb).unwrap().profile_pvis().iter().map(|p| (p.point.x, p.point.y)).collect::<Vec<_>>()
        };
        let same = |got: Vec<(f64, f64)>, want: &[(f64, f64)]| {
            assert_eq!(got.len(), want.len(), "{got:?}");
            for (g, w) in got.iter().zip(want) {
                assert!((g.0 - w.0).abs() < 1e-9 && (g.1 - w.1).abs() < 1e-9, "{got:?} {want:?}");
            }
        };
        // -6 % beyond the offset line: limited to -2 % - 4 %, then to -5 %,
        // at the curb returns' end on the road (12 + 25 + 25).
        same(pvis_of(100.0, 3.0), &[(0.0, 11.0), (12.0, 10.76), (62.0, 8.26), (100.0, 3.0)]);
        // A road 30 long: the curb returns' radius halved to 12.5 (arc end
        // at 24.5), their lines to 3.125; the rule's PVI at their end.
        same(pvis_of(30.0, 12.0), &[(0.0, 11.0), (12.0, 10.76), (27.625, 11.0725), (30.0, 12.0)]);
    }
}
