//! Multiline attributes in DXF before R2018.
//!
//! Those versions have no embedded MTEXT on ATTRIB/ATTDEF. AutoCAD writes a
//! multiline attribute as one single-line attribute per line, tagged
//! `<TAG>_001`, `<TAG>_002`, ...; the first one has an extension dictionary
//! with an `ACAD_MLATT` XRECORD (type, the other lines, the real tag and the
//! MTEXT) and an `ACAD_XREC_ROUNDTRIP` XRECORD (`ACAD_MATT_HEIGHT`).

use crate::document::CadDocument;
use crate::entities::attribute_definition::MTextFlag;
use crate::entities::mtext::{AttachmentPoint, DrawingDirection};
use crate::entities::{AttributeDefinition, AttributeEntity, EntityType, MText};
use crate::objects::{
    Dictionary, DictionaryCloningFlags, ObjectType, XRecord, XRecordEntry, XRecordValue,
};
use crate::types::{Handle, Vector3};

const MLATT: &str = "ACAD_MLATT";
const ROUNDTRIP: &str = "ACAD_XREC_ROUNDTRIP";

/// The pieces both attribute kinds share.
struct Lines {
    tag: String,
    texts: Vec<String>,
    mtext: MText,
    /// Offset from one line's base point to the next.
    step: Vector3,
}

fn lines(
    tag: &str,
    value: &str,
    embedded: Option<&MText>,
    insertion: Vector3,
    height: f64,
    rotation: f64,
) -> Lines {
    let mtext = embedded.cloned().unwrap_or_else(|| {
        let mut mtext = MText::with_value(value, insertion);
        mtext.height = height;
        mtext.rotation = rotation;
        mtext
    });
    let text = if mtext.value.is_empty() {
        value
    } else {
        &mtext.value
    };
    // AutoCAD's line pitch: 5/3 of the text height times the spacing factor.
    let pitch = mtext.height * mtext.line_spacing_factor * 5.0 / 3.0;
    Lines {
        tag: tag.to_string(),
        texts: text.split("\\P").map(str::to_string).collect(),
        step: Vector3::new(rotation.sin() * pitch, -rotation.cos() * pitch, 0.0),
        mtext,
    }
}

fn line_tag(tag: &str, index: usize) -> String {
    format!("{tag}_{:03}", index + 1)
}

fn new_dictionary(doc: &mut CadDocument, owner: Handle) -> Handle {
    let handle = doc.allocate_handle();
    let mut dictionary = Dictionary::new();
    dictionary.handle = handle;
    dictionary.owner = owner;
    dictionary.hard_owner = true;
    dictionary.duplicate_cloning = 1;
    doc.objects
        .insert(handle, ObjectType::Dictionary(dictionary));
    handle
}

fn add_record(doc: &mut CadDocument, dictionary: Handle, key: &str, entries: Vec<XRecordEntry>) {
    let handle = doc.allocate_handle();
    let mut record = XRecord::named(key);
    record.handle = handle;
    record.owner = dictionary;
    record.reactors = vec![dictionary];
    record.cloning_flags = DictionaryCloningFlags::KeepExisting;
    record.entries = entries;
    doc.objects.insert(handle, ObjectType::XRecord(record));
    if let Some(ObjectType::Dictionary(d)) = doc.objects.get_mut(&dictionary) {
        d.add_entry(key, handle);
    }
}

