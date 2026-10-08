//! Fixed-size, byte-aligned symbol-table records in AC1009 drawings.

use std::collections::HashMap;

use crate::error::{DxfError, Result};
use crate::io::dwg::crc::crc16;
use crate::io::dxf::code_page::{
    decode_cif_escapes, encode_legacy_string, encoding_from_code_page,
};

use super::entities::{EntityRecord, EntityTables};
use super::header::{TableLocator, TABLE_BEGIN, TABLE_NAMES};

pub(crate) const RECORD_SIZES: [u16; 10] = [45, 41, 198, 191, 153, 109, 253, 37, 324, 43];
pub(crate) const TABLE_END: [[u8; 16]; 10] = [
    [
        0x24, 0x10, 0x4c, 0x0f, 0x38, 0xc1, 0x92, 0x59, 0x36, 0x49, 0xdb, 0xa3, 0xb3, 0x90, 0xcd,
        0x34,
    ],
    [
        0xf1, 0x3b, 0x9b, 0x90, 0x44, 0xe2, 0x2c, 0x74, 0xff, 0xb6, 0x3d, 0x10, 0xe7, 0x15, 0x90,
        0x04,
    ],
    [
        0x1d, 0xc1, 0x3e, 0x7d, 0xbc, 0x60, 0x9e, 0x88, 0xaf, 0x54, 0x38, 0x99, 0x69, 0xff, 0xf9,
        0xe7,
    ],
    [
        0x53, 0x6f, 0xe5, 0x35, 0xe3, 0x42, 0x6a, 0xea, 0xe9, 0xe9, 0xb3, 0xeb, 0x31, 0xe7, 0x77,
        0x50,
    ],
    [
        0x3e, 0xc3, 0x55, 0xa9, 0x97, 0x0b, 0x4b, 0xe1, 0xb4, 0x8b, 0x0b, 0xf7, 0xbd, 0xb2, 0x40,
        0x5a,
    ],
    [
        0x9f, 0xb5, 0x05, 0xc2, 0x7b, 0x6f, 0x33, 0xa4, 0x10, 0x18, 0x29, 0x5a, 0x80, 0xe1, 0x9e,
        0x32,
    ],
    [
        0x09, 0x12, 0xbb, 0x9e, 0xd5, 0x23, 0x1b, 0x84, 0xb1, 0x46, 0xd4, 0x44, 0x99, 0x9f, 0x9c,
        0x72,
    ],
    [
        0x1e, 0xda, 0x3d, 0xaf, 0xc9, 0x97, 0x93, 0xf3, 0xc4, 0x2c, 0xa2, 0xa9, 0x3e, 0x86, 0xe3,
        0xc5,
    ],
    [
        0x4b, 0xe7, 0xc1, 0xbd, 0x36, 0x60, 0x00, 0x1a, 0x49, 0x1d, 0x34, 0x4c, 0x8a, 0x3c, 0x3c,
        0x4f,
    ],
    [
        0x1f, 0x35, 0xc9, 0x83, 0x11, 0x8a, 0x79, 0x0d, 0x48, 0x28, 0x8b, 0xaa, 0xfa, 0x0e, 0xbb,
        0x80,
    ],
];

fn invalid(message: impl Into<String>) -> DxfError {
    DxfError::InvalidFormat(message.into())
}

fn put(record: &mut EntityRecord, code: i32, value: impl ToString) {
    record.pairs.push((code, value.to_string()));
}

fn table_index(name: &str) -> Result<usize> {
    TABLE_NAMES
        .iter()
        .position(|entry| entry.eq_ignore_ascii_case(name))
        .ok_or_else(|| invalid(format!("Unknown AC1009 table {name:?}")))
}

pub(crate) fn entity_tables(records: &[Vec<EntityRecord>; 10]) -> EntityTables {
    let names = |index: usize| {
        records[index]
            .iter()
            .map(|record| record.string(2).to_owned())
            .collect()
    };
    EntityTables {
        blocks: names(0),
        layers: names(1),
        styles: names(2),
        linetypes: names(3),
        appids: names(7),
        dimstyles: names(8),
    }
}

