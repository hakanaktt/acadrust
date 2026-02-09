//! Reed-Solomon byte de-interleaving for AC21 (R2007) DWG files.
//!
//! The "Reed-Solomon" encoding used in DWG is actually a simple byte
//! interleaving scheme (not full error-correcting RS). Encoded data has
//! bytes distributed across `factor` interleaved tracks.
//!
//! Mirrors the `reedSolomonDecoding` method in ACadSharp `DwgReader`.

/// Decode (de-interleave) a Reed-Solomon encoded byte array.
///
/// For the AC21 file header: `factor = 3`, `block_size = 239`.
/// For AC21 section pages: `factor = (total_size + block_size - 1) / block_size`,
/// `block_size = 255` (with data blocks of 251).
///
/// # Algorithm
///
/// The encoded data is read by stepping through every `factor`-th byte,
/// `factor` times. Each pass extracts at most `block_size` bytes.
///
/// # Arguments
/// * `encoded` — The interleaved (encoded) data
/// * `output_size` — Expected size of the decoded output
/// * `factor` — Number of interleaved tracks
/// * `block_size` — Number of data bytes per track
///
/// # Returns
/// A `Vec<u8>` of decoded data with length `output_size`.
pub fn decode(encoded: &[u8], output_size: usize, factor: usize, block_size: usize) -> Vec<u8> {
    let mut buffer = vec![0u8; output_size];
    let mut index = 0usize;
    let mut n = 0usize;
    let mut length = output_size;

    for _i in 0..factor {
        let mut cindex = n;
        if n < encoded.len() {
            let size = std::cmp::min(length, block_size);
            length -= size;
            let offset = index + size;
            while index < offset {
                if cindex < encoded.len() {
                    buffer[index] = encoded[cindex];
                }
                index += 1;
                cindex += factor;
            }
        }
        n += 1;
    }

    buffer
}

/// Encode (interleave) data using the Reed-Solomon byte interleaving scheme.
///
/// This is the inverse of [`decode`]. Data bytes are distributed across
/// `factor` interleaved tracks of `block_size` bytes each.
///
/// # Arguments
/// * `data` — The plain data to encode
/// * `factor` — Number of interleaved tracks
/// * `block_size` — Number of data bytes per track
///
/// # Returns
/// A `Vec<u8>` of interleaved data with length `factor * 255`.
pub fn encode(data: &[u8], factor: usize, block_size: usize) -> Vec<u8> {
    let encoded_size = factor * 255;
    let mut encoded = vec![0u8; encoded_size];

    let mut index = 0usize;
    let mut n = 0usize;
    let mut length = data.len();

    for _i in 0..factor {
        let mut cindex = n;
        let size = std::cmp::min(length, block_size);
        if length >= size {
            length -= size;
        } else {
            length = 0;
        }
        let offset = index + size;
        while index < offset {
            if cindex < encoded_size && index < data.len() {
                encoded[cindex] = data[index];
            }
            index += 1;
            cindex += factor;
        }
        n += 1;
    }

    encoded
}

/// Compute the Reed-Solomon factor and read size for a page buffer.
///
/// Given the compressed size and correction factor from the file header,
/// computes the factor (number of RS tracks) and the total number of bytes
/// to read from the file.
///
/// Mirrors the `getPageBuffer` logic in ACadSharp `DwgReader`.
///
/// # Arguments
/// * `compressed_size` — Compressed data size from the section descriptor
/// * `correction_factor` — Correction factor from compressed metadata
/// * `block_size` — RS block size (typically 251 for page data)
///
/// # Returns
/// `(factor, read_size)` — factor for RS decoding and total bytes to read from file.
pub fn compute_page_buffer_params(
    compressed_size: u64,
    correction_factor: u64,
    block_size: usize,
) -> (usize, usize) {
    // Avoid shifted bits
    let v = compressed_size + 7;
    let v1 = v & 0xFFFF_FFFF_FFFF_FFF8;
    let total_size = (v1 * correction_factor) as usize;
    let factor = (total_size + block_size - 1) / block_size;
    let read_size = factor * 255;
    (factor, read_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_encode_roundtrip_factor3() {
        // Create test data that fits 3 blocks of 239 bytes
        let data_size = 3 * 239;
        let mut data = vec![0u8; data_size];
        for i in 0..data_size {
            data[i] = (i % 256) as u8;
        }

        let encoded = encode(&data, 3, 239);
        let decoded = decode(&encoded, data_size, 3, 239);
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_decode_encode_roundtrip_factor1() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let encoded = encode(&data, 1, 239);
        let decoded = decode(&encoded, data.len(), 1, 239);
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_decode_small() {
        // factor=3, block_size=239
        // Encoded: bytes interleaved as [a0, b0, c0, a1, b1, c1, ...]
        // where a, b, c are the three tracks
        let mut encoded = vec![0u8; 3 * 255]; // factor * 255

        // Track 0: bytes at indices 0, 3, 6, ...
        // Track 1: bytes at indices 1, 4, 7, ...
        // Track 2: bytes at indices 2, 5, 8, ...

        // Write known values
        // Track 0 data: [10, 11, 12]
        encoded[0] = 10;
        encoded[3] = 11;
        encoded[6] = 12;
        // Track 1 data: [20, 21, 22]
        encoded[1] = 20;
        encoded[4] = 21;
        encoded[7] = 22;
        // Track 2 data: [30, 31, 32]
        encoded[2] = 30;
        encoded[5] = 31;
        encoded[8] = 32;

        let decoded = decode(&encoded, 9, 3, 3);
        // Should get: track0 data followed by track1 data followed by track2 data
        assert_eq!(decoded, vec![10, 11, 12, 20, 21, 22, 30, 31, 32]);
    }

    #[test]
    fn test_compute_page_buffer_params() {
        let (factor, read_size) = compute_page_buffer_params(1000, 3, 251);
        // v = 1000 + 7 = 1007
        // v1 = 1007 & ...FFF8 = 1000
        // total_size = 1000 * 3 = 3000
        // factor = (3000 + 250) / 251 = 3250 / 251 = 12
        // read_size = 12 * 255 = 3060
        assert_eq!(factor, 12);
        assert_eq!(read_size, 3060);
    }
}
