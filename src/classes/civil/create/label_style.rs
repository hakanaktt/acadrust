//! Label styles written as copies of a drawing's label style with chosen
//! settings (see `label_style.rs` for the fields).

use super::labels::{default_label_style, ensure_label_styles, POINT_LABELS};
use super::support::*;
use crate::classes::civil::{LabelStyleSettings, LabelTextComponent};
use crate::error::DxfError;
use crate::io::dwg::dwg_reference_type::DwgReferenceType;
use crate::types::Handle;
use crate::CadDocument;

/// Text ids this crate gives the label styles it adds (the first one,
/// then one more than the highest in the collection).
const FIRST_LABEL_STYLE_ID: u32 = 0x8004_0001;

/// A label style: its settings and text components.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelStyle {
    pub settings: LabelStyleSettings,
    pub components: Vec<LabelTextComponent>,
}

/// The label style `style` (`AECC_LABEL_COLLECTOR_STYLE`) with its text
/// components (`AECC_LABEL_TEXT_STYLE`, in order; components of other
/// kinds are not listed); `None` when it or a text component cannot be
/// read.
pub fn label_style(doc: &CadDocument, style: Handle) -> Option<LabelStyle> {
    let o = object(doc, style)?;
    let components = o.label_style_components().into_iter().filter(|h| is_text(doc, *h)).map(|h| object(doc, Handle::new(h))?.label_text_component()).collect::<Option<Vec<_>>>()?;
    Some(LabelStyle { settings: o.label_style_settings()?, components })
}

fn is_text(doc: &CadDocument, component: u64) -> bool {
    object(doc, Handle::new(component)).is_some_and(|o| o.dxf_name == "AECC_LABEL_TEXT_STYLE")
}

fn kind(k: u8) -> DwgReferenceType {
    match k {
        2 => DwgReferenceType::SoftOwnership,
        3 => DwgReferenceType::HardOwnership,
        4 => DwgReferenceType::SoftPointer,
        _ => DwgReferenceType::HardPointer,
    }
}

fn text_id(id: u32) -> bool {
    id & 0x8000_0000 != 0
}

/// The id and the bits between the children list and the class data of
/// a style record without children.
fn head(doc: &CadDocument, h: Handle, class: usize) -> Option<(u32, String)> {
    let (data, start, _) = fields_of(object(doc, h)?)?;
    let mut b = crate::classes::civil::bits::Bits::new(data, start, class);
    let id = b.bit_long()? as u32;
    (b.bit_long()? == 0).then_some(())?;
    Some((id, (b.pos..class).map(|i| if data[i / 8] >> (7 - i % 8) & 1 == 1 { '1' } else { '0' }).collect()))
}

/// Adds a label style `name` to the collection of the label style
/// `source`: a copy of it and its components with the settings and text
/// components `edit` gives (it gets the source's, read with
/// [`label_style`]); components of other kinds (lines, arrows, ...) are
/// copied as they are. The components keep their ids and order; `edit`
/// changes their values, not their number. Each setting `None` takes the
/// label style defaults' value.
pub fn add_label_style(doc: &mut CadDocument, source: Handle, name: &str, edit: impl FnOnce(&mut LabelStyle)) -> Result<Handle, DxfError> {
    if name.trim().is_empty() {
        return Err(invalid("a label style needs a name"));
    }
    let src = object(doc, source).ok_or_else(|| invalid("source label style not found"))?;
    let (mut fields, own, _) = src.label_fields(true).ok_or_else(|| invalid("not a label style this crate reads"))?;
    let collection = src.owner;
    let mut style = label_style(doc, source).ok_or_else(|| invalid("label style components cannot be read"))?;
    let count = style.components.len();
    edit(&mut style);
    if style.components.len() != count {
        return Err(invalid("a label style copy keeps the source's components"));
    }
    let node = read_node(doc, collection).ok_or_else(|| invalid("label style collection cannot be read"))?;
    if node.names.iter().any(|n| n.eq_ignore_ascii_case(name)) {
        return Err(invalid("a label style of that name exists"));
    }
    let id = node.children.iter().map(|c| c.0 as u32).filter(|k| *k & 0xFFFF_0000 == FIRST_LABEL_STYLE_ID & 0xFFFF_0000).max().map_or(FIRST_LABEL_STYLE_ID, |k| k + 1);
    let (data, start, end) = src.main_stream().ok_or_else(|| invalid("label style record cannot be read"))?;
    let class = crate::classes::civil::style::class_layout(data, start, end).ok_or_else(|| invalid("label style record cannot be read"))?.0;
    let (src_id, rest) = head(doc, source, class).ok_or_else(|| invalid("label style with child styles cannot be copied"))?;
    let typed = src.typed_handles();
    let strings = src.decode().strings;
    let reactors = src.reactors.clone();
    let comps = src.label_style_components();

    let handle = doc.allocate_handle();
    let new_comps: Vec<Handle> = comps.iter().map(|_| doc.allocate_handle()).collect();
    let mut values: Vec<u64> = typed[own..].iter().map(|r| r.0).collect();
    fields.set_settings(&style.settings, &mut values);
    let base = crate::classes::civil::label_style::BASE_REFS;
    for list in 0..3 {
        for (i, h) in new_comps.iter().enumerate() {
            if let Some(slot) = values.get_mut(base + list * new_comps.len() + i) {
                *slot = h.value();
            }
        }
    }
    let mut refs: Vec<Ref> = typed[..own].iter().map(|r| Ref(kind(r.1), r.0)).collect();
    refs.extend(typed[own..].iter().zip(&values).map(|(r, v)| Ref(kind(r.1), *v)));
    let own_texts = if text_id(src_id) { 2 } else { 1 };
    let mut texts = vec![name.to_string(), String::new()];
    texts.extend(strings.get(own_texts..).unwrap_or_default().iter().cloned());
    put_object(doc, "AECC_LABEL_COLLECTOR_STYLE", handle, collection, reactors, |w| {
        w.write_bit_long(id as i32);
        w.write_bit_long(0);
        bits(w, &rest);
        fields.write(w);
    }, Body { refs, strings: texts })?;
    let mut edited = style.components.iter();
    for (c, new) in comps.iter().zip(&new_comps) {
        let comp = if is_text(doc, *c) { Some(edited.next().ok_or_else(|| invalid("label style components changed"))?) } else { None };
        copy_component(doc, Handle::new(*c), *new, handle, comp)?;
    }
    add_node_child(doc, collection, id as i32, handle, name)?;
    Ok(handle)
}