/// Give the first line its records; the other lines get an empty dictionary.
fn add_records(
    doc: &mut CadDocument,
    first: Handle,
    others: &[Handle],
    kind: i16,
    l: &Lines,
    existing: Option<Handle>,
) -> Handle {
    let m = &l.mtext;
    let mut entries = vec![
        XRecordEntry::int16(70, kind),
        XRecordEntry::int16(70, 0),
        XRecordEntry::int16(70, others.len() as i16),
    ];
    entries.extend(others.iter().map(|h| XRecordEntry::handle(340, *h)));
    entries.extend([
        XRecordEntry::double(40, 1.0),
        XRecordEntry::string(2, &l.tag),
        XRecordEntry::string(1, "Embedded Object"),
        XRecordEntry::point3d(
            10,
            m.insertion_point.x,
            m.insertion_point.y,
            m.insertion_point.z,
        ),
        XRecordEntry::double(40, m.height),
        XRecordEntry::double(41, m.rectangle_width),
        XRecordEntry::double(46, m.rectangle_height.unwrap_or(0.0)),
        XRecordEntry::int16(71, m.attachment_point as i16),
        XRecordEntry::int16(72, m.drawing_direction as i16),
        XRecordEntry::string(1, &m.value),
    ]);
    if !m.style.is_empty() {
        entries.push(XRecordEntry::string(7, &m.style));
    }
    if m.rotation != 0.0 {
        entries.push(XRecordEntry::point3d(
            11,
            m.rotation.cos(),
            m.rotation.sin(),
            0.0,
        ));
    }
    entries.push(XRecordEntry::int16(73, m.line_spacing_style as i16));
    entries.push(XRecordEntry::double(44, m.line_spacing_factor));
    // An attribute that already has an extension dictionary keeps it.
    let dictionary = existing
        .filter(|h| matches!(doc.objects.get(h), Some(ObjectType::Dictionary(_))))
        .unwrap_or_else(|| new_dictionary(doc, first));
    add_record(doc, dictionary, MLATT, entries);
    let height = [
        XRecordEntry::string(102, HEIGHT_TAG),
        XRecordEntry::double(140, m.height),
    ];
    // A round-trip record already there is shared: the height joins it.
    let existing = match doc.objects.get(&dictionary) {
        Some(ObjectType::Dictionary(d)) => d.get(ROUNDTRIP),
        _ => None,
    };
    match existing.and_then(|h| doc.objects.get_mut(&h)) {
        Some(ObjectType::XRecord(record)) => {
            strip_height(&mut record.entries);
            record.entries.extend(height);
        }
        _ => add_record(doc, dictionary, ROUNDTRIP, height.to_vec()),
    }
    dictionary
}

const HEIGHT_TAG: &str = "ACAD_MATT_HEIGHT";

/// Whether `entries` hold the multiline attribute's height and nothing else.
fn height_only(entries: &[XRecordEntry]) -> bool {
    matches!(entries, [tag, value]
        if tag.code == 102 && tag.value.as_string() == Some(HEIGHT_TAG) && value.code == 140)
}

/// Remove the `102 ACAD_MATT_HEIGHT` / `140` pair, keeping the rest.
fn strip_height(entries: &mut Vec<XRecordEntry>) {
    if let Some(at) = entries
        .iter()
        .position(|e| e.code == 102 && e.value.as_string() == Some(HEIGHT_TAG))
    {
        let end = (at + 2).min(entries.len());
        let pair = entries.get(at + 1).is_some_and(|e| e.code == 140);
        entries.drain(at..if pair { end } else { at + 1 });
    }
}

fn split_attrib(doc: &mut CadDocument, a: &AttributeEntity) -> Vec<AttributeEntity> {
    let l = lines(
        &a.tag,
        &a.value,
        a.embedded_mtext.as_deref(),
        a.insertion_point,
        a.height,
        a.rotation,
    );
    let mut out = Vec::with_capacity(l.texts.len());
    for (i, text) in l.texts.iter().enumerate() {
        let mut line = a.clone();
        if i > 0 {
            line.common.handle = doc.allocate_handle();
            let offset = l.step * i as f64;
            line.insertion_point = a.insertion_point + offset;
            line.alignment_point = a.alignment_point + offset;
            line.lock_position = true;
        }
        line.tag = line_tag(&l.tag, i);
        line.value = text.clone();
        line.is_multiline = false;
        line.mtext_flag = MTextFlag::SingleLine;
        line.line_count = 1;
        line.embedded_mtext = None;
        line.common.raw_record = None;
        out.push(line);
    }
    let others: Vec<Handle> = out[1..].iter().map(|line| line.common.handle).collect();
    out[0].common.xdictionary_handle = Some(add_records(
        doc,
        a.common.handle,
        &others,
        2,
        &l,
        a.common.xdictionary_handle,
    ));
    for line in &mut out[1..] {
        line.common.xdictionary_handle = Some(new_dictionary(doc, line.common.handle));
    }
    out
}

fn split_attdef(doc: &mut CadDocument, a: &AttributeDefinition) -> Vec<AttributeDefinition> {
    let l = lines(
        &a.tag,
        &a.default_value,
        a.embedded_mtext.as_deref(),
        a.insertion_point,
        a.height,
        a.rotation,
    );
    let mut out = Vec::with_capacity(l.texts.len());
    for (i, text) in l.texts.iter().enumerate() {
        let mut line = a.clone();
        if i > 0 {
            line.common.handle = doc.allocate_handle();
            let offset = l.step * i as f64;
            line.insertion_point = a.insertion_point + offset;
            line.alignment_point = a.alignment_point + offset;
        }
        line.tag = line_tag(&l.tag, i);
        line.default_value = text.clone();
        line.is_multiline = false;
        line.mtext_flag = MTextFlag::SingleLine;
        line.line_count = 1;
        line.embedded_mtext = None;
        line.common.raw_record = None;
        out.push(line);
    }
    let others: Vec<Handle> = out[1..].iter().map(|line| line.common.handle).collect();
    out[0].common.xdictionary_handle = Some(add_records(
        doc,
        a.common.handle,
        &others,
        4,
        &l,
        a.common.xdictionary_handle,
    ));
    for line in &mut out[1..] {
        line.common.xdictionary_handle = Some(new_dictionary(doc, line.common.handle));
    }
    out
}

