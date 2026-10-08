//! Byte-aligned entity records used by AC1009 drawings.

use crate::error::{DxfError, Result};
use crate::io::dwg::crc::crc16;
use crate::io::dxf::code_page::{encode_legacy_string, encoding_from_code_page};

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct EntityRecord {
    pub kind: String,
    pub pairs: Vec<(i32, String)>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct EntityTables {
    pub layers: Vec<String>,
    pub linetypes: Vec<String>,
    pub styles: Vec<String>,
    pub blocks: Vec<String>,
    pub dimstyles: Vec<String>,
    pub appids: Vec<String>,
}

impl EntityRecord {
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            pairs: Vec::new(),
        }
    }
    pub fn string(&self, code: i32) -> &str {
        self.pairs
            .iter()
            .find(|(c, _)| *c == code)
            .map_or("", |(_, v)| v.as_str())
    }
    pub fn number(&self, code: i32, default: f64) -> f64 {
        self.string(code).trim().parse().unwrap_or(default)
    }
    pub fn integer(&self, code: i32, default: i32) -> i32 {
        self.string(code).trim().parse().unwrap_or(default)
    }
    pub fn has(&self, code: i32) -> bool {
        self.pairs.iter().any(|(c, _)| *c == code)
    }
    fn put(&mut self, code: i32, value: impl ToString) {
        self.pairs.push((code, value.to_string()));
    }
}

fn invalid(message: impl Into<String>) -> DxfError {
    DxfError::InvalidFormat(message.into())
}
fn lookup(table: &[String], name: &str, default: i16) -> Result<i16> {
    if name.is_empty() {
        return Ok(default);
    }
    let index = table
        .iter()
        .position(|s| s.eq_ignore_ascii_case(name))
        .ok_or_else(|| {
            invalid(format!(
                "R12 entity references missing table entry {name:?}"
            ))
        })?;
    i16::try_from(index).map_err(|_| invalid("R12 table index exceeds 32767"))
}
fn table_name(table: &[String], index: i16) -> Result<&str> {
    usize::try_from(index)
        .ok()
        .and_then(|i| table.get(i))
        .map(String::as_str)
        .ok_or_else(|| invalid(format!("Invalid R12 table index {index}")))
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
    encoding: &'static encoding_rs::Encoding,
}
impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8], code_page: &str) -> Self {
        Self {
            bytes,
            pos: 0,
            encoding: encoding_from_code_page(code_page).unwrap_or(encoding_rs::WINDOWS_1252),
        }
    }
    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or_else(|| invalid("R12 entity length overflow"))?;
        let value = self
            .bytes
            .get(self.pos..end)
            .ok_or_else(|| invalid("Truncated R12 entity"))?;
        self.pos = end;
        Ok(value)
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
        Ok(f64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn string(&mut self) -> Result<String> {
        let len = self.ushort()? as usize;
        let bytes = self.take(len)?;
        Ok(self
            .encoding
            .decode_without_bom_handling(bytes)
            .0
            .trim_end_matches('\0')
            .to_owned())
    }
    fn point(&mut self, record: &mut EntityRecord, code: i32, is3d: bool, z: f64) -> Result<()> {
        record.put(code, self.double()?);
        record.put(code + 10, self.double()?);
        record.put(code + 20, if is3d { self.double()? } else { z });
        Ok(())
    }
    fn angle(&mut self, record: &mut EntityRecord, code: i32) -> Result<()> {
        record.put(code, self.double()?.to_degrees());
        Ok(())
    }
}

struct Writer {
    bytes: Vec<u8>,
    encoding: &'static encoding_rs::Encoding,
}
impl Writer {
    fn new(code_page: &str) -> Self {
        Self {
            bytes: Vec::new(),
            encoding: encoding_from_code_page(code_page).unwrap_or(encoding_rs::WINDOWS_1252),
        }
    }
    fn byte(&mut self, v: u8) {
        self.bytes.push(v);
    }
    fn short(&mut self, v: i16) {
        self.bytes.extend(v.to_le_bytes());
    }
    fn long(&mut self, v: u32) {
        self.bytes.extend(v.to_le_bytes());
    }
    fn double(&mut self, v: f64) {
        self.bytes.extend(v.to_le_bytes());
    }
    fn string(&mut self, value: &str) -> Result<()> {
        let bytes = encode_legacy_string(value, self.encoding);
        let len =
            u16::try_from(bytes.len()).map_err(|_| invalid("R12 string exceeds 65535 bytes"))?;
        self.bytes.extend(len.to_le_bytes());
        self.bytes.extend(bytes);
        Ok(())
    }
    fn ref_index(&mut self, index: i16, size: u8) {
        if size == 1 {
            self.byte(index as u8);
        } else {
            self.short(index);
        }
    }
    fn point(&mut self, record: &EntityRecord, code: i32, is3d: bool) {
        self.double(record.number(code, 0.));
        self.double(record.number(code + 10, 0.));
        if is3d {
            self.double(record.number(code + 20, if code == 210 { 1. } else { 0. }));
        }
    }
    fn angle(&mut self, record: &EntityRecord, code: i32) {
        self.double(record.number(code, 0.).to_radians());
    }
}

fn native_type(name: &str) -> Option<u8> {
    Some(match name {
        "LINE" => 1,
        "POINT" => 2,
        "CIRCLE" => 3,
        "SHAPE" => 4,
        "REPEAT" => 5,
        "ENDREP" => 6,
        "TEXT" => 7,
        "ARC" => 8,
        "TRACE" => 9,
        "LOAD" => 10,
        "SOLID" => 11,
        "BLOCK" => 12,
        "ENDBLK" => 13,
        "INSERT" | "MINSERT" => 14,
        "ATTDEF" => 15,
        "ATTRIB" => 16,
        "SEQEND" => 17,
        "JUMP" => 18,
        "POLYLINE" => 19,
        "VERTEX" => 20,
        "3DLINE" => 21,
        "3DFACE" => 22,
        "DIMENSION" => 23,
        "VIEWPORT" => 24,
        _ => return None,
    })
}

fn type_name(code: u8) -> Option<&'static str> {
    Some(match code {
        1 => "LINE",
        2 => "POINT",
        3 => "CIRCLE",
        4 => "SHAPE",
        5 => "REPEAT",
        6 => "ENDREP",
        7 => "TEXT",
        8 => "ARC",
        9 => "TRACE",
        10 => "LOAD",
        11 => "SOLID",
        12 => "BLOCK",
        13 => "ENDBLK",
        14 => "INSERT",
        15 => "ATTDEF",
        16 => "ATTRIB",
        17 => "SEQEND",
        18 => "JUMP",
        19 => "POLYLINE",
        20 => "VERTEX",
        21 => "3DLINE",
        22 => "3DFACE",
        23 => "DIMENSION",
        24 => "VIEWPORT",
        _ => return None,
    })
}

fn angle_codes(name: &str) -> &'static [i32] {
    match name {
        "TEXT" => &[50, 51],
        "ATTRIB" | "ATTDEF" => &[50, 51],
        _ => &[],
    }
}