/// Decode table payloads, validating the sentinels and CRCs before resolving indices.
pub(crate) fn decode_tables(
    data: &[u8],
    locators: &[TableLocator; 10],
    code_page: &str,
) -> Result<[Vec<EntityRecord>; 10]> {
    let mut raw_names: [Vec<EntityRecord>; 10] = std::array::from_fn(|_| Vec::new());
    for (index, locator) in locators.iter().enumerate() {
        let payload = table_payload(data, *locator, index)?;
        let size = usize::from(locator.record_size);
        if locator.count != 0 && size < 37 {
            return Err(invalid(format!(
                "Invalid AC1009 {} record size",
                TABLE_NAMES[index]
            )));
        }
        for raw in payload.chunks_exact(size.max(1)) {
            let mut reader = Reader::new(raw, code_page);
            reader.byte()?;
            let mut record = EntityRecord::new(TABLE_NAMES[index]);
            put(&mut record, 2, reader.text(32)?);
            raw_names[index].push(record);
        }
    }
    let tables = entity_tables(&raw_names);
    let mut decoded: [Vec<EntityRecord>; 10] = std::array::from_fn(|_| Vec::new());
    for (index, locator) in locators.iter().enumerate() {
        decoded[index] = decode_table(
            TABLE_NAMES[index],
            table_payload(data, *locator, index)?,
            locator.record_size,
            locator.count,
            &tables,
            code_page,
        )?;
    }
    Ok(decoded)
}

fn table_payload(data: &[u8], locator: TableLocator, index: usize) -> Result<&[u8]> {
    if locator.count == 0 {
        return Ok(&[]);
    }
    let start = locator.address as usize;
    let size = usize::from(locator.record_size)
        .checked_mul(usize::from(locator.count))
        .ok_or_else(|| invalid("AC1009 table size overflow"))?;
    let end = start
        .checked_add(size)
        .ok_or_else(|| invalid("AC1009 table offset overflow"))?;
    let begin = start
        .checked_sub(16)
        .ok_or_else(|| invalid("AC1009 table begins before file"))?;
    let end_sentinel = end
        .checked_add(16)
        .ok_or_else(|| invalid("AC1009 table offset overflow"))?;
    // BricsCAD and several R12 writers emit table sentinels in a shared
    // stream whose order is independent of the address order in the header.
    // Validate them when they directly surround this payload, while still
    // accepting valid records whose sentinels are stored elsewhere in that
    // stream. Record CRCs below remain mandatory.
    let _sentinels_match = data.get(begin..start) == Some(TABLE_BEGIN[index].as_slice())
        && data.get(end..end_sentinel) == Some(TABLE_END[index].as_slice());
    data.get(start..end)
        .ok_or_else(|| invalid("AC1009 table exceeds file bounds"))
}

/// Encode each table's payload, excluding its begin/end sentinels.
pub(crate) fn encode_tables(
    records: &[Vec<EntityRecord>; 10],
    block_offsets: &HashMap<String, u32>,
    code_page: &str,
) -> Result<[(u16, Vec<u8>); 10]> {
    let tables = entity_tables(records);
    let mut encoded = Vec::with_capacity(10);
    for (name, rows) in TABLE_NAMES.into_iter().zip(records) {
        encoded.push(encode_table(name, rows, &tables, block_offsets, code_page)?);
    }
    encoded
        .try_into()
        .map_err(|_| invalid("AC1009 table array length mismatch"))
}

