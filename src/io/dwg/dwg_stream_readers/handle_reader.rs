//! DWG Handles section reader
//!
//! Reads the AcDb:Handles section from a DWG file, producing a
//! `HashMap<u64, i64>` mapping handles to file offsets. This is used
//! by the object reader to locate individual objects in the AcDb:AcDbObjects
//! section.
//!
//! Mirrors the reference `DwgHandleReader`.
//!
//! ## Section layout
//!
//! The Handles section consists of multiple chunks, each up to 2032 bytes:
//!
//! ```text
//! ┌─────────────────────────────────────┐
//! │ Chunk:                              │
//! │   Chunk size (RS big-endian)        │
//! │   Handle-offset pairs (MC/SMC)      │
//! │   CRC-16 (RS big-endian)            │
//! └─────────────────────────────────────┘
//! ```

use crate::error::{DxfError, Result};

/// Read the Handles section from raw section bytes (already decompressed).
///
/// The handle section is byte-aligned (not bitstream), consisting of
/// size-delimited chunks with modular-char encoded handle/offset pairs.
///
/// # Arguments
/// * `data` - Complete section buffer (just the handle data, no sentinels)
///
/// # Returns
/// `HashMap<u64, i64>` mapping object handles to their file offsets within
/// the AcDb:AcDbObjects section.
pub fn read_handles(data: &[u8]) -> Result<std::collections::HashMap<u64, i64>> {
    read_handle_map(data).map(|map| map.into_iter().collect())
}

/// Handle → file offset, hashed with foldhash. Every record lookup goes
/// through this map, which is why it does not use the std hasher; the seed
/// stays in the map because the map lives in [`DwgObjectReader`], which plugins
/// may use from another binary (see `CadDocument::entity_index`).
///
/// [`DwgObjectReader`]: super::object_reader::DwgObjectReader
pub(crate) type HandleMap = std::collections::HashMap<u64, i64, crate::document::SeededState>;

/// [`read_handles`] into the map the object reader keeps.
pub(crate) fn read_handle_map(data: &[u8]) -> Result<HandleMap> {
    let mut handle_map = HandleMap::default();
    let mut pos: usize = 0;

    // Process chunks until we run out of data
    while pos + 2 <= data.len() {
        // Read big-endian u16 chunk size.
        // Per ODA spec §5.5, the size includes the 2-byte size header itself
        // but NOT the trailing 2-byte CRC.  A size of 2 means an empty
        // terminator chunk (no data, only the size header).
        let size = ((data[pos] as usize) << 8) | (data[pos + 1] as usize);
        pos += 2;

        if size <= 2 || size > 2048 {
            break;
        }

        // Data bytes = size − 2 (subtract the 2-byte size header).
        // This matches the reference `maxSectionOffset = size - 2`.
        let data_bytes = size - 2;
        let chunk_end = (pos + data_bytes).min(data.len());

        // Delta-decode handle + offset pairs
        let mut last_handle: u64 = 0;
        let mut last_offset: i64 = 0;

        while pos < chunk_end {
            // Handle delta (MC — unsigned modular char)
            let (handle_delta, bytes_read) = read_mc(&data[pos..])?;
            pos += bytes_read;

            // Offset delta (SMC — signed modular char)
            let (offset_delta, bytes_read) = read_smc(&data[pos..])?;
            pos += bytes_read;

            last_handle = last_handle.wrapping_add(handle_delta);
            last_offset = last_offset.wrapping_add(offset_delta);

            handle_map.insert(last_handle, last_offset);
        }

        // Skip CRC (2 bytes big-endian)
        if pos + 2 <= data.len() {
            pos += 2;
        }
    }

    Ok(handle_map)
}

/// Read unsigned Modular Char (MC) from byte slice.
/// Returns (value, bytes_consumed).
///
/// A value whose bits do not fit in 64 bits (ten or more continuation
/// bytes, or a tenth byte above 1) is an error: the map is damaged.
fn read_mc(data: &[u8]) -> Result<(u64, usize)> {
    let mut value: u64 = 0;
    let mut shift: u32 = 0;
    let mut i: usize = 0;

    loop {
        if i >= data.len() {
            break;
        }
        let b = data[i];
        i += 1;
        add_bits(&mut value, (b & 0x7F) as u64, shift)?;
        if (b & 0x80) == 0 {
            break;
        }
        shift += 7;
    }

    Ok((value, i))
}

