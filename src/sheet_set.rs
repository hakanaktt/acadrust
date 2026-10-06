
use crate::document::CadDocument;
use crate::fields::FieldContext;
use crate::objects::{Dictionary, ObjectType, XRecord, XRecordValue};

// ── cipher ───────────────────────────────────────────────────────────────────

const fn encode_byte(p: u8) -> u8 {
    let q = p.wrapping_add(1);
    let low = (q & 0x0F) ^ 0x0D;
    let h = q >> 4;
    let hc = if h & 1 == 1 { (h + 10) & 0x0F } else { (h + 8) & 0x0F };
    (hc << 4) | low
}

const ENCODE: [u8; 256] = {
    let mut t = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        t[i] = encode_byte(i as u8);
        i += 1;
    }
    t
};

const DECODE: [u8; 256] = {
    let mut t = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        t[ENCODE[i] as usize] = i as u8;
        i += 1;
    }
    t
};

pub fn encode(plain: &[u8]) -> Vec<u8> {
    plain.iter().map(|b| ENCODE[*b as usize]).collect()
}

pub fn decode(data: &[u8]) -> Vec<u8> {
    data.iter().map(|b| DECODE[*b as usize]).collect()
}

// ── XML tree ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub text: String,
    pub children: Vec<Element>,
}

impl Element {
    pub fn new(name: &str) -> Self {
        Self { name: name.into(), ..Self::default() }
    }

    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn id(&self) -> &str {
        self.attr("ID").unwrap_or("")
    }

    pub fn propname(&self) -> Option<&str> {
        self.attr("propname")
    }

    pub fn named(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|c| c.propname() == Some(name))
    }

    pub fn named_mut(&mut self, name: &str) -> Option<&mut Element> {
        self.children.iter_mut().find(|c| c.propname() == Some(name))
    }

    pub fn prop(&self, name: &str) -> Option<&str> {
        self.named(name).filter(|c| c.name == "AcSmProp").map(|c| c.text.as_str())
    }

    pub fn set_prop(&mut self, name: &str, value: &str) {
        self.set_prop_vt(name, 8, value);
    }

    pub fn set_prop_vt(&mut self, name: &str, vt: i32, value: &str) {
        if let Some(c) = self.named_mut(name).filter(|c| c.name == "AcSmProp") {
            c.text = value.into();
            return;
        }
        self.put_named(prop(name, vt, value));
    }

    pub fn put_named(&mut self, child: Element) {
        let name = child.propname().unwrap_or("").to_string();
        if let Some(i) = self.children.iter().position(|c| c.propname() == Some(&name)) {
            self.children[i] = child;
            return;
        }
        let key = name.to_ascii_lowercase();
        let at = self
            .children
            .iter()
            .position(|c| match c.propname() {
                Some(n) => n.to_ascii_lowercase() > key,
                None => true,
            })
            .unwrap_or(self.children.len());
        self.children.insert(at, child);
    }

    pub fn remove_named(&mut self, name: &str) {
        self.children.retain(|c| c.propname() != Some(name));
    }

    fn find(&self, id: &str) -> Option<&Element> {
        if self.id() == id {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.find(id))
    }

    fn find_mut(&mut self, id: &str) -> Option<&mut Element> {
        if self.id() == id {
            return Some(self);
        }
        self.children.iter_mut().find_map(|c| c.find_mut(id))
    }

    fn parent_of(&self, id: &str) -> Option<&Element> {
        if self.children.iter().any(|c| c.id() == id) {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.parent_of(id))
    }

    fn write(&self, out: &mut String) {
        out.push('<');
        out.push_str(&self.name);
        for (k, v) in &self.attrs {
            out.push(' ');
            out.push_str(k);
            out.push_str("=\"");
            escape_into(v, true, out);
            out.push('"');
        }
        if self.text.is_empty() && self.children.is_empty() {
            out.push_str("/>");
            return;
        }
        out.push('>');
        escape_into(&self.text, false, out);
        for c in &self.children {
            c.write(out);
        }
        out.push_str("</");
        out.push_str(&self.name);
        out.push('>');
    }
}

fn escape_into(s: &str, attr: bool, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attr => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
}

fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find('&') {
        out.push_str(&rest[..p]);
        let after = &rest[p..];
        let Some(end) = after.find(';') else {
            out.push_str(after);
            return out;
        };
        let entity = &after[1..end];
        let ch = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            e if e.starts_with("#x") || e.starts_with("#X") => {
                u32::from_str_radix(&e[2..], 16).ok().and_then(char::from_u32)
            }
            e if e.starts_with('#') => e[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => out.push(c),
            None => out.push_str(&after[..=end]),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

struct Parser<'a> {
    s: &'a str,
    i: usize,
}

impl Parser<'_> {
    fn skip_ws(&mut self) {
        // Step by the character's width: a multi-byte space (U+00A0) would
        // otherwise leave `i` inside it and the next slice would panic.
        while let Some(c) = self.s[self.i..].chars().next().filter(|c| c.is_whitespace()) {
            self.i += c.len_utf8();
        }
    }

    fn skip_misc(&mut self) {
        loop {
            self.skip_ws();
            let rest = &self.s[self.i..];
            let close = if rest.starts_with("<?") {
                "?>"
            } else if rest.starts_with("<!--") {
                "-->"
            } else if rest.starts_with("<!") {
                ">"
            } else {
                return;
            };
            match rest.find(close) {
                Some(p) => self.i += p + close.len(),
                None => {
                    self.i = self.s.len();
                    return;
                }
            }
        }
    }

    fn element(&mut self) -> Result<Element, String> {
        self.skip_misc();
        if !self.s[self.i..].starts_with('<') {
            return Err(format!("element expected at byte {}", self.i));
        }
        self.i += 1;
        let name_end = self.s[self.i..]
            .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
            .ok_or("unterminated tag")?;
        let mut el = Element::new(&self.s[self.i..self.i + name_end]);
        self.i += name_end;
        loop {
            self.skip_ws();
            let rest = &self.s[self.i..];
            if rest.starts_with("/>") {
                self.i += 2;
                return Ok(el);
            }
            if rest.starts_with('>') {
                self.i += 1;
                break;
            }
            let eq = rest.find('=').ok_or("attribute without value")?;
            let key = rest[..eq].trim().to_string();
            let after = rest[eq + 1..].trim_start();
            let quote = after.chars().next().ok_or("attribute value expected")?;
            if quote != '"' && quote != '\'' {
                return Err("quoted attribute value expected".into());
            }
            let value_end = after[1..].find(quote).ok_or("unterminated attribute")?;
            el.attrs.push((key, unescape(&after[1..1 + value_end])));
            self.i = self.s.len() - after.len() + value_end + 2;
        }
        loop {
            let rest = &self.s[self.i..];
            let lt = rest.find('<').ok_or("unterminated element")?;
            el.text.push_str(&unescape(&rest[..lt]));
            self.i += lt;
            let rest = &self.s[self.i..];
            if rest.starts_with("</") {
                let gt = rest.find('>').ok_or("unterminated end tag")?;
                self.i += gt + 1;
                // Indentation between child elements is not a value.
                if !el.children.is_empty() && el.text.trim().is_empty() {
                    el.text.clear();
                }
                return Ok(el);
            }
            if rest.starts_with("<!--") {
                self.skip_misc();
                continue;
            }
            el.children.push(self.element()?);
        }
    }
}

