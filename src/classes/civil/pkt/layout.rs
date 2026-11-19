//! A package's geometry in layout mode: the flowchart run from its start
//! node with the given parameter values (each target taking its default),
//! points relative to the attachment point.
//!
//! ```text
//! step      runs its action, then the next node
//! decision  the condition picks the true or the false node
//! switch    the expression's text picks the case of that key, else the
//!           default node
//! point     DeltaXAndDeltaY: from + (s dx, dy)
//!           SlopeAndDeltaX:  from + (s dx, slope |dx|)
//!           from = the FromPoint (the attachment point when unset); s =
//!           +1 on the right, -1 on the left; with AutoLink a link from
//!           the FromPoint to the point
//! link      between two points by name
//! shape     the links it names, in order
//! variable  defined, then assigned by name
//! ```
//!
//! Auxiliary points and links take part in the run but are not part of
//! the geometry. A positioning other than these two is refused.

use super::expr::{self, fail, Scope, Value};
use super::*;
use std::collections::HashMap;

/// The side a subassembly lies on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PktSide {
    Right,
    Left,
}

/// A parameter value given to the layout: a number (double, grade, slope),
/// an enumeration key, or a text.
#[derive(Debug, Clone, PartialEq)]
pub enum PktValue {
    Double(f64),
    Integer(i32),
    Text(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PktLayoutPoint {
    pub name: String,
    pub offset: f64,
    pub elevation: f64,
    pub codes: Vec<String>,
}

/// A link through points (indices into the layout's points).
#[derive(Debug, Clone, PartialEq)]
pub struct PktLayoutLink {
    pub name: String,
    pub points: Vec<usize>,
    pub codes: Vec<String>,
}

/// A shape bounded by links (indices into the layout's links).
#[derive(Debug, Clone, PartialEq)]
pub struct PktLayoutShape {
    pub name: String,
    pub links: Vec<usize>,
    pub codes: Vec<String>,
}

/// The geometry of a subassembly at its attachment point; `codes` lists
/// every code in order: the points', the links', the shapes'.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PktLayout {
    pub points: Vec<PktLayoutPoint>,
    pub links: Vec<PktLayoutLink>,
    pub shapes: Vec<PktLayoutShape>,
    pub codes: Vec<String>,
}

struct Run<'a> {
    pkg: &'a PktPackage,
    sign: f64,
    values: HashMap<String, Value>,
    /// every point by name (auxiliary ones too): offset, elevation
    at: HashMap<String, (f64, f64)>,
    out: PktLayout,
    links: HashMap<String, usize>,
}

impl Scope for Run<'_> {
    fn get(&self, name: &str) -> Option<Value> {
        let k = name.to_ascii_lowercase();
        self.values.get(&k).cloned().or_else(|| self.at.get(&k).map(|p| Value::Point(p.0, p.1)))
    }
}

/// `[new EnumType(key, "name"...)]`, `[new Grade(g)]`, `[new Slope(s)]`,
/// `[new Preview...Target("name", valid, value...)]`, or a literal.
fn default_value(d: &str) -> Result<Value, DxfError> {
    let Some(inner) = d.strip_prefix("[new ").and_then(|s| s.strip_suffix(")]")) else {
        return Ok(d.parse().map(Value::Number).unwrap_or_else(|_| Value::Text(d.to_string())));
    };
    let (kind, args) = inner.split_once('(').ok_or_else(|| fail(format!("default {d}")))?;
    let args: Vec<&str> = args.split(',').map(|a| a.trim().trim_matches('"')).collect();
    let n = |i: usize| args.get(i).and_then(|a| a.parse::<f64>().ok()).ok_or_else(|| fail(format!("default {d}")));
    Ok(match kind {
        "EnumType" => Value::Enum(n(0)? as i32, args.get(1).ok_or_else(|| fail("enumeration name"))?.to_string()),
        "Grade" | "Slope" => Value::Number(n(0)?),
        "PreviewOffsetTarget" | "PreviewElevationTarget" => Value::Target(args.get(1).is_some_and(|v| v.eq_ignore_ascii_case("true")), n(2)?),
        _ => return Err(fail(format!("default {d}"))),
    })
}

fn choice(pkg: &PktPackage, p: &PktParameter, key: i32) -> Result<Value, DxfError> {
    let group = p.default.as_deref().and_then(enum_default).and_then(|e| e.2).ok_or_else(|| fail(format!("{} names no enumeration", p.name)))?;
    let name = if group == "Side" {
        SIDE_GROUP.iter().find(|s| s.1 == key).map(|s| s.0.to_string())
    } else {
        pkg.enum_groups.iter().find(|g| g.name == group).and_then(|g| g.items.iter().find(|i| i.2 == key)).map(|i| i.0.clone())
    };
    Ok(Value::Enum(key, name.ok_or_else(|| fail(format!("{key} is not a choice of {}", p.name)))?))
}

