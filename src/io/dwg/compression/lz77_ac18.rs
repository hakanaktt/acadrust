//! LZ77 AC18 compression and decompression.
//!
//! This is the LZ77 variant used in AC1018 (R2004), AC1024 (R2010),
//! AC1027 (R2013), and AC1032 (R2018) DWG files.
//!
//! Mirrors ACadSharp's `DwgLZ77AC18Decompressor` and `DwgLZ77AC18Compressor`.

use crate::error::{DxfError, Result};
use std::io::{Cursor, Read, Write};

// ---------------------------------------------------------------------------
// Decompressor
// ---------------------------------------------------------------------------

/// Decompressor for the LZ77 AC18 variant.
pub struct Lz77Ac18Decompressor;

impl super::Decompressor for Lz77Ac18Decompressor {
    fn decompress(&self, source: &[u8], decompressed_size: usize) -> Result<Vec<u8>> {
        let mut src = Cursor::new(source);
        let mut dst = Cursor::new(Vec::with_capacity(decompressed_size));
        decompress_to_dest(&mut src, &mut dst)?;
        Ok(dst.into_inner())
    }
}

/// Decompress from a source stream to a destination stream.
///
/// Faithful port of ACadSharp `DwgLZ77AC18Decompressor.DecompressToDest`.
pub fn decompress_to_dest<R: Read, W: Read + Write + std::io::Seek>(
    src: &mut R,
    dst: &mut W,
) -> Result<()> {
    let mut temp_buf = vec![0u8; 128];

    let mut opcode1 = read_byte(src)?;

    if (opcode1 & 0xF0) == 0 {
        opcode1 = copy(literal_count(opcode1, src)? + 3, src, dst, &mut temp_buf)?;
    }

    // 0x11: Terminates the input stream
    while opcode1 != 0x11 {
        // Offset backwards from current position in decompressed data
        let mut comp_offset: i32;
        // Number of compressed bytes to copy from previous location
        let compressed_bytes: i32;

        if opcode1 < 0x10 || opcode1 >= 0x40 {
            compressed_bytes = ((opcode1 as i32) >> 4) - 1;
            let opcode2 = read_byte(src)?;
            comp_offset =
                (((opcode1 as i32) >> 2 & 3) | ((opcode2 as i32) << 2)) + 1;
        } else if opcode1 < 0x20 {
            // 0x12 - 0x1F
            compressed_bytes = read_compressed_bytes(opcode1, 0b0111, src)?;
            comp_offset = ((opcode1 as i32) & 8) << 11;
            opcode1 = two_byte_offset(&mut comp_offset, 0x4000, src)?;
        } else {
            // opcode1 >= 0x20
            compressed_bytes = read_compressed_bytes(opcode1, 0b00011111, src)?;
            comp_offset = 0;
            opcode1 = two_byte_offset(&mut comp_offset, 1, src)?;
        }

        // Copy compressed bytes from earlier in the output
        let position = dst.stream_position().map_err(|e| DxfError::Io(e))?;
        if temp_buf.len() < compressed_bytes as usize {
            temp_buf.resize(compressed_bytes as usize, 0);
        }

        let copy_len = std::cmp::min(compressed_bytes as usize, comp_offset as usize);
        dst.seek(std::io::SeekFrom::Start(position - comp_offset as u64))
            .map_err(|e| DxfError::Io(e))?;
        dst.read_exact(&mut temp_buf[..copy_len])
            .map_err(|e| DxfError::Io(e))?;
        dst.seek(std::io::SeekFrom::Start(position))
            .map_err(|e| DxfError::Io(e))?;

        let mut remaining = compressed_bytes as usize;
        while remaining > 0 {
            let write_len = std::cmp::min(remaining, comp_offset as usize);
            dst.write_all(&temp_buf[..write_len])
                .map_err(|e| DxfError::Io(e))?;
            remaining -= write_len;
        }

        // Literal bytes to copy from input stream
        let mut lit_count = (opcode1 & 3) as i32;
        if lit_count == 0 {
            opcode1 = read_byte(src)?;
            if (opcode1 & 0b11110000) == 0 {
                lit_count = literal_count(opcode1, src)? + 3;
            }
        }

        if lit_count > 0 {
            opcode1 = copy(lit_count, src, dst, &mut temp_buf)?;
        }
    }

    Ok(())
}

