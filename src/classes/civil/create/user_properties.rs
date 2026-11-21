//! User-defined properties of COGO points: classifications, property
//! definitions and the values points hold.
//!
//! The class node of COGO points (`AECC_CLASS_NODE` named
//! `AeccDbCogoPoint`) lists the classifications as children and, after
//! them, the properties:
//!
//! ```text
//! BL 0, B has properties,
//!   [BL n, n × { BL key, default (integer BL, double BD, boolean RC, text: none) }, B 0],
//! BL m, m × BL key                users of each key: the point database
//!                                 (while a point holds a value) and the definition
//! handles: children, m × soft pointer (the users), hard owner 0
//! strings: own name, "", the children's names, each property's key text
//!          (and its text default), each user's key text, "AeccDbCogoPoint"
//! ```
//!
//! A classification (`AECC_USER_DEFINED_ATTRIBUTE_CLASSIFICATION`) is a
//! tree node keyed by a text id of its name whose children are its
//! properties (`AECC_USER_DEFINED_ATTRIBUTE`, see
//! [`UserProperty`](crate::classes::civil::UserProperty)).

use super::points::{cogo_points, store_points, support};
use super::support::*;
use crate::classes::civil::cogo::{key_code, INCLUSIVE};
use crate::classes::civil::{bits::Bits, PropertyValue, UserProperty};
use crate::error::DxfError;
use crate::io::dwg::dwg_stream_writers::DwgMergedWriter;
use crate::types::Handle;
use crate::CadDocument;

/// Word after the bounds of a property (the same in every saved sample).
const PROPERTY_TAIL: i32 = 0x4000_806B;
/// First text id given to classification names. Its bucket (key mod 8)
/// is the last one, so a new classification follows `Unclassified` (key
/// 0xEA6): written before it (a key in a lower bucket), the application
/// reports errors on opening once the classification holds a property.
const FIRST_CLASSIFICATION_TAG: u32 = 0x8003_0007;

/// The property part of the COGO point class node.
struct ClassNode {
    tail: NodeTail,
    /// (key, key text, default).
    defaults: Vec<(i32, String, PropertyValue)>,
    /// (key, key text, user).
    users: Vec<(i32, String, u64)>,
    /// Texts and references after the property part.
    rest_texts: Vec<String>,
    rest_refs: Vec<Ref>,
}

fn read_class(doc: &CadDocument, node: Handle) -> Option<ClassNode> {
    let tail = read_node(doc, node)?;
    let (data, start, end) = fields_of(object(doc, node)?)?;
    let mut b = Bits::new(data, start, end);
    b.bit_long()?;
    for _ in 0..b.bit_long()? {
        b.bit_long()?;
    }
    (b.bit_long()? == 0).then_some(())?;
    // `tail.texts` follows the children's names.
    let mut texts = tail.texts.clone().into_iter();
    let mut defaults = Vec::new();
    if b.bit()? {
        for _ in 0..b.bit_long()? {
            let key = b.bit_long()?;
            let text = texts.next()?;
            defaults.push((key, text, PropertyValue::read(&mut b, key_code(key), &mut texts)?));
        }
        (!b.bit()?).then_some(())?;
    }
    let mut refs = tail.refs.clone().into_iter();
    let mut users = Vec::new();
    for _ in 0..b.bit_long()? {
        users.push((b.bit_long()?, texts.next()?, refs.next()?.1));
    }
    (b.pos == end).then_some(())?;
    Some(ClassNode { tail, defaults, users, rest_texts: texts.collect(), rest_refs: refs.collect() })
}

fn write_value(w: &mut DwgMergedWriter, value: &PropertyValue) {
    match value {
        PropertyValue::Integer(v) => w.write_bit_long(*v),
        PropertyValue::Double(v) => w.write_bit_double(*v),
        PropertyValue::Boolean(v) => w.write_byte(*v as u8),
        PropertyValue::Choice(v) => w.write_bit_long(*v as i32),
        PropertyValue::Text(_) => {}
    }
}