fn opts(record: &EntityRecord) -> u16 {
    let normal = record.number(210, 0.) != 0.
        || record.number(220, 0.) != 0.
        || record.number(230, 1.) != 1.;
    let mut value = 0;
    let mut set = |bit, yes| {
        if yes {
            value |= bit;
        }
    };
    match record.kind.as_str() {
        "LINE" | "3DLINE" | "CIRCLE" | "ARC" | "SOLID" | "TRACE" => set(1, normal),
        "POINT" => {
            set(1, normal);
            set(2, record.number(50, 0.) != 0.);
        }
        "TEXT" => {
            for (bit, code, default) in [
                (1, 50, 0.),
                (2, 41, 1.),
                (4, 51, 0.),
                (16, 71, 0.),
                (32, 72, 0.),
                (256, 73, 0.),
            ] {
                set(bit, record.number(code, default) != default);
            }
            set(
                8,
                !record.string(7).is_empty() && !record.string(7).eq_ignore_ascii_case("STANDARD"),
            );
            set(64, record.has(11));
            set(128, normal);
        }
        "ATTRIB" | "ATTDEF" => {
            for (bit, code, default) in [
                (2, 50, 0.),
                (4, 41, 1.),
                (8, 51, 0.),
                (32, 71, 0.),
                (64, 72, 0.),
                (512, 74, 0.),
            ] {
                set(bit, record.number(code, default) != default);
            }
            set(
                16,
                !record.string(7).is_empty() && !record.string(7).eq_ignore_ascii_case("STANDARD"),
            );
            set(128, record.has(11));
            set(256, normal);
        }
        "BLOCK" => {
            set(2, !record.string(1).is_empty());
            set(4, !record.string(2).is_empty());
        }
        "INSERT" | "MINSERT" => {
            for (bit, code, default) in [
                (1, 41, 1.),
                (2, 42, 1.),
                (4, 50, 0.),
                (8, 43, 1.),
                (16, 70, 1.),
                (32, 71, 1.),
                (64, 44, 0.),
                (128, 45, 0.),
            ] {
                set(bit, record.number(code, default) != default);
            }
            set(256, normal);
        }
        "POLYLINE" => {
            set(1, record.integer(70, 0) != 0);
            set(2, record.number(40, 0.) != 0.);
            set(4, record.number(41, 0.) != 0.);
            set(8, normal);
            for (bit, code) in [(16, 71), (32, 72), (64, 73), (128, 74), (256, 75)] {
                set(bit, record.integer(code, 0) != 0);
            }
        }
        "VERTEX" => {
            for (bit, code) in [(1, 40), (2, 41), (4, 42), (8, 70), (16, 50)] {
                set(bit, record.number(code, 0.) != 0.);
            }
            for (bit, code) in [(32, 71), (64, 72), (128, 73), (256, 74)] {
                set(bit, record.has(code));
            }
            set(
                0x4000,
                record.integer(70, 0) & 128 != 0 && record.integer(70, 0) & 64 == 0,
            );
        }
        "3DFACE" => set(1, record.integer(70, 0) != 0),
        "SHAPE" => {
            set(1, record.number(50, 0.) != 0.);
            set(2, !record.string(7).is_empty());
            set(4, record.number(41, 1.) != 1.);
            set(8, record.number(51, 0.) != 0.);
        }
        "DIMENSION" => {
            set(1, record.has(12));
            set(2, true);
            set(4, record.has(1));
            for (bit, code) in [(8, 13), (16, 14), (32, 15), (64, 16)] {
                set(bit, record.has(code));
            }
            set(128, record.number(40, 0.) != 0.);
            set(256, record.has(50));
            set(512, record.has(52));
            set(1024, record.has(53));
            set(0x4000, normal);
            set(0x8000, !record.string(3).is_empty());
        }
        _ => {}
    }
    value
}

/// Encode records without the section's begin/end sentinels.
pub(crate) fn encode_entities(
    records: &[EntityRecord],
    tables: &EntityTables,
    base_address: u32,
    code_page: &str,
) -> Result<Vec<u8>> {
    let mut result = Vec::new();
    let mut sequence = Vec::new();
    for record in records {
        let address = base_address
            .checked_add(result.len() as u32)
            .ok_or_else(|| invalid("R12 entity address overflow"))?;
        if matches!(record.kind.as_str(), "POLYLINE" | "INSERT" | "MINSERT")
            && (record.kind == "POLYLINE" || record.integer(66, 0) != 0)
        {
            sequence.push(address);
        }
        let owner = if record.kind == "SEQEND" {
            sequence.pop().unwrap_or(0)
        } else {
            0
        };
        result.extend(encode_record(record, tables, owner, code_page)?);
    }
    Ok(result)
}

fn encode_record(
    record: &EntityRecord,
    tables: &EntityTables,
    sequence_owner: u32,
    code_page: &str,
) -> Result<Vec<u8>> {
    let kind = native_type(&record.kind).ok_or_else(|| {
        DxfError::UnsupportedVersion(format!("{} is unavailable in R12 DWG", record.kind))
    })?;
    let options = opts(record);
    let mut writer = Writer::new(code_page);
    let mut flag = 0u8;
    let ltype = record.string(6);
    let color = record.integer(62, 256);
    if color != 256 {
        flag |= 1;
    }
    if !ltype.is_empty() && !ltype.eq_ignore_ascii_case("BYLAYER") {
        flag |= 2;
    }
    let is3d = matches!(record.kind.as_str(), "LINE" | "3DLINE" | "POINT" | "3DFACE");
    let elevation = if record.kind == "POLYLINE" {
        record.number(30, record.number(38, 0.))
    } else {
        record.number(30, record.number(38, 0.))
    };
    if !is3d && elevation != 0. {
        flag |= 4;
    }
    if record.number(39, 0.) != 0. {
        flag |= 8;
    }
    let handle = u64::from_str_radix(record.string(5).trim(), 16).unwrap_or(0);
    if handle != 0 {
        flag |= 32;
    }
    let eed = encode_eed(record, tables, code_page)?;
    let paper = record.integer(67, 0) != 0;
    if paper || !eed.is_empty() {
        flag |= 64;
    }
    if record.kind == "POLYLINE" || record.integer(66, 0) != 0 {
        flag |= 128;
    }
    writer.byte(kind);
    writer.byte(flag);
    writer.short(0);
    if kind != 18 {
        writer.short(lookup(&tables.layers, record.string(8), 0)?);
        writer.short(options as i16);
    }
    if kind == 18 {
        writer.long(record.integer(90, 0) as u32);
    } else {
        if flag & 64 != 0 {
            writer.byte((paper as u8) | if eed.is_empty() { 0 } else { 2 });
        }
        if !eed.is_empty() {
            writer.short(
                u16::try_from(eed.len()).map_err(|_| invalid("R12 EED exceeds 65535 bytes"))?
                    as i16,
            );
            writer.bytes.extend(eed);
        }
        if flag & 1 != 0 {
            writer.byte(color as u8);
        }
        if flag & 2 != 0 {
            writer.byte(
                (if ltype.eq_ignore_ascii_case("BYBLOCK") {
                    -2
                } else {
                    lookup(&tables.linetypes, ltype, -1)?
                }) as i8 as u8,
            );
        }
        if flag & 4 != 0 {
            writer.double(elevation);
        }
        if flag & 8 != 0 {
            writer.double(record.number(39, 0.));
        }
        if flag & 32 != 0 {
            let bytes = handle.to_be_bytes();
            let first = bytes.iter().position(|b| *b != 0).unwrap_or(7);
            writer.byte((8 - first) as u8);
            writer.bytes.extend(&bytes[first..]);
        }
        encode_geometry(&mut writer, record, tables, options, sequence_owner)?;
    }
    let size = u16::try_from(writer.bytes.len() + 2)
        .map_err(|_| invalid("R12 entity exceeds 65535 bytes"))?;
    writer.bytes[2..4].copy_from_slice(&size.to_le_bytes());
    let checksum = crc16(0xC0C1, &writer.bytes);
    writer.bytes.extend(checksum.to_le_bytes());
    Ok(writer.bytes)
}

