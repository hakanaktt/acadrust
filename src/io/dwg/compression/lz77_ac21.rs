//! LZ77 AC21 compression and decompression.
//!
//! This is the LZ77 variant used exclusively in AC1021 (R2007) DWG files.
//! It uses a different opcode format than the AC18 variant.
//!
//! Mirrors ACadSharp's `DwgLZ77AC21Decompressor` and `DwgLZ77AC21Compressor`.

use crate::error::Result;

// ---------------------------------------------------------------------------
// Decompressor
// ---------------------------------------------------------------------------

/// Decompressor for the LZ77 AC21 variant.
pub struct Lz77Ac21Decompressor;

impl super::Decompressor for Lz77Ac21Decompressor {
    fn decompress(&self, source: &[u8], decompressed_size: usize) -> Result<Vec<u8>> {
        let mut buffer = vec![0u8; decompressed_size];
        decompress(source, 0, source.len() as u32, &mut buffer)?;
        Ok(buffer)
    }
}

/// State for the AC21 decompressor.
struct DecompressState {
    source_offset: u32,
    length: u32,
    source_index: u32,
    op_code: u32,
}

/// Decompress an AC21 compressed buffer.
///
/// Faithful port of ACadSharp `DwgLZ77AC21Decompressor.Decompress`.
///
/// # Arguments
/// * `source` â€” The compressed data buffer
/// * `initial_offset` â€” Starting offset into the source buffer
/// * `length` â€” Number of bytes of compressed data
/// * `buffer` â€” Pre-allocated output buffer (must be the decompressed size)
pub fn decompress(
    source: &[u8],
    initial_offset: u32,
    length: u32,
    buffer: &mut [u8],
) -> Result<()> {
    let mut state = DecompressState {
        source_offset: 0,
        length: 0,
        source_index: initial_offset,
        op_code: source[initial_offset as usize] as u32,
    };

    let mut dest_index: u32 = 0;
    let end_index = state.source_index + length;

    state.source_index += 1;

    if state.source_index >= end_index {
        return Ok(());
    }

    if (state.op_code & 0xF0) == 0x20 {
        state.source_index += 3;
        state.length = source[state.source_index as usize - 1] as u32;
        state.length &= 7;
    }

    while state.source_index < end_index {
        next_index(source, buffer, &mut dest_index, &mut state)?;

        if state.source_index >= end_index {
            break;
        }

        dest_index =
            copy_decompressed_chunks(source, end_index, buffer, dest_index, &mut state)?;
    }

    Ok(())
}

fn next_index(
    source: &[u8],
    dest: &mut [u8],
    index: &mut u32,
    state: &mut DecompressState,
) -> Result<()> {
    if state.length == 0 {
        read_literal_length(source, state);
    }

    copy_bytes_from_source(
        source,
        state.source_index,
        dest,
        *index,
        state.length,
    );

    state.source_index += state.length;
    *index += state.length;

    Ok(())
}

fn copy_decompressed_chunks(
    src: &[u8],
    end_index: u32,
    dst: &mut [u8],
    mut dest_index: u32,
    state: &mut DecompressState,
) -> Result<u32> {
    state.length = 0;
    state.op_code = src[state.source_index as usize] as u32;
    state.source_index += 1;

    read_instructions(src, state);

    loop {
        copy_bytes_backward(dst, dest_index, state.length, state.source_offset);

        dest_index += state.length;

        state.length = state.op_code & 0x07;

        if state.length != 0 || state.source_index >= end_index {
            break;
        }

        state.op_code = src[state.source_index as usize] as u32;
        state.source_index += 1;

        if state.op_code >> 4 == 0 {
            break;
        }

        if state.op_code >> 4 == 15 {
            state.op_code &= 15;
        }

        read_instructions(src, state);
    }

    Ok(dest_index)
}