fn is_multiline(flag: bool, embedded: Option<&MText>) -> bool {
    flag || embedded.is_some()
}

/// Whether [`split`] has anything to do.
pub(crate) fn has_multiline_attributes(doc: &CadDocument) -> bool {
    doc.entities().any(|e| match e {
        EntityType::AttributeDefinition(a) => {
            is_multiline(a.is_multiline, a.embedded_mtext.as_deref())
        }
        EntityType::Insert(i) => i
            .attributes
            .iter()
            .any(|a| is_multiline(a.is_multiline, a.embedded_mtext.as_deref())),
        _ => false,
    })
}

/// Write side: replace every multiline attribute by AutoCAD's pre-R2018 lines.
pub(crate) fn split(doc: &mut CadDocument) {
    let mut inserts = Vec::new();
    let mut attdefs = Vec::new();
    for e in doc.entities() {
        match e {
            EntityType::Insert(i)
                if i.attributes
                    .iter()
                    .any(|a| is_multiline(a.is_multiline, a.embedded_mtext.as_deref())) =>
            {
                inserts.push(i.common.handle)
            }
            EntityType::AttributeDefinition(a)
                if is_multiline(a.is_multiline, a.embedded_mtext.as_deref()) =>
            {
                attdefs.push(a.clone())
            }
            _ => {}
        }
    }
    for handle in inserts {
        let Some(EntityType::Insert(insert)) = doc.get_entity(handle).cloned() else {
            continue;
        };
        let mut attributes = Vec::new();
        for a in &insert.attributes {
            if is_multiline(a.is_multiline, a.embedded_mtext.as_deref()) {
                attributes.extend(split_attrib(doc, a));
            } else {
                attributes.push(a.clone());
            }
        }
        if let Some(EntityType::Insert(i)) = doc.get_entity_mut(handle) {
            i.attributes = attributes;
        }
    }
    for a in attdefs {
        let owner = a.common.owner_handle;
        let mut lines = split_attdef(doc, &a).into_iter();
        let first = lines.next().expect("one line at least");
        if let Some(EntityType::AttributeDefinition(e)) = doc.get_entity_mut(a.common.handle) {
            *e = first;
        }
        let mut after = a.common.handle;
        for line in lines {
            let Ok(handle) = doc.add_entity(EntityType::AttributeDefinition(line)) else {
                continue;
            };
            // Keep the lines together in the block, in order.
            if let Some(record) = doc.block_records.iter_mut().find(|r| r.handle == owner) {
                record.entity_handles.retain(|h| *h != handle);
                let at = record
                    .entity_handles
                    .iter()
                    .position(|h| *h == after)
                    .map_or(record.entity_handles.len(), |p| p + 1);
                record.entity_handles.insert(at, handle);
            }
            after = handle;
        }
    }
}

/// The record's real tag, other lines, type and MTEXT.
struct Record {
    kind: i16,
    tag: String,
    others: Vec<Handle>,
    mtext: MText,
}

