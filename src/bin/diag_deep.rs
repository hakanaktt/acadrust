/// Deep diagnostic: parse the raw objects section to check entity data.
///
/// This writes a minimal DWG, then directly inspects the objects section
/// to verify entity data integrity.

use acadrust::document::CadDocument;
use acadrust::entities::*;
use acadrust::io::dwg::writer::object_writer::DwgObjectWriter;
use acadrust::types::DxfVersion;

fn main() {
    println!("═══════════════════ AC1018 (R2004) ═══════════════════");
    test_version(DxfVersion::AC1018, false);

    println!("\n═══════════════════ AC1032 (R2018) ═══════════════════");
    test_version(DxfVersion::AC1032, true);
}

fn test_version(version: DxfVersion, is_r2010_plus: bool) {
    let mut doc = CadDocument::new();
    doc.version = version;

    let line = Line::from_coords(0.0, 0.0, 0.0, 10.0, 5.0, 0.0);
    let entity_handle = doc.add_entity(EntityType::Line(line)).unwrap();

    let obj_writer = DwgObjectWriter::new(version, &doc);
    let (objects_data, handle_map) = obj_writer.write(&doc).unwrap();

    println!("Entity handle: {:#X}", entity_handle.value());

    // Check the model space block record
    let ms_handle = doc.block_records.iter()
        .find(|b| b.is_model_space())
        .map(|b| b.handle.value())
        .unwrap();
    let ms_offset = *handle_map.get(&ms_handle).unwrap() as usize;
    println!("Model Space block record: handle={:#X}, offset={}", ms_handle, ms_offset);

    // Parse the BLOCK_RECORD at ms_offset
    println!("\n--- Inspecting Model Space BLOCK_RECORD at offset {} ---", ms_offset);
    inspect_object(&objects_data, ms_offset, ms_handle, is_r2010_plus);

    // Check the BLOCK entity
    let blk_handle = doc.block_records.iter()
        .find(|b| b.is_model_space())
        .map(|b| b.block_entity_handle.value())
        .unwrap();
    let blk_offset = *handle_map.get(&blk_handle).unwrap() as usize;
    println!("\n--- Inspecting BLOCK entity at offset {} ---", blk_offset);
    inspect_object(&objects_data, blk_offset, blk_handle, is_r2010_plus);

    // Check the Line entity
    let ent_offset = *handle_map.get(&entity_handle.value()).unwrap() as usize;
    println!("\n--- Inspecting Line entity at offset {} ---", ent_offset);
    inspect_object(&objects_data, ent_offset, entity_handle.value(), is_r2010_plus);

    // Check the ENDBLK entity
    let endblk_handle = doc.block_records.iter()
        .find(|b| b.is_model_space())
        .map(|b| b.block_end_handle.value())
        .unwrap();
    let endblk_offset = *handle_map.get(&endblk_handle).unwrap() as usize;
    println!("\n--- Inspecting ENDBLK entity at offset {} ---", endblk_offset);
    inspect_object(&objects_data, endblk_offset, endblk_handle, is_r2010_plus);
}

