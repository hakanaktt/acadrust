//! The workflow part (`<id>.xaml`): the package's arguments, variables and
//! flowchart.
//!
//! ```text
//! Activity  mc:Ignorable, x:Class="Subassembly", this:Subassembly.<arg>=
//!           default per input argument, xmlns:* namespace declarations
//!   x:Members: Geometry, SubassemblyErrorCenter, SubassemblyRunMode
//!     (InOutArgument), then each argument: x:Property Name Type="In|Out
//!     Argument(type)" with x:Property.Attributes (EnabledFlag2Attribute,
//!     DisplayName2Attribute, Description2Attribute, in the order given)
//!   sap:VirtualizedContainerService.HintSize (text)
//!   mva:VisualBasic.Settings (text)
//!   Flowchart [HintSize] [mva:VisualBasic.Settings]
//!     Flowchart.Variables: Variable x:TypeArguments [Default] [Modifiers] Name
//!     view state
//!     Flowchart.StartNode: the first node, nested along its links, or x:Null
//!     x:Reference per node, in the flowchart's node order
//! node: FlowStep x:Name (view state, action, FlowStep.Next)
//!       FlowDecision x:Name Condition [HintSize] (view state,
//!         FlowDecision.True, FlowDecision.False)
//!       FlowSwitch x:TypeArguments x:Name Expression [HintSize]
//!         (FlowSwitch.Default, view state, cases: node with x:Key)
//!   a link to a node written before is `x:Reference` (with x:Key in a
//!   switch)
//! view state: sap:WorkflowViewStateService.ViewState > scg:Dictionary
//!   x:TypeArguments="x:String, x:Object" > typed values with x:Key
//! ```

use super::xml::{Element, Node};
use super::*;

const HINT: &str = "sap:VirtualizedContainerService.HintSize";
const VIEW: &str = "sap:WorkflowViewStateService.ViewState";
const VB: &str = "mva:VisualBasic.Settings";
/// The bindings every geometry activity carries to the workflow's own
/// arguments (the argument names the workflow declares first).
const GEOMETRY: &str = "[Geometry]";
const ERRORS: &str = "[SubassemblyErrorCenter]";
const RUN_MODE: &str = "[SubassemblyRunMode]";
/// The workflow's own arguments, declared before the package's.
const RUN_ARGUMENTS: [(&str, &str); 3] = [("Geometry", "Geometry"), ("SubassemblyErrorCenter", "SubassemblyErrorCenter"), ("SubassemblyRunMode", "SubassemblyRunMode")];

fn err(m: impl Into<String>) -> DxfError {
    DxfError::Parse(format!("package workflow: {}", m.into()))
}

fn req<'a>(e: &'a Element, name: &str) -> Result<&'a str, DxfError> {
    e.get(name).ok_or_else(|| err(format!("{} lacks {name}", e.name)))
}

fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn bool_attr(e: &Element, name: &str) -> Result<bool, DxfError> {
    match req(e, name)? {
        "True" => Ok(true),
        "False" => Ok(false),
        v => Err(err(format!("{name}={v} is not a boolean"))),
    }
}

fn tf(v: bool) -> &'static str {
    if v {
        "True"
    } else {
        "False"
    }
}

fn opt(e: &Element, name: &str) -> Option<String> {
    e.get(name).filter(|v| *v != "{x:Null}").map(str::to_string)
}

// ---- view state ----

fn read_view(e: &Element) -> Result<Option<PktView>, DxfError> {
    let Some(v) = e.first(VIEW) else { return Ok(None) };
    let d = v.first("scg:Dictionary").ok_or_else(|| err("view state without a dictionary"))?;
    let mut entries = Vec::new();
    for x in d.elements() {
        let key = req(x, "x:Key")?.to_string();
        let t = x.inner_text();
        let value = match x.name.as_str() {
            "x:Boolean" => PktViewValue::Boolean(t == "True"),
            "x:Double" => PktViewValue::Double(t),
            "av:Point" => PktViewValue::Point(t),
            "av:Size" => PktViewValue::Size(t),
            "av:PointCollection" => PktViewValue::Points(t),
            n => return Err(err(format!("view state value {n}"))),
        };
        entries.push(PktViewEntry { key, value });
    }
    Ok(Some(PktView(entries)))
}