fn parse(entries: &[XRecordEntry]) -> Option<Record> {
    let embedded = entries
        .iter()
        .position(|e| e.code == 1 && e.value.as_string() == Some("Embedded Object"))?;
    let (head, body) = (&entries[..embedded], &entries[embedded + 1..]);
    let kind = head
        .iter()
        .find(|e| e.code == 70)
        .and_then(|e| match e.value {
            XRecordValue::Int16(v) => Some(v),
            _ => None,
        })?;
    let tag = head
        .iter()
        .find(|e| e.code == 2)?
        .value
        .as_string()?
        .to_string();
    let others = head
        .iter()
        .filter_map(|e| match (e.code, &e.value) {
            (340, XRecordValue::Handle(h)) => Some(*h),
            _ => None,
        })
        .collect();
    let mut mtext = MText {
        rectangle_width: 0.0,
        ..MText::new()
    };
    let mut rotation = None;
    for e in body {
        let real = match e.value {
            XRecordValue::Double(v) => Some(v),
            _ => None,
        };
        let short = match e.value {
            XRecordValue::Int16(v) => Some(v),
            _ => None,
        };
        match (e.code, &e.value) {
            (1 | 3, XRecordValue::String(s)) => mtext.value.push_str(s),
            (7, XRecordValue::String(s)) => mtext.style = s.clone(),
            (10, XRecordValue::Point3D(x, y, z)) => {
                mtext.insertion_point = Vector3::new(*x, *y, *z)
            }
            (11, XRecordValue::Point3D(x, y, z)) if *x != 0.0 || *y != 0.0 => {
                rotation = Some(y.atan2(*x));
                mtext.dwg_x_direction = Some(Vector3::new(*x, *y, *z));
            }
            (210, XRecordValue::Point3D(x, y, z)) => mtext.normal = Vector3::new(*x, *y, *z),
            (40, _) => mtext.height = real.unwrap_or(mtext.height),
            (41, _) => mtext.rectangle_width = real.unwrap_or(0.0),
            (42, _) => mtext.extents_width = real.unwrap_or(0.0),
            (43, _) => mtext.extents_height = real.unwrap_or(0.0),
            (44, _) => mtext.line_spacing_factor = real.unwrap_or(mtext.line_spacing_factor),
            (46, _) => mtext.rectangle_height = real.filter(|h| *h != 0.0),
            (71, _) => {
                mtext.attachment_point = match short.unwrap_or(1) {
                    2 => AttachmentPoint::TopCenter,
                    3 => AttachmentPoint::TopRight,
                    4 => AttachmentPoint::MiddleLeft,
                    5 => AttachmentPoint::MiddleCenter,
                    6 => AttachmentPoint::MiddleRight,
                    7 => AttachmentPoint::BottomLeft,
                    8 => AttachmentPoint::BottomCenter,
                    9 => AttachmentPoint::BottomRight,
                    _ => AttachmentPoint::TopLeft,
                }
            }
            (72, _) => {
                mtext.drawing_direction = match short.unwrap_or(1) {
                    3 => DrawingDirection::TopToBottom,
                    5 => DrawingDirection::ByStyle,
                    _ => DrawingDirection::LeftToRight,
                }
            }
            (73, _) => {
                if let Some(v) = short {
                    mtext.line_spacing_style = crate::entities::LineSpacingStyle::from(v);
                }
            }
            _ => {}
        }
    }
    if let Some(r) = rotation {
        mtext.rotation = r;
    }
    Some(Record {
        kind,
        tag,
        others,
        mtext,
    })
}

/// The parsed `ACAD_MLATT` record, the objects to drop with it, and the
/// dictionary to keep (it holds more than the two records) if any.
fn take_record(
    doc: &CadDocument,
    xdictionary: Option<Handle>,
) -> Option<(Record, Vec<Handle>, Option<Handle>)> {
    let dictionary = xdictionary?;
    let Some(ObjectType::Dictionary(d)) = doc.objects.get(&dictionary) else {
        return None;
    };
    let Some(ObjectType::XRecord(record)) = doc.objects.get(&d.get(MLATT)?) else {
        return None;
    };
    if !record.entries_complete {
        return None;
    }
    let parsed = parse(&record.entries)?;
    // The two records go; the dictionary too when it holds nothing else. The
    // round-trip record is shared: it goes only when it holds just the height.
    let ours = |key: &String| {
        key == MLATT
            || (key == ROUNDTRIP
                && d.get(ROUNDTRIP).is_some_and(|h| {
                    matches!(
                        doc.objects.get(&h),
                        Some(ObjectType::XRecord(r)) if height_only(&r.entries)
                    )
                }))
    };
    let mut drop: Vec<Handle> = d
        .entries
        .iter()
        .filter(|(key, _)| ours(key))
        .map(|(_, h)| *h)
        .collect();
    let keep = if d.entries.iter().all(|(key, _)| ours(key)) {
        drop.push(dictionary);
        None
    } else {
        Some(dictionary)
    };
    Some((parsed, drop, keep))
}

