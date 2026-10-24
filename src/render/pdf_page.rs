//! The PDF page operators that describe a line or a filled outline.
//!
//! Text, images, and shadings in the content stream are left undrawn. The
//! page rectangle still comes from the MediaBox, so the underlay keeps its
//! paper size.

use std::io::Read;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PdfVectors {
    pub media_min: (f64, f64),
    pub media_max: (f64, f64),
    pub strokes: Vec<Vec<(f64, f64)>>,
    pub fills: Vec<Vec<(f64, f64)>>,
}

pub(crate) fn read_page(bytes: &[u8], page_number: usize) -> Result<PdfVectors, &'static str> {
    let objects = objects_of(bytes);
    let pages = page_bodies(&objects);
    let page = pages.get(page_number).ok_or("pdf page was not found")?;
    let (media_min, media_max) = media_box(page).unwrap_or(((0.0, 0.0), (612.0, 792.0)));
    let mut stream = Vec::new();
    for reference in content_refs(page) {
        if let Some(body) = objects.get(&reference) {
            stream.extend(stream_bytes(body)?);
        }
    }
    let mut vectors = interpret(&stream);
    vectors.media_min = media_min;
    vectors.media_max = media_max;
    Ok(vectors)
}

fn objects_of(bytes: &[u8]) -> std::collections::HashMap<(u32, u16), String> {
    let text = String::from_utf8_lossy(bytes);
    let mut objects = std::collections::HashMap::new();
    let mut rest = text.as_ref();
    while let Some(start) = rest.find(" obj") {
        let head = &rest[..start];
        let mut numbers = head.split_whitespace().rev();
        let generation = numbers.next().and_then(|word| word.parse::<u16>().ok());
        let number = numbers.next().and_then(|word| word.parse::<u32>().ok());
        let Some((number, generation)) = number.zip(generation) else {
            rest = &rest[start + 4..];
            continue;
        };
        let body_at = start + 4;
        let Some(end) = rest[body_at..].find("endobj") else {
            break;
        };
        objects.insert(
            (number, generation),
            rest[body_at..body_at + end].to_string(),
        );
        rest = &rest[body_at + end + 6..];
    }
    objects
}

fn page_bodies(objects: &std::collections::HashMap<(u32, u16), String>) -> Vec<&str> {
    let mut pages = Vec::new();
    let mut keys: Vec<_> = objects.keys().copied().collect();
    keys.sort();
    for key in keys {
        let body = &objects[&key];
        if body.contains("/Type") && body.contains("/Page") && !body.contains("/Pages") {
            pages.push(body.as_str());
        }
    }
    pages
}

fn media_box(body: &str) -> Option<((f64, f64), (f64, f64))> {
    let at = body.find("/MediaBox")?;
    let open = body[at..].find('[')? + at;
    let close = body[open..].find(']')? + open;
    let numbers: Vec<f64> = body[open + 1..close]
        .split_whitespace()
        .filter_map(|word| word.parse().ok())
        .collect();
    if numbers.len() < 4 {
        return None;
    }
    Some(((numbers[0], numbers[1]), (numbers[2], numbers[3])))
}

fn content_refs(body: &str) -> Vec<(u32, u16)> {
    let Some(at) = body.find("/Contents") else {
        return Vec::new();
    };
    let tail = &body[at + "/Contents".len()..];
    let mut refs = Vec::new();
    let mut words = tail.split_whitespace();
    if tail.trim_start().starts_with('[') {
        while let Some(word) = words.next() {
            if word == "]" {
                break;
            }
            push_ref(&mut refs, word, words.next());
        }
    } else if let Some(word) = words.next() {
        push_ref(&mut refs, word, words.next());
    }
    refs
}

fn push_ref(refs: &mut Vec<(u32, u16)>, number: &str, generation: Option<&str>) {
    let number = number.trim_start_matches('[');
    let Some(number) = number.parse::<u32>().ok() else {
        return;
    };
    let generation = generation
        .and_then(|word| word.trim_end_matches('R').parse().ok())
        .unwrap_or(0);
    refs.push((number, generation));
}

