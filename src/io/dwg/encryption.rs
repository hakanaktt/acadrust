//! Encryption and decryption routines for the DWG file format.
//!
//! Mirrors the encryption logic found in ACadSharp's `DwgReader` and
//! `DwgFileHeaderWriterBase`. The DWG format uses two encryption mechanisms:
//!
//! 1. **CRC-32 LCG XOR encryption** — For the AC18+ file header metadata
//!    block (0x6C bytes at offset 0x80). This is handled by the CRC-32
//!    stream handler in [`crate::io::dwg::crc`].
//!
//! 2. **Position-based XOR mask** — For AC18+ data page headers (8 × i32).
//!    The mask is `0x4164536B ^ stream_position`.

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::Cursor;

use super::constants::ac18;

/// Decrypt an AC18+ data section page header.
///
/// Each data page in AC18+ (R2004/R2010/R2013/R2018) files has an 8-field
/// header (32 bytes = 8 × i32). The header is encrypted by XORing each i32
/// with the mask `0x4164536B ^ stream_position`.
///
/// # Arguments
/// * `data` — 32-byte page header (read from file, encrypted)
/// * `stream_position` — The absolute file position where the page header starts
///
/// # Returns
/// A `DecryptedPageHeader` containing the 8 decoded i32 fields.
///
/// Equivalent to the `decryptDataSection` method in ACadSharp `DwgReader`.
pub fn decrypt_data_section_header(data: &[u8; 32], stream_position: u64) -> DecryptedPageHeader {
    let sec_mask = (ac18::DECRYPTION_MASK ^ (stream_position as u32)) as i32;
    let mut cursor = Cursor::new(&data[..]);

    // 0x00: Section page type (always 0x4163043B for data sections)
    let page_type = cursor.read_i32::<LittleEndian>().unwrap() ^ sec_mask;
    // 0x04: Section number
    let section_number = cursor.read_i32::<LittleEndian>().unwrap() ^ sec_mask;
    // 0x08: Data size (compressed)
    let compressed_size = cursor.read_i32::<LittleEndian>().unwrap() ^ sec_mask;
    // 0x0C: Page size (decompressed)
    let page_size = cursor.read_i32::<LittleEndian>().unwrap() ^ sec_mask;
    // 0x10: Start offset (in the decompressed buffer)
    let start_offset = cursor.read_i32::<LittleEndian>().unwrap() ^ sec_mask;
    // 0x14: Page header checksum (with data checksum as seed)
    let header_checksum = cursor.read_i32::<LittleEndian>().unwrap() ^ sec_mask;
    // 0x18: Data checksum (with seed 0)
    let data_checksum = cursor.read_i32::<LittleEndian>().unwrap() ^ sec_mask;
    // 0x1C: Unknown (ODA writes 0)
    let unknown = cursor.read_i32::<LittleEndian>().unwrap() ^ sec_mask;

    DecryptedPageHeader {
        page_type,
        section_number,
        compressed_size,
        page_size,
        start_offset,
        header_checksum,
        data_checksum,
        unknown,
    }
}

/// Encrypt an AC18+ data section page header.
///
/// This is the inverse of [`decrypt_data_section_header`]. Each i32 field is
/// XORed with `0x4164536B ^ stream_position` and written as little-endian.
///
/// Equivalent to the page header writing in ACadSharp `DwgFileHeaderWriterBase`.
pub fn encrypt_data_section_header(
    header: &DecryptedPageHeader,
    stream_position: u64,
) -> [u8; 32] {
    let sec_mask = (ac18::DECRYPTION_MASK ^ (stream_position as u32)) as i32;
    let mut result = [0u8; 32];
    let mut cursor = Cursor::new(&mut result[..]);

    cursor
        .write_i32::<LittleEndian>(header.page_type ^ sec_mask)
        .unwrap();
    cursor
        .write_i32::<LittleEndian>(header.section_number ^ sec_mask)
        .unwrap();
    cursor
        .write_i32::<LittleEndian>(header.compressed_size ^ sec_mask)
        .unwrap();
    cursor
        .write_i32::<LittleEndian>(header.page_size ^ sec_mask)
        .unwrap();
    cursor
        .write_i32::<LittleEndian>(header.start_offset ^ sec_mask)
        .unwrap();
    cursor
        .write_i32::<LittleEndian>(header.header_checksum ^ sec_mask)
        .unwrap();
    cursor
        .write_i32::<LittleEndian>(header.data_checksum ^ sec_mask)
        .unwrap();
    cursor
        .write_i32::<LittleEndian>(header.unknown ^ sec_mask)
        .unwrap();

    result
}

/// Decrypted AC18+ data section page header fields.
///
/// Each data page in the file starts with this 32-byte encrypted header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecryptedPageHeader {
    /// Section page type — should be `0x4163043B` for normal data sections.
    pub page_type: i32,
    /// Section number within the descriptor.
    pub section_number: i32,
    /// Compressed data size in bytes.
    pub compressed_size: i32,
    /// Decompressed (page) size in bytes.
    pub page_size: i32,
    /// Start offset in the decompressed buffer.
    pub start_offset: i32,
    /// Page header checksum (section page checksum from unencoded header
    /// bytes, seeded with the data checksum).
    pub header_checksum: i32,
    /// Data checksum (section page checksum from compressed data bytes,
    /// seeded with 0).
    pub data_checksum: i32,
    /// Unknown field (ODA writes 0).
    pub unknown: i32,
}

impl DecryptedPageHeader {
    /// Compute the combined offset value used for section mapping.
    ///
    /// This is `header_checksum + start_offset`, which ACadSharp stores
    /// as `section.Offset`.
    pub fn offset(&self) -> u64 {
        (self.header_checksum.wrapping_add(self.start_offset)) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let original = DecryptedPageHeader {
            page_type: 0x4163043B,
            section_number: 1,
            compressed_size: 0x7000,
            page_size: 0x7400,
            start_offset: 0,
            header_checksum: 0x12345678,
            data_checksum: 0xABCDEF00u32 as i32,
            unknown: 0,
        };

        let position = 0x100u64;
        let encrypted = encrypt_data_section_header(&original, position);
        let decrypted = decrypt_data_section_header(&encrypted, position);

        assert_eq!(decrypted, original);
    }

    #[test]
    fn test_encrypt_decrypt_different_positions() {
        let header = DecryptedPageHeader {
            page_type: 0x4163043B,
            section_number: 5,
            compressed_size: 1024,
            page_size: 2048,
            start_offset: 512,
            header_checksum: 0,
            data_checksum: 0,
            unknown: 0,
        };

        // Same header encrypted at different positions should produce different ciphertext
        let enc1 = encrypt_data_section_header(&header, 0x0);
        let enc2 = encrypt_data_section_header(&header, 0x100);
        assert_ne!(enc1, enc2);

        // But both should decrypt back to the same values
        assert_eq!(decrypt_data_section_header(&enc1, 0x0), header);
        assert_eq!(decrypt_data_section_header(&enc2, 0x100), header);
    }

    #[test]
    fn test_offset_calculation() {
        let header = DecryptedPageHeader {
            page_type: 0,
            section_number: 0,
            compressed_size: 0,
            page_size: 0,
            start_offset: 100,
            header_checksum: 200,
            data_checksum: 0,
            unknown: 0,
        };
        assert_eq!(header.offset(), 300);
    }
}
