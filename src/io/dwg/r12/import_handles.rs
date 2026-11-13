//! Identities assigned when a pre-R13 database is imported into the object model.
//! R12 tables have indices rather than stored handles. Reserve the importer
//! defaults first, then allocate in table order, independent of DXF reader state.
//! Allocation order is based on independent imports into ACAD 2024/2025/2027;
//! these are synthesized import identities, not fields stored in the R12 file.

use super::entities::EntityRecord;
use crate::error::{DxfError, Result};
use crate::objects::ObjectType;
use crate::{CadDocument, Handle};

pub(super) struct ImportHandles {
    base: u64,
    space_markers: u64,
}

fn set(record: &mut EntityRecord, code: i32, value: u64) {
    record.pairs.retain(|(c, _)| *c != code);
    record.pairs.push((code, format!("{value:X}")));
}

impl ImportHandles {
    pub(super) fn assign(
        seed: u64,
        tables: &mut [Vec<EntityRecord>; 10],
        blocks: &mut [EntityRecord],
        entities: &mut [EntityRecord],
        font_resolver: Option<&super::ImportFontResolver>,
        allocation_order: Option<&std::collections::HashMap<u32, usize>>,
    ) -> Result<Self> {
        let base = seed.max(1);
        let mut next = base
            .checked_add(114)
            .ok_or_else(|| DxfError::InvalidFormat("AC1009 import handle overflow".into()))?;
        // BLOCK, STYLE, LTYPE, VIEW, UCS, VPORT, APPID, DIMSTYLE, VX, LAYER.
        for table in [0, 2, 3, 4, 5, 6, 7, 8, 9, 1] {
            for (index, record) in tables[table].iter_mut().enumerate() {
                let default_offset = match (table, index) {
                    (1, 0) => Some(83),
                    (2, 0) => Some(84),
                    (2, 1) => Some(88),
                    (3, 0) => Some(91),
                    (7, 0) => Some(85),
                    (8, 0) => Some(86),
                    (8, 1) => Some(87),
                    _ => None,
                };
                let handle = if let Some(offset) = default_offset {
                    base + offset
                } else {
                    let handle = next;
                    next = next.checked_add(1).ok_or_else(|| {
                        DxfError::InvalidFormat("AC1009 import handle overflow".into())
                    })?;
                    handle
                };
                set(record, if table == 8 { 105 } else { 5 }, handle);
                // R12 BLOCK flags describe the block type, whereas DXF's
                // BLOCK_RECORD group 70 represents insertion units.
                if table == 0 {
                    record.pairs.retain(|(code, _)| *code != 70);
                }
            }
        }
        let space_markers = next;
        next = next
            .checked_add(4)
            .ok_or_else(|| DxfError::InvalidFormat("AC1009 import handle overflow".into()))?;
        if !tables[6]
            .iter()
            .any(|r| r.string(2).eq_ignore_ascii_case("*ACTIVE") && r.integer(70, 0) & 128 == 0)
        {
            let mut viewport = EntityRecord::new("VPORT");
            viewport.pairs.push((2, "*Active".into()));
            set(&mut viewport, 5, next);
            next = next
                .checked_add(1)
                .ok_or_else(|| DxfError::InvalidFormat("AC1009 import handle overflow".into()))?;
            tables[6].push(viewport);
        }
        // The importer creates one transient identity on first use of each
        // effective text font. The caller may resolve aliases/substitutions;
        // no operating-system font or application-profile lookup occurs here.
        let mut fonts = std::collections::HashSet::new();
        let mut records: Vec<_> = blocks.iter_mut().chain(entities.iter_mut()).collect();
        if let Some(order) = allocation_order {
            // Follow native import order even after JUMP traversal groups
            // relocated children with their logical owners.
            records.sort_by_key(|record| {
                record
                    .native_address
                    .and_then(|address| order.get(&address).copied())
                    .unwrap_or(usize::MAX)
            });
        }
        for record in records {
            if record.kind == "REPEAT" {
                continue;
            }
            if record.string(5).is_empty() || record.string(5) == "0" {
                set(record, 5, next);
                next = next.checked_add(1).ok_or_else(|| {
                    DxfError::InvalidFormat("AC1009 import handle overflow".into())
                })?;
            }
            if matches!(record.kind.as_str(), "TEXT" | "ATTRIB" | "ATTDEF") {
                let name = if record.string(7).is_empty() {
                    "STANDARD"
                } else {
                    record.string(7)
                };
                if let Some(style) = tables[2]
                    .iter()
                    .find(|s| s.string(2).eq_ignore_ascii_case(name))
                {
                    let resolve = |name: &str| {
                        if name.is_empty() {
                            String::new()
                        } else if let Some(resolver) = font_resolver {
                            resolver(name)
                        } else {
                            name.to_ascii_uppercase()
                        }
                    };
                    let font = (resolve(style.string(3)), resolve(style.string(4)));
                    if fonts.insert(font) {
                        next = next.checked_add(1).ok_or_else(|| {
                            DxfError::InvalidFormat("AC1009 import handle overflow".into())
                        })?;
                    }
                }
            }
        }
        for (name, offset, layout) in [("*Model_Space", 110, 113), ("*Paper_Space", 106, 109)] {
            let mut record = EntityRecord::new("BLOCK_RECORD");
            record.pairs.push((2, name.into()));
            set(&mut record, 5, base + offset);
            set(&mut record, 340, base + layout);
            tables[0].push(record);
        }
        for (table, kind, name, offset) in [
            (1, "LAYER", "0", 83),
            (2, "STYLE", "STANDARD", 84),
            (3, "LTYPE", "ByBlock", 89),
            (3, "LTYPE", "ByLayer", 90),
            (3, "LTYPE", "CONTINUOUS", 91),
            (7, "APPID", "AcadAnnotative", 92),
            (7, "APPID", "ACAD", 85),
            (8, "DIMSTYLE", "STANDARD", 86),
        ] {
            if !tables[table].iter().any(|r| {
                r.string(2).eq_ignore_ascii_case(name)
                    || u64::from_str_radix(r.string(if table == 8 { 105 } else { 5 }), 16).ok()
                        == Some(base + offset)
            }) {
                let mut record = EntityRecord::new(kind);
                record.pairs.push((2, name.into()));
                set(&mut record, if table == 8 { 105 } else { 5 }, base + offset);
                tables[table].push(record);
            }
        }
        Ok(Self {
            base,
            space_markers,
        })
    }

