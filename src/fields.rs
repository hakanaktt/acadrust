//! Dynamic text field (`AcDbField`) evaluation engine.
//!
//! The library owns the field **language** (the DIESEL macro language and the
//! `AcVar` named fields), the field **structure** (the container → child → host
//! linkage recovered from object owners), and all the pure date math. It does
//! **not** read the system clock, the OS user, or environment variables — those
//! come from a caller-supplied [`FieldContext`], so the engine stays
//! deterministic and platform-neutral (no `std::time`, `getenv`, or web deps in
//! the core library).
//!
//! A field-hosting entity (usually an MTEXT) stores only the *cached* evaluated
//! text, frozen at the last save. [`resolve`] recomputes it against the current
//! context. Anything it can't evaluate — an unknown `getvar`, an unsupported
//! evaluator (`AcExpr` table sums, `AcObjProp` object properties) — yields
//! `None`, and the caller keeps the cached text.

use crate::document::{CadDocument, FieldDef};
use crate::entities::table::{CellValue, CellValueType, Table};
use crate::entities::{EntityCommon, EntityType};
use crate::objects::{Dictionary, Field, FieldChildValue, FieldList, ObjectType};
use crate::types::Handle;

/// Environment values the field engine cannot derive from the document alone.
/// Implemented by the host application; every method has a "don't know" default
/// (`None` / epoch) so a minimal host only needs [`now_julian`](FieldContext::now_julian).
pub trait FieldContext {
    /// "Now" as an astronomical Julian date. Drives `$(getvar,date)` /
    /// `$(getvar,cdate)`, `$(edtime,...)` on the current time, `$(time)`, and
    /// the `Date` AcVar field.
    fn now_julian(&self) -> f64;
    fn file_times(&self) -> Option<(f64, f64)> {
        None
    }
    fn plotting(&self) -> bool {
        false
    }
    /// Current user / login name (`\AcVar Login`, `$(getvar,loginname)`).
    fn login(&self) -> Option<String> {
        None
    }
    /// OS environment variable (`$(getenv,name)`).
    fn getenv(&self, _name: &str) -> Option<String> {
        None
    }
    /// Any other system variable the host can answer (`$(getvar,name)`) beyond
    /// the ones the engine resolves itself. `None` keeps the cached field text.
    fn getvar(&self, _name: &str) -> Option<String> {
        None
    }
    fn file_size(&self) -> Option<u64> {
        None
    }
    fn date_locale(&self) -> DateLocale {
        DateLocale::default()
    }
    fn sheet_sets(
        &self,
        _f: &mut dyn FnMut(&crate::sheet_set::SheetSetDatabase) -> Option<String>,
    ) -> Option<String> {
        None
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DateLocale {
    pub months: [String; 12],
    pub months_abbr: [String; 12],
    pub days: [String; 7],
    pub days_abbr: [String; 7],
    pub am: String,
    pub pm: String,
    pub short_date: String,
    pub long_date: String,
    pub long_time: String,
}

impl Default for DateLocale {
    fn default() -> Self {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let months = s(&[
            "January", "February", "March", "April", "May", "June", "July", "August",
            "September", "October", "November", "December",
        ]);
        let days = s(&[
            "Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday",
        ]);
        let abbr = |v: &[String]| v.iter().map(|x| x[..3].to_string()).collect::<Vec<_>>();
        Self {
            months_abbr: abbr(&months).try_into().unwrap(),
            days_abbr: abbr(&days).try_into().unwrap(),
            months: months.try_into().unwrap(),
            days: days.try_into().unwrap(),
            am: "AM".into(),
            pm: "PM".into(),
            short_date: "M/d/yyyy".into(),
            long_date: "dddd, MMMM d, yyyy".into(),
            long_time: "h:mm:ss tt".into(),
        }
    }
}

/// Re-evaluate the field hosted by entity `host` (usually an MTEXT), returning
/// fresh display text, or `None` when the entity hosts no field or the field
/// can't be fully evaluated (the caller then keeps the cached text).
pub fn resolve(doc: &CadDocument, host: Handle, ctx: &dyn FieldContext) -> Option<String> {
    if doc.fields.is_empty() {
        return None;
    }
    let container = container_for_host(doc, host)?;
    eval_field(doc, container, ctx, host)
}

/// Re-evaluate one specific field object for a known host.
///
/// Table cells reference their FIELD directly rather than through the single
/// `_text` container used by MTEXT. Exposing this narrow entry point lets a
/// host render those cells without guessing which field belongs to them.
pub fn resolve_handle(
    doc: &CadDocument,
    field_handle: Handle,
    host: Handle,
    ctx: &dyn FieldContext,
) -> Option<String> {
    let field = doc.fields.get(&field_handle)?;
    eval_field(doc, field, ctx, host)
}

// ── linkage ────────────────────────────────────────────────────────────────

/// Owner of an *object* handle (field / dictionary / other object). `None` when
/// the handle is not an object — i.e. it is an entity (the field's host).
fn owner_of(doc: &CadDocument, h: Handle) -> Option<Handle> {
    if let Some(f) = doc.fields.get(&h) {
        return Some(f.owner);
    }
    match doc.objects.get(&h)? {
        ObjectType::Dictionary(d) => Some(d.owner),
        ObjectType::Unknown { owner, .. } => Some(*owner),
        _ => None,
    }
}

/// Walk the owner chain up from a container field until the owner is not an
/// object — that handle is the host entity.
fn host_of(doc: &CadDocument, container: &FieldDef) -> Option<Handle> {
    let mut h = container.handle;
    for _ in 0..12 {
        let o = owner_of(doc, h)?;
        if owner_of(doc, o).is_none() {
            return Some(o);
        }
        h = o;
    }
    None
}

fn container_for_host(doc: &CadDocument, host: Handle) -> Option<&FieldDef> {
    doc.fields
        .values()
        .find(|f| f.evaluator == "_text" && host_of(doc, f) == Some(host))
}

// ── evaluation ─────────────────────────────────────────────────────────────

fn eval_field(
    doc: &CadDocument,
    field: &FieldDef,
    ctx: &dyn FieldContext,
    host: Handle,
) -> Option<String> {
    match field.evaluator.as_str() {
        "_text" => eval_template(doc, field, ctx, host),
        e if e.starts_with("AcVar") => eval_acvar(doc, &field.code, ctx, host),
        "AcDiesel" => {
            let expr = field
                .code
                .strip_prefix("\\AcDiesel ")
                .unwrap_or(&field.code)
                .trim();
            diesel_eval(doc, expr, ctx)
        }
        // AcExpr — a table cell formula (Sum/Average/… over A1-style cell refs).
        "AcExpr" => eval_acexpr(doc, field, host),
        // AcObjProp[.ver] — a property of a referenced object.
        e if e.starts_with("AcObjProp") => eval_acobjprop(doc, field, host),
        // AcSm[.16.2] — a property of the drawing's sheet in its sheet set.
        e if e.starts_with("AcSm") => {
            let layout = host_layout(doc, host, ctx).map(|l| l.name.as_str());
            crate::sheet_set::eval_acsm(doc, &field.code, ctx, layout, text_case)
        }
        // AcCount / AcCount2 — block counts (see `crate::count`).
        "AcCount" | "AcCount2" => Some(crate::count::evaluate(doc, &field.code)),
        _ => None,
    }
}

/// Substitute each `%<\_FldIdx N>%` marker in a container's template with the
/// evaluation of its Nth child field, keeping the surrounding MTEXT codes.
fn eval_template(
    doc: &CadDocument,
    container: &FieldDef,
    ctx: &dyn FieldContext,
    host: Handle,
) -> Option<String> {
    let mut children: Vec<&FieldDef> = doc
        .fields
        .values()
        .filter(|f| f.owner == container.handle)
        .collect();
    children.sort_by_key(|f| u64::from(f.handle));
    // MTEXT contents escape backslashes (a path `C:\x` is stored as `C:\\x`).
    let mtext = matches!(doc.get_entity(host), Some(EntityType::MText(_)));

    let mut out = String::new();
    let mut rest = container.code.as_str();
    while let Some(p) = rest.find("%<") {
        out.push_str(&rest[..p]);
        let after = &rest[p..];
        let Some(end) = after.find(">%") else {
            out.push_str(after);
            return Some(out);
        };
        let marker = &after[2..end]; // e.g. "\_FldIdx 0"
        if !marker.contains("_FldIdx") {
            return None;
        }
        let idx: usize = marker.rsplit(' ').next()?.trim().parse().ok()?;
        let child = children.get(idx)?;
        let value = eval_field(doc, child, ctx, host)?;
        out.push_str(&if mtext { mtext_escape(&value) } else { value });
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    Some(out)
}

// ── AcExpr: formulas ─────────────────────────────────────────────────────────

fn eval_acexpr(doc: &CadDocument, field: &FieldDef, host: Handle) -> Option<String> {
    let code = &field.code;
    let body = code.trim().strip_prefix("\\AcExpr").unwrap_or(code);
    let (expr, fmt) = match body.find("\\f ") {
        Some(fp) => (body[..fp].trim(), format_of(&body[fp + 3..])),
        None => (body.trim(), String::new()),
    };
    let mut p = ExprParser {
        doc,
        s: expr.as_bytes(),
        i: 0,
        objects: &field.objects,
        table: table_for_host(doc, host),
        host_cells: true,
        used_cells: false,
        depth: 0,
    };
    let v = match p.parse_all() {
        Ok(v) => v,
        Err(ExprError::Invalid) => return Some("####".into()),
        Err(ExprError::Unresolved) => return None,
    };
    Some(match v {
        _ if !fmt.is_empty() => format_number(doc, v.real(), &fmt),
        // Cell formulas of the host table keep the plain numeric text.
        _ if p.used_cells => num_str(v.real()),
        Num::Int(n) => n.to_string(),
        Num::Real(x) => format!("{:.6}", x),
    })
}

/// The ACAD_TABLE whose rendered block owns the host cell (its MTEXT sits in the
/// table's block-record), falling back to the sole table when a drawing has
/// exactly one.
fn table_for_host(doc: &CadDocument, host: Handle) -> Option<&Table> {
    if let Some(EntityType::Table(table)) = doc.get_entity(host) {
        return Some(table);
    }
    if let Some(owner) = doc.get_entity(host).map(|e| e.common().owner_handle) {
        if let Some(t) = doc.entities().find_map(|e| match e {
            EntityType::Table(t) if t.block_record_handle == Some(owner) => Some(t),
            _ => None,
        }) {
            return Some(t);
        }
        // A text in model or paper space is no table cell.
        let layout_block = doc.block_records.iter().any(|b| {
            let n = b.name.to_ascii_lowercase();
            b.handle == owner && (n.starts_with("*model_space") || n.starts_with("*paper_space"))
        });
        if layout_block {
            return None;
        }
    }
    let mut tables = doc.entities().filter_map(|e| match e {
        EntityType::Table(t) => Some(t),
        _ => None,
    });
    let first = tables.next()?;
    tables.next().is_none().then_some(first)
}

enum CellNum {
    Num(Num),
    Blank,
    Unknown,
}

/// Parse an A1-style cell reference at `*i`, returning 0-based `(col, row)`.
fn parse_cellref(s: &[u8], i: &mut usize) -> Option<(usize, usize)> {
    let start = *i;
    let mut col = 0usize;
    let mut has_col = false;
    while let Some(&b) = s.get(*i) {
        if b.is_ascii_alphabetic() {
            col = col * 26 + (b.to_ascii_uppercase() - b'A' + 1) as usize;
            *i += 1;
            has_col = true;
        } else {
            break;
        }
    }
    let mut row = 0usize;
    let mut has_row = false;
    while let Some(&b) = s.get(*i) {
        if b.is_ascii_digit() {
            row = row * 10 + (b - b'0') as usize;
            *i += 1;
            has_row = true;
        } else {
            break;
        }
    }
    if has_col && has_row && col >= 1 && row >= 1 {
        Some((col - 1, row - 1))
    } else {
        *i = start;
        None
    }
}

#[derive(Clone, Copy)]
enum Num {
    Int(i64),
    Real(f64),
}

impl Num {
    fn real(self) -> f64 {
        match self {
            Num::Int(n) => n as f64,
            Num::Real(x) => x,
        }
    }
}

enum ExprError {
    Invalid,
    Unresolved,
}

type ExprResult = Result<Num, ExprError>;

fn int_checked(n: Option<i64>) -> ExprResult {
    match n {
        Some(n) if i32::try_from(n).is_ok() => Ok(Num::Int(n)),
        _ => Err(ExprError::Invalid),
    }
}

struct ExprParser<'a> {
    doc: &'a CadDocument,
    s: &'a [u8],
    i: usize,
    objects: &'a [Handle],
    table: Option<&'a Table>,
    host_cells: bool,
    used_cells: bool,
    depth: u8,
}

impl<'a> ExprParser<'a> {
    fn parse_all(&mut self) -> ExprResult {
        let v = self.parse_expr()?;
        self.skip_ws();
        if self.i == self.s.len() {
            Ok(v)
        } else {
            Err(ExprError::Invalid)
        }
    }
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }
    fn skip_ws(&mut self) {
        while self.peek() == Some(b' ') {
            self.i += 1;
        }
    }
    fn eat(&mut self, c: u8) -> bool {
        self.skip_ws();
        let hit = self.peek() == Some(c);
        if hit {
            self.i += 1;
        }
        hit
    }
    fn parse_expr(&mut self) -> ExprResult {
        let mut v = self.parse_term()?;
        loop {
            if self.eat(b'+') {
                v = match (v, self.parse_term()?) {
                    (Num::Int(a), Num::Int(b)) => int_checked(a.checked_add(b))?,
                    (a, b) => Num::Real(a.real() + b.real()),
                };
            } else if self.eat(b'-') {
                v = match (v, self.parse_term()?) {
                    (Num::Int(a), Num::Int(b)) => int_checked(a.checked_sub(b))?,
                    (a, b) => Num::Real(a.real() - b.real()),
                };
            } else {
                return Ok(v);
            }
        }
    }
    fn parse_term(&mut self) -> ExprResult {
        let mut v = self.parse_power()?;
        loop {
            if self.eat(b'*') {
                v = match (v, self.parse_power()?) {
                    (Num::Int(a), Num::Int(b)) => int_checked(a.checked_mul(b))?,
                    (a, b) => Num::Real(a.real() * b.real()),
                };
            } else if self.eat(b'/') {
                let d = self.parse_power()?.real();
                if d == 0.0 {
                    return Err(ExprError::Invalid);
                }
                v = Num::Real(v.real() / d);
            } else {
                return Ok(v);
            }
        }
    }
    fn parse_power(&mut self) -> ExprResult {
        let mut v = self.parse_unary()?;
        while self.eat(b'^') {
            let e = self.parse_unary()?;
            v = Num::Real(v.real().powf(e.real()));
        }
        Ok(v)
    }
    fn parse_unary(&mut self) -> ExprResult {
        if self.eat(b'-') {
            return Ok(match self.parse_primary()? {
                Num::Int(n) => Num::Int(-n),
                Num::Real(x) => Num::Real(-x),
            });
        }
        self.eat(b'+');
        self.parse_primary()
    }
    fn parse_primary(&mut self) -> ExprResult {
        self.skip_ws();
        match self.peek().ok_or(ExprError::Invalid)? {
            b'(' => {
                self.i += 1;
                let v = self.parse_expr()?;
                if !self.eat(b')') {
                    return Err(ExprError::Invalid);
                }
                Ok(v)
            }
            c if c.is_ascii_digit() || c == b'.' => self.parse_number(),
            c if c.is_ascii_alphabetic() => self.parse_name(),
            _ => Err(ExprError::Invalid),
        }
    }
    fn parse_number(&mut self) -> ExprResult {
        let start = self.i;
        let mut real = false;
        while let Some(b) = self.peek() {
            match b {
                b'0'..=b'9' => {}
                b'.' => real = true,
                b'e' | b'E' => {
                    real = true;
                    if matches!(self.s.get(self.i + 1), Some(b'+') | Some(b'-')) {
                        self.i += 1;
                    }
                }
                _ => break,
            }
            self.i += 1;
        }
        let text =
            std::str::from_utf8(&self.s[start..self.i]).map_err(|_| ExprError::Invalid)?;
        if real {
            text.parse().map(Num::Real).map_err(|_| ExprError::Invalid)
        } else {
            int_checked(text.parse().ok())
        }
    }
    fn parse_name(&mut self) -> ExprResult {
        let start = self.i;
        while matches!(self.peek(), Some(b) if b.is_ascii_alphabetic()) {
            self.i += 1;
        }
        let name = std::str::from_utf8(&self.s[start..self.i])
            .map_err(|_| ExprError::Invalid)?
            .to_lowercase();
        if self.peek() == Some(b'(') {
            self.i += 1;
            if name == "table" {
                return self.parse_table_ref();
            }
            let vals = self.parse_args()?;
            if !self.eat(b')') {
                return Err(ExprError::Invalid);
            }
            return apply_func(&name, &vals, self.table.is_some());
        }
        if name == "pi" {
            return Ok(Num::Real(std::f64::consts::PI));
        }
        self.i = start;
        let (col, row) = parse_cellref(self.s, &mut self.i).ok_or(ExprError::Invalid)?;
        match self.cell(col, row)? {
            CellNum::Num(v) => Ok(v),
            CellNum::Blank => Err(ExprError::Invalid),
            CellNum::Unknown => Err(ExprError::Unresolved),
        }
    }
    fn parse_table_ref(&mut self) -> ExprResult {
        self.skip_ws();
        let rest = std::str::from_utf8(&self.s[self.i..]).map_err(|_| ExprError::Invalid)?;
        let end = rest.find(">%").ok_or(ExprError::Invalid)?;
        let marker = &rest[..end];
        let handle = if let Some(n) = marker.strip_prefix("%<\\_ObjIdx ") {
            let idx: usize = n.trim().parse().map_err(|_| ExprError::Invalid)?;
            *self.objects.get(idx).ok_or(ExprError::Unresolved)?
        } else if let Some(n) = marker.strip_prefix("%<\\_ObjId ") {
            Handle::new(n.trim().parse().map_err(|_| ExprError::Invalid)?)
        } else {
            return Err(ExprError::Invalid);
        };
        self.i += end + 2;
        if !self.eat(b')') || !self.eat(b'.') {
            return Err(ExprError::Invalid);
        }
        let doc = self.doc;
        let Some(EntityType::Table(table)) = doc.get_entity(handle) else {
            return Err(ExprError::Unresolved);
        };
        let saved = (self.table, self.host_cells);
        self.table = Some(table);
        self.host_cells = false;
        let evaluate = self.s[self.i..]
            .get(..9)
            .is_some_and(|w| w.eq_ignore_ascii_case(b"evaluate("));
        let r = if evaluate {
            self.i += 9;
            self.parse_expr().and_then(|v| {
                if self.eat(b')') {
                    Ok(v)
                } else {
                    Err(ExprError::Invalid)
                }
            })
        } else {
            self.parse_name()
        };
        (self.table, self.host_cells) = saved;
        r
    }
    fn cell(&mut self, col: usize, row: usize) -> Result<CellNum, ExprError> {
        let Some(table) = self.table else {
            return Err(ExprError::Unresolved);
        };
        if self.host_cells {
            self.used_cells = true;
        }
        Ok(cell_value(self.doc, table, col, row, self.depth))
    }
    fn parse_args(&mut self) -> Result<Vec<Num>, ExprError> {
        let mut vals = Vec::new();
        loop {
            self.skip_ws();
            let save = self.i;
            match parse_cellref(self.s, &mut self.i) {
                Some((c1, r1)) if self.eat(b':') => {
                    self.skip_ws();
                    let (c2, r2) =
                        parse_cellref(self.s, &mut self.i).ok_or(ExprError::Invalid)?;
                    for r in r1.min(r2)..=r1.max(r2) {
                        for c in c1.min(c2)..=c1.max(c2) {
                            match self.cell(c, r)? {
                                CellNum::Num(v) => vals.push(v),
                                CellNum::Blank => {}
                                CellNum::Unknown => return Err(ExprError::Unresolved),
                            }
                        }
                    }
                }
                _ => {
                    self.i = save;
                    vals.push(self.parse_expr()?);
                }
            }
            if !self.eat(b',') {
                return Ok(vals);
            }
        }
    }
}

fn cell_value(doc: &CadDocument, table: &Table, col: usize, row: usize, depth: u8) -> CellNum {
    let Some(content) = table
        .rows
        .get(row)
        .and_then(|r| r.cells.get(col))
        .and_then(|c| c.contents.first())
    else {
        return CellNum::Blank;
    };
    if let Some(fh) = content.field_handle {
        let Some(mut f) = doc.fields.get(&fh) else {
            return CellNum::Unknown;
        };
        // A cell formula is the only child of the cell's `_text` container.
        if f.evaluator == "_text" && f.code.trim() == "%<\\_FldIdx 0>%" {
            match doc.fields.values().find(|c| c.owner == f.handle) {
                Some(child) => f = child,
                None => return CellNum::Unknown,
            }
        }
        if f.evaluator != "AcExpr" || depth >= 8 {
            return CellNum::Unknown;
        }
        let body = f.code.trim().strip_prefix("\\AcExpr").unwrap_or(&f.code);
        let expr = body.find("\\f ").map_or(body, |fp| &body[..fp]).trim();
        let mut p = ExprParser {
            doc,
            s: expr.as_bytes(),
            i: 0,
            objects: &f.objects,
            table: Some(table),
            host_cells: false,
            used_cells: false,
            depth: depth + 1,
        };
        return match p.parse_all() {
            Ok(v) => CellNum::Num(v),
            Err(ExprError::Invalid) => CellNum::Blank,
            Err(ExprError::Unresolved) => CellNum::Unknown,
        };
    }
    let cv = &content.value;
    match cv.value_type {
        CellValueType::Long => CellNum::Num(Num::Int(cv.numeric_value as i64)),
        CellValueType::Double => CellNum::Num(Num::Real(cv.numeric_value)),
        _ => {
            let s = if !cv.formatted_value.is_empty() {
                &cv.formatted_value
            } else {
                &cv.text
            };
            let s = s.trim();
            match (s.parse::<i32>(), s.parse::<f64>()) {
                (Ok(n), _) => CellNum::Num(Num::Int(n as i64)),
                (_, Ok(x)) => CellNum::Num(Num::Real(x)),
                _ => CellNum::Blank,
            }
        }
    }
}

fn apply_func(name: &str, vals: &[Num], table: bool) -> ExprResult {
    let xs: Vec<f64> = vals.iter().map(|v| v.real()).collect();
    let one = || match xs.as_slice() {
        [x] => Ok(*x),
        _ => Err(ExprError::Invalid),
    };
    let r = match name {
        "sum" if table => {
            if vals.iter().all(|v| matches!(v, Num::Int(_))) {
                let total = vals.iter().map(|v| v.real() as i64).sum();
                return int_checked(Some(total));
            }
            xs.iter().sum()
        }
        "average" | "mean" if table && !xs.is_empty() => xs.iter().sum::<f64>() / xs.len() as f64,
        "count" if table => return Ok(Num::Int(xs.len() as i64)),
        "min" if table => xs.iter().copied().reduce(f64::min).ok_or(ExprError::Invalid)?,
        "max" if table => xs.iter().copied().reduce(f64::max).ok_or(ExprError::Invalid)?,
        "product" if table => xs.iter().product(),
        "abs" => one()?.abs(),
        "sqrt" if one()? >= 0.0 => one()?.sqrt(),
        // `round` gives an integer (`round(2.5)` shows `3`).
        "round" => return int_checked(Some(one()?.round() as i64)),
        _ => return Err(ExprError::Invalid),
    };
    Ok(Num::Real(r))
}

// ── AcObjProp: object properties ─────────────────────────────────────────────

/// A resolved object-property value, before formatting.
enum PropVal {
    Num(f64),
    Angle(f64),
    Point([f64; 3]),
    Text(String),
    Lineweight(i16),
}

/// Evaluate an `AcObjProp` field — a property of a referenced object, e.g.
fn eval_acobjprop(doc: &CadDocument, field: &FieldDef, host: Handle) -> Option<String> {
    let code = &field.code;
    let prop = code.split(").").nth(1)?.split([' ', '\\']).next()?.trim();
    if prop.is_empty() {
        return None;
    }
    let fmt = code
        .find("\\f ")
        .map(|p| format_of(&code[p + 3..]))
        .unwrap_or_default();
    let direct;
    let handle = if code.contains("?BlockRefId") {
        match attribute_insert(doc, host) {
            Some(insert) => {
                direct = insert;
                &direct
            }
            None => return Some(text_case(prop.to_string(), &fmt)),
        }
    } else {
        match between(code, "_ObjIdx ", ">%") {
            Some(idx) => field.objects.get(idx.trim().parse::<usize>().ok()?)?,
            // `%<\_ObjId n>%` names the object directly (preview codes).
            None => {
                direct = Handle::new(between(code, "_ObjId ", ">%")?.trim().parse().ok()?);
                &direct
            }
        }
    };
    // NamedObject fields: `.Name` of a layer, block, style, linetype, view, …
    if prop == "Name" {
        if let Some(name) = named_object_name(doc, *handle) {
            return Some(text_case(name, &fmt));
        }
    }
    let entity = doc.entities().find(|e| &e.common().handle == handle)?;
    let val = object_property(doc, entity, prop)?;
    Some(format_propval(doc, val, &fmt))
}

fn attribute_insert(doc: &CadDocument, host: Handle) -> Option<Handle> {
    doc.entities().find_map(|e| match e {
        EntityType::Insert(i) if i.attributes.iter().any(|a| a.common.handle == host) => {
            Some(i.common.handle)
        }
        _ => None,
    })
}

fn named_object_name(doc: &CadDocument, h: Handle) -> Option<String> {
    use crate::tables::TableEntry;
    macro_rules! find_in {
        ($($table:ident),*) => {$(
            if let Some(e) = doc.$table.iter().find(|e| e.handle() == h) {
                return Some(e.name().to_string());
            }
        )*};
    }
    find_in!(layers, line_types, text_styles, block_records, dim_styles, views, ucss, vports, app_ids);
    doc.objects.values().find_map(|o| match o {
        ObjectType::Dictionary(d) => d
            .entries
            .iter()
            .find(|(_, v)| *v == h)
            .map(|(k, _)| k.clone()),
        _ => None,
    })
}

/// Substring strictly between `a` and the next `b` following it.
fn between<'a>(s: &'a str, a: &str, b: &str) -> Option<&'a str> {
    let start = s.find(a)? + a.len();
    let end = s[start..].find(b)? + start;
    Some(&s[start..end])
}

