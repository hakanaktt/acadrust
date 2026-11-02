//! LZ77 decompressor for DWG AC18 (R2004+) format
//!
//! This reverses the compression performed by [`super::compression::DwgLZ77AC18Compressor`].
//!
//! ## Encoding format
//!
//! The compressed stream consists of:
//! - **Initial literal run** (length prefix 0x01–0x0F or 0x00+extended)
//! - **Match references** interleaved with trailing literals:
//!   - Short match (0x40–0xFF): offset ≤ 0x400, length 3–14
//!   - Medium match (0x20–0x3F): offset ≤ 0x4000
//!   - Long match (0x10, 0x12–0x1F): offset > 0x4000
//!   - Each match encodes 0–3 trailing literal bytes in its lower bits
//!   - If trailing count is 0, additional literals may follow (prefixed 0x00–0x0F)
//! - **Terminator**: opcode 0x11
//!
//! Based on the ODA (Open Design Alliance) specification and matched against
//! the compressor in [`super::compression`].

use super::size_limits::MAX_DATA_PAGE_SIZE;

/// Decompress AC18 LZ77-compressed data.
///
/// # Arguments
/// * `source` – Compressed byte stream
/// * `decompressed_size` – Expected output size
///
/// # Returns
/// Decompressed data, truncated to `decompressed_size`.
pub fn decompress_ac18(source: &[u8], decompressed_size: usize) -> Vec<u8> {
    // `decompressed_size` comes from the file. Reserve at most one data page up front;
    // the output still grows up to `decompressed_size` as data arrives.
    let mut output = Vec::with_capacity(decompressed_size.min(MAX_DATA_PAGE_SIZE));
    let mut si = 0; // source index

    if source.is_empty() || decompressed_size == 0 {
        return output;
    }

    // ── Initial literal run ──
    // The compressor always starts with a literal prefix (>= 4 bytes).
    let first = source[si];
    si += 1;

    let lit_count = if first == 0x00 {
        // Extended literal length: read_ext_length() + 0x0F + 3
        read_ext_length(source, &mut si) + 0x0F + 3
    } else if first < 0x10 {
        // Short literal length: opcode + 3
        first as usize + 3
    } else {
        // Not a literal prefix — put back and read as opcode
        si -= 1;
        0
    };

    copy_literals(source, &mut si, &mut output, lit_count, decompressed_size);

    // ── Main decompression loop ──
    loop {
        if si >= source.len() || output.len() >= decompressed_size {
            break;
        }

        let opcode = source[si];
        si += 1;

        if opcode >= 0x40 {
            // ── Short match ──
            // Offset ≤ 0x400, length 3–14
            let length = ((opcode >> 4) as usize) - 1;
            if si >= source.len() {
                break;
            }
            let b1 = source[si] as usize;
            si += 1;
            let offset = (((opcode as usize) & 0x0C) >> 2 | (b1 << 2)) + 1;
            let trailing = (opcode & 0x03) as usize;

            copy_match(&mut output, offset, length, decompressed_size);
            copy_literals(source, &mut si, &mut output, trailing, decompressed_size);
        } else if opcode >= 0x21 {
            // ── Medium match, short length (0x21–0x3F) ──
            let length = (opcode & 0x1F) as usize + 2;
            if si + 1 >= source.len() {
                break;
            }
            let b1 = source[si] as usize;
            si += 1;
            let b2 = source[si] as usize;
            si += 1;
            let offset = ((b1 >> 2) | (b2 << 6)) + 1;
            let trailing = b1 & 0x03;

            copy_match(&mut output, offset, length, decompressed_size);
            copy_literals(source, &mut si, &mut output, trailing, decompressed_size);
        } else if opcode == 0x20 {
            // ── Medium match, extended length ──
            let length = read_ext_length(source, &mut si) + 0x21;
            if si + 1 >= source.len() {
                break;
            }
            let b1 = source[si] as usize;
            si += 1;
            let b2 = source[si] as usize;
            si += 1;
            let offset = ((b1 >> 2) | (b2 << 6)) + 1;
            let trailing = b1 & 0x03;

            copy_match(&mut output, offset, length, decompressed_size);
            copy_literals(source, &mut si, &mut output, trailing, decompressed_size);
        } else if opcode == 0x11 {
            // ── END marker ──
            break;
        } else if opcode >= 0x10 {
            // ── Long match (0x10, 0x12–0x1F) ──
            // Offset > 0x4000
            let len_bits = (opcode & 0x07) as usize;
            let length = if len_bits == 0 {
                read_ext_length(source, &mut si) + 0x09
            } else {
                len_bits + 2
            };
            if si + 1 >= source.len() {
                break;
            }
            let b1 = source[si] as usize;
            si += 1;
            let b2 = source[si] as usize;
            si += 1;
            let offset = ((b1 >> 2) | (b2 << 6) | ((opcode as usize & 0x08) << 11)) + 0x4000;
            let trailing = b1 & 0x03;

            copy_match(&mut output, offset, length, decompressed_size);
            copy_literals(source, &mut si, &mut output, trailing, decompressed_size);
        } else {
            // ── Literal run (0x00–0x0F, after match with trailing=0) ──
            let count = if opcode == 0x00 {
                read_ext_length(source, &mut si) + 0x0F + 3
            } else {
                opcode as usize + 3
            };
            copy_literals(source, &mut si, &mut output, count, decompressed_size);
        }
    }

    output.truncate(decompressed_size);
    output
}

