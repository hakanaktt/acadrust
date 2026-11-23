//! Codec-only roundtrip tests for profile views, their label groups,
//! their styles and their bands (§3h).
//!
//! Documented subset (READY-WITH-LIMITS, §3h "Bands"): the pressure
//! network, cant, distance, COGO and speed band kinds have no writer
//! (the API adds no such band to a view); the major/minor intervals and
//! the stagger line height stay at their fixed values, so only the
//! written weeding, stagger and label visibility are asserted.

use acadrust::classes::arch::ArchEntityData;
use acadrust::classes::civil::{
    add_alignment, add_profile, add_profile_view, add_profile_view_style,
    add_profile_view_with_bands, add_profile_view_with_range, ensure_civil, BandKind,
    BandStagger, ProfileViewBand, ProfileViewFrame,
};
use acadrust::classes::civil::ProfileViewStyleSettings;
use acadrust::entities::{EntityType, ExtendedEntityData};
use acadrust::types::{DxfVersion, Handle, Vector2};
use acadrust::{CadDocument, DwgReader, DwgWriter};

fn v(x: f64, y: f64) -> Vector2 {
    Vector2::new(x, y)
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

/// Drawing with one 300 m tangent alignment and a profile with two
/// parabolas (crest then sag), as §3h's samples.
fn drawing() -> (CadDocument, Handle, Handle) {
    let mut doc = doc();
    ensure_civil(&mut doc).unwrap();
    let al = add_alignment(&mut doc, "AL-PV", &[v(0.0, 0.0), v(300.0, 0.0)], None).unwrap();
    let pf = add_profile(
        &mut doc,
        al,
        "PF-PV",
        &[v(0.0, 10.0), v(50.0, 20.0), v(120.0, 12.0), v(150.0, 18.0)],
        &[60.0, 24.0],
        None,
    )
    .unwrap();
    (doc, al, pf)
}

/// A view of the automatic range: stations 200 before the alignment's
/// start to 200 past its end, the profiles' elevations rounded out to 10
/// with 20 more below and above (WV01, §3h) — equal before and after the
/// save, to the digits.
#[test]
fn profile_view_automatic_range_survives() {
    let (mut doc, al, pf) = drawing();
    let view = add_profile_view(&mut doc, al, "PV-AUTO", v(0.0, -100.0), None).unwrap();
    let user = add_profile_view_with_range(
        &mut doc,
        al,
        "PV-USER",
        v(400.0, -200.0),
        Some((-20.0, 320.0)),
        Some((-5.5, 30.25)),
        None,
    )
    .unwrap();

    let back = roundtrip(&doc, "profile-view");

    assert_eq!(
        entity(&back, view).profile_view_frame().unwrap(),
        ProfileViewFrame { origin: v(0.0, -100.0), station_min: -200.0, station_max: 500.0, elevation_min: -10.0, elevation_max: 40.0, vertical_scale: 10.0 }
    );
    // The user range is kept as written (E-PR1's third view).
    assert_eq!(
        entity(&back, user).profile_view_frame().unwrap(),
        ProfileViewFrame { origin: v(400.0, -200.0), station_min: -20.0, station_max: 320.0, elevation_min: -5.5, elevation_max: 30.25, vertical_scale: 10.0 }
    );
    // The view names its alignment (hard) and each profile (soft).
    let vrefs = refs(&entity(&back, view));
    assert_eq!(vrefs.get(3), Some(&al.value()));
    assert!(vrefs.contains(&pf.value()), "the view lists its profile");
}

/// A style with another vertical exaggeration survives with its
/// exaggeration; the view written with it reads the style's scale. With
/// the default profile label set the view's profiles get their four label
/// groups, whose labelled entities are the profile's tangents and curves.
#[test]
fn profile_view_style_and_label_groups_survive() {
    let (mut doc, al, pf) = drawing();
    let style = add_profile_view_style(&mut doc, "VS-RT", 5.0).unwrap();
    let view = add_profile_view(&mut doc, al, "PV-STYLE", v(0.0, -100.0), None).unwrap();
    let styled = acadrust::classes::civil::add_profile_view_with_style(
        &mut doc,
        al,
        "PV-EXAG",
        v(600.0, 0.0),
        Some(style),
        None,
        None,
        None,
    )
    .unwrap();

    let back = roundtrip(&doc, "profile-view-style");

    // The style: the default settings but for the exaggeration.
    let style_object = back
        .objects
        .get(&style)
        .map(|o| match o {
            acadrust::objects::ObjectType::Arch(a) => a.clone(),
            _ => panic!("the style is not an Arch object"),
        })
        .unwrap();
    assert_eq!(style_object.dxf_name, "AECC_PROFILE_VIEW_STYLE");
    let settings = style_object.profile_view_style_settings().unwrap();
    // The written style's text-style keys name the drawing's "Standard"
    // text style; the default struct carries 0.
    let standard = back.text_styles.get("Standard").unwrap().handle.value();
    let mut want = ProfileViewStyleSettings { vertical_exaggeration: 5.0, ..ProfileViewStyleSettings::default() };
    want.title.text_style = standard;
    for axis in [&mut want.left, &mut want.right, &mut want.top, &mut want.bottom] {
        axis.title.text_style = standard;
        axis.major.text_style = standard;
        axis.minor.text_style = standard;
        if let Some(g) = &mut axis.geometry {
            g.text_style = standard;
        }
    }
    assert_eq!(settings, want, "the default style but for the exaggeration and the text style");
    // The view drawn with it: its vertical scale is the style's.
    assert_eq!(entity(&back, styled).profile_view_frame().unwrap().vertical_scale, 5.0);
    assert_eq!(refs(&entity(&back, styled)).get(2), Some(&style.value()));
    // The default-style view keeps scale 10.
    assert_eq!(entity(&back, view).profile_view_frame().unwrap().vertical_scale, 10.0);

    // The default profile label set gives each profile four label groups
    // per view: lines, PVIs, sag and crest curves (§3h). Both views
    // written here carry their profile's groups.
    let groups: Vec<(String, ArchEntityData)> = back
        .entities()
        .filter_map(|e| {
            let EntityType::Extended(e) = e else { return None };
            let ExtendedEntityData::Arch(d) = &e.data else { return None };
            (d.dxf_name.starts_with("AECC_VALIGNMENT_")).then(|| (d.dxf_name.clone(), d.clone()))
        })
        .collect();
    let kind = |name: &str| groups.iter().filter(|(n, _)| n == name).count();
    for name in ["AECC_VALIGNMENT_LINE_LABEL_GROUP", "AECC_VALIGNMENT_PVI_LABEL_GROUP", "AECC_VALIGNMENT_SAGCURVE_LABEL_GROUP", "AECC_VALIGNMENT_CRESTCURVE_LABEL_GROUP"] {
        assert_eq!(kind(name), 2, "{name} for each view's profile");
    }
    // The default-style view's groups (references: soft 0, soft 0,
    // profile, label style, view — §3h).
    let of_view = |name: &str| {
        groups
            .iter()
            .find(|(n, g)| n == name && refs(g).get(4) == Some(&view.value()))
            .map(|(_, g)| g.clone())
            .unwrap_or_else(|| panic!("no {name} of the view"))
    };
    // The line group labels the profile's tangents, the curve groups
    // their curves: the first curve is a crest (the grade falls across
    // it), the second a sag. The ids are the profile's entity ids.
    let ids = entity(&back, pf).profile_entity_ids();
    let tangents: Vec<i32> = ids.iter().filter(|e| !e.1).map(|e| e.0).collect();
    let curves: Vec<i32> = ids.iter().filter(|e| e.1).map(|e| e.0).collect();
    let line = of_view("AECC_VALIGNMENT_LINE_LABEL_GROUP");
    assert_eq!(line.label_entity_ids(), tangents);
    let crest = of_view("AECC_VALIGNMENT_CRESTCURVE_LABEL_GROUP");
    assert_eq!(crest.label_entity_ids(), vec![curves[0]]);
    let sag = of_view("AECC_VALIGNMENT_SAGCURVE_LABEL_GROUP");
    assert_eq!(sag.label_entity_ids(), vec![curves[1]]);
    // A label group names its profile and view.
    assert_eq!(refs(&line).get(2), Some(&pf.value()));
    assert_eq!(refs(&line).get(4), Some(&view.value()));
    let _ = al;
}

/// A view with bands: the written band kinds reopen with their label
/// groups, a band that hides its labels has none, and a sectional data
/// band names its sample line group as its data source (§3h). The five
/// unwritten band kinds and the fixed intervals and stagger stay out of
/// scope (documented subset).
#[test]
fn profile_view_bands_survive() {
    let (mut doc, al, _) = drawing();
    let group = acadrust::classes::civil::add_sample_line_group(&mut doc, al, "SLG-BAND", None).unwrap();
    acadrust::classes::civil::add_sample_line(&mut doc, group, "SL-B1", 40.0, 10.0, 10.0, None).unwrap();
    let below = [BandKind::ProfileData, BandKind::VerticalGeometry, BandKind::HorizontalGeometry];
    let bands: Vec<ProfileViewBand> = below
        .iter()
        .map(|k| {
            let mut b = ProfileViewBand::new(*k);
            b.weeding = 50.0;
            b.stagger = BandStagger::Right;
            b
        })
        .collect();
    let mut top = ProfileViewBand::new(BandKind::ProfileData);
    top.top = true;
    let mut hidden = ProfileViewBand::new(BandKind::VerticalGeometry);
    hidden.top = true;
    hidden.show_labels = false;
    let mut sectional = ProfileViewBand::new(BandKind::SectionalData);
    sectional.data_source = Some(group);
    let view = add_profile_view_with_bands(
        &mut doc,
        al,
        "PV-BANDS",
        v(0.0, -300.0),
        None,
        &[bands.as_slice(), &[top, hidden, sectional]].concat(),
        None,
    )
    .unwrap();

    let back = roundtrip(&doc, "profile-view-bands");

    // The bands' label groups exist in model space; the hidden band has
    // none (the application deletes it; §3h).
    let names: Vec<String> = back
        .entities()
        .filter_map(|e| {
            let EntityType::Extended(e) = e else { return None };
            let ExtendedEntityData::Arch(d) = &e.data else { return None };
            (d.dxf_name.ends_with("BAND_LABELGROUP")).then(|| d.dxf_name.clone())
        })
        .collect();
    assert_eq!(names.iter().filter(|n| *n == "AECC_PROFILEDATABAND_LABELGROUP").count(), 2, "below and above");
    assert_eq!(names.iter().filter(|n| *n == "AECC_VERTICALGEOMETRYBAND_LABELGROUP").count(), 1, "the shown one only");
    assert_eq!(names.iter().filter(|n| *n == "AECC_HORIZONTALGEOMETRYBAND_LABELGROUP").count(), 1);
    assert_eq!(names.iter().filter(|n| *n == "AECC_SECTIONALDTABAND_LABELGROUP").count(), 1);
    // The view keeps its frame and names the view's alignment.
    let frame = entity(&back, view).profile_view_frame().unwrap();
    assert_eq!((frame.station_min, frame.station_max), (-200.0, 500.0));
    assert_eq!(frame.vertical_scale, 10.0);
    // The sectional data band's group reads the sample line group.
    let group_in_view = refs(&entity(&back, view)).iter().any(|r| *r == group.value());
    assert!(group_in_view, "the band's data source is among the view's references");
}