impl Run<'_> {
    fn arg<'b>(&self, p: &'b PktPoint, key: &str) -> Result<&'b str, DxfError> {
        p.arguments.iter().find(|a| a.key.as_deref() == Some(key)).and_then(|a| a.value.as_deref()).ok_or_else(|| fail(format!("{} has no {key}", p.name)))
    }

    fn point(&mut self, p: &PktPoint) -> Result<(), DxfError> {
        let from = match &p.from_point {
            Some(f) => *self.at.get(&f.to_ascii_lowercase()).ok_or_else(|| fail(format!("{} comes from unknown point {f}", p.name)))?,
            None => (0.0, 0.0),
        };
        let (dx, dy) = match p.positioning.as_str() {
            "DeltaXAndDeltaY" => (expr::number(self.arg(p, "DeltaX1")?, self)?, expr::number(self.arg(p, "DeltaY1")?, self)?),
            "SlopeAndDeltaX" => {
                let dx = expr::number(self.arg(p, "DeltaX2")?, self)?;
                (dx, expr::number(self.arg(p, "Slope2")?, self)? * dx.abs())
            }
            m => return Err(fail(format!("positioning {m} of {}", p.name))),
        };
        let at = (from.0 + self.sign * dx, from.1 + dy);
        self.at.insert(p.name.to_ascii_lowercase(), at);
        if p.auxiliary {
            return Ok(());
        }
        let i = self.out.points.len();
        self.out.points.push(PktLayoutPoint { name: p.name.clone(), offset: at.0, elevation: at.1, codes: p.point_codes.clone().unwrap_or_default() });
        if p.auto_link {
            let (Some(f), Some(l)) = (&p.from_point, &p.link_name) else { return Err(fail(format!("{} links from no point", p.name))) };
            let s = self.index(f)?;
            self.link(l, vec![s, i], p.link_codes.clone().unwrap_or_default());
        }
        Ok(())
    }

    fn index(&self, name: &str) -> Result<usize, DxfError> {
        self.out.points.iter().position(|q| q.name.eq_ignore_ascii_case(name)).ok_or_else(|| fail(format!("unknown point {name}")))
    }

    fn link(&mut self, name: &str, points: Vec<usize>, codes: Vec<String>) {
        self.links.insert(name.to_ascii_lowercase(), self.out.links.len());
        self.out.links.push(PktLayoutLink { name: name.into(), points, codes });
    }

    fn activity(&mut self, a: &PktActivity) -> Result<(), DxfError> {
        match a {
            PktActivity::Point(p) => self.point(p),
            PktActivity::Link(l) if l.auxiliary => Ok(()),
            PktActivity::Link(l) => {
                let points = vec![self.index(&l.start)?, self.index(&l.end)?];
                self.link(&l.name, points, l.codes.clone().unwrap_or_default());
                Ok(())
            }
            PktActivity::Shape(s) => {
                let links = s.links.iter().map(|l| self.links.get(&l.to_ascii_lowercase()).copied().ok_or_else(|| fail(format!("shape {} names unknown link {l}", s.name)))).collect::<Result<_, _>>()?;
                self.out.shapes.push(PktLayoutShape { name: s.name.clone(), links, codes: s.codes.clone().unwrap_or_default() });
                Ok(())
            }
            PktActivity::Sequence(s) => s.activities.iter().try_for_each(|a| self.activity(a)),
            PktActivity::DefineVariable(v) => {
                let x = expr::eval(v.value.value.as_deref().unwrap_or_default(), self)?;
                self.values.insert(v.name.to_ascii_lowercase(), x);
                Ok(())
            }
            PktActivity::SetVariable(s) => {
                let target = s.target.value.as_deref().and_then(|t| t.strip_prefix('[')).and_then(|t| t.strip_suffix(']')).ok_or_else(|| fail("assignment target"))?;
                let x = expr::eval(s.value.value.as_deref().unwrap_or_default(), self)?;
                self.values.insert(target.trim().to_ascii_lowercase(), x);
                Ok(())
            }
            PktActivity::SetOutput(_) => Ok(()),
        }
    }
}