fn inspect_object(data: &[u8], offset: usize, expected_handle: u64, is_r2010_plus: bool) {
    // Read MS (Modular Short) size
    let (ms_size, ms_bytes) = read_modular_short(data, offset);
    println!("  MS size: {} bytes (MS encoding took {} bytes)", ms_size, ms_bytes);

    let data_start = offset + ms_bytes;

    if is_r2010_plus {
        // Read MC (handle stream size in bits)
        let (mc_value, mc_bytes) = read_modular_char(data, data_start);
        println!("  MC handle stream size: {} bits ({} bytes MC encoding)", mc_value, mc_bytes);
        println!("  Total data bits: {}", ms_size * 8);
        println!("  Handle section should start at bit: {} + {} - {} = {}",
            (data_start + mc_bytes) * 8, ms_size * 8, mc_value,
            (data_start + mc_bytes) as i64 * 8 + ms_size as i64 * 8 - mc_value as i64);

        // Read first few bytes of object data
        let obj_start = data_start + mc_bytes;
        print_hex(data, obj_start, 20.min(ms_size as usize));
    } else {
        // No MC for pre-R2010
        print_hex(data, data_start, 20.min(ms_size as usize));
    }

    // Try to decode the object type from the first bytes
    let obj_data_start = if is_r2010_plus {
        let (_, mc_bytes) = read_modular_char(data, data_start);
        data_start + mc_bytes
    } else {
        data_start
    };

    // Object type is the first 2-bit code + byte for R2010+
    // or a BitShort for pre-R2010
    if is_r2010_plus && obj_data_start < data.len() {
        let first_2bits = (data[obj_data_start] >> 6) & 0x03;
        let next_byte = data[obj_data_start] & 0x3F;
        if first_2bits == 0 {
            let type_byte = (next_byte << 2) | (if obj_data_start + 1 < data.len() { data[obj_data_start + 1] >> 6 } else { 0 });
            println!("  Object type (2bit=0): {:#X} ({})", type_byte, type_name(type_byte as i16));
        } else {
            println!("  First 2 bits: {}, next 6 bits: {:#X}", first_2bits, next_byte);
        }
    } else if obj_data_start + 1 < data.len() {
        // BitShort: first 2 bits determine encoding
        let bb = (data[obj_data_start] >> 6) & 0x03;
        match bb {
            0 => {
                // Normal: 16-bit value
                let hi = ((data[obj_data_start] & 0x3F) as u16) << 10;
                let lo = ((data[obj_data_start + 1] as u16) << 2) |
                    if obj_data_start + 2 < data.len() { (data[obj_data_start + 2] >> 6) as u16 } else { 0 };
                let val = (hi | lo) as i16;
                println!("  Object type (BS bb=0): {:#X} ({}) — may be wrong due to bit alignment", val, type_name(val));
            }
            1 => {
                // Extended unsigned char
                let val = ((data[obj_data_start] & 0x3F) << 2) |
                    (data[obj_data_start + 1] >> 6);
                println!("  Object type (BS bb=1, unsigned char): {:#X} ({})", val, type_name(val as i16));
            }
            2 => {
                println!("  Object type (BS bb=2): 0x0000 (BlockControlObj)");
            }
            3 => {
                let val = ((data[obj_data_start] & 0x3F) as i16) << 2 |
                    (data[obj_data_start + 1] >> 6) as i16;
                println!("  Object type (BS bb=3, 256+UC): {}", 256 + val);
            }
            _ => unreachable!()
        }
    }
}

fn type_name(code: i16) -> &'static str {
    match code {
        0x04 => "BLOCK",
        0x05 => "ENDBLK",
        0x13 => "LINE",
        0x30 => "BLOCK_CONTROL",
        0x31 => "BLOCK_HEADER (block record)",
        0x32 => "LAYER_CONTROL",
        0x33 => "LAYER",
        0x38 => "LTYPE_CONTROL",
        0x39 => "LTYPE",
        _ => "unknown",
    }
}

fn read_modular_short(data: &[u8], offset: usize) -> (u32, usize) {
    let mut value = 0u32;
    let mut shift = 0;
    let mut pos = offset;
    loop {
        if pos + 1 >= data.len() { break; }
        let lo = data[pos] as u16;
        let hi = data[pos + 1] as u16;
        let word = lo | (hi << 8);
        pos += 2;

        value |= ((word & 0x7FFF) as u32) << shift;
        shift += 15;

        if word & 0x8000 == 0 {
            break;
        }
    }
    (value, pos - offset)
}

fn read_modular_char(data: &[u8], offset: usize) -> (i64, usize) {
    let mut value = 0i64;
    let mut shift = 0;
    let mut pos = offset;
    loop {
        if pos >= data.len() { break; }
        let byte = data[pos];
        pos += 1;

        if byte & 0x80 != 0 {
            value |= ((byte & 0x7F) as i64) << shift;
            shift += 7;
        } else {
            // Last byte
            if byte & 0x40 != 0 {
                // Negative
                value |= ((byte & 0x3F) as i64) << shift;
                value = -value;
            } else {
                value |= ((byte & 0x7F) as i64) << shift;
            }
            break;
        }
    }
    (value, pos - offset)
}

fn print_hex(data: &[u8], start: usize, count: usize) {
    let end = (start + count).min(data.len());
    print!("  Hex[{}..{}]: ", start, end);
    for i in start..end {
        print!("{:02X} ", data[i]);
    }
    println!();
}
