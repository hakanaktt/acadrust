//! Decode DXF groups stored in a DWG proxy object's filer streams.

use crate::io::dwg::dwg_stream_readers::bit_reader::DwgBitReader;
use crate::io::dwg::DwgVersion;
use crate::io::dxf::GroupCodeValueType;
use crate::objects::{ProxyObjectReference, ProxyPayload};
use crate::types::DxfVersion;

type DecodeResult<T> = Result<T, String>;

/// Restore the proxy object's own DXF groups from its main, text and handle
/// streams. The two leading bit-longs repeat the proxy and class ids stored in
/// the object header; the writer may legitimately renumber the latter.
pub(crate) fn decode_proxy_dxf_data(
    payload: &ProxyPayload,
    text_payload: &ProxyPayload,
    object_ids: &[ProxyObjectReference],
) -> DecodeResult<Vec<(i32, String)>> {
    let mut main = stream_reader(payload, DwgVersion::AC18, DxfVersion::AC1018);
    let mut text = stream_reader(text_payload, DwgVersion::AC21, DxfVersion::AC1021);
    let has_text_stream = text_payload.bit_count != 0;

    let _encoded_proxy_id = read_bit_long(&mut main)?;
    let _encoded_class_id = read_bit_long(&mut main)?;

    let mut groups = Vec::new();
    let mut next_object_id = 0usize;
    loop {
        let code = i32::from(read_bit_short(&mut main)?);
        if code == 0 {
            break;
        }

        // A DXF filer writes an X coordinate and its two following axes in one
        // operation: the Y/Z group codes are implicit in the bit stream.
        if GroupCodeValueType::coordinate_axis_raw(code) == Some(0) {
            for axis in 0..3 {
                let value = read_bit_double(&mut main)?;
                groups.push((code + axis * 10, format_double(value)?));
            }
            continue;
        }

        // ODA keeps 1005 in the text stream although the regular DXF group
        // table classifies it as a handle.
        let value = if code == 1005 {
            read_string(&mut main, &mut text, has_text_stream)?
        } else {
            match GroupCodeValueType::from_raw_code(code) {
                GroupCodeValueType::Int32 => read_bit_long(&mut main)?.to_string(),
                GroupCodeValueType::Int16 => read_bit_short(&mut main)?.to_string(),
                GroupCodeValueType::Bool => i32::from(read_bit_short(&mut main)? != 0).to_string(),
                GroupCodeValueType::Byte => read_bit_short(&mut main)?.to_string(),
                GroupCodeValueType::Int64 => read_bit_long_long(&mut main)?.to_string(),
                GroupCodeValueType::Double => format_double(read_bit_double(&mut main)?)?,
                GroupCodeValueType::String => read_string(&mut main, &mut text, has_text_stream)?,
                GroupCodeValueType::BinaryData => {
                    let length = read_bit_long(&mut main)?;
                    let length = usize::try_from(length)
                        .map_err(|_| format!("negative binary group length at {code}"))?;
                    uppercase_hex(&read_exact(&mut main, length)?)
                }
                GroupCodeValueType::Handle => {
                    let reference = object_ids
                        .get(next_object_id)
                        .ok_or_else(|| format!("missing DWG handle for group {code}"))?;
                    next_object_id += 1;
                    format!("{:X}", reference.handle.value())
                }
                GroupCodeValueType::None | GroupCodeValueType::Point3D => {
                    return Err(format!("unknown value type for group {code}"));
                }
            }
        };
        groups.push((code, value));
    }

    finish_stream(&mut main, "main")?;
    finish_stream(&mut text, "text")?;
    if next_object_id != object_ids.len() {
        return Err(format!(
            "DXF filer consumed {next_object_id} handles out of {}",
            object_ids.len()
        ));
    }
    Ok(groups)
}

fn stream_reader(
    payload: &ProxyPayload,
    version: DwgVersion,
    dxf_version: DxfVersion,
) -> DwgBitReader {
    let mut reader = DwgBitReader::new(payload.data(), version, dxf_version);
    reader.set_end_bit(i64::from(payload.bit_count));
    reader
}

