//! Section-relative control flow for the pre-R13 entity streams.

use super::entities::{EntityRecord, PositionedRecord};
use crate::error::{DxfError, Result};

#[derive(Clone, Debug, Default)]
pub(crate) struct SectionRecords {
    pub base_address: u32,
    pub size: u32,
    pub records: Vec<PositionedRecord>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Owner {
    Entities,
    Blocks,
}

#[derive(Clone, Copy)]
struct Cursor {
    section: usize,
    index: usize,
}

struct Sequence {
    address: u32,
    /// Address of the JUMP that led to the header, if one did. When AutoCAD
    /// moves a header into the extra section it leaves a JUMP in its place,
    /// and the SEQEND may still name that old address (R10 sample JUMP1).
    jumped_from: Option<u32>,
    child_kind: &'static str,
}

fn invalid(message: impl Into<String>) -> DxfError {
    DxfError::InvalidFormat(message.into())
}

struct Walker {
    sections: [SectionRecords; 3],
    visited: [Vec<Option<Owner>>; 3],
    entities: Vec<EntityRecord>,
    blocks: Vec<EntityRecord>,
}

impl Walker {
    /// R11/R12 stores compound-owner addresses with the same section marker
    /// used by JUMP targets (`0x40000000` for BLOCKS and `0x80000000` for the
    /// extra stream).  The records we expose to the bridge use absolute file
    /// addresses, so compare both forms when a SEQEND arrives after a JUMP.
    fn sequence_owner_matches(&self, raw: u32, address: u32) -> bool {
        if raw == address {
            return true;
        }
        self.sections.iter().enumerate().any(|(section, records)| {
            let Some(offset) = address.checked_sub(records.base_address) else {
                return false;
            };
            if offset > records.size {
                return false;
            }
            let flag = match section {
                0 => 0,
                1 => 0x4000_0000,
                2 => 0x8000_0000,
                _ => return false,
            };
            raw == flag | offset
        })
    }

    fn jump_target(&self, raw: u32, owner: Owner) -> Result<Option<Cursor>> {
        let mut offset = raw & 0x00ff_ffff;
        let section = match raw >> 24 {
            // ACAD's change-stream return markers address the original
            // file directly. Also accept section-relative legacy markers.
            0 if self.sections[0].base_address <= raw
                && raw
                    <= self.sections[0]
                        .base_address
                        .saturating_add(self.sections[0].size) =>
            {
                offset = raw - self.sections[0].base_address;
                0
            }
            0 if self.sections[1].size != 0
                && self.sections[1].base_address <= raw
                && raw
                    <= self.sections[1]
                        .base_address
                        .saturating_add(self.sections[1].size) =>
            {
                offset = raw - self.sections[1].base_address;
                1
            }
            0 => 0,
            0x40 => 1,
            0x80 => 2,
            flag => {
                return Err(invalid(format!(
                    "Invalid AC1009 JUMP section flag {flag:#x}"
                )))
            }
        };
        if matches!((owner, section), (Owner::Entities, 1) | (Owner::Blocks, 0)) {
            return Err(invalid("AC1009 JUMP crosses entity/block ownership"));
        }
        let target = &self.sections[section];
        if offset == target.size {
            return Ok(None);
        }
        if offset > target.size {
            return Err(invalid(format!(
                "AC1009 JUMP target {raw:#x} exceeds section bounds"
            )));
        }
        let index = target
            .records
            .binary_search_by_key(&offset, |record| record.offset)
            .map_err(|_| {
                invalid(format!(
                    "AC1009 JUMP target {raw:#x} is not a record boundary"
                ))
            })?;
        Ok(Some(Cursor { section, index }))
    }

    fn next(&self, cursor: Cursor) -> Option<Cursor> {
        (cursor.index + 1 < self.sections[cursor.section].records.len()).then_some(Cursor {
            section: cursor.section,
            index: cursor.index + 1,
        })
    }