fn encode_geometry(
    w: &mut Writer,
    r: &EntityRecord,
    t: &EntityTables,
    o: u16,
    owner: u32,
) -> Result<()> {
    let bit = |b| o & b != 0;
    match r.kind.as_str() {
        "LINE" | "3DLINE" => {
            w.point(r, 10, true);
            w.point(r, 11, true);
            if bit(1) {
                w.point(r, 210, true);
            }
        }
        "POINT" => {
            w.point(r, 10, true);
            if bit(1) {
                w.point(r, 210, true);
            }
            if bit(2) {
                w.angle(r, 50);
            }
        }
        "CIRCLE" | "ARC" => {
            w.point(r, 10, false);
            w.double(r.number(40, 0.));
            if r.kind == "ARC" {
                w.angle(r, 50);
                w.angle(r, 51);
            }
            if bit(1) {
                w.point(r, 210, true);
            }
        }
        "SOLID" | "TRACE" => {
            for code in 10..=13 {
                w.point(r, code, false);
            }
            if bit(1) {
                w.point(r, 210, true);
            }
        }
        "3DFACE" => {
            for code in 10..=13 {
                w.point(r, code, true);
            }
            if bit(1) {
                w.short(r.integer(70, 0) as i16);
            }
        }
        "TEXT" | "ATTRIB" | "ATTDEF" => {
            w.point(r, 10, false);
            w.double(r.number(40, 1.));
            w.string(r.string(1))?;
            if r.kind == "ATTDEF" {
                w.string(r.string(3))?;
            }
            if r.kind != "TEXT" {
                w.string(r.string(2))?;
                w.byte(r.integer(70, 0) as u8);
            }
            let shift = if r.kind == "TEXT" { 0 } else { 1 };
            for (base, code) in [(1, 50), (2, 41), (4, 51)] {
                if bit(base << shift) {
                    if angle_codes(&r.kind).contains(&code) {
                        w.angle(r, code);
                    } else {
                        w.double(r.number(code, 1.));
                    }
                }
            }
            if bit(8 << shift) {
                let index = lookup(&t.styles, r.string(7), 0)?;
                w.ref_index(index, 1);
            }
            for (base, code) in [(16, 71), (32, 72)] {
                if bit(base << shift) {
                    w.byte(r.integer(code, 0) as u8);
                }
            }
            if bit(64 << shift) {
                w.point(r, 11, false);
            }
            if bit(128 << shift) {
                w.point(r, 210, true);
            }
            if bit(256 << shift) {
                w.byte(r.integer(if r.kind == "TEXT" { 73 } else { 74 }, 0) as u8);
            }
        }
        "SHAPE" => {
            w.point(r, 10, false);
            w.double(r.number(40, 1.));
            w.byte(r.integer(70, r.integer(2, 0)) as u8);
            if bit(1) {
                w.angle(r, 50);
            }
            if bit(2) {
                w.ref_index(lookup(&t.styles, r.string(7), 0)?, 1);
            }
            if bit(4) {
                w.double(r.number(41, 1.));
            }
            if bit(8) {
                w.angle(r, 51);
            }
        }
        "BLOCK" => {
            w.point(r, 10, false);
            if bit(2) {
                w.string(r.string(1))?;
            }
            if bit(4) {
                w.string(r.string(2))?;
            }
        }
        "ENDBLK" | "REPEAT" => {}
        "SEQEND" => w.long(owner),
        "INSERT" | "MINSERT" => {
            w.ref_index(lookup(&t.blocks, r.string(2), 0)?, 2);
            w.point(r, 10, false);
            for (b, c, d) in [(1, 41, 1.), (2, 42, 1.), (4, 50, 0.), (8, 43, 1.)] {
                if bit(b) {
                    if c == 50 {
                        w.angle(r, c);
                    } else {
                        w.double(r.number(c, d));
                    }
                }
            }
            for (b, c) in [(16, 70), (32, 71)] {
                if bit(b) {
                    w.short(r.integer(c, 1) as i16);
                }
            }
            for (b, c) in [(64, 44), (128, 45)] {
                if bit(b) {
                    w.double(r.number(c, 0.));
                }
            }
            if bit(256) {
                w.point(r, 210, true);
            }
        }
        "POLYLINE" => {
            if bit(1) {
                w.byte(r.integer(70, 0) as u8);
            }
            if bit(2) {
                w.double(r.number(40, 0.));
            }
            if bit(4) {
                w.double(r.number(41, 0.));
            }
            if bit(8) {
                w.point(r, 210, true);
            }
            for (b, c) in [(16, 71), (32, 72), (64, 73), (128, 74), (256, 75)] {
                if bit(b) {
                    w.short(r.integer(c, 0) as i16);
                }
            }
        }
        "VERTEX" => {
            if !bit(0x4000) {
                w.point(r, 10, false);
            }
            for (b, c) in [(1, 40), (2, 41), (4, 42)] {
                if bit(b) {
                    w.double(r.number(c, 0.));
                }
            }
            if bit(8) {
                w.byte(r.integer(70, 0) as u8);
            }
            if bit(16) {
                w.angle(r, 50);
            }
            for (b, c) in [(32, 71), (64, 72), (128, 73), (256, 74)] {
                if bit(b) {
                    w.short(r.integer(c, 0) as i16);
                }
            }
        }
        "DIMENSION" => encode_dimension(w, r, t, o)?,
        "VIEWPORT" => {
            w.point(r, 10, true);
            w.double(r.number(40, 0.));
            w.double(r.number(41, 0.));
            w.short(r.integer(69, 0) as i16);
        }
        "ENDREP" => {
            w.short(r.integer(70, 0) as i16);
            w.short(r.integer(71, 0) as i16);
            w.double(r.number(40, 0.));
            w.double(r.number(41, 0.));
        }
        "LOAD" => w.string(r.string(1))?,
        _ => return Err(invalid(format!("Unsupported R12 entity {}", r.kind))),
    }
    Ok(())
}

fn encode_dimension(w: &mut Writer, r: &EntityRecord, t: &EntityTables, o: u16) -> Result<()> {
    w.ref_index(lookup(&t.blocks, r.string(2), 0)?, 2);
    w.point(r, 10, true);
    w.point(r, 11, false);
    if o & 1 != 0 {
        w.point(r, 12, false);
    }
    let subtype = r.integer(70, 0) & 7;
    if o & 2 != 0 {
        w.byte((r.integer(70, 0) & !32) as u8);
    }
    if o & 4 != 0 {
        w.string(r.string(1))?;
    }
    for (bit, code) in [(8, 13), (16, 14), (32, 15)] {
        if o & bit != 0 {
            w.point(r, code, true);
        }
    }
    if o & 64 != 0 {
        w.point(r, 16, false);
    }
    if o & 128 != 0 {
        w.double(r.number(40, 0.));
    }
    if o & 256 != 0 {
        w.angle(r, 50);
    }
    if o & 512 != 0 && subtype == 0 {
        w.angle(r, 52);
    }
    if o & 1024 != 0 {
        w.angle(r, 53);
    }
    if o & 0x4000 != 0 {
        w.point(r, 210, true);
    }
    if o & 0x8000 != 0 {
        w.ref_index(lookup(&t.dimstyles, r.string(3), 0)?, 2);
    }
    Ok(())
}

/// Decode records without the section's begin/end sentinels.
pub(crate) fn decode_entities(
    bytes: &[u8],
    tables: &EntityTables,
    _base_address: u32,
    code_page: &str,
) -> Result<Vec<EntityRecord>> {
    let mut records = Vec::new();
    let mut offset = 0;
    while offset < bytes.len() {
        let header = bytes
            .get(offset..offset + 4)
            .ok_or_else(|| invalid("Truncated R12 entity header"))?;
        let size = u16::from_le_bytes([header[2], header[3]]) as usize;
        if size < 6 {
            return Err(invalid("Invalid R12 entity length"));
        }
        let raw = bytes
            .get(offset..offset + size)
            .ok_or_else(|| invalid("R12 entity exceeds section bounds"))?;
        let expected = u16::from_le_bytes(raw[size - 2..].try_into().unwrap());
        if crc16(0xC0C1, &raw[..size - 2]) != expected {
            return Err(invalid(format!(
                "R12 entity CRC mismatch at section offset {offset}"
            )));
        }
        if header[0] & 128 == 0 {
            records.push(decode_record(&raw[..size - 2], tables, code_page)?);
        }
        offset += size;
    }
    Ok(records)
}