pub(crate) fn encode_table(
    name: &str,
    records: &[EntityRecord],
    tables: &EntityTables,
    block_offsets: &HashMap<String, u32>,
    code_page: &str,
) -> Result<(u16, Vec<u8>)> {
    let index = table_index(name)?;
    let name = TABLE_NAMES[index];
    if records.len() > i16::MAX as usize {
        return Err(invalid(format!(
            "AC1009 {name} table exceeds 32767 records"
        )));
    }
    let record_size = RECORD_SIZES[index];
    let mut payload = Vec::with_capacity(records.len() * usize::from(record_size));
    for (record_index, record) in records.iter().enumerate() {
        let mut writer = Writer::new(code_page);
        let mut flag = checked_byte(record.integer(70, 0), "table flags")?;
        if name == "STYLE" {
            flag = swap_style_flags(flag);
        }
        writer.byte(flag);
        writer.text(record.string(2), 32)?;
        writer.short(-1);
        encode_fields(
            &mut writer,
            name,
            record,
            record_index,
            tables,
            block_offsets,
        )?;
        if writer.data.len() + 2 != usize::from(record_size) {
            return Err(invalid(format!(
                "AC1009 {name} layout does not match record size (got {}, expected {})",
                writer.data.len() + 2,
                record_size
            )));
        }
        let checksum = crc16(0xc0c1, &writer.data);
        writer.ushort(checksum);
        payload.extend(writer.data);
    }
    Ok((record_size, payload))
}

pub(crate) fn decode_table(
    name: &str,
    data: &[u8],
    record_size: u16,
    count: u16,
    tables: &EntityTables,
    code_page: &str,
) -> Result<Vec<EntityRecord>> {
    let index = table_index(name)?;
    let name = TABLE_NAMES[index];
    if count == 0 {
        return Ok(Vec::new());
    }
    let size = usize::from(record_size);
    if size != usize::from(RECORD_SIZES[index]) {
        return Err(invalid(format!(
            "Unsupported AC1009 {name} record size {record_size}"
        )));
    }
    let expected_len = usize::from(count)
        .checked_mul(size)
        .ok_or_else(|| invalid("AC1009 table size overflow"))?;
    if data.len() != expected_len {
        return Err(invalid(format!("AC1009 {name} table length mismatch")));
    }
    let mut records = Vec::with_capacity(usize::from(count));
    for (record_index, raw) in data.chunks_exact(size).enumerate() {
        let expected = u16::from_le_bytes(raw[size - 2..].try_into().unwrap());
        if crc16(0xc0c1, &raw[..size - 2]) != expected {
            return Err(invalid(format!(
                "AC1009 {name} record {record_index} CRC mismatch"
            )));
        }
        let mut reader = Reader::new(&raw[..size - 2], code_page);
        let mut record = EntityRecord::new(if name == "BLOCK" {
            "BLOCK_RECORD"
        } else {
            name
        });
        let mut flag = reader.byte()?;
        if name == "STYLE" {
            flag = swap_style_flags(flag);
        }
        put(&mut record, 2, reader.text(32)?);
        put(&mut record, 70, flag);
        reader.short()?; // usage counter does not have a DXF counterpart
        decode_fields(&mut reader, name, &mut record, tables)?;
        if reader.pos != size - 2 {
            return Err(invalid(format!("AC1009 {name} record layout mismatch")));
        }
        records.push(record);
    }
    Ok(records)
}

fn swap_style_flags(flag: u8) -> u8 {
    (flag & !5) | ((flag & 1) << 2) | ((flag & 4) >> 2)
}

fn linetype_index(tables: &EntityTables, name: &str) -> Result<i16> {
    if name.eq_ignore_ascii_case("BYLAYER") {
        return Ok(32767);
    }
    if name.eq_ignore_ascii_case("BYBLOCK") {
        return Ok(32766);
    }
    if name.is_empty() {
        return Ok(0);
    }
    let index = tables
        .linetypes
        .iter()
        .position(|entry| entry.eq_ignore_ascii_case(name))
        .ok_or_else(|| invalid(format!("AC1009 layer references missing linetype {name:?}")))?;
    i16::try_from(index).map_err(|_| invalid("AC1009 linetype index exceeds 32767"))
}

fn linetype_name(tables: &EntityTables, index: i16) -> Result<&str> {
    match index {
        32767 | -1 => Ok("BYLAYER"),
        32766 | -2 => Ok("BYBLOCK"),
        _ => usize::try_from(index)
            .ok()
            .and_then(|index| tables.linetypes.get(index))
            .map(String::as_str)
            .ok_or_else(|| invalid(format!("Invalid AC1009 linetype index {index}"))),
    }
}