fn write_class(doc: &mut CadDocument, node: Handle, c: &ClassNode) -> Result<(), DxfError> {
    let o = object(doc, node).ok_or_else(|| invalid("class node cannot be read"))?;
    let (owner, reactors) = (o.owner, o.reactors.clone());
    let t = &c.tail;
    let mut refs: Vec<Ref> = t.children.iter().map(|ch| soft_owner(ch.1)).collect();
    refs.extend(c.users.iter().map(|u| soft_pointer(u.2)));
    refs.extend(c.rest_refs.iter().copied());
    let mut strings = t.own.clone();
    strings.extend(t.names.iter().cloned());
    for (_, text, value) in &c.defaults {
        strings.push(text.clone());
        if let PropertyValue::Text(s) = value {
            strings.push(s.clone());
        }
    }
    strings.extend(c.users.iter().map(|u| u.1.clone()));
    strings.extend(c.rest_texts.iter().cloned());
    put_object(
        doc,
        "AECC_CLASS_NODE",
        node,
        owner,
        reactors,
        |w| {
            w.write_bit_long(t.key);
            w.write_bit_long(t.children.len() as i32);
            for ch in &t.children {
                w.write_bit_long(ch.0);
            }
            w.write_bit_long(0);
            w.write_bit(!c.defaults.is_empty());
            if !c.defaults.is_empty() {
                w.write_bit_long(c.defaults.len() as i32);
                for (key, _, value) in &c.defaults {
                    w.write_bit_long(*key);
                    write_value(w, value);
                }
                w.write_bit(false);
            }
            w.write_bit_long(c.users.len() as i32);
            for u in &c.users {
                w.write_bit_long(u.0);
            }
        },
        Body { refs, strings },
    )
}

fn class_node(doc: &mut CadDocument) -> Result<(Handle, ClassNode), DxfError> {
    let h = Handle::new(support(doc)?.defaults.class_node);
    let c = read_class(doc, h).ok_or_else(|| invalid("COGO point class node cannot be read"))?;
    Ok((h, c))
}

/// Classification records under the COGO point class node: (name, key, handle).
fn classifications(doc: &CadDocument, c: &ClassNode) -> Vec<(String, i32, u64)> {
    let mut names = c.tail.names.iter();
    c.tail
        .children
        .iter()
        .filter(|ch| ch.0 as u32 & 0x8000_0000 != 0)
        .filter_map(|ch| Some((names.next()?.clone(), ch.0, ch.1)))
        .filter(|ch| object(doc, Handle::new(ch.2)).is_some_and(|o| o.dxf_name == "AECC_USER_DEFINED_ATTRIBUTE_CLASSIFICATION"))
        .collect()
}

/// The user-defined point properties of the drawing.
pub fn point_properties(doc: &CadDocument) -> Vec<UserProperty> {
    doc.objects
        .values()
        .filter_map(|o| match o {
            crate::objects::ObjectType::Arch(a) => a.user_property(),
            _ => None,
        })
        .collect()
}

/// Adds a point property classification named `name` (unique, case
/// ignored).
pub fn add_point_property_classification(doc: &mut CadDocument, name: &str) -> Result<Handle, DxfError> {
    if name.trim().is_empty() {
        return Err(invalid("a classification needs a name"));
    }
    let (node, c) = class_node(doc)?;
    let existing = classifications(doc, &c);
    if existing.iter().any(|e| e.0.eq_ignore_ascii_case(name)) {
        return Err(invalid("classification name already used"));
    }
    let used: Vec<u32> = c.tail.children.iter().map(|ch| ch.0 as u32).collect();
    let key = (FIRST_CLASSIFICATION_TAG..).step_by(8).find(|k| !used.contains(k)).unwrap_or(FIRST_CLASSIFICATION_TAG) as i32;
    let handle = doc.allocate_handle();
    put_object(
        doc,
        "AECC_USER_DEFINED_ATTRIBUTE_CLASSIFICATION",
        handle,
        node,
        Vec::new(),
        |w| {
            w.write_bit_long(key);
            w.write_bit_long(0);
            w.write_bit_long(0);
            bits(w, "010");
        },
        Body { refs: Vec::new(), strings: vec![name.to_string(), String::new()] },
    )?;
    add_node_child(doc, node, key, handle, name)?;
    Ok(handle)
}

/// Key text of a new property: 32 hexadecimal digits of a new identifier
/// (as the application prints them) and the name.
fn key_text(name: &str, seed: u64) -> String {
    let g = super::profile::new_identifier(seed);
    let u32_at = |i: usize| u32::from_le_bytes([g[i], g[i + 1], g[i + 2], g[i + 3]]);
    let u16_at = |i: usize| u16::from_le_bytes([g[i], g[i + 1]]);
    format!("{:08x}{:04x}{:04x}{:08x}{:08x}{name}", u32_at(0), u16_at(4), u16_at(6), u32_at(8), u32_at(12))
}

/// Default bounds of a kind: the full range, inclusive (booleans 0 to 1,
/// stored without the inclusive flag).
fn full_range(value: &PropertyValue) -> Option<((PropertyValue, bool), (PropertyValue, bool))> {
    match value {
        PropertyValue::Integer(_) => Some(((PropertyValue::Integer(i32::MIN), true), (PropertyValue::Integer(i32::MAX), true))),
        PropertyValue::Double(_) => Some(((PropertyValue::Double(-1e24), true), (PropertyValue::Double(1e24), true))),
        PropertyValue::Boolean(_) => Some(((PropertyValue::Boolean(false), false), (PropertyValue::Boolean(true), false))),
        PropertyValue::Text(_) | PropertyValue::Choice(_) => None,
    }
}

