//! DWG Handle (Object Map) section writer.
//!
//! Writes the handle-to-offset mapping as delta-encoded modular shorts
//! in 2032-byte chunks, each protected by CRC-8.
//!
//! Mirrors ACadSharp's `DwgHandleWriter`.

use crate::error::Result;
use crate::io::dwg::crc;
use crate::types::DxfVersion;

use std::collections::BTreeMap;

/// Writer for the DWG handle-to-offset object map section.
#[allow(dead_code)]
pub struct DwgHandleWriter {
    version: DxfVersion,
}

impl DwgHandleWriter {
    /// Create a new handle writer.
    pub fn new(version: DxfVersion) -> Self {
        Self { version }
    }

    /// Write the handle map section, returning the raw section bytes.
    ///
    /// `handle_map` maps object handle → byte offset in the objects section.
    /// `section_offset` is subtracted for R18+ (relative offsets); 0 for earlier.
    pub fn write(
        &self,
        handle_map: &BTreeMap<u64, i64>,
        section_offset: i32,
    ) -> Result<Vec<u8>> {
        let mut output = Vec::new();

        let mut prev_handle: u64 = 0;
        let mut prev_loc: i64 = 0;

        // Start first section
        let mut section_start = output.len();
        // Reserve 2 bytes for section size (written later)
        output.push(0);
        output.push(0);

        for (&handle, &offset) in handle_map {
            let handle_off = handle - prev_handle;
            let loc = offset + section_offset as i64;
            let loc_diff = loc - prev_loc;

            let mut handle_buf = [0u8; 10];
            let mut handle_size = Self::modular_short_to_value(handle_off, &mut handle_buf);

            let mut loc_buf = [0u8; 5];
            let mut loc_size = Self::signed_modular_short_to_value(loc_diff as i32, &mut loc_buf);

            // Check if adding this entry would exceed the 2032-byte section limit
            if (output.len() - section_start) + handle_size + loc_size > 2032 {
                // Finalize current section
                Self::process_section(&mut output, section_start);

                section_start = output.len();
                output.push(0);
                output.push(0);

                // Recalculate deltas from zero for the new section
                handle_size = Self::modular_short_to_value(handle, &mut handle_buf);
                loc_size = Self::signed_modular_short_to_value(loc as i32, &mut loc_buf);
            }

            output.extend_from_slice(&handle_buf[..handle_size]);
            output.extend_from_slice(&loc_buf[..loc_size]);

            prev_handle = handle;
            prev_loc = loc;
        }

        // Finalize the last section
        Self::process_section(&mut output, section_start);

        // Write the empty terminating section
        section_start = output.len();
        output.push(0);
        output.push(0);
        Self::process_section(&mut output, section_start);

        Ok(output)
    }

    /// Encode an unsigned value as a modular short (7-bit groups, high bit = continuation).
    ///
    /// Returns the number of bytes written to `arr`.
    fn modular_short_to_value(mut value: u64, arr: &mut [u8]) -> usize {
        let mut i = 0;
        while value >= 0x80 {
            arr[i] = ((value & 0x7F) | 0x80) as u8;
            i += 1;
            value >>= 7;
        }
        arr[i] = value as u8;
        i + 1
    }

    /// Encode a signed value as a signed modular short.
    ///
    /// Returns the number of bytes written to `arr`.
    fn signed_modular_short_to_value(mut value: i32, arr: &mut [u8]) -> usize {
        let mut i = 0;
        if value < 0 {
            value = -value;
            while value >= 64 {
                arr[i] = ((value as u32 & 0x7F) | 0x80) as u8;
                i += 1;
                value >>= 7;
            }
            arr[i] = (value as u32 | 0x40) as u8;
            return i + 1;
        }

        while value >= 64 {
            arr[i] = ((value as u32 & 0x7F) | 0x80) as u8;
            i += 1;
            value >>= 7;
        }
        arr[i] = value as u8;
        i + 1
    }

    /// Finalize a section chunk: write the section size at its start, then append CRC-8.
    fn process_section(output: &mut Vec<u8>, section_start: usize) {
        let section_len = (output.len() - section_start) as u16;

        // Write section size (big endian) at the reserved 2 bytes
        output[section_start] = (section_len >> 8) as u8;
        output[section_start + 1] = (section_len & 0xFF) as u8;

        // Compute CRC-8 over the section
        let crc_val = crc::crc8(0xC0C1, &output[section_start..]);

        // Append CRC (big endian)
        output.push((crc_val >> 8) as u8);
        output.push((crc_val & 0xFF) as u8);
    }
}