const DIM_DOUBLES: [(i32, f64); 15] = [
    (40, 1.0),
    (41, 0.18),
    (42, 0.0625),
    (43, 0.38),
    (44, 0.18),
    (45, 0.0),
    (46, 0.0),
    (47, 0.0),
    (48, 0.0),
    (140, 0.18),
    (141, 0.09),
    (142, 0.0),
    (143, 25.4),
    (144, 1.0),
    (145, 0.0),
];
const DIM_BYTES: [(i32, i32); 14] = [
    (71, 0),
    (72, 0),
    (73, 1),
    (74, 1),
    (75, 0),
    (76, 0),
    (77, 0),
    (78, 0),
    (170, 0),
    (171, 2),
    (172, 0),
    (173, 0),
    (174, 0),
    (175, 0),
];
const DIM_STRINGS: [(i32, usize); 5] = [(3, 16), (4, 16), (5, 16), (6, 16), (7, 66)];

fn encode_fields(
    writer: &mut Writer,
    name: &str,
    record: &EntityRecord,
    record_index: usize,
    tables: &EntityTables,
    block_offsets: &HashMap<String, u32>,
) -> Result<()> {
    match name {
        "BLOCK" => {
            let offset = block_offsets
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(record.string(2)))
                .map(|(_, offset)| *offset)
                .unwrap_or_else(|| record.integer(90, 0) as u32);
            if offset & 0x80000000 != 0 {
                return Err(invalid(
                    "AC1009 block offset exceeds 30-bit section address",
                ));
            }
            writer.long(offset | 0x40000000);
            writer.short(checked_short(
                record.integer(91, record_index as i32),
                "block entity index",
            )?);
            writer.short(checked_short(
                record.integer(92, 0),
                "block secondary flags",
            )?);
        }
        "LAYER" => {
            writer.short(checked_short(record.integer(62, 7), "layer color")?);
            writer.short(linetype_index(tables, record.string(6))?);
        }
        "STYLE" => {
            writer.number(record, 40, 0.0)?;
            writer.number(record, 41, 1.0)?;
            writer.angle(record, 50)?;
            writer.byte(checked_byte(
                record.integer(71, 0),
                "text generation flags",
            )?);
            writer.number(record, 42, 0.2)?;
            writer.text(
                if record.string(3).is_empty() {
                    "txt"
                } else {
                    record.string(3)
                },
                64,
            )?;
            writer.text(record.string(4), 64)?;
        }
        "LTYPE" => {
            writer.text(record.string(3), 48)?;
            writer.byte(checked_byte(record.integer(72, 65), "linetype alignment")?);
            let dashes: Vec<f64> = record
                .pairs
                .iter()
                .filter(|(code, _)| *code == 49)
                .map(|(_, value)| {
                    value
                        .parse::<f64>()
                        .map_err(|_| invalid("Invalid AC1009 linetype dash"))
                })
                .collect::<Result<_>>()?;
            if dashes.len() > 12 {
                return Err(invalid("AC1009 linetypes support at most 12 dashes"));
            }
            if record
                .pairs
                .iter()
                .any(|(code, value)| *code == 74 && value.trim() != "0")
            {
                return Err(DxfError::UnsupportedVersion(
                    "Complex linetypes are unavailable in R12 DWG".into(),
                ));
            }
            writer.byte(dashes.len() as u8);
            writer.number(record, 40, dashes.iter().map(|dash| dash.abs()).sum())?;
            for dash_index in 0..12 {
                writer.double(*dashes.get(dash_index).unwrap_or(&0.0))?;
            }
        }
        "VIEW" => {
            writer.number(record, 40, 1.0)?;
            writer.point(record, 10, &[0.0, 0.0])?;
            writer.number(record, 41, 1.0)?;
            writer.point(record, 11, &[0.0, 0.0, 1.0])?;
            writer.short(checked_short(record.integer(90, 0), "view 3D flags")?);
            writer.point(record, 12, &[0.0, 0.0, 0.0])?;
            writer.short(checked_short(record.integer(71, 0), "view mode")?);
            for (code, default) in [(42, 50.0), (43, 0.0), (44, 0.0)] {
                writer.number(record, code, default)?;
            }
            writer.angle(record, 50)?;
        }
        "UCS" => {
            writer.point(record, 10, &[0.0, 0.0, 0.0])?;
            writer.point(record, 11, &[1.0, 0.0, 0.0])?;
            writer.point(record, 12, &[0.0, 1.0, 0.0])?;
        }
        "VPORT" => {
            writer.point(record, 10, &[0.0, 0.0])?;
            writer.point(record, 11, &[1.0, 1.0])?;
            writer.point(record, 17, &[0.0, 0.0, 0.0])?;
            writer.point(record, 16, &[0.0, 0.0, 1.0])?;
            writer.angle(record, 51)?;
            writer.number(record, 40, 1.0)?;
            writer.point(record, 12, &[0.0, 0.0])?;
            for (code, default) in [(41, 1.0), (42, 50.0), (43, 0.0), (44, 0.0)] {
                writer.number(record, code, default)?;
            }
            for (code, default) in [
                (71, 0),
                (72, 100),
                (73, 1),
                (74, 3),
                (75, 0),
                (76, 0),
                (77, 0),
                (78, 0),
            ] {
                let mut value = record.integer(code, default);
                if code == 74 {
                    value = swap_ucs_icon(value);
                }
                writer.short(checked_short(value, "viewport setting")?);
            }
            writer.angle(record, 50)?;
            writer.point(record, 13, &[0.0, 0.0])?;
            writer.point(record, 14, &[0.5, 0.5])?;
            writer.point(record, 15, &[0.5, 0.5])?;
        }
        "APPID" => {}
        "DIMSTYLE" => {
            for (code, default) in DIM_DOUBLES {
                writer.number(record, code, default)?;
            }
            for (code, default) in DIM_BYTES {
                writer.byte(checked_byte(
                    record.integer(code, default),
                    "dimension setting",
                )?);
            }
            for (code, size) in DIM_STRINGS {
                writer.text(record.string(code), size)?;
            }
            for code in 176..=178 {
                writer.short(checked_short(record.integer(code, 0), "dimension color")?);
            }
            writer.byte(checked_byte(
                record.integer(288, 0),
                "dimension cursor placement",
            )?);
            writer.number(record, 146, 1.0)?;
            writer.number(record, 147, 0.09)?;
        }
        "VX" => {
            writer.ushort(
                u16::try_from(record.integer(90, 0))
                    .map_err(|_| invalid("Invalid AC1009 viewport entity address"))?,
            );
            writer.short(checked_short(record.integer(91, -1), "viewport index")?);
            writer.short(checked_short(
                record.integer(92, -1),
                "previous viewport index",
            )?);
        }
        _ => return Err(invalid("Unknown AC1009 table layout")),
    }
    Ok(())
}