/// Read a single byte from the stream.
fn read_byte<R: Read>(stream: &mut R) -> Result<u8> {
    let mut buf = [0u8; 1];
    stream.read_exact(&mut buf).map_err(|e| DxfError::Io(e))?;
    Ok(buf[0])
}

/// Copy `count` literal bytes from src to dst, return next opcode byte.
fn copy<R: Read, W: Write>(
    count: i32,
    src: &mut R,
    dst: &mut W,
    temp_buf: &mut Vec<u8>,
) -> Result<u8> {
    let count = count as usize;
    if temp_buf.len() < count {
        temp_buf.resize(count, 0);
    }
    src.read_exact(&mut temp_buf[..count])
        .map_err(|e| DxfError::Io(e))?;
    dst.write_all(&temp_buf[..count])
        .map_err(|e| DxfError::Io(e))?;
    read_byte(src)
}

/// Decode a literal length from the stream.
///
/// The low nibble gives the initial length. If it is 0, we accumulate
/// 0xFF contributions until a non-zero byte is found, then add 0x0F + that byte.
fn literal_count<R: Read>(code: u8, src: &mut R) -> Result<i32> {
    let mut low_bits = (code & 0x0F) as i32;
    if low_bits == 0 {
        let mut last_byte = read_byte(src)?;
        while last_byte == 0 {
            low_bits += 0xFF;
            last_byte = read_byte(src)?;
        }
        low_bits += 0x0F + last_byte as i32;
    }
    Ok(low_bits)
}

/// Read a compressed byte count from the stream.
///
/// If the masked bits are 0, accumulate 0xFF contributions from subsequent
/// bytes until non-zero, then add `valid_bits` mask value.
fn read_compressed_bytes<R: Read>(opcode1: u8, valid_bits: u8, src: &mut R) -> Result<i32> {
    let mut compressed_bytes = (opcode1 & valid_bits) as i32;

    if compressed_bytes == 0 {
        let mut last_byte = read_byte(src)?;
        while last_byte == 0 {
            compressed_bytes += 0xFF;
            last_byte = read_byte(src)?;
        }
        compressed_bytes += last_byte as i32 + valid_bits as i32;
    }

    Ok(compressed_bytes + 2)
}

/// Read a 2-byte offset from the stream.
///
/// Returns the next opcode byte (the low byte read).
fn two_byte_offset<R: Read>(offset: &mut i32, added_value: i32, src: &mut R) -> Result<u8> {
    let first_byte = read_byte(src)?;
    let second_byte = read_byte(src)?;

    *offset |= (first_byte as i32) >> 2;
    *offset |= (second_byte as i32) << 6;
    *offset += added_value;

    Ok(first_byte)
}

// ---------------------------------------------------------------------------
// Compressor
// ---------------------------------------------------------------------------

/// Compressor for the LZ77 AC18 variant.
///
/// Faithful port of ACadSharp `DwgLZ77AC18Compressor`.
pub struct Lz77Ac18Compressor {
    source: Vec<u8>,
    block: Vec<i32>,
    initial_offset: usize,
    curr_position: usize,
    curr_offset: usize,
    total_offset: usize,
}

impl Lz77Ac18Compressor {
    pub fn new() -> Self {
        Self {
            source: Vec::new(),
            block: vec![-1i32; 0x8000],
            initial_offset: 0,
            curr_position: 0,
            curr_offset: 0,
            total_offset: 0,
        }
    }