fn decode_record(bytes: &[u8], t: &EntityTables, code_page: &str) -> Result<EntityRecord> {
    let mut r = Reader::new(bytes, code_page);
    let kind = r.byte()? & 127;
    let flag = r.byte()?;
    r.ushort()?;
    let name = type_name(kind).ok_or_else(|| invalid(format!("Unknown R12 entity type {kind}")))?;
    let mut out = EntityRecord::new(name);
    if kind == 18 {
        out.put(90, r.long()? as i32);
        return Ok(out);
    }
    out.put(8, table_name(&t.layers, r.short()?)?);
    let options = r.ushort()?;
    let extra = if flag & 64 != 0 { r.byte()? } else { 0 };
    if extra & 1 != 0 {
        out.put(67, 1);
    }
    if extra & 2 != 0 {
        let size = r.ushort()? as usize;
        decode_eed(r.take(size)?, t, code_page, &mut out)?;
    }
    if flag & 1 != 0 {
        out.put(62, r.byte()? as i8 as i16);
    }
    if flag & 2 != 0 {
        let index = r.byte()? as i8 as i16;
        out.put(
            6,
            match index {
                -1 => "BYLAYER",
                -2 => "BYBLOCK",
                _ => table_name(&t.linetypes, index)?,
            },
        );
    }
    let is3d = matches!(kind, 1 | 2 | 21 | 22);
    let elevation = if flag & 4 != 0 && !is3d {
        r.double()?
    } else {
        0.
    };
    if flag & 8 != 0 {
        out.put(39, r.double()?);
    }
    if flag & 32 != 0 {
        let size = r.byte()? as usize;
        if size > 8 {
            return Err(invalid("R12 handle exceeds 8 bytes"));
        }
        let mut handle = 0u64;
        for b in r.take(size)? {
            handle = (handle << 8) | *b as u64;
        }
        out.put(5, format!("{handle:X}"));
    }
    if extra & 4 != 0 {
        r.short()?;
    }
    if flag & 128 != 0 && matches!(kind, 14 | 19) {
        out.put(66, 1);
    }
    decode_geometry(&mut r, &mut out, t, options, flag, elevation)?;
    Ok(out)
}

fn decode_geometry(
    r: &mut Reader<'_>,
    out: &mut EntityRecord,
    t: &EntityTables,
    o: u16,
    flag: u8,
    z: f64,
) -> Result<()> {
    let bit = |b| o & b != 0;
    match out.kind.as_str() {
        "LINE" | "3DLINE" => {
            r.point(out, 10, flag & 4 == 0, 0.)?;
            r.point(out, 11, flag & 4 == 0, 0.)?;
            if bit(1) {
                r.point(out, 210, true, 1.)?;
            }
        }
        "POINT" => {
            r.point(out, 10, flag & 4 == 0, 0.)?;
            if bit(1) {
                r.point(out, 210, true, 1.)?;
            }
            if bit(2) {
                r.angle(out, 50)?;
            }
        }
        "CIRCLE" | "ARC" => {
            r.point(out, 10, false, z)?;
            out.put(40, r.double()?);
            if out.kind == "ARC" {
                r.angle(out, 50)?;
                r.angle(out, 51)?;
            }
            if bit(1) {
                r.point(out, 210, true, 1.)?;
            }
            if bit(2) {
                out.put(38, r.double()?);
            }
        }
        "SOLID" | "TRACE" => {
            for code in 10..=13 {
                r.point(out, code, false, z)?;
            }
            if bit(1) {
                r.point(out, 210, true, 1.)?;
            }
            if bit(2) {
                out.put(38, r.double()?);
            }
        }
        "3DFACE" => {
            for code in 10..=13 {
                r.point(out, code, flag & 4 == 0, 0.)?;
            }
            if bit(1) {
                out.put(70, r.short()?);
            }
        }
        "TEXT" | "ATTRIB" | "ATTDEF" => {
            r.point(out, 10, false, z)?;
            out.put(40, r.double()?);
            out.put(1, r.string()?);
            if out.kind == "ATTDEF" {
                out.put(3, r.string()?);
            }
            let shift = if out.kind == "TEXT" {
                0
            } else {
                out.put(2, r.string()?);
                out.put(70, r.byte()?);
                1
            };
            for (base, code) in [(1, 50), (2, 41), (4, 51)] {
                if bit(base << shift) {
                    if angle_codes(&out.kind).contains(&code) {
                        r.angle(out, code)?;
                    } else {
                        out.put(code, r.double()?);
                    }
                }
            }
            if bit(8 << shift) {
                out.put(7, table_name(&t.styles, r.byte()? as i8 as i16)?);
            }
            for (base, code) in [(16, 71), (32, 72)] {
                if bit(base << shift) {
                    out.put(code, r.byte()?);
                }
            }
            if bit(64 << shift) {
                r.point(out, 11, false, z)?;
            }
            if bit(128 << shift) {
                r.point(out, 210, true, 1.)?;
            }
            if bit(256 << shift) {
                out.put(if out.kind == "TEXT" { 73 } else { 74 }, r.byte()?);
            }
        }
        "SHAPE" => {
            r.point(out, 10, false, z)?;
            out.put(40, r.double()?);
            let number = r.byte()?;
            out.put(70, number);
            out.put(2, number);
            if bit(1) {
                r.angle(out, 50)?;
            }
            if bit(2) {
                out.put(7, table_name(&t.styles, r.byte()? as i8 as i16)?);
            }
            if bit(4) {
                out.put(41, r.double()?);
            }
            if bit(8) {
                r.angle(out, 51)?;
            }
        }
        "BLOCK" => {
            r.point(out, 10, false, z)?;
            if bit(2) {
                out.put(1, r.string()?);
            }
            if bit(4) {
                out.put(2, r.string()?);
            }
        }
        "ENDBLK" | "REPEAT" => {}
        "SEQEND" => {
            r.long()?;
        }
        "INSERT" => {
            out.put(2, table_name(&t.blocks, r.short()?)?);
            r.point(out, 10, false, z)?;
            for (b, c) in [(1, 41), (2, 42), (4, 50), (8, 43)] {
                if bit(b) {
                    if c == 50 {
                        r.angle(out, c)?;
                    } else {
                        out.put(c, r.double()?);
                    }
                }
            }
            for (b, c) in [(16, 70), (32, 71)] {
                if bit(b) {
                    out.put(c, r.short()?);
                }
            }
            for (b, c) in [(64, 44), (128, 45)] {
                if bit(b) {
                    out.put(c, r.double()?);
                }
            }
            if bit(256) {
                r.point(out, 210, true, 1.)?;
            }
        }
        "POLYLINE" => {
            out.put(10, 0);
            out.put(20, 0);
            out.put(30, z);
            if bit(1) {
                out.put(70, r.byte()?);
            }
            if bit(2) {
                out.put(40, r.double()?);
            }
            if bit(4) {
                out.put(41, r.double()?);
            }
            if bit(8) {
                r.point(out, 210, true, 1.)?;
            }
            for (b, c) in [(16, 71), (32, 72), (64, 73), (128, 74), (256, 75)] {
                if bit(b) {
                    out.put(c, r.short()?);
                }
            }
            // With the R11/R12 extra-section bit set, the remaining bytes
            // belong to the extra entity stream reached through a JUMP
            // record. The section walker already bounds this record, so the
            // opaque tail can be skipped here while vertices are decoded from
            // the extra section itself.
        }
        "VERTEX" => {
            if !bit(0x4000) {
                r.point(out, 10, false, z)?;
            } else {
                out.put(10, 0);
                out.put(20, 0);
                out.put(30, 0);
            }
            for (b, c) in [(1, 40), (2, 41), (4, 42)] {
                if bit(b) {
                    out.put(c, r.double()?);
                }
            }
            if bit(8) {
                out.put(70, r.byte()?);
            }
            if bit(16) {
                r.angle(out, 50)?;
            }
            for (b, c) in [(32, 71), (64, 72), (128, 73), (256, 74)] {
                if bit(b) {
                    out.put(c, r.short()?);
                }
            }
        }
        "DIMENSION" => decode_dimension(r, out, t, o, z)?,
        "VIEWPORT" => {
            r.point(out, 10, true, 0.)?;
            out.put(40, r.double()?);
            out.put(41, r.double()?);
            out.put(69, r.short()?);
            out.put(68, 1);
        }
        "ENDREP" => {
            out.put(70, r.short()?);
            out.put(71, r.short()?);
            out.put(40, r.double()?);
            out.put(41, r.double()?);
        }
        "LOAD" => {
            out.put(1, r.string()?);
        }
        _ => return Err(invalid(format!("Unsupported R12 geometry {}", out.kind))),
    }
    Ok(())
}

