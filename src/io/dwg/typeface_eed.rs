
pub(crate) const DEFAULT_FONT_FLAGS: i32 = 34;

pub(crate) fn encode(wide: bool, typeface: &str, flags: i32) -> Vec<u8> {
    let mut b = vec![0];
    if wide {
        let units: Vec<u16> = typeface.encode_utf16().collect();
        b.extend_from_slice(&(units.len() as u16).to_le_bytes());
        for u in units {
            b.extend_from_slice(&u.to_le_bytes());
        }
    } else {
        let bytes = typeface.as_bytes();
        b.push(bytes.len().min(255) as u8);
        b.extend_from_slice(&0u16.to_le_bytes()); // codepage
        b.extend_from_slice(&bytes[..bytes.len().min(255)]);
    }
    b.push(71);
    b.extend_from_slice(&flags.to_le_bytes());
    b
}

pub(crate) fn decode(bytes: &[u8], wide: bool) -> Option<(String, i32)> {
    let mut i = 0usize;
    let mut typeface: Option<String> = None;
    let mut flags = DEFAULT_FONT_FLAGS;
    while i < bytes.len() {
        let code = bytes[i];
        i += 1;
        match code {
            0 => {
                let text = if wide {
                    let n = u16::from_le_bytes([*bytes.get(i)?, *bytes.get(i + 1)?]) as usize;
                    let units: Vec<u16> = bytes
                        .get(i + 2..i + 2 + n * 2)?
                        .chunks_exact(2)
                        .map(|c| u16::from_le_bytes([c[0], c[1]]))
                        .collect();
                    i += 2 + n * 2;
                    String::from_utf16_lossy(&units)
                } else {
                    let n = *bytes.get(i)? as usize;
                    let text = String::from_utf8_lossy(bytes.get(i + 3..i + 3 + n)?).into_owned();
                    i += 3 + n;
                    text
                };
                typeface.get_or_insert(text);
            }
            2 => i += 1,
            4 => {
                let n = *bytes.get(i)? as usize;
                i += 1 + n;
            }
            3 | 5 => i += 8,
            10..=13 => i += 24,
            40..=42 => i += 8,
            70 => i += 2,
            71 => {
                flags = i32::from_le_bytes(bytes.get(i..i + 4)?.try_into().ok()?);
                i += 4;
            }
            _ => return None,
        }
    }
    typeface.map(|t| (t, flags))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        for wide in [true, false] {
            let bytes = encode(wide, "Arial", DEFAULT_FONT_FLAGS);
            assert_eq!(decode(&bytes, wide), Some(("Arial".to_string(), 34)));
        }
    }
}
