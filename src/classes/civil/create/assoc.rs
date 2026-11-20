//! Civil actions in the drawing's associative network: the network under
//! the named objects dictionary, the model space's network under its
//! extension dictionary, and the action, its body and its dependencies
//! (`AECC_ASSOC_*`).

use super::support::*;
use crate::error::DxfError;
use crate::io::dwg::dwg_reference_type::DwgReferenceType::HardOwnership;
use crate::objects::{AssocAction, AssocActionDependency, AssocNetwork, AssociativeData, AssociativeObject, Dictionary, ObjectType};
use crate::types::Handle;
use crate::CadDocument;

/// The key of both levels of the network in their dictionaries.
const NETWORK: &str = "ACAD_ASSOCNETWORK";

/// A dependency of an action: the entity it depends on, whether the
/// action writes it (else reads it) and its order among the entity's
/// dependencies (lower first).
pub(super) struct Dependency {
    pub object: Handle,
    pub writes: bool,
    pub order: i32,
}

/// An action's body: its class, the objects it points to (hard pointers)
/// and the bits after its version word.
pub(super) struct ActionBody<'a> {
    pub dxf: &'a str,
    pub refs: Vec<Handle>,
    pub tail: &'a str,
}

/// A dependency record's fields.
struct DependencyRecord {
    object: Handle,
    writes: bool,
    order: i32,
    index: i32,
    previous: Handle,
    next: Handle,
    body: Handle,
}

/// Writes a dependency (`AECC_ASSOC_DEPENDENCY`, owned by `action`):
///
/// ```text
/// BS 2, BL 0, B read, B write, B 1, B 1, BL order, B 0, BL index (1..n),
/// version word; references: object (soft), previous and next dependency
/// on the object (soft), body (hard ownership)
/// ```
fn write_dependency(doc: &mut CadDocument, handle: Handle, action: Handle, d: &DependencyRecord) -> Result<(), DxfError> {
    put_plain_object(
        doc,
        "AECC_ASSOC_DEPENDENCY",
        handle,
        action,
        Vec::new(),
        |w| {
            w.write_bit_short(2);
            w.write_bit_long(0);
            for b in [!d.writes, d.writes, true, true] {
                w.write_bit(b);
            }
            w.write_bit_long(d.order);
            w.write_bit(false);
            w.write_bit_long(d.index);
            w.write_bit_short(VERSION);
        },
        Body { refs: vec![soft_pointer(d.object.value()), soft_pointer(d.previous.value()), soft_pointer(d.next.value()), Ref(HardOwnership, d.body.value())], strings: Vec::new() },
    )
}

/// A dependency record read back, with its owning action.
fn read_dependency(doc: &CadDocument, handle: Handle) -> Option<(Handle, DependencyRecord)> {
    let o = object(doc, handle).filter(|o| o.dxf_name == "AECC_ASSOC_DEPENDENCY")?;
    let (data, start, end) = o.main_stream()?;
    let mut b = super::super::bits::Bits::new(data, start, end);
    b.bit_short()?;
    b.bit_long()?;
    let (_, writes) = (b.bit()?, b.bit()?);
    b.bit()?;
    b.bit()?;
    let order = b.bit_long()?;
    b.bit()?;
    let index = b.bit_long()?;
    let r = o.typed_handles();
    let h = |i: usize| Handle::new(r.get(i).map_or(0, |x| x.0));
    Some((o.owner, DependencyRecord { object: h(0), writes, order, index, previous: h(1), next: h(2), body: h(3) }))
}

