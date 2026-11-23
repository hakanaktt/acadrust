//! Section views of sample lines.

use super::default_styles::{ensure_section_view_styles, KEY_SECTION_VIEW_STYLES};
use super::profile_view::{point, text};
use super::sample_lines::{arch, read_line, write_line};
use super::site::default_style_of;
use super::support::*;
use crate::entities::EntityCommon;
use crate::error::DxfError;
use crate::io::dwg::dwg_reference_type::DwgReferenceType;
use crate::io::dwg::dwg_stream_writers::DwgMergedWriter;
use crate::types::{Handle, Vector2};
use crate::CadDocument;

/// The class words of a section view record.
const GRAPH_WORDS: [i32; 2] = [357_877_593, 142_348_576];
/// The graph keys and their values.
const GRAPH_KEYS: [(u32, i32); 2] = [(0x0301_D029, 1), (0x0D01_D024, 0)];
/// The text id the view's range is kept under.
const VIEW_TEXT_ID: i32 = 0x8000_0440_u32 as i32;
/// The view title (a field code the application expands).
const TITLE: &str = "<[33673217(2097|2474|2387|2433|2593|2609|2657|2674|2689|2706|2721|2707|2803|2817)]>";
/// The range the default style gives a view: the sample line's offsets
/// and the section's elevations padded by 20 and rounded out to 10. A
/// section the application has not sampled has no elevations, so the
/// view spans elevations -20 to 20.
const MARGIN: f64 = 20.0;
const STEP: f64 = 10.0;
const ELEVATION_MIN: f64 = -20.0;
const ELEVATION_MAX: f64 = 20.0;

/// An offset value of the view's graph record.
#[derive(Clone, Copy)]
enum Value {
    OffsetMin,
    OffsetMax,
    Width,
    Center,
}