fn read_instructions(buffer: &[u8], state: &mut DecompressState) {
    match state.op_code >> 4 {
        0 => {
            state.length = (state.op_code & 0xF) + 0x13;
            state.source_offset = buffer[state.source_index as usize] as u32;
            state.source_index += 1;
            state.op_code = buffer[state.source_index as usize] as u32;
            state.source_index += 1;
            state.length = ((state.op_code >> 3) & 0x10) + state.length;
            state.source_offset = ((state.op_code & 0x78) << 5) + 1 + state.source_offset;
        }
        1 => {
            state.length = (state.op_code & 0xF) + 3;
            state.source_offset = buffer[state.source_index as usize] as u32;
            state.source_index += 1;
            state.op_code = buffer[state.source_index as usize] as u32;
            state.source_index += 1;
            state.source_offset = ((state.op_code & 0xF8) << 5) + 1 + state.source_offset;
        }
        2 => {
            state.source_offset = buffer[state.source_index as usize] as u32;
            state.source_index += 1;
            state.source_offset = ((buffer[state.source_index as usize] as u32) << 8 & 0xFF00)
                | state.source_offset;
            state.source_index += 1;
            state.length = state.op_code & 7;
            if (state.op_code & 8) == 0 {
                state.op_code = buffer[state.source_index as usize] as u32;
                state.source_index += 1;
                state.length = (state.op_code & 0xF8) + state.length;
            } else {
                state.source_offset += 1;
                state.length =
                    ((buffer[state.source_index as usize] as u32) << 3) + state.length;
                state.source_index += 1;
                state.op_code = buffer[state.source_index as usize] as u32;
                state.source_index += 1;
                state.length = ((state.op_code & 0xF8) << 8) + state.length + 0x100;
            }
        }
        _ => {
            // Default: opcode >> 4 >= 3
            state.length = state.op_code >> 4;
            state.source_offset = state.op_code & 0x0F;
            state.op_code = buffer[state.source_index as usize] as u32;
            state.source_index += 1;
            state.source_offset = ((state.op_code & 0xF8) << 1) + state.source_offset + 1;
        }
    }
}

fn read_literal_length(buffer: &[u8], state: &mut DecompressState) {
    state.length = state.op_code + 8;
    if state.length == 0x17 {
        let mut n = buffer[state.source_index as usize] as u32;
        state.source_index += 1;
        state.length += n;

        if n == 0xFF {
            loop {
                n = buffer[state.source_index as usize] as u32;
                state.source_index += 1;
                n |= (buffer[state.source_index as usize] as u32) << 8;
                state.source_index += 1;
                state.length += n;

                if n != 0xFFFF {
                    break;
                }
            }
        }
    }
}

/// Copy bytes from source to destination (literal copy).
fn copy_bytes_from_source(src: &[u8], src_index: u32, dst: &mut [u8], dst_index: u32, length: u32) {
    let si = src_index as usize;
    let di = dst_index as usize;
    let len = length as usize;

    // Use chunks of 32 for efficiency where possible
    let mut remaining = len;
    let mut s = si;
    let mut d = di;

    while remaining >= 32 {
        dst[d..d + 32].copy_from_slice(&src[s..s + 32]);
        s += 32;
        d += 32;
        remaining -= 32;
    }

    if remaining > 0 {
        dst[d..d + remaining].copy_from_slice(&src[s..s + remaining]);
    }
}

/// Copy bytes backward within the destination buffer (for back-references).
fn copy_bytes_backward(dst: &mut [u8], dst_index: u32, length: u32, src_offset: u32) {
    let mut initial_index = (dst_index - src_offset) as usize;
    let max_index = initial_index + length as usize;
    let mut di = dst_index as usize;

    while initial_index < max_index {
        dst[di] = dst[initial_index];
        di += 1;
        initial_index += 1;
    }
}

// ---------------------------------------------------------------------------
// Compressor
// ---------------------------------------------------------------------------

/// Compressor for the LZ77 AC21 variant.
///
/// Produces compressed output that matches the AC21 opcode table,
/// invertible by [`Lz77Ac21Decompressor`].
pub struct Lz77Ac21Compressor;

impl super::Compressor for Lz77Ac21Compressor {
    fn compress(&self, source: &[u8], offset: usize, total_size: usize) -> Result<Vec<u8>> {
        compress_ac21(&source[offset..offset + total_size])
    }
}

