//! Codec-only roundtrip test for the tree bootstrap and the point/label
//! style defaults (§1, §2, §2a).
//!
//! A drawing that gets a point without a settings and style tree is
//! bootstrapped by the writers: the root tree node with the point group
//! node (root key 32), the point database (34) and the outer query of
//! `_All Points` (75), plus the point style collection, the class node
//! and the root settings node (§2a). The structural facts are asserted
//! after a save and reopen: the root's children in stored order (the
//! entries are stored by hash bucket, key mod 8, so keys 32, 34, 75 come
//! first, §2), each child owned by the root, `_All Points` with the
//! point, and the point style collection listing the unnamed default
//! style (id 30, §2a).

use acadrust::classes::arch::ArchObject;
use acadrust::classes::civil::{add_cogo_point, cogo_points, ensure_civil};
use acadrust::objects::ObjectType;
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter};

fn roundtrip(doc: &CadDocument, tag: &str) -> CadDocument {
    let path = std::env::temp_dir().join(format!("rt_infra_{tag}_{}.dwg", std::process::id()));
    DwgWriter::write_to_file(&path, doc).expect("DWG write failed");
    let back = DwgReader::from_file(&path)
        .expect("DWG reopen failed")
        .read()
        .expect("DWG read failed");
    let _ = std::fs::remove_file(&path);
    back
}

fn object<'a>(doc: &'a CadDocument, handle: acadrust::types::Handle) -> &'a ArchObject {
    match doc.objects.get(&handle) {
        Some(ObjectType::Arch(a)) => a,
        _ => panic!("no Arch object at {:#x}", handle.value()),
    }
}

/// The class-body reference values of an object, in stream order.
fn object_refs(o: &ArchObject) -> Vec<u64> {
    o.typed_handles().iter().map(|r| r.0).collect()
}

#[test]
fn tree_bootstrap_and_point_style_defaults_survive() {
    let mut doc = CadDocument::new();
    doc.version = DxfVersion::AC1032;
    let root = ensure_civil(&mut doc).unwrap();
    add_cogo_point(&mut doc, Some(1), Vector3::new(100.0, 200.0, 10.0), Some("RT"), None, None).unwrap();

    let back = roundtrip(&doc, "tree");

    // The named-objects entry "Root" is an AECC_TREE_NODE object.
    let root_handle = back
        .named_objects_dictionary()
        .unwrap()
        .get("Root")
        .expect("the root tree node");
    let root_node = object(&back, root_handle);
    assert_eq!(root_node.dxf_name, "AECC_TREE_NODE");
    assert_eq!(root_handle.value(), root.value(), "the root keeps its handle");

    // The root's children, in stored order: keys 32 (point group node),
    // 34 (point database) and 75 (the `_All Points` query) first — their
    // hash buckets 0, 2 and 3 come before the bootstrapped style tree
    // (31), class (5) and settings (45) nodes (§2, §2a).
    let children: Vec<u64> = root_node.decode().handles;
    assert!(children.len() >= 3, "the bootstrapped root has children");
    let kinds: Vec<&str> = children
        .iter()
        .map(|h| object(&back, acadrust::types::Handle::new(*h)).dxf_name.as_str())
        .collect();
    assert_eq!(&kinds[..3], &["AECC_POINT_GROUP_NODE", "AECC_POINT_NODE", "AECC_QUERY_STATEMENT"]);
    assert!(kinds.contains(&"AECC_TREE_NODE"), "the style tree");
    assert!(kinds.contains(&"AECC_CLASS_NODE"), "the class node of COGO points");
    assert!(kinds.contains(&"AECC_ROOT_SETTINGS_NODE"), "the root settings node");
    // Every child is owned by the root (soft owner, the first reference).
    for h in &children {
        let child = object(&back, acadrust::types::Handle::new(*h));
        assert_eq!(child.owner, root_handle, "{:#x} owned by the root", h);
    }

    // `_All Points`: the point group node owns the groups, `_All Points`
    // first in stored order with no stored name (§3), and it lists the
    // written point.
    let group_node = object(&back, acadrust::types::Handle::new(children[0]));
    let groups: Vec<&ArchObject> = group_node
        .decode()
        .handles
        .iter()
        .map(|h| object(&back, acadrust::types::Handle::new(*h)))
        .collect();
    let all = groups
        .iter()
        .find(|g| g.dxf_name == "AECC_POINT_PG" && g.decode().strings.first().map(String::as_str) == Some("") && g.point_group_numbers() == vec![1])
        .expect("the _All Points group holds the point");
    assert_eq!(all.point_group_numbers(), vec![1], "the group's member list");
    // The point database (key 34) holds the written point's record: its
    // own `cogo_points()` lists number, position (bit-exact) and
    // description.
    let db = object(&back, acadrust::types::Handle::new(children[1]));
    assert_eq!(db.dxf_name, "AECC_POINT_NODE");
    let listed = db.cogo_points();
    assert_eq!(listed.len(), 1, "the database lists the point");
    assert_eq!((listed[0].number, listed[0].position, listed[0].description.as_deref()), (1, Vector3::new(100.0, 200.0, 10.0), Some("RT")));

    // The point survives with its number, position (bit-exact),
    // description and name.
    let points = cogo_points(&back);
    assert_eq!(points.len(), 1);
    assert_eq!((points[0].number, points[0].position), (1, Vector3::new(100.0, 200.0, 10.0)));
    assert_eq!(points[0].description.as_deref(), Some("RT"));

    // The point style collection (style tree key 35) lists the unnamed
    // default style (id 30, §2a): two AECC_POINT_STYLE records, one
    // owning the other, the owned one without a name.
    let styles: Vec<&ArchObject> = back
        .objects
        .values()
        .filter_map(|o| match o {
            ObjectType::Arch(a) if a.dxf_name == "AECC_POINT_STYLE" => Some(a),
            _ => None,
        })
        .collect();
    assert_eq!(styles.len(), 2, "collection and default");
    let (collection, default) = if object_refs(styles[0]).contains(&styles[1].handle.value()) {
        (styles[0], styles[1])
    } else {
        assert!(object_refs(styles[1]).contains(&styles[0].handle.value()), "one lists the other");
        (styles[1], styles[0])
    };
    assert_eq!(default.decode().strings.first().map(String::as_str), Some(""), "the default style has no name");
    let default_ref = default.handle.value();
    assert!(collection.decode().handles.contains(&default_ref), "the collection lists the default");
    // The point draws with the written default styles (the audit pilot:
    // a written point has nonzero point and label style — the defaults
    // the bootstrapped tree provides).
    assert_eq!(points[0].style, default_ref, "the point takes the default point style");
    assert!(points[0].label_style != 0, "the point has a label style");
}