fn view_element(v: &PktView) -> Element {
    let mut d = Element::new("scg:Dictionary").attr("x:TypeArguments", "x:String, x:Object");
    for x in &v.0 {
        let (n, t) = match &x.value {
            PktViewValue::Boolean(b) => ("x:Boolean", tf(*b).to_string()),
            PktViewValue::Double(s) => ("x:Double", s.clone()),
            PktViewValue::Point(s) => ("av:Point", s.clone()),
            PktViewValue::Size(s) => ("av:Size", s.clone()),
            PktViewValue::Points(s) => ("av:PointCollection", s.clone()),
        };
        d = d.child(Element::new(n).attr("x:Key", &x.key).text(&t));
    }
    Element::new(VIEW).child(d)
}

fn push_view(e: &mut Element, v: &Option<PktView>) {
    if let Some(v) = v {
        e.children.push(Node::Element(view_element(v)));
    }
}

// ---- arguments and code lists ----

fn read_argument(x: &Element) -> Result<PktArgument, DxfError> {
    let direction = match x.name.as_str() {
        "InArgument" => PktDirection::In,
        "OutArgument" => PktDirection::Out,
        n => return Err(err(format!("argument {n}"))),
    };
    let text = x.inner_text();
    Ok(PktArgument {
        direction,
        type_name: req(x, "x:TypeArguments")?.to_string(),
        key: x.get("x:Key").map(str::to_string),
        value: (!x.children.is_empty()).then_some(text),
        preserve_space: x.get("xml:space") == Some("preserve"),
    })
}

fn argument_element(a: &PktArgument) -> Element {
    let mut e = Element::new(if a.direction == PktDirection::Out { "OutArgument" } else { "InArgument" }).attr("x:TypeArguments", &a.type_name);
    if let Some(k) = &a.key {
        e = e.attr("x:Key", k);
    }
    if a.preserve_space {
        e = e.attr("xml:space", "preserve");
    }
    if let Some(v) = &a.value {
        e = e.text(v);
    }
    e
}

/// `scg:List x:TypeArguments="InArgument(x:String)" Capacity` of codes.
fn read_codes(x: &Element) -> Result<Vec<String>, DxfError> {
    let list = x.first("scg:List").ok_or_else(|| err("code list"))?;
    Ok(list.elements().map(|i| i.inner_text()).collect())
}

/// The list's capacity as the editor's lists grow: 4, then doubled.
fn capacity(n: usize) -> usize {
    let mut c = 4;
    while c < n {
        c *= 2;
    }
    c
}

fn codes_element(name: &str, codes: &[String]) -> Element {
    let mut l = Element::new("scg:List").attr("x:TypeArguments", "InArgument(x:String)").attr("Capacity", &capacity(codes.len()).to_string());
    for c in codes {
        l = l.child(Element::new("InArgument").attr("x:TypeArguments", "x:String").text(c));
    }
    Element::new(name).child(l)
}

// ---- activities ----

fn common(e: &Element) -> Result<PktCommon, DxfError> {
    Ok(PktCommon {
        activity_id: req(e, "ActivityId")?.parse().map_err(|_| err("ActivityId"))?,
        display_name: req(e, "DisplayName")?.to_string(),
        show_errors: bool_attr(e, "ShowErrors")?,
        hint_size: e.get(HINT).map(str::to_string),
        view: read_view(e)?,
    })
}