/// The substring immediately after `a` (to the end).
fn after<'a>(s: &'a str, a: &str) -> Option<&'a str> {
    s.find(a).map(|p| &s[p + a.len()..])
}

fn object_property(doc: &CadDocument, e: &EntityType, prop: &str) -> Option<PropVal> {
    use std::f64::consts::PI;
    let c = e.common();
    let point = |v: &crate::types::Vector3| PropVal::Point([v.x, v.y, v.z]);
    Some(match prop {
        "Center" => PropVal::Point(center(e)?),
        "StartPoint" => match e {
            EntityType::Line(l) => point(&l.start),
            _ => return None,
        },
        "EndPoint" => match e {
            EntityType::Line(l) => point(&l.end),
            _ => return None,
        },
        "Radius" => PropVal::Num(radius(e)?),
        "Diameter" => PropVal::Num(2.0 * radius(e)?),
        "Circumference" => PropVal::Num(2.0 * PI * radius(e)?),
        "Area" => PropVal::Num(area(e)?),
        "Length" | "Perimeter" => PropVal::Num(length(e)?),
        "Normal" => match e {
            EntityType::Circle(c) => point(&c.normal),
            EntityType::Arc(a) => point(&a.normal),
            EntityType::Insert(i) => point(&i.normal),
            _ => return None,
        },
        "Thickness" => match e {
            EntityType::Circle(c) => PropVal::Num(c.thickness),
            EntityType::Arc(a) => PropVal::Num(a.thickness),
            EntityType::Line(l) => PropVal::Num(l.thickness),
            _ => return None,
        },
        // Common entity properties.
        "Layer" => PropVal::Text(c.layer.clone()),
        "TrueColor" | "Color" => PropVal::Text(color_text(&c.color)),
        "Linetype" => PropVal::Text(match c.linetype.to_ascii_lowercase().as_str() {
            "" | "bylayer" => "ByLayer".into(),
            "byblock" => "ByBlock".into(),
            _ => c.linetype.clone(),
        }),
        "LinetypeScale" => PropVal::Num(c.linetype_scale),
        "Lineweight" => PropVal::Lineweight(c.line_weight.value()),
        "EntityTransparency" => PropVal::Text(match c.transparency {
            crate::types::Transparency::ByLayer => "ByLayer".into(),
            crate::types::Transparency::ByBlock => "ByBlock".into(),
            crate::types::Transparency::Explicit(a) => {
                ((a as f64) * 100.0 / 255.0).round().to_string()
            }
        }),
        "PlotStyleName" => PropVal::Text(match (c.plotstyle_flags, c.plotstyle_handle) {
            (1, _) => "ByBlock".into(),
            (3, Some(h)) => named_object_name(doc, h)?,
            _ if doc.header.plotstyle_mode => "ByColor".into(),
            _ => "ByLayer".into(),
        }),
        "Material" => PropVal::Text(match (c.material_flags, c.material_handle) {
            (1, _) => "ByBlock".into(),
            (3, Some(h)) => named_object_name(doc, h)?,
            _ => "ByLayer".into(),
        }),
        "ObjectName" => PropVal::Text(
            match e {
                EntityType::Insert(_) => "AcDbBlockReference",
                EntityType::Circle(_) => "AcDbCircle",
                EntityType::Arc(_) => "AcDbArc",
                EntityType::Line(_) => "AcDbLine",
                EntityType::Ellipse(_) => "AcDbEllipse",
                EntityType::MText(_) => "AcDbMText",
                EntityType::Text(_) => "AcDbText",
                EntityType::LwPolyline(_) => "AcDbPolyline",
                EntityType::Table(_) => "AcDbTable",
                _ => return None,
            }
            .into(),
        ),
        // Block reference properties.
        "EffectiveName" | "Name" => match e {
            EntityType::Insert(i) => PropVal::Text(i.block_name.clone()),
            _ => return None,
        },
        "InsertionPoint" | "Position" => match e {
            EntityType::Insert(i) => point(&i.insert_point),
            _ => return None,
        },
        "Rotation" => match e {
            EntityType::Insert(i) => PropVal::Angle(i.rotation),
            _ => return None,
        },
        "XScaleFactor" | "YScaleFactor" | "ZScaleFactor" | "XEffectiveScaleFactor"
        | "YEffectiveScaleFactor" | "ZEffectiveScaleFactor" => match e {
            EntityType::Insert(i) => {
                let s = match &prop[..1] {
                    "X" => i.x_scale(),
                    "Y" => i.y_scale(),
                    _ => i.z_scale(),
                };
                // The effective scale leaves out the block-unit conversion.
                let unit = if prop.contains("Effective") { insert_unit_factor(doc, i) } else { 1.0 };
                PropVal::Num(s / unit)
            }
            _ => return None,
        },
        "InsUnits" => match e {
            EntityType::Insert(i) => {
                let u = doc.block_records.get(&i.block_name).map_or(0, |b| b.units);
                PropVal::Text(INSERT_UNITS.get(u as usize)?.0.into())
            }
            _ => return None,
        },
        "InsUnitsFactor" => match e {
            EntityType::Insert(i) => PropVal::Num(insert_unit_factor(doc, i)),
            _ => return None,
        },
        // View/section-specific properties (StartIdentifier, EndIdentifier,
        // StandardScaleViewLabel, …) live on objects the reader keeps as raw
        // bytes — not evaluable, so the cache is kept.
        _ => return None,
    })
}