fn decode_dimension(
    r: &mut Reader<'_>,
    out: &mut EntityRecord,
    t: &EntityTables,
    o: u16,
    z: f64,
) -> Result<()> {
    out.put(2, table_name(&t.blocks, r.short()?)?);
    r.point(out, 10, true, 0.)?;
    r.point(out, 11, false, z)?;
    if o & 1 != 0 {
        r.point(out, 12, false, z)?;
    }
    let flag = if o & 2 != 0 { r.byte()? } else { 0 };
    out.put(70, flag as u16 | 32);
    if o & 4 != 0 {
        out.put(1, r.string()?);
    }
    let subtype = flag & 7;
    for (bit, code) in [(8, 13), (16, 14), (32, 15)] {
        if o & bit != 0 {
            r.point(out, code, !(subtype == 3 && flag & 4 != 0), z)?;
        }
    }
    if o & 64 != 0 {
        r.point(out, 16, false, z)?;
    }
    if o & 128 != 0 {
        out.put(40, r.double()?);
    }
    if o & 256 != 0 {
        r.angle(out, 50)?;
    }
    if o & 512 != 0 && subtype == 0 {
        r.angle(out, 52)?;
    }
    if o & 1024 != 0 {
        r.angle(out, 53)?;
    }
    if o & 0x4000 != 0 {
        r.point(out, 210, true, 1.)?;
    }
    if o & 0x8000 != 0 {
        out.put(3, table_name(&t.dimstyles, r.short()?)?);
    }
    Ok(())
}

fn encode_eed(record: &EntityRecord, tables: &EntityTables, code_page: &str) -> Result<Vec<u8>> {
    let mut w = Writer::new(code_page);
    for (i, (code, value)) in record
        .pairs
        .iter()
        .enumerate()
        .filter(|(_, (code, _))| *code >= 1000)
    {
        let item = code - 1000;
        if *code == 1001 {
            w.byte(1);
            w.short(lookup(&tables.appids, value, 0)?);
            continue;
        }
        if matches!(item, 20..=33) {
            continue;
        }
        w.byte(u8::try_from(item).map_err(|_| invalid("Unsupported R12 XDATA code"))?);
        match item {
            0 => {
                let bytes = encode_legacy_string(value, w.encoding);
                let len = u8::try_from(bytes.len())
                    .map_err(|_| invalid("R12 XDATA string exceeds 255 bytes"))?;
                w.byte(len);
                w.bytes.extend(bytes);
            }
            2 => w.byte((value == "}") as u8),
            3 => w.short(lookup(&tables.layers, value, 0)?),
            4 => {
                let bytes = parse_hex(value)?;
                w.byte(
                    u8::try_from(bytes.len())
                        .map_err(|_| invalid("R12 XDATA binary chunk exceeds 255 bytes"))?,
                );
                w.bytes.extend(bytes);
            }
            5 => {
                let value = u64::from_str_radix(value.trim(), 16)
                    .map_err(|_| invalid("Invalid R12 XDATA handle"))?;
                w.bytes.extend(value.to_be_bytes());
            }
            10..=13 => {
                w.double(
                    value
                        .parse()
                        .map_err(|_| invalid("Invalid R12 XDATA point"))?,
                );
                for axis in [10, 20] {
                    let coordinate = record.pairs[i + 1..]
                        .iter()
                        .take_while(|(c, _)| *c != 1001)
                        .find(|(c, _)| *c == *code + axis)
                        .map_or("0", |(_, s)| s.as_str());
                    w.double(
                        coordinate
                            .parse()
                            .map_err(|_| invalid("Invalid R12 XDATA point"))?,
                    );
                }
            }
            40..=42 => w.double(
                value
                    .parse()
                    .map_err(|_| invalid("Invalid R12 XDATA real"))?,
            ),
            70 => w.short(
                value
                    .parse()
                    .map_err(|_| invalid("Invalid R12 XDATA short"))?,
            ),
            71 => w.long(
                value
                    .parse::<i32>()
                    .map_err(|_| invalid("Invalid R12 XDATA long"))? as u32,
            ),
            _ => return Err(invalid(format!("Unsupported R12 XDATA code {code}"))),
        }
    }
    Ok(w.bytes)
}

fn decode_eed(
    bytes: &[u8],
    tables: &EntityTables,
    code_page: &str,
    out: &mut EntityRecord,
) -> Result<()> {
    let mut r = Reader::new(bytes, code_page);
    while r.pos < bytes.len() {
        let code = r.byte()?;
        match code {
            0 => {
                let len = r.byte()? as usize;
                let encoding = r.encoding;
                let value = encoding
                    .decode_without_bom_handling(r.take(len)?)
                    .0
                    .into_owned();
                out.put(1000, value);
            }
            1 => out.put(1001, table_name(&tables.appids, r.short()?)?),
            2 => out.put(1002, if r.byte()? == 0 { "{" } else { "}" }),
            3 => out.put(1003, table_name(&tables.layers, r.short()?)?),
            4 => {
                let len = r.byte()? as usize;
                out.put(1004, hex_string(r.take(len)?));
            }
            5 => {
                let value = u64::from_be_bytes(r.take(8)?.try_into().unwrap());
                out.put(1005, format!("{value:X}"));
            }
            10..=13 => r.point(out, 1000 + code as i32, true, 0.)?,
            40..=42 => out.put(1000 + code as i32, r.double()?),
            70 => out.put(1070, r.short()?),
            71 => out.put(1071, r.long()? as i32),
            _ => return Err(invalid(format!("Unknown R12 XDATA item {code}"))),
        }
    }
    Ok(())
}