/// The layout of `pkg` on `side` with `values` (parameters not named take
/// their defaults; `Side` is `side`).
pub fn layout(pkg: &PktPackage, side: PktSide, values: &[(String, PktValue)]) -> Result<PktLayout, DxfError> {
    let mut v = HashMap::new();
    for var in &pkg.variables {
        if let Some(d) = &var.default {
            v.insert(var.name.to_ascii_lowercase(), default_value(d)?);
        }
    }
    for p in &pkg.parameters {
        let given = values.iter().find(|x| x.0.eq_ignore_ascii_case(&p.name)).map(|x| &x.1);
        let value = if p.name == "Side" {
            choice(pkg, p, if side == PktSide::Left { 1 } else { 0 })?
        } else {
            match (given, p.type_name.as_str()) {
                (Some(PktValue::Integer(k)), "asw:EnumType") => choice(pkg, p, *k)?,
                (Some(PktValue::Double(x)), "x:Double" | "asw:Grade" | "asw:Slope") => Value::Number(*x),
                (Some(PktValue::Text(t)), "x:String") => Value::Text(t.clone()),
                (Some(_), t) => return Err(fail(format!("a value of the wrong kind for {} ({t})", p.name))),
                (None, _) => match &p.default {
                    Some(d) => default_value(d)?,
                    None if p.direction == PktDirection::Out => continue,
                    None => return Err(fail(format!("{} has no value", p.name))),
                },
            }
        };
        v.insert(p.name.to_ascii_lowercase(), value);
    }
    if let Some(x) = values.iter().find(|x| !x.0.eq_ignore_ascii_case("Side") && !pkg.parameters.iter().any(|p| p.name.eq_ignore_ascii_case(&x.0))) {
        return Err(fail(format!("{} is not a parameter", x.0)));
    }
    let mut run = Run { pkg, sign: if side == PktSide::Left { -1.0 } else { 1.0 }, values: v, at: HashMap::new(), out: PktLayout::default(), links: HashMap::new() };
    let mut node = pkg.start;
    let mut steps = 0;
    while let Some(i) = node {
        steps += 1;
        if steps > 100_000 {
            return Err(fail("the flowchart does not end"));
        }
        node = match pkg.flowchart.get(i).ok_or_else(|| fail("link outside the flowchart"))? {
            PktNode::Step(s) => {
                run.activity(&s.action)?;
                s.next
            }
            PktNode::Decision(d) => match expr::eval(&d.condition, &run)? {
                Value::Bool(true) => d.on_true,
                Value::Bool(false) => d.on_false,
                x => return Err(fail(format!("condition {x:?}"))),
            },
            PktNode::Switch(s) => {
                let key = match expr::eval(&s.expression, &run)? {
                    Value::Text(t) => t,
                    Value::Number(n) => n.to_string(),
                    x => return Err(fail(format!("switch on {x:?}"))),
                };
                s.cases.iter().find(|c| c.0 == key).map(|c| c.1).or(s.default)
            }
        };
    }
    let _ = run.pkg;
    let o = &mut run.out;
    o.codes = o.points.iter().flat_map(|p| p.codes.clone()).chain(o.links.iter().flat_map(|l| l.codes.clone())).chain(o.shapes.iter().flat_map(|s| s.codes.clone())).collect();
    Ok(run.out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lays_out_the_lane() {
        let p = super::super::tests::lane();
        let l = layout(&p, PktSide::Right, &[("Width".into(), PktValue::Double(4.0)), ("Depth".into(), PktValue::Double(0.25)), ("Slope".into(), PktValue::Double(-0.03))]).unwrap();
        let at: Vec<(f64, f64)> = l.points.iter().map(|p| (p.offset, p.elevation)).collect();
        // the slope falls over |dx| on the way back as well
        assert_eq!(at, [(0.0, 0.0), (4.0, -0.12), (4.0, -0.37), (0.0, -0.49)]);
        assert_eq!(l.links.iter().map(|k| k.points.clone()).collect::<Vec<_>>(), [vec![0, 1], vec![1, 2], vec![2, 3], vec![3, 0]]);
        assert_eq!(l.shapes[0].links, [0, 1, 2, 3]);
        assert_eq!(l.codes, ["Crown", "ETW", "ETW_Datum", "Crown_Datum", "Top", "Pave", "Datum", "Pave"]);
        let left = layout(&p, PktSide::Left, &[]).unwrap();
        assert_eq!((left.points[1].offset, left.points[1].elevation), (-3.5, -0.07));
        assert!(layout(&p, PktSide::Right, &[("Nope".into(), PktValue::Double(1.0))]).is_err());
        assert!(layout(&p, PktSide::Right, &[("Width".into(), PktValue::Text("x".into()))]).is_err());
    }
}
