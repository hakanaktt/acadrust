//! The expressions of a package's workflow (the editor's Visual Basic
//! form, in brackets): the subset the layout evaluation reads.
//!
//! ```text
//! expr    := or
//! or      := and (("Or" | "OrElse") and)*
//! and     := not (("And" | "AndAlso") not)*
//! not     := "Not" not | compare
//! compare := sum (("=" | "<>" | "<" | ">" | "<=" | ">=") sum)?
//! sum     := term (("+" | "-" | "&") term)*
//! term    := unary (("*" | "/") unary)*
//! unary   := "-" unary | "+" unary | power
//! power   := atom ("^" unary)?
//! atom    := number | "text" | True | False | name ("." member)*
//!            ["(" [expr ("," expr)*] ")"] | "(" expr ")"
//! ```
//!
//! Names and keywords are matched without regard to case. Calls:
//! `IF(c, a, b)`, `Math.Abs/Min/Max/Sqrt/Sin/Cos/Tan/Atan/Round/Floor/
//! Ceiling`. Members: `.Value` (an enumeration's name),
//! `.IsValid`, `.Offset`, `.Elevation`, `.X`, `.Y` (a point's offset and
//! elevation), `SA.IsLayout`. Anything else is refused.

use crate::error::DxfError;

/// A value of an expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    Bool(bool),
    Text(String),
    /// An enumeration value: key and name.
    Enum(i32, String),
    /// A target: whether it is valid, its value.
    Target(bool, f64),
    /// A point: offset and elevation.
    Point(f64, f64),
}

/// What a name resolves to.
pub trait Scope {
    fn get(&self, name: &str) -> Option<Value>;
}

pub fn fail(m: impl Into<String>) -> DxfError {
    DxfError::Parse(format!("package expression: {}", m.into()))
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Name(String),
    Op(String),
}

fn tokens(s: &str) -> Result<Vec<Tok>, DxfError> {
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
        } else if ch.is_ascii_digit() || (ch == '.' && c.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let st = i;
            while i < c.len() && (c[i].is_ascii_digit() || c[i] == '.') {
                i += 1;
            }
            if i < c.len() && (c[i] == 'e' || c[i] == 'E') && c.get(i + 1).is_some_and(|d| d.is_ascii_digit() || *d == '-' || *d == '+') {
                i += 2;
                while i < c.len() && c[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let t: String = c[st..i].iter().collect();
            out.push(Tok::Num(t.parse().map_err(|_| fail(format!("number {t}")))?));
        } else if ch == '"' {
            let st = i + 1;
            i = st;
            while i < c.len() && c[i] != '"' {
                i += 1;
            }
            if i >= c.len() {
                return Err(fail("unterminated text"));
            }
            out.push(Tok::Str(c[st..i].iter().collect()));
            i += 1;
        } else if ch.is_alphabetic() || ch == '_' {
            let st = i;
            while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_') {
                i += 1;
            }
            out.push(Tok::Name(c[st..i].iter().collect()));
        } else {
            let two: String = c[i..(i + 2).min(c.len())].iter().collect();
            if ["<>", "<=", ">="].contains(&two.as_str()) {
                out.push(Tok::Op(two));
                i += 2;
            } else if "+-*/^&=<>(),.".contains(ch) {
                out.push(Tok::Op(ch.to_string()));
                i += 1;
            } else {
                return Err(fail(format!("character '{ch}'")));
            }
        }
    }
    Ok(out)
}

struct P<'a> {
    t: Vec<Tok>,
    i: usize,
    scope: &'a dyn Scope,
    /// Nesting of the recursive rules (`not` and `unary`, which every
    /// nesting goes through), bounded so a package cannot overflow the stack.
    depth: usize,
}

/// The deepest nesting an expression may have.
const MAX_DEPTH: usize = 128;

fn kw(t: Option<&Tok>, k: &str) -> bool {
    matches!(t, Some(Tok::Name(n)) if n.eq_ignore_ascii_case(k))
}

