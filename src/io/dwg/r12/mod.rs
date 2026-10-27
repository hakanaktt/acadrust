//! Native pre-R13 (AC1009/R11-R12) DWG support.

pub mod entities;
pub mod header;
mod import_handles;
mod import_layouts;
mod metadata;
pub(crate) mod tables;
mod traversal;

pub(crate) type ImportFontResolver = dyn Fn(&str) -> String + Send + Sync;

#[cfg(test)]
mod code_page_tests;
#[cfg(test)]
mod import_handles_tests;

use std::collections::HashMap;
use std::io::{BufReader, Cursor};

use crate::document::CadDocument;
use crate::entities::EntityType;
use crate::error::{DxfError, Result};
use crate::io::dxf::{DxfReader, DxfStreamReader, DxfTextReader, DxfWriter};
use crate::tables::{TableEntry, VxTableRecord};
use crate::types::{DxfVersion, Handle};

use self::entities::{decode_positioned_entities, encode_entities, EntityRecord, EntityTables};
use self::header::{
    write_auxheader, write_header, HeaderLayout, HeaderReferences, TableLocator,
    BLOCK_ENTITIES_BEGIN, BLOCK_ENTITIES_END, ENTITIES_BEGIN, ENTITIES_END, EXTRA_ENTITIES_BEGIN,
    EXTRA_ENTITIES_END,
};
use self::traversal::{reassemble_sections, SectionRecords};

fn section_records(
    data: &[u8],
    field_start: u32,
    content_size: u32,
    prelude: usize,
    begin: &[u8; 16],
    end: &[u8; 16],
    tables: &EntityTables,
    code_page: &str,
    preceding_table: Option<TableLocator>,
) -> Result<SectionRecords> {
    if content_size == 0 {
        return Ok(SectionRecords {
            base_address: field_start,
            ..SectionRecords::default()
        });
    }
    let field_start = usize::try_from(field_start)
        .map_err(|_| DxfError::InvalidFormat("AC1009 section offset overflow".into()))?;
    let content_size = usize::try_from(content_size)
        .map_err(|_| DxfError::InvalidFormat("AC1009 section size overflow".into()))?;
    let start = field_start
        .checked_sub(prelude)
        .ok_or_else(|| DxfError::InvalidFormat("AC1009 section starts before file".into()))?;
    let size = content_size
        .checked_add(prelude + 16)
        .ok_or_else(|| DxfError::InvalidFormat("AC1009 section size overflow".into()))?;
    let section = data
        .get(start..start.saturating_add(size))
        .ok_or_else(|| DxfError::InvalidFormat("AC1009 section exceeds file bounds".into()))?;
    let begin_offset = prelude.saturating_sub(16);
    // BricsCAD places the validated VX payload between BLOCK's begin
    // sentinel and the first BLOCK record when paper-space viewports exist.
    let shared_begin = preceding_table.is_some_and(|locator| {
        locator
            .address
            .checked_add(u32::from(locator.record_size) * u32::from(locator.count))
            == Some(field_start as u32)
            && (locator.address as usize)
                .checked_sub(16)
                .is_some_and(|offset| data.get(offset..offset + 16) == Some(begin.as_slice()))
    });
    if section.len() < begin_offset + 16 + 16
        || (section.get(begin_offset..begin_offset + 16) != Some(begin.as_slice()) && !shared_begin)
        || &section[section.len() - 16..] != end
    {
        return Err(DxfError::InvalidFormat(format!(
            "Invalid AC1009 section sentinels at field_start={field_start:#x}, content_size={content_size:#x}, prelude={prelude}, section_len={}",
            section.len()
        )));
    }
    let records = decode_positioned_entities(
        &section[prelude..section.len() - 16],
        tables,
        field_start as u32,
        code_page,
    )?;
    Ok(SectionRecords {
        base_address: field_start as u32,
        size: content_size as u32,
        records,
    })
}

fn ensure_table_defaults(mut tables: EntityTables) -> EntityTables {
    if tables.layers.is_empty() {
        tables.layers.push("0".into());
    }
    if tables.linetypes.is_empty() {
        tables.linetypes.push("CONTINUOUS".into());
    }
    if tables.styles.is_empty() {
        tables.styles.push("STANDARD".into());
    }
    if tables.blocks.is_empty() {
        tables.blocks.push("*MODEL_SPACE".into());
    }
    if tables.dimstyles.is_empty() {
        tables.dimstyles.push("STANDARD".into());
    }
    if tables.appids.is_empty() {
        tables.appids.push("ACAD".into());
    }
    tables
}

fn write_record_section(out: &mut String, name: &str, records: &[EntityRecord]) {
    out.push_str("0\nSECTION\n2\n");
    out.push_str(name);
    out.push('\n');
    for record in records {
        out.push_str("0\n");
        out.push_str(&record.kind);
        out.push('\n');
        for (code, value) in &record.pairs {
            out.push_str(&code.to_string());
            out.push('\n');
            out.push_str(value);
            out.push('\n');
        }
    }
    out.push_str("0\nENDSEC\n");
}