fn read_activity(e: &Element) -> Result<PktActivity, DxfError> {
    let kind = local(&e.name);
    let prop = |p: &str| e.first(&format!("{}.{p}", e.name));
    match kind {
        "CreatePoint" | "CreateAuxPoint" => {
            let arguments = prop("Arguments").map(|a| a.elements().map(read_argument).collect::<Result<Vec<_>, _>>()).transpose()?.unwrap_or_default();
            Ok(PktActivity::Point(PktPoint {
                auxiliary: kind == "CreateAuxPoint",
                common: common(e)?,
                name: req(e, "PointNumber")?.to_string(),
                positioning: req(e, "Positioning")?.to_string(),
                from_point: opt(e, "FromPoint"),
                auto_link: bool_attr(e, "AutoLink")?,
                link_name: opt(e, "AutoLinkGeometryName"),
                apply_aor: bool_attr(e, "ApplyAOR")?,
                arguments,
                point_codes: prop("PointCodes").map(read_codes).transpose()?,
                link_codes: prop("AutoLinkCodes").map(read_codes).transpose()?,
            }))
        }
        "CreateLink" | "CreateAuxLink" => Ok(PktActivity::Link(PktLink {
            auxiliary: kind == "CreateAuxLink",
            common: common(e)?,
            name: req(e, "LinkNumber")?.to_string(),
            start: req(e, "StartPoint")?.to_string(),
            end: req(e, "EndPoint")?.to_string(),
            apply_aor: bool_attr(e, "ApplyAOR")?,
            enabled: bool_attr(e, "IsEnabled")?,
            codes: prop("LinkCodes").map(read_codes).transpose()?,
        })),
        "CreateShape" => {
            let links: Vec<String> = prop("ComponentNames").map(|c| c.elements().map(|s| s.inner_text()).collect()).unwrap_or_default();
            if req(e, "Links")? != links.join(",") {
                return Err(err("shape links do not match its component names"));
            }
            Ok(PktActivity::Shape(PktShape { common: common(e)?, name: req(e, "ShapeNumber")?.to_string(), links, codes: prop("ShapeCodes").map(read_codes).transpose()? }))
        }
        "Sequence" => {
            let activities = e.elements().filter(|c| c.name != VIEW).map(read_activity).collect::<Result<Vec<_>, _>>()?;
            Ok(PktActivity::Sequence(PktSequence { display_name: e.get("DisplayName").map(str::to_string), hint_size: e.get(HINT).map(str::to_string), view: read_view(e)?, activities }))
        }
        "InternalVariableDefine" => {
            let value = prop("DefaultValue").and_then(|d| d.elements().next()).map(read_argument).transpose()?;
            let target = prop("Variable").and_then(|d| d.elements().next()).map(read_argument).transpose()?;
            Ok(PktActivity::DefineVariable(PktDefineVariable {
                common: common(e)?,
                name: req(e, "VariableName")?.to_string(),
                old_name: req(e, "OldVariableName")?.to_string(),
                type_name: req(e, "VariableType")?.to_string(),
                value: value.ok_or_else(|| err("variable without a default value"))?,
                target: target.ok_or_else(|| err("variable without its target"))?,
            }))
        }
        "SetVariableValue" | "SetOutputParameterValue" => {
            let (t, v) = if kind == "SetVariableValue" { ("VariableName", "VariableValue") } else { ("OutputParammeter", "Value") };
            let target = prop(t).and_then(|d| d.elements().next()).map(read_argument).transpose()?.ok_or_else(|| err("assignment without its target"))?;
            let value = prop(v).and_then(|d| d.elements().next()).map(read_argument).transpose()?.ok_or_else(|| err("assignment without its value"))?;
            let a = PktAssignment { common: common(e)?, target, value };
            Ok(if kind == "SetVariableValue" { PktActivity::SetVariable(a) } else { PktActivity::SetOutput(a) })
        }
        n => Err(err(format!("activity {n} is not supported"))),
    }
}

fn common_attrs(e: Element, c: &PktCommon) -> Element {
    e.attr("ActivityId", &c.activity_id.to_string()).attr("DisplayName", &c.display_name)
}

fn tail_attrs(mut e: Element, c: &PktCommon, geometry: bool) -> Element {
    if geometry {
        e = e.attr("Geometry", GEOMETRY);
    }
    if let Some(h) = &c.hint_size {
        e = e.attr(HINT, h);
    }
    e
}