/// A pending match found during scanning.
struct PendingMatch {
    /// Number of literal bytes before the match.
    literal_len: usize,
    /// Position in input where the match starts.
    match_pos: usize,
    /// Back-reference distance (1-based).
    distance: usize,
    /// Match length.
    match_len: usize,
}

/// Compute a 16-bit hash for a 3-byte sequence at `pos`.
#[inline]
fn hash3(data: &[u8], pos: usize) -> usize {
    let h = (data[pos] as u32)
        | ((data[pos + 1] as u32) << 8)
        | ((data[pos + 2] as u32) << 16);
    ((h.wrapping_mul(0x9E3779B1)) >> 16) as usize & 0xFFFF
}

/// Compress a buffer using the LZ77 AC21 encoding.
///
/// The output is structured so that [`decompress`] can recover the original
/// data exactly: `decompress(compress(data)) == data`.
fn compress_ac21(input: &[u8]) -> Result<Vec<u8>> {
    let len = input.len();
    if len == 0 {
        return Ok(vec![0x20, 0x00, 0x00, 0x00]);
    }

    // -----------------------------------------------------------------------
    // Pass 1: find all matches using a hash table
    // -----------------------------------------------------------------------
    let mut hash_table = vec![u32::MAX; 0x10000];
    let mut matches: Vec<PendingMatch> = Vec::new();
    let mut pos: usize = 0;
    let mut literal_start: usize = 0;

    while pos + 2 < len {
        let h = hash3(input, pos);
        let prev = hash_table[h];
        hash_table[h] = pos as u32;

        if prev == u32::MAX {
            pos += 1;
            continue;
        }

        let prev_pos = prev as usize;
        let distance = pos - prev_pos;
        if distance == 0 || distance > 0xFFFF {
            pos += 1;
            continue;
        }

        // Check 3-byte match
        if input[prev_pos] != input[pos]
            || input[prev_pos + 1] != input[pos + 1]
            || input[prev_pos + 2] != input[pos + 2]
        {
            pos += 1;
            continue;
        }

        // Extend match length (non-overlapping part)
        let mut match_len: usize = 3;
        while pos + match_len < len
            && prev_pos + match_len < pos
            && input[prev_pos + match_len] == input[pos + match_len]
        {
            match_len += 1;
        }

        // Also allow overlapping match extension (repeating pattern)
        while pos + match_len < len
            && input[prev_pos + (match_len % distance)] == input[pos + match_len]
        {
            match_len += 1;
        }

        if !can_encode_match(distance, match_len) {
            pos += 1;
            continue;
        }

        matches.push(PendingMatch {
            literal_len: pos - literal_start,
            match_pos: pos,
            distance,
            match_len,
        });

        // Update hash for positions within the match
        for i in 1..match_len {
            if pos + i + 2 < len {
                let h2 = hash3(input, pos + i);
                hash_table[h2] = (pos + i) as u32;
            }
        }

        pos += match_len;
        literal_start = pos;
    }

    let trailing_literal_len = len - literal_start;

    // -----------------------------------------------------------------------
    // Pass 2: serialize into the AC21 opcode stream
    // -----------------------------------------------------------------------
    let mut out = Vec::with_capacity(len);

    if matches.is_empty() {
        // No matches - emit everything as a single literal block
        emit_initial_literal_block(input, 0, len, &mut out);
        return Ok(out);
    }

    // Emit initial literal block (before first match)
    emit_initial_literal_block(input, 0, matches[0].literal_len, &mut out);

    // Emit each match followed by its trailing literals
    for (i, m) in matches.iter().enumerate() {
        // Determine how many literal bytes follow this match
        let lit_after = if i + 1 < matches.len() {
            matches[i + 1].literal_len
        } else {
            trailing_literal_len
        };
        let lit_after_start = m.match_pos + m.match_len;

        // When the previous match had 0 trailing literals (inline_lits == 0),
        // the decompressor's read_instructions loop reads the next byte and
        // checks `op_code >> 4 == 0` to decide if it's a literal-length
        // indicator. Case 0 opcodes also have top nibble 0, so they would
        // be misinterpreted. We must forbid Case 0 in that situation.
        let prev_had_zero_lits = if i > 0 {
            let prev_lit = m.literal_len; // lits before THIS match = lits after previous
            let prev_inline = if prev_lit <= 7 { prev_lit } else { 0 };
            prev_inline == 0
        } else {
            false
        };

        emit_match_and_literals(input, m.distance, m.match_len, lit_after_start, lit_after, prev_had_zero_lits, &mut out);
    }

    Ok(out)
}