/// Decode on a clone so a malformed value cannot poison the source cursor.
fn transactional<T>(
    reader: &mut DwgBitReader,
    decode: impl FnOnce(&mut DwgBitReader) -> DecodeResult<T>,
) -> DecodeResult<T> {
    let mut next = reader.clone();
    let value = decode(&mut next)?;
    next.check_error().map_err(|error| error.to_string())?;
    *reader = next;
    Ok(value)
}

fn read_bit_short(reader: &mut DwgBitReader) -> DecodeResult<i16> {
    transactional(reader, |next| {
        next.try_read_bit_short()
            .ok_or_else(|| "truncated BS value in DXF filer".into())
    })
}

fn read_bit_long(reader: &mut DwgBitReader) -> DecodeResult<i32> {
    transactional(reader, |next| {
        if next.remaining_bits() < 2 {
            return Err("truncated BL prefix in DXF filer".into());
        }
        match next.read_2bits() {
            0 => {
                let bytes = read_exact(next, 4)?;
                let mut raw = [0u8; 4];
                raw.copy_from_slice(&bytes);
                Ok(i32::from_le_bytes(raw))
            }
            1 => Ok(i32::from(read_exact(next, 1)?[0])),
            2 => Ok(0),
            3 => Err("reserved BL prefix in DXF filer".into()),
            _ => unreachable!(),
        }
    })
}

fn read_bit_long_long(reader: &mut DwgBitReader) -> DecodeResult<i64> {
    transactional(reader, |next| {
        if next.remaining_bits() < 3 {
            return Err("truncated BLL prefix in DXF filer".into());
        }
        let mut length = 0usize;
        for _ in 0..3 {
            length = (length << 1) | usize::from(next.read_bit());
        }
        let bytes = read_exact(next, length)?;
        let mut raw = [0u8; 8];
        raw[..length].copy_from_slice(&bytes);
        Ok(u64::from_le_bytes(raw) as i64)
    })
}

fn read_bit_double(reader: &mut DwgBitReader) -> DecodeResult<f64> {
    transactional(reader, |next| {
        if next.remaining_bits() < 2 {
            return Err("truncated BD prefix in DXF filer".into());
        }
        match next.read_2bits() {
            0 => {
                let bytes = read_exact(next, 8)?;
                let mut raw = [0u8; 8];
                raw.copy_from_slice(&bytes);
                Ok(f64::from_le_bytes(raw))
            }
            1 => Ok(1.0),
            2 => Ok(0.0),
            3 => Err("reserved BD prefix in DXF filer".into()),
            _ => unreachable!(),
        }
    })
}

fn format_double(value: f64) -> DecodeResult<String> {
    if !value.is_finite() {
        return Err("non-finite double in DXF filer".into());
    }
    Ok(ryu::Buffer::new().format_finite(value).to_owned())
}

fn read_string(
    main: &mut DwgBitReader,
    text: &mut DwgBitReader,
    has_text_stream: bool,
) -> DecodeResult<String> {
    let reader = if has_text_stream { text } else { main };
    transactional(reader, |next| {
        next.try_read_variable_text()
            .ok_or_else(|| "truncated string in DXF filer".into())
    })
}

fn read_exact(reader: &mut DwgBitReader, length: usize) -> DecodeResult<Vec<u8>> {
    let bits = length
        .checked_mul(8)
        .and_then(|bits| i64::try_from(bits).ok())
        .ok_or_else(|| "value length overflows the DXF filer stream".to_string())?;
    if reader.remaining_bits() < bits {
        return Err(format!(
            "truncated value in DXF filer: need {bits} bits, have {}",
            reader.remaining_bits()
        ));
    }
    let bytes = reader.read_bytes(length);
    reader.check_error().map_err(|error| error.to_string())?;
    Ok(bytes)
}

fn finish_stream(reader: &mut DwgBitReader, name: &str) -> DecodeResult<()> {
    reader.check_error().map_err(|error| error.to_string())?;
    let remaining = reader.remaining_bits();
    if remaining >= 8 {
        return Err(format!(
            "{remaining} unread bits after BS 0 in the {name} stream"
        ));
    }
    for _ in 0..remaining {
        if reader.read_bit() {
            return Err(format!("non-zero padding in the {name} stream"));
        }
    }
    Ok(())
}