fn run_attrs(e: Element, c: &PktCommon) -> Element {
    e.attr("ShowErrors", tf(c.show_errors)).attr("SubassemblyErrorCenter", ERRORS).attr("SubassemblyRunMode", RUN_MODE)
}

fn activity_element(a: &PktActivity) -> Element {
    match a {
        PktActivity::Point(p) => {
            let name = if p.auxiliary { "asa1:CreateAuxPoint" } else { "asa1:CreatePoint" };
            let mut e = Element::new(name);
            // the editor writes the unset references first
            for (n, set) in [("AutoLinkCodes", p.link_codes.is_some()), ("AutoLinkGeometryName", p.link_name.is_some()), ("FromPoint", p.from_point.is_some()), ("PointCodes", p.point_codes.is_some())] {
                if !set && (n != "PointCodes" || p.auxiliary) {
                    e = e.attr(n, "{x:Null}");
                }
            }
            e = common_attrs(e, &p.common).attr("ApplyAOR", tf(p.apply_aor)).attr("AutoLink", tf(p.auto_link));
            if let Some(l) = &p.link_name {
                e = e.attr("AutoLinkGeometryName", l);
            }
            if let Some(f) = &p.from_point {
                e = e.attr("FromPoint", f);
            }
            e = tail_attrs(e, &p.common, true).attr("PointNumber", &p.name).attr("Positioning", &p.positioning);
            e = run_attrs(e, &p.common).attr("Side", "[Side]");
            // ShowErrors comes before Side in the editor's order; attribute
            // order does not change the document.
            let mut args = Element::new(&format!("{name}.Arguments"));
            for x in &p.arguments {
                args = args.child(argument_element(x));
            }
            e = e.child(args);
            if let Some(c) = &p.link_codes {
                e = e.child(codes_element(&format!("{name}.AutoLinkCodes"), c));
            }
            if let Some(c) = &p.point_codes {
                e = e.child(codes_element(&format!("{name}.PointCodes"), c));
            }
            push_view(&mut e, &p.common.view);
            e
        }
        PktActivity::Link(l) => {
            let name = if l.auxiliary { "asa1:CreateAuxLink" } else { "asa1:CreateLink" };
            let mut e = Element::new(name);
            if l.codes.is_none() {
                e = e.attr("LinkCodes", "{x:Null}");
            }
            e = common_attrs(e, &l.common).attr("ApplyAOR", tf(l.apply_aor)).attr("EndPoint", &l.end);
            e = tail_attrs(e, &l.common, true).attr("IsEnabled", tf(l.enabled)).attr("LinkNumber", &l.name).attr("StartPoint", &l.start);
            e = run_attrs(e, &l.common);
            if let Some(c) = &l.codes {
                e = e.child(codes_element(&format!("{name}.LinkCodes"), c));
            }
            push_view(&mut e, &l.common.view);
            e
        }
        PktActivity::Shape(s) => {
            let mut e = common_attrs(Element::new("asa1:CreateShape"), &s.common);
            e = tail_attrs(e, &s.common, true).attr("Links", &s.links.join(",")).attr("ShapeNumber", &s.name);
            e = run_attrs(e, &s.common);
            let mut names = Element::new("asa1:CreateShape.ComponentNames");
            for l in &s.links {
                names = names.child(Element::new("x:String").text(l));
            }
            e = e.child(names);
            if let Some(c) = &s.codes {
                e = e.child(codes_element("asa1:CreateShape.ShapeCodes", c));
            }
            push_view(&mut e, &s.common.view);
            e
        }
        PktActivity::Sequence(s) => {
            let mut e = Element::new("Sequence");
            if let Some(d) = &s.display_name {
                e = e.attr("DisplayName", d);
            }
            if let Some(h) = &s.hint_size {
                e = e.attr(HINT, h);
            }
            push_view(&mut e, &s.view);
            for a in &s.activities {
                e = e.child(activity_element(a));
            }
            e
        }
        PktActivity::DefineVariable(v) => {
            let mut e = common_attrs(Element::new("asa1:InternalVariableDefine"), &v.common);
            e = tail_attrs(e, &v.common, false).attr("OldVariableName", &v.old_name);
            e = run_attrs(e, &v.common).attr("VariableName", &v.name).attr("VariableType", &v.type_name);
            e = e.child(Element::new("asa1:InternalVariableDefine.DefaultValue").child(argument_element(&v.value)));
            e = e.child(Element::new("asa1:InternalVariableDefine.Variable").child(argument_element(&v.target)));
            push_view(&mut e, &v.common.view);
            e
        }
        PktActivity::SetVariable(a) => assignment_element(a, true),
        PktActivity::SetOutput(a) => assignment_element(a, false),
    }
}