/// Read an extended length value from the compressed stream.
///
/// The format uses run-length encoding: each 0x00 byte adds 0xFF to the total,
/// and the first non-zero byte adds its value and terminates the sequence.
///
/// This reverses [`DwgLZ77AC18Compressor::write_len()`].
fn read_ext_length(source: &[u8], si: &mut usize) -> usize {
    let mut total = 0usize;
    loop {
        if *si >= source.len() {
            break;
        }
        let b = source[*si];
        *si += 1;
        if b == 0 {
            total += 0xFF;
        } else {
            total += b as usize;
            break;
        }
    }
    total
}

/// Copy literal bytes from the compressed source to the output.
#[inline]
fn copy_literals(
    source: &[u8],
    si: &mut usize,
    output: &mut Vec<u8>,
    count: usize,
    max_size: usize,
) {
    let n = count
        .min(source.len().saturating_sub(*si))
        .min(max_size.saturating_sub(output.len()));
    output.extend_from_slice(&source[*si..*si + n]);
    *si += n;
}

/// Copy a match from earlier in the output buffer (back-reference).
#[inline]
fn copy_match(output: &mut Vec<u8>, offset: usize, length: usize, max_size: usize) {
    let n = length.min(max_size.saturating_sub(output.len()));
    let mut from = output.len().saturating_sub(offset);
    if from == output.len() {
        // Nothing to refer to (empty output or offset 0): emit zeros.
        output.resize(output.len() + n, 0);
        return;
    }
    let mut left = n;
    while left > 0 {
        let take = left.min(output.len() - from);
        output.extend_from_within(from..from + take);
        from += take;
        left -= take;
    }
}

