//! Records of the style tree written from templates: the application's
//! default styles, their children and dates as fields and everything else
//! as constant runs.

use super::support::*;
use super::tree::file_time;
use crate::error::DxfError;
use crate::io::dwg::dwg_reference_type::DwgReferenceType;
use crate::io::dwg::dwg_stream_writers::DwgMergedWriter;
use crate::types::Handle;
use crate::CadDocument;

/// What a reference of a template points to.
#[derive(Clone, Copy)]
pub(super) enum Target {
    Null,
    /// Another template of the same table, by index.
    Record(usize),
    Layer0,
    ByBlock,
    /// The text style "Standard".
    Standard,
    /// The material "ByLayer", or null in a drawing without it.
    ByLayerMaterial,
    /// The style `id` of the style tree's collection `key` (written
    /// before this table).
    Style(i32, i32),
    /// The label style `id` of the label node's collection `key`.
    Label(u32, u32),
}

/// A piece of a template's fields after the version word.
pub(super) enum Piece {
    /// A constant run: its length in bits and the bits as hex digits
    /// (the last digit padded with zero bits).
    Run(usize, &'static str),
    /// A date key's value: the time the record is written.
    Time,
    /// The record's children: `BL n`, n x `BL key`.
    Children,
}

/// A record in the style layout of the style tree (id, children, keys,
/// component lists, class data).
pub(super) struct Template {
    pub dxf: &'static str,
    /// The record that owns it; `Null` for the style tree.
    pub owner: Target,
    /// `(key, template)` children.
    pub children: &'static [(u32, usize)],
    pub fields: &'static [Piece],
    /// The references after the children's soft owner references.
    pub refs: &'static [(DwgReferenceType, Target)],
    pub strings: &'static [&'static str],
}

pub(super) fn run(w: &mut DwgMergedWriter, len: usize, hex: &str) {
    let mut left = len;
    for c in hex.chars() {
        let v = c.to_digit(16).unwrap_or(0);
        for i in (0..4).rev() {
            if left == 0 {
                return;
            }
            w.write_bit(v >> i & 1 == 1);
            left -= 1;
        }
    }
}

/// Writes `templates` under the style tree `styles` and returns their
/// handles; the caller lists the top-level ones in the style tree.
///
/// Every table's first template is its family's collection root, and it
/// carries the same DXF class as the styles it owns. A class-instance
/// census of an `AECC_*_STYLE` family therefore counts one record more
/// than the family has styles, while Civil 3D's own style collections
/// enumerate only a root's children and so report the style count
/// without the root. `DE_Busbucht_2023.dwg` (Civil 3D 2027) shows both
/// sides of that: its alignment family holds 18 `AeccDbAlignmentStyle`
/// records — the collection root `0x1D3` owned by the `AeccDbTreeNode`
/// `0xF3`, plus the 17 named styles the root owns — while the .NET
/// `Styles.AlignmentStyles` collection reports 17.
pub(super) fn write_templates(doc: &mut CadDocument, styles: Handle, templates: &[Template]) -> Result<Vec<Handle>, DxfError> {
    let table = |doc: &CadDocument, t: Target| -> Option<u64> {
        Some(match t {
            Target::Layer0 => doc.layers.get("0")?.handle.value(),
            Target::ByBlock => doc.line_types.get("ByBlock")?.handle.value(),
            Target::Standard => doc.text_styles.get("Standard")?.handle.value(),
            Target::ByLayerMaterial => doc.objects.iter().find_map(|(h, o)| matches!(o, crate::objects::ObjectType::Material(m) if m.name == "ByLayer").then_some(h.value())).unwrap_or(0),
            Target::Style(key, id) => {
                let tree = read_node(doc, root_node(doc, super::tree::KEY_STYLES)?)?;
                let collection = tree.children.iter().find(|c| c.0 == key)?.1;
                read_node(doc, Handle::new(collection))?.children.iter().find(|c| c.0 == id)?.1
            }
            Target::Label(key, id) => {
                let tree = read_node(doc, root_node(doc, super::tree::KEY_STYLES)?)?;
                let labels = tree.children.iter().find(|c| c.0 == super::labels::KEY_LABELS)?.1;
                let collection = read_node(doc, Handle::new(labels))?.children.iter().find(|c| c.0 as u32 == key)?.1;
                read_node(doc, Handle::new(collection))?.children.iter().find(|c| c.0 as u32 == id)?.1
            }
            _ => 0,
        })
    };
    for t in [Target::Layer0, Target::ByBlock, Target::Standard] {
        table(doc, t).ok_or_else(|| invalid("drawing lacks layer 0, the ByBlock linetype or the Standard text style"))?;
    }
    for t in templates.iter().flat_map(|t| t.refs.iter().map(|r| r.1)) {
        if matches!(t, Target::Style(..) | Target::Label(..)) && table(doc, t).is_none() {
            return Err(invalid("a style these styles refer to is missing"));
        }
    }
    let handles: Vec<Handle> = templates.iter().map(|_| doc.allocate_handle()).collect();
    let now = file_time().to_le_bytes();
    for (i, t) in templates.iter().enumerate() {
        let target = |doc: &CadDocument, x: Target| match x {
            Target::Record(j) => handles[j].value(),
            other => table(doc, other).unwrap_or(0),
        };
        let owner = match t.owner {
            Target::Record(j) => handles[j],
            _ => styles,
        };
        let mut refs: Vec<Ref> = t.children.iter().map(|c| soft_owner(handles[c.1].value())).collect();
        refs.extend(t.refs.iter().map(|(kind, x)| Ref(*kind, target(doc, *x))));
        let strings = t.strings.iter().map(|s| s.to_string()).collect();
        put_object(doc, t.dxf, handles[i], owner, Vec::new(), |w| {
            for p in t.fields {
                match p {
                    Piece::Run(len, hex) => run(w, *len, hex),
                    Piece::Time => w.write_bytes(&now),
                    Piece::Children => {
                        w.write_bit_long(t.children.len() as i32);
                        for c in t.children {
                            w.write_bit_long(c.0 as i32);
                        }
                    }
                }
            }
        }, Body { refs, strings })?;
    }
    Ok(handles)
}