fn decode_fields(
    reader: &mut Reader<'_>,
    name: &str,
    record: &mut EntityRecord,
    tables: &EntityTables,
) -> Result<()> {
    match name {
        "BLOCK" => {
            put(record, 90, reader.long()?);
            put(record, 91, reader.short()?);
            put(record, 92, reader.short()?);
        }
        "LAYER" => {
            put(record, 62, reader.short()?);
            put(record, 6, linetype_name(tables, reader.short()?)?);
        }
        "STYLE" => {
            reader.number(record, 40)?;
            reader.number(record, 41)?;
            reader.angle(record, 50)?;
            put(record, 71, reader.byte()?);
            reader.number(record, 42)?;
            put(record, 3, reader.text(64)?);
            put(record, 4, reader.text(64)?);
        }
        "LTYPE" => {
            put(record, 3, reader.text(48)?);
            put(record, 72, reader.byte()?);
            let dash_count = reader.byte()?;
            if dash_count > 12 {
                return Err(invalid("Invalid AC1009 linetype dash count"));
            }
            put(record, 73, dash_count);
            reader.number(record, 40)?;
            for index in 0..12 {
                let dash = reader.double()?;
                if index < dash_count {
                    put(record, 49, dash);
                }
            }
        }
        "VIEW" => {
            reader.number(record, 40)?;
            reader.point(record, 10, 2)?;
            reader.number(record, 41)?;
            reader.point(record, 11, 3)?;
            put(record, 90, reader.short()?);
            reader.point(record, 12, 3)?;
            put(record, 71, reader.short()?);
            for code in [42, 43, 44] {
                reader.number(record, code)?;
            }
            reader.angle(record, 50)?;
        }
        "UCS" => {
            for code in [10, 11, 12] {
                reader.point(record, code, 3)?;
            }
        }
        "VPORT" => {
            reader.point(record, 10, 2)?;
            reader.point(record, 11, 2)?;
            reader.point(record, 17, 3)?;
            reader.point(record, 16, 3)?;
            reader.angle(record, 51)?;
            reader.number(record, 40)?;
            reader.point(record, 12, 2)?;
            for code in [41, 42, 43, 44] {
                reader.number(record, code)?;
            }
            for code in 71..=78 {
                let mut value = i32::from(reader.short()?);
                if code == 74 {
                    value = swap_ucs_icon(value);
                }
                put(record, code, value);
            }
            reader.angle(record, 50)?;
            for code in [13, 14, 15] {
                reader.point(record, code, 2)?;
            }
        }
        "APPID" => {}
        "DIMSTYLE" => {
            for (code, _) in DIM_DOUBLES {
                reader.number(record, code)?;
            }
            for (code, _) in DIM_BYTES {
                put(record, code, reader.byte()?);
            }
            for (code, size) in DIM_STRINGS {
                put(record, code, reader.text(size)?);
            }
            for code in 176..=178 {
                put(record, code, reader.short()?);
            }
            put(record, 288, reader.byte()?);
            reader.number(record, 146)?;
            reader.number(record, 147)?;
        }
        "VX" => {
            put(record, 90, reader.ushort()?);
            put(record, 91, reader.short()?);
            put(record, 92, reader.short()?);
        }
        _ => return Err(invalid("Unknown AC1009 table layout")),
    }
    Ok(())
}

