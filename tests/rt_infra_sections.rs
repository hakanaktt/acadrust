//! Codec-only roundtrip tests for sections (unsampled, §3j) and section
//! views with their group plots (§3j).
//!
//! Documented subset (READY-WITH-LIMITS):
//! - sections are written before the application samples them: a written
//!   section holds no points, so `section_geometry()` reads an empty
//!   geometry after the reopen (asserted), and nothing asserts sampled
//!   geometry;
//! - corridor and pipe network section sources are not written (§3j);
//! - views of sampled sections are not written; the unsampled view constants are -20..20 in elevation
//!   with the line's offsets padded by 20 and rounded out to 10 (§3j).

use acadrust::classes::arch::ArchEntityData;
use acadrust::classes::civil::{
    add_alignment, add_sample_line, add_sample_line_group, add_sections, add_section_view,
    add_section_view_group, add_tin_surface,
};
use acadrust::classes::civil::{SectionViewFrame, SectionViewGroupPlot};
use acadrust::entities::{EntityType, ExtendedEntityData};
use acadrust::types::{DxfVersion, Handle, Vector2, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter};

fn v(x: f64, y: f64) -> Vector2 {
    Vector2::new(x, y)
}
fn p(x: f64, y: f64, z: f64) -> Vector3 {
    Vector3::new(x, y, z)
}

fn doc() -> CadDocument {
    let mut doc = CadDocument::new();
    doc.version = DxfVersion::AC1032;
    doc
}

fn roundtrip(doc: &CadDocument, tag: &str) -> CadDocument {
    let path = std::env::temp_dir().join(format!("rt_infra_{tag}_{}.dwg", std::process::id()));
    DwgWriter::write_to_file(&path, doc).expect("DWG write failed");
    let back = DwgReader::from_file(&path)
        .expect("DWG reopen failed")
        .read()
        .expect("DWG read failed");
    let _ = std::fs::remove_file(&path);
    back
}

fn entity(doc: &CadDocument, handle: Handle) -> ArchEntityData {
    let EntityType::Extended(e) = doc
        .get_entity(handle)
        .unwrap_or_else(|| panic!("entity {:#x} missing", handle.value()))
    else {
        panic!("handle {:#x} is not an extended entity", handle.value());
    };
    let ExtendedEntityData::Arch(data) = &e.data else {
        panic!("handle {:#x} is not an Arch entity", handle.value());
    };
    data.clone()
}

fn refs(data: &ArchEntityData) -> Vec<u64> {
    data.typed_handles().iter().map(|r| r.0).collect()
}

/// An unsampled section source: the group's two lines get one section
/// each, named "<group> - <line> - <surface>(1)"; the reopened sections
/// carry the style, line and source references and hold no points.
#[test]
fn unsampled_sections_survive_and_hold_no_points() {
    let mut doc = doc();
    let al = add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0)], None).unwrap();
    let g = add_sample_line_group(&mut doc, al, "SLG-RT", None).unwrap();
    let l1 = add_sample_line(&mut doc, g, "SL1", 25.0, 25.0, 15.0, None).unwrap();
    let l2 = add_sample_line(&mut doc, g, "SL2", 75.0, 25.0, 15.0, None).unwrap();
    let surface = add_tin_surface(&mut doc, "S-RT", &[p(-10.0, -40.0, 10.0), p(110.0, -40.0, 10.0), p(110.0, 40.0, 10.0)], None).unwrap();

    let sections = add_sections(&mut doc, g, surface).unwrap();
    assert_eq!(sections.len(), 2, "one section per line");
    // A group that samples sources takes no further lines (§3j).
    assert!(add_sample_line(&mut doc, g, "SL3", 50.0, 5.0, 5.0, None).is_err());
    let names: Vec<String> = sections
        .iter()
        .map(|h| entity(&doc, *h).decode().strings[2].clone())
        .collect();

    let back = roundtrip(&doc, "sections");

    // The sections reopen as AECC_SECTION entities named
    // "<group> - <line> - <surface>(1)" (§3j).
    let reopened: Vec<(Handle, String)> = sections
        .iter()
        .map(|h| (*h, entity(&back, *h).decode().strings[2].clone()))
        .collect();
    for ((_, name), line) in reopened.iter().zip(["SL1", "SL2"]) {
        assert_eq!(name, &format!("SLG-RT - {line} - S-RT(1)"));
    }
    assert_eq!(reopened.iter().map(|r| r.1.clone()).collect::<Vec<_>>(), names);

    // After the reopen: the sections decode with no points (the
    // application samples them; READY-WITH-LIMITS subset). The reader
    // surfaces the unsampled record as an empty geometry.
    for h in &sections {
        let s = entity(&back, *h);
        assert_eq!(s.dxf_name, "AECC_SECTION");
        assert!(s.section_geometry().expect("an unsampled geometry").points.is_empty());
    }

    // References: soft 0, hard owner 0, section style, sample line
    // (hard), source (soft) — §3j.
    let s1_refs = refs(&entity(&back, sections[0]));
    assert_eq!(s1_refs.get(3), Some(&l1.value()), "the section names its line");
    assert_eq!(s1_refs.get(4), Some(&surface.value()), "the section names its source");
    assert!(s1_refs.get(2).is_some_and(|r| *r != 0), "section style written");

    // The line lists its section; the group lists its source with the
    // section style (§3j).
    let line_refs = refs(&entity(&back, l1));
    assert!(line_refs.contains(&sections[0].value()), "the line lists its section");
    let group_refs = refs(&entity(&back, g));
    assert!(group_refs.contains(&surface.value()), "the group lists its source surface");

    // Each line keeps its station and swath. (The group's
    // `sample_lines()` reads the record before it gains section sources;
    // with a source the group record carries the source lists and the
    // reader returns None — reader boundary.)
    assert_eq!(entity(&back, l1).sample_line_station(), Some(25.0));
    assert_eq!(entity(&back, l2).sample_line_station(), Some(75.0));
    assert_eq!(reopened[1].1, "SLG-RT - SL2 - S-RT(1)");
}