/// Read side: turn AutoCAD's pre-R2018 lines back into one multiline attribute.
pub(crate) fn merge(doc: &mut CadDocument) {
    let mut drop_objects = Vec::new();
    let mut drop_lines: Vec<Handle> = Vec::new();
    let mut attdefs = Vec::new();
    let mut inserts = Vec::new();
    for e in doc.entities() {
        match e {
            EntityType::AttributeDefinition(a) => {
                if let Some(found) = take_record(doc, a.common.xdictionary_handle) {
                    attdefs.push((a.common.handle, found));
                }
            }
            EntityType::Insert(i) => {
                let found: Vec<_> = i
                    .attributes
                    .iter()
                    .enumerate()
                    .filter_map(|(n, a)| {
                        take_record(doc, a.common.xdictionary_handle).map(|f| (n, f))
                    })
                    .collect();
                if !found.is_empty() {
                    inserts.push((i.common.handle, found));
                }
            }
            _ => {}
        }
    }
    let line_dictionaries = |doc: &CadDocument, handles: &[Handle]| -> Vec<Handle> {
        handles
            .iter()
            .filter_map(|h| {
                doc.get_entity(*h)
                    .and_then(|e| e.common().xdictionary_handle)
            })
            .collect()
    };
    let mut keep_dictionaries = Vec::new();
    for (handle, (mut record, drop, keep)) in attdefs {
        drop_objects.extend(drop);
        keep_dictionaries.extend(keep);
        // Only other ATTDEF lines of the same block, never this one or
        // whatever else a broken 340 names.
        let owner = doc.get_entity(handle).map(|e| e.common().owner_handle);
        record.others.retain(|h| {
            *h != handle
                && matches!(doc.get_entity(*h), Some(EntityType::AttributeDefinition(o))
                    if Some(o.common.owner_handle) == owner)
        });
        if record.mtext.value.is_empty() {
            record.mtext.value = std::iter::once(handle)
                .chain(record.others.iter().copied())
                .filter_map(|h| match doc.get_entity(h) {
                    Some(EntityType::AttributeDefinition(o)) => Some(o.default_value.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\\P");
        }
        drop_objects.extend(line_dictionaries(doc, &record.others));
        drop_lines.extend(&record.others);
        if let Some(EntityType::AttributeDefinition(a)) = doc.get_entity_mut(handle) {
            a.tag = record.tag;
            a.default_value = record.mtext.value.clone();
            a.mtext_flag = MTextFlag::from_value(record.kind);
            a.is_multiline = true;
            a.line_count = a.default_value.matches("\\P").count() as i16 + 1;
            a.embedded_mtext = Some(Box::new(record.mtext));
            if keep.is_none() {
                a.common.xdictionary_handle = None;
            }
        }
    }
    for (handle, found) in inserts {
        let Some(EntityType::Insert(insert)) = doc.get_entity_mut(handle) else {
            continue;
        };
        let mut gone = Vec::new();
        for (n, (mut record, drop, keep)) in found {
            drop_objects.extend(drop);
            keep_dictionaries.extend(keep);
            // Only other attributes of this INSERT, never this one.
            let own = insert.attributes[n].common.handle;
            record
                .others
                .retain(|h| *h != own && insert.attributes.iter().any(|o| o.common.handle == *h));
            if record.mtext.value.is_empty() {
                record.mtext.value = std::iter::once(own)
                    .chain(record.others.iter().copied())
                    .filter_map(|h| {
                        insert
                            .attributes
                            .iter()
                            .find(|o| o.common.handle == h)
                            .map(|o| o.value.clone())
                    })
                    .collect::<Vec<_>>()
                    .join("\\P");
            }
            gone.extend(record.others.iter().copied());
            let a = &mut insert.attributes[n];
            a.tag = record.tag;
            a.value = record.mtext.value.clone();
            a.mtext_flag = MTextFlag::from_value(record.kind);
            a.is_multiline = true;
            a.line_count = a.value.matches("\\P").count() as i16 + 1;
            a.embedded_mtext = Some(Box::new(record.mtext));
            if keep.is_none() {
                a.common.xdictionary_handle = None;
            }
        }
        drop_objects.extend(
            insert
                .attributes
                .iter()
                .filter(|a| gone.contains(&a.common.handle))
                .filter_map(|a| a.common.xdictionary_handle),
        );
        insert
            .attributes
            .retain(|a| !gone.contains(&a.common.handle));
    }
    for handle in drop_lines {
        doc.remove_entity(handle);
        for record in doc.block_records.iter_mut() {
            record.entity_handles.retain(|h| *h != handle);
        }
    }
    for handle in &drop_objects {
        doc.objects.remove(handle);
    }
    for handle in keep_dictionaries {
        let mut shared = None;
        if let Some(ObjectType::Dictionary(d)) = doc.objects.get_mut(&handle) {
            d.entries.retain(|(key, h)| {
                key != MLATT && !(key == ROUNDTRIP && drop_objects.contains(h))
            });
            shared = d.get(ROUNDTRIP);
        }
        // A shared round-trip record keeps everything but the height.
        if let Some(ObjectType::XRecord(record)) = shared.and_then(|h| doc.objects.get_mut(&h)) {
            strip_height(&mut record.entries);
        }
    }
}