fn swap_ucs_icon(value: i32) -> i32 {
    match value {
        1 => 2,
        2 => 1,
        _ => value,
    }
}

fn checked_byte(value: i32, field: &str) -> Result<u8> {
    u8::try_from(value).map_err(|_| invalid(format!("AC1009 {field} exceeds byte range")))
}

fn checked_short(value: i32, field: &str) -> Result<i16> {
    i16::try_from(value).map_err(|_| invalid(format!("AC1009 {field} exceeds signed-short range")))
}

struct Writer {
    data: Vec<u8>,
    encoding: &'static encoding_rs::Encoding,
}

impl Writer {
    fn new(code_page: &str) -> Self {
        Self {
            data: Vec::new(),
            encoding: encoding_from_code_page(code_page).unwrap_or(encoding_rs::WINDOWS_1252),
        }
    }
    fn byte(&mut self, value: u8) {
        self.data.push(value);
    }
    fn short(&mut self, value: i16) {
        self.data.extend(value.to_le_bytes());
    }
    fn ushort(&mut self, value: u16) {
        self.data.extend(value.to_le_bytes());
    }
    fn long(&mut self, value: u32) {
        self.data.extend(value.to_le_bytes());
    }
    fn double(&mut self, value: f64) -> Result<()> {
        if !value.is_finite() {
            return Err(invalid("Non-finite AC1009 table number"));
        }
        self.data.extend(value.to_le_bytes());
        Ok(())
    }
    fn number(&mut self, record: &EntityRecord, code: i32, default: f64) -> Result<()> {
        self.double(record.number(code, default))
    }
    fn angle(&mut self, record: &EntityRecord, code: i32) -> Result<()> {
        self.double(record.number(code, 0.0).to_radians())
    }
    fn point(&mut self, record: &EntityRecord, code: i32, defaults: &[f64]) -> Result<()> {
        for (index, default) in defaults.iter().enumerate() {
            self.number(record, code + index as i32 * 10, *default)?;
        }
        Ok(())
    }
    fn text(&mut self, value: &str, size: usize) -> Result<()> {
        if value.contains('\0') {
            return Err(invalid("Embedded NUL in AC1009 table string"));
        }
        let bytes = encode_legacy_string(value, self.encoding);
        if bytes.len() > size {
            return Err(invalid(format!(
                "AC1009 table string exceeds {size} encoded bytes: {value:?}"
            )));
        }
        self.data.extend_from_slice(&bytes);
        self.data.resize(self.data.len() + size - bytes.len(), 0);
        Ok(())
    }
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    encoding: &'static encoding_rs::Encoding,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8], code_page: &str) -> Self {
        Self {
            data,
            pos: 0,
            encoding: encoding_from_code_page(code_page).unwrap_or(encoding_rs::WINDOWS_1252),
        }
    }
    fn take(&mut self, size: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(size)
            .ok_or_else(|| invalid("AC1009 table offset overflow"))?;
        let result = self
            .data
            .get(self.pos..end)
            .ok_or_else(|| invalid("Truncated AC1009 table record"))?;
        self.pos = end;
        Ok(result)
    }
    fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn short(&mut self) -> Result<i16> {
        Ok(i16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn ushort(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn long(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn double(&mut self) -> Result<f64> {
        let value = f64::from_le_bytes(self.take(8)?.try_into().unwrap());
        if !value.is_finite() {
            return Err(invalid("Non-finite AC1009 table number"));
        }
        Ok(value)
    }
    fn number(&mut self, record: &mut EntityRecord, code: i32) -> Result<()> {
        put(record, code, self.double()?);
        Ok(())
    }
    fn angle(&mut self, record: &mut EntityRecord, code: i32) -> Result<()> {
        put(record, code, self.double()?.to_degrees());
        Ok(())
    }
    fn point(&mut self, record: &mut EntityRecord, code: i32, dimensions: i32) -> Result<()> {
        for dimension in 0..dimensions {
            self.number(record, code + dimension * 10)?;
        }
        Ok(())
    }
    fn text(&mut self, size: usize) -> Result<String> {
        let bytes = self.take(size)?;
        let length = bytes.iter().position(|byte| *byte == 0).unwrap_or(size);
        Ok(decode_cif_escapes(
            &self
                .encoding
                .decode_without_bom_handling(&bytes[..length])
                .0,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(kind: &str, pairs: &[(i32, &str)]) -> EntityRecord {
        EntityRecord {
            kind: kind.to_owned(),
            pairs: pairs
                .iter()
                .map(|(code, value)| (*code, (*value).to_owned()))
                .collect(),
        }
    }

    #[test]
    fn all_native_table_layouts_round_trip() {
        let records = [
            vec![record("BLOCK_RECORD", &[(2, "B1"), (70, "2")])],
            vec![record(
                "LAYER",
                &[(2, "L1"), (62, "-4"), (6, "DASHED"), (70, "5")],
            )],
            vec![record(
                "STYLE",
                &[
                    (2, "S1"),
                    (70, "4"),
                    (41, "1.3"),
                    (50, "15"),
                    (3, "txt.shx"),
                ],
            )],
            vec![record(
                "LTYPE",
                &[
                    (2, "DASHED"),
                    (3, "Dashed line"),
                    (49, "0.5"),
                    (49, "-0.25"),
                ],
            )],
            vec![record(
                "VIEW",
                &[(2, "V1"), (40, "3"), (41, "5"), (50, "30"), (71, "5")],
            )],
            vec![record("UCS", &[(2, "U1"), (10, "2"), (20, "3"), (30, "4")])],
            vec![record(
                "VPORT",
                &[
                    (2, "*ACTIVE"),
                    (40, "8"),
                    (41, "1.5"),
                    (74, "1"),
                    (50, "45"),
                    (51, "10"),
                ],
            )],
            vec![record("APPID", &[(2, "ACAD")])],
            vec![record(
                "DIMSTYLE",
                &[
                    (2, "D1"),
                    (3, "<> mm"),
                    (6, "B1"),
                    (40, "2"),
                    (77, "1"),
                    (176, "4"),
                    (288, "1"),
                ],
            )],
            vec![record(
                "VX",
                &[(2, "*ACTIVE"), (90, "1755"), (91, "0"), (92, "-1")],
            )],
        ];
        let offsets = HashMap::from([("B1".to_owned(), 42)]);
        let encoded = encode_tables(&records, &offsets, "ANSI_1252").unwrap();
        let mut file = Vec::new();
        let mut locators = [TableLocator::default(); 10];
        for (index, (size, payload)) in encoded.iter().enumerate() {
            assert_eq!(*size, RECORD_SIZES[index]);
            assert_eq!(payload.len(), *size as usize);
            file.extend(TABLE_BEGIN[index]);
            locators[index] = TableLocator {
                record_size: *size,
                count: 1,
                flags: 0,
                address: file.len() as u32,
            };
            file.extend(payload);
            file.extend(TABLE_END[index]);
        }
        let decoded = decode_tables(&file, &locators, "ANSI_1252").unwrap();
        assert_eq!(decoded[0][0].integer(90, 0) as u32, 0x4000002a);
        assert_eq!(decoded[1][0].integer(62, 0), -4);
        assert_eq!(decoded[1][0].string(6), "DASHED");
        assert_eq!(decoded[2][0].integer(70, 0), 4);
        assert_eq!(decoded[3][0].number(40, 0.0), 0.75);
        assert_eq!(
            decoded[3][0]
                .pairs
                .iter()
                .filter(|(code, _)| *code == 49)
                .count(),
            2
        );
        assert_eq!(decoded[4][0].integer(71, 0), 5);
        assert_eq!(decoded[5][0].number(30, 0.0), 4.0);
        assert_eq!(decoded[6][0].integer(74, 0), 1);
        assert!((decoded[6][0].number(50, 0.0) - 45.0).abs() < 1e-12);
        assert_eq!(decoded[7][0].string(2), "ACAD");
        assert_eq!(decoded[8][0].string(3), "<> mm");
        assert_eq!(decoded[8][0].integer(176, 0), 4);
        assert_eq!(decoded[9][0].integer(90, 0), 1755);
        assert_eq!(
            encode_tables(&decoded, &offsets, "ANSI_1252").unwrap(),
            encoded
        );
    }

    #[test]
    fn bricscad_layer_and_appid_bytes_match() {
        let tables = EntityTables {
            linetypes: vec!["CONTINUOUS".into()],
            ..Default::default()
        };
        let layer = record("LAYER", &[(2, "0"), (62, "7"), (6, "CONTINUOUS")]);
        let (_, bytes) =
            encode_table("LAYER", &[layer], &tables, &HashMap::new(), "ANSI_1252").unwrap();
        assert_eq!(&bytes[..2], &[0, b'0']);
        assert_eq!(&bytes[33..], &[0xff, 0xff, 7, 0, 0, 0, 0xaf, 0x42]);
        let appid = record("APPID", &[(2, "ACAD")]);
        let (_, bytes) =
            encode_table("APPID", &[appid], &tables, &HashMap::new(), "ANSI_1252").unwrap();
        assert_eq!(&bytes[33..], &[0xff, 0xff, 0x0a, 0xd1]);
    }

    #[test]
    fn crc_and_legacy_string_limits_are_checked() {
        let table = EntityTables::default();
        let appid = record("APPID", &[(2, "ACAD")]);
        let (size, mut bytes) =
            encode_table("APPID", &[appid], &table, &HashMap::new(), "ANSI_1252").unwrap();
        bytes[1] ^= 1;
        assert!(decode_table("APPID", &bytes, size, 1, &table, "ANSI_1252").is_err());
        let long_name = "A".repeat(33);
        let long = record("APPID", &[(2, &long_name)]);
        assert!(encode_table("APPID", &[long], &table, &HashMap::new(), "ANSI_1252").is_err());
        let named = record("APPID", &[(2, "\u{130}SLEM")]);
        let (size, bytes) = encode_table(
            "APPID",
            &[named.clone()],
            &table,
            &HashMap::new(),
            "ANSI_1254",
        )
        .unwrap();
        assert_eq!(
            decode_table("APPID", &bytes, size, 1, &table, "ANSI_1254").unwrap()[0].string(2),
            named.string(2)
        );
    }
}