/// Read signed Modular Char (SMC) from byte slice.
/// Returns (value, bytes_consumed).
///
/// Its magnitude must fit in 64 bits, as for [`read_mc`].
fn read_smc(data: &[u8]) -> Result<(i64, usize)> {
    let mut value: u64 = 0;
    let mut shift: u32 = 0;
    let mut i: usize = 0;
    let mut last_byte: u8 = 0;

    loop {
        if i >= data.len() {
            break;
        }
        let b = data[i];
        i += 1;
        last_byte = b;

        if (b & 0x80) == 0 {
            // Final byte: bits 0-5 = value, bit 6 = sign
            add_bits(&mut value, (b & 0x3F) as u64, shift)?;
            break;
        } else {
            add_bits(&mut value, (b & 0x7F) as u64, shift)?;
            shift += 7;
        }
    }

    let signed_value = value as i64;
    if (last_byte & 0x40) != 0 {
        Ok((-signed_value, i))
    } else {
        Ok((signed_value, i))
    }
}

/// ORs `bits` into `value` at `shift`, or fails if the byte starts at bit 64
/// or later, or any of its bits would fall past bit 63.
fn add_bits(value: &mut u64, bits: u64, shift: u32) -> Result<()> {
    if shift >= 64 || (bits != 0 && bits.leading_zeros() < shift) {
        return Err(DxfError::InvalidFormat(format!(
            "AcDb:Handles: a modular char runs past 64 bits ({} continuation bytes)",
            shift / 7
        )));
    }
    *value |= bits << shift;
    Ok(())
}