fn table_section_to_dxf(
    out: &mut String,
    records: &[Vec<EntityRecord>; 10],
    handles: &import_handles::ImportHandles,
) {
    out.push_str("0\nSECTION\n2\nTABLES\n");
    for (index, rows) in records.iter().enumerate() {
        if rows.is_empty() {
            continue;
        }
        out.push_str("0\nTABLE\n2\n");
        out.push_str(if index == 0 {
            "BLOCK_RECORD"
        } else {
            crate::io::dwg::r12::header::TABLE_NAMES[index]
        });
        out.push_str(&format!("\n5\n{:X}", handles.control(index)));
        out.push_str("\n70\n");
        out.push_str(&rows.len().to_string());
        out.push('\n');
        for row in rows {
            out.push_str("0\n");
            out.push_str(if row.kind == "BLOCK_RECORD" {
                "BLOCK_RECORD"
            } else {
                &row.kind
            });
            out.push('\n');
            for (code, value) in &row.pairs {
                out.push_str(&code.to_string());
                out.push('\n');
                out.push_str(value);
                out.push('\n');
            }
        }
        out.push_str("0\nENDTAB\n");
    }
    out.push_str("0\nENDSEC\n");
}

fn records_to_dxf(
    entities: &[EntityRecord],
    blocks: &[EntityRecord],
    tables: &[Vec<EntityRecord>; 10],
    handles: &import_handles::ImportHandles,
) -> Vec<u8> {
    let mut out = String::from("0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1009\n0\nENDSEC\n");
    table_section_to_dxf(&mut out, tables, handles);
    if !blocks.is_empty() {
        write_record_section(&mut out, "BLOCKS", blocks);
    }
    write_record_section(&mut out, "ENTITIES", entities);
    out.push_str("0\nEOF\n");
    out.into_bytes()
}