fn parse_hex(value: &str) -> Result<Vec<u8>> {
    if value.len() % 2 != 0 {
        return Err(invalid("Invalid R12 binary hex string"));
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let s =
                std::str::from_utf8(pair).map_err(|_| invalid("Invalid R12 binary hex string"))?;
            u8::from_str_radix(s, 16).map_err(|_| invalid("Invalid R12 binary hex string"))
        })
        .collect()
}
fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tables() -> EntityTables {
        EntityTables {
            layers: vec!["0".into()],
            linetypes: vec!["CONTINUOUS".into()],
            styles: vec!["STANDARD".into(), "ALT".into()],
            blocks: vec!["*MODEL_SPACE".into(), "*PAPER_SPACE".into(), "B".into()],
            dimstyles: vec!["STANDARD".into()],
            appids: vec!["ACAD".into()],
        }
    }

    fn record(kind: &str) -> EntityRecord {
        let mut r = EntityRecord::new(kind);
        r.put(8, "0");
        r
    }

    fn point(r: &mut EntityRecord, code: i32, x: f64, y: f64, z: f64) {
        r.put(code, x);
        r.put(code + 10, y);
        r.put(code + 20, z);
    }

    fn native_records() -> Vec<EntityRecord> {
        let mut out = Vec::new();

        let mut r = record("LINE");
        point(&mut r, 10, 1., 2., 3.);
        point(&mut r, 11, 4., 5., 6.);
        out.push(r);

        let mut r = record("POINT");
        point(&mut r, 10, 1., 2., 3.);
        out.push(r);

        let mut r = record("CIRCLE");
        point(&mut r, 10, 1., 2., 0.);
        r.put(40, 2.5);
        out.push(r);

        let mut r = record("SHAPE");
        point(&mut r, 10, 1., 2., 0.);
        r.put(40, 1.25);
        r.put(70, 7);
        out.push(r);

        out.push(record("REPEAT"));

        let mut r = record("ENDREP");
        r.put(70, 1);
        r.put(71, 2);
        r.put(40, 3.);
        r.put(41, 4.);
        out.push(r);

        let mut r = record("TEXT");
        point(&mut r, 10, 1., 2., 0.);
        r.put(40, 2.);
        r.put(1, "hello");
        out.push(r);

        let mut r = record("ARC");
        point(&mut r, 10, 1., 2., 0.);
        r.put(40, 3.);
        r.put(50, 10.);
        r.put(51, 80.);
        out.push(r);

        let mut r = record("TRACE");
        for (i, code) in (10..=13).enumerate() {
            point(&mut r, code, i as f64, i as f64 + 1., 0.);
        }
        out.push(r);

        let mut r = record("LOAD");
        r.put(1, "shape.shx");
        out.push(r);

        let mut r = record("SOLID");
        for (i, code) in (10..=13).enumerate() {
            point(&mut r, code, i as f64, i as f64 + 1., 0.);
        }
        out.push(r);

        let mut r = record("BLOCK");
        point(&mut r, 10, 0., 0., 0.);
        r.put(1, "B");
        r.put(2, "description");
        out.push(r);

        out.push(record("ENDBLK"));

        let mut r = record("INSERT");
        r.put(2, "B");
        point(&mut r, 10, 1., 2., 0.);
        out.push(r);

        let mut r = record("ATTDEF");
        point(&mut r, 10, 1., 2., 0.);
        r.put(40, 2.);
        r.put(1, "value");
        r.put(2, "TAG");
        r.put(3, "prompt");
        r.put(70, 0);
        out.push(r);

        let mut r = record("ATTRIB");
        point(&mut r, 10, 1., 2., 0.);
        r.put(40, 2.);
        r.put(1, "value");
        r.put(2, "TAG");
        r.put(70, 0);
        out.push(r);

        out.push(record("SEQEND"));

        let mut r = record("JUMP");
        r.put(90, 1234);
        out.push(r);

        let mut r = record("POLYLINE");
        r.put(70, 1);
        r.put(40, 0.1);
        r.put(41, 0.2);
        r.put(71, 2);
        out.push(r);

        let mut r = record("VERTEX");
        point(&mut r, 10, 1., 2., 3.);
        r.put(40, 0.1);
        r.put(41, 0.2);
        r.put(42, 0.3);
        r.put(70, 1);
        r.put(50, 20.);
        r.put(71, 1);
        out.push(r);

        let mut r = record("3DLINE");
        point(&mut r, 10, 1., 2., 3.);
        point(&mut r, 11, 4., 5., 6.);
        out.push(r);

        let mut r = record("3DFACE");
        for (i, code) in (10..=13).enumerate() {
            point(&mut r, code, i as f64, i as f64 + 1., i as f64 + 2.);
        }
        r.put(70, 1);
        out.push(r);

        let mut r = record("DIMENSION");
        r.put(2, "B");
        point(&mut r, 10, 1., 2., 3.);
        point(&mut r, 11, 4., 5., 0.);
        r.put(70, 0);
        r.put(1, "<>" );
        r.put(3, "STANDARD");
        out.push(r);

        let mut r = record("VIEWPORT");
        point(&mut r, 10, 1., 2., 3.);
        r.put(40, 10.);
        r.put(41, 5.);
        r.put(69, 2);
        out.push(r);

        out
    }

    #[test]
    fn every_native_entity_code_round_trips_through_byte_codec() {
        let tables = tables();
        let records = native_records();
        let encoded = encode_entities(&records, &tables, 0, "ANSI_1252").unwrap();
        let decoded = decode_entities(&encoded, &tables, 0, "ANSI_1252").unwrap();
        let expected: Vec<&str> = records.iter().map(|r| r.kind.as_str()).collect();
        let actual: Vec<&str> = decoded.iter().map(|r| r.kind.as_str()).collect();
        assert_eq!(actual, expected);
        assert_eq!(decoded.len(), 24);
        assert_eq!(decoded.iter().find(|r| r.kind == "3DLINE").unwrap().number(30, 0.), 3.);
    }

    #[test]
    fn native_type_and_reverse_mapping_cover_all_codes() {
        let names = [
            "LINE", "POINT", "CIRCLE", "SHAPE", "REPEAT", "ENDREP", "TEXT", "ARC",
            "TRACE", "LOAD", "SOLID", "BLOCK", "ENDBLK", "INSERT", "ATTDEF", "ATTRIB",
            "SEQEND", "JUMP", "POLYLINE", "VERTEX", "3DLINE", "3DFACE", "DIMENSION",
            "VIEWPORT",
        ];
        for (code, name) in names.into_iter().enumerate() {
            let code = code as u8 + 1;
            assert_eq!(native_type(name), Some(code), "native code for {name}");
            assert_eq!(type_name(code), Some(name), "reverse code for {name}");
        }
        assert_eq!(native_type("MINSERT"), Some(14));
    }

    #[test]
    fn common_optional_fields_and_xdata_survive_round_trip() {
        let tables = tables();
        let mut line = record("LINE");
        point(&mut line, 10, 1., 2., 3.);
        point(&mut line, 11, 4., 5., 6.);
        point(&mut line, 210, 0., 1., 0.);
        line.put(62, 3);
        line.put(6, "CONTINUOUS");
        line.put(39, 1.5);
        line.put(5, "ABCD");
        line.put(67, 1);
        line.put(1001, "ACAD");
        line.put(1000, "hello");
        line.put(1010, "1.25");
        line.put(1020, "2.5");
        line.put(1030, "3.75");
        line.put(1070, -7);
        line.put(1071, 123456);
        let encoded = encode_entities(&[line], &tables, 0, "ANSI_1252").unwrap();
        let decoded = decode_entities(&encoded, &tables, 0, "ANSI_1252").unwrap();
        let line = &decoded[0];
        assert_eq!(line.integer(62, 0), 3);
        assert_eq!(line.string(5), "ABCD");
        assert_eq!(line.integer(67, 0), 1);
        assert_eq!(line.string(1001), "ACAD");
        assert_eq!(line.string(1000), "hello");
        assert_eq!(line.number(1010, 0.), 1.25);
        assert_eq!(line.number(1020, 0.), 2.5);
        assert_eq!(line.number(1030, 0.), 3.75);
        assert_eq!(line.integer(1070, 0), -7);
        assert_eq!(line.integer(1071, 0), 123456);
    }

    #[test]
    fn every_optional_geometry_field_survives_round_trip() {
        let tables = tables();
        let mut records = Vec::new();

        let mut text = record("TEXT");
        point(&mut text, 10, 1., 2., 3.);
        text.put(40, 2.);
        text.put(1, "text");
        text.put(50, 10.);
        text.put(41, 1.5);
        text.put(51, 20.);
        text.put(7, "ALT");
        text.put(71, 3);
        text.put(72, 4);
        text.put(11, 5.);
        text.put(21, 6.);
        text.put(31, 7.);
        text.put(73, 2);
        point(&mut text, 210, 0., 1., 0.);
        records.push(text);

        for kind in ["ATTRIB", "ATTDEF"] {
            let mut r = record(kind);
            point(&mut r, 10, 1., 2., 3.);
            r.put(40, 2.);
            r.put(1, "value");
            if kind == "ATTDEF" {
                r.put(3, "prompt");
            }
            r.put(2, "TAG");
            r.put(70, 1);
            r.put(50, 10.);
            r.put(41, 1.5);
            r.put(51, 20.);
            r.put(7, "ALT");
            r.put(71, 3);
            r.put(72, 4);
            r.put(11, 5.);
            r.put(21, 6.);
            r.put(31, 7.);
            r.put(74, 2);
            point(&mut r, 210, 0., 1., 0.);
            records.push(r);
        }

        let mut shape = record("SHAPE");
        point(&mut shape, 10, 1., 2., 3.);
        shape.put(40, 1.5);
        shape.put(70, 7);
        shape.put(50, 10.);
        shape.put(7, "ALT");
        shape.put(41, 2.);
        shape.put(51, 20.);
        records.push(shape);

        let mut insert = record("MINSERT");
        insert.put(2, "B");
        insert.put(66, 1);
        point(&mut insert, 10, 1., 2., 3.);
        insert.put(41, 1.5);
        insert.put(42, 2.5);
        insert.put(50, 10.);
        insert.put(43, 3.5);
        insert.put(70, 2);
        insert.put(71, 3);
        insert.put(44, 4.5);
        insert.put(45, 5.5);
        point(&mut insert, 210, 0., 1., 0.);
        records.push(insert);

        let mut polyline = record("POLYLINE");
        polyline.put(66, 1);
        polyline.put(70, 1);
        polyline.put(40, 0.1);
        polyline.put(41, 0.2);
        polyline.put(71, 2);
        polyline.put(72, 3);
        polyline.put(73, 4);
        polyline.put(74, 5);
        polyline.put(75, 6);
        point(&mut polyline, 210, 0., 1., 0.);
        records.push(polyline);

        let mut vertex = record("VERTEX");
        point(&mut vertex, 10, 1., 2., 3.);
        vertex.put(40, 0.1);
        vertex.put(41, 0.2);
        vertex.put(42, 0.3);
        vertex.put(70, 1);
        vertex.put(50, 10.);
        vertex.put(71, 2);
        vertex.put(72, 3);
        vertex.put(73, 4);
        vertex.put(74, 5);
        records.push(vertex);

        let mut dimension = record("DIMENSION");
        dimension.put(2, "B");
        point(&mut dimension, 10, 1., 2., 3.);
        point(&mut dimension, 11, 4., 5., 6.);
        point(&mut dimension, 12, 7., 8., 9.);
        dimension.put(70, 0);
        dimension.put(1, "dim");
        dimension.put(3, "STANDARD");
        point(&mut dimension, 13, 1., 2., 3.);
        point(&mut dimension, 14, 4., 5., 6.);
        point(&mut dimension, 15, 7., 8., 9.);
        point(&mut dimension, 16, 1., 2., 0.);
        dimension.put(40, 2.);
        dimension.put(50, 10.);
        dimension.put(52, 20.);
        dimension.put(53, 30.);
        point(&mut dimension, 210, 0., 1., 0.);
        records.push(dimension);

        let encoded = encode_entities(&records, &tables, 0, "ANSI_1252").unwrap();
        let decoded = decode_entities(&encoded, &tables, 0, "ANSI_1252").unwrap();
        assert_eq!(decoded.len(), records.len());
        assert_eq!(decoded[0].integer(71, 0), 3);
        assert_eq!(decoded[2].string(3), "prompt");
        assert_eq!(decoded[1].integer(74, 0), 2);
        assert_eq!(decoded[3].kind, "SHAPE");
        assert_eq!(decoded[4].kind, "INSERT");
        assert_eq!(decoded[4].integer(70, 0), 2);
        assert_eq!(decoded[5].integer(75, 0), 6);
        assert_eq!(decoded[6].integer(74, 0), 5);
        assert!((decoded[7].number(53, 0.) - 30.).abs() < 1e-12);
    }

    #[test]
    fn all_xdata_item_kinds_round_trip() {
        let tables = tables();
        let mut line = record("LINE");
        point(&mut line, 10, 1., 2., 3.);
        point(&mut line, 11, 4., 5., 6.);
        line.put(1001, "ACAD");
        line.put(1000, "text");
        line.put(1002, "{");
        line.put(1003, "0");
        line.put(1004, "DEADBEEF");
        line.put(1005, "ABCD");
        line.put(1010, 1.);
        line.put(1020, 2.);
        line.put(1030, 3.);
        line.put(1011, 4.);
        line.put(1021, 5.);
        line.put(1031, 6.);
        line.put(1012, 7.);
        line.put(1022, 8.);
        line.put(1032, 9.);
        line.put(1013, 10.);
        line.put(1023, 11.);
        line.put(1033, 12.);
        line.put(1040, 1.25);
        line.put(1041, 2.5);
        line.put(1042, 3.75);
        line.put(1070, -8);
        line.put(1071, 123456);
        let encoded = encode_entities(&[line], &tables, 0, "ANSI_1252").unwrap();
        let decoded = decode_entities(&encoded, &tables, 0, "ANSI_1252").unwrap();
        let line = &decoded[0];
        assert_eq!(line.string(1001), "ACAD");
        assert_eq!(line.string(1000), "text");
        assert_eq!(line.string(1002), "{");
        assert_eq!(line.string(1003), "0");
        assert_eq!(line.string(1004), "DEADBEEF");
        assert_eq!(line.string(1005), "ABCD");
        for (code, value) in [
            (1010, 1.),
            (1020, 2.),
            (1030, 3.),
            (1011, 4.),
            (1021, 5.),
            (1031, 6.),
            (1012, 7.),
            (1022, 8.),
            (1032, 9.),
            (1013, 10.),
            (1023, 11.),
            (1033, 12.),
            (1040, 1.25),
            (1041, 2.5),
            (1042, 3.75),
        ] {
            assert_eq!(line.number(code, f64::NAN), value, "XDATA code {code}");
        }
        assert_eq!(line.integer(1070, 0), -8);
        assert_eq!(line.integer(1071, 0), 123456);
    }

    #[test]
    fn compound_sequences_preserve_insert_and_polyline_owners() {
        let tables = tables();
        let base = 0x1000;
        let mut insert = record("INSERT");
        insert.put(2, "B");
        insert.put(66, 1);
        point(&mut insert, 10, 0., 0., 0.);
        let mut attrib = record("ATTRIB");
        point(&mut attrib, 10, 1., 2., 0.);
        attrib.put(1, "value");
        attrib.put(2, "TAG");
        let mut polyline = record("POLYLINE");
        polyline.put(70, 0);
        let mut vertex = record("VERTEX");
        point(&mut vertex, 10, 3., 4., 0.);
        let records = vec![
            insert,
            attrib,
            record("SEQEND"),
            polyline,
            vertex,
            record("SEQEND"),
        ];
        let encoded = encode_entities(&records, &tables, base, "ANSI_1252").unwrap();
        let mut starts = Vec::new();
        let mut offset = 0usize;
        while offset < encoded.len() {
            starts.push(offset);
            let size = u16::from_le_bytes([encoded[offset + 2], encoded[offset + 3]]) as usize;
            assert!(size >= 6);
            offset += size;
        }
        assert_eq!(starts.len(), records.len());
        let first_owner =
            u32::from_le_bytes(encoded[starts[2] + 8..starts[2] + 12].try_into().unwrap());
        let second_owner =
            u32::from_le_bytes(encoded[starts[5] + 8..starts[5] + 12].try_into().unwrap());
        assert_eq!(first_owner, base);
        assert_eq!(second_owner, base + starts[3] as u32);
        let decoded = decode_entities(&encoded, &tables, base, "ANSI_1252").unwrap();
        let kinds: Vec<_> = decoded.iter().map(|r| r.kind.as_str()).collect();
        assert_eq!(
            kinds,
            ["INSERT", "ATTRIB", "SEQEND", "POLYLINE", "VERTEX", "SEQEND"]
        );
        assert_eq!(decoded[0].integer(66, 0), 1);
        assert_eq!(decoded[3].integer(66, 0), 1);
    }

    #[test]
    fn minsert_uses_insert_wire_code_and_decodes_as_insert() {
        let tables = tables();
        let mut minsert = record("MINSERT");
        minsert.put(2, "B");
        point(&mut minsert, 10, 1., 2., 0.);
        minsert.put(70, 2);
        minsert.put(71, 3);
        let encoded = encode_entities(&[minsert], &tables, 0, "ANSI_1252").unwrap();
        assert_eq!(encoded[0] & 127, 14);
        let decoded = decode_entities(&encoded, &tables, 0, "ANSI_1252").unwrap();
        assert_eq!(decoded[0].kind, "INSERT");
        assert_eq!(decoded[0].integer(70, 0), 2);
        assert_eq!(decoded[0].integer(71, 0), 3);
    }

    #[test]
    fn optional_fields_set_the_native_option_masks() {
        let tables = tables();
        let mut records = Vec::new();

        let mut text = record("TEXT");
        point(&mut text, 10, 1., 2., 3.);
        text.put(1, "text");
        text.put(40, 2.);
        text.put(50, 10.);
        text.put(41, 2.);
        text.put(51, 10.);
        text.put(7, "ALT");
        text.put(71, 1);
        text.put(72, 1);
        text.put(73, 1);
        point(&mut text, 11, 4., 5., 6.);
        point(&mut text, 210, 0., 1., 0.);
        records.push((text, 0x01ffu16));

        for kind in ["ATTRIB", "ATTDEF"] {
            let mut r = record(kind);
            point(&mut r, 10, 1., 2., 3.);
            r.put(1, "value");
            r.put(2, "TAG");
            if kind == "ATTDEF" {
                r.put(3, "prompt");
            }
            r.put(40, 2.);
            r.put(50, 10.);
            r.put(41, 2.);
            r.put(51, 10.);
            r.put(7, "ALT");
            r.put(70, 1);
            r.put(71, 1);
            r.put(72, 1);
            r.put(74, 1);
            point(&mut r, 11, 4., 5., 6.);
            point(&mut r, 210, 0., 1., 0.);
            records.push((r, 0x03fe));
        }

        let mut shape = record("SHAPE");
        point(&mut shape, 10, 1., 2., 3.);
        shape.put(40, 2.);
        shape.put(50, 10.);
        shape.put(7, "ALT");
        shape.put(41, 2.);
        shape.put(51, 10.);
        records.push((shape, 0x000f));

        let mut insert = record("INSERT");
        insert.put(2, "B");
        point(&mut insert, 10, 1., 2., 3.);
        for (code, value) in [(41, 2.), (42, 2.), (50, 10.), (43, 2.), (44, 2.), (45, 2.)] {
            insert.put(code, value);
        }
        insert.put(70, 2);
        insert.put(71, 2);
        point(&mut insert, 210, 0., 1., 0.);
        records.push((insert, 0x01ff));

        let mut polyline = record("POLYLINE");
        polyline.put(70, 1);
        polyline.put(40, 1.);
        polyline.put(41, 1.);
        for code in 71..=75 {
            polyline.put(code, 1);
        }
        point(&mut polyline, 210, 0., 1., 0.);
        records.push((polyline, 0x01ff));

        let mut vertex = record("VERTEX");
        point(&mut vertex, 10, 1., 2., 3.);
        vertex.put(40, 1.);
        vertex.put(41, 1.);
        vertex.put(42, 1.);
        vertex.put(70, 1);
        vertex.put(50, 10.);
        for code in 71..=74 {
            vertex.put(code, 1);
        }
        records.push((vertex, 0x01ff));

        let mut dimension = record("DIMENSION");
        dimension.put(2, "B");
        point(&mut dimension, 10, 1., 2., 3.);
        point(&mut dimension, 11, 4., 5., 6.);
        point(&mut dimension, 12, 7., 8., 9.);
        dimension.put(70, 0);
        dimension.put(1, "dim");
        for code in [13, 14, 15, 16] {
            point(&mut dimension, code, 1., 2., 3.);
        }
        dimension.put(40, 2.);
        dimension.put(50, 10.);
        dimension.put(52, 20.);
        dimension.put(53, 30.);
        dimension.put(3, "STANDARD");
        point(&mut dimension, 210, 0., 1., 0.);
        records.push((dimension, 0xc7ff));

        let input: Vec<EntityRecord> = records.iter().map(|(r, _)| r.clone()).collect();
        let encoded = encode_entities(&input, &tables, 0, "ANSI_1252").unwrap();
        let mut offset = 0;
        for (expected, (_, options)) in records.iter().enumerate() {
            assert_eq!(encoded[offset], native_type(input[expected].kind.as_str()).unwrap());
            let actual = u16::from_le_bytes([encoded[offset + 6], encoded[offset + 7]]);
            assert_eq!(actual, *options, "option mask for {}", input[expected].kind);
            let size = u16::from_le_bytes([encoded[offset + 2], encoded[offset + 3]]) as usize;
            offset += size;
        }
        assert_eq!(offset, encoded.len());
    }

    #[test]
    fn vertex_extra_section_flag_omits_coordinates_without_desynchronizing() {
        let tables = tables();
        let mut vertex = record("VERTEX");
        // Bit 7 marks a face/extra-section vertex; bit 6 would request a
        // regular coordinate payload, so only bit 7 selects the omission.
        vertex.put(70, 128);
        vertex.put(40, 0.25);
        let encoded = encode_entities(&[vertex], &tables, 0, "ANSI_1252").unwrap();
        let options = u16::from_le_bytes([encoded[6], encoded[7]]);
        assert_eq!(options & 0x4000, 0x4000);
        let decoded = decode_entities(&encoded, &tables, 0, "ANSI_1252").unwrap();
        assert_eq!(decoded[0].integer(70, 0), 128);
        assert_eq!(decoded[0].number(10, -1.), 0.);
        assert_eq!(decoded[0].number(20, -1.), 0.);
        assert_eq!(decoded[0].number(30, -1.), 0.);
        assert_eq!(decoded[0].number(40, 0.), 0.25);
    }

    #[test]
    fn malformed_entity_frames_are_rejected() {
        let tables = tables();
        assert!(decode_entities(&[1, 0, 5, 0, 0], &tables, 0, "ANSI_1252").is_err());

        let mut line = record("LINE");
        point(&mut line, 10, 0., 0., 0.);
        point(&mut line, 11, 1., 1., 0.);
        let mut encoded = encode_entities(&[line], &tables, 0, "ANSI_1252").unwrap();
        encoded[2] = 0xff;
        encoded[3] = 0xff;
        assert!(decode_entities(&encoded, &tables, 0, "ANSI_1252").is_err());
    }

    #[test]
    fn polyline_extra_section_flag_is_skipped_as_an_opaque_tail() {
        let tables = tables();
        let mut polyline = record("POLYLINE");
        polyline.put(70, 0);
        let mut encoded = encode_entities(&[polyline], &tables, 0, "ANSI_1252").unwrap();
        let old_size = u16::from_le_bytes([encoded[2], encoded[3]]) as usize;
        let options = u16::from_le_bytes([encoded[6], encoded[7]]) | 0x8000;
        encoded[6..8].copy_from_slice(&options.to_le_bytes());
        encoded.splice(old_size - 2..old_size - 2, [0xAA, 0x55]);
        let new_size = old_size + 2;
        encoded[2..4].copy_from_slice(&(new_size as u16).to_le_bytes());
        let checksum = crc16(0xC0C1, &encoded[..new_size - 2]);
        encoded[new_size - 2..new_size].copy_from_slice(&checksum.to_le_bytes());
        let decoded = decode_entities(&encoded[..new_size], &tables, 0, "ANSI_1252").unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].kind, "POLYLINE");
    }
}