// ---- nodes ----

fn read_node(e: &Element, names: &mut Vec<String>, nodes: &mut Vec<Option<PktNode>>) -> Result<usize, DxfError> {
    if e.name == "x:Reference" {
        let r = e.inner_text();
        return slot(&r, names, nodes);
    }
    let reference = req(e, "x:Name")?.to_string();
    let i = slot(&reference, names, nodes)?;
    let node = match e.name.as_str() {
        "FlowStep" => {
            let mut action = None;
            let mut next = None;
            for c in e.elements() {
                match c.name.as_str() {
                    VIEW => {}
                    "FlowStep.Next" => next = c.elements().next().map(|n| read_node(n, names, nodes)).transpose()?,
                    _ => action = Some(read_activity(c)?),
                }
            }
            PktNode::Step(PktStep { reference, view: read_view(e)?, action: action.ok_or_else(|| err("step without an action"))?, next })
        }
        "FlowDecision" => {
            let branch = |n: &str, names: &mut Vec<String>, nodes: &mut Vec<Option<PktNode>>| e.first(n).and_then(|b| b.elements().next()).map(|x| read_node(x, names, nodes)).transpose();
            let on_true = branch("FlowDecision.True", names, nodes)?;
            let on_false = branch("FlowDecision.False", names, nodes)?;
            PktNode::Decision(PktDecision { reference, condition: req(e, "Condition")?.to_string(), display_name: e.get("DisplayName").map(str::to_string), hint_size: e.get(HINT).map(str::to_string), view: read_view(e)?, on_true, on_false })
        }
        "FlowSwitch" => {
            let default = e.first("FlowSwitch.Default").and_then(|b| b.elements().next()).map(|x| read_node(x, names, nodes)).transpose()?;
            let mut cases = Vec::new();
            for c in e.elements().filter(|c| c.name != VIEW && c.name != "FlowSwitch.Default") {
                cases.push((req(c, "x:Key")?.to_string(), read_node(c, names, nodes)?));
            }
            PktNode::Switch(PktSwitch {
                reference,
                type_name: req(e, "x:TypeArguments")?.to_string(),
                expression: req(e, "Expression")?.to_string(),
                display_name: e.get("DisplayName").map(str::to_string),
                hint_size: e.get(HINT).map(str::to_string),
                view: read_view(e)?,
                default,
                cases,
            })
        }
        n => return Err(err(format!("node {n}"))),
    };
    if nodes[i].is_some() {
        return Err(err(format!("node {} written twice", names[i])));
    }
    nodes[i] = Some(node);
    Ok(i)
}

fn slot(r: &str, names: &mut Vec<String>, nodes: &mut Vec<Option<PktNode>>) -> Result<usize, DxfError> {
    Ok(match names.iter().position(|n| n == r) {
        Some(i) => i,
        None => {
            names.push(r.to_string());
            nodes.push(None);
            names.len() - 1
        }
    })
}

