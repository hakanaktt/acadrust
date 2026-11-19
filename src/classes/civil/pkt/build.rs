//! Builders for a new package's workflow: points, links, shapes, input
//! parameters and the flowchart that runs them in order, with the settings
//! the package editor writes for them.

use super::*;

/// The program the package editor names as the creator of its own
/// packages (samples K00 to K05): product, version, version number.
pub const EDITOR_PRODUCT: &str = "Autodesk Subassembly Composer";
pub const EDITOR_VERSION: &str = "ForVail";
pub const EDITOR_VERSION_NUMBER: &str = "13.8.280.0";
/// The workflow's namespace declarations the package editor writes into a
/// package with activities (samples K01 to K05; K00, without activities,
/// lacks `asa1`), in its order.
pub const EDITOR_NAMESPACES: [(&str, &str); 13] = [
    ("", "http://schemas.microsoft.com/netfx/2009/xaml/activities"),
    ("asa", "clr-namespace:Autodesk.SubassemblyComposer.API;assembly=Subassembly.API"),
    ("asa1", "clr-namespace:Autodesk.SubassemblyComposer.ActivityLibrary;assembly=Subassembly.ActivityLibrary"),
    ("asw", "clr-namespace:Autodesk.SubassemblyComposer.WorkflowEngine;assembly=Subassembly.WorkflowEngine"),
    ("av", "http://schemas.microsoft.com/winfx/2006/xaml/presentation"),
    ("mc", "http://schemas.openxmlformats.org/markup-compatibility/2006"),
    ("mva", "clr-namespace:Microsoft.VisualBasic.Activities;assembly=System.Activities"),
    ("s", "clr-namespace:System;assembly=mscorlib"),
    ("sads", "http://schemas.microsoft.com/netfx/2010/xaml/activities/debugger"),
    ("sap", "http://schemas.microsoft.com/netfx/2009/xaml/activities/presentation"),
    ("scg", "clr-namespace:System.Collections.Generic;assembly=mscorlib"),
    ("this", "clr-namespace:"),
    ("x", "http://schemas.microsoft.com/winfx/2006/xaml"),
];

impl PktPackage {
    /// A new package as the package editor makes one: `PktPackage::new`
    /// with the creator and the workflow's namespace declarations the
    /// editor writes. The workflow, parameters and description are the
    /// caller's.
    pub fn new_package(name: &str, part: &str) -> Self {
        let mut p = PktPackage::new(name, part);
        p.creator = PktCreator { product: EDITOR_PRODUCT.into(), version: EDITOR_VERSION.into(), version_number: EDITOR_VERSION_NUMBER.into() };
        p.namespaces = EDITOR_NAMESPACES.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
        p
    }
}

/// A part name (32 hexadecimal digits, a version-4 GUID as the editor
/// writes) that depends only on `seed`, so that a package written from the
/// same seed has the same bytes (and the same MD5 in a drawing).
pub fn new_part_name(seed: &str) -> String {
    let mut b = crate::io::dwg::security::md5(seed.as_bytes());
    b[6] = b[6] & 0x0F | 0x40;
    b[8] = b[8] & 0x3F | 0x80;
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn common(id: i32, display_name: String) -> PktCommon {
    PktCommon { activity_id: id, display_name, show_errors: true, hint_size: Some("200,22.4".into()), view: None }
}

fn list(c: &[&str]) -> Option<Vec<String>> {
    (!c.is_empty()).then(|| c.iter().map(|s| s.to_string()).collect())
}

fn new_point(id: i32, name: &str, from: Option<&str>, link: Option<(&str, &[&str])>, positioning: &str, arguments: Vec<PktArgument>, codes: &[&str]) -> PktActivity {
    PktActivity::Point(PktPoint {
        auxiliary: false,
        common: common(id, match link {
            Some((l, _)) => format!("{name}&{l}"),
            None => name.into(),
        }),
        name: name.into(),
        positioning: positioning.into(),
        from_point: from.map(Into::into),
        auto_link: link.is_some(),
        link_name: link.map(|l| l.0.into()),
        apply_aor: false,
        arguments,
        point_codes: list(codes),
        link_codes: link.and_then(|l| list(l.1)),
    })
}

/// A point `name` at (`delta_x`, `delta_y`) from `from` (or the origin);
/// `link` names the link drawn from `from` and its codes. The deltas are
/// expressions (`0`, `[Width]`, `[-Depth]`).
pub fn point_delta(id: i32, name: &str, from: Option<&str>, link: Option<(&str, &[&str])>, delta_x: &str, delta_y: &str, codes: &[&str]) -> PktActivity {
    let d = "x:Double";
    new_point(id, name, from, link, "DeltaXAndDeltaY", vec![PktArgument::new(d, "DeltaX1", delta_x), PktArgument::new(d, "DeltaY1", delta_y)], codes)
}

/// What a point placed by slope and offset may follow: expressions of the
/// package's targets (`[WidthTarget]`), or `None` for no target.
#[derive(Debug, Clone, Copy, Default)]
pub struct PointTargets<'a> {
    pub offset: Option<&'a str>,
    pub elevation: Option<&'a str>,
    pub superelevation: Option<&'a str>,
}