fn num(v: &Value) -> Result<f64, DxfError> {
    match v {
        Value::Number(n) => Ok(*n),
        Value::Target(_, n) => Ok(*n),
        v => Err(fail(format!("{v:?} is not a number"))),
    }
}

fn truth(v: &Value) -> Result<bool, DxfError> {
    match v {
        Value::Bool(b) => Ok(*b),
        v => Err(fail(format!("{v:?} is not a condition"))),
    }
}

impl P<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }

    fn op(&mut self, o: &str) -> bool {
        if matches!(self.peek(), Some(Tok::Op(x)) if x == o) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, o: &str) -> Result<(), DxfError> {
        self.op(o).then_some(()).ok_or_else(|| fail(format!("expected '{o}'")))
    }

    fn or(&mut self) -> Result<Value, DxfError> {
        let mut v = self.and()?;
        while kw(self.peek(), "Or") || kw(self.peek(), "OrElse") {
            self.i += 1;
            let r = self.and()?;
            v = Value::Bool(truth(&v)? || truth(&r)?);
        }
        Ok(v)
    }

    fn and(&mut self) -> Result<Value, DxfError> {
        let mut v = self.not()?;
        while kw(self.peek(), "And") || kw(self.peek(), "AndAlso") {
            self.i += 1;
            let r = self.not()?;
            v = Value::Bool(truth(&v)? && truth(&r)?);
        }
        Ok(v)
    }

    /// Enters a recursive rule; refuses an expression nested too deep.
    fn enter(&mut self) -> Result<(), DxfError> {
        self.depth += 1;
        (self.depth <= MAX_DEPTH).then_some(()).ok_or_else(|| fail("expression nested too deep"))
    }

    fn not(&mut self) -> Result<Value, DxfError> {
        self.enter()?;
        let v = self.not_inner();
        self.depth -= 1;
        v
    }

    fn not_inner(&mut self) -> Result<Value, DxfError> {
        if kw(self.peek(), "Not") {
            self.i += 1;
            return Ok(Value::Bool(!truth(&self.not()?)?));
        }
        self.compare()
    }

    fn compare(&mut self) -> Result<Value, DxfError> {
        let a = self.sum()?;
        for o in ["<>", "<=", ">=", "=", "<", ">"] {
            if self.op(o) {
                let b = self.sum()?;
                let ord = match (&a, &b) {
                    (Value::Text(x), Value::Text(y)) => x.cmp(y),
                    (Value::Enum(x, _), Value::Enum(y, _)) => x.cmp(y),
                    (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
                    _ => num(&a)?.partial_cmp(&num(&b)?).ok_or_else(|| fail("not a number"))?,
                };
                use std::cmp::Ordering::*;
                let r = match o {
                    "=" => ord == Equal,
                    "<>" => ord != Equal,
                    "<" => ord == Less,
                    ">" => ord == Greater,
                    "<=" => ord != Greater,
                    _ => ord != Less,
                };
                return Ok(Value::Bool(r));
            }
        }
        Ok(a)
    }

    fn sum(&mut self) -> Result<Value, DxfError> {
        let mut v = self.term()?;
        loop {
            if self.op("+") {
                let r = self.term()?;
                v = match (&v, &r) {
                    (Value::Text(a), Value::Text(b)) => Value::Text(format!("{a}{b}")),
                    _ => Value::Number(num(&v)? + num(&r)?),
                };
            } else if self.op("-") {
                v = Value::Number(num(&v)? - num(&self.term()?)?);
            } else if self.op("&") {
                let r = self.term()?;
                v = Value::Text(format!("{}{}", text(&v), text(&r)));
            } else {
                return Ok(v);
            }
        }
    }

    fn term(&mut self) -> Result<Value, DxfError> {
        let mut v = self.unary()?;
        loop {
            if self.op("*") {
                v = Value::Number(num(&v)? * num(&self.unary()?)?);
            } else if self.op("/") {
                v = Value::Number(num(&v)? / num(&self.unary()?)?);
            } else {
                return Ok(v);
            }
        }
    }

    fn unary(&mut self) -> Result<Value, DxfError> {
        self.enter()?;
        let v = self.unary_inner();
        self.depth -= 1;
        v
    }

    fn unary_inner(&mut self) -> Result<Value, DxfError> {
        if self.op("-") {
            return Ok(Value::Number(-num(&self.unary()?)?));
        }
        if self.op("+") {
            return Ok(Value::Number(num(&self.unary()?)?));
        }
        let a = self.atom()?;
        if self.op("^") {
            return Ok(Value::Number(num(&a)?.powf(num(&self.unary()?)?)));
        }
        Ok(a)
    }

    fn args(&mut self) -> Result<Vec<Value>, DxfError> {
        let mut out = Vec::new();
        if self.op(")") {
            return Ok(out);
        }
        loop {
            out.push(self.or()?);
            if self.op(")") {
                return Ok(out);
            }
            self.expect(",")?;
        }
    }

    fn atom(&mut self) -> Result<Value, DxfError> {
        match self.peek().cloned() {
            Some(Tok::Num(n)) => {
                self.i += 1;
                Ok(Value::Number(n))
            }
            Some(Tok::Str(s)) => {
                self.i += 1;
                Ok(Value::Text(s))
            }
            Some(Tok::Op(o)) if o == "(" => {
                self.i += 1;
                let v = self.or()?;
                self.expect(")")?;
                Ok(v)
            }
            Some(Tok::Name(n)) => {
                self.i += 1;
                self.name(n)
            }
            t => Err(fail(format!("unexpected {t:?}"))),
        }
    }

    fn name(&mut self, n: String) -> Result<Value, DxfError> {
        let low = n.to_ascii_lowercase();
        if low == "true" || low == "false" {
            return Ok(Value::Bool(low == "true"));
        }
        if low == "if" && self.op("(") {
            // both branches are evaluated, as the editor's IF function does
            let a = self.args()?;
            let [c, x, y] = a.as_slice() else { return Err(fail("IF takes three arguments")) };
            return Ok(if truth(c)? { x.clone() } else { y.clone() });
        }
        if low == "math" {
            self.expect(".")?;
            let Some(Tok::Name(f)) = self.peek().cloned() else { return Err(fail("Math function")) };
            self.i += 1;
            self.expect("(")?;
            let a: Vec<f64> = self.args()?.iter().map(num).collect::<Result<_, _>>()?;
            let one = |f: fn(f64) -> f64| a.first().map(|x| f(*x)).ok_or_else(|| fail("Math function argument"));
            let v = match f.to_ascii_lowercase().as_str() {
                "abs" => one(f64::abs)?,
                "sqrt" => one(f64::sqrt)?,
                "sin" => one(f64::sin)?,
                "cos" => one(f64::cos)?,
                "tan" => one(f64::tan)?,
                "atan" => one(f64::atan)?,
                "floor" => one(f64::floor)?,
                "ceiling" => one(f64::ceil)?,
                "round" => one(f64::round_ties_even)?,
                "min" | "max" if a.len() == 2 => {
                    if f.eq_ignore_ascii_case("min") {
                        a[0].min(a[1])
                    } else {
                        a[0].max(a[1])
                    }
                }
                _ => return Err(fail(format!("Math.{f}"))),
            };
            return Ok(Value::Number(v));
        }
        if low == "sa" {
            self.expect(".")?;
            return match self.peek().cloned() {
                Some(Tok::Name(m)) if m.eq_ignore_ascii_case("IsLayout") => {
                    self.i += 1;
                    Ok(Value::Bool(true))
                }
                t => Err(fail(format!("SA.{t:?}"))),
            };
        }
        let mut v = self.scope.get(&n).ok_or_else(|| fail(format!("unknown name {n}")))?;
        while self.op(".") {
            let Some(Tok::Name(m)) = self.peek().cloned() else { return Err(fail("member name")) };
            self.i += 1;
            let m = m.to_ascii_lowercase();
            v = match (&v, m.as_str()) {
                (Value::Enum(_, name), "value") => Value::Text(name.clone()),
                (Value::Target(valid, _), "isvalid") => Value::Bool(*valid),
                (Value::Target(_, x), "offset" | "elevation" | "value") => Value::Number(*x),
                (Value::Point(x, _), "x" | "offset") => Value::Number(*x),
                (Value::Point(_, y), "y" | "elevation") => Value::Number(*y),
                _ => return Err(fail(format!("member {m} of {n}"))),
            };
        }
        Ok(v)
    }
}