pub fn parse_xml(text: &str) -> Result<Element, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    Parser { s: text, i: 0 }.element()
}

pub fn write_xml(root: &Element) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
    root.write(&mut out);
    out.push_str("\r\n");
    out
}

// ── component classes ────────────────────────────────────────────────────────

pub const CLSID_DATABASE: &str = "g2162C6B6-0CE4-40E8-912B-46F59DFDF826";
pub const CLSID_SHEET_SET: &str = "gB20534F2-0978-418C-8D14-2E6928A077ED";
pub const CLSID_SUBSET: &str = "g076D548F-B0F5-4FE1-B35D-7F7B73B8D322";
pub const CLSID_SHEET: &str = "g16A07941-BC15-4D48-A880-9D5A211D5065";
pub const CLSID_CALLOUT_BLOCKS: &str = "g203EAB46-483B-4E6B-A10B-15E9A4B210FF";
pub const CLSID_CUSTOM_PROPERTY_BAG: &str = "g4D103908-8C86-4D95-BBF4-68B9A7B00731";
pub const CLSID_CUSTOM_PROPERTY_VALUE: &str = "g8D22A2A4-1777-4D78-84CC-69EF741FE954";
pub const CLSID_PROJECT_POINT_LOCATIONS: &str = "gE40EA246-BAB4-4907-81A5-511EA30C16FD";
pub const CLSID_PUBLISH_OPTIONS: &str = "gF57F96E7-0F16-4DC9-8F09-52F7BB389AB6";
pub const CLSID_RESOURCES: &str = "g3F0FAF10-09DE-4EBA-AED1-C4E4D6FECF5D";
pub const CLSID_SHEET_SEL_SETS: &str = "g444780B8-6527-43A8-8DC4-FAB41B7E48BB";
pub const CLSID_VIEW_CATEGORIES: &str = "g021730DF-5BEA-48E9-BC7A-35087A674FD0";
pub const CLSID_LAYOUT_REFERENCE: &str = "g94910E94-4FCA-427C-B6ED-2EC9E1C900C7";
pub const CLSID_SHEET_VIEWS: &str = "gF40F931B-64BC-4B90-9FC8-A11A77D6815B";
pub const CLSID_FILE_REFERENCE: &str = "g6BF87AE7-1BEC-4BDB-98BB-5B91F7772793";
pub const CLSID_SIMPLE_FILE_REFERENCE: &str = "gD15A03C2-C39B-428A-9BBA-C031347C496F";
pub const CLSID_SHEET_VIEW: &str = "gB6E09611-4659-4F0D-981D-D62B11FD8426";
pub const CLSID_VIEW_REFERENCE: &str = "g9BEA33B1-05AD-419F-B680-BC7FF6A4F41D";
pub const CLSID_VIEW_CATEGORY: &str = "g4AEA81ED-C24F-477B-A534-EA69220A276A";
pub const CLSID_CALLOUT_BLOCK_REFERENCES: &str = "g67C52FE4-0A6B-4C82-A4CC-5E68537747B0";
pub const CLSID_OBJECT_REFERENCE: &str = "g00DEB7FB-A073-4ECD-BCE0-121B45C6864D";
pub const CLSID_BLOCK_RECORD_REFERENCE: &str = "g11782523-474B-4C83-9646-57C052847FBB";

pub const CUSTOM_SHEET_SET_PROP: i32 = 1;
pub const CUSTOM_SHEET_PROP: i32 = 2;

pub fn new_id() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let word = |salt: u64| {
        let mut h = RandomState::new().build_hasher();
        h.write_u64(n);
        h.write_u64(salt);
        h.write_u128(web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0));
        h.finish()
    };
    let (a, b) = (word(0x5eed), word(0xfeed));
    // Version 4, RFC 4122 variant.
    let b = (b & 0x3FFF_FFFF_FFFF_FFFF) | 0x8000_0000_0000_0000;
    let a = (a & 0xFFFF_FFFF_FFFF_0FFF) | 0x0000_0000_0000_4000;
    format!(
        "g{:08X}-{:04X}-{:04X}-{:04X}-{:012X}",
        a >> 32,
        (a >> 16) & 0xFFFF,
        a & 0xFFFF,
        b >> 48,
        b & 0xFFFF_FFFF_FFFF
    )
}

pub fn prop(name: &str, vt: i32, value: &str) -> Element {
    Element {
        name: "AcSmProp".into(),
        attrs: vec![("propname".into(), name.into()), ("vt".into(), vt.to_string())],
        text: value.into(),
        children: Vec::new(),
    }
}