/// Check if a match with given offset and length can be encoded.
fn can_encode_match(offset: usize, length: usize) -> bool {
    if length < 3 {
        return false;
    }
    if offset == 0 || offset > 0xFFFF {
        return false;
    }
    true
}

// ---------------------------------------------------------------------------
// Initial literal block emission
// ---------------------------------------------------------------------------

/// Emit the initial literal block at the start of the compressed stream.
///
/// The decompressor's initial flow:
/// 1. Read initial opcode byte
/// 2. If `(opcode & 0xF0) == 0x20`: skip source_index by 3, read
///    `length = source[source_index-1] & 7`
/// 3. Otherwise: enter `next_index` -> `read_literal_length` if length==0
///    which sets length = opcode + 8 (with extension for opcode == 0x0F)
fn emit_initial_literal_block(input: &[u8], start: usize, count: usize, out: &mut Vec<u8>) {
    if count == 0 {
        // Use 0x20 path with length=0
        out.push(0x20);
        out.push(0x00);
        out.push(0x00);
        out.push(0x00);
        return;
    }

    if count <= 7 {
        // Use 0x20 path: length = 3rd_byte & 7
        out.push(0x20);
        out.push(0x00);
        out.push(0x00);
        out.push(count as u8);
        out.extend_from_slice(&input[start..start + count]);
        return;
    }

    // Use read_literal_length path: length = opcode + 8
    // opcode = count - 8, must have opcode < 0x10 for simple case
    let base = count - 8;
    if base < 0x0F {
        out.push(base as u8);
        out.extend_from_slice(&input[start..start + count]);
        return;
    }

    // base >= 0x0F: opcode = 0x0F triggers extended literal length
    // read_literal_length: length = 0x17, then read extension byte(s)
    out.push(0x0F);
    let remaining = count - 0x17; // count >= 23 since base >= 15
    emit_extended_length_bytes(remaining, out);
    out.extend_from_slice(&input[start..start + count]);
}

/// Emit extension bytes for a long literal length.
fn emit_extended_length_bytes(remaining: usize, out: &mut Vec<u8>) {
    if remaining <= 0xFE {
        out.push(remaining as u8);
    } else {
        out.push(0xFF);
        let mut r = remaining - 0xFF;
        while r >= 0xFFFF {
            out.push(0xFF);
            out.push(0xFF);
            r -= 0xFFFF;
        }
        out.push((r & 0xFF) as u8);
        out.push(((r >> 8) & 0xFF) as u8);
    }
}

// ---------------------------------------------------------------------------
// Match + trailing literal emission
// ---------------------------------------------------------------------------

/// Emit a back-reference match instruction followed by trailing literal bytes.
fn emit_match_and_literals(
    input: &[u8],
    distance: usize,
    match_len: usize,
    lit_start: usize,
    lit_len: usize,
    forbid_case0: bool,
    out: &mut Vec<u8>,
) {
    let adj_offset = distance - 1; // 0-based for encoding

    // How many trailing literal bytes to encode in the opcode's low 3 bits
    let inline_lits = if lit_len <= 7 { lit_len } else { 0 };

    // Try short default encoding first (most compact)
    if try_emit_default(adj_offset, match_len, inline_lits, out) {
        emit_lit_tail(input, lit_start, lit_len, out);
        return;
    }
    if try_emit_case1(adj_offset, match_len, inline_lits, out) {
        emit_lit_tail(input, lit_start, lit_len, out);
        return;
    }
    // Case 0 opcodes have top nibble 0, which the decompressor interprets as
    // a literal-length indicator when the previous match had 0 inline literals.
    // Skip Case 0 when it would be misinterpreted.
    if !forbid_case0 {
        if try_emit_case0(adj_offset, match_len, inline_lits, out) {
            emit_lit_tail(input, lit_start, lit_len, out);
            return;
        }
    }
    // Case 2 always works for offset <= 0xFFFF
    // Note: case 2 uses different offset encoding (variant A has no +1,
    // variant B adds +1), so we pass the raw distance, not adj_offset.
    emit_case2(distance, match_len, inline_lits, out);
    emit_lit_tail(input, lit_start, lit_len, out);
}

