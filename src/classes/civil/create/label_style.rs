//! Label styles written as copies of a drawing's label style with chosen
//! settings (see `label_style.rs` for the fields).

use super::labels::{default_label_style, ensure_label_styles, POINT_LABELS};
use super::support::*;
use crate::classes::civil::{LabelArrowComponent, LabelBlockComponent, LabelIteratorComponent, LabelLineComponent, LabelStyleSettings, LabelTextComponent, LabelTickComponent};
use crate::error::DxfError;
use crate::io::dwg::dwg_reference_type::DwgReferenceType;
use crate::types::Handle;
use crate::CadDocument;

/// Text ids this crate gives the label styles it adds (the first one,
/// then one more than the highest in the collection).
const FIRST_LABEL_STYLE_ID: u32 = 0x8004_0001;
/// Text ids this crate gives the text, line, tick and direction arrow
/// components it adds to a style.
const FIRST_COMPONENT_ID: u32 = 0x8005_0001;
/// Ids the label style editor gives the block components it adds (one
/// more for each: LC06).
const FIRST_BLOCK_ID: u32 = 0xA71;

/// A label style: its settings and text components.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelStyle {
    pub settings: LabelStyleSettings,
    pub components: Vec<LabelTextComponent>,
    /// The line, block, tick and direction arrow components, in order: a
    /// copy writes the values of those it keeps, leaves out those removed
    /// and adds those with id 0 after the others.
    pub lines: Vec<LabelLineComponent>,
    pub blocks: Vec<LabelBlockComponent>,
    pub ticks: Vec<LabelTickComponent>,
    pub arrows: Vec<LabelArrowComponent>,
    /// The text iterators: a copy writes the values of those it keeps
    /// and leaves out those removed (none can be added).
    pub iterators: Vec<LabelIteratorComponent>,
}

impl LabelStyle {
    /// A line component to add (id 0) named `name`, with the values the
    /// label style editor gives a new one (LC04): from the labelled
    /// object's location, 1/12 long at angle 0, the defaults' color,
    /// lineweight and linetype.
    pub fn new_line_component(&self, name: &str) -> Option<LabelLineComponent> {
        crate::classes::civil::label_style::LabelFields::new_component(true)?.line_component(0, Some(name.to_string()), &[0; 9])
    }

    /// A block component to add (id 0) with the values the label style
    /// editor gives a new one (LC05): height 1/120, at the insertion
    /// point, no block (set `block` to a block record).
    pub fn new_block_component(&self) -> Option<LabelBlockComponent> {
        crate::classes::civil::label_style::LabelFields::new_component(false)?.block_component(0, None, &[0; 10])
    }

    /// A tick component to add (id 0) named `name`, with the values the
    /// label style editor gives a new one (TA000): the default tick, 1/120
    /// high, aligned with the labelled object.
    pub fn new_tick_component(&self, name: &str) -> Option<LabelTickComponent> {
        crate::classes::civil::label_style::LabelFields::new_mark(true)?.tick_component(0, Some(name.to_string()), &[0; 10])
    }

    /// A direction arrow component to add (id 0) named `name`, with the
    /// values the label style editor gives a new one (TA000): at least
    /// 1/40 long, arrowhead 1/120.
    pub fn new_arrow_component(&self, name: &str) -> Option<LabelArrowComponent> {
        crate::classes::civil::label_style::LabelFields::new_mark(false)?.arrow_component(0, Some(name.to_string()))
    }

    /// A text component to add to the style (id 0) named `name`, with the
    /// values the label style editor gives a new one (LC01): "Label Text"
    /// at the labelled object's location, middle center attachment, the
    /// defaults' height and colors; the border is the first component's,
    /// hidden. `None` for a style without text components.
    pub fn new_text_component(&self, name: &str) -> Option<LabelTextComponent> {
        let first = self.components.first()?;
        Some(LabelTextComponent {
            id: 0,
            name: Some(name.to_string()),
            contents: "Label Text".into(),
            height: None,
            visible: true,
            anchor_component: 0,
            anchor_name: None,
            anchor_point: -4,
            attachment: 4,
            x_offset: 0.0,
            y_offset: 0.0,
            angle: 0.0,
            max_width: 0.0,
            text_color: None,
            text_lineweight: None,
            border: crate::classes::civil::LabelBorder { visible: false, border_type: 0, background_mask: false, ..first.border.clone() },
        })
    }
}