// ════════════════════════════════════════════════════════════════════════════
//  Tests
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mc_encoding() {
        // Single byte: 5 → 0x05
        let (val, len) = read_mc(&[0x05]).unwrap();
        assert_eq!(val, 5);
        assert_eq!(len, 1);

        // Two bytes: 200 = 0b11001000
        // byte0: (200 & 0x7F) | 0x80 = 0xC8
        // byte1: (200 >> 7) = 1 → 0x01
        let (val, len) = read_mc(&[0xC8, 0x01]).unwrap();
        assert_eq!(val, 200);
        assert_eq!(len, 2);
    }

    #[test]
    fn test_smc_encoding() {
        // Single byte positive: 36 → 0x24 (bit 6 = 0 = positive)
        let (val, _) = read_smc(&[0x24]).unwrap();
        assert_eq!(val, 36);

        // Single byte negative: -36 → 0x64 (36 | 0x40)
        let (val, _) = read_smc(&[0x64]).unwrap();
        assert_eq!(val, -36);

        // Multi-byte positive: 100 → first byte 0xE4 (100|0x80), second byte 0x00
        let (val, len) = read_smc(&[0xE4, 0x00]).unwrap();
        assert_eq!(val, 100);
        assert_eq!(len, 2);
    }

    #[test]
    fn test_handle_reader_basic() {
        // Build a chunk with 2 handle-offset pairs using correct MC/SMC encoding
        let mut chunk_data = Vec::new();

        // Handle 1: MC delta=5 (0x05), SMC offset=100 (0xE4, 0x00)
        chunk_data.push(0x05); // MC: 5
        chunk_data.push(0xE4); // SMC: 100 (multi-byte)
        chunk_data.push(0x00);

        // Handle 2: MC delta=3 (0x03), SMC offset=50 (0x32, positive, single byte)
        chunk_data.push(0x03); // MC: 3
        chunk_data.push(0x32); // SMC: 50

        // Build full section: big-endian size + chunk + CRC
        // Size includes the 2-byte size header itself (matching writer convention)
        let mut data = Vec::new();
        let size = (2 + chunk_data.len()) as u16;
        data.push((size >> 8) as u8);
        data.push((size & 0xFF) as u8);
        data.extend_from_slice(&chunk_data);
        data.push(0x00); // CRC dummy
        data.push(0x00);

        let handles = read_handles(&data).unwrap();
        assert_eq!(handles.len(), 2);
        assert_eq!(handles[&5_u64], 100_i64);
        assert_eq!(handles[&8_u64], 150_i64); // 5+3=8, 100+50=150
    }

    /// Handle 5 at offset 100, handle 8 at offset 150.
    const TWO_HANDLES: [u8; 9] = [0x00, 0x07, 0x05, 0xE4, 0x00, 0x03, 0x32, 0x00, 0x00];

    #[test]
    fn the_public_and_the_read_path_maps_hold_the_same_entries() {
        let public = read_handles(&TWO_HANDLES).unwrap();
        let internal = read_handle_map(&TWO_HANDLES).unwrap();
        assert_eq!(public.len(), 2);
        assert_eq!(internal.len(), public.len());
        for (handle, offset) in &public {
            assert_eq!(internal.get(handle), Some(offset));
        }
    }

    /// One handle-map chunk holding `pairs`, with its size header and a
    /// dummy CRC.
    fn section(pairs: &[u8]) -> Vec<u8> {
        let size = (2 + pairs.len()) as u16;
        let mut data = size.to_be_bytes().to_vec();
        data.extend_from_slice(pairs);
        data.extend_from_slice(&[0x00, 0x00]);
        data
    }

    /// Ten continuation bytes put the eleventh byte at a shift of 70 bits.
    #[test]
    fn a_handle_with_ten_continuation_bytes_is_an_error() {
        let mut pairs = vec![0xFF; 10];
        pairs.push(0x01); // last handle byte
        pairs.push(0x00); // offset 0
        assert!(read_handle_map(&section(&pairs)).is_err());
    }

    #[test]
    fn an_offset_with_ten_continuation_bytes_is_an_error() {
        let mut pairs = vec![0x01]; // handle 1
        pairs.extend_from_slice(&[0xFF; 10]);
        pairs.push(0x00); // last offset byte
        assert!(read_handle_map(&section(&pairs)).is_err());
    }

    /// The tenth byte sits at a shift of 63, so only its lowest bit fits in
    /// 64 bits. A 2 there would be dropped without a word.
    #[test]
    fn a_handle_whose_bits_do_not_fit_in_64_bits_is_an_error() {
        let mut pairs = vec![0xFF; 9];
        pairs.push(0x02);
        pairs.push(0x00);
        assert!(read_handle_map(&section(&pairs)).is_err());
    }

    #[test]
    fn an_offset_whose_bits_do_not_fit_in_64_bits_is_an_error() {
        let mut pairs = vec![0x01];
        pairs.extend_from_slice(&[0xFF; 9]);
        pairs.push(0x02); // final offset byte, value 2, positive
        assert!(read_handle_map(&section(&pairs)).is_err());
    }

    /// Continuation bytes with no bits in them still may not run past bit 63.
    #[test]
    fn empty_continuation_bytes_past_64_bits_are_an_error() {
        let mut pairs = vec![0x80; 10];
        pairs.push(0x00);
        pairs.push(0x00);
        assert!(read_handle_map(&section(&pairs)).is_err());
    }

    /// Guard: the widest handle that does fit still reads.
    #[test]
    fn the_largest_handle_that_fits_still_reads() {
        let mut pairs = vec![0xFF; 9];
        pairs.push(0x01);
        pairs.push(0x00);
        let map = read_handle_map(&section(&pairs)).unwrap();
        assert_eq!(map.get(&u64::MAX), Some(&0));
    }

    /// These signatures are public API: downstream code names the std types.
    #[test]
    fn the_public_entry_points_take_and_return_std_maps() {
        use crate::io::dwg::dwg_stream_readers::object_reader::DwgObjectReader;
        use crate::types::DxfVersion;

        let map: std::collections::HashMap<u64, i64> = read_handles(&TWO_HANDLES).unwrap();
        let new = DwgObjectReader::new(Vec::new(), DxfVersion::AC1018, map.clone()).unwrap();
        let with_encoding = DwgObjectReader::with_encoding(
            Vec::new(),
            DxfVersion::AC1018,
            map,
            encoding_rs::WINDOWS_1252,
        )
        .unwrap();
        for reader in [new, with_encoding] {
            assert_eq!(reader.offset_for(5), Some(100));
            assert_eq!(reader.offset_for(8), Some(150));
            assert_eq!(reader.offset_for(9), None);
            let mut handles = reader.handles();
            handles.sort_unstable();
            assert_eq!(handles, [5, 8]);
        }
    }
}