/// Default opcode (top nibble >= 3): short match, offset 1..512, length 3..15.
///
/// Decompressor:
///   length = opcode >> 4
///   source_offset = (opcode & 0x0F) + ((next & 0xF8) << 1) + 1
///   trailing_lits = next & 0x07
fn try_emit_default(adj_offset: usize, match_len: usize, lit_bits: usize, out: &mut Vec<u8>) -> bool {
    if match_len < 3 || match_len > 15 {
        return false;
    }
    let low_nib = adj_offset & 0x0F;
    let remainder = adj_offset - low_nib;
    // remainder = (next & 0xF8) << 1, max = 0xF8 << 1 = 0x1F0
    if remainder > 0x1F0 {
        return false;
    }
    let next_high = ((remainder >> 1) & 0xF8) as u8;
    // Verify round-trip
    if ((next_high as usize & 0xF8) << 1) + low_nib != adj_offset {
        return false;
    }

    let opcode = ((match_len as u8) << 4) | (low_nib as u8);
    let next_byte = next_high | (lit_bits as u8);
    out.push(opcode);
    out.push(next_byte);
    true
}

/// Case 1 (top nibble == 1): medium match, offset 1..8193, length 3..18.
///
/// Decompressor:
///   length = (opcode & 0xF) + 3
///   source_offset = next1 + ((next2 & 0xF8) << 5) + 1
///   trailing_lits = next2 & 0x07
fn try_emit_case1(adj_offset: usize, match_len: usize, lit_bits: usize, out: &mut Vec<u8>) -> bool {
    if match_len < 3 || match_len > 18 {
        return false;
    }
    let len_nibble = match_len - 3;
    if len_nibble > 0x0F {
        return false;
    }
    let next1 = (adj_offset & 0xFF) as u8;
    let remainder = adj_offset - (next1 as usize);
    // remainder = (next2 & 0xF8) << 5, max = 0xF8 << 5 = 0x1F00
    if remainder > 0x1F00 {
        return false;
    }
    let next2_high = ((remainder >> 5) & 0xF8) as u8;
    if (next1 as usize) + ((next2_high as usize & 0xF8) << 5) != adj_offset {
        return false;
    }

    let opcode = 0x10 | (len_nibble as u8);
    let next2 = next2_high | (lit_bits as u8);
    out.push(opcode);
    out.push(next1);
    out.push(next2);
    true
}

/// Case 0 (top nibble == 0): long match, offset 1..4096, length 19..50.
///
/// Decompressor:
///   length = (opcode & 0xF) + 0x13 + ((next2 >> 3) & 0x10)
///   source_offset = next1 + ((next2 & 0x78) << 5) + 1
///   trailing_lits = next2 & 0x07
fn try_emit_case0(adj_offset: usize, match_len: usize, lit_bits: usize, out: &mut Vec<u8>) -> bool {
    if match_len < 19 || match_len > 50 {
        return false;
    }
    let next1 = (adj_offset & 0xFF) as u8;
    let offset_rem = adj_offset - (next1 as usize);
    if offset_rem > (0x78usize << 5) {
        return false;
    }
    let next2_offset_bits = ((offset_rem >> 5) & 0x78) as u8;
    if (next1 as usize) + ((next2_offset_bits as usize & 0x78) << 5) != adj_offset {
        return false;
    }

    let (extra_bit, base_len) = if match_len >= 35 {
        (true, match_len - 0x13 - 0x10)
    } else {
        (false, match_len - 0x13)
    };
    if base_len > 0x0F {
        return false;
    }

    let opcode = base_len as u8;
    let extra_byte_bit: u8 = if extra_bit { 0x80 } else { 0 };
    let next2 = extra_byte_bit | next2_offset_bits | (lit_bits as u8);

    // Final verification
    let check_len = (opcode & 0x0F) as usize + 0x13
        + if (next2 >> 3) & 0x10 != 0 { 0x10 } else { 0 };
    let check_off = (next1 as usize) + (((next2 & 0x78) as usize) << 5);
    if check_len != match_len || check_off != adj_offset {
        return false;
    }

    out.push(opcode);
    out.push(next1);
    out.push(next2);
    true
}

