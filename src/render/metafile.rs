//! The EMF and WMF records whose geometry is fully specified.
//!
//! A record that is not one of these is skipped. Nothing is substituted for it.

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MetaPicture {
    pub strokes: Vec<Vec<(f64, f64)>>,
    pub fills: Vec<Vec<(f64, f64)>>,
    /// Device bounds: left, top, right, bottom. Y grows downward.
    pub bounds: (f64, f64, f64, f64),
}

pub(crate) fn parse_emf(bytes: &[u8]) -> Option<MetaPicture> {
    if bytes.len() < 88 || bytes[0..4] != [1, 0, 0, 0] || &bytes[40..44] != b" EMF" {
        return None;
    }
    let left = i32::from_le_bytes(bytes[8..12].try_into().ok()?) as f64;
    let top = i32::from_le_bytes(bytes[12..16].try_into().ok()?) as f64;
    let right = i32::from_le_bytes(bytes[16..20].try_into().ok()?) as f64;
    let bottom = i32::from_le_bytes(bytes[20..24].try_into().ok()?) as f64;
    let mut picture = MetaPicture {
        strokes: Vec::new(),
        fills: Vec::new(),
        bounds: (left, top, right, bottom),
    };
    let mut cursor = u32::from_le_bytes(bytes[4..8].try_into().ok()?) as usize;
    let mut guard = 0;
    while cursor + 8 <= bytes.len() && guard < 100_000 {
        let kind = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().ok()?);
        let size = u32::from_le_bytes(bytes[cursor + 4..cursor + 8].try_into().ok()?) as usize;
        if size < 8 || cursor + size > bytes.len() {
            break;
        }
        let body = &bytes[cursor + 8..cursor + size];
        match kind {
            14 => break,
            4 | 3 => {
                if let Some(points) = pointls_after_bounds(body) {
                    if kind == 3 {
                        picture.fills.push(points);
                    } else {
                        picture.strokes.push(points);
                    }
                }
            }
            43 => {
                if body.len() >= 16 {
                    picture.strokes.push(rect_points(read_rect(body)));
                }
            }
            _ => {}
        }
        cursor += size;
        guard += 1;
    }
    Some(picture)
}

pub(crate) fn parse_wmf(bytes: &[u8]) -> Option<MetaPicture> {
    let (data, bounds) =
        if bytes.len() >= 22 && u32::from_le_bytes(bytes[0..4].try_into().ok()?) == 0x9AC6_CDD7 {
            let left = i16::from_le_bytes(bytes[6..8].try_into().ok()?) as f64;
            let top = i16::from_le_bytes(bytes[8..10].try_into().ok()?) as f64;
            let right = i16::from_le_bytes(bytes[10..12].try_into().ok()?) as f64;
            let bottom = i16::from_le_bytes(bytes[12..14].try_into().ok()?) as f64;
            (&bytes[22..], (left, top, right, bottom))
        } else {
            (bytes, (0.0, 0.0, 1.0, 1.0))
        };
    if data.len() < 18 {
        return None;
    }
    let header_words = u16::from_le_bytes(data[2..4].try_into().ok()?) as usize;
    if header_words < 9 {
        return None;
    }
    let mut picture = MetaPicture {
        strokes: Vec::new(),
        fills: Vec::new(),
        bounds,
    };
    let mut cursor = header_words * 2;
    let mut guard = 0;
    while cursor + 6 <= data.len() && guard < 100_000 {
        let words = u32::from_le_bytes(data[cursor..cursor + 4].try_into().ok()?) as usize;
        let function = u16::from_le_bytes(data[cursor + 4..cursor + 6].try_into().ok()?);
        let size = words.checked_mul(2)?;
        if size < 6 || cursor + size > data.len() {
            break;
        }
        let params = &data[cursor + 6..cursor + size];
        match function {
            0 => break,
            0x0325 => {
                if let Some(points) = wmf_points(params) {
                    picture.strokes.push(points);
                }
            }
            0x0324 => {
                if let Some(points) = wmf_points(params) {
                    picture.fills.push(points);
                }
            }
            0x041B => {
                if params.len() >= 8 {
                    let bottom = i16::from_le_bytes(params[0..2].try_into().ok()?) as f64;
                    let right = i16::from_le_bytes(params[2..4].try_into().ok()?) as f64;
                    let top = i16::from_le_bytes(params[4..6].try_into().ok()?) as f64;
                    let left = i16::from_le_bytes(params[6..8].try_into().ok()?) as f64;
                    picture
                        .strokes
                        .push(rect_points((left, top, right, bottom)));
                }
            }
            _ => {}
        }
        cursor += size;
        guard += 1;
    }
    Some(picture)
}