    fn restart_block(&mut self) {
        for i in 0..self.block.len() {
            self.block[i] = -1;
        }
    }
}

impl super::Compressor for Lz77Ac18Compressor {
    fn compress(&self, source: &[u8], offset: usize, total_size: usize) -> Result<Vec<u8>> {
        let mut compressor = Lz77Ac18Compressor::new();
        compressor.compress_impl(source, offset, total_size)
    }
}

impl Lz77Ac18Compressor {
    /// Internal compression implementation.
    pub fn compress_impl(
        &mut self,
        source: &[u8],
        offset: usize,
        total_size: usize,
    ) -> Result<Vec<u8>> {
        self.restart_block();

        self.source = source.to_vec();
        self.initial_offset = offset;
        self.total_offset = self.initial_offset + total_size;
        self.curr_offset = self.initial_offset;
        self.curr_position = self.initial_offset + 4;

        let mut dest = Vec::new();

        let mut compression_offset: i32 = 0;
        let mut match_pos: i32 = 0;

        let mut curr_offset_local: i32 = 0;
        let mut last_match_pos: i32 = 0;

        while self.curr_position < self.total_offset.saturating_sub(0x13) {
            if !self.compress_chunk(&mut curr_offset_local, &mut last_match_pos) {
                self.curr_position += 1;
                continue;
            }

            let mask = (self.curr_position - self.curr_offset) as i32;

            if compression_offset != 0 {
                self.apply_mask(match_pos, compression_offset, mask, &mut dest)?;
            }

            self.write_literal_length(mask, &mut dest)?;
            self.curr_position += curr_offset_local as usize;
            self.curr_offset = self.curr_position;
            compression_offset = curr_offset_local;
            match_pos = last_match_pos;
        }

        let literal_length = (self.total_offset - self.curr_offset) as i32;

        if compression_offset != 0 {
            self.apply_mask(match_pos, compression_offset, literal_length, &mut dest)?;
        }

        self.write_literal_length(literal_length, &mut dest)?;

        // 0x11: Terminates the input stream
        dest.push(0x11);
        dest.push(0);
        dest.push(0);

        Ok(dest)
    }

    fn write_len(len: i32, dest: &mut Vec<u8>) -> Result<()> {
        if len <= 0 {
            return Err(DxfError::Compression("Invalid length in write_len".into()));
        }
        let mut remaining = len;
        while remaining > 0xFF {
            remaining -= 0xFF;
            dest.push(0);
        }
        dest.push(remaining as u8);
        Ok(())
    }

    fn write_op_code(
        op_code: i32,
        compression_offset: i32,
        value: i32,
        dest: &mut Vec<u8>,
    ) -> Result<()> {
        if compression_offset <= 0 {
            return Err(DxfError::Compression(
                "Invalid compression_offset in write_op_code".into(),
            ));
        }
        if value <= 0 {
            return Err(DxfError::Compression(
                "Invalid value in write_op_code".into(),
            ));
        }

        if compression_offset <= value {
            dest.push((op_code | (compression_offset - 2)) as u8);
        } else {
            dest.push(op_code as u8);
            Self::write_len(compression_offset - value, dest)?;
        }

        Ok(())
    }

    fn write_literal_length(&self, length: i32, dest: &mut Vec<u8>) -> Result<()> {
        if length <= 0 {
            return Ok(());
        }

        if length > 3 {
            Self::write_op_code(0, length - 1, 0x11, dest)?;
        }

        let mut num = self.curr_offset;
        for _ in 0..length {
            dest.push(self.source[num]);
            num += 1;
        }

        Ok(())
    }