/// Case 2 (top nibble == 2): variable-length match, offset 1..65536.
///
/// Variant A (bit3=0): length <= 255, source_offset = next1 | (next2 << 8) [NO +1]
/// Variant B (bit3=1): length >= 256, source_offset = (next1 | (next2 << 8)) + 1 [+1]
fn emit_case2(distance: usize, match_len: usize, lit_bits: usize, out: &mut Vec<u8>) {
    // Variant A first (shorter encoding)
    // Decompressor: source_offset = next1 | (next2 << 8), NO +1 added
    // So we store the raw distance directly
    if match_len <= 255 {
        let len_low3 = match_len & 7;
        let len_high = match_len - len_low3;
        if len_high <= 0xF8 {
            let opcode = 0x20u8 | (len_low3 as u8);
            let next1 = (distance & 0xFF) as u8;
            let next2 = ((distance >> 8) & 0xFF) as u8;
            let next3 = (len_high as u8 & 0xF8) | (lit_bits as u8);
            out.push(opcode);
            out.push(next1);
            out.push(next2);
            out.push(next3);
            return;
        }
    }

    // Variant B for long matches (>= 256)
    // Decompressor: source_offset = (next1 | (next2 << 8)) + 1
    // So we store distance - 1
    let stored_offset = distance - 1;
    let adj_len = match_len.saturating_sub(0x100);
    let len_low3 = adj_len & 7;
    let rem_len = adj_len - len_low3;
    let next3_val = ((rem_len & 0x7F8) >> 3) as u8;
    let next4_high = ((rem_len >> 11) & 0xF8) as u8;

    let opcode = 0x28u8 | (len_low3 as u8);
    let next1 = (stored_offset & 0xFF) as u8;
    let next2 = ((stored_offset >> 8) & 0xFF) as u8;
    let next4 = next4_high | (lit_bits as u8);

    out.push(opcode);
    out.push(next1);
    out.push(next2);
    out.push(next3_val);
    out.push(next4);
}

/// Emit trailing literal bytes after a match instruction.
///
/// If `lit_len <= 7`, the inline count was already encoded in the opcode.
/// If `lit_len > 7`, inline_lits was set to 0. The decompressor sees
/// op_code & 7 == 0, reads next byte; if top nibble == 0, enters
/// `read_literal_length` which gives count = byte + 8.
fn emit_lit_tail(
    input: &[u8],
    lit_start: usize,
    lit_len: usize,
    out: &mut Vec<u8>,
) {
    if lit_len == 0 {
        return;
    }

    if lit_len <= 7 {
        // Inline literals - just emit the bytes
        out.extend_from_slice(&input[lit_start..lit_start + lit_len]);
        return;
    }

    // lit_len > 7: emit a literal length indicator byte (top nibble == 0)
    let count = lit_len;
    if count <= 22 {
        // count - 8 fits in 0..14
        out.push((count - 8) as u8);
    } else {
        // Extended: byte = 0x0F, then extension bytes
        out.push(0x0F);
        let remaining = count - 0x17;
        emit_extended_length_bytes(remaining, out);
    }
    out.extend_from_slice(&input[lit_start..lit_start + count]);
}

#[cfg(test)]
mod tests {
    use super::super::{Compressor, Decompressor};

