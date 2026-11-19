//! The package container: a ZIP archive of deflated (or stored) parts.
//!
//! ```text
//! per part: local header (0x04034B50, version 20, flags, method, DOS time
//!   and date, CRC-32, compressed and plain size, name length, extra
//!   length, name, extra), data
//! central directory: per part 0x02014B50 with the same fields and the
//!   local header's offset
//! end record: 0x06054B50, part count, directory size and offset
//! ```

use crate::error::DxfError;
use std::io::{Read, Write};

fn bad(m: &str) -> DxfError {
    DxfError::Parse(format!("package archive: {m}"))
}

fn u16_at(b: &[u8], i: usize) -> Result<u16, DxfError> {
    b.get(i..i + 2).map(|s| u16::from_le_bytes([s[0], s[1]])).ok_or_else(|| bad("truncated"))
}

fn u32_at(b: &[u8], i: usize) -> Result<u32, DxfError> {
    b.get(i..i + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]])).ok_or_else(|| bad("truncated"))
}

/// The largest part read: a package's parts are small, and a header must
/// not make the reader reserve or inflate more than this.
const MAX_PART: usize = 64 << 20;

/// The parts of an archive, in directory order: (name, bytes).
pub fn read(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, DxfError> {
    let end = (0..bytes.len().saturating_sub(21)).rev().find(|&i| bytes[i..].starts_with(&[0x50, 0x4B, 0x05, 0x06])).ok_or_else(|| bad("no end record"))?;
    let count = u16_at(bytes, end + 10)? as usize;
    let mut at = u32_at(bytes, end + 16)? as usize;
    let mut parts = Vec::with_capacity(count);
    for _ in 0..count {
        if u32_at(bytes, at)? != 0x0201_4B50 {
            return Err(bad("bad directory entry"));
        }
        let method = u16_at(bytes, at + 10)?;
        let crc = u32_at(bytes, at + 16)?;
        let packed = u32_at(bytes, at + 20)? as usize;
        let size = u32_at(bytes, at + 24)? as usize;
        let name_len = u16_at(bytes, at + 28)? as usize;
        let extra = u16_at(bytes, at + 30)? as usize + u16_at(bytes, at + 32)? as usize;
        let local = u32_at(bytes, at + 42)? as usize;
        let name = String::from_utf8(bytes.get(at + 46..at + 46 + name_len).ok_or_else(|| bad("truncated"))?.to_vec()).map_err(|_| bad("name is not UTF-8"))?;
        at += 46 + name_len + extra;
        if u32_at(bytes, local)? != 0x0403_4B50 {
            return Err(bad("bad local header"));
        }
        let start = local + 30 + u16_at(bytes, local + 26)? as usize + u16_at(bytes, local + 28)? as usize;
        if size > MAX_PART {
            return Err(bad("part too large"));
        }
        let data = bytes.get(start..start + packed).ok_or_else(|| bad("truncated part"))?;
        let plain = match method {
            0 => data.to_vec(),
            8 => {
                // Inflated no further than the size the header gives, so a
                // part that expands past it is caught by the size check.
                let mut v = Vec::with_capacity(size);
                flate2::read::DeflateDecoder::new(data)
                    .take(size as u64 + 1)
                    .read_to_end(&mut v)
                    .map_err(|e| DxfError::Compression(e.to_string()))?;
                v
            }
            _ => return Err(bad("unsupported compression method")),
        };
        let mut c = flate2::Crc::new();
        c.update(&plain);
        if plain.len() != size || c.sum() != crc {
            return Err(bad("part does not match its checksum"));
        }
        parts.push((name, plain));
    }
    Ok(parts)
}

/// An archive of `parts`, each deflated.
pub fn write(parts: &[(String, Vec<u8>)]) -> Result<Vec<u8>, DxfError> {
    // DOS date 1980-01-01, time 00:00.
    const TIME: u16 = 0;
    const DATE: u16 = 0x21;
    let mut out = Vec::new();
    let mut dir = Vec::new();
    for (name, plain) in parts {
        let mut z = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        z.write_all(plain)?;
        let data = z.finish()?;
        let mut c = flate2::Crc::new();
        c.update(plain);
        let offset = out.len() as u32;
        let fields = |h: &mut Vec<u8>| {
            for v in [8u16, TIME, DATE] {
                h.extend_from_slice(&v.to_le_bytes());
            }
            for v in [c.sum(), data.len() as u32, plain.len() as u32] {
                h.extend_from_slice(&v.to_le_bytes());
            }
            h.extend_from_slice(&(name.len() as u16).to_le_bytes());
            h.extend_from_slice(&0u16.to_le_bytes());
        };
        out.extend_from_slice(&0x0403_4B50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        fields(&mut out);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&data);
        dir.extend_from_slice(&0x0201_4B50u32.to_le_bytes());
        dir.extend_from_slice(&20u16.to_le_bytes());
        dir.extend_from_slice(&20u16.to_le_bytes());
        dir.extend_from_slice(&0u16.to_le_bytes());
        fields(&mut dir);
        // comment length, disk, internal and external attributes
        dir.extend_from_slice(&[0; 10]);
        dir.extend_from_slice(&offset.to_le_bytes());
        dir.extend_from_slice(name.as_bytes());
    }
    let at = out.len() as u32;
    out.extend_from_slice(&dir);
    out.extend_from_slice(&0x0605_4B50u32.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    for _ in 0..2 {
        out.extend_from_slice(&(parts.len() as u16).to_le_bytes());
    }
    out.extend_from_slice(&(dir.len() as u32).to_le_bytes());
    out.extend_from_slice(&at.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn refuses_parts_larger_than_their_header() {
        // A part of 10 MB of zeros deflates to a few kB; with its header
        // claiming 10 bytes the reader stops after 11.
        let parts = vec![("a.bin".to_string(), vec![0u8; 10 << 20])];
        let mut z = super::write(&parts).unwrap();
        let central = z.windows(4).rposition(|w| w == [0x50, 0x4B, 0x01, 0x02]).unwrap();
        for at in [22, central + 24] {
            z[at..at + 4].copy_from_slice(&10u32.to_le_bytes());
        }
        assert!(super::read(&z).is_err());
        // A header claiming more than a part may hold is refused before
        // anything is reserved.
        let mut z = super::write(&[("b".to_string(), b"x".to_vec())]).unwrap();
        let central = z.windows(4).rposition(|w| w == [0x50, 0x4B, 0x01, 0x02]).unwrap();
        z[central + 24..central + 28].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(super::read(&z).is_err());
    }

    #[test]
    fn archives_round_trip() {
        let parts = vec![("a.txt".to_string(), b"hello hello hello".to_vec()), ("[b].xml".to_string(), Vec::new())];
        assert_eq!(super::read(&super::write(&parts).unwrap()).unwrap(), parts);
    }
}
