//! Civil actions in the drawing's associative network: the network under
//! the named objects dictionary, the model space's network under its
//! extension dictionary, and the action, its body and its dependencies
//! (`AECC_ASSOC_*`).

use super::support::*;
use crate::error::DxfError;
use crate::objects::{AssocAction, AssocActionDependency, AssocNetwork, AssociativeData, AssociativeObject, Dictionary, ObjectType};
use crate::types::Handle;
use crate::CadDocument;

/// The key of both levels of the network in their dictionaries.
const NETWORK: &str = "ACAD_ASSOCNETWORK";

/// A dependency of an action: the object it depends on and whether the
/// action writes it (else reads it).
pub(super) struct Dependency {
    pub object: Handle,
    pub writes: bool,
}

fn network(doc: &CadDocument, h: Handle) -> Option<&AssocNetwork> {
    match doc.objects.get(&h)? {
        ObjectType::Associative(AssociativeObject { data: AssociativeData::Network(n), .. }) => Some(n),
        _ => None,
    }
}

fn dictionary(doc: &CadDocument, h: Handle) -> Option<&Dictionary> {
    match doc.objects.get(&h)? {
        ObjectType::Dictionary(d) => Some(d),
        _ => None,
    }
}

/// The network a dictionary (`owner`'s `ACAD_ASSOCNETWORK` dictionary)
/// holds, made when missing: `holder` holds a dictionary that holds the
/// network.
fn ensure_network(doc: &mut CadDocument, holder: Handle, owning: Handle) -> Result<Handle, DxfError> {
    let inner = dictionary(doc, holder).and_then(|d| d.get(NETWORK));
    if let Some(n) = inner.and_then(|d| dictionary(doc, d)).and_then(|d| d.get(NETWORK)) {
        network(doc, n).ok_or_else(|| invalid("the associative network is not one this crate reads"))?;
        return Ok(n);
    }
    let (dict, net) = (doc.allocate_handle(), doc.allocate_handle());
    let mut d = Dictionary::new();
    d.handle = dict;
    d.owner = holder;
    d.add_entry(NETWORK, net);
    doc.objects.insert(dict, ObjectType::Dictionary(d));
    let mut o = AssociativeObject::new("ACDBASSOCNETWORK", "AcDbAssocNetwork");
    o.handle = net;
    o.owner = dict;
    o.reactors = vec![dict];
    o.data = AssociativeData::Network(AssocNetwork {
        action: AssocAction { class_version: 2, owning_network: owning, ..AssocAction::default() },
        ..AssocNetwork::default()
    });
    doc.objects.insert(net, ObjectType::Associative(o));
    match doc.objects.get_mut(&holder) {
        Some(ObjectType::Dictionary(h)) => h.add_entry(NETWORK, dict),
        _ => return Err(invalid("dictionary not found")),
    }
    Ok(net)
}

/// Appends `action` to `net`'s actions (`owned` by it), numbering it, and
/// returns its index.
fn add_to(doc: &mut CadDocument, net: Handle, action: Handle, owned: bool) -> Result<i32, DxfError> {
    match doc.objects.get_mut(&net) {
        Some(ObjectType::Associative(AssociativeObject { data: AssociativeData::Network(n), .. })) => {
            n.actions.push(AssocActionDependency { is_owned: owned, dependency: action });
            n.network_action_index = n.actions.len() as i32;
            Ok(n.network_action_index)
        }
        _ => Err(invalid("associative network not found")),
    }
}

/// The model space's network (inside the drawing's network, which lists
/// it), made when missing with the model space's extension dictionary.
fn model_space_network(doc: &mut CadDocument) -> Result<Handle, DxfError> {
    let nod = doc.header.named_objects_dict_handle;
    let top = ensure_network(doc, nod, Handle::NULL)?;
    let ms = doc.header.model_space_block_handle;
    let xdict = match doc.xdic_by_handle.get(&ms).copied().filter(|h| dictionary(doc, *h).is_some()) {
        Some(h) => h,
        None => {
            let h = doc.allocate_handle();
            let mut d = Dictionary::new();
            d.handle = h;
            d.owner = ms;
            d.hard_owner = true;
            doc.objects.insert(h, ObjectType::Dictionary(d));
            doc.xdic_by_handle.insert(ms, h);
            h
        }
    };
    let made = dictionary(doc, xdict).and_then(|d| d.get(NETWORK)).is_none();
    let net = ensure_network(doc, xdict, top)?;
    if made {
        let index = add_to(doc, top, net, false)?;
        if let Some(ObjectType::Associative(AssociativeObject { data: AssociativeData::Network(n), .. })) = doc.objects.get_mut(&net) {
            n.action.action_index = index;
        }
    }
    Ok(net)
}

