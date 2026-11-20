//! The XML the package parts use: elements, attributes and text, read and
//! written in the layout the package editor writes (two-space indent,
//! `<a />` for an empty element, text kept on the element's line).

/// One element: qualified name, attributes in document order, children.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

/// A child of an element.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Element(Element),
    Text(String),
}

impl Element {
    pub fn new(name: &str) -> Self {
        Element { name: name.to_string(), ..Default::default() }
    }

    pub fn attr(mut self, name: &str, value: &str) -> Self {
        self.attrs.push((name.to_string(), value.to_string()));
        self
    }

    pub fn child(mut self, e: Element) -> Self {
        self.children.push(Node::Element(e));
        self
    }

    pub fn text(mut self, t: &str) -> Self {
        self.children.push(Node::Text(t.to_string()));
        self
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|a| a.0 == name).map(|a| a.1.as_str())
    }

    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|n| match n {
            Node::Element(e) => Some(e),
            Node::Text(_) => None,
        })
    }

    pub fn first(&self, name: &str) -> Option<&Element> {
        self.elements().find(|e| e.name == name)
    }

    /// The element's text, whitespace-only text left out.
    pub fn inner_text(&self) -> String {
        self.children
            .iter()
            .filter_map(|n| match n {
                Node::Text(t) => Some(t.as_str()),
                Node::Element(_) => None,
            })
            .collect()
    }

    /// The element as a document comparison sees it: attributes in name
    /// order, whitespace-only text dropped.
    pub fn normalized(&self) -> Element {
        let mut attrs = self.attrs.clone();
        attrs.sort();
        let children = self
            .children
            .iter()
            .filter_map(|n| match n {
                Node::Element(e) => Some(Node::Element(e.normalized())),
                Node::Text(t) if t.trim().is_empty() => None,
                Node::Text(t) => Some(Node::Text(t.clone())),
            })
            .collect();
        Element { name: self.name.clone(), attrs, children }
    }
}

fn unescape(s: &str) -> Result<String, String> {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let end = rest[i..].find(';').ok_or("unterminated entity")? + i;
        let ent = &rest[i + 1..end];
        let c = match ent {
            "lt" => '<',
            "gt" => '>',
            "amp" => '&',
            "quot" => '"',
            "apos" => '\'',
            _ if ent.starts_with("#x") || ent.starts_with("#X") => u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32).ok_or("bad entity")?,
            _ if ent.starts_with('#') => ent[1..].parse().ok().and_then(char::from_u32).ok_or("bad entity")?,
            _ => return Err(format!("unknown entity &{ent};")),
        };
        out.push(c);
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// The deepest element nesting a document may have, so a package cannot
/// overflow the stack of the code that walks its elements.
const MAX_DEPTH: usize = 4096;

/// Parses a document; returns its root element.
pub fn parse(text: &str) -> Result<Element, String> {
    let text = text.trim_start_matches('\u{feff}');
    let b = text.as_bytes();
    let mut i = 0;
    let mut stack: Vec<Element> = Vec::new();
    let mut root = None;
    while i < b.len() {
        if b[i] == b'<' {
            if text[i..].starts_with("<?") {
                i += text[i..].find("?>").ok_or("unterminated declaration")? + 2;
            } else if text[i..].starts_with("<!--") {
                i += text[i..].find("-->").ok_or("unterminated comment")? + 3;
            } else if text[i..].starts_with("</") {
                let end = text[i..].find('>').ok_or("unterminated end tag")? + i;
                let name = text[i + 2..end].trim();
                let e = stack.pop().ok_or("unbalanced end tag")?;
                if e.name != name {
                    return Err(format!("end tag {name} closes {}", e.name));
                }
                match stack.last_mut() {
                    Some(p) => p.children.push(Node::Element(e)),
                    None => root = Some(e),
                }
                i = end + 1;
            } else {
                // start tag: name, then attributes until '>' or '/>'
                let mut j = i + 1;
                while j < b.len() && !b[j].is_ascii_whitespace() && b[j] != b'>' && b[j] != b'/' {
                    j += 1;
                }
                let mut e = Element::new(&text[i + 1..j]);
                loop {
                    while j < b.len() && b[j].is_ascii_whitespace() {
                        j += 1;
                    }
                    if j >= b.len() {
                        return Err("unterminated start tag".into());
                    }
                    if b[j] == b'>' || b[j] == b'/' {
                        break;
                    }
                    let eq = text[j..].find('=').ok_or("attribute without value")? + j;
                    let name = text[j..eq].trim().to_string();
                    let mut q = eq + 1;
                    while q < b.len() && b[q].is_ascii_whitespace() {
                        q += 1;
                    }
                    let quote = *b.get(q).ok_or("unterminated attribute")?;
                    if quote != b'"' && quote != b'\'' {
                        return Err("attribute value without quotes".into());
                    }
                    let close = text[q + 1..].find(quote as char).ok_or("unterminated attribute")? + q + 1;
                    e.attrs.push((name, unescape(&text[q + 1..close])?));
                    j = close + 1;
                }
                if b[j] == b'/' {
                    if b.get(j + 1) != Some(&b'>') {
                        return Err("malformed empty element".into());
                    }
                    match stack.last_mut() {
                        Some(p) => p.children.push(Node::Element(e)),
                        None => root = Some(e),
                    }
                    i = j + 2;
                } else {
                    if stack.len() >= MAX_DEPTH {
                        return Err("elements nested too deep".into());
                    }
                    stack.push(e);
                    i = j + 1;
                }
            }
        } else {
            let end = text[i..].find('<').map(|k| k + i).unwrap_or(b.len());
            if let Some(p) = stack.last_mut() {
                p.children.push(Node::Text(unescape(&text[i..end])?));
            }
            i = end;
        }
    }
    if !stack.is_empty() {
        return Err("unclosed element".into());
    }
    root.ok_or_else(|| "no root element".into())
}

