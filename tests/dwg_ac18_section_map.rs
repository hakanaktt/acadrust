//! The AC18 section map (2004 to 2018 DWG) as AutoCAD writes it: an empty, unnamed descriptor
//! with section id 0 comes first and the named sections are numbered from 1. AutoCAD refuses a
//! file whose Header section is numbered 0 ("drawing file is not valid").

use std::io::Cursor;

use acadrust::entities::{EntityType, Line};
use acadrust::io::dwg::decompressor_ac18::decompress_ac18;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfVersion};

/// Section types of the AC18 page map and section map (system pages, headers not masked).
const PAGE_MAP: u32 = 0x4163_0E3B;
const SECTION_MAP: u32 = 0x4163_003B;
/// Data page headers are masked with this value XOR the page's file offset.
const DATA_PAGE_MASK: u32 = 0x4164_536B;

const VERSIONS: [DxfVersion; 4] = [
    DxfVersion::AC1018,
    DxfVersion::AC1024,
    DxfVersion::AC1027,
    DxfVersion::AC1032,
];

fn dwg(version: DxfVersion) -> Vec<u8> {
    let mut document = CadDocument::with_version(version);
    for i in 0..3 {
        let x = f64::from(i);
        document
            .add_entity(EntityType::Line(Line::from_coords(
                x,
                0.0,
                0.0,
                x + 1.0,
                1.0,
                0.0,
            )))
            .unwrap();
    }
    DwgWriter::write_to_vec(&document).unwrap()
}

fn i32_at(bytes: &[u8], at: usize) -> i32 {
    i32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn u64_at(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())
}

/// Offset and decompressed data of the AC18 system page of `kind`: a 20-byte header (type,
/// decompressed size, compressed size, compression 2, checksum) followed by the data.
fn system_page(bytes: &[u8], kind: u32) -> (usize, Vec<u8>) {
    let magic = kind.to_le_bytes();
    let hits: Vec<usize> = (0..bytes.len().saturating_sub(20))
        .filter(|&at| bytes[at..at + 4] == magic)
        .filter(|&at| {
            let comp = i32_at(bytes, at + 8);
            i32_at(bytes, at + 4) > 0
                && comp > 0
                && i32_at(bytes, at + 12) == 2
                && at + 20 + comp as usize <= bytes.len()
        })
        .collect();
    assert_eq!(hits.len(), 1, "system page {kind:#X} found at {hits:?}");
    let at = hits[0];
    let size = i32_at(bytes, at + 4) as usize;
    let compressed = i32_at(bytes, at + 8) as usize;
    (
        at,
        decompress_ac18(&bytes[at + 20..at + 20 + compressed], size),
    )
}

struct Descriptor {
    name: String,
    size: u64,
    id: i32,
    /// Page numbers of the section's data pages.
    pages: Vec<i32>,
}

/// The section map's two descriptor counts (at 0x00 and 0x10) and its descriptors.
fn section_map(bytes: &[u8]) -> (i32, i32, Vec<Descriptor>) {
    let (_, map) = system_page(bytes, SECTION_MAP);
    let count = i32_at(&map, 0);
    let mut descriptors = Vec::new();
    let mut pos = 20;
    for _ in 0..count {
        let page_count = i32_at(&map, pos + 8).max(0) as usize;
        let name = map[pos + 32..pos + 96]
            .split(|&b| b == 0)
            .next()
            .unwrap_or_default();
        let pages = (0..page_count)
            .map(|k| i32_at(&map, pos + 96 + 16 * k))
            .collect();
        descriptors.push(Descriptor {
            name: String::from_utf8_lossy(name).into_owned(),
            size: u64_at(&map, pos),
            id: i32_at(&map, pos + 24),
            pages,
        });
        pos += 96 + 16 * page_count;
    }
    (count, i32_at(&map, 0x10), descriptors)
}

/// File offset of every page in the page map: pages follow each other from 0x100.
fn page_offsets(bytes: &[u8]) -> Vec<(i32, usize)> {
    let (_, map) = system_page(bytes, PAGE_MAP);
    let mut offsets = Vec::new();
    let mut at = 0x100;
    for entry in map.chunks_exact(8) {
        let (number, size) = (i32_at(entry, 0), i32_at(entry, 4));
        offsets.push((number, at));
        at += size.max(0) as usize;
    }
    offsets
}

/// Section number stored in the masked header of the data page at `at`.
fn data_page_section(bytes: &[u8], at: usize) -> i32 {
    let mask = (DATA_PAGE_MASK ^ at as u32).to_le_bytes();
    let field: Vec<u8> = (4..8).map(|k| bytes[at + k] ^ mask[k % 4]).collect();
    i32_at(&field, 0)
}

#[test]
fn ac18_section_map_opens_with_the_empty_descriptor() {
    for version in VERSIONS {
        let bytes = dwg(version);
        let (count, count_again, descriptors) = section_map(&bytes);
        assert_eq!(count as usize, descriptors.len(), "{version:?}");
        assert_eq!(count_again, count, "{version:?}: the two descriptor counts");
        let first = &descriptors[0];
        assert_eq!(
            (first.name.as_str(), first.id, first.size, first.pages.len()),
            ("", 0, 0, 0),
            "{version:?}: the first descriptor is the empty one with id 0"
        );
        let document = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
        assert_eq!(document.entities().count(), 3, "{version:?}");
    }
}

#[test]
fn ac18_named_sections_are_numbered_from_one() {
    for version in VERSIONS {
        let bytes = dwg(version);
        let (_, _, descriptors) = section_map(&bytes);
        let named: Vec<&Descriptor> = descriptors.iter().filter(|d| !d.name.is_empty()).collect();
        assert_eq!(named.len(), descriptors.len() - 1, "{version:?}");
        let mut ids: Vec<i32> = named.iter().map(|d| d.id).collect();
        ids.sort_unstable();
        let expected: Vec<i32> = (1..=named.len() as i32).collect();
        assert_eq!(ids, expected, "{version:?}: named section ids");
        let header = named.iter().find(|d| d.name == "AcDb:Header").unwrap();
        assert_eq!(header.id, 1, "{version:?}: the Header section is section 1");
    }
}

#[test]
fn ac18_data_pages_carry_their_section_id() {
    for version in VERSIONS {
        let bytes = dwg(version);
        let (_, _, descriptors) = section_map(&bytes);
        let offsets = page_offsets(&bytes);
        for descriptor in &descriptors {
            for page in &descriptor.pages {
                let at = offsets.iter().find(|(n, _)| n == page).unwrap().1;
                assert_eq!(
                    data_page_section(&bytes, at),
                    descriptor.id,
                    "{version:?}: page {page} of {}",
                    descriptor.name
                );
            }
        }
    }
}