pub fn object(tag: &str, clsid: &str, propname: Option<&str>) -> Element {
    let mut el = Element::new(tag);
    el.attrs.push(("clsid".into(), clsid.into()));
    el.attrs.push(("ID".into(), new_id()));
    if let Some(p) = propname {
        el.attrs.push(("propname".into(), p.into()));
        el.attrs.push(("vt".into(), "13".into()));
    }
    el
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentKind {
    SheetSet,
    Subset,
    Sheet,
}

impl ComponentKind {
    pub fn of(el: &Element) -> Option<Self> {
        match el.name.as_str() {
            "AcSmSheetSet" => Some(Self::SheetSet),
            "AcSmSubset" => Some(Self::Subset),
            "AcSmSheet" => Some(Self::Sheet),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LayoutReference {
    pub file_name: String,
    pub name: String,
    pub handle: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SheetSetDatabase {
    pub root: Element,
    pub path: Option<String>,
}

impl SheetSetDatabase {
    pub fn new(name: &str, description: &str) -> Self {
        let mut root = object("AcSmDatabase", CLSID_DATABASE, None);
        root.children.push(prop("DbFingerPrint", 8, &new_id()));
        root.children.push(prop("DbVersion", 8, "1.1"));
        root.children.push(prop("FileRevision", 3, "0"));
        let mut set = object("AcSmSheetSet", CLSID_SHEET_SET, Some("SheetSet"));
        for (tag, clsid, name) in [
            ("AcSmCalloutBlocks", CLSID_CALLOUT_BLOCKS, "CalloutBlocks"),
            ("AcSmCustomPropertyBag", CLSID_CUSTOM_PROPERTY_BAG, "CustomPropertyBag"),
            ("AcSmProjectPointLocations", CLSID_PROJECT_POINT_LOCATIONS, "ProjectPointLocations"),
            ("AcSmResources", CLSID_RESOURCES, "Resources"),
            ("AcSmSheetSelSets", CLSID_SHEET_SEL_SETS, "SheetSelSets"),
            ("AcSmViewCategories", CLSID_VIEW_CATEGORIES, "ViewCategories"),
        ] {
            set.put_named(object(tag, clsid, Some(name)));
        }
        let mut publish = object("AcSmPublishOptions", CLSID_PUBLISH_OPTIONS, Some("PublishOptions"));
        publish.set_prop_vt("DwfType", 2, "-1");
        publish.set_prop_vt("PromptForName", 2, "-1");
        set.put_named(publish);
        set.set_prop("Desc", description);
        set.set_prop("Name", name);
        root.children.push(set);
        Self { root, path: None }
    }

    pub fn from_xml(text: &str) -> Result<Self, String> {
        let root = parse_xml(text)?;
        if root.name != "AcSmDatabase" {
            return Err(format!("not a sheet set database ({})", root.name));
        }
        Ok(Self { root, path: None })
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        let plain = decode(data);
        let text = String::from_utf8(plain).map_err(|e| e.to_string())?;
        Self::from_xml(&text)
    }

    pub fn to_xml(&self) -> String {
        write_xml(&self.root)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        encode(self.to_xml().as_bytes())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn read(path: &str) -> Result<Self, String> {
        let data = std::fs::read(path).map_err(|e| e.to_string())?;
        let mut db = Self::from_bytes(&data)?;
        db.path = Some(path.to_string());
        Ok(db)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn write(&mut self, path: &str) -> Result<(), String> {
        // The publish options always carry the (empty) default output folder.
        if let Some(p) = self.sheet_set_mut().named_mut("PublishOptions") {
            if p.named("DefaultOutputdir").is_none() {
                p.put_named(object("AcSmSimpleFileReferece", CLSID_SIMPLE_FILE_REFERENCE, Some("DefaultOutputdir")));
            }
        }
        // Every save gets a new fingerprint and counts the revision up.
        let revision = self.file_revision() + 1;
        self.root.set_prop("DbFingerPrint", &new_id());
        self.root.set_prop_vt("FileRevision", 3, &revision.to_string());
        std::fs::write(path, self.to_bytes()).map_err(|e| e.to_string())?;
        self.path = Some(path.to_string());
        Ok(())
    }

    pub fn file_revision(&self) -> i32 {
        self.root.prop("FileRevision").and_then(|v| v.trim().parse().ok()).unwrap_or(0)
    }

    pub fn sheet_set(&self) -> &Element {
        self.root
            .children
            .iter()
            .find(|c| c.name == "AcSmSheetSet")
            .unwrap_or(&self.root)
    }

    pub fn sheet_set_mut(&mut self) -> &mut Element {
        let i = self.root.children.iter().position(|c| c.name == "AcSmSheetSet");
        match i {
            Some(i) => &mut self.root.children[i],
            None => &mut self.root,
        }
    }

    pub fn name(&self) -> &str {
        self.sheet_set().prop("Name").unwrap_or("")
    }

    pub fn find(&self, id: &str) -> Option<&Element> {
        self.root.find(id)
    }

    pub fn find_mut(&mut self, id: &str) -> Option<&mut Element> {
        self.root.find_mut(id)
    }

    pub fn parent_of(&self, id: &str) -> Option<&Element> {
        self.root.parent_of(id)
    }

    fn folder(&self) -> Option<std::path::PathBuf> {
        let path = std::path::Path::new(self.path.as_deref()?);
        Some(path.parent()?.to_path_buf())
    }

    pub fn add_subset(&mut self, parent: &str, name: &str, description: &str) -> Option<String> {
        let prompt = self.find(parent)?.prop("PromptForDwt").is_some_and(|v| !matches!(v.trim(), "" | "0"));
        let inherited: Vec<Element> = {
            let p = self.find(parent)?;
            ["DefDwtLayout", "NewSheetLocation"]
                .iter()
                .filter_map(|n| p.named(n).cloned())
                .map(|mut e| {
                    renew_ids(&mut e);
                    e
                })
                .collect()
        };
        let mut sub = object("AcSmSubset", CLSID_SUBSET, None);
        for e in inherited {
            sub.put_named(e);
        }
        sub.set_prop("Desc", description);
        sub.set_prop("Name", name);
        if prompt {
            sub.set_prop_vt("PromptForDwt", 2, "-1");
        }
        let id = sub.id().to_string();
        self.find_mut(parent)?.children.push(sub);
        Some(id)
    }

    pub fn add_sheet(&mut self, parent: &str, number: &str, title: &str, description: &str) -> Option<String> {
        // Empty fields are left out; the property bag comes with the first
        // sheet property.
        let mut sheet = object("AcSmSheet", CLSID_SHEET, None);
        if !description.is_empty() {
            sheet.set_prop("Desc", description);
        }
        if !number.is_empty() {
            sheet.set_prop("Number", number);
        }
        sheet.put_named(object("AcSmSheetViews", CLSID_SHEET_VIEWS, Some("SheetViews")));
        sheet.set_prop("Title", title);
        // New sheets get the set's sheet properties with their defaults.
        for (name, value, flags) in custom_properties(self.sheet_set()) {
            if flags & CUSTOM_SHEET_PROP != 0 {
                set_custom_property(&mut sheet, &name, &value, flags);
            }
        }
        let id = sheet.id().to_string();
        let p = self.find_mut(parent)?;
        p.children.push(sheet);
        Some(id)
    }

    pub fn remove(&mut self, id: &str) -> bool {
        fn go(el: &mut Element, id: &str) -> bool {
            if let Some(i) = el.children.iter().position(|c| c.id() == id) {
                el.children.remove(i);
                return true;
            }
            el.children.iter_mut().any(|c| go(c, id))
        }
        go(&mut self.root, id)
    }

    pub fn set_file_reference(&mut self, component: &str, propname: &str, file: &str) -> bool {
        let folder = self.folder();
        let Some(el) = self.find_mut(component) else {
            return false;
        };
        if file.is_empty() {
            el.remove_named(propname);
            return true;
        }
        let mut r = object("AcSmFileReference", CLSID_FILE_REFERENCE, Some(propname));
        set_file_props(&mut r, file, folder.as_deref());
        el.put_named(r);
        true
    }

    pub fn file_reference(&self, component: &str, propname: &str) -> Option<String> {
        let r = self.find(component)?.named(propname)?;
        Some(self.resolve_file(r))
    }

    pub fn set_layout_reference(&mut self, component: &str, propname: &str, reference: &LayoutReference) -> bool {
        let folder = self.folder();
        let Some(el) = self.find_mut(component) else {
            return false;
        };
        let mut r = match el.named(propname) {
            Some(old) if old.name == "AcSmAcDbLayoutReference" => {
                let mut r = old.clone();
                r.children.clear();
                r
            }
            _ => object("AcSmAcDbLayoutReference", CLSID_LAYOUT_REFERENCE, Some(propname)),
        };
        if !reference.handle.is_empty() {
            r.set_prop("AcDbHandle", &reference.handle);
        }
        set_file_props(&mut r, &reference.file_name, folder.as_deref());
        r.set_prop("Name", &reference.name);
        el.put_named(r);
        true
    }

    pub fn layout_reference(&self, component: &str, propname: &str) -> Option<LayoutReference> {
        let r = self.find(component)?.named(propname)?;
        Some(LayoutReference {
            file_name: self.resolve_file(r),
            name: r.prop("Name").unwrap_or("").to_string(),
            handle: r.prop("AcDbHandle").unwrap_or("").to_string(),
        })
    }

    pub fn resolve_file(&self, reference: &Element) -> String {
        let absolute = reference.prop("FileName").unwrap_or("").to_string();
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let (Some(rel), Some(folder)) = (reference.prop("Relative_FileName"), self.folder()) {
                let joined = folder.join(rel.replace('\\', std::path::MAIN_SEPARATOR_STR));
                if joined.exists() || absolute.is_empty() {
                    return normalize_path(&joined);
                }
            }
        }
        absolute
    }

    pub fn sheets(&self) -> Vec<&Element> {
        fn go<'a>(el: &'a Element, out: &mut Vec<&'a Element>) {
            for c in &el.children {
                match ComponentKind::of(c) {
                    Some(ComponentKind::Sheet) => out.push(c),
                    Some(ComponentKind::Subset) => go(c, out),
                    _ => {}
                }
            }
        }
        let mut out = Vec::new();
        go(self.sheet_set(), &mut out);
        out
    }

    pub fn sheet_for(&self, drawing: &str, layout: Option<&str>) -> Option<&Element> {
        let want = path_key(drawing);
        let of_drawing: Vec<&Element> = self
            .sheets()
            .into_iter()
            .filter(|s| {
                s.named("Layout").is_some_and(|r| {
                    path_key(&self.resolve_file(r)) == want
                        || r.prop("FileName").is_some_and(|f| path_key(f) == want)
                })
            })
            .collect();
        layout
            .and_then(|name| {
                of_drawing.iter().copied().find(|s| {
                    s.named("Layout")
                        .and_then(|r| r.prop("Name"))
                        .is_some_and(|n| n.eq_ignore_ascii_case(name))
                })
            })
            .or_else(|| of_drawing.first().copied())
    }

    pub fn sheet_value(&self, sheet: &Element, component: &str, property: &str) -> Option<String> {
        let set = self.sheet_set();
        let custom = |el: &Element, name: &str| {
            custom_properties(el).into_iter().find(|(n, _, _)| n == name).map(|(_, v, _)| v)
        };
        match component {
            "Sheet" => Some(match property {
                "Number" | "Title" | "RevisionNumber" | "RevisionDate" | "IssuePurpose" | "Category" => {
                    sheet.prop(property).unwrap_or("").to_string()
                }
                "Description" => sheet.prop("Desc").unwrap_or("").to_string(),
                "NumberAndTitle" => number_and_title(sheet),
                // A sheet property the sheet lacks shows the set's default.
                other => custom(sheet, other).or_else(|| {
                    custom_properties(set)
                        .into_iter()
                        .find(|(n, _, f)| n == other && f & CUSTOM_SHEET_PROP != 0)
                        .map(|(_, v, _)| v)
                })?,
            }),
            "SheetSet" => self.set_value(property),
            "Subset" => {
                let parent = self.parent_of(sheet.id())?;
                if ComponentKind::of(parent) != Some(ComponentKind::Subset) {
                    return None;
                }
                Some(match property {
                    "Description" => parent.prop("Desc").unwrap_or("").to_string(),
                    other => parent.prop(other).unwrap_or("").to_string(),
                })
            }
            _ => None,
        }
    }
}

impl SheetSetDatabase {
    pub fn set_value(&self, property: &str) -> Option<String> {
        let set = self.sheet_set();
        Some(match property {
            "Name" | "ProjectName" | "ProjectNumber" | "ProjectPhase" | "ProjectMilestone" => {
                set.prop(property).unwrap_or("").to_string()
            }
            "Description" => set.prop("Desc").unwrap_or("").to_string(),
            other => custom_properties(set).into_iter().find(|(n, _, _)| n == other).map(|(_, v, _)| v)?,
        })
    }

    pub fn navigation_value(&self, set_id: &str, component: Option<&str>, property: &str) -> Option<String> {
        if !self.sheet_set().id().eq_ignore_ascii_case(set_id) {
            return None;
        }
        let Some(id) = component else {
            return self.set_value(property);
        };
        let el = self.find(id)?;
        match ComponentKind::of(el) {
            Some(ComponentKind::Sheet) => self.sheet_value(el, "Sheet", property),
            Some(ComponentKind::SheetSet) => self.set_value(property),
            // A sheet view: its number and title (the viewport scale lives in
            // the sheet drawing, not in the set).
            _ if el.name == "AcSmSheetView" => match property {
                "Number" | "Title" => Some(el.prop(property).unwrap_or("").to_string()),
                "NumberAndTitle" => Some(number_and_title(el)),
                _ => None,
            },
            _ => match property {
                "Name" => Some(el.prop("Name").unwrap_or("").to_string()),
                "Description" => Some(el.prop("Desc").unwrap_or("").to_string()),
                _ => None,
            },
        }
    }
}

impl SheetSetDatabase {
    pub fn view_categories(&self) -> Vec<&Element> {
        self.sheet_set()
            .named("ViewCategories")
            .map(|v| v.children.iter().filter(|c| c.name == "AcSmViewCategory" && c.prop("Name").is_some()).collect())
            .unwrap_or_default()
    }

    pub fn callout_blocks(&self) -> Vec<&Element> {
        self.sheet_set().named("CalloutBlocks").map(|v| v.children.iter().collect()).unwrap_or_default()
    }

    pub fn add_callout_block(&mut self, file: &str, name: &str, handle: &str) -> Option<String> {
        let folder = self.folder();
        let mut r = object("AcSmAcDbBlockRecordReference", CLSID_BLOCK_RECORD_REFERENCE, None);
        r.set_prop("AcDbHandle", handle);
        set_file_props(&mut r, file, folder.as_deref());
        r.set_prop("Name", name);
        let id = r.id().to_string();
        let set = self.sheet_set_mut();
        if set.named("CalloutBlocks").is_none() {
            set.put_named(object("AcSmCalloutBlocks", CLSID_CALLOUT_BLOCKS, Some("CalloutBlocks")));
        }
        set.named_mut("CalloutBlocks")?.children.push(r);
        Some(id)
    }

    pub fn category_blocks(&self, id: &str) -> Vec<String> {
        self.find(id)
            .and_then(|c| c.named("CalloutBlocks"))
            .map(|r| r.children.iter().filter_map(|o| o.prop("ReferencedObject").map(str::to_string)).collect())
            .unwrap_or_default()
    }

    pub fn set_view_category(&mut self, id: Option<&str>, name: &str, blocks: &[String]) -> Option<String> {
        let mut refs = object("AcSmCalloutBlockReferences", CLSID_CALLOUT_BLOCK_REFERENCES, Some("CalloutBlocks"));
        for b in blocks {
            let mut o = object("AcSmObjectReference", CLSID_OBJECT_REFERENCE, None);
            o.set_prop_vt("ReferencedObject", -1, b);
            refs.children.push(o);
        }
        if let Some(id) = id {
            let c = self.find_mut(id)?;
            c.put_named(refs);
            c.set_prop("Name", name);
            return Some(id.to_string());
        }
        let mut c = object("AcSmViewCategory", CLSID_VIEW_CATEGORY, None);
        c.put_named(refs);
        c.set_prop("Name", name);
        let new = c.id().to_string();
        let set = self.sheet_set_mut();
        if set.named("ViewCategories").is_none() {
            set.put_named(object("AcSmViewCategories", CLSID_VIEW_CATEGORIES, Some("ViewCategories")));
        }
        set.named_mut("ViewCategories")?.children.push(c);
        Some(new)
    }

    pub fn sheet_views<'a>(&'a self, sheet: &'a Element) -> Vec<&'a Element> {
        sheet.named("SheetViews").map(|v| v.children.iter().filter(|c| c.name == "AcSmSheetView").collect()).unwrap_or_default()
    }

    pub fn view_category_of(view: &Element) -> Option<&str> {
        view.named("Category").and_then(|c| c.prop("ReferencedObject"))
    }

    pub fn add_sheet_view(&mut self, sheet: &str, category: Option<&str>, view: &LayoutReference, title: &str) -> Option<String> {
        let folder = self.folder();
        let mut v = object("AcSmSheetView", CLSID_SHEET_VIEW, None);
        if let Some(c) = category {
            let mut r = object("AcSmObjectReference", CLSID_OBJECT_REFERENCE, Some("Category"));
            r.set_prop_vt("ReferencedObject", -1, c);
            v.put_named(r);
        }
        let mut r = object("AcSmAcDbViewReference", CLSID_VIEW_REFERENCE, Some("NamedView"));
        r.set_prop("AcDbHandle", &view.handle);
        set_file_props(&mut r, &view.file_name, folder.as_deref());
        r.set_prop("Name", &view.name);
        v.put_named(r);
        v.set_prop("Title", title);
        let id = v.id().to_string();
        let sh = self.find_mut(sheet)?;
        if sh.named("SheetViews").is_none() {
            sh.put_named(object("AcSmSheetViews", CLSID_SHEET_VIEWS, Some("SheetViews")));
        }
        sh.named_mut("SheetViews")?.children.push(v);
        Some(id)
    }

    pub fn add_resource(&mut self, folder: &str) -> Option<String> {
        let base = self.folder();
        let mut r = object("AcSmFileReference", CLSID_FILE_REFERENCE, None);
        set_file_props(&mut r, folder, base.as_deref());
        let id = r.id().to_string();
        let set = self.sheet_set_mut();
        if set.named("Resources").is_none() {
            set.put_named(object("AcSmResources", CLSID_RESOURCES, Some("Resources")));
        }
        set.named_mut("Resources")?.children.push(r);
        Some(id)
    }
}

pub fn number_and_title(sheet: &Element) -> String {
    let number = sheet.prop("Number").unwrap_or("");
    let title = sheet.prop("Title").unwrap_or("");
    if number.is_empty() {
        title.to_string()
    } else {
        format!("{number} - {title}")
    }
}

fn renew_ids(el: &mut Element) {
    if let Some(a) = el.attrs.iter_mut().find(|(k, _)| k == "ID") {
        a.1 = new_id();
    }
    for c in &mut el.children {
        renew_ids(c);
    }
}

pub fn custom_properties(el: &Element) -> Vec<(String, String, i32)> {
    el.named("CustomPropertyBag")
        .map(|bag| {
            bag.children
                .iter()
                .filter(|c| c.name == "AcSmCustomPropertyValue")
                .map(|c| {
                    (
                        c.propname().unwrap_or("").to_string(),
                        c.prop("Value").unwrap_or("").to_string(),
                        c.prop("Flags").and_then(|f| f.trim().parse().ok()).unwrap_or(0),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn set_custom_property(el: &mut Element, name: &str, value: &str, flags: i32) {
    if el.named("CustomPropertyBag").is_none() {
        el.put_named(object("AcSmCustomPropertyBag", CLSID_CUSTOM_PROPERTY_BAG, Some("CustomPropertyBag")));
    }
    let bag = el.named_mut("CustomPropertyBag").expect("bag");
    if let Some(v) = bag.named_mut(name).filter(|c| c.name == "AcSmCustomPropertyValue") {
        v.set_prop_vt("Flags", 3, &flags.to_string());
        v.set_prop("Value", value);
        return;
    }
    let mut v = object("AcSmCustomPropertyValue", CLSID_CUSTOM_PROPERTY_VALUE, Some(name));
    v.set_prop_vt("Flags", 3, &flags.to_string());
    v.set_prop("Value", value);
    // The bag keeps its values in insertion order.
    bag.children.push(v);
}

pub fn remove_custom_property(el: &mut Element, name: &str) {
    if let Some(bag) = el.named_mut("CustomPropertyBag") {
        bag.children.retain(|c| c.propname() != Some(name));
    }
}

fn set_file_props(r: &mut Element, file: &str, folder: Option<&std::path::Path>) {
    for p in ["Environ_FileName", "FileName", "Relative_FileName", "SpecialFolder_FileName"] {
        r.remove_named(p);
    }
    let file = &native_path(file);
    r.set_prop("FileName", file);
    if let Some(rel) = folder.and_then(|f| relative_path(f, std::path::Path::new(file))) {
        r.set_prop("Relative_FileName", &rel);
    }
}

pub fn relative_path(folder: &std::path::Path, file: &std::path::Path) -> Option<String> {
    use std::path::Component;
    let parts = |p: &std::path::Path| -> Vec<String> {
        p.components()
            .filter(|c| !matches!(c, Component::CurDir))
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect()
    };
    let (a, b) = (parts(folder), parts(file));
    if a.is_empty() || b.is_empty() || !a[0].eq_ignore_ascii_case(&b[0]) {
        return None;
    }
    let common = a.iter().zip(&b).take_while(|(x, y)| x.eq_ignore_ascii_case(y)).count();
    let mut out: Vec<String> = Vec::new();
    if common == a.len() {
        out.push(".".into());
    }
    out.extend(std::iter::repeat("..".to_string()).take(a.len() - common));
    out.extend(b[common..].iter().cloned());
    Some(out.join("\\"))
}

fn normalize_path(p: &std::path::Path) -> String {
    use std::path::Component;
    let mut out = std::path::PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out.to_string_lossy().to_string()
}

pub fn native_path(p: &str) -> String {
    if cfg!(windows) {
        p.replace('/', "\\")
    } else {
        p.to_string()
    }
}

pub fn path_key(p: &str) -> String {
    p.replace('/', "\\").trim_end_matches('\\').to_lowercase()
}

// ── the drawing's sheet link (AcSheetSetData) ────────────────────────────────

pub const SHEET_SET_DATA: &str = "AcSheetSetData";

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SheetSetData {
    pub layout_handle: String,
    pub layout_name: String,
    pub sheet_dwg_name: String,
    pub sheet_set_file_name: String,
    pub sheet_set_version: i32,
    pub update_count: i32,
    pub update_time: String,
}

impl CadDocument {
    pub fn sheet_set_data(&self) -> Option<SheetSetData> {
        let ObjectType::Dictionary(nod) = self.objects.get(&self.header.named_objects_dict_handle)? else {
            return None;
        };
        let ObjectType::Dictionary(dict) = self.objects.get(&nod.get(SHEET_SET_DATA)?)? else {
            return None;
        };
        let value = |key: &str| -> Option<&XRecordValue> {
            let ObjectType::XRecord(x) = self.objects.get(&dict.get(key)?)? else {
                return None;
            };
            x.entries.iter().find(|e| e.code != 280).map(|e| &e.value)
        };
        let text = |key: &str| value(key).and_then(|v| v.as_string()).unwrap_or("").to_string();
        let int = |key: &str| value(key).and_then(|v| v.as_i32()).unwrap_or(0);
        Some(SheetSetData {
            layout_handle: text("LayoutHandle"),
            layout_name: text("LayoutName"),
            sheet_dwg_name: text("SheetDwgName"),
            sheet_set_file_name: text("ShSetFileName"),
            sheet_set_version: int("ShSetVersion"),
            update_count: int("UpdateCount"),
            update_time: text("UpdateTime"),
        })
    }

    pub fn set_sheet_set_data(&mut self, data: &SheetSetData) {
        let nod = self.header.named_objects_dict_handle;
        let existing = match self.objects.get(&nod) {
            Some(ObjectType::Dictionary(d)) => d.get(SHEET_SET_DATA),
            _ => None,
        };
        let dict_handle = match existing {
            Some(h) if matches!(self.objects.get(&h), Some(ObjectType::Dictionary(_))) => h,
            _ => {
                let h = self.allocate_handle();
                let mut d = Dictionary::new();
                d.handle = h;
                d.owner = nod;
                d.hard_owner = true;
                d.duplicate_cloning = 1;
                d.reactors = vec![nod];
                self.objects.insert(h, ObjectType::Dictionary(d));
                if let Some(ObjectType::Dictionary(n)) = self.objects.get_mut(&nod) {
                    n.add_entry(SHEET_SET_DATA, h);
                }
                h
            }
        };
        let records: [(&str, XRecordValue, i32); 7] = [
            ("LayoutHandle", XRecordValue::String(data.layout_handle.clone()), 1),
            ("LayoutName", XRecordValue::String(data.layout_name.clone()), 1),
            ("SheetDwgName", XRecordValue::String(data.sheet_dwg_name.clone()), 1),
            ("ShSetFileName", XRecordValue::String(data.sheet_set_file_name.clone()), 1),
            ("ShSetVersion", XRecordValue::Int32(data.sheet_set_version), 90),
            ("UpdateCount", XRecordValue::Int32(data.update_count), 90),
            ("UpdateTime", XRecordValue::String(data.update_time.clone()), 1),
        ];
        for (key, value, code) in records {
            let old = match self.objects.get(&dict_handle) {
                Some(ObjectType::Dictionary(d)) => d.get(key),
                _ => None,
            };
            let handle = old
                .filter(|h| matches!(self.objects.get(h), Some(ObjectType::XRecord(_))))
                .unwrap_or_else(|| self.allocate_handle());
            let mut x = XRecord::new();
            x.handle = handle;
            x.owner = dict_handle;
            x.reactors = vec![dict_handle];
            x.cloning_flags = crate::objects::DictionaryCloningFlags::KeepExisting;
            x.entries.push(crate::objects::XRecordEntry::new(code, value));
            self.objects.insert(handle, ObjectType::XRecord(x));
            if old != Some(handle) {
                if let Some(ObjectType::Dictionary(d)) = self.objects.get_mut(&dict_handle) {
                    d.entries.retain(|(k, _)| k != key);
                    d.add_entry(key, handle);
                }
            }
        }
    }
}

// ── \AcSm fields ─────────────────────────────────────────────────────────────

pub fn placeholder_type_name(component: &str, property: &str) -> String {
    match (component, property) {
        ("Sheet", "NumberAndTitle" | "Title" | "Number" | "Description") => format!("Sheet{property}"),
        ("Sheet", "RevisionNumber" | "RevisionDate" | "IssuePurpose" | "Category") => property.to_string(),
        ("Sheet", _) => "Custom".into(),
        ("View", "ViewportScale") => "ViewportScale".into(),
        ("View", p) => format!("View{p}"),
        _ => property.to_string(),
    }
}

pub fn parse_code(code: &str) -> Option<(String, String, String)> {
    let body = code.trim().trim_start_matches('\\');
    let body = body.split_once(char::is_whitespace)?.1.trim();
    // A navigation field may carry a hyperlink to its sheet after the format.
    let body = body.find(" \\href ").map_or(body, |p| body[..p].trim());
    let (target, fmt) = match body.find("\\f ") {
        Some(p) => {
            let f = body[p + 3..].trim();
            let f = f.strip_prefix('"').unwrap_or(f);
            let f = f.strip_suffix('"').unwrap_or(f);
            (body[..p].trim(), f.replace("\\\"", "\""))
        }
        None => (body, String::new()),
    };
    // `Database("…").SheetSet("…")[.Component("…")].Property`: the path has
    // dots of its own, the property follows the last call.
    let (component, property) = if target.starts_with("Database(") {
        let p = target.rfind(").")?;
        (&target[..=p], &target[p + 2..])
    } else {
        target.split_once('.')?
    };
    Some((component.to_string(), property.to_string(), fmt))
}

pub fn parse_navigation(component: &str) -> Option<(String, String, Option<String>)> {
    let arg = |s: &str, call: &str| -> Option<(String, usize)> {
        let start = s.find(call)? + call.len();
        let end = start + s[start..].find("\")")?;
        Some((s[start..end].to_string(), end + 2))
    };
    let (file, _) = arg(component, "Database(\"")?;
    let (set, after) = arg(component, ".SheetSet(\"")?;
    let comp = arg(&component[after..], ".Component(\"").map(|(c, _)| c);
    Some((file, set, comp))
}

pub fn field_child_values(code: &str) -> Vec<(&'static str, String)> {
    let Some((component, property, _)) = parse_code(code) else {
        return Vec::new();
    };
    match parse_navigation(&component) {
        Some((file, set, comp)) => {
            let mut v = Vec::new();
            if let Some(c) = comp {
                v.push(("SheetSetCompId", c));
                v.push(("SheetSetCompName", "Component".to_string()));
            }
            v.push(("SheetSetFile", file));
            v.push(("SheetSetId", set));
            v.push(("SheetSetPropertyName", property));
            v
        }
        None => vec![("SheetSetCompName", component), ("SheetSetPropertyName", property)],
    }
}

pub(crate) fn eval_acsm(
    doc: &CadDocument,
    code: &str,
    ctx: &dyn FieldContext,
    layout: Option<&str>,
    text_case: impl Fn(String, &str) -> String,
) -> Option<String> {
    const NOT_A_SHEET: &str = "####";
    let Some((component, property, fmt)) = parse_code(code) else {
        return Some(NOT_A_SHEET.into());
    };
    if let Some(component) = component.strip_prefix('?') {
        return Some(placeholder_type_name(component, &property));
    }
    // A view's ViewportScale: the scale of the viewport showing its named
    // view in the sheet drawing.
    if property == "ViewportScale" {
        if let Some((file, set, Some(comp))) = parse_navigation(&component) {
            let named = ctx
                .sheet_sets(&mut |db: &SheetSetDatabase| {
                    db.path.as_deref().filter(|p| path_key(p) == path_key(&file))?;
                    view_named_ref(db, &set, &comp)
                })
                .or_else(|| read_view_named_ref(&file, &set, &comp));
            let Some(named) = named else {
                return Some(NOT_A_SHEET.into());
            };
            let (drawing, handle, name) = split_named_ref(&named);
            return Some(view_scale(doc, drawing, handle, name).map_or_else(|| NOT_A_SHEET.into(), |scale| crate::fields::plot_scale_text(doc, scale, &fmt)));
        }
    }
    let value = if let Some((file, set, comp)) = parse_navigation(&component) {
        // A navigation field names its database: an open one, else the file.
        let key = path_key(&file);
        ctx.sheet_sets(&mut |db: &SheetSetDatabase| {
            db.path.as_deref().filter(|p| path_key(p) == key)?;
            db.navigation_value(&set, comp.as_deref(), &property)
        })
        .or_else(|| read_navigation(&file, &set, comp.as_deref(), &property))
    } else {
        // The current sheet: a field on a sheet's layout (model space is no sheet).
        let Some(drawing) = doc.source_path.as_deref() else {
            return Some(NOT_A_SHEET.into());
        };
        let Some(layout) = layout.filter(|l| !l.eq_ignore_ascii_case("Model")) else {
            return Some(NOT_A_SHEET.into());
        };
        ctx.sheet_sets(&mut |db: &SheetSetDatabase| {
            let sheet = db.sheet_for(drawing, Some(layout))?;
            db.sheet_value(sheet, &component, &property)
        })
    };
    Some(match value {
        // A known property without a value.
        Some(v) if v.is_empty() => "----".into(),
        Some(v) => text_case(v, &fmt),
        None => NOT_A_SHEET.into(),
    })
}

fn view_named_ref(db: &SheetSetDatabase, set: &str, comp: &str) -> Option<String> {
    if !db.sheet_set().id().eq_ignore_ascii_case(set) {
        return None;
    }
    let r = db.find(comp)?.named("NamedView")?;
    Some(format!("{}|{}|{}", db.resolve_file(r), r.prop("AcDbHandle").unwrap_or(""), r.prop("Name").unwrap_or("")))
}

fn split_named_ref(s: &str) -> (&str, &str, &str) {
    let mut p = s.splitn(3, '|');
    (p.next().unwrap_or(""), p.next().unwrap_or(""), p.next().unwrap_or(""))
}

#[cfg(not(target_arch = "wasm32"))]
fn read_view_named_ref(file: &str, set: &str, comp: &str) -> Option<String> {
    view_named_ref(&SheetSetDatabase::read(file).ok()?, set, comp)
}

#[cfg(target_arch = "wasm32")]
fn read_view_named_ref(_: &str, _: &str, _: &str) -> Option<String> {
    None
}

fn view_scale(doc: &CadDocument, drawing: &str, handle: &str, name: &str) -> Option<f64> {
    let in_doc = doc.source_path.as_deref().is_some_and(|p| path_key(p) == path_key(drawing));
    #[cfg(not(target_arch = "wasm32"))]
    let loaded;
    let sheet: &CadDocument = if in_doc {
        doc
    } else {
        #[cfg(not(target_arch = "wasm32"))]
        {
            loaded = crate::DwgReader::from_file(drawing).ok()?.read().ok()?;
            &loaded
        }
        #[cfg(target_arch = "wasm32")]
        return None;
    };
    let view = sheet
        .views
        .iter()
        .find(|v| format!("{:X}", v.handle.value()).eq_ignore_ascii_case(handle))
        .or_else(|| sheet.views.iter().find(|v| v.name.eq_ignore_ascii_case(name)))?;
    // ponytail: the view and its viewport are paired by the nearest centre;
    // a stored link would be exact if one turns up.
    sheet
        .entities()
        .filter_map(|e| match e {
            crate::EntityType::Viewport(vp) if vp.id != 1 && vp.view_height > 0.0 => Some(vp),
            _ => None,
        })
        .min_by(|a, b| {
            let d = |vp: &crate::entities::Viewport| (vp.center.x - view.center.x).hypot(vp.center.y - view.center.y);
            d(a).total_cmp(&d(b))
        })
        .map(|vp| if vp.custom_scale > 0.0 { vp.custom_scale } else { vp.height / vp.view_height })
}

#[cfg(not(target_arch = "wasm32"))]
fn read_navigation(file: &str, set: &str, component: Option<&str>, property: &str) -> Option<String> {
    SheetSetDatabase::read(file).ok()?.navigation_value(set, component, property)
}

#[cfg(target_arch = "wasm32")]
fn read_navigation(_: &str, _: &str, _: Option<&str>, _: &str) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cipher_and_round_trip() {
        assert_eq!(encode(b"<?xml"), vec![0xD0, 0xCD, 0x14, 0xE3, 0xE0]);
        let all: Vec<u8> = (0..=255).collect();
        assert_eq!(decode(&encode(&all)), all);

        let mut db = SheetSetDatabase::new("Demo Set", "A & B <c>");
        let set = db.sheet_set().id().to_string();
        let sub = db.add_subset(&set, "Architectural", "Arch subset").unwrap();
        let sheet = db.add_sheet(&sub, "A-101", "Ground Floor Plan", "Sheet description").unwrap();
        db.set_layout_reference(
            &sheet,
            "Layout",
            &LayoutReference { file_name: "C:\\p\\sheet1.dwg".into(), name: "Layout1".into(), handle: "6F".into() },
        );
        set_custom_property(db.sheet_set_mut(), "Drawn By", "RS", CUSTOM_SHEET_PROP);
        let back = SheetSetDatabase::from_bytes(&db.to_bytes()).unwrap();
        assert_eq!(back.root, db.root);
        let s = back.sheet_for("c:/p/SHEET1.dwg", Some("Layout1")).unwrap();
        assert_eq!(back.sheet_value(s, "Sheet", "NumberAndTitle").unwrap(), "A-101 - Ground Floor Plan");
        assert_eq!(back.sheet_value(s, "Subset", "Name").unwrap(), "Architectural");
        assert_eq!(back.sheet_value(s, "Sheet", "Drawn By").unwrap(), "RS");
        assert_eq!(back.sheet_value(s, "SheetSet", "Description").unwrap(), "A & B <c>");
        // Named children sorted, Layout between IssuePurpose and Number.
        let names: Vec<_> = s.children.iter().filter_map(|c| c.propname()).collect();
        assert_eq!(names, ["Desc", "Layout", "Number", "SheetViews", "Title"]);
        #[cfg(windows)]
        assert_eq!(
            relative_path(std::path::Path::new("C:\\p"), std::path::Path::new("C:\\p\\sheet1.dwg")).unwrap(),
            ".\\sheet1.dwg"
        );
        assert_eq!(
            parse_code("\\AcSm.16.2 ?Sheet.Drawn By \\f \"%tc4\""),
            Some(("?Sheet".into(), "Drawn By".into(), "%tc4".into()))
        );
    }
}