const INSERT_UNITS: [(&str, f64); 25] = [
    ("Unitless", 1.0),
    ("Inches", 0.0254),
    ("Feet", 0.3048),
    ("Miles", 1609.344),
    ("Millimeters", 0.001),
    ("Centimeters", 0.01),
    ("Meters", 1.0),
    ("Kilometers", 1000.0),
    ("Microinches", 2.54e-8),
    ("Mils", 2.54e-5),
    ("Yards", 0.9144),
    ("Angstroms", 1e-10),
    ("Nanometers", 1e-9),
    ("Microns", 1e-6),
    ("Decimeters", 0.1),
    ("Dekameters", 10.0),
    ("Hectometers", 100.0),
    ("Gigameters", 1e9),
    ("Astronomical", 1.496e11),
    ("Light Years", 9.4605e15),
    ("Parsecs", 3.084123e16),
    ("US Survey Feet", 1200.0 / 3937.0),
    ("US Survey Inch", 100.0 / 3937.0),
    ("US Survey Yard", 3600.0 / 3937.0),
    ("US Survey Mile", 6336000.0 / 3937.0),
];

fn insert_unit_factor(doc: &CadDocument, i: &crate::entities::Insert) -> f64 {
    let block = doc.block_records.get(&i.block_name).map_or(0, |b| b.units);
    let drawing = doc.header.insertion_units;
    match (INSERT_UNITS.get(block as usize), INSERT_UNITS.get(drawing as usize)) {
        (Some(b), Some(d)) if block > 0 && drawing > 0 => b.1 / d.1,
        _ => 1.0,
    }
}

fn color_text(c: &crate::types::Color) -> String {
    use crate::types::Color;
    const NAMES: [&str; 7] = ["red", "yellow", "green", "cyan", "blue", "magenta", "white"];
    match c {
        Color::ByLayer | Color::None => "BYLAYER".into(),
        Color::ByBlock => "BYBLOCK".into(),
        Color::Index(i @ 1..=7) => NAMES[*i as usize - 1].into(),
        Color::Index(i) => i.to_string(),
        Color::Rgb { r, g, b } => format!("{r},{g},{b}"),
    }
}

fn center(e: &EntityType) -> Option<[f64; 3]> {
    match e {
        EntityType::Circle(c) => Some([c.center.x, c.center.y, c.center.z]),
        EntityType::Arc(a) => Some([a.center.x, a.center.y, a.center.z]),
        EntityType::Ellipse(el) => Some([el.center.x, el.center.y, el.center.z]),
        EntityType::Line(l) => Some([
            (l.start.x + l.end.x) / 2.0,
            (l.start.y + l.end.y) / 2.0,
            (l.start.z + l.end.z) / 2.0,
        ]),
        _ => None,
    }
}

fn radius(e: &EntityType) -> Option<f64> {
    match e {
        EntityType::Circle(c) => Some(c.radius),
        EntityType::Arc(a) => Some(a.radius),
        _ => None,
    }
}

fn area(e: &EntityType) -> Option<f64> {
    use std::f64::consts::PI;
    match e {
        EntityType::Circle(c) => Some(PI * c.radius * c.radius),
        _ => None,
    }
}

fn length(e: &EntityType) -> Option<f64> {
    use std::f64::consts::PI;
    match e {
        EntityType::Line(l) => {
            let (dx, dy, dz) = (
                l.end.x - l.start.x,
                l.end.y - l.start.y,
                l.end.z - l.start.z,
            );
            Some((dx * dx + dy * dy + dz * dz).sqrt())
        }
        EntityType::Circle(c) => Some(2.0 * PI * c.radius),
        EntityType::Arc(a) => {
            let mut sweep = a.end_angle - a.start_angle;
            while sweep < 0.0 {
                sweep += 2.0 * PI;
            }
            Some(a.radius * sweep)
        }
        _ => None,
    }
}

fn format_propval(doc: &CadDocument, val: PropVal, pic: &str) -> String {
    let empty = pic.trim().is_empty();
    match val {
        PropVal::Num(n) | PropVal::Angle(n) if empty => format!("{n:.6}"),
        PropVal::Num(n) | PropVal::Angle(n) => format_number(doc, n, pic),
        PropVal::Point(p) if empty => p.map(|v| format!("{v:.6}")).join(", "),
        PropVal::Point(p) => format_point(doc, p, pic),
        PropVal::Text(s) => text_case(s, pic),
        PropVal::Lineweight(w) if !pic.contains("%lw") => w.to_string(),
        PropVal::Lineweight(w) => format_number(doc, w as f64, pic),
    }
}

const NO_VALUE: &str = "----";

fn format_of(s: &str) -> String {
    let s = s.trim();
    let s = s.strip_prefix('"').unwrap_or(s);
    let s = s.strip_suffix('"').unwrap_or(s);
    s.replace("\\\"", "\"")
}

fn mtext_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
}

fn or_no_value(s: &str) -> String {
    nonempty(s).unwrap_or_else(|| NO_VALUE.into())
}

fn eval_acvar(
    doc: &CadDocument,
    code: &str,
    ctx: &dyn FieldContext,
    host: Handle,
) -> Option<String> {
    // Drop the evaluator word (`\AcVar`, `\AcVar.16.2`).
    let body = code.trim().trim_start_matches('\\');
    let body = body.split_once(char::is_whitespace).map_or("", |(_, r)| r).trim();

    // Hyperlink: `\href "url#location#text to display#flags"` — the text, or
    // the address when the text is empty.
    if let Some(rest) = body.strip_prefix("\\href") {
        let rest = rest.trim_start().strip_prefix('"')?;
        let end = rest.find('"')?;
        let target = &rest[..end];
        let fmt = rest[end + 1..]
            .find("\\f ")
            .map(|fp| format_of(&rest[end + 1 + fp + 3..]))
            .unwrap_or_default();
        let (url, rest) = target.split_once('#').unwrap_or((target, ""));
        let (_location, rest) = rest.split_once('#').unwrap_or(("", rest));
        let shown = rest.rsplit_once('#').map_or(rest, |(text, _flags)| text);
        let shown = if shown.is_empty() { url } else { shown };
        return Some(text_case(shown.to_string(), &fmt));
    }

    let (name, fmt) = match body.find("\\f ") {
        Some(fp) => (body[..fp].trim(), format_of(&body[fp + 3..])),
        None => (body, String::new()),
    };
    let si = &doc.summary_info;
    let saved = doc.source_path.is_some();
    // Without a format, dates use the regional short date.
    let date_fmt = if fmt.is_empty() { "%x" } else { fmt.as_str() };
    let date = |jd: f64| Some(format_dt_in(julian_parts(jd), date_fmt, &ctx.date_locale()));
    let value = match name {
        // System / clock / user.
        "Login" => ctx.login(),
        // The drawing file's own creation / last-write times; a drawing that
        // was never saved shows no value.
        "CreateDate" | "SaveDate" if !saved => Some(NO_VALUE.into()),
        "CreateDate" | "SaveDate" => {
            let create = name == "CreateDate";
            match ctx.file_times() {
                Some((c, m)) => date(if create { c } else { m }),
                None => {
                    let h = &doc.header;
                    let jd = if create { h.create_date_julian } else { h.update_date_julian };
                    // Header dates count the day fraction from midnight.
                    if jd > 0.0 { date(jd - 0.5) } else { Some(NO_VALUE.into()) }
                }
            }
        }
        "Date" => date(ctx.now_julian()),
        // Updated only by plotting; otherwise keep the cached text.
        "PlotDate" if ctx.plotting() => date(ctx.now_julian()),
        "PlotDate" => None,
        // Document summary properties (DWGPROPS / SummaryInfo section).
        "Author" => Some(or_no_value(&si.author)),
        "Title" => Some(or_no_value(&si.title)),
        "Subject" => Some(or_no_value(&si.subject)),
        "Keywords" => Some(or_no_value(&si.keywords)),
        "Comments" => Some(or_no_value(&si.comments)),
        "HyperlinkBase" => Some(or_no_value(&si.hyperlink_base)),
        "RevisionNumber" => Some(or_no_value(&si.revision_number)),
        "LastSavedBy" => Some(
            nonempty(&si.last_saved_by)
                .unwrap_or_else(|| or_no_value(&doc.header.last_saved_by)),
        ),
        // File provenance. An unsaved drawing has only its name (DWGNAME,
        // from the host). Without a format the full path shows (`%fn7`).
        "Filename" | "FileName" => {
            let path = doc.source_path.clone().or_else(|| ctx.getvar("dwgname"));
            let bits = picture_number(&fmt, "%fn").unwrap_or(7);
            Some(
                path.as_deref()
                    .and_then(|p| filename_parts(p, bits))
                    .unwrap_or_else(|| NO_VALUE.into()),
            )
        }
        "FilePath" => filepath(doc),
        // `%by1` bytes, `%by2` kilobytes, `%by3` megabytes; `%.2f` keeps two
        // decimals, otherwise the number is truncated.
        "Filesize" | "FileSize" if !saved => Some(NO_VALUE.into()),
        "Filesize" | "FileSize" => ctx.file_size().map(|n| {
            let unit = picture_number(&fmt, "%by").unwrap_or(1).clamp(1, 3) as i32;
            let v = n as f64 / 1024f64.powi(unit - 1);
            match after(&fmt, "%.").and_then(|s| s.split('f').next()?.parse().ok()) {
                Some(decimals) => format!("{:.*}", decimals, v),
                None => (v.trunc() as u64).to_string(),
            }
        }),
        // Plot settings of the host's layout (the current layout without a host).
        "DeviceName" | "PageSetupName" | "PaperSize" | "PlotOrientation" | "PlotScale"
        | "PlotStyleTable" => {
            let l = host_layout(doc, host, ctx)?;
            Some(match name {
                "DeviceName" if l.plot_printer_name == "none_device" => "None".into(),
                "DeviceName" => or_no_value(&l.plot_printer_name),
                "PageSetupName" => or_no_value(&l.plot_page_name),
                // The media's display name: the canonical name with spaces.
                "PaperSize" => or_no_value(&l.paper_size.replace('_', " ")),
                "PlotOrientation" => PLOT_ORIENTATIONS[(l.plot_rotation & 3) as usize].into(),
                "PlotStyleTable" => or_no_value(&l.plot_style_sheet),
                _ => {
                    let s = if l.plot_scale_denominator != 0.0 {
                        l.plot_scale_numerator / l.plot_scale_denominator
                    } else {
                        1.0
                    };
                    plot_scale_text(doc, s, &fmt)
                }
            })
        }
        // Otherwise a custom document property, else a system variable.
        _ => si
            .custom_properties
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .and_then(|(_, v)| nonempty(v))
            .or_else(|| {
                let v = getvar(doc, name, ctx)?;
                let numeric = ["%lu", "%au", "%pr", "%bl", "%zs", "%qf", "%ct", "%ps"]
                    .iter()
                    .any(|c| fmt.contains(c));
                let nums: Vec<f64> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                Some(match (numeric, nums.as_slice()) {
                    (true, [n]) => format_number(doc, *n, &fmt),
                    // A point variable (`viewctr`, `insbase`, …).
                    (true, [x, y]) => format_point(doc, [*x, *y, 0.0], &fmt),
                    (true, [x, y, z]) => format_point(doc, [*x, *y, *z], &fmt),
                    // Without a format a point shows six decimals per coordinate.
                    (false, [_, _] | [_, _, _]) if fmt.is_empty() => {
                        let mut p = [0.0; 3];
                        p[..nums.len()].copy_from_slice(&nums);
                        p.map(|c| format!("{c:.6}")).join(", ")
                    }
                    _ => v,
                })
            }),
    }?;
    Some(text_case(value, &fmt))
}