// ════════════════════════════════════════════════════════════════════════════
//  Tests
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::dwg::compression::DwgLZ77AC18Compressor;

    /// Roundtrip test: compress then decompress and verify.
    fn roundtrip(data: &[u8]) {
        let mut comp = DwgLZ77AC18Compressor::new();
        let compressed = comp.compress(data, 0, data.len());
        let decompressed = decompress_ac18(&compressed, data.len());
        assert_eq!(
            decompressed.len(),
            data.len(),
            "Length mismatch: expected {}, got {}",
            data.len(),
            decompressed.len()
        );
        assert_eq!(
            &decompressed[..],
            data,
            "Data mismatch after roundtrip (first diff at byte {})",
            decompressed
                .iter()
                .zip(data.iter())
                .position(|(a, b)| a != b)
                .unwrap_or(0)
        );
    }

    #[test]
    fn test_roundtrip_zeros() {
        roundtrip(&vec![0u8; 1000]);
    }

    #[test]
    fn test_roundtrip_small() {
        roundtrip(b"Hello, World! This is a test of the decompressor.");
    }

    #[test]
    fn test_roundtrip_repetitive() {
        let mut data = Vec::new();
        for _ in 0..100 {
            data.extend_from_slice(b"ABCDEFGH");
        }
        roundtrip(&data);
    }

    #[test]
    fn test_roundtrip_sequential() {
        let data: Vec<u8> = (0..=255).cycle().take(2000).collect();
        roundtrip(&data);
    }

    #[test]
    fn test_roundtrip_mixed() {
        let mut data = Vec::new();
        // Some unique data
        data.extend_from_slice(b"Unique header text 12345");
        // Followed by repetitive data
        for _ in 0..50 {
            data.extend_from_slice(b"REPEAT");
        }
        // More unique data
        data.extend_from_slice(b"Another unique section here");
        // More repetitive
        for _ in 0..30 {
            data.extend_from_slice(b"XY");
        }
        roundtrip(&data);
    }

    #[test]
    fn test_roundtrip_page_sized() {
        // Test with 0x7400 bytes (typical DWG page size)
        roundtrip(&vec![0xAA; 0x7400]);
    }

    #[test]
    fn test_roundtrip_page_sized_pattern() {
        let mut data = vec![0u8; 0x7400];
        for (i, byte) in data.iter_mut().enumerate() {
            *byte = (i % 256) as u8;
        }
        roundtrip(&data);
    }

    #[test]
    fn test_roundtrip_minimum_data() {
        // Minimum size that the compressor can handle (needs at least 4 bytes for hash)
        roundtrip(&[1, 2, 3, 4]);
    }

    #[test]
    fn test_roundtrip_large_repetitive() {
        // Large enough to trigger long matches
        roundtrip(&vec![42u8; 100_000]);
    }

    #[test]
    fn test_decompress_empty() {
        let result = decompress_ac18(&[], 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_decompress_zero_target() {
        let result = decompress_ac18(&[0x01, 0xAA, 0xBB, 0xCC, 0xDD, 0x11, 0x00, 0x00], 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_read_ext_length_simple() {
        let data = [5u8];
        let mut si = 0;
        assert_eq!(read_ext_length(&data, &mut si), 5);
        assert_eq!(si, 1);
    }

    #[test]
    fn test_read_ext_length_extended() {
        // 0, 0, 3 → 0xFF + 0xFF + 3 = 0x201
        let data = [0u8, 0, 3];
        let mut si = 0;
        assert_eq!(read_ext_length(&data, &mut si), 0x201);
        assert_eq!(si, 3);
    }

    /// The byte-at-a-time copy `copy_match` has to agree with.
    fn copy_match_bytewise(output: &mut Vec<u8>, offset: usize, length: usize, max_size: usize) {
        let start = output.len().saturating_sub(offset);
        for i in 0..length {
            if output.len() >= max_size {
                break;
            }
            let byte = output.get(start + i).copied().unwrap_or(0);
            output.push(byte);
        }
    }

    #[test]
    fn copy_match_agrees_with_a_bytewise_copy() {
        let prefix: Vec<u8> = (1..=9).collect();
        for len in 0..=prefix.len() {
            for offset in 0..=12 {
                for length in 0..=30 {
                    for max_size in [0, len, len + 1, len + length / 2, len + length, 64] {
                        let mut fast = prefix[..len].to_vec();
                        let mut slow = fast.clone();
                        copy_match(&mut fast, offset, length, max_size);
                        copy_match_bytewise(&mut slow, offset, length, max_size);
                        assert_eq!(fast, slow, "{len} {offset} {length} {max_size}");
                    }
                }
            }
        }
    }

    #[test]
    fn copy_literals_stops_at_the_source_and_at_max_size() {
        // (source length, start, requested, max_size, bytes copied)
        let cases: [(u8, usize, usize, usize, usize); 6] = [
            (5, 0, 5, 64, 5),
            (5, 0, 9, 64, 5),
            (5, 2, 9, 64, 3),
            (5, 5, 1, 64, 0),
            (5, 0, 5, 3, 3),
            (5, 0, 5, 0, 0),
        ];
        for (source_len, start, count, max_size, copied) in cases {
            let source: Vec<u8> = (1..=source_len).collect();
            let mut si = start;
            let mut output = Vec::new();
            copy_literals(&source, &mut si, &mut output, count, max_size);
            assert_eq!(output, source[start..start + copied]);
            assert_eq!(si, start + copied);
        }
    }

    /// `abcd`, then a six-byte match of the given short-match offset (1 to 4).
    fn abcd_then_match(offset: u8) -> Vec<u8> {
        vec![
            0x01,
            b'a',
            b'b',
            b'c',
            b'd',
            0x70 | ((offset - 1) << 2),
            0x00,
        ]
    }

    #[test]
    fn an_overlapping_match_repeats_what_it_has_just_written() {
        let cases: [(u8, &[u8]); 4] = [
            (1, b"abcddddddd"),
            (2, b"abcdcdcdcd"),
            (3, b"abcdbcdbcd"),
            (4, b"abcdabcdab"),
        ];
        for (offset, expected) in cases {
            assert_eq!(decompress_ac18(&abcd_then_match(offset), 64), expected);
        }
    }

    #[test]
    fn a_match_stops_at_the_expected_size() {
        assert_eq!(decompress_ac18(&abcd_then_match(1), 6), b"abcddd");
    }

    #[test]
    fn a_match_reaching_before_the_start_copies_from_the_start() {
        // Offset 97, with four bytes written.
        let source = [0x01, b'a', b'b', b'c', b'd', 0x70, 0x18];
        assert_eq!(decompress_ac18(&source, 64), b"abcdabcdab");
    }

    #[test]
    fn a_match_with_nothing_before_it_emits_zeros() {
        assert_eq!(decompress_ac18(&[0x70, 0x00], 64), [0u8; 6]);
    }

    #[test]
    fn a_truncated_literal_run_keeps_what_is_there() {
        assert_eq!(decompress_ac18(&[0x05, b'a', b'b'], 64), b"ab");
    }
}