fn as_number(v: &PropertyValue) -> Option<f64> {
    match v {
        PropertyValue::Integer(i) => Some(f64::from(*i)),
        PropertyValue::Double(d) => Some(*d),
        PropertyValue::Boolean(b) => Some(f64::from(u8::from(*b))),
        PropertyValue::Text(_) | PropertyValue::Choice(_) => None,
    }
}

/// Whether `value` lies within `bounds` (a boolean always does: its
/// bounds are stored without the inclusive flag).
fn within(value: &PropertyValue, bounds: &Option<((PropertyValue, bool), (PropertyValue, bool))>) -> bool {
    let (Some(((lo, lo_in), (hi, hi_in))), false) = (bounds, matches!(value, PropertyValue::Boolean(_))) else { return true };
    match (as_number(value), as_number(lo), as_number(hi)) {
        (Some(v), Some(l), Some(h)) => (if *lo_in { v >= l } else { v > l }) && (if *hi_in { v <= h } else { v < h }),
        _ => false,
    }
}

/// Adds a point property `name` to the classification `classification`.
/// `default` gives its kind; `bounds` `None` bounds a number by its kind's
/// full range (texts have none). Returns the property's key text, the key
/// of the values points hold ([`set_cogo_point_property`]). Enumerations
/// are added with [`add_point_choice_property`].
pub fn add_point_property(
    doc: &mut CadDocument,
    classification: &str,
    name: &str,
    description: &str,
    default: PropertyValue,
    use_default: bool,
    bounds: Option<((PropertyValue, bool), (PropertyValue, bool))>,
) -> Result<String, DxfError> {
    if matches!(default, PropertyValue::Choice(_)) {
        return Err(invalid("an enumeration is added with add_point_choice_property"));
    }
    add_property(doc, classification, name, description, default, use_default, bounds, &[])
}

/// Adds an enumeration point property `name` with `choices` (distinct,
/// in order) and the choice `default` (an index) to `classification`; a
/// point's value is a [`PropertyValue::Choice`] index.
pub fn add_point_choice_property(doc: &mut CadDocument, classification: &str, name: &str, description: &str, choices: &[&str], default: u32, use_default: bool) -> Result<String, DxfError> {
    if choices.is_empty() || choices.iter().any(|c| c.is_empty()) || (1..choices.len()).any(|i| choices[..i].contains(&choices[i])) || default as usize >= choices.len() {
        return Err(invalid("an enumeration needs distinct choices and a default among them"));
    }
    let choices: Vec<String> = choices.iter().map(|c| c.to_string()).collect();
    add_property(doc, classification, name, description, PropertyValue::Choice(default), use_default, None, &choices)
}

#[allow(clippy::too_many_arguments)]
fn add_property(
    doc: &mut CadDocument,
    classification: &str,
    name: &str,
    description: &str,
    default: PropertyValue,
    use_default: bool,
    bounds: Option<((PropertyValue, bool), (PropertyValue, bool))>,
    choices: &[String],
) -> Result<String, DxfError> {
    if name.trim().is_empty() {
        return Err(invalid("a property needs a name"));
    }
    let (node, mut c) = class_node(doc)?;
    let Some((class_name, _, class_handle)) = classifications(doc, &c).into_iter().find(|e| e.0.eq_ignore_ascii_case(classification)) else {
        return Err(invalid("no such classification"));
    };
    if point_properties(doc).iter().any(|p| p.classification == class_name && p.name.eq_ignore_ascii_case(name)) {
        return Err(invalid("property name already used in the classification"));
    }
    let code = default.code();
    let bounds = match (&default, bounds) {
        (PropertyValue::Text(_) | PropertyValue::Choice(_), Some(_)) => return Err(invalid("text and enumeration properties have no bounds")),
        (_, Some(b)) if b.0 .0.code() != code || b.1 .0.code() != code => return Err(invalid("bounds are not of the property's kind")),
        (_, Some(b)) => Some(b),
        (v, None) => full_range(v),
    };
    if !within(&default, &bounds) {
        return Err(invalid("default lies outside the bounds"));
    }
    let serial = c.defaults.iter().filter(|d| key_code(d.0) == code).map(|d| d.0 as u32 & 0x00FF_FFFF).max().unwrap_or(0) + 1;
    let key = (0x8000_0000 | code << 24 | serial) as i32;
    let handle = doc.allocate_handle();
    let text = key_text(name, handle.value());
    let class = match code {
        1 => "AeccAttributeTypeInfoInt",
        2 => "AeccAttributeTypeInfoDouble",
        3 => "AeccAttributeTypeInfoBool",
        13 => "AeccAttributeTypeInfoUserDefinedEnum",
        _ => "AeccAttributeTypeInfoString",
    };
    let mut strings = vec![text.clone(), String::new(), text.clone()];
    if let PropertyValue::Text(s) = &default {
        strings.push(s.clone());
    }
    strings.extend([text.clone(), class.to_string()]);
    strings.extend(choices.iter().cloned());
    strings.extend([name.to_string(), description.to_string(), class_name.clone()]);
    put_object(
        doc,
        "AECC_USER_DEFINED_ATTRIBUTE",
        handle,
        Handle::new(class_handle),
        Vec::new(),
        |w| {
            w.write_bit_long(key);
            w.write_bit_long(0);
            w.write_bit_long(0);
            bits(w, "010");
            w.write_bit_long(key);
            write_value(w, &default);
            w.write_bit_long(key);
            w.write_bit(use_default);
            w.write_bit(bounds.is_some());
            if let Some(((lo, lo_in), (hi, hi_in))) = &bounds {
                for (v, inclusive) in [(lo, lo_in), (hi, hi_in)] {
                    w.write_bit_long((code << 24 | if *inclusive { INCLUSIVE } else { 0 }) as i32);
                    write_value(w, v);
                }
            }
            if code == 13 {
                w.write_bit_long(choices.len() as i32);
            }
            w.write_bit_long(PROPERTY_TAIL);
            bits(w, "010");
        },
        Body { refs: vec![hard_pointer(handle.value())], strings },
    )?;
    add_node_child(doc, Handle::new(class_handle), key, handle, &text)?;
    c.defaults.push((key, text.clone(), default));
    c.users.push((key, text.clone(), handle.value()));
    write_class(doc, node, &c)?;
    Ok(text)
}