fn escape(s: &str, attr: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' if attr => out.push_str("&quot;"),
            '\n' if attr => out.push_str("&#xA;"),
            '\r' if attr => out.push_str("&#xD;"),
            '\t' if attr => out.push_str("&#x9;"),
            _ => out.push(c),
        }
    }
    out
}

/// Writes `e` and its children at `depth`, the root's namespace
/// declarations on lines of their own when `split_root` is set (the
/// workflow document's layout).
pub fn write(e: &Element, depth: usize, split_root: bool, out: &mut String) {
    let pad = "  ".repeat(depth);
    out.push_str(&pad);
    out.push('<');
    out.push_str(&e.name);
    for (n, v) in &e.attrs {
        out.push_str(if split_root && n.starts_with("xmlns") { "\n " } else { " " });
        out.push_str(n);
        out.push_str("=\"");
        out.push_str(&escape(v, true));
        out.push('"');
    }
    let elements = e.children.iter().any(|c| matches!(c, Node::Element(_)));
    let mixed = elements && e.children.iter().any(|c| matches!(c, Node::Text(t) if !t.trim().is_empty()));
    if mixed {
        // text and elements together stay on one line
        out.push('>');
        for c in &e.children {
            match c {
                Node::Text(t) => out.push_str(&escape(t, false)),
                Node::Element(c) => write_inline(c, out),
            }
        }
        out.push_str("</");
        out.push_str(&e.name);
        out.push('>');
    } else if e.children.is_empty() {
        out.push_str(" />");
    } else if !elements {
        out.push('>');
        out.push_str(&escape(&e.inner_text(), false));
        out.push_str("</");
        out.push_str(&e.name);
        out.push('>');
    } else {
        out.push('>');
        for c in &e.children {
            if let Node::Element(c) = c {
                out.push_str("\r\n");
                write(c, depth + 1, false, out);
            }
        }
        out.push_str("\r\n");
        out.push_str(&pad);
        out.push_str("</");
        out.push_str(&e.name);
        out.push('>');
    }
}

/// `e` on one line (inside mixed content).
fn write_inline(e: &Element, out: &mut String) {
    out.push('<');
    out.push_str(&e.name);
    for (n, v) in &e.attrs {
        out.push(' ');
        out.push_str(n);
        out.push_str("=\"");
        out.push_str(&escape(v, true));
        out.push('"');
    }
    if e.children.is_empty() {
        out.push_str(" />");
        return;
    }
    out.push('>');
    for c in &e.children {
        match c {
            Node::Text(t) => out.push_str(&escape(t, false)),
            Node::Element(c) => write_inline(c, out),
        }
    }
    out.push_str("</");
    out.push_str(&e.name);
    out.push('>');
}

/// A part written as the package editor writes its settings parts:
/// `<?xml version="1.0"?>` and the element tree.
pub fn document(e: &Element) -> String {
    let mut out = String::from("<?xml version=\"1.0\"?>\r\n");
    write(e, 0, false, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_malformed_documents_without_panicking() {
        for bad in ["<a b=", "<a b= ", "<a b=x>", "<a b=\u{e9}x>", "<a/\u{e9}", "<a/x>", "<a b=\"1\""] {
            assert!(parse(bad).is_err(), "{bad}");
        }
        let deep = format!("{}{}", "<a>".repeat(MAX_DEPTH + 1), "</a>".repeat(MAX_DEPTH + 1));
        assert!(parse(&deep).is_err());
        let fine = format!("{}{}", "<a>".repeat(100), "</a>".repeat(100));
        assert!(parse(&fine).is_ok());
    }

    #[test]
    fn parses_and_writes_back() {
        let text = "\u{feff}<?xml version=\"1.0\"?>\r\n<a x=\"1 &lt; 2\" y='q&#xA;r'>\r\n  <b>t &amp; u</b>\r\n  <c />\r\n</a>";
        let e = parse(text).unwrap();
        assert_eq!(e.get("x"), Some("1 < 2"));
        assert_eq!(e.get("y"), Some("q\nr"));
        assert_eq!(e.first("b").unwrap().inner_text(), "t & u");
        let mut out = String::new();
        write(&e, 0, false, &mut out);
        assert_eq!(parse(&out).unwrap().normalized(), e.normalized());
        assert_eq!(out, "<a x=\"1 &lt; 2\" y=\"q&#xA;r\">\r\n  <b>t &amp; u</b>\r\n  <c />\r\n</a>");
    }
}