fn stream_bytes(body: &str) -> Result<Vec<u8>, &'static str> {
    let Some(marker) = body.find("stream") else {
        return Ok(Vec::new());
    };
    let mut start = marker + "stream".len();
    if body.as_bytes().get(start) == Some(&b'\r') {
        start += 1;
    }
    if body.as_bytes().get(start) == Some(&b'\n') {
        start += 1;
    }
    let end = body[start..]
        .find("endstream")
        .ok_or("pdf page was not found")?
        + start;
    let mut data = body.as_bytes()[start..end].to_vec();
    if data.last() == Some(&b'\n') {
        data.pop();
        if data.last() == Some(&b'\r') {
            data.pop();
        }
    }
    if body[..marker].contains("/FlateDecode") {
        let mut inflated = Vec::new();
        flate2::read::ZlibDecoder::new(data.as_slice())
            .read_to_end(&mut inflated)
            .map_err(|_| "pdf content stream was not inflated")?;
        return Ok(inflated);
    }
    Ok(data)
}

fn interpret(stream: &[u8]) -> PdfVectors {
    let text = String::from_utf8_lossy(stream);
    let tokens = tokenize(&text);
    let mut vectors = PdfVectors {
        media_min: (0.0, 0.0),
        media_max: (1.0, 1.0),
        strokes: Vec::new(),
        fills: Vec::new(),
    };
    let mut stack: Vec<f64> = Vec::new();
    let mut subpath: Vec<(f64, f64)> = Vec::new();
    let mut path: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut cursor = (0.0, 0.0);
    let mut ctm = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mut ctm_stack = Vec::new();
    for token in tokens {
        if let Ok(number) = token.parse::<f64>() {
            stack.push(number);
            continue;
        }
        match token.as_str() {
            "q" => ctm_stack.push(ctm),
            "Q" => {
                if let Some(saved) = ctm_stack.pop() {
                    ctm = saved;
                }
            }
            "cm" => {
                if stack.len() >= 6 {
                    let f = stack.pop().unwrap();
                    let e = stack.pop().unwrap();
                    let d = stack.pop().unwrap();
                    let c = stack.pop().unwrap();
                    let b = stack.pop().unwrap();
                    let a = stack.pop().unwrap();
                    ctm = concat_ctm(ctm, [a, b, c, d, e, f]);
                }
                stack.clear();
            }
            "m" => {
                if stack.len() >= 2 {
                    let y = stack.pop().unwrap();
                    let x = stack.pop().unwrap();
                    if subpath.len() > 1 {
                        path.push(std::mem::take(&mut subpath));
                    } else {
                        subpath.clear();
                    }
                    cursor = apply_ctm(ctm, (x, y));
                    subpath.push(cursor);
                }
            }
            "l" => {
                if stack.len() >= 2 {
                    let y = stack.pop().unwrap();
                    let x = stack.pop().unwrap();
                    cursor = apply_ctm(ctm, (x, y));
                    if subpath.is_empty() {
                        subpath.push(cursor);
                    } else {
                        subpath.push(cursor);
                    }
                }
            }
            "c" => {
                if stack.len() >= 6 {
                    let y3 = stack.pop().unwrap();
                    let x3 = stack.pop().unwrap();
                    let y2 = stack.pop().unwrap();
                    let x2 = stack.pop().unwrap();
                    let y1 = stack.pop().unwrap();
                    let x1 = stack.pop().unwrap();
                    let start = cursor;
                    for step in 1..=8 {
                        let t = step as f64 / 8.0;
                        subpath.push(cubic(
                            start,
                            apply_ctm(ctm, (x1, y1)),
                            apply_ctm(ctm, (x2, y2)),
                            apply_ctm(ctm, (x3, y3)),
                            t,
                        ));
                    }
                    cursor = (x3, y3);
                }
            }
            "h" => {
                if let Some(first) = subpath.first().copied() {
                    subpath.push(first);
                }
            }
            "re" => {
                if stack.len() >= 4 {
                    let height = stack.pop().unwrap();
                    let width = stack.pop().unwrap();
                    let y = stack.pop().unwrap();
                    let x = stack.pop().unwrap();
                    if subpath.len() > 1 {
                        path.push(std::mem::take(&mut subpath));
                    }
                    path.push(vec![
                        apply_ctm(ctm, (x, y)),
                        apply_ctm(ctm, (x + width, y)),
                        apply_ctm(ctm, (x + width, y + height)),
                        apply_ctm(ctm, (x, y + height)),
                        apply_ctm(ctm, (x, y)),
                    ]);
                    cursor = apply_ctm(ctm, (x, y));
                }
            }
            "S" | "s" => {
                if token == "s" {
                    if let Some(first) = subpath.first().copied() {
                        subpath.push(first);
                    }
                }
                if subpath.len() > 1 {
                    path.push(std::mem::take(&mut subpath));
                }
                vectors.strokes.extend(path.drain(..));
                stack.clear();
            }
            "f" | "f*" | "B" | "B*" | "b" | "b*" => {
                if matches!(token.as_str(), "b" | "b*") {
                    if let Some(first) = subpath.first().copied() {
                        subpath.push(first);
                    }
                }
                if subpath.len() > 1 {
                    path.push(std::mem::take(&mut subpath));
                }
                if matches!(token.as_str(), "B" | "B*" | "b" | "b*") {
                    vectors.strokes.extend(path.iter().cloned());
                }
                vectors.fills.extend(path.drain(..));
                stack.clear();
            }
            "n" => {
                subpath.clear();
                path.clear();
                stack.clear();
            }
            _ => stack.clear(),
        }
    }
    vectors
}

