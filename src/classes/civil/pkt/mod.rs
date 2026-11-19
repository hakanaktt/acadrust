//! Subassembly packages (`.pkt`): the package editor's archive of a
//! subassembly's workflow and its settings.
//!
//! ```text
//! [Content_Types].xml   Types > Default Extension="<ext>" ContentType=""
//!                       per part extension: atc, cfg, xaml, image, help,
//!                       pvd, emd, cdmd
//! <id>.atc   Category > ItemID, Properties (ItemName, Image), CustomData,
//!            Source, Palettes, Packages, Tools > Tool Name: ItemID,
//!            Properties (ItemName, Image [src], [Description], ToolTip,
//!            Help (HelpFile, HelpCommand, HelpData)), Source,
//!            StockToolRef, Data (AeccDbSubassembly: GeometryGenerateMode,
//!            DotNetClass Assembly="<id>.xaml" "Subassembly.<name>",
//!            Version, Params), Units; StockTools
//!            Params: per input argument <name DataType TypeInfo DisplayName
//!            Description>value[<Enum> choices]</name>
//! <id>.cfg   Configuration > CreatedWith > ProductName, Version,
//!            VersionNumber
//! <id>.xaml  the workflow (see `xaml`)
//! <id>.pvd   PreviewData > Superelevation > CrossSlopes >
//!            PreviewCrossSlope (CrossSegmentType, Slope, IsDefined);
//!            Cant > CantParams > PreviewCantParam (Name, Value)
//! <id>.emd   EnumData > EnumDatas > Groups > EnumGroup Name > Items >
//!            EnumItem (Name, DisplayName, Key); DefinedVariables > string
//! <id>.cdmd  CodeData > CodeItems (empty in every package seen)
//! image, help  as given
//! ```
//!
//! `<id>` is the package's part name, 32 hexadecimal digits.

mod build;
mod expr;
mod layout;
mod xaml;
mod xml;
mod zip;

pub use build::{EDITOR_NAMESPACES, EDITOR_PRODUCT, EDITOR_VERSION, EDITOR_VERSION_NUMBER, chain, link, new_part_name, parameter_double, parameter_grade, parameter_side, parameter_slope, point_delta, point_slope, shape, PointTargets};
pub use layout::{layout, PktLayout, PktLayoutLink, PktLayoutPoint, PktLayoutShape, PktSide, PktValue};

use crate::error::DxfError;
use xml::Element;

fn err(m: impl Into<String>) -> DxfError {
    DxfError::Parse(format!("package: {}", m.into()))
}

/// Direction of a workflow argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PktDirection {
    In,
    Out,
    InOut,
}

/// A declared argument of the package: an input or output parameter, or
/// a target (types `asw:OffsetTarget`, `asw:ElevationTarget`,
/// `asw:SurfaceTarget`).
#[derive(Debug, Clone, PartialEq)]
pub struct PktParameter {
    pub name: String,
    pub direction: PktDirection,
    /// The argument's type as the workflow writes it (`x:Double`,
    /// `asw:Grade`, `asw:Slope`, `x:String`, `asw:EnumType`, a target type).
    pub type_name: String,
    /// The default as the workflow writes it: a number, a text, or an
    /// expression in brackets (`[new Grade(-0.02)]`, `[new EnumType(0,
    /// "Right", "Side")]`, `[new PreviewOffsetTarget("T", False, 4)]`).
    pub default: Option<String>,
    pub attributes: Vec<PktAttribute>,
}

/// The editor's settings of an argument, in the order written.
#[derive(Debug, Clone, PartialEq)]
pub enum PktAttribute {
    Enabled(bool),
    DisplayName(String),
    Description(String),
}

impl PktParameter {
    pub fn is_target(&self) -> bool {
        self.type_name.ends_with("Target")
    }

    pub fn display_name(&self) -> &str {
        self.attributes.iter().find_map(|a| if let PktAttribute::DisplayName(s) = a { Some(s.as_str()) } else { None }).unwrap_or(&self.name)
    }

    pub fn description(&self) -> &str {
        self.attributes.iter().find_map(|a| if let PktAttribute::Description(s) = a { Some(s.as_str()) } else { None }).unwrap_or_else(|| self.display_name())
    }
}

/// A target argument (layout mode gives it its default).
pub type PktTarget = PktParameter;