fn text(v: &Value) -> String {
    match v {
        Value::Text(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Enum(_, n) => n.clone(),
        Value::Bool(b) => if *b { "True" } else { "False" }.into(),
        v => format!("{v:?}"),
    }
}

/// Evaluates an argument: `[expression]`, or a literal (a number, else
/// text).
pub fn eval(s: &str, scope: &dyn Scope) -> Result<Value, DxfError> {
    let t = s.trim();
    let Some(inner) = t.strip_prefix('[').and_then(|x| x.strip_suffix(']')) else {
        return Ok(t.parse().map(Value::Number).unwrap_or_else(|_| Value::Text(t.to_string())));
    };
    let mut p = P { t: tokens(inner)?, i: 0, scope, depth: 0 };
    let v = p.or()?;
    if p.i != p.t.len() {
        return Err(fail(format!("unexpected text after {inner}")));
    }
    Ok(v)
}

pub fn number(s: &str, scope: &dyn Scope) -> Result<f64, DxfError> {
    num(&eval(s, scope)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct S;
    impl Scope for S {
        fn get(&self, n: &str) -> Option<Value> {
            match n.to_ascii_lowercase().as_str() {
                "width" => Some(Value::Number(4.0)),
                "side" => Some(Value::Enum(0, "Right".into())),
                "left" => Some(Value::Enum(1, "Left".into())),
                "t" => Some(Value::Target(false, 7.0)),
                "p1" => Some(Value::Point(1.0, -2.0)),
                _ => None,
            }
        }
    }

    #[test]
    fn refuses_expressions_nested_too_deep() {
        let deep = format!("[{}1{}]", "(".repeat(10_000), ")".repeat(10_000));
        assert!(eval(&deep, &S).is_err());
        assert!(eval(&format!("[{}1]", "-".repeat(10_000)), &S).is_err());
        assert!(eval(&format!("[{}True]", "Not ".repeat(10_000)), &S).is_err());
        assert_eq!(number(&format!("[{}2{}]", "(".repeat(50), ")".repeat(50)), &S).unwrap(), 2.0);
    }

    #[test]
    fn evaluates_the_subset() {
        let n = |s: &str| number(s, &S).unwrap();
        assert_eq!(n("[-Width/2 + 3*2^2]"), 10.0);
        assert_eq!(n("[IF(T.IsValid, T.Offset, Width)]"), 4.0);
        assert_eq!(n("[math.abs(-3) + Math.Max(1, 2)]"), 5.0);
        assert_eq!(n("[P1.y]"), -2.0);
        assert_eq!(n("2.5"), 2.5);
        assert_eq!(eval("[Side.Value]", &S).unwrap(), Value::Text("Right".into()));
        assert_eq!(eval("[Side = Left Or Not (Width < 3)]", &S).unwrap(), Value::Bool(true));
        assert_eq!(eval("[Side.Value = \"Right\" AndAlso SA.IsLayout]", &S).unwrap(), Value::Bool(true));
        assert!(eval("[Side.ToString()]", &S).is_err());
        assert!(eval("[Foo + 1]", &S).is_err());
        assert!(eval("[Width.Bar]", &S).is_err());
        assert!(eval("[Width +]", &S).is_err());
    }
}