/// The label style `style` (`AECC_LABEL_COLLECTOR_STYLE`) with its text
/// components (`AECC_LABEL_TEXT_STYLE`, in order; components of other
/// kinds are not listed); `None` when it or a text component cannot be
/// read.
pub fn label_style(doc: &CadDocument, style: Handle) -> Option<LabelStyle> {
    let o = object(doc, style)?;
    let comps = o.label_style_components();
    let components = comps.iter().filter(|h| is_text(doc, **h)).map(|h| object(doc, Handle::new(*h))?.label_text_component()).collect::<Option<Vec<_>>>()?;
    let of = |dxf: &str| comps.iter().filter_map(|h| object(doc, Handle::new(*h))).filter(|c| c.dxf_name == dxf).cloned().collect::<Vec<_>>();
    // Line and block components of layouts this crate does not read are
    // left out (and copied as they are).
    let lines = of("AECC_LABEL_RADIAL_LINE_STYLE").iter().filter_map(|c| c.label_line_component()).collect();
    let blocks = of("AECC_LABEL_BLOCK_STYLE").iter().filter_map(|c| c.label_block_component()).collect();
    let ticks = of("AECC_LABEL_TICK_STYLE").iter().filter_map(|c| c.label_tick_component()).collect();
    let arrows = of("AECC_LABEL_VECTOR_ARROW_STYLE").iter().filter_map(|c| c.label_arrow_component()).collect();
    let iterators = comps.iter().filter_map(|h| object(doc, Handle::new(*h))?.label_iterator_component()).collect();
    Some(LabelStyle { settings: o.label_style_settings()?, components, lines, blocks, ticks, arrows, iterators })
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
/// copied as they are. A text component keeps its id; one `edit` drops is
/// removed, one it adds with id 0 (see [`LabelStyle::new_text_component`])
/// is a new named component after the others, as the label style editor
/// adds them (LC01 to LC03). Each setting `None` takes the label style
/// defaults' value.
pub fn add_label_style(doc: &mut CadDocument, source: Handle, name: &str, edit: impl FnOnce(&mut LabelStyle)) -> Result<Handle, DxfError> {
    if name.trim().is_empty() {
        return Err(invalid("a label style needs a name"));
    }
    let src = object(doc, source).ok_or_else(|| invalid("source label style not found"))?;
    let (mut fields, own, _) = src.label_fields(true).ok_or_else(|| invalid("not a label style this crate reads"))?;
    let collection = src.owner;
    let mut style = label_style(doc, source).ok_or_else(|| invalid("label style components cannot be read"))?;
    let source_ids: Vec<u32> = style.components.iter().map(|c| c.id).collect();
    edit(&mut style);
    let mut seen = Vec::new();
    for c in &style.components {
        if c.id != 0 && (!source_ids.contains(&c.id) || seen.contains(&c.id)) {
            return Err(invalid("a label style component id is not one of the source's, or repeats"));
        }
        if c.id == 0 && c.name.as_deref().is_none_or(|n| n.trim().is_empty()) {
            return Err(invalid("a new label style component needs a name"));
        }
        seen.push(c.id);
    }
    let removed: Vec<u32> = source_ids.iter().copied().filter(|id| !seen.contains(id)).collect();
    let source_style = label_style(doc, source).ok_or_else(|| invalid("label style components cannot be read"))?;
    // The ids of the line, block, tick and arrow components: the source's
    // and those `edit` leaves.
    let others = |s: &LabelStyle| -> [Vec<u32>; 5] {
        [s.lines.iter().map(|l| l.id).collect(), s.blocks.iter().map(|b| b.id).collect(), s.ticks.iter().map(|t| t.id).collect(), s.arrows.iter().map(|a| a.id).collect(), s.iterators.iter().map(|i| i.text.id).collect()]
    };
    let (source_others, edited) = (others(&source_style), others(&style));
    let kept = |ids: &[u32], of: &[u32]| ids.iter().filter(|i| **i != 0).all(|i| of.contains(i)) && (1..ids.len()).all(|n| ids[n] == 0 || !ids[..n].contains(&ids[n]));
    if !edited.iter().zip(&source_others).all(|(ids, of)| kept(ids, of)) {
        return Err(invalid("a label style line, block, tick, arrow or iterator component id is not one of the source's, or repeats"));
    }
    if style.iterators.iter().any(|i| i.text.id == 0) {
        return Err(invalid("text iterators cannot be added"));
    }
    let unnamed = |n: &Option<String>| n.as_deref().is_none_or(|n| n.trim().is_empty());
    if style.lines.iter().any(|l| l.id == 0 && unnamed(&l.name)) || style.ticks.iter().any(|t| t.id == 0 && unnamed(&t.name)) || style.arrows.iter().any(|a| a.id == 0 && unnamed(&a.name)) {
        return Err(invalid("a new line, tick or arrow component needs a name"));
    }
    let mut removed = removed;
    for (ids, of) in edited.iter().zip(&source_others) {
        removed.extend(of.iter().copied().filter(|id| !ids.contains(id)));
    }
    if style.components.iter().any(|c| removed.contains(&c.anchor_component)) {
        return Err(invalid("a label style component is anchored to a removed one"));
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
    // The written components: the source's (but the removed ones), then
    // the added text, line and block ones; each with the record it copies
    // (a text component of the style for an added line or block), its
    // values and, for an added one, its new id.
    let mut by_id: Vec<LabelTextComponent> = style.components.iter().filter(|c| c.id != 0).cloned().collect();
    let mut plan: Vec<(Handle, Option<Edit>, Option<u32>)> = Vec::new();

    let mut source_text = source_ids.iter();
    let mut skeleton = None;
    for c in &comps {
        if is_text(doc, *c) {
            let id = *source_text.next().ok_or_else(|| invalid("label style components changed"))?;
            skeleton.get_or_insert(Handle::new(*c));
            if let Some(i) = by_id.iter().position(|e| e.id == id) {
                plan.push((Handle::new(*c), Some(Edit::Text(by_id.remove(i))), None));
            }
        } else {
            let dxf = object(doc, Handle::new(*c)).map(|o| o.dxf_name.clone()).unwrap_or_default();
            let id = head_id(doc, Handle::new(*c));
            let edit = match dxf.as_str() {
                "AECC_LABEL_RADIAL_LINE_STYLE" => style.lines.iter().find(|l| Some(l.id) == id).cloned().map(Edit::Line),
                "AECC_LABEL_BLOCK_STYLE" => style.blocks.iter().find(|b| Some(b.id) == id).cloned().map(Edit::Block),
                "AECC_LABEL_TICK_STYLE" => style.ticks.iter().find(|t| Some(t.id) == id).cloned().map(Edit::Tick),
                "AECC_LABEL_VECTOR_ARROW_STYLE" => style.arrows.iter().find(|a| Some(a.id) == id).cloned().map(Edit::Arrow),
                d if d.starts_with("AECC_LABEL_TEXT_ITERATOR") => style.iterators.iter().find(|i| Some(i.text.id) == id).cloned().map(Edit::Iterator),
                _ => None,
            };
            let kind = if dxf.starts_with("AECC_LABEL_TEXT_ITERATOR") { "AECC_LABEL_TEXT_ITERATOR" } else { dxf.as_str() };
            let slot = ["AECC_LABEL_RADIAL_LINE_STYLE", "AECC_LABEL_BLOCK_STYLE", "AECC_LABEL_TICK_STYLE", "AECC_LABEL_VECTOR_ARROW_STYLE", "AECC_LABEL_TEXT_ITERATOR"].iter().position(|d| *d == kind);
            let gone = edit.is_none() && slot.is_some_and(|k| id.is_some_and(|id| source_others[k].contains(&id)));
            if !gone {
                plan.push((Handle::new(*c), edit, None));
            }
        }
    }
    let mut next_id = comps.iter().filter_map(|c| head_id(doc, Handle::new(*c))).filter(|k| *k & 0xFFFF_0000 == FIRST_COMPONENT_ID & 0xFFFF_0000).max().map_or(FIRST_COMPONENT_ID, |k| k + 1);
    for c in style.components.iter().filter(|c| c.id == 0) {
        let source = skeleton.ok_or_else(|| invalid("a label style without text components takes no new one"))?;
        plan.push((source, Some(Edit::Text(c.clone())), Some(next_id)));
        next_id += 1;
    }
    let named = style.lines.iter().filter(|l| l.id == 0).cloned().map(Edit::Line).chain(style.ticks.iter().filter(|t| t.id == 0).cloned().map(Edit::Tick)).chain(style.arrows.iter().filter(|a| a.id == 0).cloned().map(Edit::Arrow));
    for e in named {
        let source = skeleton.ok_or_else(|| invalid("a label style without text components takes no new line, tick or arrow"))?;
        plan.push((source, Some(e), Some(next_id)));
        next_id += 1;
    }
    let mut next_block = comps.iter().filter(|c| object(doc, Handle::new(**c)).is_some_and(|o| o.dxf_name == "AECC_LABEL_BLOCK_STYLE")).filter_map(|c| head_id(doc, Handle::new(*c))).filter(|k| !text_id(*k)).max().map_or(FIRST_BLOCK_ID, |k| k.max(FIRST_BLOCK_ID - 1) + 1);
    for b in style.blocks.iter().filter(|b| b.id == 0) {
        let source = skeleton.ok_or_else(|| invalid("a label style without text components takes no new block"))?;
        plan.push((source, Some(Edit::Block(b.clone())), Some(next_block)));
        next_block += 1;
    }

    let handle = doc.allocate_handle();
    let new_comps: Vec<Handle> = plan.iter().map(|_| doc.allocate_handle()).collect();
    fields.set_entries(new_comps.len());
    let base = crate::classes::civil::label_style::BASE_REFS;
    let mut values: Vec<u64> = typed[own..own + base].iter().map(|r| r.0).collect();
    fields.set_settings(&style.settings, &mut values);
    let mut refs: Vec<Ref> = typed[..own].iter().map(|r| Ref(kind(r.1), r.0)).collect();
    refs.extend(typed[own..own + base].iter().zip(&values).map(|(r, v)| Ref(kind(r.1), *v)));
    // Three lists of the components and a closing null reference.
    for k in [DwgReferenceType::HardPointer, DwgReferenceType::HardOwnership, DwgReferenceType::SoftPointer] {
        refs.extend(new_comps.iter().map(|h| Ref(k, h.value())));
    }
    refs.push(Ref(DwgReferenceType::HardPointer, 0));
    let own_texts = if text_id(src_id) { 2 } else { 1 };
    let mut texts = vec![name.to_string(), String::new()];
    texts.extend(strings.get(own_texts..).unwrap_or_default().iter().cloned());
    put_object(doc, "AECC_LABEL_COLLECTOR_STYLE", handle, collection, reactors, |w| {
        w.write_bit_long(id as i32);
        w.write_bit_long(0);
        bits(w, &rest);
        fields.write(w);
    }, Body { refs, strings: texts })?;
    for ((source, comp, id), new) in plan.iter().zip(&new_comps) {
        copy_component(doc, *source, *new, handle, comp.as_ref(), *id)?;
    }
    add_node_child(doc, collection, id as i32, handle, name)?;
    Ok(handle)
}

/// The values a component is written with.
enum Edit {
    Text(LabelTextComponent),
    Line(LabelLineComponent),
    Block(LabelBlockComponent),
    Tick(LabelTickComponent),
    Arrow(LabelArrowComponent),
    Iterator(LabelIteratorComponent),
}

/// The id of a label style component record.
fn head_id(doc: &CadDocument, h: Handle) -> Option<u32> {
    let (data, start, end) = fields_of(object(doc, h)?)?;
    Some(crate::classes::civil::bits::Bits::new(data, start, end).bit_long()? as u32)
}

/// Writes a copy of the component `source` as `handle`, owned by
/// `style`: a text component with the values of `c`, a component of
/// another kind as it is; with `new_id` a new text, line or block
/// component under that id (`source` a text component of the style).
fn copy_component(doc: &mut CadDocument, source: Handle, handle: Handle, style: Handle, edit: Option<&Edit>, new_id: Option<u32>) -> Result<(), DxfError> {
    if let (Some(id), Some(Edit::Line(_) | Edit::Block(_) | Edit::Tick(_) | Edit::Arrow(_))) = (new_id, edit) {
        return new_component(doc, source, handle, style, edit, id);
    }
    let c = match edit {
        Some(Edit::Text(c)) => Some(c),
        Some(Edit::Iterator(i)) => Some(&i.text),
        _ => None,
    };
    let src = object(doc, source).ok_or_else(|| invalid("label style component not found"))?;
    let dxf = src.dxf_name.clone();
    let (data, start, end) = src.main_stream().ok_or_else(|| invalid("label style component cannot be read"))?;
    let (class, own, text_keys) = crate::classes::civil::style::class_layout(data, start, end).ok_or_else(|| invalid("label style component cannot be read"))?;
    let (source_id, rest) = head(doc, source, class).ok_or_else(|| invalid("label style component cannot be read"))?;
    let id = new_id.unwrap_or(source_id);
    let class_bits: String = (class..end).map(|i| if data[i / 8] >> (7 - i % 8) & 1 == 1 { '1' } else { '0' }).collect();
    let typed = src.typed_handles();
    let strings = src.decode().strings;
    let mut values: Vec<u64> = typed[own..].iter().map(|r| r.0).collect();
    let fields = match c {
        Some(c) => {
            let mut fields = src.label_fields(false).ok_or_else(|| invalid("not a label text component this crate reads"))?.0;
            if new_id.is_some() {
                fields.set_new_text_component(c);
            }
            fields.set_text_component(c, &mut values);
            if let Some(Edit::Iterator(LabelIteratorComponent { spiral_contents: Some(t), .. })) = edit {
                fields.set_spiral_contents(t);
            }
            Some(fields)
        }
        None => match edit {
            Some(Edit::Line(l)) => {
                let mut fields = src.label_component_fields(true).ok_or_else(|| invalid("not a label line component this crate reads"))?.0;
                fields.set_line_component(l, &mut values);
                Some(fields)
            }
            Some(Edit::Block(b)) => {
                let mut fields = src.label_component_fields(false).ok_or_else(|| invalid("not a label block component this crate reads"))?.0;
                fields.set_block_component(b, &mut values);
                Some(fields)
            }
            Some(Edit::Tick(t)) => {
                let mut fields = src.label_mark_fields(true).ok_or_else(|| invalid("not a label tick component this crate reads"))?.0;
                fields.set_tick_component(t, &mut values);
                Some(fields)
            }
            Some(Edit::Arrow(a)) => {
                let mut fields = src.label_mark_fields(false).ok_or_else(|| invalid("not a label arrow component this crate reads"))?.0;
                fields.set_arrow_component(a);
                Some(fields)
            }
            _ => None,
        },
    };
    if let Some(slot) = values.get_mut(crate::classes::civil::label_style::REF_OWNER) {
        *slot = style.value();
    }
    let mut refs: Vec<Ref> = typed[..own].iter().map(|r| Ref(kind(r.1), r.0)).collect();
    refs.extend(typed[own..].iter().zip(&values).map(|(r, v)| Ref(kind(r.1), *v)));
    let texts = match c {
        Some(c) => {
            let own_texts = if text_id(source_id) { 2 } else { 1 };
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

/// Writes a new line, block, tick or arrow component (`edit`) as
/// `handle`, owned by `style`, under `id`, as the label style editor adds
/// them (LC04 to LC06, TA000): the head of the style's text component
/// `skeleton`, the class data of a new component with the values of
/// `edit`, the references of the skeleton's base (none but its border
/// linetype: no owner, layer or block), a name under its text id and
/// empty users.
fn new_component(doc: &mut CadDocument, skeleton: Handle, handle: Handle, style: Handle, edit: Option<&Edit>, id: u32) -> Result<(), DxfError> {
    let bad = || invalid("label style component cannot be read");
    let src = object(doc, skeleton).ok_or_else(bad)?;
    let (data, start, end) = src.main_stream().ok_or_else(bad)?;
    let class = crate::classes::civil::style::class_layout(data, start, end).ok_or_else(bad)?.0;
    let rest = head(doc, skeleton, class).ok_or_else(bad)?.1;
    let border = src.typed_handles().get(crate::classes::civil::label_style::BASE_REFS - 3).map_or(0, |r| r.0);
    let line = matches!(edit, Some(Edit::Line(_)));
    let fields = match edit {
        Some(Edit::Tick(_)) => crate::classes::civil::label_style::LabelFields::new_mark(true),
        Some(Edit::Arrow(_)) => crate::classes::civil::label_style::LabelFields::new_mark(false),
        _ => crate::classes::civil::label_style::LabelFields::new_component(line),
    };
    let mut fields = fields.ok_or_else(bad)?;
    let mut values = vec![0; if line { 9 } else { 10 }];
    values[crate::classes::civil::label_style::BASE_REFS - 3] = border;
    let (dxf, texts) = match edit {
        Some(Edit::Line(l)) => {
            fields.set_line_component(l, &mut values);
            ("AECC_LABEL_RADIAL_LINE_STYLE", vec![l.name.clone().unwrap_or_default(), String::new(), String::new(), String::new(), String::new()])
        }
        Some(Edit::Block(b)) => {
            fields.set_block_component(b, &mut values);
            ("AECC_LABEL_BLOCK_STYLE", vec![String::new(); 3])
        }
        Some(Edit::Tick(t)) => {
            fields.set_tick_component(t, &mut values);
            ("AECC_LABEL_TICK_STYLE", vec![t.name.clone().unwrap_or_default(), String::new(), String::new(), String::new()])
        }
        Some(Edit::Arrow(a)) => {
            fields.set_arrow_component(a);
            ("AECC_LABEL_VECTOR_ARROW_STYLE", vec![a.name.clone().unwrap_or_default(), String::new(), String::new(), String::new()])
        }
        _ => return Err(bad()),
    };
    let mut refs = vec![Ref(DwgReferenceType::SoftPointer, values[0])];
    refs.extend(values[1..].iter().map(|v| Ref(DwgReferenceType::HardPointer, *v)));
    put_object(doc, dxf, handle, style, Vec::new(), |w| {
        w.write_bit_long(id as i32);
        w.write_bit_long(0);
        bits(w, &rest);
        fields.write(w);
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

    /// A copy without the last text component and with two new ones lists
    /// the kept component and the new ones (ids from 0x80050001) in order.
    #[test]
    fn components_are_removed_and_added() {
        let mut doc = CadDocument::new();
        crate::classes::civil::add_cogo_point(&mut doc, None, Vector3::new(1.0, 2.0, 3.0), None, None, None).unwrap();
        let h = add_point_label_style(&mut doc, "LS2", |s| {
            let last = s.components.pop().unwrap();
            let mut a = s.new_text_component("A1").unwrap();
            a.contents = "Extra".into();
            let b = s.new_text_component("A2").unwrap();
            s.components.extend([a, b]);
            assert_ne!(last.id, 0);
        })
        .unwrap();
        let back = label_style(&doc, h).unwrap();
        let ids: Vec<u32> = back.components.iter().map(|c| c.id).collect();
        assert_eq!(ids.len(), 4);
        assert_eq!(&ids[2..], &[0x8005_0001, 0x8005_0002]);
        assert_eq!((back.components[2].name.as_deref(), back.components[2].contents.as_str(), back.components[2].anchor_point), (Some("A1"), "Extra", -4));
        let fields = object(&doc, h).unwrap().label_fields(true).unwrap().0;
        assert_eq!(fields.entry_count(), 4);
        assert!(add_point_label_style(&mut doc, "LS3", |s| s.components.push(s.new_text_component("").unwrap())).is_err());
    }

    /// Line and block components are added after the others (lines under
    /// text ids with their names, blocks numbered from 0xA71) and can be
    /// removed again in a copy.
    #[test]
    fn line_and_block_components_are_added_and_removed() {
        let mut doc = CadDocument::new();
        crate::classes::civil::add_cogo_point(&mut doc, None, Vector3::new(1.0, 2.0, 3.0), None, None, None).unwrap();
        let h = add_point_label_style(&mut doc, "LB1", |s| {
            let mut l = s.new_line_component("L1").unwrap();
            l.angle = 0.5;
            let mut b = s.new_block_component().unwrap();
            b.height = 0.05;
            s.lines.push(l);
            s.blocks.extend([b.clone(), b]);
        })
        .unwrap();
        let back = label_style(&doc, h).unwrap();
        assert_eq!((back.lines.len(), back.lines[0].id, back.lines[0].name.as_deref(), back.lines[0].angle), (1, 0x8005_0001, Some("L1"), 0.5));
        assert_eq!(back.blocks.iter().map(|b| (b.id, b.height)).collect::<Vec<_>>(), vec![(0xA71, 0.05), (0xA72, 0.05)]);
        assert!(add_point_label_style(&mut doc, "LB2", |s| s.lines.push(s.new_line_component(" ").unwrap())).is_err());
        let h2 = add_label_style(&mut doc, h, "LB3", |s| {
            s.lines.clear();
            s.blocks.remove(0);
        })
        .unwrap();
        let back = label_style(&doc, h2).unwrap();
        assert_eq!((back.lines.len(), back.blocks.iter().map(|b| b.id).collect::<Vec<_>>()), (0, vec![0xA72]));
        assert_eq!(object(&doc, h2).unwrap().label_fields(true).unwrap().0.entry_count(), 4);
    }

    /// Tick and direction arrow components are added under text ids with
    /// their names, read back with the written values and removed again
    /// in a copy.
    #[test]
    fn tick_and_arrow_components_are_added_and_removed() {
        let mut doc = CadDocument::new();
        crate::classes::civil::add_cogo_point(&mut doc, None, Vector3::new(1.0, 2.0, 3.0), None, None, None).unwrap();
        let h = add_point_label_style(&mut doc, "TA1", |s| {
            let mut t = s.new_tick_component("T1").unwrap();
            (t.rotation, t.height, t.align_with_object, t.block) = (0.3, 0.03, false, 0x15);
            let mut a = s.new_arrow_component("A1").unwrap();
            (a.length, a.fixed_length, a.rotation, a.x_offset) = (0.06, true, 0.2, 0.01);
            s.ticks.push(t);
            s.arrows.push(a);
        })
        .unwrap();
        let back = label_style(&doc, h).unwrap();
        let t = &back.ticks[0];
        assert_eq!((t.id, t.name.as_deref(), t.rotation, t.height, t.align_with_object, t.block), (0x8005_0001, Some("T1"), 0.3, 0.03, false, 0x15));
        let a = &back.arrows[0];
        assert_eq!((a.id, a.name.as_deref(), a.length, a.fixed_length, a.rotation, a.x_offset, a.arrowhead_size), (0x8005_0002, Some("A1"), 0.06, true, 0.2, 0.01, 1.0 / 120.0));
        assert!(add_point_label_style(&mut doc, "TA2", |s| s.arrows.push(s.new_arrow_component("").unwrap())).is_err());
        let h2 = add_label_style(&mut doc, h, "TA3", |s| s.ticks.clear()).unwrap();
        let back = label_style(&doc, h2).unwrap();
        assert_eq!((back.ticks.len(), back.arrows.len()), (0, 1));
    }

    /// A text iterator's text settings and kind read; a copy writes its
    /// edited values, leaves it out when removed and refuses a new one.
    #[test]
    fn text_iterators_are_edited_and_removed() {
        let mut doc = CadDocument::new();
        crate::classes::civil::add_cogo_point(&mut doc, None, Vector3::new(1.0, 2.0, 3.0), None, None, None).unwrap();
        let basic = Handle::new(super::super::default_styles::ensure_intersection_label_style(&mut doc).unwrap());
        let source = label_style(&doc, basic).unwrap();
        assert_eq!((source.iterators.len(), source.iterators[0].iterates, source.iterators[0].spiral_contents.is_none()), (1, 0x1907_A000, true));
        let h = add_label_style(&mut doc, basic, "IT1", |s| {
            s.iterators[0].text.contents = "Road".into();
            s.iterators[0].text.height = Some(0.02);
        })
        .unwrap();
        let back = label_style(&doc, h).unwrap();
        assert_eq!((back.iterators[0].text.id, back.iterators[0].text.contents.as_str(), back.iterators[0].text.height), (source.iterators[0].text.id, "Road", Some(0.02)));
        let h2 = add_label_style(&mut doc, h, "IT2", |s| s.iterators.clear()).unwrap();
        assert!(label_style(&doc, h2).unwrap().iterators.is_empty());
        assert!(add_label_style(&mut doc, h, "IT3", |s| {
            let mut i = s.iterators[0].clone();
            i.text.id = 0;
            s.iterators.push(i);
        })
        .is_err());
    }
}