/// A variable of the flowchart.
#[derive(Debug, Clone, PartialEq)]
pub struct PktVariable {
    pub type_name: String,
    pub name: String,
    pub default: Option<String>,
    pub read_only: bool,
}

/// An enumeration of the package: its items (name, display name, key).
#[derive(Debug, Clone, PartialEq)]
pub struct PktEnumGroup {
    pub name: String,
    pub items: Vec<(String, String, i32)>,
}

/// A value of the editor's view state.
#[derive(Debug, Clone, PartialEq)]
pub enum PktViewValue {
    Boolean(bool),
    Double(String),
    Point(String),
    Size(String),
    Points(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PktViewEntry {
    pub key: String,
    pub value: PktViewValue,
}

/// Where the editor draws a node or activity (its keyed values).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PktView(pub Vec<PktViewEntry>);

/// An argument of an activity: an expression (text, or `[...]`) of a
/// type, keyed by the activity's argument name.
#[derive(Debug, Clone, PartialEq)]
pub struct PktArgument {
    pub direction: PktDirection,
    pub type_name: String,
    pub key: Option<String>,
    /// `None` for an unset (empty) argument.
    pub value: Option<String>,
    /// The expression keeps its line breaks (`xml:space="preserve"`).
    pub preserve_space: bool,
}

impl PktArgument {
    pub fn new(type_name: &str, key: &str, value: &str) -> Self {
        PktArgument { direction: PktDirection::In, type_name: type_name.into(), key: Some(key.into()), value: Some(value.into()), preserve_space: false }
    }
}

/// What every activity carries.
#[derive(Debug, Clone, PartialEq)]
pub struct PktCommon {
    pub activity_id: i32,
    pub display_name: String,
    pub show_errors: bool,
    pub hint_size: Option<String>,
    pub view: Option<PktView>,
}

/// A point (or auxiliary point): placed by `positioning` with its
/// arguments, optionally linked from `from_point`.
#[derive(Debug, Clone, PartialEq)]
pub struct PktPoint {
    pub auxiliary: bool,
    pub common: PktCommon,
    pub name: String,
    /// `DeltaXAndDeltaY`, `SlopeAndDeltaX`, `SlopeAndDeltaY`,
    /// `AngleAndDeltaX`, ... (the arguments are keyed with its number).
    pub positioning: String,
    pub from_point: Option<String>,
    pub auto_link: bool,
    pub link_name: Option<String>,
    pub apply_aor: bool,
    pub arguments: Vec<PktArgument>,
    pub point_codes: Option<Vec<String>>,
    pub link_codes: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PktLink {
    pub auxiliary: bool,
    pub common: PktCommon,
    pub name: String,
    pub start: String,
    pub end: String,
    pub apply_aor: bool,
    pub enabled: bool,
    pub codes: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PktShape {
    pub common: PktCommon,
    pub name: String,
    pub links: Vec<String>,
    pub codes: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PktSequence {
    pub display_name: Option<String>,
    pub hint_size: Option<String>,
    pub view: Option<PktView>,
    pub activities: Vec<PktActivity>,
}

/// A defined variable: `name` of `type_name` (`Double`, `String`, ...)
/// starting at `value`; `target` is the `[name]` it binds.
#[derive(Debug, Clone, PartialEq)]
pub struct PktDefineVariable {
    pub common: PktCommon,
    pub name: String,
    pub old_name: String,
    pub type_name: String,
    pub value: PktArgument,
    pub target: PktArgument,
}

/// `target = value` (a variable or an output parameter).
#[derive(Debug, Clone, PartialEq)]
pub struct PktAssignment {
    pub common: PktCommon,
    pub target: PktArgument,
    pub value: PktArgument,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PktActivity {
    Point(PktPoint),
    Link(PktLink),
    Shape(PktShape),
    Sequence(PktSequence),
    DefineVariable(PktDefineVariable),
    SetVariable(PktAssignment),
    SetOutput(PktAssignment),
}

/// A flowchart node running `action`, then `next`.
#[derive(Debug, Clone, PartialEq)]
pub struct PktStep {
    pub reference: String,
    pub view: Option<PktView>,
    pub action: PktActivity,
    pub next: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PktDecision {
    pub reference: String,
    pub condition: String,
    pub display_name: Option<String>,
    pub hint_size: Option<String>,
    pub view: Option<PktView>,
    pub on_true: Option<usize>,
    pub on_false: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PktSwitch {
    pub reference: String,
    /// The type of the expression and case keys (`x:String`).
    pub type_name: String,
    pub expression: String,
    pub display_name: Option<String>,
    pub hint_size: Option<String>,
    pub view: Option<PktView>,
    pub default: Option<usize>,
    pub cases: Vec<(String, usize)>,
}

/// A flowchart node; links are indices into the package's flowchart.
#[derive(Debug, Clone, PartialEq)]
pub enum PktNode {
    Step(PktStep),
    Decision(PktDecision),
    Switch(PktSwitch),
}

impl PktNode {
    pub fn reference(&self) -> &str {
        match self {
            PktNode::Step(s) => &s.reference,
            PktNode::Decision(d) => &d.reference,
            PktNode::Switch(s) => &s.reference,
        }
    }

    fn links(&self) -> Vec<usize> {
        match self {
            PktNode::Step(s) => s.next.into_iter().collect(),
            PktNode::Decision(d) => d.on_true.into_iter().chain(d.on_false).collect(),
            PktNode::Switch(s) => s.default.into_iter().chain(s.cases.iter().map(|c| c.1)).collect(),
        }
    }

    fn renumber(&mut self, at: &[usize]) {
        let f = |x: &mut Option<usize>| *x = x.map(|i| at[i]);
        match self {
            PktNode::Step(s) => f(&mut s.next),
            PktNode::Decision(d) => {
                f(&mut d.on_true);
                f(&mut d.on_false);
            }
            PktNode::Switch(s) => {
                f(&mut s.default);
                for c in &mut s.cases {
                    c.1 = at[c.1];
                }
            }
        }
    }
}

/// The program that wrote the package (its configuration part).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PktCreator {
    pub product: String,
    pub version: String,
    pub version_number: String,
}

/// The preview settings: cross slopes per segment (type, slope, defined)
/// and the cant values (name, value).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PktPreview {
    pub cross_slopes: Vec<(String, String, bool)>,
    pub cant: Vec<(String, String)>,
}

/// A subassembly package.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PktPackage {
    /// The subassembly's name (tool name; the script is `Subassembly.<name>`).
    pub name: String,
    pub description: String,
    /// `Major.Minor`.
    pub version: String,
    /// The part name of the workflow and settings (32 hex digits).
    pub part: String,
    /// The catalog's and the tool's identifiers (`{...}`).
    pub category_id: String,
    pub tool_id: String,
    /// The identifier the tool refers to as its kind.
    pub stock_tool_id: String,
    pub units: String,
    pub creator: PktCreator,
    /// The preview image: file name and PNG bytes.
    pub preview_png_name: String,
    pub preview_png: Option<Vec<u8>>,
    /// The help file: name and bytes.
    pub help_name: String,
    pub help: Option<Vec<u8>>,
    pub help_command: String,
    pub help_data: String,
    /// Arguments in declaration order (input and output parameters and
    /// targets).
    pub parameters: Vec<PktParameter>,
    pub enum_groups: Vec<PktEnumGroup>,
    /// Names of the variables the flowchart defines.
    pub defined_variables: Vec<String>,
    pub variables: Vec<PktVariable>,
    pub flowchart: Vec<PktNode>,
    pub start: Option<usize>,
    pub preview: PktPreview,
    /// The workflow's namespace declarations (prefix, URI), in order.
    pub namespaces: Vec<(String, String)>,
    pub ignorable: String,
    pub workflow_hint_size: Option<String>,
    pub vb_settings: Option<String>,
    pub flowchart_hint_size: Option<String>,
    pub flowchart_vb_settings: Option<String>,
    pub flowchart_view: Option<PktView>,
}

/// The kind every package's tool refers to (all 67 stock packages and
/// every package the editor saved for the samples).
pub const STOCK_TOOL_ID: &str = "{7F55AAC0-0256-48D7-BFA5-914702663FDE}";
/// The read-only enumeration values the editor declares in every
/// flowchart (name, key), in its order.
const BUILT_IN_VALUES: [(&str, i32); 17] = [
    ("AwayFromCrown", 41), ("Left", 1), ("LeftInsideLane", 20), ("LeftInsideShoulder", 21), ("LeftOutsideLane", 22), ("LeftOutsideShoulder", 23), ("No", 11), ("None", -1), ("Right", 0),
    ("RightInsideLane", 24), ("RightInsideShoulder", 25), ("RightOutsideLane", 26), ("RightOutsideShoulder", 27), ("Supported", 30), ("TowardsCrown", 40), ("Unsupported", 31), ("Yes", 10),
];
/// The preview cross slopes of a new package (K00): lanes -0.02,
/// shoulders -0.05.
const PREVIEW_SEGMENTS: [&str; 8] = ["LeftInsideLane", "LeftInsideShoulder", "LeftOutsideLane", "LeftOutsideShoulder", "RightInsideLane", "RightInsideShoulder", "RightOutsideLane", "RightOutsideShoulder"];
const PREVIEW_CANT: [(&str, &str); 5] = [("CantPivotType", "CenterLine"), ("LeftRail", ""), ("LeftRailDeltaElevation", "0"), ("RightRail", ""), ("RightRailDeltaElevation", "0")];
const VB_SETTINGS: &str = "Assembly references and imported namespaces serialized as XML namespaces";

fn guid(hex: &str) -> String {
    format!("{{{}-{}-{}-{}-{}}}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32])
}

impl PktPackage {
    /// An empty package named `name` with the parts named `part` (32
    /// hexadecimal digits), the editor's settings of a new package and
    /// its built-in values. The creator and the workflow's namespaces are
    /// left for the caller (copy them from a package the editor saved).
    pub fn new(name: &str, part: &str) -> Self {
        let part = part.to_ascii_lowercase();
        let reversed: String = part.chars().rev().collect();
        let ok = part.len() == 32 && part.bytes().all(|b| b.is_ascii_hexdigit());
        PktPackage {
            name: name.into(),
            version: "0.1".into(),
            category_id: if ok { guid(&reversed) } else { String::new() },
            tool_id: if ok { guid(&part) } else { String::new() },
            part,
            stock_tool_id: STOCK_TOOL_ID.into(),
            units: "m".into(),
            variables: BUILT_IN_VALUES.iter().map(|(n, k)| PktVariable { type_name: "asw:EnumType".into(), name: n.to_string(), default: Some(format!("[new EnumType({k}, \"{n}\")]")), read_only: true }).collect(),
            preview: PktPreview {
                cross_slopes: PREVIEW_SEGMENTS.iter().map(|s| (s.to_string(), if s.ends_with("Lane") { "-0.02" } else { "-0.05" }.to_string(), true)).collect(),
                cant: PREVIEW_CANT.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
            },
            ignorable: "sads sap".into(),
            workflow_hint_size: Some("654.4,676.8".into()),
            vb_settings: Some(VB_SETTINGS.into()),
            flowchart_hint_size: Some("614.4,636.8".into()),
            flowchart_vb_settings: Some(VB_SETTINGS.into()),
            flowchart_view: Some(PktView(vec![
                PktViewEntry { key: "IsExpanded".into(), value: PktViewValue::Boolean(false) },
                PktViewEntry { key: "ShapeLocation".into(), value: PktViewValue::Point("270,2.5".into()) },
                PktViewEntry { key: "ShapeSize".into(), value: PktViewValue::Size("60,75.2".into()) },
            ])),
            ..Default::default()
        }
    }

    /// The target arguments.
    pub fn targets(&self) -> impl Iterator<Item = &PktTarget> {
        self.parameters.iter().filter(|p| p.is_target())
    }
}

// ---- settings parts ----

const XSD: &str = "http://www.w3.org/2001/XMLSchema";
const XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";
const TYPES: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
/// The catalog's own name and image size (every package seen).
const CATEGORY_NAME: &str = "Category1";
const CATEGORY_IMAGE: (&str, &str) = ("93", "123");
const TOOL_IMAGE: (&str, &str) = ("64", "64");
/// How the host builds the subassembly (every package seen).
const GENERATE_MODE: &str = "UseDotNet";

fn serializer(name: &str) -> Element {
    Element::new(name).attr("xmlns:xsd", XSD).attr("xmlns:xsi", XSI)
}

fn text(e: &Element, path: &[&str]) -> Result<String, DxfError> {
    let mut x = e;
    for p in path {
        x = x.first(p).ok_or_else(|| err(format!("{} lacks {p}", x.name)))?;
    }
    Ok(x.inner_text())
}

fn attr<'a>(e: &'a Element, name: &str) -> Result<&'a str, DxfError> {
    e.get(name).ok_or_else(|| err(format!("{} lacks {name}", e.name)))
}

fn leaf(name: &str, t: &str) -> Element {
    if t.is_empty() {
        Element::new(name)
    } else {
        Element::new(name).text(t)
    }
}

/// `[new EnumType(key, "name", "group")]` → (key, name, group).
pub(crate) fn enum_default(d: &str) -> Option<(i32, String, Option<String>)> {
    let inner = d.strip_prefix("[new EnumType(")?.strip_suffix(")]")?;
    let mut parts = inner.splitn(2, ',');
    let key = parts.next()?.trim().parse().ok()?;
    let rest: Vec<String> = parts.next()?.split(',').map(|s| s.trim().trim_matches('"').to_string()).collect();
    Some((key, rest.first()?.clone(), rest.get(1).cloned()))
}

/// The number inside `[new Grade(x)]` / `[new Slope(x)]`, or the text.
pub(crate) fn number_default(d: &str) -> &str {
    d.strip_prefix('[').and_then(|s| s.find('(').map(|i| &s[i + 1..])).and_then(|s| s.strip_suffix(")]")).unwrap_or(d)
}

/// The side choices the host defines (the workflow's `Side` group).
const SIDE_GROUP: [(&str, i32); 2] = [("Left", 1), ("Right", 0)];

fn params_element(pkg: &PktPackage) -> Result<Element, DxfError> {
    let mut ps = Element::new("Params");
    for p in pkg.parameters.iter().filter(|p| p.direction == PktDirection::In && !p.is_target()) {
        let d = p.default.clone().unwrap_or_default();
        // A subassembly without a side (Side = None, as in a new package)
        // lists no side.
        if p.name == "Side" && enum_default(&d).is_some_and(|e| e.0 == -1) {
            continue;
        }
        let (data, info) = match p.type_name.as_str() {
            "x:Double" => ("double", 16),
            "asw:Grade" => ("double", 9),
            "asw:Slope" => ("double", 10),
            "x:String" => ("string", 16),
            "asw:EnumType" => ("long", 16),
            t => return Err(err(format!("parameter {} of type {t}", p.name))),
        };
        let mut e = Element::new(&p.name).attr("DataType", data).attr("TypeInfo", &info.to_string()).attr("DisplayName", p.display_name()).attr("Description", p.description());
        if p.type_name == "asw:EnumType" {
            let (key, _, group) = enum_default(&d).ok_or_else(|| err(format!("enumeration default of {}", p.name)))?;
            let group = group.ok_or_else(|| err(format!("{} names no enumeration", p.name)))?;
            let items: Vec<(String, String, i32)> = if group == "Side" {
                SIDE_GROUP.iter().map(|(n, k)| (n.to_string(), n.to_string(), *k)).collect()
            } else {
                pkg.enum_groups.iter().find(|g| g.name == group).ok_or_else(|| err(format!("enumeration {group}")))?.items.clone()
            };
            let mut en = Element::new("Enum");
            for (n, dn, k) in items {
                en = en.child(Element::new(&n).attr("DisplayName", &dn).text(&k.to_string()));
            }
            e = e.text(&key.to_string()).child(en);
        } else {
            e = leaf_text(e, number_default(&d));
        }
        ps = ps.child(e);
    }
    Ok(ps)
}

fn leaf_text(e: Element, t: &str) -> Element {
    if t.is_empty() {
        e
    } else {
        e.text(t)
    }
}

fn tool_tip(pkg: &PktPackage) -> String {
    // The editor joins the description and the version with the two
    // characters `\n`.
    if pkg.description.is_empty() {
        format!("Version: {}", pkg.version)
    } else {
        format!("{}\\nVersion: {}", pkg.description, pkg.version)
    }
}

fn atc_element(pkg: &PktPackage) -> Result<Element, DxfError> {
    let mut image = Element::new("Image").attr("cx", TOOL_IMAGE.0).attr("cy", TOOL_IMAGE.1);
    if pkg.preview_png.is_some() {
        image = image.attr("src", &pkg.preview_png_name);
    }
    let mut props = Element::new("Properties").child(leaf("ItemName", &pkg.name)).child(Element::new("Images").child(image));
    if !pkg.description.is_empty() {
        props = props.child(leaf("Description", &pkg.description));
    }
    let help_file = if pkg.help.is_some() { pkg.help_name.as_str() } else { "" };
    props = props.child(leaf("ToolTip", &tool_tip(pkg))).child(Element::new("Help").child(leaf("HelpFile", help_file)).child(leaf("HelpCommand", &pkg.help_command)).child(leaf("HelpData", &pkg.help_data)));
    let data = Element::new("Data")
        .child(
            Element::new("AeccDbSubassembly")
                .child(leaf("GeometryGenerateMode", GENERATE_MODE))
                .child(Element::new("DotNetClass").attr("Assembly", &format!("{}.xaml", pkg.part)).text(&format!("Subassembly.{}", pkg.name)))
                .child(leaf("Version", &pkg.version))
                .child(params_element(pkg)?),
        )
        .child(leaf("Units", &pkg.units));
    let tool = Element::new("Tool")
        .attr("Name", &pkg.name)
        .child(Element::new("ItemID").attr("idValue", &pkg.tool_id))
        .child(props)
        .child(Element::new("Source"))
        .child(Element::new("StockToolRef").attr("idValue", &pkg.stock_tool_id))
        .child(data);
    Ok(serializer("Category")
        .child(Element::new("ItemID").attr("idValue", &pkg.category_id))
        .child(Element::new("Properties").child(leaf("ItemName", CATEGORY_NAME)).child(Element::new("Images").child(Element::new("Image").attr("cx", CATEGORY_IMAGE.0).attr("cy", CATEGORY_IMAGE.1))))
        .child(Element::new("CustomData"))
        .child(Element::new("Source"))
        .child(Element::new("Palettes"))
        .child(Element::new("Packages"))
        .child(Element::new("Tools").child(tool))
        .child(Element::new("StockTools")))
}

fn read_atc(e: &Element, pkg: &mut PktPackage) -> Result<(), DxfError> {
    pkg.category_id = attr(e.first("ItemID").ok_or_else(|| err("catalog id"))?, "idValue")?.to_string();
    let tools: Vec<&Element> = e.first("Tools").map(|t| t.elements().collect()).unwrap_or_default();
    let [tool] = tools.as_slice() else { return Err(err("a package holds one tool")) };
    pkg.name = attr(tool, "Name")?.to_string();
    pkg.tool_id = attr(tool.first("ItemID").ok_or_else(|| err("tool id"))?, "idValue")?.to_string();
    pkg.stock_tool_id = attr(tool.first("StockToolRef").ok_or_else(|| err("tool kind"))?, "idValue")?.to_string();
    let props = tool.first("Properties").ok_or_else(|| err("tool properties"))?;
    pkg.description = props.first("Description").map(|d| d.inner_text()).unwrap_or_default();
    if let Some(src) = props.first("Images").and_then(|i| i.first("Image")).and_then(|i| i.get("src")) {
        pkg.preview_png_name = src.to_string();
    }
    pkg.help_name = text(props, &["Help", "HelpFile"])?;
    pkg.help_command = text(props, &["Help", "HelpCommand"])?;
    pkg.help_data = text(props, &["Help", "HelpData"])?;
    let data = tool.first("Data").ok_or_else(|| err("tool data"))?;
    pkg.version = text(data, &["AeccDbSubassembly", "Version"])?;
    pkg.units = text(data, &["Units"])?;
    let class = data.first("AeccDbSubassembly").and_then(|a| a.first("DotNetClass")).ok_or_else(|| err("script"))?;
    pkg.part = attr(class, "Assembly")?.strip_suffix(".xaml").ok_or_else(|| err("script part"))?.to_string();
    Ok(())
}

fn cfg_element(pkg: &PktPackage) -> Element {
    let c = &pkg.creator;
    serializer("Configuration").child(Element::new("CreatedWith").child(leaf("ProductName", &c.product)).child(leaf("Version", &c.version)).child(leaf("VersionNumber", &c.version_number)))
}

fn pvd_element(pkg: &PktPackage) -> Element {
    let mut cs = Element::new("CrossSlopes");
    for (t, s, d) in &pkg.preview.cross_slopes {
        cs = cs.child(Element::new("PreviewCrossSlope").attr("CrossSegmentType", t).attr("Slope", s).attr("IsDefined", if *d { "true" } else { "false" }));
    }
    let mut cp = Element::new("CantParams");
    for (n, v) in &pkg.preview.cant {
        cp = cp.child(Element::new("PreviewCantParam").attr("Name", n).attr("Value", v));
    }
    serializer("PreviewData").child(Element::new("Superelevation").child(cs)).child(Element::new("Cant").child(cp))
}

fn emd_element(pkg: &PktPackage) -> Element {
    let mut gs = Element::new("Groups");
    for g in &pkg.enum_groups {
        let mut items = Element::new("Items");
        for (n, d, k) in &g.items {
            items = items.child(Element::new("EnumItem").attr("Name", n).attr("DisplayName", d).attr("Key", &k.to_string()));
        }
        gs = gs.child(Element::new("EnumGroup").attr("Name", &g.name).child(items));
    }
    let mut dv = Element::new("DefinedVariables");
    for v in &pkg.defined_variables {
        dv = dv.child(Element::new("string").text(v));
    }
    serializer("EnumData").child(Element::new("EnumDatas").child(gs).child(dv))
}

fn cdmd_element() -> Element {
    serializer("CodeData").child(Element::new("CodeItems"))
}

fn extension(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or("")
}

fn content_types(pkg: &PktPackage) -> String {
    let mut exts = vec!["atc", "cfg", "xaml"];
    if pkg.preview_png.is_some() {
        exts.push(extension(&pkg.preview_png_name));
    }
    if pkg.help.is_some() {
        exts.push(extension(&pkg.help_name));
    }
    exts.extend(["pvd", "emd", "cdmd"]);
    let mut seen = Vec::new();
    let mut out = format!("\u{feff}<?xml version=\"1.0\" encoding=\"utf-8\"?><Types xmlns=\"{TYPES}\">");
    for x in exts {
        if !seen.contains(&x) {
            seen.push(x);
            out.push_str(&format!("<Default Extension=\"{x}\" ContentType=\"\" />"));
        }
    }
    out.push_str("</Types>");
    out
}

/// The parts of `pkg` as the package holds them: (name, document element
/// or bytes).
fn parts(pkg: &PktPackage) -> Result<Vec<(String, Vec<u8>)>, DxfError> {
    if pkg.name.is_empty() || pkg.part.len() != 32 || !pkg.part.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(err("a package needs a name and a 32-digit part name"));
    }
    let doc = |e: Element| xml::document(&e).into_bytes();
    let p = &pkg.part;
    let mut out = vec![
        (format!("{p}.atc"), doc(atc_element(pkg)?)),
        ("[Content_Types].xml".to_string(), content_types(pkg).into_bytes()),
        (format!("{p}.cfg"), doc(cfg_element(pkg))),
        (format!("{p}.xaml"), xaml::write(pkg)?.into_bytes()),
        (format!("{p}.pvd"), doc(pvd_element(pkg))),
        (format!("{p}.emd"), doc(emd_element(pkg))),
        (format!("{p}.cdmd"), doc(cdmd_element())),
    ];
    if let Some(png) = &pkg.preview_png {
        out.push((pkg.preview_png_name.clone(), png.clone()));
    }
    if let Some(help) = &pkg.help {
        out.push((pkg.help_name.clone(), help.clone()));
    }
    Ok(out)
}

/// Writes `pkg` as a package archive.
pub fn write_pkt(pkg: &PktPackage) -> Result<Vec<u8>, DxfError> {
    zip::write(&parts(pkg)?)
}

/// Reads a package archive. A package whose tool parameters, content
/// types or code part do not follow from its workflow and settings (none
/// the package editor writes) is refused.
pub fn read_pkt(bytes: &[u8]) -> Result<PktPackage, DxfError> {
    let archive = zip::read(bytes)?;
    let get = |n: &str| archive.iter().find(|p| p.0 == n).map(|p| p.1.as_slice());
    let doc = |n: &str| -> Result<Element, DxfError> {
        let b = get(n).ok_or_else(|| err(format!("no part {n}")))?;
        xml::parse(std::str::from_utf8(b).map_err(|_| err(format!("{n} is not UTF-8")))?).map_err(|m| err(format!("{n}: {m}")))
    };
    let atc_name = archive.iter().find(|p| p.0.ends_with(".atc")).ok_or_else(|| err("no tool catalog"))?.0.clone();
    let mut pkg = PktPackage::default();
    let atc = doc(&atc_name)?;
    read_atc(&atc, &mut pkg)?;
    let p = pkg.part.clone();
    let cfg = doc(&format!("{p}.cfg"))?;
    pkg.creator = PktCreator { product: text(&cfg, &["CreatedWith", "ProductName"])?, version: text(&cfg, &["CreatedWith", "Version"])?, version_number: text(&cfg, &["CreatedWith", "VersionNumber"])? };
    let pvd = doc(&format!("{p}.pvd"))?;
    for c in pvd.first("Superelevation").and_then(|s| s.first("CrossSlopes")).map(|c| c.elements().collect::<Vec<_>>()).unwrap_or_default() {
        pkg.preview.cross_slopes.push((attr(c, "CrossSegmentType")?.into(), attr(c, "Slope")?.into(), attr(c, "IsDefined")? == "true"));
    }
    for c in pvd.first("Cant").and_then(|s| s.first("CantParams")).map(|c| c.elements().collect::<Vec<_>>()).unwrap_or_default() {
        pkg.preview.cant.push((attr(c, "Name")?.into(), attr(c, "Value")?.into()));
    }
    let emd = doc(&format!("{p}.emd"))?;
    let datas = emd.first("EnumDatas").ok_or_else(|| err("enumeration data"))?;
    for g in datas.first("Groups").map(|g| g.elements().collect::<Vec<_>>()).unwrap_or_default() {
        let mut items = Vec::new();
        for i in g.first("Items").map(|i| i.elements().collect::<Vec<_>>()).unwrap_or_default() {
            items.push((attr(i, "Name")?.to_string(), attr(i, "DisplayName")?.to_string(), attr(i, "Key")?.parse().map_err(|_| err("enumeration key"))?));
        }
        pkg.enum_groups.push(PktEnumGroup { name: attr(g, "Name")?.to_string(), items });
    }
    pkg.defined_variables = datas.first("DefinedVariables").map(|d| d.elements().map(|s| s.inner_text()).collect()).unwrap_or_default();
    let xaml_text = std::str::from_utf8(get(&format!("{p}.xaml")).ok_or_else(|| err("no workflow"))?).map_err(|_| err("workflow is not UTF-8"))?;
    xaml::read(xaml_text, &mut pkg)?;
    if !pkg.preview_png_name.is_empty() {
        pkg.preview_png = Some(get(&pkg.preview_png_name).ok_or_else(|| err("no preview image part"))?.to_vec());
    }
    if !pkg.help_name.is_empty() {
        pkg.help = Some(get(&pkg.help_name).ok_or_else(|| err("no help part"))?.to_vec());
    }
    // what the settings derive from the workflow must be what was stored
    if atc_element(&pkg)?.normalized() != atc.normalized() {
        return Err(err("the tool catalog does not follow from the package"));
    }
    if doc(&format!("{p}.cdmd"))?.normalized() != cdmd_element().normalized() {
        return Err(err("code data is not supported"));
    }
    let types = xml::parse(&content_types(&pkg)).map_err(err)?.normalized();
    if types != doc("[Content_Types].xml")?.normalized() {
        return Err(err("content types do not follow from the parts"));
    }
    if archive.len() != parts(&pkg)?.len() {
        return Err(err("the package holds parts that are not supported"));
    }
    Ok(pkg)
}

/// Whether two packages have equal parts: XML parts equal as documents
/// (attribute order and layout whitespace aside), other parts equal bytes.
pub fn same_parts(a: &[u8], b: &[u8]) -> Result<bool, DxfError> {
    let (mut a, mut b) = (zip::read(a)?, zip::read(b)?);
    a.sort();
    b.sort();
    if a.len() != b.len() {
        return Ok(false);
    }
    for ((na, da), (nb, db)) in a.iter().zip(&b) {
        if na != nb {
            return Ok(false);
        }
        let as_xml = |d: &[u8]| std::str::from_utf8(d).ok().and_then(|t| xml::parse(t).ok()).map(|e| e.normalized());
        let same = match (as_xml(da), as_xml(db)) {
            (Some(x), Some(y)) if !na.ends_with(".png") => x == y,
            _ => da == db,
        };
        if !same {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
pub(crate) mod tests;