    pub(super) fn control(&self, table: usize) -> u64 {
        self.base + [0, 1, 2, 4, 5, 6, 7, 8, 9, 10][table]
    }

    pub(super) fn apply_layouts(&self, document: &mut CadDocument) -> Result<()> {
        // The reserved marker handles appear nowhere in the bridge, so its
        // default objects may already hold them; such a marker takes a fresh
        // handle instead (nothing refers to a marker but its block, #371).
        let mut used = used_handles(document);
        for (name, offset) in [("*Paper_Space", 0), ("*Model_Space", 2)] {
            if document.block_records.get(name).is_none() {
                continue;
            }
            let mut marker = |reserved: u64| {
                if used.insert(reserved) {
                    Handle::new(reserved)
                } else {
                    let fresh = document.allocate_handle();
                    used.insert(fresh.value());
                    fresh
                }
            };
            let begin = marker(self.space_markers + offset);
            let end = marker(self.space_markers + offset + 1);
            if let Some(block) = document.block_records.get_mut(name) {
                block.block_entity_handle = begin;
                block.block_end_handle = end;
            }
        }
        for (name, offset) in [("Model", 113), ("Layout1", 109)] {
            let old = document.objects.iter().find_map(|(h, o)| match o {
                ObjectType::Layout(l) if l.name == name => Some(*h),
                _ => None,
            });
            let Some(old) = old else {
                continue;
            };
            let target = Handle::new(self.base + offset);
            if old == target {
                continue;
            }
            if document.objects.contains_key(&target) || document.get_entity(target).is_some() {
                return Err(DxfError::InvalidFormat(
                    "AC1009 imported layout handle collision".into(),
                ));
            }
            let Some(ObjectType::Layout(mut layout)) = document.objects.remove(&old) else {
                unreachable!()
            };
            layout.handle = target;
            document.objects.insert(target, ObjectType::Layout(layout));
            for object in document.objects.values_mut() {
                if let ObjectType::Dictionary(dict) = object {
                    for (_, value) in &mut dict.entries {
                        if *value == old {
                            *value = target;
                        }
                    }
                }
            }
            for block in document.block_records.iter_mut() {
                if block.layout == old {
                    block.layout = target;
                }
            }
        }
        Ok(())
    }
}

/// Every handle the document's records hold.
fn used_handles(document: &CadDocument) -> std::collections::HashSet<u64> {
    use crate::tables::TableEntry;
    let mut used: std::collections::HashSet<u64> =
        document.objects.keys().map(|h| h.value()).collect();
    used.extend(document.entities().map(|e| e.common().handle.value()));
    macro_rules! table {
        ($table:expr) => {
            used.insert($table.handle().value());
            used.extend($table.iter().map(|entry| entry.handle().value()));
        };
    }
    table!(document.layers);
    table!(document.line_types);
    table!(document.text_styles);
    table!(document.dim_styles);
    table!(document.app_ids);
    table!(document.views);
    table!(document.vports);
    table!(document.ucss);
    table!(document.vx_table);
    table!(document.block_records);
    // The space blocks' markers are the ones being replaced.
    let space = |name: &str| {
        name.eq_ignore_ascii_case("*Model_Space") || name.eq_ignore_ascii_case("*Paper_Space")
    };
    for block in document.block_records.iter().filter(|b| !space(&b.name)) {
        used.insert(block.block_entity_handle.value());
        used.insert(block.block_end_handle.value());
    }
    used.remove(&0);
    used
}