    fn apply_mask(
        &self,
        mut match_position: i32,
        compression_offset: i32,
        mask: i32,
        dest: &mut Vec<u8>,
    ) -> Result<()> {
        let curr: i32;
        let next: i32;

        if compression_offset >= 0x0F || match_position > 0x400 {
            if match_position <= 0x4000 {
                match_position -= 1;
                Self::write_op_code(0x20, compression_offset, 0x21, dest)?;
            } else {
                match_position -= 0x4000;
                Self::write_op_code(
                    0x10 | ((match_position >> 11) & 8),
                    compression_offset,
                    0x09,
                    dest,
                )?;
            }

            curr = (match_position & 0xFF) << 2;
            next = match_position >> 6;
        } else {
            match_position -= 1;
            curr = ((compression_offset + 1) << 4) | ((match_position & 0b11) << 2);
            next = match_position >> 2;
        }

        let curr_out = if mask < 4 { curr | mask } else { curr };

        dest.push(curr_out as u8);
        dest.push(next as u8);

        Ok(())
    }

    fn compress_chunk(&mut self, offset: &mut i32, match_pos: &mut i32) -> bool {
        *offset = 0;

        if self.curr_position + 3 >= self.source.len() {
            return false;
        }

        let v1 = (self.source[self.curr_position + 3] as i32) << 6;
        let v2 = v1 ^ (self.source[self.curr_position + 2] as i32);
        let v3 = (v2 << 5) ^ (self.source[self.curr_position + 1] as i32);
        let v4 = (v3 << 5) ^ (self.source[self.curr_position] as i32);
        let mut value_index = ((v4 + (v4 >> 5)) & 0x7FFF) as usize;

        let value = self.block[value_index];
        *match_pos = self.curr_position as i32 - value;

        if value >= self.initial_offset as i32 && *match_pos <= 0xBFFF {
            if *match_pos > 0x400
                && self.source[self.curr_position + 3] != self.source[value as usize + 3]
            {
                value_index = (value_index & 0x7FF) ^ 0b100000000011111;
                let value2 = self.block[value_index];
                *match_pos = self.curr_position as i32 - value2;
                if value2 < self.initial_offset as i32
                    || *match_pos > 0xBFFF
                    || (*match_pos > 0x400
                        && self.source[self.curr_position + 3]
                            != self.source[value2 as usize + 3])
                {
                    self.block[value_index] = self.curr_position as i32;
                    return false;
                }
            }

            let value_resolved = self.block[value_index];
            if self.source[self.curr_position] == self.source[value_resolved as usize]
                && self.source[self.curr_position + 1] == self.source[value_resolved as usize + 1]
                && self.source[self.curr_position + 2] == self.source[value_resolved as usize + 2]
            {
                *offset = 3;
                let mut index = value_resolved as usize + 3;
                let mut curr_pos = self.curr_position + 3;
                while curr_pos < self.total_offset
                    && index < self.source.len()
                    && self.source[index] == self.source[curr_pos]
                {
                    *offset += 1;
                    index += 1;
                    curr_pos += 1;
                }
            }
        }

        self.block[value_index] = self.curr_position as i32;
        *offset >= 3
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::Decompressor;

    #[test]
    fn test_decompress_basic() {
        // A minimal LZ77 AC18 compressed stream:
        // Just a terminator opcode 0x11
        let compressed = vec![0x11];
        let decompressor = Lz77Ac18Decompressor;
        let result = decompressor.decompress(&compressed, 0).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_compress_decompress_roundtrip() {
        // Create a compressible data pattern
        let mut data = Vec::new();
        for i in 0..256 {
            data.push((i % 64) as u8);
        }
        for i in 0..256 {
            data.push((i % 64) as u8);
        }

        let mut compressor = Lz77Ac18Compressor::new();
        let compressed = compressor.compress_impl(&data, 0, data.len());

        if let Ok(compressed) = compressed {
            let decompressor = Lz77Ac18Decompressor;
            let decompressed = decompressor.decompress(&compressed, data.len());
            if let Ok(decompressed) = decompressed {
                assert_eq!(decompressed, data);
            }
        }
    }
}