fn assign_compound_owners(records: &mut [EntityRecord]) {
    let mut parent = None;
    for record in records {
        match record.kind.as_str() {
            "POLYLINE" => parent = Some(record.string(5).to_owned()),
            "INSERT" if record.integer(66, 0) != 0 => parent = Some(record.string(5).to_owned()),
            "VERTEX" | "ATTRIB" | "SEQEND" => {
                if let Some(owner) = &parent {
                    record.pairs.push((330, owner.clone()));
                }
                if record.kind == "SEQEND" {
                    parent = None;
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn read_document(
    data: &[u8],
    font_resolver: Option<&ImportFontResolver>,
    code_page_override: Option<&str>,
) -> Result<CadDocument> {
    let header = header::read_header_with_code_page(data, code_page_override)?;
    let code_page = code_page_override.unwrap_or(header.variables.code_page.as_str());
    let mut table_records = tables::decode_tables(data, &header.tables, code_page)?;
    let mut tables = ensure_table_defaults(tables::entity_tables(&table_records));
    // R11 dimension blocks share one table name (`*D`) while each BLOCK entity
    // carries the distinct name (`*D0`, `*D1`, …). Dimensions address the table
    // by index, so the shared name has to be replaced before those indexes are
    // turned into DXF names.
    if header.layout.blocks_size != 0 {
        let preview = section_records(
            data,
            header.layout.blocks_start,
            header.layout.blocks_size,
            16,
            &BLOCK_ENTITIES_BEGIN,
            &BLOCK_ENTITIES_END,
            &tables,
            code_page,
            Some(header.tables[9]),
        )?;
        adopt_distinct_block_names(&mut table_records[0], &preview, header.layout.blocks_start);
        tables = ensure_table_defaults(tables::entity_tables(&table_records));
    }
    let entities = section_records(
        data,
        header.layout.entities_start,
        header
            .layout
            .entities_end
            .saturating_sub(header.layout.entities_start),
        18,
        &ENTITIES_BEGIN,
        &ENTITIES_END,
        &tables,
        code_page,
        None,
    )?;
    let blocks = if header.layout.blocks_size != 0 {
        section_records(
            data,
            header.layout.blocks_start,
            header.layout.blocks_size,
            16,
            &BLOCK_ENTITIES_BEGIN,
            &BLOCK_ENTITIES_END,
            &tables,
            code_page,
            Some(header.tables[9]),
        )?
    } else {
        SectionRecords {
            base_address: header.layout.blocks_start,
            ..SectionRecords::default()
        }
    };
    let extras = if header.layout.extras_size != 0 {
        section_records(
            data,
            header.layout.extras_start,
            header.layout.extras_size,
            16,
            &EXTRA_ENTITIES_BEGIN,
            &EXTRA_ENTITIES_END,
            &tables,
            code_page,
            None,
        )?
    } else {
        SectionRecords {
            base_address: header.layout.extras_start,
            ..SectionRecords::default()
        }
    };
    let allocation_order: HashMap<_, _> = blocks
        .records
        .iter()
        .chain(entities.records.iter())
        .chain(extras.records.iter())
        .filter(|record| record.record.is_some() && record.jump_address.is_none())
        .enumerate()
        .map(|(index, record)| (record.address, index))
        .collect();
    let (mut entities, mut blocks) = reassemble_sections(entities, blocks, extras)?;
    for entity in &mut entities {
        let paper = entity.kind == "VIEWPORT"
            || entity
                .native_viewport
                .and_then(|index| table_records[9].get(usize::from(index)))
                .is_some_and(|vx| vx.integer(90, 65535) != 65535);
        if paper && !entity.has(67) {
            entity.pairs.push((67, "1".into()));
        }
    }
    for block in blocks.iter_mut().filter(|record| record.kind == "BLOCK") {
        if block.string(2).is_empty() {
            if let Some(table_record) = table_records[0].iter().find(|record| {
                let offset = record.integer(90, 0) as u32 & 0x3fff_ffff;
                header.layout.blocks_start.checked_add(offset) == block.native_address
            }) {
                block.pairs.push((2, table_record.string(2).to_owned()));
            }
        }
        if let Some(table_record) = table_records[0]
            .iter()
            .find(|record| record.string(2).eq_ignore_ascii_case(block.string(2)))
        {
            block
                .pairs
                .push((70, table_record.integer(70, 0).to_string()));
        }
    }
    let handles = import_handles::ImportHandles::assign(
        header.variables.handle_seed,
        &mut table_records,
        &mut blocks,
        &mut entities,
        font_resolver,
        Some(&allocation_order),
    )?;
    // The signed usage word is native to AC1009 table rows and is not emitted
    // by the DXF bridge.  Capture it against the synthetic table-row handles
    // assigned above so a later native save can restore it.
    let table_usage: HashMap<Handle, i16> = table_records
        .iter()
        .flat_map(|rows| rows.iter())
        .filter_map(|record| {
            let code = if record.kind == "DIMSTYLE" { 105 } else { 5 };
            let handle = u64::from_str_radix(record.string(code), 16).ok()?;
            record
                .native_table_usage
                .map(|usage| (Handle::new(handle), usage))
        })
        .collect();
    assign_compound_owners(&mut entities);
    assign_compound_owners(&mut blocks);
    let dxf = records_to_dxf(&entities, &blocks, &table_records, &handles);
    let mut document = DxfReader::from_reader(Cursor::new(dxf))?.read()?;
    handles.apply_layouts(&mut document)?;
    import_layouts::apply_defaults(&mut document);
    metadata::restore(&mut document, &entities, &blocks, &table_records[2])?;
    let layer_handles: HashMap<_, _> = document
        .layers
        .iter()
        .map(|layer| (crate::tables::normalize_name(&layer.name), layer.handle))
        .collect();
    for entity in document.entities_mut_keep_raw() {
        if let crate::entities::EntityType::Viewport(viewport) = entity {
            crate::io::dwg::legacy_viewport::restore_r12(viewport, |name| {
                layer_handles
                    .get(&crate::tables::normalize_name(name))
                    .copied()
            });
        }
    }
    document.version = DxfVersion::AC1009;
    document.maintenance_version = header.maintenance_version;
    header.apply_variables(&mut document.header, code_page_override)?;
    // R12 stores table indices in the header. Resolve them after importing
    // tables; the DXF bridge cannot infer these references from entity records.
    let table_name = |table: usize, index: i16| {
        usize::try_from(index)
            .ok()
            .and_then(|index| table_records[table].get(index))
            .map(|record| record.string(2))
    };
    if let Some(layer) =
        table_name(1, header.references.layer_index).and_then(|name| document.layers.get(name))
    {
        document.header.current_layer_handle = layer.handle;
    }
    if let Some(style) = table_name(2, header.references.text_style_index)
        .and_then(|name| document.text_styles.get(name))
    {
        document.header.current_text_style_handle = style.handle;
    }
    let linetype_name = match header.references.linetype_index {
        32767 | -1 => Some("ByLayer"),
        32766 | -2 => Some("ByBlock"),
        index => table_name(3, index),
    };
    if let Some(linetype) = linetype_name.and_then(|name| document.line_types.get(name)) {
        document.header.current_linetype_handle = linetype.handle;
    }
    if let Some(name) = table_name(5, header.references.ucs_index) {
        document.header.model_space_ucs_name = name.to_owned();
    }
    if let Some(name) = table_name(5, header.references.paper_ucs_index) {
        document.header.paper_space_ucs_name = name.to_owned();
    }
    // VX has no DXF table reader, so retain its native address/index fields here.
    document
        .vx_table
        .set_handle(Handle::new(handles.control(9)));
    for record in &table_records[9] {
        let mut vx = VxTableRecord::new(record.string(2));
        vx.handle = Handle::new(u64::from_str_radix(record.string(5), 16).unwrap_or(0));
        let flags = record.integer(70, 0);
        vx.is_on = flags & 1 != 0;
        vx.is_xref_dependent = flags & 16 != 0;
        vx.is_xref_resolved = flags & 32 != 0;
        vx.is_xref_reference = flags & 64 != 0;
        vx.legacy_viewport_entity_address = record.integer(90, 0) as u16;
        vx.legacy_viewport_index = record.integer(91, -1) as i16;
        vx.legacy_previous_entry_index = record.integer(92, -1) as i16;
        vx.viewport = entities
            .iter()
            .chain(blocks.iter())
            .find(|entity| {
                entity.kind == "VIEWPORT"
                    && entity.native_address == Some(u32::from(vx.legacy_viewport_entity_address))
            })
            .and_then(|entity| u64::from_str_radix(entity.string(5), 16).ok())
            .map(Handle::new)
            .unwrap_or(Handle::NULL);
        vx.previous_entry = usize::try_from(vx.legacy_previous_entry_index)
            .ok()
            .filter(|index| *index != 0)
            .and_then(|index| table_records[9].get(index))
            .and_then(|record| u64::from_str_radix(record.string(5), 16).ok())
            .map(Handle::new)
            .unwrap_or(Handle::NULL);
        if let Some(crate::entities::EntityType::Viewport(viewport)) =
            document.get_entity_mut(vx.viewport)
        {
            viewport.status.is_on = vx.is_on;
            viewport.dxf_status = Some(if vx.is_on { viewport.id } else { 0 });
        }
        document.vx_table.add_allow_duplicate(vx);
    }
    document.synchronize_handle_allocator();
    document.dwg_source_version = Some(DxfVersion::AC1009);
    document.r12_header_raw = Some(header.raw);
    document.r12_table_usage = table_usage;
    document.r12_block_tags = table_records[0]
        .iter()
        .filter_map(|record| {
            let handle = u64::from_str_radix(record.string(5), 16).ok()?;
            Some((Handle::new(handle), record.native_block_tag?))
        })
        .collect();
    Ok(document)
}

struct DxfRecords {
    entities: Vec<EntityRecord>,
    blocks: Vec<EntityRecord>,
    tables: [Vec<EntityRecord>; 10],
}

fn parse_dxf_records(data: &[u8], code_page: &str) -> Result<DxfRecords> {
    let mut reader = DxfTextReader::new(BufReader::new(Cursor::new(data)))?;
    if let Some(encoding) = crate::io::dxf::code_page::encoding_from_code_page(code_page) {
        reader.set_encoding(encoding);
    }
    let mut records = DxfRecords {
        entities: Vec::new(),
        blocks: Vec::new(),
        tables: std::array::from_fn(|_| Vec::new()),
    };
    let mut section = String::new();
    let mut section_name_pending = false;
    let mut current: Option<EntityRecord> = None;
    let flush = |current: &mut Option<EntityRecord>, section: &str, records: &mut DxfRecords| {
        if let Some(record) = current.take() {
            match section {
                "ENTITIES" => records.entities.push(record),
                "BLOCKS" => records.blocks.push(record),
                "TABLES" => {
                    let name = if record.kind == "BLOCK_RECORD" {
                        "BLOCK"
                    } else {
                        &record.kind
                    };
                    if let Some(index) = header::TABLE_NAMES.iter().position(|table| *table == name)
                    {
                        records.tables[index].push(record);
                    }
                }
                _ => {}
            }
        }
    };
    while let Some(pair) = reader.read_pair()? {
        let code = pair.code;
        let value = pair.value_string.as_str();
        if code == 0 && value == "SECTION" {
            flush(&mut current, &section, &mut records);
            section.clear();
            section_name_pending = true;
            continue;
        }
        if section_name_pending && code == 2 {
            section = value.to_owned();
            section_name_pending = false;
            continue;
        }
        if code == 0 && value.eq_ignore_ascii_case("ENDSEC") {
            flush(&mut current, &section, &mut records);
            section.clear();
            continue;
        }
        if !matches!(section.as_str(), "ENTITIES" | "BLOCKS" | "TABLES") {
            continue;
        }
        if code == 0 {
            flush(&mut current, &section, &mut records);
            if !matches!(value, "TABLE" | "ENDTAB") {
                current = Some(EntityRecord::new(value));
            }
        } else if let Some(record) = current.as_mut() {
            record.pairs.push((code, value.to_owned()));
        }
    }
    flush(&mut current, &section, &mut records);
    Ok(records)
}

fn adopt_distinct_block_names(
    rows: &mut [EntityRecord],
    blocks: &SectionRecords,
    blocks_start: u32,
) {
    // Only duplicated table names lose their index identity. A unique name
    // stays as stored so a differing BLOCK string cannot rename another block.
    let mut counts = std::collections::HashMap::<String, usize>::new();
    for row in rows.iter() {
        *counts
            .entry(row.string(2).to_ascii_uppercase())
            .or_default() += 1;
    }
    let mut claimed: std::collections::HashSet<String> = rows
        .iter()
        .map(|row| row.string(2).to_ascii_uppercase())
        .collect();
    for row in rows.iter_mut() {
        let current = row.string(2).to_ascii_uppercase();
        if counts.get(&current).copied().unwrap_or(0) < 2 {
            continue;
        }
        let offset = row.integer(90, 0) as u32 & 0x3fff_ffff;
        let Some(address) = blocks_start.checked_add(offset) else {
            continue;
        };
        let Some(name) = blocks.records.iter().find_map(|positioned| {
            let record = positioned.record.as_ref()?;
            if positioned.address != address || record.kind != "BLOCK" {
                return None;
            }
            let name = record.string(2);
            if name.is_empty() {
                None
            } else {
                Some(name.to_owned())
            }
        }) else {
            continue;
        };
        let claimed_name = name.to_ascii_uppercase();
        if claimed_name == current || claimed.contains(&claimed_name) {
            continue;
        }
        claimed.insert(claimed_name);
        row.pairs.retain(|(code, _)| *code != 2);
        row.pairs.push((2, name));
    }
}

fn records_reference_application(records: &DxfRecords, application: &str) -> bool {
    records
        .entities
        .iter()
        .chain(records.blocks.iter())
        .chain(records.tables.iter().flatten())
        .any(|record| {
            record
                .pairs
                .iter()
                .any(|(code, value)| *code == 1001 && value.eq_ignore_ascii_case(application))
        })
}

fn complete_table_records(document: &CadDocument, records: &mut DxfRecords) {
    // Restore native AC1009 usage words which have no DXF counterpart.
    for rows in &mut records.tables {
        for record in rows {
            let code = if record.kind == "DIMSTYLE" { 105 } else { 5 };
            if let Ok(handle) = u64::from_str_radix(record.string(code), 16) {
                record.native_table_usage =
                    document.r12_table_usage.get(&Handle::new(handle)).copied();
            }
        }
    }
    for record in &mut records.tables[0] {
        let Ok(handle) = u64::from_str_radix(record.string(5), 16) else {
            continue;
        };
        record.native_block_tag = document.r12_block_tags.get(&Handle::new(handle)).copied();
    }
    let system_space = |name: &str| {
        name.eq_ignore_ascii_case("*MODEL_SPACE") || name.eq_ignore_ascii_case("*PAPER_SPACE")
    };
    records.tables[0].retain(|record| !system_space(record.string(2)));
    records.tables[3].retain(|record| {
        !record.string(2).eq_ignore_ascii_case("BYLAYER")
            && !record.string(2).eq_ignore_ascii_case("BYBLOCK")
    });
    // `AcadAnnotative` is synthesized for newer documents. Keep it when the
    // drawing's extended data actually names the application; otherwise the
    // entity encoder cannot resolve that reference.
    let keep_annotative = records_reference_application(records, "ACADANNOTATIVE");
    records.tables[7].retain(|record| {
        keep_annotative || !record.string(2).eq_ignore_ascii_case("ACADANNOTATIVE")
    });
    let mut in_system_space = false;
    records.blocks.retain(|record| {
        if record.kind == "BLOCK" {
            in_system_space = system_space(record.string(2));
        }
        let retain = !in_system_space;
        if record.kind == "ENDBLK" {
            in_system_space = false;
        }
        retain
    });
    // DXF intentionally blanks shape-file STYLE names; native DWG does not.
    for (index, row) in records.tables[2].iter_mut().enumerate() {
        let handle = u64::from_str_radix(row.string(5), 16).unwrap_or(0);
        // A style added without a handle gets one only on the DXF writer's
        // output copy, so its row is found by position instead.
        let style = document
            .text_styles
            .iter()
            .find(|style| handle != 0 && style.handle.value() == handle)
            .or_else(|| document.text_styles.iter().nth(index));
        if let Some(style) = style {
            row.pairs.retain(|(code, _)| *code != 2);
            row.pairs.push((2, style.name.clone()));
        }
    }
    for row in &mut records.tables[0] {
        // BLOCK_RECORD group 70 is insertion units; R12 needs the BLOCK flags.
        let flags = records
            .blocks
            .iter()
            .find(|block| {
                block.kind == "BLOCK" && block.string(2).eq_ignore_ascii_case(row.string(2))
            })
            .map(|block| block.integer(70, 0))
            .unwrap_or(0);
        row.pairs.retain(|(code, _)| *code != 70);
        row.pairs.push((70, flags.to_string()));
    }
    for vx in document.vx_table.iter() {
        let mut record = EntityRecord::new("VX");
        let flags = i32::from(vx.is_on)
            | (i32::from(vx.is_xref_dependent) << 4)
            | (i32::from(vx.is_xref_resolved) << 5)
            | (i32::from(vx.is_xref_reference) << 6);
        record.pairs.extend([
            (2, vx.name.clone()),
            (70, flags.to_string()),
            (90, vx.legacy_viewport_entity_address.to_string()),
            (91, vx.legacy_viewport_index.to_string()),
            (92, vx.legacy_previous_entry_index.to_string()),
        ]);
        record.native_table_usage = document.r12_table_usage.get(&vx.handle).copied();
        records.tables[9].push(record);
    }
}

fn restore_table_usage(document: &CadDocument, records: &mut [Vec<EntityRecord>; 10]) {
    for (index, rows) in records.iter_mut().enumerate() {
        let handle_code = if index == 8 { 105 } else { 5 };
        for row in rows {
            let Ok(value) = u64::from_str_radix(row.string(handle_code), 16) else {
                continue;
            };
            if let Some(usage) = document.r12_table_usage.get(&Handle::new(value)) {
                row.native_table_usage = Some(*usage);
            }
        }
    }
}

fn prepare_compound_handles(document: &CadDocument, records: &mut [EntityRecord]) {
    let mut seqend = None;
    for record in records {
        if matches!(record.kind.as_str(), "INSERT" | "POLYLINE") {
            let handle = u64::from_str_radix(record.string(5), 16).unwrap_or(0);
            seqend = match document.get_entity(Handle::new(handle)) {
                Some(crate::entities::EntityType::Insert(insert)) => insert.seqend_handle,
                Some(crate::entities::EntityType::PolyfaceMesh(mesh)) => mesh.seqend_handle,
                _ => None,
            };
        } else if record.kind == "SEQEND" {
            if let Some(handle) = seqend.take().filter(|handle| handle.is_valid()) {
                record.pairs.retain(|(code, _)| *code != 5);
                record.pairs.push((5, format!("{:X}", handle.value())));
            }
        }
    }
}

fn prepare_viewport_xdata(document: &CadDocument, records: &mut [EntityRecord]) -> Result<()> {
    for record in records
        .iter_mut()
        .filter(|record| record.kind == "VIEWPORT")
    {
        let handle = u64::from_str_radix(record.string(5), 16).unwrap_or(0);
        if let Some(crate::entities::EntityType::Viewport(viewport)) =
            document.get_entity(Handle::new(handle))
        {
            let mut native = EntityRecord::new("VIEWPORT");
            let mut data = crate::xdata::ExtendedData::new();
            data.add_record(crate::io::dwg::legacy_viewport::record_r12(
                viewport, document,
            ));
            let mut buffer = Vec::new();
            let mut writer = crate::io::dxf::DxfTextWriter::new(&mut buffer);
            crate::io::dxf::SectionWriter::new(&mut writer, 0, 0).write_xdata(&data)?;
            // Parse a standard stream to retain all structured XDATA values.
            use crate::io::dxf::DxfStreamWriter;
            writer.flush()?;
            drop(writer);
            let mut reader = DxfTextReader::new(BufReader::new(Cursor::new(buffer)))?;
            while let Some(pair) = reader.read_pair()? {
                native.pairs.push((pair.code, pair.value_string));
            }
            let mut in_acad = false;
            record.pairs.retain(|(code, value)| {
                if *code == 1001 {
                    in_acad = value.eq_ignore_ascii_case("ACAD");
                }
                *code < 1000 || !in_acad
            });
            record.pairs.extend(native.pairs);
        }
    }
    Ok(())
}

fn block_offsets(records: &[EntityRecord], encoded: &[u8]) -> HashMap<String, u32> {
    let mut offsets = HashMap::new();
    let mut offset = 0;
    // R12 BLOCK table addresses are relative to the block entity section.
    for record in records {
        if record.kind == "BLOCK" {
            offsets.insert(record.string(2).to_owned(), offset as u32);
        }
        offset += u16::from_le_bytes(encoded[offset + 2..offset + 4].try_into().unwrap()) as usize;
    }
    offsets
}

fn entity_addresses(
    records: &[EntityRecord],
    encoded: &[u8],
    base: u32,
) -> Result<HashMap<Handle, u32>> {
    let mut result = HashMap::new();
    let mut offset = 0;
    for record in records {
        if let Ok(handle) = u64::from_str_radix(record.string(5), 16) {
            result.insert(
                Handle::new(handle),
                base.checked_add(offset as u32).ok_or_else(|| {
                    DxfError::InvalidFormat("AC1009 entity address overflow".into())
                })?,
            );
        }
        offset += u16::from_le_bytes(encoded[offset + 2..offset + 4].try_into().unwrap()) as usize;
    }
    Ok(result)
}

fn header_references(
    document: &CadDocument,
    records: &[Vec<EntityRecord>; 10],
) -> Result<HeaderReferences> {
    let index = |table: usize, name: &str, default: i16| {
        if name.is_empty() {
            return Ok(default);
        }
        let index = records[table]
            .iter()
            .position(|row| row.string(2).eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                DxfError::InvalidFormat(format!(
                    "AC1009 header references missing table entry {name:?}"
                ))
            })?;
        i16::try_from(index)
            .map_err(|_| DxfError::InvalidFormat("AC1009 header table index exceeds 32767".into()))
    };
    let layer = document
        .layers
        .iter()
        .find(|entry| entry.handle() == document.header.current_layer_handle)
        .map(TableEntry::name)
        .unwrap_or("0");
    let style = document
        .text_styles
        .iter()
        .find(|entry| entry.handle() == document.header.current_text_style_handle)
        .map(TableEntry::name)
        .unwrap_or("STANDARD");
    let linetype = document
        .line_types
        .iter()
        .find(|entry| entry.handle() == document.header.current_linetype_handle)
        .map(TableEntry::name)
        .unwrap_or("BYLAYER");
    Ok(HeaderReferences {
        layer_index: index(1, layer, 0)?,
        text_style_index: index(2, style, 0)?,
        linetype_index: tables::linetype_index(&tables::entity_tables(records), linetype)?,
        ucs_index: index(5, &document.header.model_space_ucs_name, -1)?,
        paper_ucs_index: index(5, &document.header.paper_space_ucs_name, -1)?,
    })
}

/// Whether the document contains an entity that needs the genuine DXF R12
/// down-save before it can be represented in the byte-aligned DWG stream.
/// Native AC1009 records are deliberately kept on the handle-preserving bridge
/// so ordinary legacy edits retain their identities.
fn needs_r12_downsave(document: &CadDocument) -> bool {
    document.entities().any(|entity| {
        !matches!(
            entity,
            EntityType::Point(_)
                | EntityType::Line(_)
                | EntityType::Circle(_)
                | EntityType::Arc(_)
                | EntityType::Text(_)
                | EntityType::Solid(_)
                | EntityType::Face3D(_)
                | EntityType::Insert(_)
                | EntityType::Block(_)
                | EntityType::BlockEnd(_)
                | EntityType::Viewport(_)
                | EntityType::AttributeDefinition(_)
                | EntityType::AttributeEntity(_)
                | EntityType::Dimension(_)
                | EntityType::Polyline(_)
                | EntityType::Polyline2D(_)
                | EntityType::Polyline3D(_)
                | EntityType::PolyfaceMesh(_)
                | EntityType::PolygonMesh(_)
                | EntityType::Shape(_)
                | EntityType::Seqend(_)
        )
    })
}

pub(crate) fn write_document(document: &CadDocument) -> Result<Vec<u8>> {
    // The normal bridge keeps handles and native entity records intact.  Some
    // modern entities have no AC1009 representation, but the genuine DXF R12
    // writer can down-save many of them to supported primitives (for example
    // MTEXT to TEXT and ELLIPSE to polylines).  Use that conversion only when
    // the lossless bridge rejects the document; this keeps native R11/R12
    // round-trips handle-stable while making explicit down-saves useful.
    let dxf = if needs_r12_downsave(document) {
        DxfWriter::new(document)
            .with_version(DxfVersion::AC1009)
            .write_to_vec()?
    } else {
        match DxfWriter::new(document).write_to_vec() {
            Ok(bytes) => bytes,
            Err(DxfError::UnsupportedVersion(_)) => DxfWriter::new(document)
                .with_version(DxfVersion::AC1009)
                .write_to_vec()?,
            Err(error) => return Err(error),
        }
    };
    let mut records = parse_dxf_records(&dxf, &document.header.code_page)?;
    complete_table_records(document, &mut records);
    restore_table_usage(document, &mut records.tables);
    metadata::prepare(document, &mut records.entities, &records.tables[2])?;
    metadata::prepare(document, &mut records.blocks, &records.tables[2])?;
    prepare_compound_handles(document, &mut records.entities);
    prepare_compound_handles(document, &mut records.blocks);
    prepare_viewport_xdata(document, &mut records.entities)?;
    prepare_viewport_xdata(document, &mut records.blocks)?;
    let mut variables = document.header.clone();
    for record in records.entities.iter().chain(records.blocks.iter()) {
        if let Ok(handle) = u64::from_str_radix(record.string(5), 16) {
            let next = handle
                .checked_add(1)
                .ok_or_else(|| DxfError::InvalidFormat("AC1009 stored handle overflow".into()))?;
            variables.handle_seed = variables.handle_seed.max(next);
        }
    }
    let tables = ensure_table_defaults(tables::entity_tables(&records.tables));
    let entities_start = if document
        .r12_header_raw
        .as_deref()
        .is_some_and(|raw| raw.len() == 0x6bb)
    {
        header::R11_ENTITIES_START
    } else {
        header::ENTITIES_START
    };
    let encoded_entities = encode_entities(
        &records.entities,
        &tables,
        entities_start,
        document.header.code_page.as_str(),
    )?;
    let encoded_len = u32::try_from(encoded_entities.len()).map_err(|_| {
        DxfError::InvalidFormat("AC1009 entity section exceeds 32-bit offsets".into())
    })?;
    let entities_end = entities_start.checked_add(encoded_len).ok_or_else(|| {
        DxfError::InvalidFormat("AC1009 entity section exceeds 32-bit offsets".into())
    })?;
    let blocks_start = entities_end + 32;
    let encoded_blocks = encode_entities(
        &records.blocks,
        &tables,
        blocks_start,
        document.header.code_page.as_str(),
    )?;
    let encoded_blocks_len = u32::try_from(encoded_blocks.len()).map_err(|_| {
        DxfError::InvalidFormat("AC1009 block section exceeds 32-bit offsets".into())
    })?;
    let extras_start = blocks_start
        .checked_add(encoded_blocks_len)
        .and_then(|value| value.checked_add(32))
        .ok_or_else(|| {
            DxfError::InvalidFormat("AC1009 block section exceeds 32-bit offsets".into())
        })?;
    // The table stream follows both the extra-entity begin and end sentinels.
    let tables_start = extras_start + 16;
    let mut addresses = entity_addresses(&records.entities, &encoded_entities, entities_start)?;
    addresses.extend(entity_addresses(
        &records.blocks,
        &encoded_blocks,
        blocks_start,
    )?);
    for (row, vx) in records.tables[9].iter_mut().zip(document.vx_table.iter()) {
        if vx.viewport.is_valid() {
            let address = addresses.get(&vx.viewport).ok_or_else(|| {
                DxfError::InvalidFormat("AC1009 VX references a missing viewport entity".into())
            })?;
            let address = u16::try_from(*address).map_err(|_| {
                DxfError::InvalidFormat("AC1009 VX viewport address exceeds 65535".into())
            })?;
            row.pairs.retain(|(code, _)| *code != 90);
            row.pairs.push((90, address.to_string()));
        }
    }
    let table_records = &records.tables;
    let encoded_tables = tables::encode_tables(
        table_records,
        &block_offsets(&records.blocks, &encoded_blocks),
        document.header.code_page.as_str(),
    )?;
    let mut table_locators = [TableLocator::default(); 10];
    let mut table_bytes = Vec::new();
    let mut table_address = tables_start;
    for (index, (record_size, payload)) in encoded_tables.into_iter().enumerate() {
        table_bytes.extend_from_slice(&header::TABLE_BEGIN[index]);
        table_address = table_address.checked_add(16).ok_or_else(|| {
            DxfError::InvalidFormat("AC1009 table section exceeds 32-bit offsets".into())
        })?;
        table_locators[index] = TableLocator {
            record_size,
            count: table_records[index].len().min(u16::MAX as usize) as u16,
            flags: 0,
            address: table_address,
        };
        table_bytes.extend_from_slice(&payload);
        table_address = table_address
            .checked_add(u32::try_from(payload.len()).map_err(|_| {
                DxfError::InvalidFormat("AC1009 table section exceeds 32-bit offsets".into())
            })?)
            .ok_or_else(|| {
                DxfError::InvalidFormat("AC1009 table section exceeds 32-bit offsets".into())
            })?;
        table_bytes.extend_from_slice(&tables::TABLE_END[index]);
        table_address = table_address.checked_add(16).ok_or_else(|| {
            DxfError::InvalidFormat("AC1009 table section exceeds 32-bit offsets".into())
        })?;
    }
    let aux_address = tables_start
        .checked_add(u32::try_from(table_bytes.len()).map_err(|_| {
            DxfError::InvalidFormat("AC1009 table section exceeds 32-bit offsets".into())
        })?)
        .ok_or_else(|| {
            DxfError::InvalidFormat("AC1009 table section exceeds 32-bit offsets".into())
        })?;
    let layout = HeaderLayout {
        entities_start,
        entities_end,
        blocks_start,
        blocks_size: encoded_blocks_len,
        extras_start,
        extras_size: 0,
    };
    let header = write_header(
        &variables,
        layout,
        &table_locators,
        header_references(document, table_records)?,
        document.maintenance_version,
        document.r12_header_raw.as_deref(),
    )?;
    let mut out = header;
    out.extend_from_slice(&encoded_entities);
    out.extend_from_slice(&ENTITIES_END);
    out.extend_from_slice(&BLOCK_ENTITIES_BEGIN);
    out.extend_from_slice(&encoded_blocks);
    out.extend_from_slice(&BLOCK_ENTITIES_END);
    out.extend_from_slice(&EXTRA_ENTITIES_BEGIN);
    out.extend_from_slice(&EXTRA_ENTITIES_END);
    out.extend_from_slice(&table_bytes);
    out.extend_from_slice(&write_auxheader(
        layout,
        &table_locators,
        variables.handle_seed,
        aux_address,
    ));
    Ok(out)
}