fn pointls_after_bounds(body: &[u8]) -> Option<Vec<(f64, f64)>> {
    if body.len() < 20 {
        return None;
    }
    let count = u32::from_le_bytes(body[16..20].try_into().ok()?) as usize;
    let mut points = Vec::with_capacity(count);
    for index in 0..count {
        let offset = 20 + index * 8;
        let point = body.get(offset..offset + 8)?;
        let x = i32::from_le_bytes(point[0..4].try_into().ok()?) as f64;
        let y = i32::from_le_bytes(point[4..8].try_into().ok()?) as f64;
        points.push((x, y));
    }
    Some(points)
}

fn wmf_points(params: &[u8]) -> Option<Vec<(f64, f64)>> {
    if params.len() < 2 {
        return None;
    }
    let count = u16::from_le_bytes(params[0..2].try_into().ok()?) as usize;
    let mut points = Vec::with_capacity(count);
    for index in 0..count {
        let offset = 2 + index * 4;
        let point = params.get(offset..offset + 4)?;
        let x = i16::from_le_bytes(point[0..2].try_into().ok()?) as f64;
        let y = i16::from_le_bytes(point[2..4].try_into().ok()?) as f64;
        points.push((x, y));
    }
    Some(points)
}

fn read_rect(body: &[u8]) -> (f64, f64, f64, f64) {
    let left = i32::from_le_bytes(body[0..4].try_into().unwrap_or([0; 4])) as f64;
    let top = i32::from_le_bytes(body[4..8].try_into().unwrap_or([0; 4])) as f64;
    let right = i32::from_le_bytes(body[8..12].try_into().unwrap_or([0; 4])) as f64;
    let bottom = i32::from_le_bytes(body[12..16].try_into().unwrap_or([0; 4])) as f64;
    (left, top, right, bottom)
}

fn rect_points((left, top, right, bottom): (f64, f64, f64, f64)) -> Vec<(f64, f64)> {
    vec![
        (left, top),
        (right, top),
        (right, bottom),
        (left, bottom),
        (left, top),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emf_rectangle_is_the_only_record_that_is_drawn() {
        let mut bytes = vec![0u8; 88];
        bytes[0..4].copy_from_slice(&1u32.to_le_bytes());
        bytes[4..8].copy_from_slice(&88u32.to_le_bytes());
        bytes[16..20].copy_from_slice(&100i32.to_le_bytes());
        bytes[20..24].copy_from_slice(&50i32.to_le_bytes());
        bytes[40..44].copy_from_slice(b" EMF");
        let mut rectangle = vec![0u8; 24];
        rectangle[0..4].copy_from_slice(&43u32.to_le_bytes());
        rectangle[4..8].copy_from_slice(&24u32.to_le_bytes());
        rectangle[8..12].copy_from_slice(&10i32.to_le_bytes());
        rectangle[12..16].copy_from_slice(&10i32.to_le_bytes());
        rectangle[16..20].copy_from_slice(&40i32.to_le_bytes());
        rectangle[20..24].copy_from_slice(&30i32.to_le_bytes());
        bytes.extend_from_slice(&rectangle);
        bytes.extend_from_slice(&14u32.to_le_bytes());
        bytes.extend_from_slice(&8u32.to_le_bytes());
        let picture = parse_emf(&bytes).unwrap();
        assert_eq!(picture.bounds, (0.0, 0.0, 100.0, 50.0));
        assert_eq!(picture.strokes.len(), 1);
        assert_eq!(picture.strokes[0][0], (10.0, 10.0));
        assert_eq!(picture.strokes[0][2], (40.0, 30.0));
        assert!(picture.fills.is_empty());
    }
}