/// Writes the dependency `handle` (of `action`, owning `body`) into the
/// chain of dependencies on its entity, after those of a lower or equal
/// order, and relinks its neighbours; the entity lists its dependencies
/// as reactors in chain order (WT41).
fn link_dependency(doc: &mut CadDocument, handle: Handle, action: Handle, body: Handle, d: &Dependency, index: i32) -> Result<(), DxfError> {
    let reactors = doc.get_entity(d.object).map(|e| e.common().reactors.clone()).ok_or_else(|| invalid("an action depends on a missing entity"))?;
    let chain: Vec<(Handle, Handle, DependencyRecord)> = reactors.iter().filter_map(|r| read_dependency(doc, *r).map(|(a, x)| (*r, a, x))).collect();
    let at = chain.iter().position(|c| c.2.order > d.order).unwrap_or(chain.len());
    let previous = at.checked_sub(1).map_or(Handle::NULL, |i| chain[i].0);
    let next = chain.get(at).map_or(Handle::NULL, |c| c.0);
    write_dependency(doc, handle, action, &DependencyRecord { object: d.object, writes: d.writes, order: d.order, index, previous, next, body })?;
    for (i, (h, owner, mut x)) in chain.into_iter().enumerate() {
        if i + 1 == at {
            x.next = handle;
        } else if i == at {
            x.previous = handle;
        } else {
            continue;
        }
        write_dependency(doc, h, owner, &x)?;
    }
    let e = doc.get_entity_mut(d.object).ok_or_else(|| invalid("an action depends on a missing entity"))?;
    let reactors = &mut e.common_mut().reactors;
    let pos = match reactors.iter().position(|r| *r == next) {
        Some(p) if !next.is_null() => p,
        _ => reactors.iter().position(|r| *r == previous && !previous.is_null()).map_or(reactors.len(), |p| p + 1),
    };
    reactors.insert(pos, handle);
    Ok(())
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

/// Adds an action with `body` depending on `dependencies` (entities) to
/// the model space's network. Each dependency (`AECC_ASSOC_DEPENDENCY`)
/// owns a body (`AECC_ASSOC_DEPENDENCY_BODY`) and joins the chain of
/// dependencies on its entity, which lists it as a reactor.
///
/// ```text
/// action:     BS 2, BL 0, BL index, BL n, BL n, n x B 1 (owned), BS 0,
///             BL 0, BS 0, BL 0, version word; references: network (soft),
///             body (hard ownership), dependencies (hard ownership)
/// bodies:     BL 2 (action, then its tail) or BL 1 (dependency), version
///             word
/// ```
///
/// (OP01, OP02, OB01, WT41: the records equal the samples' bit for bit.)
pub(super) fn add_action(doc: &mut CadDocument, body: ActionBody, dependencies: &[Dependency]) -> Result<(), DxfError> {
    let net = model_space_network(doc)?;
    let action = doc.allocate_handle();
    let index = add_to(doc, net, action, true)?;
    let body_handle = doc.allocate_handle();
    let deps: Vec<(Handle, Handle)> = dependencies.iter().map(|_| (doc.allocate_handle(), doc.allocate_handle())).collect();
    let n = deps.len() as i32;
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
            w.write_bit_short(VERSION);
        },
        Body { refs: [soft_pointer(net.value()), Ref(HardOwnership, body_handle.value())].into_iter().chain(deps.iter().map(|d| Ref(HardOwnership, d.0.value()))).collect(), strings: Vec::new() },
    )?;
    put_plain_object(
        doc,
        body.dxf,
        body_handle,
        action,
        Vec::new(),
        |w| {
            w.write_bit_long(2);
            w.write_bit_short(VERSION);
            bits(w, body.tail);
        },
        Body { refs: body.refs.iter().map(|h| hard_pointer(h.value())).collect(), strings: Vec::new() },
    )?;
    for (i, ((dep, dep_body), d)) in deps.iter().zip(dependencies).enumerate() {
        link_dependency(doc, *dep, action, *dep_body, d, i as i32 + 1)?;
        put_plain_object(
            doc,
            "AECC_ASSOC_DEPENDENCY_BODY",
            *dep_body,
            *dep,
            Vec::new(),
            |w| {
                w.write_bit_long(1);
                w.write_bit_short(VERSION);
            },
            Body { refs: Vec::new(), strings: Vec::new() },
        )?;
    }
    Ok(())
}
