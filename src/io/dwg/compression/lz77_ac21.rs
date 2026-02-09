//! LZ77 AC21 compression and decompression.
//!
//! This is the LZ77 variant used exclusively in AC1021 (R2007) DWG files.
//! It uses a different opcode format than the AC18 variant.
//!
//! Mirrors ACadSharp's `DwgLZ77AC21Decompressor` and `DwgLZ77AC21Compressor`.

use crate::error::{DxfError, Result};

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
/// * `source` — The compressed data buffer
/// * `initial_offset` — Starting offset into the source buffer
/// * `length` — Number of bytes of compressed data
/// * `buffer` — Pre-allocated output buffer (must be the decompressed size)
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
// Compressor (not implemented for AC21, same as ACadSharp)
// ---------------------------------------------------------------------------

/// Compressor for the LZ77 AC21 variant.
///
/// **Not implemented.** AC21 (R2007) compression is not supported for writing,
/// matching ACadSharp's behavior.
pub struct Lz77Ac21Compressor;

impl super::Compressor for Lz77Ac21Compressor {
    fn compress(&self, _source: &[u8], _offset: usize, _total_size: usize) -> Result<Vec<u8>> {
        Err(DxfError::NotImplemented(
            "LZ77 AC21 compression is not implemented".into(),
        ))
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_lz77_ac21_compressor_not_implemented() {
        use super::super::Compressor;
        let compressor = super::Lz77Ac21Compressor;
        assert!(compressor.compress(&[0u8; 10], 0, 10).is_err());
    }

    #[test]
    fn test_copy_bytes_backward() {
        let mut buf = vec![1, 2, 3, 4, 0, 0, 0, 0];
        // Copy 4 bytes from offset 4 back (position 4 - 4 = 0)
        super::copy_bytes_backward(&mut buf, 4, 4, 4);
        assert_eq!(buf, vec![1, 2, 3, 4, 1, 2, 3, 4]);
    }

    #[test]
    fn test_copy_bytes_backward_overlapping() {
        // Overlapping copy: repeat pattern
        let mut buf = vec![1, 2, 0, 0, 0, 0];
        // Copy 4 bytes from offset 2 back from position 2
        super::copy_bytes_backward(&mut buf, 2, 4, 2);
        assert_eq!(buf, vec![1, 2, 1, 2, 1, 2]);
    }
}
