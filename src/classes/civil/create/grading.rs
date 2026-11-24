//! The drawing's root grading group (`AECC_GRADING_GROUP`, the root tree
//! node's child key 259): the tree the application lists the named grading
//! groups of a drawing under.
//!
//! ```text
//! BS 0x04BE, BL 259 (the key), BL n, n x BL (the members' name tags,
//! each 0x80000000 | tag), BL 0, 139 constant bits
//! handles: n x member (soft ownership), soft pointer 0
//! strings: "" (the root's own name) + the members' names
//! ```
//!
//! The 139 bits after the terminator are the same in every tree-form
//! record of the samples — 14 records in 14 drawings, with 0, 1 and 2
//! members — and parse as no coded field; they are the class tail.
//!
//! A named grading group (`AECC_GRADING_GROUP`, the [`GradingGroup`]
//! form) stores a settings body the application saves per record: two
//! layouts (a short one and a long one with an ~83-bit constant head and
//! a per-record run that changes when the application touches the
//! group) with no field the API reads. Nothing maps it, so this module
//! writes the empty tree only — the form the samples hold for a drawing
//! without named groups.

use super::support::*;
use crate::error::DxfError;
use crate::types::Handle;
use crate::CadDocument;

/// Key of the root tree node's grading group child; also the key the
/// record itself stores.
const KEY_GRADING_GROUPS: i32 = 259;

/// The tree form's class tail: 139 bits, the same in every tree-form
/// record of the samples.
const TREE_TAIL: &str = "0101000000000000000000000000000000000000000000000000000000010010001000000001101011011101011011110111111001111101001110011101010101000111111";

/// Writes the drawing's root grading group: the tree the application's
/// grading groups hang from, as the samples store it for a drawing
/// without named groups (key 259 under the root tree node, no members,
/// one null terminator, one empty name). Refuses when the drawing has
/// one. Returns the record's handle.
///
/// Writing a named group would need its settings body (see the module
/// notes) and member gradings, which no writer of this crate produces.
pub fn add_grading_group_tree(doc: &mut CadDocument) -> Result<Handle, DxfError> {
    if root_node(doc, KEY_GRADING_GROUPS).is_some() {
        return Err(invalid("the drawing already has a grading group tree"));
    }
    let root = ensure_civil(doc)?;
    let handle = doc.allocate_handle();
    put_object(
        doc,
        "AECC_GRADING_GROUP",
        handle,
        root,
        Vec::new(),
        |w| {
            w.write_bit_long(KEY_GRADING_GROUPS);
            w.write_bit_long(0);
            w.write_bit_long(0);
            bits(w, TREE_TAIL);
        },
        Body { refs: vec![soft_pointer(0)], strings: vec![String::new()] },
    )?;
    set_root_entry(doc, KEY_GRADING_GROUPS, handle)?;
    Ok(handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classes::civil::GradingGroupRecord;
    use crate::io::dwg::{DwgReader, DwgWriter};
    use crate::types::DxfVersion;

    fn bl(value: u32) -> String {
        let mut bits = String::from("00");
        for b in value.to_le_bytes() {
            bits.push_str(&format!("{b:08b}"));
        }
        bits
    }

    /// The record's main stream as the application writes it: version
    /// word, key, member count, terminator, class tail.
    fn expected_bits() -> String {
        let mut bits = String::from("00");
        for b in 0x04BEu16.to_le_bytes() {
            bits.push_str(&format!("{b:08b}"));
        }
        bits.push_str(&bl(259));
        bits.push_str("10");
        bits.push_str("10");
        bits.push_str(TREE_TAIL);
        bits
    }

    fn tree(doc: &CadDocument) -> Option<crate::classes::arch::ArchObject> {
        doc.objects.iter().find_map(|(_, o)| match o {
            crate::objects::ObjectType::Arch(a) if a.dxf_name == "AECC_GRADING_GROUP" => Some(a.clone()),
            _ => None,
        })
    }

    #[test]
    fn writes_the_empty_root_group() {
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let made = add_grading_group_tree(&mut doc).unwrap();
        assert!(add_grading_group_tree(&mut doc).is_err());
        let record = tree(&doc).unwrap();
        assert_eq!(record.handle, made);
        assert_eq!(record.cpp_class_name, "AeccDbGradingGroup");
        // Owned by the root tree node, listed under key 259.
        let root = crate::classes::civil::create::support::root(&doc).unwrap();
        assert_eq!(record.owner, root);
        let entries = crate::classes::civil::create::support::root_entries(&doc).unwrap();
        assert!(entries.contains(&(259, made)));
        // The handle stream: one null soft pointer; the text stream: the
        // root's empty name.
        assert_eq!(record.typed_handles(), [(0u64, 4u8)]);
        assert_eq!(record.decode().strings, [String::new()]);
    }

    #[test]
    fn the_written_tree_round_trips_and_decodes() {
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let made = add_grading_group_tree(&mut doc).unwrap();
        let path = std::env::temp_dir().join(format!("civil-grading-tree-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        // The class table entry, as the application's own files carry it.
        let class = back.classes.get_by_name("AECC_GRADING_GROUP").unwrap();
        assert_eq!(class.cpp_class_name, "AeccDbGradingGroup");
        assert_eq!(class.application_name, "AeccLand130");
        assert_eq!(class.proxy_flags, crate::classes::ProxyFlags(0x81));
        assert_eq!(class.is_an_entity, false);
        // Decode equality on every typed field.
        let record = tree(&back).unwrap();
        assert_eq!(record.handle, made);
        assert_eq!(record.owner, tree(&doc).unwrap().owner);
        assert_eq!(record.grading_group(), Some(GradingGroupRecord::Tree(Default::default())));
        // The saved bits are the application's own form: version word,
        // key 259, no members, terminator, class tail — 195 bits, the
        // length of the samples' empty records.
        let (data, start, end) = record.main_stream().unwrap();
        let bits: String = (start..end).map(|i| if data[i / 8] >> (7 - i % 8) & 1 == 1 { '1' } else { '0' }).collect();
        assert_eq!(bits, expected_bits());
        assert_eq!(end - start, 195);
        // The root still lists it under key 259, after its other keys of
        // the same hash bucket (259 % 8 = 3).
        let root = crate::classes::civil::create::support::root(&back).unwrap();
        let entries = crate::classes::civil::create::support::root_entries(&back).unwrap();
        assert_eq!(entries.iter().find(|e| e.0 == 259).map(|e| e.1), Some(made));
        assert!(root != crate::types::Handle::NULL);
        let buckets: Vec<i32> = entries.iter().map(|e| e.0 % 8).collect();
        let mut sorted = buckets.clone();
        sorted.sort_unstable();
        assert_eq!(buckets, sorted);
    }
}
