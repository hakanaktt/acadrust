//! Native pre-R13 (AC1009/R12) DWG support.

pub mod entities;
pub mod header;
pub(crate) mod tables;

use std::io::Cursor;

use crate::document::CadDocument;
use crate::error::{DxfError, Result};
use crate::io::dxf::{DxfReader, DxfWriter};
use crate::tables::TableEntry;
use crate::types::DxfVersion;

use self::entities::{decode_entities, encode_entities, EntityRecord, EntityTables};
use self::header::{
    read_header, write_auxheader, write_header, HeaderLayout, HeaderReferences, TableLocator,
    BLOCK_ENTITIES_BEGIN, BLOCK_ENTITIES_END, ENTITIES_BEGIN, ENTITIES_END, EXTRA_ENTITIES_BEGIN,
    EXTRA_ENTITIES_END,
};

fn section_records(
    data: &[u8],
    field_start: u32,
    content_size: u32,
    prelude: usize,
    begin: &[u8; 16],
    end: &[u8; 16],
    tables: &EntityTables,
    code_page: &str,
) -> Result<Vec<EntityRecord>> {
    if content_size == 0 {
        return Ok(Vec::new());
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
    if section.len() < begin_offset + 16 + 16
        || section.get(begin_offset..begin_offset + 16) != Some(begin.as_slice())
        || &section[section.len() - 16..] != end
    {
        return Err(DxfError::InvalidFormat(format!(
            "Invalid AC1009 section sentinels at field_start={field_start:#x}, content_size={content_size:#x}, prelude={prelude}, section_len={}",
            section.len()
        )));
    }
    decode_entities(
        &section[prelude..section.len() - 16],
        tables,
        field_start as u32,
        code_page,
    )
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

fn table_section_to_dxf(out: &mut String, records: &[Vec<EntityRecord>; 10]) {
    out.push_str("0\nSECTION\n2\nTABLES\n");
    for (index, rows) in records.iter().enumerate() {
        if rows.is_empty() {
            continue;
        }
        out.push_str("0\nTABLE\n2\n");
        out.push_str(crate::io::dwg::r12::header::TABLE_NAMES[index]);
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
) -> Vec<u8> {
    let mut out = String::from("0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1009\n0\nENDSEC\n");
    table_section_to_dxf(&mut out, tables);
    write_record_section(&mut out, "ENTITIES", entities);
    if !blocks.is_empty() {
        write_record_section(&mut out, "BLOCKS", blocks);
    }
    out.push_str("0\nEOF\n");
    out.into_bytes()
}

pub(crate) fn read_document(data: &[u8]) -> Result<CadDocument> {
    let header = read_header(data)?;
    let table_records =
        tables::decode_tables(data, &header.tables, header.variables.code_page.as_str())?;
    let tables = ensure_table_defaults(tables::entity_tables(&table_records));
    let code_page = header.variables.code_page.as_str();
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
        )?
    } else {
        Vec::new()
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
        )?
    } else {
        Vec::new()
    };
    let mut entities = entities;
    entities.extend(extras);
    let dxf = records_to_dxf(&entities, &blocks, &table_records);
    let mut document = DxfReader::from_reader(Cursor::new(dxf))?.read()?;
    document.version = DxfVersion::AC1009;
    document.maintenance_version = header.maintenance_version;
    document.header = header.variables;
    document.dwg_source_version = Some(DxfVersion::AC1009);
    Ok(document)
}

fn parse_dxf_records(data: &[u8]) -> (Vec<EntityRecord>, Vec<EntityRecord>) {
    let text = String::from_utf8_lossy(data);
    let lines: Vec<&str> = text.lines().map(str::trim_end).collect();
    let mut entities = Vec::new();
    let mut blocks = Vec::new();
    let mut section = "";
    let mut index = 0usize;
    let mut current: Option<(String, EntityRecord)> = None;
    let flush = |current: &mut Option<(String, EntityRecord)>,
                 entities: &mut Vec<EntityRecord>,
                 blocks: &mut Vec<EntityRecord>| {
        if let Some((section, record)) = current.take() {
            if section == "BLOCKS" {
                blocks.push(record);
            } else {
                entities.push(record);
            }
        }
    };
    while index + 1 < lines.len() {
        let code = lines[index].trim().parse::<i32>().unwrap_or(-1);
        let value = lines[index + 1].trim();
        index += 2;
        if code == 2
            && (value.eq_ignore_ascii_case("ENTITIES") || value.eq_ignore_ascii_case("BLOCKS"))
        {
            section = if value.eq_ignore_ascii_case("ENTITIES") {
                "ENTITIES"
            } else {
                "BLOCKS"
            };
            continue;
        }
        if code == 0 && value.eq_ignore_ascii_case("ENDSEC") {
            flush(&mut current, &mut entities, &mut blocks);
            section = "";
            continue;
        }
        if section != "ENTITIES" && section != "BLOCKS" {
            continue;
        }
        if code == 0 {
            flush(&mut current, &mut entities, &mut blocks);
            current = Some((section.to_owned(), EntityRecord::new(value)));
        } else if let Some((_, record)) = current.as_mut() {
            record.pairs.push((code, value.to_owned()));
        }
    }
    flush(&mut current, &mut entities, &mut blocks);
    (entities, blocks)
}

