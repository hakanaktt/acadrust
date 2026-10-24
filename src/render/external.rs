//! DWF opcodes whose geometry is explicit in the file.
//!
//! ASCII WHIP `(Line x,y x,y)` records become strokes. A DWFx package and any
//! other opcode are refused so the caller can say the vectors were not decoded.

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DwfVectors {
    pub strokes: Vec<Vec<(f64, f64)>>,
}

pub(crate) fn read_dwf(bytes: &[u8]) -> Result<DwfVectors, &'static str> {
    if bytes.starts_with(b"PK") {
        return Err("dwfx package vectors were not decoded");
    }
    let text = String::from_utf8_lossy(bytes);
    if !text.contains("(DWF") && !text.contains("DWF V") {
        return Err("dwf opcodes were not decoded");
    }
    let mut strokes = Vec::new();
    for record in text.split('(').skip(1) {
        let body = record.split(')').next().unwrap_or("");
        let mut words = body.split_whitespace();
        let Some(opcode) = words.next() else {
            continue;
        };
        if !opcode.eq_ignore_ascii_case("Line") {
            continue;
        }
        let mut numbers = Vec::new();
        for word in words {
            for piece in word.split(',') {
                if piece.is_empty() {
                    continue;
                }
                let Ok(value) = piece.parse::<f64>() else {
                    break;
                };
                numbers.push(value);
            }
        }
        if numbers.len() >= 4 {
            strokes.push(vec![(numbers[0], numbers[1]), (numbers[2], numbers[3])]);
        }
    }
    if strokes.is_empty() {
        Err("dwf file had no supported opcodes")
    } else {
        Ok(DwfVectors { strokes })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ascii_line_opcode_is_kept() {
        let bytes = b"(DWF V06.00)\n(Line 1,2 3,4)\n";
        let vectors = read_dwf(bytes).unwrap();
        assert_eq!(vectors.strokes, vec![vec![(1.0, 2.0), (3.0, 4.0)]]);
    }

    #[test]
    fn a_dwfx_package_is_refused() {
        let error = read_dwf(b"PK\x03\x04").unwrap_err();
        assert_eq!(error, "dwfx package vectors were not decoded");
    }
}