/// A section view of an unsampled line: the range the application gives a
/// new view — the line's offsets padded by 20 and rounded out to 10,
/// elevations -20..20 — and a group plot with one view per line (§3j).
#[test]
fn section_views_and_group_plots_survive() {
    let mut doc = doc();
    let al = add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0)], None).unwrap();
    // A single-line group for the single view, a three-line group for
    // the plot (a plot makes one view per line of its group).
    let g_view = add_sample_line_group(&mut doc, al, "SLG-VIEW", None).unwrap();
    let lone = add_sample_line(&mut doc, g_view, "SL-LONE", 20.0, 10.0, 10.0, None).unwrap();
    let g = add_sample_line_group(&mut doc, al, "SLG-PLOT", None).unwrap();
    let mut plots_lines = Vec::new();
    for (n, st) in [("SL1", 25.0), ("SL2", 50.0), ("SL3", 75.0)] {
        plots_lines.push(add_sample_line(&mut doc, g, n, st, 25.0, 15.0, None).unwrap());
    }
    // A single view of the -10..10 line: offsets -30..30, elevations
    // -20..20, vertical scale 1 (the unsampled constants, §3j).
    let view = add_section_view(&mut doc, lone, "SV-LONE", v(500.0, 0.0), None).unwrap();
    // A group plot of three lines: one view per line in station order
    // (§3j's placement: in from the plot point by 0.5, two views a row).
    let views = add_section_view_group(&mut doc, g, v(200.0, 0.0), None).unwrap();
    assert_eq!(views.len(), 3);

    let back = roundtrip(&doc, "section-views");

    let frame = entity(&back, view).section_view_frame().unwrap();
    assert_eq!(
        frame,
        SectionViewFrame { origin: v(500.0, 0.0), offset_min: -30.0, offset_max: 30.0, elevation_min: -20.0, elevation_max: 20.0, vertical_scale: 1.0 }
    );
    // The view names its sample line (hard); the line has no section.
    let vrefs = refs(&entity(&back, view));
    assert_eq!(vrefs.get(3), Some(&lone.value()));
    assert_eq!(entity(&back, view).decode().strings[2], "SV-LONE");
    // The line lists its view.
    assert!(refs(&entity(&back, lone)).contains(&view.value()));

    // The plotted views: the same constant ranges, the documented
    // placement (WG01: two views a row, 92 apart, rows 42.5 apart).
    let origins: Vec<(f64, f64)> = views
        .iter()
        .map(|h| {
            let f = entity(&back, *h).section_view_frame().unwrap();
            (f.origin.x, f.origin.y)
        })
        .collect();
    assert_eq!(
        origins,
        vec![(250.5, 0.5), (342.5, 0.5), (250.5, 43.0)],
        "placement by the default group plot style"
    );
    for (h, line) in views.iter().zip(&plots_lines) {
        let f = entity(&back, *h).section_view_frame().unwrap();
        assert_eq!((f.offset_min, f.offset_max, f.elevation_min, f.elevation_max, f.vertical_scale), (-50.0, 40.0, -20.0, 20.0, 1.0));
        assert_eq!(refs(&entity(&back, *h)).get(3), Some(&line.value()));
    }
    // Named by station and a counter across the drawing's group plots:
    // a single view takes no counter, so the plot runs (1) to (3).
    let names: Vec<String> = views
        .iter()
        .map(|h| entity(&back, *h).decode().strings[2].clone())
        .collect();
    assert_eq!(names, ["0+25.00 (1)", "0+50.00 (2)", "0+75.00 (3)"]);

    // The group's plots read back: index, point, the stored count 3, the
    // constant values -30, 30, 40 (§3j).
    let plots: Vec<SectionViewGroupPlot> = entity(&back, g).section_view_group_plots().unwrap();
    assert_eq!(plots.len(), 1);
    let plot = &plots[0];
    assert_eq!((plot.index, plot.count), (1, 3));
    assert_eq!((plot.origin.x, plot.origin.y, plot.origin.z), (200.0, 0.0, 0.0));
    assert_eq!(plot.values, [-30.0, 30.0, 40.0]);
    assert_eq!(plot.extra, [0.0; 8]);
    assert_eq!(plot.flags, [false, false]);
}