fn document_tables(document: &CadDocument) -> EntityTables {
    ensure_table_defaults(EntityTables {
        layers: document
            .layers
            .iter()
            .map(|entry| entry.name().to_owned())
            .collect(),
        linetypes: document
            .line_types
            .iter()
            .map(|entry| entry.name().to_owned())
            .collect(),
        styles: document
            .text_styles
            .iter()
            .map(|entry| entry.name().to_owned())
            .collect(),
        blocks: document
            .block_records
            .iter()
            .map(|entry| entry.name().to_owned())
            .collect(),
        dimstyles: document
            .dim_styles
            .iter()
            .map(|entry| entry.name().to_owned())
            .collect(),
        appids: document
            .app_ids
            .iter()
            .map(|entry| entry.name().to_owned())
            .collect(),
    })
}

fn document_table_records(document: &CadDocument) -> [Vec<EntityRecord>; 10] {
    let tables = document_tables(document);
    let mut records: [Vec<EntityRecord>; 10] = std::array::from_fn(|_| Vec::new());
    for name in tables.blocks {
        let mut record = EntityRecord::new("BLOCK_RECORD");
        record.pairs.push((2, name));
        records[0].push(record);
    }
    for name in tables.layers {
        let mut record = EntityRecord::new("LAYER");
        record.pairs.push((2, name));
        record.pairs.push((6, "CONTINUOUS".into()));
        record.pairs.push((62, "7".into()));
        records[1].push(record);
    }
    for name in tables.styles {
        let mut record = EntityRecord::new("STYLE");
        record.pairs.push((2, name));
        records[2].push(record);
    }
    for name in tables.linetypes {
        let mut record = EntityRecord::new("LTYPE");
        record.pairs.push((2, name));
        records[3].push(record);
    }
    for name in tables.appids {
        let mut record = EntityRecord::new("APPID");
        record.pairs.push((2, name));
        records[7].push(record);
    }
    for name in tables.dimstyles {
        let mut record = EntityRecord::new("DIMSTYLE");
        record.pairs.push((2, name));
        records[8].push(record);
    }
    records
}

pub(crate) fn write_document(document: &CadDocument) -> Result<Vec<u8>> {
    let dxf = DxfWriter::new(document).write_to_vec()?;
    let (entity_records, block_records) = parse_dxf_records(&dxf);
    let tables = document_tables(document);
    let encoded_entities = encode_entities(
        &entity_records,
        &tables,
        header::ENTITIES_START + 16,
        document.header.code_page.as_str(),
    )?;
    let entities_start = header::ENTITIES_START;
    let encoded_len = u32::try_from(encoded_entities.len()).map_err(|_| {
        DxfError::InvalidFormat("AC1009 entity section exceeds 32-bit offsets".into())
    })?;
    let entities_end = entities_start.checked_add(encoded_len).ok_or_else(|| {
        DxfError::InvalidFormat("AC1009 entity section exceeds 32-bit offsets".into())
    })?;
    let blocks_start = entities_end + 32;
    let encoded_blocks = encode_entities(
        &block_records,
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
    let table_records = document_table_records(document);
    let encoded_tables = tables::encode_tables(
        &table_records,
        &std::collections::HashMap::new(),
        document.header.code_page.as_str(),
    )?;
    let mut table_locators = [TableLocator::default(); 10];
    let mut table_bytes = Vec::new();
    let mut table_address = tables_start;
    for (index, (record_size, payload)) in encoded_tables.into_iter().enumerate() {
        if payload.is_empty() {
            continue;
        }
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
        &document.header,
        layout,
        &table_locators,
        HeaderReferences::default(),
        document.maintenance_version,
        None,
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
        document.header.handle_seed,
        aux_address,
    ));
    Ok(out)
}