fn cubic(p0: (f64, f64), p1: (f64, f64), p2: (f64, f64), p3: (f64, f64), t: f64) -> (f64, f64) {
    let u = 1.0 - t;
    let b0 = u * u * u;
    let b1 = 3.0 * u * u * t;
    let b2 = 3.0 * u * t * t;
    let b3 = t * t * t;
    (
        b0 * p0.0 + b1 * p1.0 + b2 * p2.0 + b3 * p3.0,
        b0 * p0.1 + b1 * p1.1 + b2 * p2.1 + b3 * p3.1,
    )
}

fn apply_ctm(matrix: [f64; 6], point: (f64, f64)) -> (f64, f64) {
    (
        matrix[0] * point.0 + matrix[2] * point.1 + matrix[4],
        matrix[1] * point.0 + matrix[3] * point.1 + matrix[5],
    )
}

fn concat_ctm(left: [f64; 6], right: [f64; 6]) -> [f64; 6] {
    [
        left[0] * right[0] + left[2] * right[1],
        left[1] * right[0] + left[3] * right[1],
        left[0] * right[2] + left[2] * right[3],
        left[1] * right[2] + left[3] * right[3],
        left[0] * right[4] + left[2] * right[5] + left[4],
        left[1] * right[4] + left[3] * right[5] + left[5],
    ]
}

fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch.is_whitespace() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            continue;
        }
        if matches!(ch, '[' | ']' | '(' | ')' | '<' | '>' | '/') {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            if ch == '/' {
                let mut name = String::from("/");
                while let Some(next) = chars.peek() {
                    if next.is_whitespace()
                        || matches!(next, '[' | ']' | '(' | ')' | '<' | '>' | '/')
                    {
                        break;
                    }
                    name.push(chars.next().unwrap());
                }
                tokens.push(name);
            }
            continue;
        }
        current.push(ch);
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_line_and_rectangle_keep_their_page_coordinates() {
        let pdf = b"%PDF-1.4\n\
1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n\
2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj\n\
3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R >> endobj\n\
4 0 obj << /Length 28 >> stream\n\
10 10 m 40 10 l S\n\
endstream\n\
endobj\n\
trailer << /Root 1 0 R >>\n";
        let page = read_page(pdf, 0).unwrap();
        assert_eq!(page.media_max, (200.0, 100.0));
        assert_eq!(page.strokes.len(), 1);
        assert_eq!(page.strokes[0][0], (10.0, 10.0));
        assert_eq!(page.strokes[0][1], (40.0, 10.0));
    }
}