/// A piece of the view's graph record after the vertical scale.
enum Piece {
    /// A constant run (the same in every sample).
    Bits(&'static str),
    /// A `BD` from the view's offset range.
    Value(Value),
    /// The view title (`BL` byte count, UTF-16 with its terminator).
    Title,
    /// `BL` the index of the group plot that made the view (0 for a single
    /// view; WG01, WX23).
    Group,
}

/// The graph record after the vertical scale: the title again, then the
/// graph's frame and axes. Only the offset range's limits, width and
/// center depend on the view; the elevation values (-20 to 20) and the
/// default style's settings are part of the constant runs, which are the
/// same in every sample (views at two locations, offset ranges -30 to 30,
/// -40 to 50 and -30 to 60, lines with and without a section).
// ponytail: the runs are not split into fields; name them once a view of
// a sampled section (other elevations) or another style is written.
const GRAPH: &[Piece] = &[
    Piece::Bits("10100000010000000000100000000000000000"),
    Piece::Title,
    Piece::Bits("101000000000000000000000000000000000000000000000000000010001000100000010100001010101010101010101010101010101010101010101010110010101001111111000000100010001000100010001000100010001000100010001100000010011111110100100001000"),
    Piece::Value(Value::OffsetMin),
    Piece::Bits("000000000000000000000000000000000000000000000000000011010011000000101000000000000000000000000000000000000000000000000000001101001100000010"),
    Piece::Value(Value::OffsetMax),
    Piece::Bits("0000000000000000000000000000000000000000000000000000110100010000001001000001100100000001"),
    Piece::Value(Value::OffsetMin),
    Piece::Bits("10101000000000000000000000000000000000000000000000000000010001000100000010100000000000000000000000000000000000000000000000000001000100010000001010101001000001000100010100010001010000000001101100000000000110010100000000011101100000000001100001000000000111010000000000011010010000000001101111000000000110111000000000000000000000000010"),
    Piece::Value(Value::OffsetMin),
    Piece::Bits("0000000000000000000000000000000000000000000000000000110100010000001000000100010001000100010001000100010001000100010001100000011011111110100000010001000100010001000100010001000100010001000110000001001111111000110111100001111010000100100000000111110011011001000100100100000001000010000100000010101010100000000000000000000000000000000000000000000000000001000100010000001010000000000000000000000000000000000000000000000000000100010001000000101010100100000100010001010001000101000000000110110000000000011001010000000001110110000000000110000100000000011101000000000001101001000000000110111100000000011011100000000000000000000000001010000000000000000000000000000000000000000000000000000011010001000000100000010001000100010001000100010001000100010001000110000001101111111010000001000100010001000100010001000100010001000100011000000100111111101001000010000100000100"),
    Piece::Value(Value::OffsetMax),
    Piece::Bits("10101000000000000000000000000000000000000000000000000000010001000100000010100000000000000000000000000000000000000000000000000001000100010000001010101001000001000100010100010001010000000001101100000000000110010100000000011101100000000001100001000000000111010000000000011010010000000001101111000000000110111000000000000000000000000010"),
    Piece::Value(Value::OffsetMax),
    Piece::Bits("00000000000000000000000000000000000000000000000000001101000100000010000001000100010001000100010001000100010001000100011000000100111111101000000100010001000100010001000100010001000100010001100000010011111110000100101011011000000100100100110111111011001000011111100100111111010000100001000010001000000000000000000000000000000000000000000000000000010001000100000010"),
    Piece::Value(Value::Width),
    Piece::Bits("1010"),
    Piece::Value(Value::OffsetMax),
    Piece::Bits("1010"),
    Piece::Value(Value::OffsetMin),
    Piece::Bits("101001000001000100001110010011110000000001100110000000000110011000000000011100110000000001100101000000000111010000000000000000000000000010"),
    Piece::Value(Value::Center),
    Piece::Bits("00000000000000000000000000000000000000000000000000010001000100000010100000010001000100010001000100010001000100010001000110000001001111111000000100010001000100010001000100010001000100010001100000010011111110100100001000010001000010101010101010101010101010010000001000000000000000001010101010101010101001000001010100100000101010"),
    Piece::Value(Value::Width),
    Piece::Bits("1010"),
    Piece::Value(Value::OffsetMax),
    Piece::Bits("1010"),
    Piece::Value(Value::OffsetMin),
    Piece::Bits("101001000001000100001110010011110000000001100110000000000110011000000000011100110000000001100101000000000111010000000000000000000000000010"),
    Piece::Value(Value::Center),
    Piece::Bits("1010100000010001000100010001000100010001000100010001000110000001101111111000000100010001000100010001000100010001000100010001100000010011111110100100000010"),
    Piece::Value(Value::OffsetMin),
    Piece::Bits("000000000000000000000000000000000000000000000000000011010011000000101000000000000000000000000000000000000000000000000000001101001100000010"),
    Piece::Value(Value::OffsetMax),
    Piece::Bits("00000000000000000000000000000000000000000000000000001101000100000010010101010000010101010101010101010101010101010101010101010101101001010011111100010101010101010101010101010101010101010101010101101001010011111100010101010101010101010101010101010101010101010101101001010011111100010101010101010101010101010101010101010101010101101001010011111110"),
    Piece::Group,
    Piece::Bits("10101010100100000001100100000010100100000011100100000100100001011111000000100"),
];

/// The range of a view of `l`.
fn line_range(l: &super::sample_lines::Line) -> (f64, f64) {
    range(l.sides[0].iter().map(|v| -v.0).fold(0.0, f64::max), l.sides[1].iter().map(|v| v.0).fold(0.0, f64::max))
}

/// The range of a view of a line reaching `left` and `right`.
fn range(left: f64, right: f64) -> (f64, f64) {
    (((-left - MARGIN) / STEP).floor() * STEP, ((right + MARGIN) / STEP).ceil() * STEP)
}

/// The section view record (`AECC_GRAPH_SECTION_VIEW`, an entity in
/// model space):
///
/// ```text
/// BL 80, 2 bits 11, the class words, BL 5, BL 2, the graph keys,
/// BL 0 (byte form), 121 zero bits
/// 3BD origin, 9 BD (identity), BL n sections, n x 5 bits 10000, BL 0,
/// B 1, 3BD origin, BL 4, BL text id
/// 3BD (offset min, elevation min, 0), (0, elevation min, 0), (offset
///   max, elevation max, 0), twice
/// BL 528, the title (BL byte count, UTF-16), BL 0
/// 7 BD (0, height, 0, 0, 1/48, 0, 1/120)
/// BL 0, 0, 8, 1, 4, text id
/// 3BD origin, BD 0, 0, 0, 1, BD 1 (vertical scale), then [`GRAPH`]
/// references: soft 0, hard owner 0, style (hard), sample line (hard),
///   per section {section (soft), hard 0, hard 0}, hard 0 x 4, soft 0,
///   hard 0
/// texts: "", "", name, "", name, ""
/// ```
///
/// The origin is the point of offset 0 at the view's lowest elevation.
/// Written with the samples' origins, lines and sections the record
/// equals theirs bit for bit.
#[allow(clippy::too_many_arguments)]
fn write_view(doc: &mut CadDocument, common: EntityCommon, name: &str, origin: Vector2, (lo, hi): (f64, f64), style: u64, line: u64, sections: &[u64], group: i32) -> Result<Handle, DxfError> {
    let mut refs = vec![soft_pointer(0), Ref(DwgReferenceType::HardOwnership, 0), hard_pointer(style), hard_pointer(line)];
    for s in sections {
        refs.extend([soft_pointer(*s), hard_pointer(0), hard_pointer(0)]);
    }
    refs.extend([hard_pointer(0), hard_pointer(0), hard_pointer(0), hard_pointer(0), soft_pointer(0), hard_pointer(0)]);
    let strings = ["", "", name, "", name, ""].iter().map(|s| s.to_string()).collect();
    let corners = [Vector2::new(lo, ELEVATION_MIN), Vector2::new(0.0, ELEVATION_MIN), Vector2::new(hi, ELEVATION_MAX)];
    let value = |v: Value| match v {
        Value::OffsetMin => lo,
        Value::OffsetMax => hi,
        Value::Width => hi - lo,
        Value::Center => (lo + hi) / 2.0,
    };
    put_entity(
        doc,
        "AECC_GRAPH_SECTION_VIEW",
        common,
        |w: &mut DwgMergedWriter| {
            w.write_bit_long(80);
            bits(w, "11");
            for v in GRAPH_WORDS {
                w.write_bit_long(v);
            }
            w.write_bit_long(5);
            w.write_bit_long(GRAPH_KEYS.len() as i32);
            for (k, v) in GRAPH_KEYS {
                w.write_bit_long(k as i32);
                if k >> 24 == 3 {
                    w.write_byte(v as u8);
                } else {
                    w.write_bit_long(v);
                }
            }
            bl_byte(w, 0);
            bits(w, &"0".repeat(121));
            point(w, origin);
            for v in [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0] {
                w.write_bit_double(v);
            }
            w.write_bit_long(sections.len() as i32);
            for _ in sections {
                bits(w, "10000");
            }
            w.write_bit_long(0);
            w.write_bit(true);
            point(w, origin);
            w.write_bit_long(4);
            w.write_bit_long(VIEW_TEXT_ID);
            for p in corners.iter().chain(&corners) {
                point(w, *p);
            }
            w.write_bit_long(528);
            text(w, TITLE);
            w.write_bit_long(0);
            for v in [0.0, ELEVATION_MAX - ELEVATION_MIN, 0.0, 0.0, 1.0 / 48.0, 0.0, 1.0 / 120.0] {
                w.write_bit_double(v);
            }
            for v in [0, 0, 8, 1, 4, VIEW_TEXT_ID] {
                w.write_bit_long(v);
            }
            point(w, origin);
            for v in [0.0, 0.0, 0.0, 1.0, 1.0] {
                w.write_bit_double(v);
            }
            for piece in GRAPH {
                match piece {
                    Piece::Bits(b) => bits(w, b),
                    Piece::Value(v) => w.write_bit_double(value(*v)),
                    Piece::Title => text(w, TITLE),
                    Piece::Group => w.write_bit_long(group),
                }
            }
        },
        Body { refs, strings },
    )
}

/// Adds a section view `name` of the sample line `line`, its point of
/// offset 0 at the lowest elevation at `origin`, on `layer` (layer "0"
/// when `None`), with the drawing's default section view style and the
/// default range: the line's offsets and the section's elevations padded
/// by 20 and rounded out to 10. The line must be one this crate writes
/// (it is written again with the view in its list) and its sections (one
/// per source, SM01) not yet sampled (as [`super::add_sections`] writes
/// them): the view spans elevations -20 to 20.
pub fn add_section_view(doc: &mut CadDocument, line: Handle, name: &str, origin: Vector2, layer: Option<&str>) -> Result<Handle, DxfError> {
    if name.is_empty() {
        return Err(invalid("a section view needs a name"));
    }
    if !origin.x.is_finite() || !origin.y.is_finite() {
        return Err(invalid("a section view needs a finite origin"));
    }
    let mut l = read_line(doc, line).ok_or_else(|| invalid("the sample line is not one this crate writes"))?;
    if l.sections.iter().any(|s| arch(doc, Handle::new(*s)).and_then(|a| a.section_geometry()).is_some_and(|g| !g.points.is_empty())) {
        return Err(invalid("views of a sampled section are not written"));
    }
    ensure_section_view_styles(doc)?;
    let style = default_style_of(doc, KEY_SECTION_VIEW_STYLES).ok_or_else(|| invalid("drawing has no section view styles"))?;
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    let handle = doc.allocate_handle();
    let common = EntityCommon { handle, owner_handle: doc.header.model_space_block_handle, layer: layer.into(), layer_handle: Some(layer_handle), ..EntityCommon::default() };
    let sections = l.sections.clone();
    write_view(doc, common, name, origin, line_range(&l), style, line.value(), &sections, 0)?;
    l.views.push(handle.value());
    write_line(doc, &l)?;
    Ok(handle)
}

/// Where the default group plot style puts a group plot's views (WG01 to
/// WG12): from the plot's point in by `PLOT_MARGIN`, filled by rows from
/// the lower left, `ceil(sqrt(n))` views a row; each column as wide as its
/// widest reach left and right of the views' centerlines (aligned), each
/// row as high as its highest view; `COLUMN_GAP` and `ROW_GAP` between
/// them.
const PLOT_MARGIN: f64 = 0.5;
const COLUMN_GAP: f64 = 2.0;
const ROW_GAP: f64 = 2.5;
/// The name template of a group plot's views (template id 0x8F).
const VIEW_NAMES: (i32, &str) = (0x8F, "<[Section View Station]> (<[Next Counter(CP)]>)");

/// The origins of `n` views of the ranges `ranges` placed by a group plot
/// at `at` (see `PLOT_MARGIN`).
fn plot_origins(at: Vector2, ranges: &[(f64, f64)]) -> Vec<Vector2> {
    let n = ranges.len();
    let columns = (1..=n).find(|c| c * c >= n).unwrap_or(1);
    let height = ELEVATION_MAX - ELEVATION_MIN;
    let column = |c: usize| ranges.iter().skip(c).step_by(columns);
    let left: Vec<f64> = (0..columns).map(|c| column(c).map(|r| -r.0).fold(0.0, f64::max)).collect();
    let right: Vec<f64> = (0..columns).map(|c| column(c).map(|r| r.1).fold(0.0, f64::max)).collect();
    (0..n)
        .map(|i| {
            let (row, c) = (i / columns, i % columns);
            let x = (0..c).map(|k| left[k] + right[k] + COLUMN_GAP).sum::<f64>() + left[c];
            Vector2::new(at.x + PLOT_MARGIN + x, at.y + PLOT_MARGIN + row as f64 * (height + ROW_GAP))
        })
        .collect()
}

/// A station as the view names show it: hundreds, `+`, the rest with two
/// decimals (`0+25.00`).
fn station_text(station: f64) -> String {
    let cents = (station * 100.0).round() as i64;
    let (sign, cents) = if cents < 0 { ("-", -cents) } else { ("", cents) };
    format!("{sign}{}+{:02}.{:02}", cents / 10_000, cents / 100 % 100, cents % 100)
}

/// Adds a group plot of `group`'s section views at `at` (the lower left
/// of the plot), as the application's default group plot makes it: one
/// view per line in station order, named by station and a drawing-wide
/// counter (`0+25.00 (1)`), placed by the default group plot style (see
/// `PLOT_MARGIN`), with the default section view style, on `layer` (layer
/// "0" when `None`). The group lists the plot (its group plot style, the
/// point, the count 3) and each line its view. The lines must be ones
/// this crate writes, their sections not yet sampled.
/// Returns the views.
pub fn add_section_view_group(doc: &mut CadDocument, group: Handle, at: Vector2, layer: Option<&str>) -> Result<Vec<Handle>, DxfError> {
    use super::sample_lines::{read_group, write_group, Plot};
    if !at.x.is_finite() || !at.y.is_finite() {
        return Err(invalid("a group plot needs a finite point"));
    }
    let mut g = read_group(doc, group).ok_or_else(|| invalid("the group is not one this crate writes"))?;
    let mut lines = g.lines.iter().map(|l| read_line(doc, Handle::new(*l))).collect::<Option<Vec<_>>>().ok_or_else(|| invalid("a line of the group cannot be read"))?;
    if lines.is_empty() {
        return Err(invalid("the group has no lines"));
    }
    for l in &lines {
        if l.sections.iter().any(|s| arch(doc, Handle::new(*s)).and_then(|a| a.section_geometry()).is_some_and(|g| !g.points.is_empty())) {
            return Err(invalid("views of lines with sampled sections are not written"));
        }
    }
    ensure_section_view_styles(doc)?;
    super::default_styles::ensure_group_plot_styles(doc)?;
    let style = default_style_of(doc, KEY_SECTION_VIEW_STYLES).ok_or_else(|| invalid("drawing has no section view styles"))?;
    let plot_style = default_style_of(doc, super::default_styles::KEY_GROUP_PLOT_STYLES).ok_or_else(|| invalid("drawing has no group plot styles"))?;
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    let first = super::assembly::take_names(doc, VIEW_NAMES, lines.len() as i32)?;
    let index = g.plots.iter().map(|p| p.index).max().unwrap_or(0) + 1;
    let ranges: Vec<(f64, f64)> = lines.iter().map(line_range).collect();
    let origins = plot_origins(at, &ranges);
    let mut views = Vec::with_capacity(lines.len());
    for (i, l) in lines.iter_mut().enumerate() {
        let handle = doc.allocate_handle();
        let common = EntityCommon { handle, owner_handle: doc.header.model_space_block_handle, layer: layer.into(), layer_handle: Some(layer_handle), ..EntityCommon::default() };
        let name = format!("{} ({})", station_text(l.station), first + i as i32);
        let sections = l.sections.clone();
        write_view(doc, common, &name, origins[i], ranges[i], style, l.common.handle.value(), &sections, index)?;
        l.views.push(handle.value());
        write_line(doc, l)?;
        views.push(handle);
    }
    g.plots.push(Plot { index, style: plot_style, origin: crate::types::Vector3::new(at.x, at.y, 0.0), count: super::sample_lines::PLOT_COUNT });
    write_group(doc, &g)?;
    Ok(views)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classes::civil::SectionViewFrame;
    use crate::io::dwg::{DwgReader, DwgWriter};
    use crate::types::DxfVersion;
    use super::super::sample_lines::read_group;

    #[test]
    fn views_take_the_line_offsets_and_join_the_line() {
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0)], None).unwrap();
        let g = crate::classes::civil::add_sample_line_group(&mut doc, al, "SLG1", None).unwrap();
        let a = crate::classes::civil::add_sample_line(&mut doc, g, "SL1", 50.0, 3.0, 33.0, None).unwrap();
        let b = crate::classes::civil::add_sample_line(&mut doc, g, "SL2", 80.0, 10.0, 10.0, None).unwrap();
        let p = |x: f64, y: f64| crate::types::Vector3::new(x, y, 10.0);
        let surface = crate::classes::civil::add_tin_surface(&mut doc, "S1", &[p(-10.0, -40.0), p(110.0, -40.0), p(110.0, 40.0)], None).unwrap();
        crate::classes::civil::add_sections(&mut doc, g, surface).unwrap();
        assert!(add_section_view(&mut doc, a, "", v(200.0, 0.0), None).is_err());
        assert!(add_section_view(&mut doc, g, "SV1", v(200.0, 0.0), None).is_err());
        let first = add_section_view(&mut doc, a, "SV1", v(200.0, 0.0), None).unwrap();
        let second = add_section_view(&mut doc, a, "SV1b", v(200.0, 100.0), None).unwrap();
        let third = add_section_view(&mut doc, b, "SV2", v(300.0, 0.0), None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-section-views-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let frame = |h: Handle| arch(&back, h).unwrap().section_view_frame().unwrap();
        assert_eq!(frame(first), SectionViewFrame { origin: v(200.0, 0.0), offset_min: -30.0, offset_max: 60.0, elevation_min: -20.0, elevation_max: 20.0, vertical_scale: 1.0 });
        assert_eq!(frame(second).origin, v(200.0, 100.0));
        assert_eq!((frame(third).offset_min, frame(third).offset_max), (-30.0, 30.0));
        let line = read_line(&back, a).unwrap();
        assert_eq!(line.views, vec![first.value(), second.value()]);
        let view = arch(&back, first).unwrap();
        let refs: Vec<u64> = view.typed_handles().iter().map(|r| r.0).collect();
        // The drawing gets the default section view style.
        let style = default_style_of(&back, KEY_SECTION_VIEW_STYLES).unwrap();
        assert_eq!(object(&back, Handle::new(style)).unwrap().dxf_name, "AECC_SECTION_VIEW_STYLE");
        assert_eq!(&refs[2..5], &[style, a.value(), line.sections[0]]);
        assert_eq!(view.decode().strings[2], "SV1");
    }

    #[test]
    fn group_plots_place_one_view_per_line_by_rows() {
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0)], None).unwrap();
        let g = crate::classes::civil::add_sample_line_group(&mut doc, al, "SLG", None).unwrap();
        for (n, st) in [("SL1", 25.0), ("SL2", 50.0), ("SL3", 75.0)] {
            crate::classes::civil::add_sample_line(&mut doc, g, n, st, 25.0, 15.0, None).unwrap();
        }
        let first = add_section_view_group(&mut doc, g, v(200.0, 0.0), None).unwrap();
        let second = add_section_view_group(&mut doc, g, v(300.0, -200.0), None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-section-view-groups-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        // WG01: two views a row, 92 apart, rows 42.5 apart.
        let origins: Vec<Vector2> = first.iter().map(|h| arch(&back, *h).unwrap().section_view_frame().unwrap().origin).collect();
        assert_eq!(origins, vec![v(250.5, 0.5), v(342.5, 0.5), v(250.5, 43.0)]);
        assert_eq!(arch(&back, second[0]).unwrap().decode().strings[2], "0+25.00 (4)");
        let plots = arch(&back, g).unwrap().section_view_group_plots().unwrap();
        assert_eq!(plots.iter().map(|p| (p.index, p.origin.y, p.count)).collect::<Vec<_>>(), vec![(1, 0.0, 3), (2, -200.0, 3)]);
        assert_eq!(read_line(&back, Handle::new(read_group(&back, g).unwrap().lines[0])).unwrap().views.len(), 2);
        assert_eq!(station_text(125.5), "1+25.50");
    }

    /// A line with a section of each of two surfaces (SM01, SM02): its view
    /// lists both sections, and a group plot's count stays 3.
    #[test]
    fn views_of_lines_with_several_sections() {
        use crate::types::Vector3;
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0)], None).unwrap();
        let surface = |doc: &mut CadDocument, name: &str, z: f64| {
            let points: Vec<Vector3> = [-10.0, 50.0, 110.0].iter().flat_map(|x| [-40.0, 0.0, 40.0].map(|y| Vector3::new(*x, y, z + x / 10.0))).collect();
            crate::classes::civil::add_tin_surface(doc, name, &points, None).unwrap()
        };
        let (s1, s2) = (surface(&mut doc, "S1", 10.0), surface(&mut doc, "S2", 8.0));
        let g = crate::classes::civil::add_sample_line_group(&mut doc, al, "SLG1", None).unwrap();
        let l1 = crate::classes::civil::add_sample_line(&mut doc, g, "SL1", 50.0, 25.0, 15.0, None).unwrap();
        crate::classes::civil::add_sample_line(&mut doc, g, "SL2", 75.0, 10.0, 30.0, None).unwrap();
        let a = crate::classes::civil::add_sections(&mut doc, g, s1).unwrap();
        let b = crate::classes::civil::add_sections(&mut doc, g, s2).unwrap();
        let view = add_section_view(&mut doc, l1, "SV1", v(200.0, 0.0), None).unwrap();
        add_section_view_group(&mut doc, g, v(400.0, 0.0), None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-section-views-several-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let refs: Vec<u64> = arch(&back, view).unwrap().typed_handles().iter().map(|r| r.0).collect();
        // The view lists the line's sections in the line's order.
        let at = |h: Handle| refs.iter().position(|r| *r == h.value()).expect("section listed");
        assert!(at(a[0]) < at(b[0]));
        assert_eq!(read_line(&back, l1).unwrap().sections, vec![a[0].value(), b[0].value()]);
        assert_eq!(arch(&back, view).unwrap().section_view_frame().unwrap().origin, v(200.0, 0.0));
        assert_eq!(arch(&back, g).unwrap().section_view_group_plots().unwrap()[0].count, 3);
        assert_eq!(read_line(&back, l1).unwrap().views.len(), 2);
    }
}