    fn walk(&mut self, start: Cursor, owner: Owner) -> Result<()> {
        let mut cursor = Some(start);
        let mut sequence: Option<Sequence> = None;
        let mut in_block = false;
        // First JUMP of the chain that reached the current record.
        let mut jumped_from: Option<u32> = None;
        while let Some(current) = cursor {
            let record = &self.sections[current.section].records[current.index];
            if let Some(previous) = self.visited[current.section][current.index] {
                let message = if previous != owner {
                    "duplicate entity/block ownership"
                } else {
                    "cycle or duplicate record ownership"
                };
                return Err(invalid(format!(
                    "AC1009 JUMP {message} at {:#x}",
                    record.address
                )));
            }
            self.visited[current.section][current.index] = Some(owner);
            if let Some(target) = record.jump_address {
                jumped_from.get_or_insert(record.address);
                cursor = self.jump_target(target, owner)?;
                continue;
            }
            let reached_from = jumped_from.take();
            if let Some(entity) = &record.record {
                let kind = entity.kind.as_str();
                match kind {
                    "BLOCK" => {
                        if owner != Owner::Blocks || in_block || sequence.is_some() {
                            return Err(invalid("Invalid AC1009 BLOCK ownership"));
                        }
                        in_block = true;
                    }
                    "ENDBLK" => {
                        if !in_block || sequence.is_some() {
                            return Err(invalid("Invalid AC1009 ENDBLK ownership"));
                        }
                        in_block = false;
                    }
                    "VERTEX" | "ATTRIB" => {
                        if sequence.as_ref().map(|s| s.child_kind) != Some(kind) {
                            return Err(invalid(format!(
                                "Orphan AC1009 {kind} at {:#x}",
                                record.address
                            )));
                        }
                    }
                    "SEQEND" => {
                        let Some(open) = sequence.take() else {
                            return Err(invalid(format!(
                                "Orphan AC1009 SEQEND at {:#x}",
                                record.address
                            )));
                        };
                        if !record.sequence_owner.is_some_and(|owner| {
                            self.sequence_owner_matches(owner, open.address)
                                || open
                                    .jumped_from
                                    .is_some_and(|jump| self.sequence_owner_matches(owner, jump))
                        }) {
                            return Err(invalid(format!(
                                "AC1009 SEQEND at {:#x} names owner {:?}, expected {:#x}",
                                record.address, record.sequence_owner, open.address
                            )));
                        }
                    }
                    _ => {
                        if sequence.is_some() {
                            return Err(invalid(format!(
                                "AC1009 compound entity interrupted by {kind} at {:#x}",
                                record.address
                            )));
                        }
                        if owner == Owner::Blocks && !in_block {
                            return Err(invalid(
                                "AC1009 block entity appears outside BLOCK/ENDBLK",
                            ));
                        }
                        if kind == "POLYLINE" || kind == "INSERT" && entity.integer(66, 0) != 0 {
                            sequence = Some(Sequence {
                                address: record.address,
                                jumped_from: reached_from,
                                child_kind: if kind == "POLYLINE" {
                                    "VERTEX"
                                } else {
                                    "ATTRIB"
                                },
                            });
                        }
                    }
                }
                let output = match owner {
                    Owner::Entities => &mut self.entities,
                    Owner::Blocks => &mut self.blocks,
                };
                output.push(entity.clone());
            }
            cursor = self.next(current);
        }
        if sequence.is_some() {
            return Err(invalid("AC1009 compound entity has no SEQEND"));
        }
        if in_block {
            return Err(invalid("AC1009 BLOCK has no ENDBLK"));
        }
        Ok(())
    }
}

/// Follow JUMPs before passing records to the DXF bridge. Unclaimed extra
/// drawables are independent entities; compound children must have a reachable owner.
pub(crate) fn reassemble_sections(
    entities: SectionRecords,
    blocks: SectionRecords,
    extras: SectionRecords,
) -> Result<(Vec<EntityRecord>, Vec<EntityRecord>)> {
    let sections = [entities, blocks, extras];
    for section in &sections {
        let mut expected = 0;
        for record in &section.records {
            if record.offset != expected
                || record.address
                    != section
                        .base_address
                        .checked_add(expected)
                        .ok_or_else(|| invalid("AC1009 record address overflow"))?
            {
                return Err(invalid("AC1009 positioned records are not contiguous"));
            }
            expected = expected
                .checked_add(u32::from(record.size))
                .ok_or_else(|| invalid("AC1009 section size overflow"))?;
        }
        if expected != section.size {
            return Err(invalid("AC1009 positioned records do not cover section"));
        }
    }
    let visited = std::array::from_fn(|index| vec![None; sections[index].records.len()]);
    let mut walker = Walker {
        sections,
        visited,
        entities: Vec::new(),
        blocks: Vec::new(),
    };
    for section in 0..3 {
        for index in 0..walker.sections[section].records.len() {
            if walker.visited[section][index].is_some() {
                continue;
            }
            let owner = if section == 1 {
                Owner::Blocks
            } else {
                Owner::Entities
            };
            walker.walk(Cursor { section, index }, owner)?;
        }
    }
    Ok((walker.entities, walker.blocks))
}

#[cfg(test)]
#[path = "traversal_tests.rs"]
mod tests;
