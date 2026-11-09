//! Strict framing and payload validation, separate from the permissive byte-preservation API.
use super::*;

/// Why a checked proxy stream cannot be represented as the supplied records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyGraphicErrorReason {
    Framing,
    Count,
    Overflow,
    PayloadLength,
    InvalidValue,
    InvalidString,
}

/// Location of a checked proxy decoding or encoding failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "proxy graphics at byte {byte_offset}, record {record_index:?} ({record_type:?}): {reason:?}"
)]
pub struct ProxyGraphicError {
    pub record_index: Option<usize>,
    pub byte_offset: usize,
    pub record_type: Option<u32>,
    pub reason: ProxyGraphicErrorReason,
}

/// A complete proxy metafile decoded with validation and typed commands.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CheckedProxyGraphics {
    pub records: Vec<CheckedProxyGraphicRecord>,
}

/// Records of a checked metafile. Unknown types retain their original bytes.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CheckedProxyGraphicRecord {
    /// Type 21: the empty record recognized by the legacy decoder.
    FillOff,
    /// Type 36: single-line UTF-16 text.
    UnicodeText(ProxyUnicodeText),
    /// A typed drawing or trait command.
    Command(ProxyGraphicCommand),
    /// A record whose type is outside the checked decoder's command set.
    Unknown { record_type: u32, data: Vec<u8> },
}

impl CheckedProxyGraphics {
    /// Decode exactly one complete metafile. Known malformed payloads are errors;
    /// future record types retain their bytes as `Unknown`.
    pub fn decode_checked(data: &[u8]) -> Result<Self, ProxyGraphicError> {
        use ProxyGraphicErrorReason as R;
        let header_error = |reason| ProxyGraphicError {
            record_index: None,
            byte_offset: 0,
            record_type: None,
            reason,
        };
        let total = read_u32(data, 0).ok_or_else(|| header_error(R::Framing))? as usize;
        let count = read_u32(data, 4).ok_or_else(|| header_error(R::Framing))? as usize;
        if total != data.len() || total < HEADER_SIZE {
            return Err(header_error(R::Framing));
        }
        if count > (total - HEADER_SIZE) / RECORD_HEADER_SIZE {
            return Err(header_error(R::Count));
        }
        let mut records = Vec::with_capacity(count);
        let mut offset = HEADER_SIZE;
        for index in 0..count {
            let mut error = ProxyGraphicError {
                record_index: Some(index),
                byte_offset: offset,
                record_type: None,
                reason: R::Framing,
            };
            let size = read_u32(data, offset).ok_or_else(|| error.clone())? as usize;
            let kind = read_u32(data, offset + 4).ok_or_else(|| error.clone())?;
            error.record_type = Some(kind);
            let end = offset.checked_add(size).ok_or_else(|| {
                error.reason = R::Overflow;
                error.clone()
            })?;
            if size < RECORD_HEADER_SIZE || end > total {
                return Err(error);
            }
            let payload = &data[offset + RECORD_HEADER_SIZE..end];
            let command = super::records::decode(kind, payload).map_err(|reason| {
                error.reason = reason;
                error.clone()
            })?;
            if command.is_none() {
                validate_payload(kind, payload).map_err(|reason| {
                    error.reason = reason;
                    error
                })?;
            }
            records.push(
                command
                    .map(CheckedProxyGraphicRecord::Command)
                    .unwrap_or_else(|| match decode_record(kind, payload) {
                        ProxyGraphicRecord::FillOff => CheckedProxyGraphicRecord::FillOff,
                        ProxyGraphicRecord::UnicodeText(text) => {
                            CheckedProxyGraphicRecord::UnicodeText(text)
                        }
                        ProxyGraphicRecord::Unknown { record_type, data } => {
                            CheckedProxyGraphicRecord::Unknown { record_type, data }
                        }
                    }),
            );
            offset = end;
        }
        if offset != total {
            return Err(header_error(R::Count));
        }
        Ok(Self { records })
    }

    /// Encode a complete checked stream. This validates known payloads even when
    /// the caller supplied them through the permissive `Unknown` representation.
    pub fn encode_checked(&self) -> Result<Vec<u8>, ProxyGraphicError> {
        use ProxyGraphicErrorReason as R;
        let mut output = vec![0; HEADER_SIZE];
        let count = u32::try_from(self.records.len()).map_err(|_| ProxyGraphicError {
            record_index: None,
            byte_offset: 4,
            record_type: None,
            reason: R::Overflow,
        })?;
        output[4..8].copy_from_slice(&count.to_le_bytes());
        for (index, record) in self.records.iter().enumerate() {
            let mut error = ProxyGraphicError {
                record_index: Some(index),
                byte_offset: output.len(),
                record_type: None,
                reason: R::InvalidValue,
            };
            if let CheckedProxyGraphicRecord::UnicodeText(text) = record {
                if text.text.contains('\0') {
                    error.reason = R::InvalidString;
                    return Err(error);
                }
            }
            let (kind, payload) = match record {
                CheckedProxyGraphicRecord::FillOff => (21, Vec::new()),
                CheckedProxyGraphicRecord::UnicodeText(text) => (36, encode_unicode_text(text)),
                CheckedProxyGraphicRecord::Command(command) => {
                    super::records::encode(command).ok_or_else(|| error.clone())?
                }
                CheckedProxyGraphicRecord::Unknown { record_type, data } => {
                    (*record_type, data.clone())
                }
            };
            error.record_type = Some(kind);
            validate_payload(kind, &payload).map_err(|reason| {
                error.reason = reason;
                error.clone()
            })?;
            let size = payload
                .len()
                .checked_add(RECORD_HEADER_SIZE)
                .and_then(|size| u32::try_from(size).ok())
                .ok_or_else(|| {
                    error.reason = R::Overflow;
                    error.clone()
                })?;
            let total = output
                .len()
                .checked_add(size as usize)
                .filter(|&total| u32::try_from(total).is_ok())
                .ok_or_else(|| {
                    error.reason = R::Overflow;
                    error.clone()
                })?;
            output.reserve(total - output.len());
            output.extend_from_slice(&size.to_le_bytes());
            output.extend_from_slice(&kind.to_le_bytes());
            output.extend_from_slice(&payload);
        }
        let total = output.len() as u32;
        output[..4].copy_from_slice(&total.to_le_bytes());
        Ok(output)
    }
}

fn validate_payload(kind: u32, data: &[u8]) -> Result<(), ProxyGraphicErrorReason> {
    use ProxyGraphicErrorReason as R;
    if super::records::decode(kind, data)?.is_some() {
        return Ok(());
    }
    match kind {
        21 if !data.is_empty() => Err(R::PayloadLength),
        36 => {
            if data.len() < UNICODE_TEXT_FIXED_SIZE + 2 {
                return Err(R::PayloadLength);
            }
            for offset in (0..UNICODE_TEXT_FIXED_SIZE).step_by(8) {
                if !read_f64(data, offset).ok_or(R::PayloadLength)?.is_finite() {
                    return Err(R::InvalidValue);
                }
            }
            let tail = &data[UNICODE_TEXT_FIXED_SIZE..];
            let terminator = tail
                .chunks_exact(2)
                .position(|b| b == [0, 0])
                .ok_or(R::InvalidString)?;
            let text_end = UNICODE_TEXT_FIXED_SIZE + 2 * (terminator + 1);
            let padded_end = text_end.checked_add(3).ok_or(R::Overflow)? & !3;
            if data.len() != padded_end {
                return Err(R::PayloadLength);
            }
            decode_unicode_text(data).ok_or(R::InvalidString)?;
            Ok(())
        }
        _ => Ok(()),
    }
}