fn node_element(i: usize, nodes: &[PktNode], written: &mut [bool], key: Option<&str>) -> Element {
    let n = &nodes[i];
    if written[i] {
        let mut e = Element::new("x:Reference");
        if let Some(k) = key {
            e = e.attr("x:Key", k);
        }
        return e.text(n.reference());
    }
    written[i] = true;
    match n {
        PktNode::Step(s) => {
            let mut e = Element::new("FlowStep");
            if let Some(k) = key {
                e = e.attr("x:Key", k);
            }
            e = e.attr("x:Name", &s.reference);
            push_view(&mut e, &s.view);
            e = e.child(activity_element(&s.action));
            if let Some(x) = s.next {
                e = e.child(Element::new("FlowStep.Next").child(node_element(x, nodes, written, None)));
            }
            e
        }
        PktNode::Decision(d) => {
            let mut e = Element::new("FlowDecision");
            if let Some(k) = key {
                e = e.attr("x:Key", k);
            }
            e = e.attr("x:Name", &d.reference).attr("Condition", &d.condition);
            if let Some(v) = &d.display_name {
                e = e.attr("DisplayName", v);
            }
            if let Some(h) = &d.hint_size {
                e = e.attr(HINT, h);
            }
            push_view(&mut e, &d.view);
            for (tag, b) in [("FlowDecision.True", d.on_true), ("FlowDecision.False", d.on_false)] {
                if let Some(x) = b {
                    e = e.child(Element::new(tag).child(node_element(x, nodes, written, None)));
                }
            }
            e
        }
        PktNode::Switch(s) => {
            let mut e = Element::new("FlowSwitch").attr("x:TypeArguments", &s.type_name);
            if let Some(k) = key {
                e = e.attr("x:Key", k);
            }
            e = e.attr("x:Name", &s.reference).attr("Expression", &s.expression);
            if let Some(v) = &s.display_name {
                e = e.attr("DisplayName", v);
            }
            if let Some(h) = &s.hint_size {
                e = e.attr(HINT, h);
            }
            if let Some(x) = s.default {
                e = e.child(Element::new("FlowSwitch.Default").child(node_element(x, nodes, written, None)));
            }
            push_view(&mut e, &s.view);
            for (k, x) in &s.cases {
                e = e.child(node_element(*x, nodes, written, Some(k)));
            }
            e
        }
    }
}

fn assignment_element(a: &PktAssignment, variable: bool) -> Element {
    let (name, t, v) = if variable { ("asa1:SetVariableValue", "VariableName", "VariableValue") } else { ("asa1:SetOutputParameterValue", "OutputParammeter", "Value") };
    let mut e = common_attrs(Element::new(name), &a.common);
    e = tail_attrs(e, &a.common, false);
    e = run_attrs(e, &a.common);
    e = e.child(Element::new(&format!("{name}.{t}")).child(argument_element(&a.target)));
    e = e.child(Element::new(&format!("{name}.{v}")).child(argument_element(&a.value)));
    push_view(&mut e, &a.common.view);
    e
}

// ---- the document ----

fn read_member(p: &Element) -> Result<Option<(String, PktDirection, String, Vec<PktAttribute>)>, DxfError> {
    let name = req(p, "Name")?.to_string();
    let ty = req(p, "Type")?;
    let open = ty.find('(').ok_or_else(|| err("argument type"))?;
    let direction = match &ty[..open] {
        "InArgument" => PktDirection::In,
        "OutArgument" => PktDirection::Out,
        "InOutArgument" => PktDirection::InOut,
        d => return Err(err(format!("argument direction {d}"))),
    };
    let type_name = ty[open + 1..ty.len() - 1].to_string();
    if direction == PktDirection::InOut {
        if RUN_ARGUMENTS.iter().any(|r| r.0 == name && type_name == format!("asw:{}", r.1)) {
            return Ok(None);
        }
        return Err(err(format!("in-out argument {name}")));
    }
    let mut attributes = Vec::new();
    if let Some(a) = p.first("x:Property.Attributes") {
        for x in a.elements() {
            attributes.push(match x.name.as_str() {
                "asw:EnabledFlag2Attribute" => PktAttribute::Enabled(bool_attr(x, "EnabledFlag")?),
                "asw:DisplayName2Attribute" => PktAttribute::DisplayName(req(x, "DisplayName")?.to_string()),
                "asw:Description2Attribute" => PktAttribute::Description(req(x, "Description")?.to_string()),
                n => return Err(err(format!("argument attribute {n}"))),
            });
        }
    }
    Ok(Some((name, direction, type_name, attributes)))
}