    #[test]
    fn test_lz77_ac21_compress_empty() {
        let compressor = super::Lz77Ac21Compressor;
        let compressed = compressor.compress(&[], 0, 0).unwrap();
        let decompressor = super::Lz77Ac21Decompressor;
        let decompressed = decompressor.decompress(&compressed, 0).unwrap();
        assert_eq!(decompressed.len(), 0);
    }

    #[test]
    fn test_lz77_ac21_compress_small_literal() {
        let data = vec![1, 2, 3, 4, 5];
        let compressor = super::Lz77Ac21Compressor;
        let compressed = compressor.compress(&data, 0, data.len()).unwrap();
        let decompressor = super::Lz77Ac21Decompressor;
        let decompressed = decompressor.decompress(&compressed, data.len()).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_lz77_ac21_compress_repeated_pattern() {
        let mut data = Vec::new();
        for i in 0..128 {
            data.push((i % 16) as u8);
        }
        let compressor = super::Lz77Ac21Compressor;
        let compressed = compressor.compress(&data, 0, data.len()).unwrap();
        let decompressor = super::Lz77Ac21Decompressor;
        let decompressed = decompressor.decompress(&compressed, data.len()).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_lz77_ac21_compress_large_random() {
        // Pseudo-random data with some repeating patterns
        let mut data = Vec::new();
        let mut state: u32 = 12345;
        for _ in 0..4096 {
            state = state.wrapping_mul(1103515245).wrapping_add(12345);
            data.push((state >> 16) as u8);
        }
        let compressor = super::Lz77Ac21Compressor;
        let compressed = compressor.compress(&data, 0, data.len()).unwrap();
        let decompressor = super::Lz77Ac21Decompressor;
        let decompressed = decompressor.decompress(&compressed, data.len()).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_lz77_ac21_roundtrip_identity() {
        for pattern in &[
            vec![0u8; 100],
            vec![0xFF; 100],
            (0..=255).collect::<Vec<u8>>(),
            vec![1, 2, 3, 1, 2, 3, 1, 2, 3, 1, 2, 3],
            b"Hello, World! Hello, World! Hello, World!".to_vec(),
        ] {
            let compressor = super::Lz77Ac21Compressor;
            let compressed = compressor.compress(pattern, 0, pattern.len()).unwrap();
            let decompressor = super::Lz77Ac21Decompressor;
            let decompressed = decompressor.decompress(&compressed, pattern.len()).unwrap();
            assert_eq!(&decompressed, pattern, "Roundtrip failed for pattern of len {}", pattern.len());
        }
    }

    #[test]
    fn test_lz77_ac21_compress_all_zeros() {
        let data = vec![0u8; 1024];
        let compressor = super::Lz77Ac21Compressor;
        let compressed = compressor.compress(&data, 0, data.len()).unwrap();
        assert!(compressed.len() < data.len(), "Compression should reduce size for all-zeros");
        let decompressor = super::Lz77Ac21Decompressor;
        let decompressed = decompressor.decompress(&compressed, data.len()).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_lz77_ac21_compress_all_ones() {
        let data = vec![0xFF; 1024];
        let compressor = super::Lz77Ac21Compressor;
        let compressed = compressor.compress(&data, 0, data.len()).unwrap();
        assert!(compressed.len() < data.len());
        let decompressor = super::Lz77Ac21Decompressor;
        let decompressed = decompressor.decompress(&compressed, data.len()).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_copy_bytes_backward() {
        let mut buf = vec![1, 2, 3, 4, 0, 0, 0, 0];
        super::copy_bytes_backward(&mut buf, 4, 4, 4);
        assert_eq!(buf, vec![1, 2, 3, 4, 1, 2, 3, 4]);
    }

    #[test]
    fn test_copy_bytes_backward_overlapping() {
        let mut buf = vec![1, 2, 0, 0, 0, 0];
        super::copy_bytes_backward(&mut buf, 2, 4, 2);
        assert_eq!(buf, vec![1, 2, 1, 2, 1, 2]);
    }
}