/// Sets (or with `None` clears) the value point `point` holds for the
/// property with key text `key`.
pub fn set_cogo_point_property(doc: &mut CadDocument, point: Handle, key: &str, value: Option<PropertyValue>) -> Result<(), DxfError> {
    let property = point_properties(doc).into_iter().find(|p| p.key == key).ok_or_else(|| invalid("no such point property"))?;
    if let Some(v) = &value {
        if v.code() != property.default.code() {
            return Err(invalid("value is not of the property's kind"));
        }
        if !within(v, &property.bounds) || matches!(v, PropertyValue::Choice(i) if *i as usize >= property.choices.len()) {
            return Err(invalid("value lies outside the property's bounds or choices"));
        }
    }
    let number = super::points::point_number(doc, point)?;
    let mut points = cogo_points(doc);
    let p = points.iter_mut().find(|p| p.number == number).ok_or_else(|| invalid("point is not in the database"))?;
    p.properties.retain(|(k, _)| k != key);
    if let Some(v) = value {
        p.properties.push((key.to_string(), v));
    }
    store_points(doc, &points)
}

/// Keys of the point properties by key text (for the database writer).
pub(super) fn property_keys(doc: &CadDocument, class_node: u64) -> Vec<(String, i32)> {
    read_class(doc, Handle::new(class_node)).map(|c| c.defaults.into_iter().map(|d| (d.1, d.0)).collect()).unwrap_or_default()
}

/// Lists the point database as a user of each key some point holds a
/// value of (before the definition), as the application does, and drops
/// it from the others.
pub(super) fn sync_users(doc: &mut CadDocument, class_node: u64, database: u64, held: &[i32]) -> Result<(), DxfError> {
    let node = Handle::new(class_node);
    let Some(mut c) = read_class(doc, node) else { return Ok(()) };
    if c.defaults.is_empty() {
        return Ok(());
    }
    let mut users: Vec<(i32, String, u64)> = c.users.iter().filter(|u| u.2 != database).cloned().collect();
    for &key in held {
        if let Some(at) = users.iter().position(|u| u.0 == key) {
            let text = users[at].1.clone();
            users.insert(at, (key, text, database));
        }
    }
    if users == c.users {
        return Ok(());
    }
    c.users = users;
    write_class(doc, node, &c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_check_inclusive_and_exclusive_ends() {
        let b = Some(((PropertyValue::Double(0.0), false), (PropertyValue::Double(10.0), true)));
        assert!(!within(&PropertyValue::Double(0.0), &b));
        assert!(within(&PropertyValue::Double(10.0), &b));
        assert!(!within(&PropertyValue::Double(10.5), &b));
        assert!(within(&PropertyValue::Text("a".into()), &None));
        let k = key_text("P1", 7);
        assert_eq!(k.len(), 34);
        assert!(k[..32].bytes().all(|c| c.is_ascii_hexdigit()) && k.ends_with("P1"));
        // The version digit of the identifier (the 13th) is 4.
        assert_eq!(&k[12..13], "4");
    }
}