/// Reads the workflow into `pkg` (its arguments, variables, flowchart and
/// document settings).
pub(super) fn read(text: &str, pkg: &mut PktPackage) -> Result<(), DxfError> {
    let root = super::xml::parse(text).map_err(err)?;
    if root.name != "Activity" || root.get("x:Class") != Some("Subassembly") {
        return Err(err("the root is not the subassembly activity"));
    }
    let mut defaults = Vec::new();
    for (n, v) in &root.attrs {
        if let Some(arg) = n.strip_prefix("xmlns:") {
            pkg.namespaces.push((arg.to_string(), v.clone()));
        } else if n == "xmlns" {
            pkg.namespaces.push((String::new(), v.clone()));
        } else if n == "mc:Ignorable" {
            pkg.ignorable = v.clone();
        } else if let Some(arg) = n.strip_prefix("this:Subassembly.") {
            defaults.push((arg.to_string(), v.clone()));
        } else if n != "x:Class" {
            return Err(err(format!("activity attribute {n}")));
        }
    }
    for m in root.first("x:Members").map(|m| m.elements().collect::<Vec<_>>()).unwrap_or_default() {
        if let Some((name, direction, type_name, attributes)) = read_member(m)? {
            let default = defaults.iter().find(|d| d.0 == name).map(|d| d.1.clone());
            pkg.parameters.push(PktParameter { name, direction, type_name, default, attributes });
        }
    }
    if defaults.iter().any(|d| !pkg.parameters.iter().any(|p| p.name == d.0)) {
        return Err(err("a default for an undeclared argument"));
    }
    pkg.workflow_hint_size = root.first(HINT).map(|h| h.inner_text());
    pkg.vb_settings = root.first(VB).map(|h| h.inner_text());
    let f = root.first("Flowchart").ok_or_else(|| err("no flowchart"))?;
    pkg.flowchart_hint_size = f.get(HINT).map(str::to_string);
    pkg.flowchart_vb_settings = f.get(VB).map(str::to_string);
    pkg.flowchart_view = read_view(f)?;
    if let Some(vs) = f.first("Flowchart.Variables") {
        for v in vs.elements() {
            pkg.variables.push(PktVariable {
                type_name: req(v, "x:TypeArguments")?.to_string(),
                name: req(v, "Name")?.to_string(),
                default: v.get("Default").map(str::to_string),
                read_only: v.get("Modifiers") == Some("ReadOnly"),
            });
        }
    }
    let mut names = Vec::new();
    let mut nodes = Vec::new();
    let start = f.first("Flowchart.StartNode").and_then(|s| s.elements().next());
    pkg.start = match start {
        Some(s) if s.name == "x:Null" => None,
        Some(s) => Some(read_node(s, &mut names, &mut nodes)?),
        None => None,
    };
    let order: Vec<String> = f.elements().filter(|e| e.name == "x:Reference").map(|e| e.inner_text()).collect();
    if order.len() != names.len() || order.iter().any(|o| !names.contains(o)) {
        return Err(err("the node list does not match the nodes"));
    }
    // nodes in the flowchart's order; links renumbered
    let at: Vec<usize> = names.iter().map(|n| order.iter().position(|o| o == n).unwrap()).collect();
    let mut sorted: Vec<Option<PktNode>> = vec![None; names.len()];
    for (i, n) in nodes.into_iter().enumerate() {
        let mut n = n.ok_or_else(|| err(format!("node {} is referenced but not written", names[i])))?;
        n.renumber(&at);
        sorted[at[i]] = Some(n);
    }
    pkg.flowchart = sorted.into_iter().map(|n| n.unwrap()).collect();
    pkg.start = pkg.start.map(|s| at[s]);
    Ok(())
}