fn uppercase_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789ABCDEF";
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0F)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::dwg::dwg_stream_writers::bit_writer::DwgBitWriter;
    use crate::objects::{ProxyObjectReference, ProxyReferenceKind};
    use crate::types::Handle;

    fn payload(
        version: DwgVersion,
        dxf_version: DxfVersion,
        write: impl FnOnce(&mut DwgBitWriter),
    ) -> ProxyPayload {
        let mut writer = DwgBitWriter::new(version, dxf_version);
        write(&mut writer);
        let bit_count = writer.position_in_bits() as u32;
        ProxyPayload::from_bits(&writer.into_bytes(), bit_count)
    }

    fn main_payload(write: impl FnOnce(&mut DwgBitWriter)) -> ProxyPayload {
        payload(DwgVersion::AC18, DxfVersion::AC1018, write)
    }

    fn header(writer: &mut DwgBitWriter) {
        writer.write_bit_long(499);
        writer.write_bit_long(543);
    }

    #[test]
    fn decodes_legacy_inline_string_and_signed_byte() {
        let main = main_payload(|writer| {
            header(writer);
            writer.write_bit_short(1);
            writer.write_variable_text("legacy");
            writer.write_bit_short(280);
            writer.write_bit_short(-1);
            writer.write_bit_short(0);
        });
        assert_eq!(
            decode_proxy_dxf_data(&main, &ProxyPayload::default(), &[]).unwrap(),
            vec![(1, "legacy".into()), (280, "-1".into())]
        );
    }

    #[test]
    fn decodes_bool_and_a_three_dimensional_point() {
        let main = main_payload(|writer| {
            header(writer);
            writer.write_bit_short(290);
            writer.write_bit_short(1);
            writer.write_bit_short(10);
            writer.write_bit_double(1.5);
            writer.write_bit_double(2.5);
            writer.write_bit_double(3.5);
            writer.write_bit_short(0);
        });
        assert_eq!(
            decode_proxy_dxf_data(&main, &ProxyPayload::default(), &[]).unwrap(),
            vec![
                (290, "1".into()),
                (10, "1.5".into()),
                (20, "2.5".into()),
                (30, "3.5".into()),
            ]
        );
    }

    #[test]
    fn string_handle_1005_does_not_consume_object_ids() {
        let main = main_payload(|writer| {
            header(writer);
            writer.write_bit_short(1005);
            writer.write_bit_short(0);
        });
        let text = payload(DwgVersion::AC21, DxfVersion::AC1021, |writer| {
            writer.write_variable_text("9FCBA");
        });
        assert_eq!(
            decode_proxy_dxf_data(&main, &text, &[]).unwrap(),
            vec![(1005, "9FCBA".into())]
        );
    }

    #[test]
    fn rejects_truncated_double_without_accepting_a_partial_group() {
        let main = main_payload(|writer| {
            header(writer);
            writer.write_bit_short(40);
            writer.write_2bits(0);
        });
        assert!(decode_proxy_dxf_data(&main, &ProxyPayload::default(), &[]).is_err());
    }

    #[test]
    fn rejects_trailing_data_nonzero_padding_and_unused_references() {
        let trailing = main_payload(|writer| {
            header(writer);
            writer.write_bit_short(0);
            writer.write_byte(0);
        });
        assert!(decode_proxy_dxf_data(&trailing, &ProxyPayload::default(), &[]).is_err());

        let padding = main_payload(|writer| {
            header(writer);
            writer.write_bit_short(0);
            writer.write_bit(true);
        });
        assert!(decode_proxy_dxf_data(&padding, &ProxyPayload::default(), &[]).is_err());

        let unused = main_payload(|writer| {
            header(writer);
            writer.write_bit_short(0);
        });
        let references = [ProxyObjectReference {
            handle: Handle::from(0x9FCBA),
            kind: ProxyReferenceKind::SoftPointer,
        }];
        assert!(decode_proxy_dxf_data(&unused, &ProxyPayload::default(), &references).is_err());
    }
}