pub const PLOT_SCALE_FORMATS: [(&str, &str); 7] = [
    ("(none)", r#"\AcVar PlotScale \f "%lu2%qf2816""#),
    ("#:1", r#"\AcVar PlotScale \f "%lu2%qf2816:1""#),
    ("1:#", r#"\AcVar PlotScale \f "1:%lu2%ct1%qf2816""#),
    (
        "1\" = #'",
        r#"\AcVar.16.2 PlotScale \f "1\" = %lu2%zs12%ct9[0.0833333333333333]'""#,
    ),
    ("#\" = 1'", r#"\AcVar PlotScale \f "%lu4%pr8%ct2%qf2816 = 1'""#),
    ("#\" = 1'-0\"", r#"\AcVar PlotScale \f "%lu4%pr8%ct2%qf2816 = 1'-0\"""#),
    ("Use scale name", r#"\AcVar.16.2 PlotScale \f "%sn""#),
];

pub const FORMULA_FORMATS: [(&str, &str); 7] = [
    ("(none)", ""),
    ("Current units", "%lu6"),
    ("Decimal", "%lu2"),
    ("Architectural", "%lu4"),
    ("Engineering", "%lu3"),
    ("Fractional", "%lu5"),
    ("Scientific", "%lu1"),
];

const PLOT_ORIENTATIONS: [&str; 4] = [
    "Portrait",
    "Landscape",
    "Portrait (upside-down)",
    "Landscape (upside-down)",
];

fn host_layout<'a>(
    doc: &'a CadDocument,
    host: Handle,
    ctx: &dyn FieldContext,
) -> Option<&'a crate::objects::Layout> {
    let layouts = || {
        doc.objects.values().filter_map(|o| match o {
            ObjectType::Layout(l) => Some(l),
            _ => None,
        })
    };
    // An attribute sits on its block reference's layout.
    let host = attribute_insert(doc, host).unwrap_or(host);
    let owner = doc.get_entity(host).map(|e| e.common().owner_handle);
    let ctab = ctx.getvar("ctab");
    layouts()
        .find(|l| Some(l.block_record) == owner)
        .or_else(|| layouts().find(|l| Some(l.name.as_str()) == ctab.as_deref()))
        .or_else(|| layouts().find(|l| l.name.eq_ignore_ascii_case("Model")))
}

pub(crate) fn plot_scale_text(doc: &CadDocument, scale: f64, fmt: &str) -> String {
    if fmt.is_empty() {
        return format!("{:.6}", scale);
    }
    if fmt.contains("%sn") {
        return scale_list(doc)
            .into_iter()
            .find(|s| {
                s.drawing_units != 0.0
                    && (s.paper_units / s.drawing_units - scale).abs() <= 1e-9 * scale.abs().max(1.0)
            })
            .map(|s| s.name.clone())
            .unwrap_or_else(|| format!("{:.6}", scale));
    }
    format_number(doc, scale, fmt)
}

fn scale_list(doc: &CadDocument) -> Vec<&crate::objects::Scale> {
    let scale = |h: &Handle| match doc.objects.get(h) {
        Some(ObjectType::Scale(s)) => Some(s),
        _ => None,
    };
    doc.objects
        .values()
        .find_map(|o| match o {
            ObjectType::Dictionary(d) if d.entries.iter().any(|(_, h)| scale(h).is_some()) => {
                Some(d.entries.iter().filter_map(|(_, h)| scale(h)).collect())
            }
            _ => None,
        })
        .unwrap_or_default()
}

#[derive(Default)]
struct Picture {
    lit: String,
    slot: Option<usize>,
    lu: Option<i64>,
    au: Option<i64>,
    pr: Option<usize>,
    zs: i64,
    qf: i64,
    ds: Option<char>,
    th: Option<char>,
    ls: Option<char>,
    pt: Option<i64>,
    bl: Option<i64>,
    lw: Option<i64>,
    printf: Option<usize>,
    pre: String,
    suf: String,
    conv: Option<(i64, Option<f64>)>,
}

impl Picture {
    fn parse(pic: &str) -> Self {
        let mut p = Picture::default();
        let mut rest = pic;
        while let Some(at) = rest.find('%') {
            p.lit.push_str(&rest[..at]);
            let tail = &rest[at + 1..];
            // `%.2f` — a printf precision.
            if let Some(d) = tail.strip_prefix('.') {
                let digits: String = d.chars().take_while(|c| c.is_ascii_digit()).collect();
                if !digits.is_empty() && d[digits.len()..].starts_with('f') {
                    p.printf = digits.parse().ok();
                    p.slot.get_or_insert(p.lit.len());
                    rest = &d[digits.len() + 1..];
                    continue;
                }
            }
            let key: String = tail.chars().take_while(|c| c.is_ascii_alphabetic()).take(2).collect();
            if key.len() < 2 {
                p.lit.push('%');
                rest = tail;
                continue;
            }
            let after_key = &tail[key.len()..];
            let digits: String = after_key.chars().take_while(|c| c.is_ascii_digit()).collect();
            let mut end = key.len() + digits.len();
            let arg = after_key[digits.len()..]
                .strip_prefix('[')
                .and_then(|a| a.find(']').map(|e| &a[..e]));
            if let Some(a) = arg {
                end += a.len() + 2;
            }
            let n: i64 = digits.parse().unwrap_or(0);
            let ch = || char::from_u32(n as u32);
            match key.as_str() {
                "lu" => p.lu = Some(n),
                "au" => p.au = Some(n),
                "pr" => p.pr = Some(n as usize),
                "zs" => p.zs = n,
                "qf" => p.qf = n,
                "ds" => p.ds = ch(),
                "th" => p.th = ch(),
                "ls" => p.ls = ch(),
                "pt" => p.pt = Some(n),
                "bl" => p.bl = Some(n),
                "lw" => p.lw = Some(n),
                "ct" => p.conv = Some((n, arg.and_then(|a| a.trim().parse().ok()))),
                "ps" => {
                    let a = arg.unwrap_or("");
                    let (x, y) = a.split_once(',').unwrap_or((a, ""));
                    p.pre = x.to_string();
                    p.suf = y.to_string();
                }
                _ => {}
            }
            p.slot.get_or_insert(p.lit.len());
            rest = &tail[end..];
        }
        p.lit.push_str(rest);
        p
    }

    fn zeros(&self) -> Zeros {
        let bit = |zs: i64, qf: i64| self.zs & zs != 0 || self.qf & qf != 0;
        Zeros {
            feet: bit(1, 256),
            inches: bit(2, 512),
            leading: bit(4, 1024),
            trailing: bit(8, 2048),
        }
    }

    fn convert(&self, v: f64) -> f64 {
        let Some((n, f)) = self.conv else { return v };
        let r = if n & 1 != 0 { 1.0 / v } else { v };
        if n & 8 != 0 {
            return r * f.unwrap_or(1.0);
        }
        match n {
            2 => v * 12.0,
            3 => 1.0 / (12.0 * v),
            4 => v / 144.0,
            _ => r,
        }
    }

    fn wrap(&self, value: &str) -> String {
        let num = format!("{}{value}{}", self.pre, self.suf);
        match self.slot {
            Some(at) => format!("{}{}{}", &self.lit[..at], num, &self.lit[at..]),
            None => format!("{}{num}", self.lit),
        }
    }

    fn linear_mode(&self, doc: &CadDocument) -> i64 {
        match self.lu.unwrap_or(2) {
            6 => match doc.header.linear_unit_format as i64 {
                3 | 4 if self.qf & 1 != 0 => 2,
                m => m,
            },
            m => m,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Zeros {
    feet: bool,
    inches: bool,
    leading: bool,
    trailing: bool,
}

fn format_number(doc: &CadDocument, value: f64, pic: &str) -> String {
    let p = Picture::parse(pic);
    if let Some(mode) = p.bl {
        let names = match mode {
            1 => ["True", "False"],
            2 => ["Yes", "No"],
            3 => ["On", "Off"],
            4 => ["Enabled", "Disabled"],
            _ => return p.wrap(&num_str(value)),
        };
        return p.wrap(names[(value == 0.0) as usize]);
    }
    if let Some(unit) = p.lw {
        let named = match value as i64 {
            -1 => Some("ByLayer"),
            -2 => Some("ByBlock"),
            -3 => Some("Default"),
            _ => None,
        };
        if let Some(n) = named {
            return n.into();
        }
        let mm = value / 100.0;
        let v = if unit == 2 { mm / 25.4 } else { mm };
        return p.wrap(&format!("{:.*}", p.printf.unwrap_or(6), v));
    }
    let v = p.convert(value);
    if !v.is_finite() {
        return "####".into();
    }
    p.wrap(&number_text(doc, &p, v))
}

fn number_text(doc: &CadDocument, p: &Picture, v: f64) -> String {
    let z = p.zeros();
    if let Some(au) = p.au {
        let mode = if au == 5 { doc.header.angular_unit_format as i64 } else { au };
        let prec = p.pr.unwrap_or(doc.header.angular_unit_precision.max(0) as usize);
        return angle_text(v, mode, prec.min(16), z, true);
    }
    let prec = p.pr.unwrap_or(doc.header.linear_unit_precision.max(0) as usize).min(16);
    // `%qf2` / `%qf4`: an angle in radians whose `%lu` number is the angle
    // mode, normalised to one turn with 2, as is with 4.
    if p.qf & 6 != 0 {
        return angle_text(v, p.lu.unwrap_or(2), prec, z, p.qf & 2 != 0);
    }
    unit_text(v, p.linear_mode(doc), prec, z, p.ds.unwrap_or('.'), p.th)
}

fn format_point(doc: &CadDocument, pt: [f64; 3], pic: &str) -> String {
    let p = Picture::parse(pic);
    let bits = p.pt.unwrap_or(7);
    let sep = format!("{} ", p.ls.unwrap_or(','));
    let parts: Vec<String> = (0..3)
        .filter(|i| bits & (1 << i) != 0)
        .map(|i| number_text(doc, &p, p.convert(pt[i])))
        .collect();
    p.wrap(&parts.join(&sep))
}

fn unit_text(v: f64, mode: i64, prec: usize, z: Zeros, ds: char, th: Option<char>) -> String {
    let sign = if v < 0.0 { "-" } else { "" };
    let a = v.abs();
    let decimal = |x: f64, p: usize| {
        let m = 10f64.powi(p as i32);
        let mut s = format!("{:.*}", p, (x * m).round() / m);
        if let Some(t) = th {
            let (int, frac) = s.split_at(s.find('.').unwrap_or(s.len()));
            let mut grouped = String::new();
            for (i, c) in int.chars().enumerate() {
                if i > 0 && (int.len() - i) % 3 == 0 {
                    grouped.push(t);
                }
                grouped.push(c);
            }
            s = grouped + frac;
        }
        if z.trailing && s.contains('.') {
            s = s.trim_end_matches('0').trim_end_matches('.').to_string();
        }
        if z.leading && s.starts_with("0.") {
            s.remove(0);
        }
        s.replace('.', &ds.to_string())
    };
    // Whole units and a reduced 1/2^prec fraction.
    let fraction = |x: f64| {
        let den = 1i64 << prec.min(8);
        let n = (x * den as f64).round() as i64;
        let (whole, mut num, mut den) = (n / den, n % den, den);
        while num != 0 && num % 2 == 0 {
            num /= 2;
            den /= 2;
        }
        (whole, num, den)
    };
    match mode {
        1 => {
            // The mantissa rounds half away from zero (1234.5 → 1.235E+03 at
            // three decimals), on the value scaled by an exact power of ten.
            let mut e = if a > 0.0 { a.log10().floor() as i32 } else { 0 };
            let scaled = |e: i32| {
                let k = prec as i32 - e;
                if k >= 0 { (a * 10f64.powi(k)).round() } else { (a / 10f64.powi(-k)).round() }
            };
            let mut n = scaled(e);
            if n >= 10f64.powi(prec as i32 + 1) {
                e += 1;
                n = scaled(e);
            }
            let digits = format!("{:01$.0}", n, prec + 1);
            let m = match prec {
                0 => digits,
                _ => format!("{}{ds}{}", &digits[..1], &digits[1..]),
            };
            format!("{sign}{m}E{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
        }
        3 => {
            let m = 10f64.powi(prec as i32);
            let total = (a * m).round() / m;
            let feet = (total / 12.0).trunc();
            let inches = total - feet * 12.0;
            if z.feet && feet == 0.0 {
                format!("{sign}{}\"", decimal(inches, prec))
            } else if z.inches && inches == 0.0 {
                format!("{sign}{feet}'")
            } else {
                format!("{sign}{feet}'-{}\"", decimal(inches, prec))
            }
        }
        4 => {
            let (inches, num, den) = fraction(a);
            let (feet, inch) = (inches / 12, inches % 12);
            let frac = if num != 0 { format!("{num}/{den}") } else { String::new() };
            let inch_text = |drop_zero: bool| match (inch, frac.is_empty()) {
                (0, false) if drop_zero => frac.clone(),
                (i, false) => format!("{i} {frac}"),
                (i, true) => i.to_string(),
            };
            if z.feet && feet == 0 {
                format!("{sign}{}\"", inch_text(true))
            } else if z.inches && inch == 0 && num == 0 {
                format!("{sign}{feet}'")
            } else {
                format!("{sign}{feet}'-{}\"", inch_text(false))
            }
        }
        5 => {
            let (whole, num, den) = fraction(a);
            match (whole, num) {
                (w, 0) => format!("{sign}{w}"),
                (0, n) => format!("{sign}{n}/{den}"),
                (w, n) => format!("{sign}{w} {n}/{den}"),
            }
        }
        _ => format!("{sign}{}", decimal(a, prec)),
    }
}

fn angle_text(rad: f64, mode: i64, prec: usize, z: Zeros, normalize: bool) -> String {
    use std::f64::consts::TAU;
    let a = if normalize { rad.rem_euclid(TAU) } else { rad };
    let fixed = |x: f64| {
        let mut s = format!("{x:.prec$}");
        if z.trailing && s.contains('.') {
            s = s.trim_end_matches('0').trim_end_matches('.').to_string();
        }
        if z.leading {
            if let Some(i) = s.find("0.").filter(|&i| s[..i].chars().all(|c| c == '-')) {
                s.remove(i);
            }
        }
        s
    };
    let dms = |deg: f64| match prec {
        0 => format!("{}d", deg.round()),
        1 | 2 => {
            let m = (deg * 60.0).round() as i64;
            format!("{}d{}'", m / 60, m % 60)
        }
        _ => {
            let k = prec.saturating_sub(4);
            let scale = 10f64.powi(k as i32);
            let s = (deg * 3600.0 * scale).round() / scale;
            let whole = s.trunc() as i64;
            let sec = s - (whole - whole % 60) as f64;
            format!("{}d{}'{:.k$}\"", whole / 3600, whole % 3600 / 60, sec)
        }
    };
    match mode {
        1 => dms(a.to_degrees()),
        2 => format!("{}g", fixed(a * 200.0 / std::f64::consts::PI)),
        3 => format!("{}r", fixed(a)),
        4 => {
            let deg = a.to_degrees();
            let (ns, bearing, ew) = if deg <= 90.0 {
                ('N', 90.0 - deg, 'E')
            } else if deg <= 180.0 {
                ('N', deg - 90.0, 'W')
            } else if deg <= 270.0 {
                ('S', 270.0 - deg, 'W')
            } else {
                ('S', deg - 270.0, 'E')
            };
            if prec > 0 && (bearing - 90.0).abs() < 1e-9 {
                ew.to_string()
            } else if prec > 0 && bearing.abs() < 1e-9 {
                ns.to_string()
            } else {
                format!("{ns} {} {ew}", dms(bearing))
            }
        }
        _ => fixed(a.to_degrees()),
    }
}

fn picture_number(pic: &str, key: &str) -> Option<u32> {
    after(pic, key)?
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .ok()
}

fn text_case(s: String, pic: &str) -> String {
    match picture_number(pic, "%tc") {
        Some(1) => s.to_uppercase(),
        Some(2) => s.to_lowercase(),
        Some(3) => {
            let mut c = s.chars();
            match c.next() {
                Some(f) => f.to_uppercase().chain(c).collect(),
                None => s,
            }
        }
        Some(4) => {
            let mut out = String::with_capacity(s.len());
            let mut start = true;
            for c in s.chars() {
                if start {
                    out.extend(c.to_uppercase());
                } else {
                    out.push(c);
                }
                start = c.is_whitespace();
            }
            out
        }
        _ => s,
    }
}

fn nonempty(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}


fn filename_parts(p: &str, bits: u32) -> Option<String> {
    let (dir, sep, base) = match p.rfind(['/', '\\']) {
        Some(i) => (&p[..i], &p[i..i + 1], &p[i + 1..]),
        None => ("", "\\", p),
    };
    let (stem, ext) = match base.rfind('.') {
        Some(i) => (&base[..i], &base[i + 1..]),
        None => (base, ""),
    };
    let mut s = String::new();
    if bits & 1 != 0 {
        s.push_str(dir);
    }
    if bits & 2 != 0 {
        if !s.is_empty() {
            s.push_str(sep);
        }
        s.push_str(stem);
    }
    if bits & 4 != 0 {
        if !s.is_empty() {
            s.push('.');
        }
        s.push_str(ext);
    }
    nonempty(&s)
}
fn filepath(doc: &CadDocument) -> Option<String> {
    doc.source_path.as_deref().and_then(nonempty)
}

// ── DIESEL ─────────────────────────────────────────────────────────────────

fn diesel_eval(doc: &CadDocument, s: &str, ctx: &dyn FieldContext) -> Option<String> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '$' && i + 1 < chars.len() && chars[i + 1] == '(' {
            let mut depth = 0;
            let mut j = i + 1;
            while j < chars.len() {
                match chars[j] {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            let inner: String = chars[i + 2..j].iter().collect();
            out.push_str(&diesel_macro(doc, &inner, ctx)?);
            i = j + 1;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    Some(out)
}

/// set; an unknown function concatenates its arguments (DIESEL's own fallback).
fn diesel_macro(doc: &CadDocument, inner: &str, ctx: &dyn FieldContext) -> Option<String> {
    let parts = split_top_commas(inner);
    let func = parts[0].trim().to_lowercase();
    let raw: Vec<&str> = parts[1..].iter().map(|s| s.trim()).collect();
    let ev = |i: usize| -> Option<String> {
        raw.get(i)
            .and_then(|a| diesel_eval(doc, a.trim_matches('"'), ctx))
    };

    // Control-flow functions evaluate their branches lazily.
    match func.as_str() {
        "if" => {
            let cond = ev(0)?;
            let take = cond
                .trim()
                .parse::<f64>()
                .map(|n| n != 0.0)
                .unwrap_or(!cond.trim().is_empty());
            return if take {
                ev(1)
            } else if raw.len() > 2 {
                ev(2)
            } else {
                Some(String::new())
            };
        }
        "nth" => {
            let w: usize = ev(0)?.trim().parse().ok()?;
            return ev(1 + w).or(Some(String::new()));
        }
        "index" => {
            let w: usize = ev(0)?.trim().parse().ok()?;
            return Some(ev(1)?.split(',').nth(w).unwrap_or("").to_string());
        }
        _ => {}
    }

    // Everything else evaluates all arguments first.
    let mut args = Vec::with_capacity(raw.len());
    for i in 0..raw.len() {
        args.push(ev(i)?);
    }
    let a = |i: usize| args.get(i).cloned().unwrap_or_default();
    let n = |i: usize| a(i).trim().parse::<f64>().unwrap_or(0.0);
    let ni = |i: usize| a(i).trim().parse::<i64>().unwrap_or(0);

    Some(match func.as_str() {
        "getvar" => return getvar(doc, &a(0), ctx),
        "getenv" => ctx.getenv(&a(0)).unwrap_or_default(),
        "eval" => return diesel_eval(doc, &a(0), ctx),
        "substr" => {
            let chars: Vec<char> = a(0).chars().collect();
            let s0 = args
                .get(1)
                .and_then(|x| x.parse::<usize>().ok())
                .unwrap_or(1)
                .saturating_sub(1);
            let end = match args.get(2).and_then(|x| x.parse::<usize>().ok()) {
                Some(l) => (s0 + l).min(chars.len()),
                None => chars.len(),
            };
            chars
                .get(s0..end)
                .map(|c| c.iter().collect())
                .unwrap_or_default()
        }
        "strlen" => a(0).chars().count().to_string(),
        "upper" => a(0).to_uppercase(),
        "strfill" => a(0).repeat(ni(1).max(0) as usize),
        "eq" => bool_str(a(0) == a(1)),
        "=" => bool_str(n(0) == n(1)),
        "!=" => bool_str(n(0) != n(1)),
        "<" => bool_str(n(0) < n(1)),
        ">" => bool_str(n(0) > n(1)),
        "<=" => bool_str(n(0) <= n(1)),
        ">=" => bool_str(n(0) >= n(1)),
        "and" => (0..args.len())
            .map(&ni)
            .fold(!0i64, |acc, x| acc & x)
            .to_string(),
        "or" => (0..args.len())
            .map(&ni)
            .fold(0i64, |acc, x| acc | x)
            .to_string(),
        "xor" => (0..args.len())
            .map(&ni)
            .fold(0i64, |acc, x| acc ^ x)
            .to_string(),
        "+" => num_str((0..args.len()).map(&n).sum()),
        "*" => num_str((0..args.len()).map(&n).product()),
        "-" => num_str((1..args.len()).map(&n).fold(n(0), |acc, x| acc - x)),
        "/" => num_str(
            (1..args.len())
                .map(&n)
                .fold(n(0), |acc, x| if x != 0.0 { acc / x } else { acc }),
        ),
        "fix" => (n(0).trunc() as i64).to_string(),
        "rtos" => rtos(n(0), args.get(2).and_then(|x| x.parse().ok())),
        "angtos" => angtos(n(0), ni(1), args.get(2).and_then(|x| x.parse().ok())),
        "edtime" => edtime(n(0), &a(1)),
        "time" => (((ctx.now_julian() - 2_440_587.5) * 86_400.0).round() as i64).to_string(),
        // Unknown function — DIESEL concatenates the evaluated arguments.
        _ => args.join(""),
    })
}

/// AutoCAD system variables the engine answers directly (from the document +
/// context clock); anything else is delegated to the host via
/// [`FieldContext::getvar`]. `None` keeps the cached field text.
fn getvar(doc: &CadDocument, name: &str, ctx: &dyn FieldContext) -> Option<String> {
    let key = name.trim().trim_start_matches('*').to_lowercase();
    match key.as_str() {
        "cdate" => {
            let (y, mo, d, h, mi, s) = julian_parts(ctx.now_julian());
            Some(format!(
                "{:04}{:02}{:02}.{:02}{:02}{:02}",
                y, mo, d, h, mi, s
            ))
        }
        // DATE counts the day fraction from midnight.
        "date" => Some(format!("{:.6}", ctx.now_julian() + 0.5)),
        "loginname" => ctx.login(),
        "tdcreate" => Some(format!("{:.6}", doc.header.create_date_julian)),
        "tducreate" => Some(format!("{:.6}", doc.header.universal_create_or_local())),
        "tdupdate" => Some(format!("{:.6}", doc.header.update_date_julian)),
        "tduupdate" => Some(format!("{:.6}", doc.header.universal_update_or_local())),
        _ => ctx.getvar(name),
    }
}

fn split_top_commas(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '(' => {
                depth += 1;
                cur.push(c);
            }
            ')' => {
                depth -= 1;
                cur.push(c);
            }
            ',' if depth == 0 => {
                parts.push(cur.clone());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    parts.push(cur);
    parts
}

fn bool_str(b: bool) -> String {
    if b { "1" } else { "0" }.to_string()
}

// ── pure numeric / date formatting ─────────────────────────────────────────

fn num_str(x: f64) -> String {
    if x.fract() == 0.0 && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else {
        format!("{}", x)
    }
}

/// `$(rtos,value[,mode,prec])` — decimal with `prec` fractional digits (4).
fn rtos(val: f64, prec: Option<usize>) -> String {
    format!("{:.*}", prec.unwrap_or(4), val)
}

/// `$(angtos,value[,mode,prec])` — `value` in radians; mode 3 = radians, else degrees.
fn angtos(val: f64, mode: i64, prec: Option<usize>) -> String {
    angle_text(val, mode, prec.unwrap_or(0), Zeros::default(), true)
}

/// Gregorian (Y, M, D, h, m, s) for an astronomical Julian date.
pub fn julian_parts(jd: f64) -> (i64, u32, u32, u32, u32, u32) {
    if jd <= 0.0 {
        return (0, 1, 1, 0, 0, 0);
    }
    let secs = ((jd - 2_440_587.5) * 86_400.0).round() as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // days → civil date (Howard Hinnant).
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d as u32, h as u32, mi as u32, s as u32)
}

/// Day of week for a Julian date: 0 = Sunday … 6 = Saturday.
pub(crate) fn weekday(jd: f64) -> u32 {
    let secs = ((jd - 2_440_587.5) * 86_400.0).round() as i64;
    let days = secs.div_euclid(86_400);
    (days + 4).rem_euclid(7) as u32
}

/// Format (Y, M, D, h, m, s) with a .NET-style picture (`yyyy`, `yy`, `MM`,
/// `dd`, `HH`, `mm`, `ss`) — used by `\AcVar … \f "…"`.
pub fn format_dt(dt: (i64, u32, u32, u32, u32, u32), fmt: &str) -> String {
    format_dt_in(dt, fmt, &DateLocale::default())
}

pub fn format_dt_in(dt: (i64, u32, u32, u32, u32, u32), fmt: &str, loc: &DateLocale) -> String {
    let regional = match fmt {
        "%x" => Some(loc.short_date.clone()),
        "%#x" => Some(loc.long_date.clone()),
        "%X" | "%#X" => Some(loc.long_time.clone()),
        "%c" => Some(format!("{} {}", loc.short_date, loc.long_time)),
        "%#c" => Some(format!("{} {}", loc.long_date, loc.long_time)),
        _ => None,
    };
    let fmt = regional.as_deref().unwrap_or(fmt);
    let (y, mo, d, h, mi, s) = dt;
    let h12 = if h % 12 == 0 { 12 } else { h % 12 };
    // Day of week from the civil date (0 = Sunday).
    let wd = {
        let (yy, mm) = if mo <= 2 { (y - 1, mo + 12) } else { (y, mo) };
        let k = yy.rem_euclid(100);
        let j = yy.div_euclid(100);
        let zeller = (d as i64 + (13 * (mm as i64 + 1)) / 5 + k + k / 4 + j / 4 + 5 * j) % 7;
        ((zeller + 6) % 7) as usize // Zeller: 0 = Saturday
    };
    let month = (mo as usize).clamp(1, 12) - 1;
    let chars: Vec<char> = fmt.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' || c == '"' {
            let end = chars[i + 1..].iter().position(|&x| x == c).map_or(chars.len(), |p| i + 1 + p);
            out.extend(&chars[i + 1..end]);
            i = end + 1;
            continue;
        }
        if c == '\\' && i + 1 < chars.len() {
            out.push(chars[i + 1]);
            i += 2;
            continue;
        }
        let mut n = 1;
        while i + n < chars.len() && chars[i + n] == c {
            n += 1;
        }
        let ampm = if h < 12 { &loc.am } else { &loc.pm };
        match c {
            'd' => out.push_str(&match n {
                1 => d.to_string(),
                2 => format!("{:02}", d),
                3 => loc.days_abbr[wd].clone(),
                _ => loc.days[wd].clone(),
            }),
            'M' => out.push_str(&match n {
                1 => mo.to_string(),
                2 => format!("{:02}", mo),
                3 => loc.months_abbr[month].clone(),
                _ => loc.months[month].clone(),
            }),
            'y' => out.push_str(&match n {
                1 => (y % 100).unsigned_abs().to_string(),
                2 => format!("{:02}", (y % 100).unsigned_abs()),
                _ => format!("{:0width$}", y, width = n),
            }),
            'h' => out.push_str(&if n == 1 { h12.to_string() } else { format!("{:02}", h12) }),
            'H' => out.push_str(&if n == 1 { h.to_string() } else { format!("{:02}", h) }),
            'm' => out.push_str(&if n == 1 { mi.to_string() } else { format!("{:02}", mi) }),
            's' => out.push_str(&if n == 1 { s.to_string() } else { format!("{:02}", s) }),
            't' => {
                if n == 1 {
                    out.extend(ampm.chars().next());
                } else {
                    out.push_str(ampm);
                }
            }
            _ => {
                out.extend(&chars[i..i + n]);
            }
        }
        i += n;
    }
    out
}

/// `$(edtime,time,picture)` — format the Julian `time` per a DIESEL picture
/// (case-sensitive tokens; longest match first). `MM` = minutes, `MO` = month.
fn edtime(jd: f64, pic: &str) -> String {
    let (y, mo, d, h, mi, s) = julian_parts(jd);
    let wd = weekday(jd);
    let ampm = pic.contains("AM/PM")
        || pic.contains("am/pm")
        || pic.contains("A/P")
        || pic.contains("a/p");
    let hh = if ampm {
        let x = h % 12;
        if x == 0 {
            12
        } else {
            x
        }
    } else {
        h
    };
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    const DAYS: [&str; 7] = [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ];
    let month = MONTHS[(mo as usize).clamp(1, 12) - 1];
    let day = DAYS[(wd as usize) % 7];
    let toks: [(&str, String); 17] = [
        ("MONTH", month.to_string()),
        ("MON", month[..3].to_string()),
        ("MO", format!("{:02}", mo)),
        ("DDDD", day.to_string()),
        ("DDD", day[..3].to_string()),
        ("DD", format!("{:02}", d)),
        ("YYYY", format!("{:04}", y)),
        ("YY", format!("{:02}", (y % 100).unsigned_abs())),
        ("HH", format!("{:02}", hh)),
        ("MM", format!("{:02}", mi)),
        ("SS", format!("{:02}", s)),
        ("AM/PM", if h < 12 { "AM" } else { "PM" }.to_string()),
        ("am/pm", if h < 12 { "am" } else { "pm" }.to_string()),
        ("A/P", if h < 12 { "A" } else { "P" }.to_string()),
        ("a/p", if h < 12 { "a" } else { "p" }.to_string()),
        ("M", format!("{}", mo)),
        ("D", format!("{}", d)),
    ];
    let chars: Vec<char> = pic.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    'scan: while i < chars.len() {
        for (tok, val) in &toks {
            let tl = tok.chars().count();
            if i + tl <= chars.len() && chars[i..i + tl].iter().collect::<String>() == *tok {
                out.push_str(val);
                i += tl;
                continue 'scan;
            }
        }
        if chars[i] == 'H' {
            out.push_str(&format!("{}", hh));
            i += 1;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

// ── authoring ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct NewField {
    pub evaluator: String,
    pub code: String,
    pub objects: Vec<Handle>,
    pub value: CellValue,
    pub evaluation_option: i32,
}

impl NewField {
    pub fn new(code: impl Into<String>, display: impl Into<String>) -> Self {
        let code = code.into();
        let display = display.into();
        let evaluator = code_evaluator(&code).to_string();
        let mut value = CellValue::text(&display);
        value.flags = 4;
        value.format = code_format(&code).to_string();
        // The reference application leaves the Date field on demand only.
        // A count is an integer value (`####` while its code is empty).
        if evaluator.starts_with("AcCount") {
            if let Ok(n) = display.parse::<i64>() {
                value = CellValue::integer(n);
                value.flags = 4;
            }
        }
        let evaluation_option = if evaluator == "AcVar" && code_word(&code, 1) == Some("Date") {
            32
        } else {
            63
        };
        Self {
            evaluator,
            code,
            objects: Vec::new(),
            value,
            evaluation_option,
        }
    }
}

fn code_evaluator(code: &str) -> &str {
    code_word(code, 0).unwrap_or("")
}

fn code_word(code: &str, n: usize) -> Option<&str> {
    code.trim().trim_start_matches('\\').split_whitespace().nth(n)
}

fn code_format(code: &str) -> &str {
    code.find("\\f ")
        .map(|p| code[p + 3..].trim().trim_matches('"'))
        .unwrap_or("")
}

fn field_text_checksum(text: &str) -> f64 {
    text.encode_utf16()
        .enumerate()
        .map(|(i, u)| (i as f64 + 1.0) * u as f64)
        .sum()
}

#[derive(Clone, Copy, PartialEq)]
enum TextHostKind {
    MText,
    Text,
    AttributeDefinition,
    Attribute,
}

impl CadDocument {
    pub fn set_text_field(
        &mut self,
        host: Handle,
        template: &str,
        children: Vec<NewField>,
    ) -> Option<Handle> {
        let kind = self.text_host(host, |_, _, kind| kind)?;
        let display = template_display(template, &children, kind == TextHostKind::MText)?;
        self.remove_text_field(host);

        for (dxf, cpp) in [("FIELD", "AcDbField"), ("FIELDLIST", "AcDbFieldList")] {
            if !self.classes.contains(dxf) {
                let mut class = crate::classes::DxfClass::new(dxf, cpp);
                class.proxy_flags = crate::classes::ProxyFlags(1152);
                self.classes.add_or_update(class);
            }
        }

        // ACAD_XDICTIONARY of the host (reuse an existing one).
        let existing = self.text_host(host, |common, _, _| common.xdictionary_handle)?;
        let xdict = match existing {
            Some(h) if matches!(self.objects.get(&h), Some(ObjectType::Dictionary(_))) => h,
            _ => {
                let h = self.allocate_handle();
                let mut d = Dictionary::new();
                d.handle = h;
                d.owner = host;
                d.hard_owner = true;
                self.objects.insert(h, ObjectType::Dictionary(d));
                h
            }
        };
        let field_dict = self.allocate_handle();
        let mut d = Dictionary::new();
        d.handle = field_dict;
        d.owner = xdict;
        d.hard_owner = true;
        d.reactors = vec![xdict];
        let container = self.allocate_handle();
        d.add_entry("TEXT", container);
        self.objects.insert(field_dict, ObjectType::Dictionary(d));
        if let Some(ObjectType::Dictionary(x)) = self.objects.get_mut(&xdict) {
            x.add_entry("ACAD_FIELD", field_dict);
        }

        let child_handles: Vec<Handle> = children.iter().map(|_| self.allocate_handle()).collect();
        if children.iter().any(|c| hyperlink_xdata(&c.code).is_some())
            && self.app_ids.get("PE_URL").is_none()
        {
            use crate::tables::TableEntry;
            let mut app = crate::tables::AppId::new("PE_URL");
            app.set_handle(self.allocate_handle());
            self.app_ids.add(app).ok();
        }

        let mut checksum = CellValue::number(field_text_checksum(&display));
        checksum.flags = 2;
        checksum.formatted_value.clear();
        let mut child_values = Vec::new();
        if kind == TextHostKind::AttributeDefinition {
            let mut v = CellValue::integer(1);
            v.flags = 2;
            v.formatted_value.clear();
            child_values.push(FieldChildValue {
                key: "ACFD_FIELDTEXT_ATTDEF".into(),
                value: v,
            });
        }
        child_values.push(FieldChildValue {
            key: "ACFD_FIELDTEXT_CHECKSUM".into(),
            value: checksum,
        });
        let mut empty = CellValue::new();
        empty.flags = 3;
        let container_field = Field {
            handle: container,
            owner: field_dict,
            evaluator_id: "_text".into(),
            code: template.into(),
            child_fields: child_handles.clone(),
            evaluation_option: 63,
            state: if kind == TextHostKind::MText { 13 } else { 9 },
            evaluation_status: 2,
            value: empty,
            child_values,
            ..Field::default()
        };
        let mut all = vec![container_field];
        for (child, handle) in children.into_iter().zip(&child_handles) {
            all.push(child_field(child, *handle, container));
        }

        self.register_fields(all);

        self.text_host(host, |common, text, _| {
            common.xdictionary_handle = Some(xdict);
            *text = display;
        })?;
        Some(container)
    }

    fn table_block_plan(
        &self,
        t: &Table,
        extra: &[(usize, usize)],
    ) -> (Vec<((usize, usize), crate::entities::MText)>, Vec<crate::entities::Line>, Vec<crate::entities::Solid>) {
        use crate::entities::table::CellStylePropertyFlags as P;
        use crate::entities::{Line, MText, Solid};
        use crate::types::{Color, LineWeight, Transparency, Vector3};
        let style = t.table_style_handle.and_then(|h| match self.objects.get(&h) {
            Some(ObjectType::TableStyle(s)) => Some(s.clone()),
            _ => None,
        });
        let margin = style.as_ref().map_or(0.06, |s| s.horizontal_margin);
        let legacy = t.legacy_style_override.clone().unwrap_or_default();
        let title = !legacy.title_suppressed.unwrap_or(false);
        let header = !legacy.header_suppressed.unwrap_or(false);
        // (text height, alignment) of a row by its kind: title, header, data.
        let row_style = |r: usize| {
            // Newer tables name each row's cell style (1 title, 2 header, 3 data).
            let named_kind = t.rows.get(r).map(|row| row.style_id).filter(|id| (1..=3).contains(id));
            let kind = if let Some(id) = named_kind {
                (id - 1) as usize
            } else if title && r == 0 {
                0
            } else if header && r == usize::from(title) {
                1
            } else {
                2
            };
            style.as_ref().map_or((0.18, 5), |s| {
                // Newer styles keep their rows as the named cell styles.
                let named = s.modern_overrides.iter().find(|(_, n)| {
                    n.name.eq_ignore_ascii_case(["_TITLE", "_HEADER", "_DATA"][kind])
                });
                match named {
                    Some((_, n)) if n.cell_style.content_format.text_height > 0.0 => (
                        n.cell_style.content_format.text_height,
                        n.cell_style.content_format.cell_alignment as i32,
                    ),
                    _ => {
                        let rs = [&s.title_row_style, &s.header_row_style, &s.data_row_style][kind];
                        (rs.text_height, rs.alignment as i32)
                    }
                }
            })
        };
        let by_block = |c: &mut EntityCommon| {
            c.layer = "0".into();
            c.color = Color::ByBlock;
            c.transparency = Transparency::ByBlock;
        };
        let widths: Vec<f64> = t.columns.iter().map(|c| c.width).collect();
        let heights: Vec<f64> = t.rows.iter().map(|r| r.height).collect();
        let (nr, nc) = (heights.len(), widths.len());
        let xs: Vec<f64> = std::iter::once(0.0).chain(widths.iter().scan(0.0, |a, w| { *a += w; Some(*a) })).collect();
        let ys: Vec<f64> = std::iter::once(0.0).chain(heights.iter().scan(0.0, |a, h| { *a += h; Some(*a) })).collect();
        // Merged regions: the top-left cell spans its merge width and height;
        // `owner[r][c]` is the cell a grid square belongs to.
        let mut owner: Vec<Vec<(usize, usize)>> = (0..nr).map(|r| (0..nc).map(|c| (r, c)).collect()).collect();
        let mut span = vec![vec![(1usize, 1usize); nc]; nr];
        // Merges come as the table's merged ranges (binary files) or as the
        // origin cell's merge width and height (DXF).
        let mut regions: Vec<(usize, usize, usize, usize)> = t
            .merged_ranges
            .iter()
            .map(|m| (m.top_row, m.left_col, m.col_count(), m.row_count()))
            .collect();
        for (r, row) in t.rows.iter().enumerate() {
            for (c, cell) in row.cells.iter().enumerate().take(nc) {
                let (w, h) = (cell.merge_width.max(1) as usize, cell.merge_height.max(1) as usize);
                if w > 1 || h > 1 {
                    regions.push((r, c, w, h));
                }
            }
        }
        for (r, c, w, h) in regions {
            if r >= nr || c >= nc || owner[r][c] != (r, c) || (w <= 1 && h <= 1) {
                continue;
            }
            span[r][c] = (w.min(nc - c), h.min(nr - r));
            for rr in r..(r + h).min(nr) {
                for cc in c..(c + w).min(nc) {
                    owner[rr][cc] = (r, c);
                }
            }
        }
        let vmargin = style.as_ref().map_or(0.06, |s| s.vertical_margin);
        let mut texts = Vec::new();
        let mut fills = Vec::new();
        for (r, row) in t.rows.iter().enumerate() {
            for c in 0..nc {
                if owner[r][c] != (r, c) {
                    continue;
                }
                let cell = row.cells.get(c);
                // A cell's own override (text height, style, colour, alignment,
                // background) wins over its row's style.
                // (DXF and binary cells say it differently; the binary layout
                // keeps text height, style and colour on the content.)
                let laid = cell.map(|cell| cell.binary_layout());
                let own = laid.as_deref().and_then(|cell| cell.style.as_ref());
                let own_content = laid.as_deref().and_then(|cell| cell.contents.first());
                let style_sets = |p: P| own.filter(|s| s.sets(p));
                let content_sets = |p: P| own_content.filter(|c| c.sets(p));
                if let Some(s) = style_sets(P::BACKGROUND_COLOR).filter(|s| s.fill_enabled) {
                    let (w, h) = span[r][c];
                    let (x0, x1, y0, y1) = (xs[c], xs[c + w], -ys[r], -ys[r + h]);
                    let mut solid = Solid::new(
                        Vector3::new(x0, y0, 0.0),
                        Vector3::new(x1, y0, 0.0),
                        Vector3::new(x0, y1, 0.0),
                        Vector3::new(x1, y1, 0.0),
                    );
                    by_block(&mut solid.common);
                    solid.common.color = s.background_color.clone();
                    fills.push(solid);
                }
                let text = cell.and_then(|cell| cell.contents.first()).map(|ct| ct.value.display().to_string()).unwrap_or_default();
                let has_field = cell.and_then(|cell| cell.contents.first()).is_some_and(|ct| ct.field_handle.is_some());
                if text.is_empty() && !has_field && !extra.contains(&(r, c)) {
                    continue;
                }
                let (mut height, mut align) = row_style(r);
                // A cell overriding its style brings its own alignment.
                if let Some(s) = style_sets(P::ALIGNMENT).filter(|s| (1..=9).contains(&s.alignment)) {
                    align = s.alignment;
                }
                if let Some(c) = content_sets(P::TEXT_HEIGHT).filter(|c| c.text_height > 0.0) {
                    height = c.text_height;
                }
                let (w, h) = span[r][c];
                let (x0, x1, y0, y1) = (xs[c], xs[c + w], ys[r], ys[r + h]);
                // The text sits where its alignment puts it inside the margins.
                let a = (align.clamp(1, 9) - 1) as usize;
                let x = [x0 + margin, (x0 + x1) / 2.0, x1 - margin][a % 3];
                let y = [-(y0 + vmargin), -(y0 + y1) / 2.0, -(y1 - vmargin)][a / 3];
                let mut m = MText::new();
                by_block(&mut m.common);
                m.value = text;
                m.height = height;
                m.rectangle_width = (x1 - x0) - 2.0 * margin;
                m.insertion_point = Vector3::new(x, y, 0.0);
                m.attachment_point = attachment(align);
                m.drawing_direction = crate::entities::DrawingDirection::ByStyle;
                if let Some(c) = content_sets(P::CONTENT_COLOR) {
                    m.common.color = c.color.clone();
                }
                if let Some(s) = content_sets(P::TEXT_STYLE) {
                    let name = if s.text_style_name.is_empty() {
                        s.text_style_handle
                            .and_then(|h| self.text_styles.iter().find(|st| st.handle == h))
                            .map(|st| st.name.clone())
                            .unwrap_or_default()
                    } else {
                        s.text_style_name.clone()
                    };
                    if !name.is_empty() {
                        m.style = name;
                    }
                }
                texts.push(((r, c), m));
            }
        }
        let line = |a: Vector3, b: Vector3| {
            let mut l = Line::from_points(a, b);
            by_block(&mut l.common);
            l.common.linetype = "ByBlock".into();
            l.common.line_weight = LineWeight::ByBlock;
            l
        };
        // Grid lines, skipping the inside of merged regions: each row
        // boundary top to bottom as runs left to right, then each column
        // boundary left to right as runs top to bottom.
        let mut lines = Vec::new();
        for k in 0..=nr {
            let open = |c: usize| k == 0 || k == nr || owner[k - 1][c] != owner[k][c];
            let mut c = 0;
            while c < nc {
                if !open(c) {
                    c += 1;
                    continue;
                }
                let start = c;
                while c < nc && open(c) {
                    c += 1;
                }
                lines.push(line(Vector3::new(xs[start], -ys[k], 0.0), Vector3::new(xs[c], -ys[k], 0.0)));
            }
        }
        for k in 0..=nc {
            let open = |r: usize| k == 0 || k == nc || owner[r][k - 1] != owner[r][k];
            let mut r = 0;
            while r < nr {
                if !open(r) {
                    r += 1;
                    continue;
                }
                let start = r;
                while r < nr && open(r) {
                    r += 1;
                }
                lines.push(line(Vector3::new(xs[k], -ys[start], 0.0), Vector3::new(xs[k], -ys[r], 0.0)));
            }
        }
        let mut point = Line::from_points(Vector3::ZERO, Vector3::ZERO);
        point.common.layer = "Defpoints".into();
        point.common.color = Color::from_index(8);
        point.common.invisible = true;
        point.common.transparency = Transparency::ByBlock;
        point.common.line_weight = LineWeight::from_value(0);
        lines.push(point);
        (texts, lines, fills)
    }

    pub fn build_table_block(
        &mut self,
        table: Handle,
        fields: Vec<(usize, usize, String, Vec<NewField>)>,
    ) -> Option<Vec<Option<Handle>>> {
        let cells: Vec<(usize, usize)> = fields.iter().map(|f| (f.0, f.1)).collect();
        let texts = self.redraw_table_block(table, &cells, true)?;
        let mut out = Vec::new();
        for (r, c, template, children) in fields {
            let container = texts.get(&(r, c)).and_then(|h| self.set_text_field(*h, &template, children));
            let display = texts.get(&(r, c)).and_then(|h| match self.get_entity(*h) {
                Some(EntityType::MText(text)) => Some(text.value.clone()),
                _ => None,
            }).unwrap_or_default();
            if let (Some(container), Some(EntityType::Table(t))) = (container, self.get_entity_mut(table)) {
                if let Some(cell) = t.rows.get_mut(r).and_then(|row| row.cells.get_mut(c)) {
                    if cell.contents.is_empty() {
                        cell.contents.push(crate::entities::table::CellContent::new());
                    }
                    let content = &mut cell.contents[0];
                    content.content_type = crate::entities::table::TableCellContentType::Field;
                    content.value = CellValue::text(&display);
                    content.block_handle = None;
                    content.field_handle = Some(container);
                }
                t.field_handles.push(container);
            }
            out.push(container);
        }
        Some(out)
    }

    pub fn refresh_table_block(&mut self, table: Handle) -> bool {
        self.redraw_table_block(table, &[], false).is_some()
    }

    fn redraw_table_block(
        &mut self,
        table: Handle,
        extra: &[(usize, usize)],
        always: bool,
    ) -> Option<std::collections::HashMap<(usize, usize), Handle>> {
        let Some(EntityType::Table(t)) = self.get_entity(table) else {
            return None;
        };
        let t = (**t).clone();
        let (texts, lines, fills) = self.table_block_plan(&t, extra);
        let record = t
            .block_record_handle
            .and_then(|h| self.block_records.iter().find(|r| r.handle == h))
            .or_else(|| self.block_records.get(&t.block_name).filter(|_| !t.block_name.is_empty()))
            .map(|r| (r.handle, r.name.clone(), r.entity_handles.clone()));
        // A table that let go of its block (found by name) keeps drawing from
        // its own data, which shows more than the block plan does (borders,
        // block cells, row and table backgrounds); only the block is brought
        // up to date for readers that draw it.

        // What the block shows now, against what it should show.
        let key = |e: &EntityType| -> Option<String> {
            let r = |v: f64| format!("{:.9}", v + 0.0);
            match e {
                EntityType::MText(m) => Some(format!(
                    "T {} {} {} {} {} {:?} {:?} {:?}",
                    r(m.insertion_point.x),
                    r(m.insertion_point.y),
                    r(m.height),
                    r(m.rectangle_width),
                    m.attachment_point as i32,
                    m.common.color,
                    m.style,
                    if m.common.xdictionary_handle.is_some() { "" } else { m.value.as_str() }
                )),
                EntityType::Line(l) => Some(format!("L {} {} {} {} {}", r(l.start.x), r(l.start.y), r(l.end.x), r(l.end.y), l.common.layer)),
                EntityType::Solid(f) => Some(format!("S {} {} {} {} {:?}", r(f.first_corner.x), r(f.first_corner.y), r(f.fourth_corner.x), r(f.fourth_corner.y), f.common.color)),
                _ => None,
            }
        };
        let field_cells: Vec<(usize, usize)> = t
            .rows
            .iter()
            .enumerate()
            .flat_map(|(r, row)| {
                row.cells
                    .iter()
                    .enumerate()
                    .filter(|(_, cell)| cell.contents.first().is_some_and(|ct| ct.field_handle.is_some()))
                    .map(move |(c, _)| (r, c))
            })
            .collect();
        if let (false, Some((_, _, old))) = (always, record.as_ref()) {
            let mut now: Vec<String> = old.iter().filter_map(|h| self.get_entity(*h)).filter_map(key).collect();
            let mut want: Vec<String> = texts
                .iter()
                .map(|(cell, m)| {
                    let mut m = m.clone();
                    if field_cells.contains(cell) {
                        m.common.xdictionary_handle = Some(Handle::NULL);
                    }
                    key(&EntityType::MText(m)).unwrap_or_default()
                })
                .chain(lines.iter().filter_map(|l| key(&EntityType::Line(l.clone()))))
                .chain(fills.iter().filter_map(|f| key(&EntityType::Solid(f.clone()))))
                .collect();
            now.sort();
            want.sort();
            if now == want {
                return None;
            }
        }

        // Field cells keep their text's dictionary (the field) and value.
        let mut carried: std::collections::HashMap<(usize, usize), (Handle, String)> = std::collections::HashMap::new();
        for (cell, container) in t.rows.iter().enumerate().flat_map(|(r, row)| {
            row.cells.iter().enumerate().filter_map(move |(c, cell)| cell.contents.first().and_then(|ct| ct.field_handle).map(|h| ((r, c), h)))
        }) {
            let owner_of = |h: Handle| match self.objects.get(&h) {
                Some(ObjectType::Field(f)) => Some(f.owner),
                Some(ObjectType::Dictionary(d)) => Some(d.owner),
                _ => None,
            };
            // container › TEXT dictionary › extension dictionary › MTEXT
            let xdict = owner_of(container).and_then(owner_of);
            let host = xdict.and_then(owner_of);
            if let (Some(xdict), Some(EntityType::MText(m))) = (xdict, host.and_then(|h| self.get_entity(h))) {
                carried.insert(cell, (xdict, m.value.clone()));
            }
        }

        let owner = match record {
            Some((handle, _, old)) => {
                for h in &old {
                    self.remove_entity(*h);
                }
                if let Some(r) = self.block_records.iter_mut().find(|r| r.handle == handle) {
                    r.entity_handles.clear();
                }
                handle
            }
            None => {
                let mut index = 1;
                while self.block_records.get(&format!("*T{index}")).is_some() {
                    index += 1;
                }
                let mut record = crate::tables::BlockRecord::new(format!("*T{index}"));
                record.handle = self.allocate_handle();
                record.block_entity_handle = self.allocate_handle();
                record.block_end_handle = self.allocate_handle();
                record.flags.anonymous = true;
                let handle = record.handle;
                let name = record.name.clone();
                self.block_records.add(record).ok()?;
                if let Some(EntityType::Table(t)) = self.get_entity_mut(table) {
                    t.block_name = name;
                    t.block_record_handle = Some(handle);
                }
                handle
            }
        };
        if !self.layers.contains("Defpoints") {
            let mut layer = crate::tables::Layer::new("Defpoints");
            layer.handle = self.allocate_handle();
            layer.is_plottable = false;
            self.layers.add_or_replace(layer);
        }
        let mut handles = std::collections::HashMap::new();
        // Cell backgrounds first, under the texts and the grid.
        for mut f in fills {
            f.common.owner_handle = owner;
            let _ = self.add_entity(EntityType::Solid(f));
        }
        for (cell, mut m) in texts {
            m.common.owner_handle = owner;
            if let Some((xdict, value)) = carried.get(&cell) {
                m.common.xdictionary_handle = Some(*xdict);
                m.value = value.clone();
            }
            if let Ok(h) = self.add_entity(EntityType::MText(m)) {
                if let Some((xdict, _)) = carried.get(&cell) {
                    if let Some(ObjectType::Dictionary(d)) = self.objects.get_mut(xdict) {
                        d.owner = h;
                    }
                }
                handles.insert(cell, h);
            }
        }
        for mut l in lines {
            l.common.owner_handle = owner;
            let _ = self.add_entity(EntityType::Line(l));
        }
        Some(handles)
    }

    fn register_fields(&mut self, all: Vec<Field>) {
        let list = self.field_list_handle();
        if let Some(ObjectType::FieldList(l)) = self.objects.get_mut(&list) {
            l.fields.extend(all.iter().map(|f| f.handle));
        }
        for f in all {
            self.fields.insert(
                f.handle,
                FieldDef {
                    handle: f.handle,
                    owner: f.owner,
                    evaluator: f.evaluator_id.clone(),
                    code: f.code.clone(),
                    objects: f.referenced_objects.clone(),
                },
            );
            self.objects.insert(f.handle, ObjectType::Field(f));
        }
    }

    pub fn remove_text_field(&mut self, host: Handle) -> bool {
        let Some(Some(xdict)) = self.text_host(host, |common, _, _| common.xdictionary_handle)
        else {
            return false;
        };
        let Some(ObjectType::Dictionary(x)) = self.objects.get(&xdict) else {
            return false;
        };
        let Some(field_dict) = x.get("ACAD_FIELD") else {
            return false;
        };
        let doomed: Vec<Handle> = self
            .objects
            .keys()
            .copied()
            .filter(|h| self.object_or_field_reaches(*h, field_dict))
            .collect();
        for h in &doomed {
            self.objects.remove(h);
            self.fields.remove(h);
        }
        for obj in self.objects.values_mut() {
            if let ObjectType::FieldList(l) = obj {
                l.fields.retain(|h| !doomed.contains(h));
            }
        }
        let now_empty = match self.objects.get_mut(&xdict) {
            Some(ObjectType::Dictionary(x)) => {
                x.entries.retain(|(k, _)| !k.eq_ignore_ascii_case("ACAD_FIELD"));
                x.entries.is_empty()
            }
            _ => false,
        };
        if now_empty {
            self.objects.remove(&xdict);
            self.text_host(host, |common, _, _| common.xdictionary_handle = None);
        }
        true
    }

    pub fn attach_attribute_fields(
        &mut self,
        insert: Handle,
        ctx: &dyn FieldContext,
    ) -> Vec<Handle> {
        self.attach_attribute_fields_mapped(insert, ctx, &|code: &str| code.to_string())
    }

    pub fn attach_attribute_fields_mapped(
        &mut self,
        insert: Handle,
        ctx: &dyn FieldContext,
        map: &dyn Fn(&str) -> String,
    ) -> Vec<Handle> {
        // ATTRIBs need handles to host fields.
        let unset = match self.get_entity(insert) {
            Some(EntityType::Insert(i)) => i.attributes.iter().filter(|a| a.common.handle.is_null()).count(),
            _ => return Vec::new(),
        };
        let fresh: Vec<Handle> = (0..unset).map(|_| self.allocate_handle()).collect();
        if let Some(EntityType::Insert(i)) = self.get_entity_mut(insert) {
            let owner = i.common.handle;
            let mut fresh = fresh.into_iter();
            for a in i.attributes.iter_mut().filter(|a| a.common.handle.is_null()) {
                a.common.handle = fresh.next().unwrap_or_default();
                a.common.owner_handle = owner;
            }
        }
        let Some(EntityType::Insert(ins)) = self.get_entity(insert) else {
            return Vec::new();
        };
        let mut plans = Vec::new();
        for attrib in &ins.attributes {
            let Some(def) = self.entities_in_block(&ins.block_name).find_map(|e| match e {
                EntityType::AttributeDefinition(d) if d.tag.eq_ignore_ascii_case(&attrib.tag) => {
                    Some(d.common.handle)
                }
                _ => None,
            }) else {
                continue;
            };
            let Some(container) = container_for_host(self, def) else {
                continue;
            };
            let mut kids: Vec<&FieldDef> = self
                .fields
                .values()
                .filter(|f| f.owner == container.handle)
                .collect();
            kids.sort_by_key(|f| u64::from(f.handle));
            let children = kids
                .iter()
                .map(|k| {
                    let placeholder = k.code.contains("?BlockRefId");
                    let code = map(&k.code.replace("?BlockRefId", "%<\\_ObjIdx 0>%"));
                    let objects = if placeholder { vec![insert] } else { k.objects.clone() };
                    let stored = match self.objects.get(&k.handle) {
                        Some(ObjectType::Field(f)) => Some(f),
                        _ => None,
                    };
                    let cached = stored.map(|f| f.value.display().to_string()).unwrap_or_default();
                    let shown = evaluate_code(self, &code, &objects, Some(attrib.common.handle), ctx)
                        .unwrap_or(cached);
                    let mut child = NewField::new(code, shown);
                    child.objects = objects;
                    if let Some(f) = stored {
                        child.evaluation_option = f.evaluation_option;
                    }
                    child
                })
                .collect::<Vec<_>>();
            plans.push((attrib.common.handle, container.code.clone(), children));
        }
        plans
            .into_iter()
            .filter_map(|(attrib, template, children)| {
                self.set_text_field(attrib, &template, children).map(|_| attrib)
            })
            .collect()
    }

    pub fn stamp_plot_fields(&mut self, ctx: &dyn FieldContext) -> Vec<Handle> {
        let plot = Plotting(ctx);
        self.restamp_fields(&plot, ctx, &|_, f| f.evaluation_option & 4 != 0, None).0
    }

    pub fn update_fields(
        &mut self,
        ctx: &dyn FieldContext,
        event: i32,
        hosts: Option<&[Handle]>,
    ) -> (Vec<Handle>, usize) {
        self.restamp_fields(ctx, ctx, &|_, f| f.evaluation_option & event != 0, hosts)
    }

    pub fn refresh_table_formulas(&mut self, ctx: &dyn FieldContext, table: Handle) -> Vec<Handle> {
        let hosts = match self.get_entity(table) {
            Some(EntityType::Table(t)) => t
                .block_record_handle
                .and_then(|h| self.block_records.iter().find(|r| r.handle == h))
                .map(|r| r.entity_handles.clone()),
            _ => None,
        };
        match hosts {
            Some(hosts) => self.restamp_fields(ctx, ctx, &|kid, _| kid.evaluator == "AcExpr", Some(&hosts)).0,
            None => Vec::new(),
        }
    }

    pub fn refresh_sheet_set_fields(&mut self, ctx: &dyn FieldContext) -> Vec<Handle> {
        self.restamp_fields(ctx, ctx, &|kid, _| kid.evaluator.starts_with("AcSm"), None).0
    }

    fn restamp_fields(
        &mut self,
        eval_ctx: &dyn FieldContext,
        ctx: &dyn FieldContext,
        pick: &dyn Fn(&FieldDef, &Field) -> bool,
        only: Option<&[Handle]>,
    ) -> (Vec<Handle>, usize) {
        let mut found = 0;
        let plot = eval_ctx;
        let mut values: Vec<(Handle, CellValue)> = Vec::new();
        let mut hosts: Vec<(Handle, Handle, String)> = Vec::new();
        for container in self.fields.values().filter(|f| f.evaluator == "_text") {
            let Some(host) = host_of(self, container) else {
                continue;
            };
            if only.is_some_and(|only| !only.contains(&host)) {
                continue;
            }
            let mut kids: Vec<&FieldDef> = self
                .fields
                .values()
                .filter(|f| f.owner == container.handle)
                .collect();
            kids.sort_by_key(|f| u64::from(f.handle));
            found += kids.len();
            let mut shown = Vec::new();
            let mut changed = false;
            for kid in kids {
                let Some(ObjectType::Field(stored)) = self.objects.get(&kid.handle) else {
                    shown.push(String::new());
                    continue;
                };
                let cached = match stored.value.display() {
                    "" => stored.value_string.clone(),
                    shown => shown.to_string(),
                };
                let fresh = pick(kid, stored).then(|| eval_field(self, kid, plot, host)).flatten();
                // A sheet set field with no set open to answer it keeps the
                // value it was saved with rather than `####`.
                let unanswered = |text: &str| kid.evaluator.starts_with("AcSm") && text == "####";
                match fresh {
                    Some(text) if text != cached && !unanswered(&text) => {
                        changed = true;
                        values.push((kid.handle, plot_value(kid, &text, ctx.now_julian())));
                        shown.push(text);
                    }
                    _ => shown.push(cached),
                }
            }
            let mtext = matches!(self.get_entity(host), Some(EntityType::MText(_)));
            if let Some(text) = changed.then(|| fill_template(&container.code, &shown, mtext)).flatten() {
                hosts.push((host, container.handle, text));
            }
        }
        for (h, value) in values {
            if let Some(ObjectType::Field(f)) = self.objects.get_mut(&h) {
                f.value_string = value.display().to_string();
                f.value_string_length = f.value_string.encode_utf16().count() as i32;
                f.evaluation_status = 2;
                f.value = value;
            }
        }
        let mut changed = Vec::new();
        for (host, container, text) in hosts {
            if let Some(ObjectType::Field(c)) = self.objects.get_mut(&container) {
                for cv in &mut c.child_values {
                    if cv.key == "ACFD_FIELDTEXT_CHECKSUM" {
                        cv.value.numeric_value = field_text_checksum(&text);
                    }
                }
            }
            let replaced = self.text_host(host, |_, t, _| std::mem::replace(t, text.clone()));
            if replaced.is_some_and(|old| old != text) {
                changed.push(host);
            }
        }
        (changed, found)
    }

    fn object_or_field_reaches(&self, start: Handle, target: Handle) -> bool {
        let mut cur = start;
        for _ in 0..16 {
            if cur == target {
                return true;
            }
            let next = match self.objects.get(&cur) {
                Some(ObjectType::Field(f)) => Some(f.owner),
                _ => self.object_owner(cur),
            };
            match next {
                Some(n) if n != cur && !n.is_null() => cur = n,
                _ => return false,
            }
        }
        false
    }

    fn field_list_handle(&mut self) -> Handle {
        let nod = self.header.named_objects_dict_handle;
        let in_nod = match self.objects.get(&nod) {
            Some(ObjectType::Dictionary(d)) => d.get("ACAD_FIELDLIST"),
            _ => None,
        };
        if let Some(h) = in_nod.filter(|h| matches!(self.objects.get(h), Some(ObjectType::FieldList(_)))) {
            return h;
        }
        if let Some(h) = self
            .objects
            .iter()
            .find_map(|(h, o)| matches!(o, ObjectType::FieldList(_)).then_some(*h))
        {
            return h;
        }
        let h = self.allocate_handle();
        self.objects.insert(
            h,
            ObjectType::FieldList(FieldList {
                handle: h,
                owner: nod,
                ..FieldList::default()
            }),
        );
        if let Some(ObjectType::Dictionary(d)) = self.objects.get_mut(&nod) {
            d.add_entry("ACAD_FIELDLIST", h);
        }
        h
    }

    fn text_host<R>(
        &mut self,
        host: Handle,
        f: impl FnOnce(&mut EntityCommon, &mut String, TextHostKind) -> R,
    ) -> Option<R> {
        if self.get_entity(host).is_some() {
            return match self.get_entity_mut(host)? {
                EntityType::MText(e) => Some(f(&mut e.common, &mut e.value, TextHostKind::MText)),
                EntityType::Text(e) => Some(f(&mut e.common, &mut e.value, TextHostKind::Text)),
                EntityType::AttributeDefinition(e) => Some(f(
                    &mut e.common,
                    &mut e.default_value,
                    TextHostKind::AttributeDefinition,
                )),
                EntityType::AttributeEntity(e) => {
                    Some(f(&mut e.common, &mut e.value, TextHostKind::Attribute))
                }
                _ => None,
            };
        }
        let insert = self.entities().find_map(|e| match e {
            EntityType::Insert(i) if i.attributes.iter().any(|a| a.common.handle == host) => {
                Some(i.common.handle)
            }
            _ => None,
        })?;
        let EntityType::Insert(i) = self.get_entity_mut(insert)? else {
            return None;
        };
        let a = i.attributes.iter_mut().find(|a| a.common.handle == host)?;
        Some(f(&mut a.common, &mut a.value, TextHostKind::Attribute))
    }
}

fn child_field(child: NewField, handle: Handle, owner: Handle) -> Field {
    let mut child_values = Vec::new();
    if child.evaluator.starts_with("AcVar") {
        if let Some(name) = code_word(&child.code, 1) {
            // A hyperlink (`\href`) names no variable.
            let name = if name.starts_with('\\') { "" } else { name };
            let mut v = CellValue::text(name);
            v.flags = 2;
            v.formatted_value.clear();
            child_values.push(FieldChildValue {
                key: "Variable".into(),
                value: v,
            });
        }
    }
    if child.evaluator == "AcDiesel" {
        let expr = child.code.trim().trim_start_matches("\\AcDiesel").trim();
        let mut v = CellValue::text(expr);
        v.flags = 2;
        v.formatted_value.clear();
        child_values.push(FieldChildValue {
            key: "DieselExpression".into(),
            value: v,
        });
    }
    if child.evaluator.starts_with("AcObjProp") {
        if let Some(&object) = child.objects.first() {
            let mut id = CellValue::new();
            id.value_type = CellValueType::Handle;
            id.raw_type_code = 0x40;
            id.handle_value = Some(object);
            id.flags = 2;
            child_values.push(FieldChildValue {
                key: "ObjectPropertyId".into(),
                value: id,
            });
        }
        if let Some(prop) = child
            .code
            .split(").")
            .nth(1)
            .and_then(|s| s.split([' ', '\\']).next())
        {
            let mut v = CellValue::text(prop);
            v.flags = 2;
            v.formatted_value.clear();
            child_values.push(FieldChildValue {
                key: "ObjectPropertyName".into(),
                value: v,
            });
        }
        // `Object(<id>,1)`: the option, as a number and as text.
        let object_arg = between(&child.code, "Object(", ").").unwrap_or("");
        if let Some((_, option)) = object_arg.rsplit_once(',') {
            let option = option.trim();
            let mut n = CellValue::integer(option.parse().unwrap_or(0));
            n.flags = 2;
            n.formatted_value.clear();
            let mut s = CellValue::text(option);
            s.flags = 2;
            s.formatted_value.clear();
            child_values.push(FieldChildValue {
                key: "ObjectPropertyOption".into(),
                value: n,
            });
            child_values.push(FieldChildValue {
                key: "ObjectPropertyOptionString".into(),
                value: s,
            });
        }
        // A block placeholder names its object only on insertion.
        if object_arg.starts_with("?BlockRefId") {
            let mut v = CellValue::text("?BlockRefId");
            v.flags = 2;
            v.formatted_value.clear();
            child_values.push(FieldChildValue {
                key: "ObjectPropertyUnresolvedId".into(),
                value: v,
            });
        }
    }
    if child.evaluator.starts_with("AcCount") {
        // The count's JSON query, kept beside the code.
        let body = child.code.trim().trim_start_matches('\\');
        let json = body.split_once(char::is_whitespace).map(|(_, j)| j.trim()).unwrap_or("");
        if !json.is_empty() {
            let mut v = CellValue::text(json);
            v.flags = 2;
            v.formatted_value.clear();
            child_values.push(FieldChildValue {
                key: "CountJsonString".into(),
                value: v,
            });
        }
    }
    // A sheet set field names its component and property; in a drawing that
    // is no sheet it is left unevaluated (`####`).
    let mut unevaluated = false;
    if child.evaluator.starts_with("AcSm") {
        for (key, text) in crate::sheet_set::field_child_values(&child.code) {
            let mut v = CellValue::text(&text);
            v.flags = 2;
            v.formatted_value.clear();
            child_values.push(FieldChildValue { key: key.into(), value: v });
        }
        unevaluated = child.value.display() == "####";
    }
    let shown = child.value.display().to_string();
    let xdata = hyperlink_xdata(&child.code).unwrap_or_default();
    let value = if unevaluated {
        let mut v = CellValue::new();
        v.flags = 1;
        v.format = child.value.format.clone();
        v
    } else if shown == "----" && child.evaluator.starts_with("AcSm") {
        // A property without a value keeps an empty string; only the shown
        // text is `----`.
        let mut v = CellValue::text("");
        v.format = child.value.format.clone();
        v
    } else {
        child.value.clone()
    };
    let (state, evaluation_status, evaluation_error_code) =
        if unevaluated { (43, 32, 22) } else { (59, 2, 0) };
    Field {
        handle,
        owner,
        // Pre-R2007 files keep the format on the field itself.
        format: child.value.format.clone(),
        evaluator_id: child.evaluator,
        code: child.code,
        referenced_objects: child.objects,
        evaluation_option: child.evaluation_option,
        state,
        evaluation_status,
        evaluation_error_code,
        value,
        value_string_length: shown.encode_utf16().count() as i32,
        value_string: shown,
        xdata,
        child_values,
        ..Field::default()
    }
}

fn template_display(template: &str, children: &[NewField], mtext: bool) -> Option<String> {
    let shown: Vec<&str> = children.iter().map(|c| c.value.display()).collect();
    fill_template(template, &shown, mtext)
}

fn fill_template<S: AsRef<str>>(template: &str, shown: &[S], mtext: bool) -> Option<String> {
    let mut out = String::new();
    let mut rest = template;
    while let Some(p) = rest.find("%<\\_FldIdx ") {
        out.push_str(&rest[..p]);
        let after = &rest[p + 11..];
        let end = after.find(">%")?;
        let idx: usize = after[..end].trim().parse().ok()?;
        let shown = shown.get(idx)?.as_ref();
        out.push_str(&if mtext { mtext_escape(shown) } else { shown.to_string() });
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    Some(out)
}

pub fn field_spans(
    doc: &CadDocument,
    host: Handle,
    text: &str,
) -> Option<Vec<std::ops::Range<usize>>> {
    let container = container_for_host(doc, host)?;
    let shown = cached_children(doc, container);
    let mtext = matches!(doc.get_entity(host), Some(EntityType::MText(_)));
    let mut out = String::new();
    let mut spans = Vec::new();
    let mut rest = container.code.as_str();
    while let Some(p) = rest.find("%<\\_FldIdx ") {
        out.push_str(&rest[..p]);
        let after = &rest[p + 11..];
        let end = after.find(">%")?;
        let idx: usize = after[..end].trim().parse().ok()?;
        let value = shown.get(idx)?;
        let start = out.len();
        out.push_str(&if mtext { mtext_escape(value) } else { value.clone() });
        spans.push(start..out.len());
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    (out == text).then_some(spans)
}

fn cached_value(doc: &CadDocument, field: Handle) -> String {
    match doc.objects.get(&field) {
        Some(ObjectType::Field(stored)) => match stored.value.display() {
            "" => stored.value_string.clone(),
            shown => shown.to_string(),
        },
        _ => String::new(),
    }
}

fn cached_children(doc: &CadDocument, container: &FieldDef) -> Vec<String> {
    let mut kids: Vec<&FieldDef> =
        doc.fields.values().filter(|f| f.owner == container.handle).collect();
    kids.sort_by_key(|f| u64::from(f.handle));
    kids.iter().map(|kid| cached_value(doc, kid.handle)).collect()
}

pub fn cell_field_text(
    doc: &CadDocument,
    field: Handle,
    table: Handle,
    ctx: &dyn FieldContext,
) -> Option<String> {
    let def = doc.fields.get(&field)?;
    if def.evaluator != "_text" {
        return if def.evaluator == "AcExpr" {
            resolve_handle(doc, field, table, ctx)
        } else {
            Some(cached_value(doc, field))
        };
    }
    let formula = doc.fields.values().any(|f| f.owner == def.handle && f.evaluator == "AcExpr");
    if formula {
        return resolve_handle(doc, field, table, ctx);
    }
    fill_template(&def.code, &cached_children(doc, def), false)
}

struct Plotting<'a>(&'a dyn FieldContext);

impl FieldContext for Plotting<'_> {
    fn now_julian(&self) -> f64 {
        self.0.now_julian()
    }
    fn file_times(&self) -> Option<(f64, f64)> {
        self.0.file_times()
    }
    fn plotting(&self) -> bool {
        true
    }
    fn login(&self) -> Option<String> {
        self.0.login()
    }
    fn getenv(&self, name: &str) -> Option<String> {
        self.0.getenv(name)
    }
    fn getvar(&self, name: &str) -> Option<String> {
        self.0.getvar(name)
    }
    fn file_size(&self) -> Option<u64> {
        self.0.file_size()
    }
    fn date_locale(&self) -> DateLocale {
        self.0.date_locale()
    }
    fn sheet_sets(
        &self,
        f: &mut dyn FnMut(&crate::sheet_set::SheetSetDatabase) -> Option<String>,
    ) -> Option<String> {
        self.0.sheet_sets(f)
    }
}

fn plot_value(field: &FieldDef, shown: &str, jd: f64) -> CellValue {
    let format = code_format(&field.code);
    let mut v = CellValue::text(shown);
    if code_word(&field.code, 1) == Some("PlotDate") {
        let (y, mo, d, h, mi, s) = julian_parts(jd);
        let wd = weekday(jd);
        v.value_type = CellValueType::Date;
        v.raw_type_code = CellValueType::Date as i32;
        v.text.clear();
        v.data_size = 16;
        v.binary_value = [y as u32, mo, wd, d, h, mi, s, 0]
            .iter()
            .flat_map(|x| (*x as u16).to_le_bytes())
            .collect();
    }
    v.flags = 4;
    v.format = if format.is_empty() && v.value_type == CellValueType::Date {
        "%x".into()
    } else {
        format.to_string()
    };
    v
}

pub const BLOCK_PLACEHOLDER_PROPERTIES: [(&str, &str, &str); 17] = [
    ("Block Unit", "InsUnits", "%tc4"),
    ("Color", "TrueColor", "%tc4"),
    ("Layer", "Layer", "%tc4"),
    ("Linetype", "Linetype", "%tc4"),
    ("Linetype scale", "LinetypeScale", "%lu6"),
    ("Lineweight", "Lineweight", "%.2f mm%lw1"),
    ("Material", "Material", "%tc4"),
    ("Name", "EffectiveName", "%tc4"),
    ("Object name", "ObjectName", "%tc4"),
    ("Plot style", "PlotStyleName", "%tc4"),
    ("Position", "InsertionPoint", "%lu6"),
    ("Rotation", "Rotation", "%au5"),
    ("Scale X", "XEffectiveScaleFactor", "%lu6"),
    ("Scale Y", "YEffectiveScaleFactor", "%lu6"),
    ("Scale Z", "ZEffectiveScaleFactor", "%lu6"),
    ("Transparency", "EntityTransparency", "%tc4"),
    ("Unit factor", "InsUnitsFactor", "%lu6"),
];

pub const ANGLE_FORMATS: [(&str, &str); 7] = [
    ("(none)", ""),
    ("Current units", "%au5"),
    ("Decimal degrees", "%au0"),
    ("Deg/min/sec", "%au1"),
    ("Grads", "%au2"),
    ("Radians", "%au3"),
    ("Surveyor's units", "%au4"),
];

pub const LINEWEIGHT_FORMATS: [(&str, &str); 2] =
    [("Millimeters", "%.2f mm%lw1"), ("Inches", "%.3f\"%lw2")];

fn hyperlink_xdata(code: &str) -> Option<crate::xdata::ExtendedData> {
    use crate::xdata::{ExtendedData, ExtendedDataRecord, XDataValue};
    let rest = code.split_once("\\href")?.1.trim_start().strip_prefix('"')?;
    let target = &rest[..rest.find('"')?];
    let (url, rest) = target.split_once('#').unwrap_or((target, ""));
    let (location, rest) = rest.split_once('#').unwrap_or(("", rest));
    let (text, flags) = rest.rsplit_once('#').unwrap_or((rest, "0"));
    let mut rec = ExtendedDataRecord::new("PE_URL");
    rec.add_value(XDataValue::String(url.into()));
    rec.add_value(XDataValue::ControlString("{".into()));
    for s in [text, location] {
        if !s.is_empty() {
            rec.add_value(XDataValue::String(s.into()));
        }
    }
    rec.add_value(XDataValue::ControlString("{".into()));
    rec.add_value(XDataValue::Integer32(flags.trim().parse().unwrap_or(0)));
    rec.add_value(XDataValue::ControlString("}".into()));
    rec.add_value(XDataValue::ControlString("}".into()));
    let mut xdata = ExtendedData::new();
    xdata.add_record(rec);
    Some(xdata)
}

pub fn evaluate_code(
    doc: &CadDocument,
    code: &str,
    objects: &[Handle],
    host: Option<Handle>,
    ctx: &dyn FieldContext,
) -> Option<String> {
    let mut code = code.trim();
    if let Some(inner) = code.strip_prefix("%<").and_then(|c| c.strip_suffix(">%")) {
        code = inner.trim();
    }
    let field = FieldDef {
        handle: Handle::NULL,
        owner: Handle::NULL,
        evaluator: code_evaluator(code).to_string(),
        code: code.to_string(),
        objects: objects.to_vec(),
    };
    eval_field(doc, &field, ctx, host.unwrap_or(Handle::NULL)).or_else(|| {
        // A new PlotDate field has never been plotted.
        (field.evaluator.starts_with("AcVar") && code_word(code, 1) == Some("PlotDate"))
            .then(|| NO_VALUE.into())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn julian_and_edtime() {
        // 2452257.345 = 2001-12-13, a Thursday.
        assert_eq!(julian_parts(2452257.345).0, 2001);
        assert_eq!(julian_parts(2452257.345).1, 12);
        assert_eq!(julian_parts(2452257.345).2, 13);
        assert_eq!(weekday(2452257.345), 4);
        assert_eq!(edtime(2452257.345, "YYYY/MO/DD"), "2001/12/13");
        assert_eq!(
            edtime(2452257.345, "DDDD, MONTH D, YYYY"),
            "Thursday, December 13, 2001"
        );
        assert_eq!(edtime(2452257.345, "DD.MON.YY"), "13.Dec.01");
        assert_eq!(
            format_dt(julian_parts(2452257.345), "yyyy/MM/dd"),
            "2001/12/13"
        );
        assert_eq!(num_str(5.0), "5");
        assert_eq!(rtos(3.14159, Some(2)), "3.14");
    }
}

fn attachment(align: i32) -> crate::entities::AttachmentPoint {
    use crate::entities::AttachmentPoint as A;
    match align {
        1 => A::TopLeft,
        2 => A::TopCenter,
        3 => A::TopRight,
        4 => A::MiddleLeft,
        6 => A::MiddleRight,
        7 => A::BottomLeft,
        8 => A::BottomCenter,
        9 => A::BottomRight,
        _ => A::MiddleCenter,
    }
}
