/// Diagnostic: debug the LZ77 AC21 decompression crash
use acadrust::document::CadDocument;
use acadrust::io::dwg::writer::DwgWriter;
use acadrust::types::DxfVersion;
use std::io::{Cursor, Read, Seek, SeekFrom};
use byteorder::{LittleEndian, ReadBytesExt};

fn main() {
    // Write an AC1021 file
    let mut doc = CadDocument::new();
    doc.version = DxfVersion::AC1021;
    let dwg_data = DwgWriter::write(&doc).unwrap();

    let mut f = Cursor::new(&dwg_data);

    // RS-decode metadata
    f.seek(SeekFrom::Start(0x80)).unwrap();
    let mut rs_block = vec![0u8; 0x400];
    f.read_exact(&mut rs_block).unwrap();
    let decoded = acadrust::io::dwg::reed_solomon::decode(&rs_block, 3 * 239, 3, 239);
    let compr_len = i32::from_le_bytes([decoded[24], decoded[25], decoded[26], decoded[27]]);
    let mut meta_buf = vec![0u8; 0x110];
    acadrust::io::dwg::compression::lz77_ac21::decompress(
        &decoded[32..32 + compr_len as usize], 0, compr_len as u32, &mut meta_buf
    ).unwrap();

    let mut mc = Cursor::new(&meta_buf);
    for _ in 0..7 { mc.read_u64::<LittleEndian>().unwrap(); }
    let pm_offset = mc.read_u64::<LittleEndian>().unwrap();
    for _ in 0..7 { mc.read_u64::<LittleEndian>().unwrap(); }
    for _ in 0..9 { mc.read_u64::<LittleEndian>().unwrap(); }
    let sm_id = mc.read_u64::<LittleEndian>().unwrap();

    // Read page map
    f.seek(SeekFrom::Start(pm_offset)).unwrap();
    f.read_i32::<LittleEndian>().unwrap();
    let pm_decomp_size = f.read_i32::<LittleEndian>().unwrap();
    let pm_comp_size = f.read_i32::<LittleEndian>().unwrap();
    f.read_i32::<LittleEndian>().unwrap();
    f.read_i32::<LittleEndian>().unwrap();
    let mut pm_comp_data = vec![0u8; pm_comp_size as usize];
    f.read_exact(&mut pm_comp_data).unwrap();
    let mut pm_data = vec![0u8; pm_decomp_size as usize];
    acadrust::io::dwg::compression::lz77_ac21::decompress(
        &pm_comp_data, 0, pm_comp_size as u32, &mut pm_data
    ).unwrap();

    // Find section map page
    let mut rc = Cursor::new(&pm_data);
    let mut running = 0x480i64;
    let mut sm_offset = 0i64;
    while (rc.position() as usize) + 8 <= pm_data.len() {
        let page_num = rc.read_i32::<LittleEndian>().unwrap();
        let size = rc.read_i32::<LittleEndian>().unwrap();
        if page_num as u64 == sm_id { sm_offset = running; }
        running += size as i64;
    }

    // Read section map header + data
    f.seek(SeekFrom::Start(sm_offset as u64)).unwrap();
    f.read_i32::<LittleEndian>().unwrap(); // type
    let hdr_decomp = f.read_i32::<LittleEndian>().unwrap();
    let hdr_comp = f.read_i32::<LittleEndian>().unwrap();
    f.read_i32::<LittleEndian>().unwrap(); // comp_type
    f.read_i32::<LittleEndian>().unwrap(); // checksum
    let mut comp_data = vec![0u8; hdr_comp as usize];
    f.read_exact(&mut comp_data).unwrap();

    println!("Section map: compressed={} decompressed={}", hdr_comp, hdr_decomp);
    println!("Compressed data ({} bytes): {:02X?}", comp_data.len(), &comp_data[..comp_data.len().min(100)]);

    // Manual trace of decompression
    println!("\n=== Manual decompression trace ===");
    debug_decompress(&comp_data, hdr_comp as u32, hdr_decomp as usize);
}

