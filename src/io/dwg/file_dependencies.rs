//! Bounded AcDb:FileDepList values (ODA DWG specification, section 17).
use crate::io::dxf::code_page::{decode_legacy_escapes_in, encode_legacy_string, LegacyEncoding};
use crate::{
    document::FileDependency,
    error::{DxfError, Result},
    types::DxfVersion,
};

fn invalid(message: &str) -> DxfError {
    DxfError::InvalidFormat(format!("FileDepList: {message}"))
}

struct Reader<'a> {
    bytes: &'a [u8],
    unicode: bool,
    encoding: LegacyEncoding,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let value = self
            .bytes
            .get(..n)
            .ok_or_else(|| invalid("truncated record"))?;
        self.bytes = &self.bytes[n..];
        Ok(value)
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn text(&mut self) -> Result<String> {
        let n = self.u32()? as usize;
        let data = self.take(n)?;
        let value = if self.unicode {
            // Texts occur as UTF-16 or, from writers with a 4-byte wchar_t
            // (the macOS application's file names and paths), as UCS-4.
            // Lengths count bytes, not characters.
            let wide = !data.is_empty()
                && data.len() % 4 == 0
                && data.chunks_exact(4).all(|b| b[2] == 0 && b[3] == 0);
            let units = if wide {
                data.chunks_exact(4)
                    .map(|b| u16::from_le_bytes([b[0], b[1]]))
                    .collect::<Vec<_>>()
            } else {
                if data.len() % 2 != 0 {
                    return Err(invalid("odd Unicode byte length"));
                }
                data.chunks_exact(2)
                    .map(|b| u16::from_le_bytes([b[0], b[1]]))
                    .collect()
            };
            String::from_utf16(&units).map_err(|_| invalid("invalid Unicode text"))?
        } else {
            let (text, errors) = self.encoding.decode_without_bom_handling(data);
            if errors {
                return Err(invalid("invalid codepage text"));
            }
            decode_legacy_escapes_in(&text, self.encoding)
        };
        // Tolerate independent writers that include a terminal zero in String32.
        Ok(value.strip_suffix('\0').unwrap_or(&value).to_owned())
    }
}

pub(crate) fn read(
    data: &[u8],
    version: DxfVersion,
    encoding: LegacyEncoding,
) -> Result<Vec<FileDependency>> {
    let mut r = Reader {
        bytes: data,
        unicode: version >= DxfVersion::AC1021,
        encoding,
    };
    let count = r.u32()? as usize;
    if count > r.bytes.len() / 4 {
        return Err(invalid("feature count exceeds section"));
    }
    let mut features = Vec::new();
    features
        .try_reserve(count)
        .map_err(|_| invalid("feature allocation failed"))?;
    for _ in 0..count {
        features.push(r.text()?);
    }
    let count = r.u32()? as usize;
    if count > r.bytes.len() / 34 {
        return Err(invalid("file count exceeds section"));
    }
    let mut files = Vec::new();
    files
        .try_reserve(count)
        .map_err(|_| invalid("file allocation failed"))?;
    for _ in 0..count {
        let file_name = r.text()?;
        let found_path = r.text()?;
        let fingerprint = r.text()?;
        let version_guid = r.text()?;
        let feature_index = r.u32()? as usize;
        let feature = features
            .get(feature_index)
            .ok_or_else(|| invalid("invalid feature index"))?
            .clone();
        let timestamp = r.u32()?;
        let file_size = r.u32()?;
        // A one-byte flag in a two-byte slot: some writers leave the upper
        // byte as uninitialized padding (0x50 seen).
        let affects_graphics = match r.take(2)?[0] {
            0 => false,
            1 => true,
            _ => return Err(invalid("invalid graphics flag")),
        };
        let reference_count = r.u32()?;
        files.push(FileDependency {
            feature,
            file_name,
            found_path,
            fingerprint_guid: (!fingerprint.is_empty()).then_some(fingerprint),
            version_guid: (!version_guid.is_empty()).then_some(version_guid),
            timestamp,
            file_size,
            affects_graphics,
            reference_count,
        });
    }
    // Decompressed sections may carry zero alignment padding.
    if r.bytes.iter().any(|b| *b != 0) {
        return Err(invalid("trailing section data"));
    }
    Ok(files)
}

fn text(out: &mut Vec<u8>, value: &str, unicode: bool, encoding: LegacyEncoding) -> Result<()> {
    if value.contains('\0') {
        return Err(invalid("embedded text terminator"));
    }
    let bytes = if unicode {
        value
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    } else {
        encode_legacy_string(value, encoding)
    };
    let len = u32::try_from(bytes.len()).map_err(|_| invalid("text too long"))?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&bytes);
    Ok(())
}

pub(crate) fn write(
    files: &[FileDependency],
    version: DxfVersion,
    encoding: LegacyEncoding,
) -> Result<Vec<u8>> {
    let mut features: Vec<&str> = Vec::new();
    for file in files {
        if !features.contains(&file.feature.as_str()) {
            features.push(&file.feature);
        }
    }
    let mut out = Vec::new();
    let feature_count = u32::try_from(features.len()).map_err(|_| invalid("too many features"))?;
    out.extend_from_slice(&feature_count.to_le_bytes());
    let unicode = version >= DxfVersion::AC1021;
    for feature in &features {
        text(&mut out, feature, unicode, encoding)?;
    }
    let file_count = u32::try_from(files.len()).map_err(|_| invalid("too many files"))?;
    out.extend_from_slice(&file_count.to_le_bytes());
    for file in files {
        for value in [
            file.file_name.as_str(),
            file.found_path.as_str(),
            file.fingerprint_guid.as_deref().unwrap_or(""),
            file.version_guid.as_deref().unwrap_or(""),
        ] {
            text(&mut out, value, unicode, encoding)?;
        }
        let index = features.iter().position(|v| *v == file.feature).unwrap() as u32;
        for value in [index, file.timestamp, file.file_size] {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(&u16::from(file.affects_graphics).to_le_bytes());
        out.extend_from_slice(&file.reference_count.to_le_bytes());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn encoding() -> LegacyEncoding {
        crate::io::dxf::code_page::encoding_from_dwg_code_page(30)
    }
    #[test]
    fn truncated_lengths_indexes_and_counts_are_rejected() {
        let dependency = FileDependency {
            feature: "Acad:XRef".into(),
            file_name: "a.dwg".into(),
            found_path: String::new(),
            fingerprint_guid: None,
            version_guid: None,
            timestamp: 0,
            file_size: 0,
            affects_graphics: true,
            reference_count: 1,
        };
        let bytes = write(&[dependency], DxfVersion::AC1032, encoding()).unwrap();
        for end in 0..bytes.len() {
            assert!(
                read(&bytes[..end], DxfVersion::AC1032, encoding()).is_err(),
                "{end}"
            );
        }
        let mut bad = bytes.clone();
        bad[..4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(read(&bad, DxfVersion::AC1032, encoding()).is_err());
        let mut bad = bytes.clone();
        bad[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(read(&bad, DxfVersion::AC1032, encoding()).is_err());
        let mut bad = bytes.clone();
        let index = bad.len() - 18;
        bad[index..index + 4].copy_from_slice(&1u32.to_le_bytes());
        assert!(read(&bad, DxfVersion::AC1032, encoding()).is_err());
    }
}