/// A point `name` at `delta_x` from `from` along `slope` (expressions);
/// `link` as for `point_delta`.
#[allow(clippy::too_many_arguments)]
pub fn point_slope(id: i32, name: &str, from: &str, link: Option<(&str, &[&str])>, slope: &str, delta_x: &str, targets: PointTargets, codes: &[&str]) -> PktActivity {
    let d = "x:Double";
    let target = |t: &str, k: &str, v: Option<&str>| PktArgument { direction: PktDirection::In, type_name: t.into(), key: Some(k.into()), value: v.map(Into::into), preserve_space: false };
    let arguments = vec![
        PktArgument::new(d, "Slope2", slope),
        PktArgument::new(d, "DeltaX2", delta_x),
        target("asw:OffsetTarget", "OffsetTarget2", targets.offset),
        target("asw:ElevationTarget", "ElevationTarget2", targets.elevation),
        target("asw:CrossSegmentType", "Superelevation2", targets.superelevation),
    ];
    new_point(id, name, Some(from), link, "SlopeAndDeltaX", arguments, codes)
}

/// A link `name` from point `start` to point `end`.
pub fn link(id: i32, name: &str, start: &str, end: &str, codes: &[&str]) -> PktActivity {
    PktActivity::Link(PktLink { auxiliary: false, common: common(id, name.into()), name: name.into(), start: start.into(), end: end.into(), apply_aor: false, enabled: true, codes: list(codes) })
}

/// A shape `name` closed by `links`.
pub fn shape(id: i32, name: &str, links: &[&str], codes: &[&str]) -> PktActivity {
    PktActivity::Shape(PktShape { common: common(id, name.into()), name: name.into(), links: links.iter().map(|s| s.to_string()).collect(), codes: list(codes) })
}

fn input(name: &str, type_name: &str, default: String, description: &str) -> PktParameter {
    PktParameter {
        name: name.into(),
        direction: PktDirection::In,
        type_name: type_name.into(),
        default: Some(default),
        attributes: vec![PktAttribute::Enabled(true), PktAttribute::DisplayName(name.into()), PktAttribute::Description(description.into())],
    }
}

/// An input distance parameter.
pub fn parameter_double(name: &str, default: f64, description: &str) -> PktParameter {
    input(name, "x:Double", default.to_string(), description)
}

/// An input grade parameter (a rise over a run, `-0.02`).
pub fn parameter_grade(name: &str, default: f64, description: &str) -> PktParameter {
    input(name, "asw:Grade", format!("[new Grade({default})]"), description)
}

/// An input slope parameter (a run over a rise, `4` for 4:1).
pub fn parameter_slope(name: &str, default: f64, description: &str) -> PktParameter {
    input(name, "asw:Slope", format!("[new Slope({default})]"), description)
}

/// The side parameter every subassembly has, defaulting to `side`.
pub fn parameter_side(side: PktSide) -> PktParameter {
    let (key, name) = match side {
        PktSide::Right => (0, "Right"),
        PktSide::Left => (1, "Left"),
    };
    PktParameter { name: "Side".into(), direction: PktDirection::In, type_name: "asw:EnumType".into(), default: Some(format!("[new EnumType({key}, \"{name}\", \"Side\")]")), attributes: vec![PktAttribute::Enabled(true)] }
}

/// A flowchart running `acts` one after the other, and its start node.
pub fn chain(acts: Vec<PktActivity>) -> (Vec<PktNode>, Option<usize>) {
    let n = acts.len();
    let nodes = acts
        .into_iter()
        .enumerate()
        .map(|(i, action)| PktNode::Step(PktStep { reference: format!("__ReferenceID{i}"), view: None, action, next: (i + 1 < n).then_some(i + 1) }))
        .collect();
    (nodes, (n > 0).then_some(0))
}