/// Writes a copy of the component `source` as `handle`, owned by
/// `style`: a text component with the values of `c`, a component of
/// another kind as it is.
fn copy_component(doc: &mut CadDocument, source: Handle, handle: Handle, style: Handle, c: Option<&LabelTextComponent>) -> Result<(), DxfError> {
    let src = object(doc, source).ok_or_else(|| invalid("label style component not found"))?;
    let dxf = src.dxf_name.clone();
    let (data, start, end) = src.main_stream().ok_or_else(|| invalid("label style component cannot be read"))?;
    let (class, own, text_keys) = crate::classes::civil::style::class_layout(data, start, end).ok_or_else(|| invalid("label style component cannot be read"))?;
    let (id, rest) = head(doc, source, class).ok_or_else(|| invalid("label style component cannot be read"))?;
    let class_bits: String = (class..end).map(|i| if data[i / 8] >> (7 - i % 8) & 1 == 1 { '1' } else { '0' }).collect();
    let typed = src.typed_handles();
    let strings = src.decode().strings;
    let mut values: Vec<u64> = typed[own..].iter().map(|r| r.0).collect();
    let fields = match c {
        Some(c) => {
            let mut fields = src.label_fields(false).ok_or_else(|| invalid("not a label text component this crate reads"))?.0;
            fields.set_text_component(c, &mut values);
            Some(fields)
        }
        None => None,
    };
    if let Some(slot) = values.get_mut(crate::classes::civil::label_style::REF_OWNER) {
        *slot = style.value();
    }
    let mut refs: Vec<Ref> = typed[..own].iter().map(|r| Ref(kind(r.1), r.0)).collect();
    refs.extend(typed[own..].iter().zip(&values).map(|(r, v)| Ref(kind(r.1), *v)));
    let texts = match c {
        Some(c) => {
            let own_texts = if text_id(id) { 2 } else { 1 };
            let mut texts: Vec<String> = if text_id(id) { vec![c.name.clone().unwrap_or_default(), String::new()] } else { vec![String::new()] };
            texts.extend(strings.iter().skip(own_texts).take(text_keys).cloned());
            if text_id(c.anchor_component) {
                texts.push(c.anchor_name.clone().ok_or_else(|| invalid("an anchor component under a text id needs its name"))?);
            }
            texts
        }
        None => strings,
    };
    put_object(doc, &dxf, handle, style, Vec::new(), |w| {
        w.write_bit_long(id as i32);
        w.write_bit_long(0);
        bits(w, &rest);
        match &fields {
            Some(f) => f.write(w),
            None => bits(w, &class_bits),
        }
    }, Body { refs, strings: texts })
}

/// [`add_label_style`] from the drawing's default point label style
/// (written with the default label styles when the drawing lacks them).
pub fn add_point_label_style(doc: &mut CadDocument, name: &str, edit: impl FnOnce(&mut LabelStyle)) -> Result<Handle, DxfError> {
    ensure_label_styles(doc)?;
    let source = default_label_style(doc, POINT_LABELS).ok_or_else(|| invalid("drawing has no default point label style"))?;
    add_label_style(doc, Handle::new(source), name, edit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Color, Vector3};

    /// A copy of the default point label style reads back with the
    /// settings and component values it was written with.
    #[test]
    fn point_label_style_reads_back_as_written() {
        let mut doc = CadDocument::new();
        crate::classes::civil::add_cogo_point(&mut doc, None, Vector3::new(1.0, 2.0, 3.0), None, None, None).unwrap();
        let h = add_point_label_style(&mut doc, "LS1", |s| {
            s.settings.orientation = Some(2);
            s.settings.leader.color = Some(Color::from_index(1));
            s.settings.dragged.max_text_width = Some(0.5);
            s.components[0].height = Some(0.25);
            s.components[0].contents = "N <[16797697(2803|2817)]>".into();
            s.components[1].x_offset = 0.1;
            s.components[1].text_color = Some(Color::ByBlock);
        })
        .unwrap();
        let back = label_style(&doc, h).unwrap();
        assert_eq!(back.settings.orientation, Some(2));
        assert_eq!(back.settings.leader.color, Some(Color::from_index(1)));
        assert_eq!(back.settings.dragged.max_text_width, Some(0.5));
        assert_eq!(back.settings.insert, None);
        assert_eq!(back.components.len(), 3);
        assert_eq!((back.components[0].height, back.components[0].contents.as_str()), (Some(0.25), "N <[16797697(2803|2817)]>"));
        assert_eq!((back.components[1].x_offset, back.components[1].text_color), (0.1, Some(Color::ByBlock)));
        assert!(add_point_label_style(&mut doc, "ls1", |_| {}).is_err());
    }
}