/// The workflow document of `pkg`.
pub(super) fn write(pkg: &PktPackage) -> Result<String, DxfError> {
    let mut root = Element::new("Activity").attr("mc:Ignorable", &pkg.ignorable).attr("x:Class", "Subassembly");
    for p in &pkg.parameters {
        if let Some(d) = &p.default {
            root = root.attr(&format!("this:Subassembly.{}", p.name), d);
        }
    }
    for (prefix, uri) in &pkg.namespaces {
        root = root.attr(&if prefix.is_empty() { "xmlns".to_string() } else { format!("xmlns:{prefix}") }, uri);
    }
    let mut members = Element::new("x:Members");
    for (n, t) in RUN_ARGUMENTS {
        members = members.child(Element::new("x:Property").attr("Name", n).attr("Type", &format!("InOutArgument(asw:{t})")));
    }
    for p in &pkg.parameters {
        let dir = match p.direction {
            PktDirection::In => "InArgument",
            PktDirection::Out => "OutArgument",
            PktDirection::InOut => "InOutArgument",
        };
        let mut x = Element::new("x:Property").attr("Name", &p.name).attr("Type", &format!("{dir}({})", p.type_name));
        if !p.attributes.is_empty() {
            let mut a = Element::new("x:Property.Attributes");
            for at in &p.attributes {
                a = a.child(match at {
                    PktAttribute::Enabled(b) => Element::new("asw:EnabledFlag2Attribute").attr("EnabledFlag", tf(*b)),
                    PktAttribute::DisplayName(s) => Element::new("asw:DisplayName2Attribute").attr("DisplayName", s),
                    PktAttribute::Description(s) => Element::new("asw:Description2Attribute").attr("Description", s),
                });
            }
            x = x.child(a);
        }
        members = members.child(x);
    }
    root = root.child(members);
    if let Some(h) = &pkg.workflow_hint_size {
        root = root.child(Element::new(HINT).text(h));
    }
    if let Some(v) = &pkg.vb_settings {
        root = root.child(Element::new(VB).text(v));
    }
    let mut f = Element::new("Flowchart");
    if let Some(h) = &pkg.flowchart_hint_size {
        f = f.attr(HINT, h);
    }
    if let Some(v) = &pkg.flowchart_vb_settings {
        f = f.attr(VB, v);
    }
    if !pkg.variables.is_empty() {
        let mut vs = Element::new("Flowchart.Variables");
        for v in &pkg.variables {
            let mut x = Element::new("Variable").attr("x:TypeArguments", &v.type_name);
            if let Some(d) = &v.default {
                x = x.attr("Default", d);
            }
            if v.read_only {
                x = x.attr("Modifiers", "ReadOnly");
            }
            vs = vs.child(x.attr("Name", &v.name));
        }
        f = f.child(vs);
    }
    push_view(&mut f, &pkg.flowchart_view);
    let mut written = vec![false; pkg.flowchart.len()];
    let start = match pkg.start {
        Some(s) if s < pkg.flowchart.len() => node_element(s, &pkg.flowchart, &mut written, None),
        Some(_) => return Err(err("the start node is not in the flowchart")),
        None => Element::new("x:Null"),
    };
    if pkg.flowchart.iter().any(|n| n.links().iter().any(|l| *l >= pkg.flowchart.len())) {
        return Err(err("a node links to a node outside the flowchart"));
    }
    f = f.child(Element::new("Flowchart.StartNode").child(start));
    // nodes nothing reaches are written where the list names them
    for i in 0..pkg.flowchart.len() {
        if !written[i] {
            return Err(err(format!("node {} is not reached from the start node", pkg.flowchart[i].reference())));
        }
        f = f.child(Element::new("x:Reference").text(pkg.flowchart[i].reference()));
    }
    root = root.child(f);
    let mut out = String::from("\u{feff}");
    super::xml::write(&root, 0, true, &mut out);
    Ok(out)
}