fn debug_decompress(source: &[u8], length: u32, decompressed_size: usize) {
    let mut source_index: u32 = 0;
    let mut op_code: u32 = source[0] as u32;
    source_index += 1;
    let mut dest_index: u32 = 0;
    let end_index = length;
    let mut step = 0;
    let mut lit_length: u32 = 0;

    println!("Initial opcode: 0x{:02X}", op_code);

    if (op_code & 0xF0) == 0x20 {
        source_index += 3;
        lit_length = source[(source_index - 1) as usize] as u32 & 7;
        println!("Initial 0x20 path: skip 3, literal_length = {}", lit_length);
    } else {
        // read_literal_length path
        lit_length = op_code + 8;
        if lit_length == 0x17 {
            let n = source[source_index as usize] as u32;
            source_index += 1;
            lit_length += n;
            // More extension handling would go here
        }
        println!("Literal path: literal_length = {}", lit_length);
    }

    // Copy initial literals
    println!("Step {}: copy {} literals from source[{}] to dest[{}]", step, lit_length, source_index, dest_index);
    source_index += lit_length;
    dest_index += lit_length;
    step += 1;

    // Main loop
    while source_index < end_index {
        // Read instruction opcode
        let opc = source[source_index as usize] as u32;
        source_index += 1;
        
        let case = opc >> 4;
        
        let (src_offset, match_len, trailing_lits);
        
        match case {
            0 => {
                let base_len = (opc & 0xF) + 0x13;
                let next1 = source[source_index as usize] as u32;
                source_index += 1;
                let next2 = source[source_index as usize] as u32;
                source_index += 1;
                let extra_len = (next2 >> 3) & 0x10;
                match_len = base_len + extra_len;
                src_offset = next1 + ((next2 & 0x78) << 5) + 1;
                trailing_lits = next2 & 7;
                println!("Step {}: Case 0 opcode=0x{:02X} next1=0x{:02X} next2=0x{:02X} => offset={} len={} lits={}", 
                    step, opc, next1, next2, src_offset, match_len, trailing_lits);
            }
            1 => {
                match_len = (opc & 0xF) + 3;
                let next1 = source[source_index as usize] as u32;
                source_index += 1;
                let next2 = source[source_index as usize] as u32;
                source_index += 1;
                src_offset = next1 + ((next2 & 0xF8) << 5) + 1;
                trailing_lits = next2 & 7;
                println!("Step {}: Case 1 opcode=0x{:02X} next1=0x{:02X} next2=0x{:02X} => offset={} len={} lits={}", 
                    step, opc, next1, next2, src_offset, match_len, trailing_lits);
            }
            2 => {
                let next1 = source[source_index as usize] as u32;
                source_index += 1;
                let next2 = source[source_index as usize] as u32;
                source_index += 1;
                let raw_offset = next1 | (next2 << 8);
                let len_low = opc & 7;
                
                if (opc & 8) == 0 {
                    // Variant A
                    let next3 = source[source_index as usize] as u32;
                    source_index += 1;
                    match_len = (next3 & 0xF8) + len_low;
                    src_offset = raw_offset; // NO +1
                    trailing_lits = next3 & 7;
                    println!("Step {}: Case 2A opcode=0x{:02X} next1=0x{:02X} next2=0x{:02X} next3=0x{:02X} => offset={} len={} lits={}", 
                        step, opc, next1, next2, next3, src_offset, match_len, trailing_lits);
                } else {
                    // Variant B
                    let next3 = source[source_index as usize] as u32;
                    source_index += 1;
                    let next4 = source[source_index as usize] as u32;
                    source_index += 1;
                    src_offset = raw_offset + 1;
                    let len_ext = (next3 << 3) + len_low;
                    match_len = ((next4 & 0xF8) << 8) + len_ext + 0x100;
                    trailing_lits = next4 & 7;
                    println!("Step {}: Case 2B opcode=0x{:02X} next1..4=0x{:02X} 0x{:02X} 0x{:02X} 0x{:02X} => offset={} len={} lits={}",
                        step, opc, next1, next2, next3, next4, src_offset, match_len, trailing_lits);
                }
            }
            _ => {
                // Default case (case >= 3)
                match_len = opc >> 4;
                let low_nib = opc & 0x0F;
                let next1 = source[source_index as usize] as u32;
                source_index += 1;
                src_offset = ((next1 & 0xF8) << 1) + low_nib + 1;
                trailing_lits = next1 & 7;
                println!("Step {}: Default opcode=0x{:02X} next=0x{:02X} => offset={} len={} lits={}", 
                    step, opc, next1, src_offset, match_len, trailing_lits);
            }
        }

        // Check if this back-reference is valid
        if src_offset > dest_index {
            println!("  ** BUG: src_offset {} > dest_index {} - would crash!", src_offset, dest_index);
            println!("  source_index={}, end_index={}", source_index, end_index);
            return;
        }

        dest_index += match_len;
        
        // Handle trailing literals
        if trailing_lits > 0 {
            println!("  -> copy_back {} bytes from offset -{}, dest now {}, then {} inline lits", match_len, src_offset, dest_index, trailing_lits);
            source_index += trailing_lits;
            dest_index += trailing_lits;
        } else {
            println!("  -> copy_back {} bytes from offset -{}, dest now {}", match_len, src_offset, dest_index);
            // Check if next byte starts a new opcode or literal length
            if source_index < end_index {
                let next_opc = source[source_index as usize] as u32;
                if next_opc >> 4 == 0 {
                    // This is a literal length indicator, NOT a case 0 match
                    let lit_len = next_opc + 8;
                    println!("  -> literal length indicator: 0x{:02X} => {} bytes", next_opc, lit_len);
                    source_index += 1;
                    source_index += lit_len;
                    dest_index += lit_len;
                }
            }
        }

        step += 1;
        if step > 200 {
            println!("... stopping after 200 steps");
            break;
        }
    }
    
    println!("Final dest_index: {}, expected: {}", dest_index, decompressed_size);
}