/// Adds an action of `body_dxf` (its version `BL 2`, then the version
/// word; a hard pointer to `target`) depending on `dependencies` to the
/// model space's network. Each dependency (`AECC_ASSOC_DEPENDENCY`) owns
/// a body (`AECC_ASSOC_DEPENDENCY_BODY`). Returns the dependencies, which
/// their objects list as reactors.
///
/// ```text
/// action:     BS 2, BL 0, BL index, BL n, BL n, n x B 1 (owned), BS 0,
///             BL 0, BS 0, BL 0, version word; references: network (soft),
///             body (hard ownership), dependencies (hard ownership)
/// dependency: BS 2, BL 0, B read, B write, B 1, B 1, BL 0, B 0, BL i
///             (1..n), version word; references: object (soft), 0, 0, body
///             (hard ownership)
/// bodies:     BL 2 (action) or BL 1 (dependency), version word
/// ```
///
/// (OP01, OP02, OB01: the records equal the samples' bit for bit.)
pub(super) fn add_action(doc: &mut CadDocument, body_dxf: &str, target: Handle, dependencies: &[Dependency]) -> Result<Vec<Handle>, DxfError> {
    let net = model_space_network(doc)?;
    let action = doc.allocate_handle();
    let index = add_to(doc, net, action, true)?;
    let body = doc.allocate_handle();
    let deps: Vec<(Handle, Handle)> = dependencies.iter().map(|_| (doc.allocate_handle(), doc.allocate_handle())).collect();
    let n = deps.len() as i32;
    let version = |w: &mut crate::io::dwg::dwg_stream_writers::DwgMergedWriter| w.write_bit_short(VERSION);
    put_plain_object(
        doc,
        "AECC_ASSOC_ACTION",
        action,
        net,
        Vec::new(),
        |w| {
            w.write_bit_short(2);
            for v in [0, index, n, n] {
                w.write_bit_long(v);
            }
            for _ in 0..n {
                w.write_bit(true);
            }
            for _ in 0..2 {
                w.write_bit_short(0);
                w.write_bit_long(0);
            }
            version(w);
        },
        Body { refs: [soft_pointer(net.value()), Ref(crate::io::dwg::dwg_reference_type::DwgReferenceType::HardOwnership, body.value())].into_iter().chain(deps.iter().map(|d| Ref(crate::io::dwg::dwg_reference_type::DwgReferenceType::HardOwnership, d.0.value()))).collect(), strings: Vec::new() },
    )?;
    put_plain_object(
        doc,
        body_dxf,
        body,
        action,
        Vec::new(),
        |w| {
            w.write_bit_long(2);
            version(w);
        },
        Body { refs: vec![hard_pointer(target.value())], strings: Vec::new() },
    )?;
    for (i, ((dep, dep_body), d)) in deps.iter().zip(dependencies).enumerate() {
        put_plain_object(
            doc,
            "AECC_ASSOC_DEPENDENCY",
            *dep,
            action,
            Vec::new(),
            |w| {
                w.write_bit_short(2);
                w.write_bit_long(0);
                for b in [!d.writes, d.writes, true, true] {
                    w.write_bit(b);
                }
                w.write_bit_long(0);
                w.write_bit(false);
                w.write_bit_long(i as i32 + 1);
                version(w);
            },
            Body { refs: vec![soft_pointer(d.object.value()), soft_pointer(0), soft_pointer(0), Ref(crate::io::dwg::dwg_reference_type::DwgReferenceType::HardOwnership, dep_body.value())], strings: Vec::new() },
        )?;
        put_plain_object(
            doc,
            "AECC_ASSOC_DEPENDENCY_BODY",
            *dep_body,
            *dep,
            Vec::new(),
            |w| {
                w.write_bit_long(1);
                version(w);
            },
            Body { refs: Vec::new(), strings: Vec::new() },
        )?;
    }
    Ok(deps.into_iter().map(|d| d.0).collect())
}
