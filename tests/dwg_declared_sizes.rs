//! Sizes a DWG file declares for its page map, section map and sections are checked before
//! the reader allocates: a corrupt size ends in an error or a diagnostic, never in a
//! multi-gigabyte allocation that aborts the process.

use std::io::Cursor;

use acadrust::entities::{EntityType, Line};
use acadrust::io::dwg::compression::DwgLZ77AC18Compressor;
use acadrust::io::dwg::decompressor_ac18::decompress_ac18;
use acadrust::io::dwg::decompressor_ac21::decompress_ac21;
use acadrust::io::dwg::dwg21_metadata::Dwg21CompressedMetadata;
use acadrust::{CadDocument, DwgReadOptions, DwgReader, DwgWriter, DxfVersion};

/// Section types of the AC18 page map and section map (system pages, headers not masked).
const PAGE_MAP: u32 = 0x4163_0E3B;
const SECTION_MAP: u32 = 0x4163_003B;
/// AC21 file header: 0x400 bytes at 0x80, interleaved over 3 Reed-Solomon blocks of 239 data
/// bytes. The decoded data holds the metadata length at 0x18 (positive: compressed, negative:
/// stored) and the metadata at 0x20.
const AC21_HEADER: usize = 0x80;
const RS_BLOCKS: usize = 3;
const RS_DATA: usize = 239;

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

fn entity_count(bytes: &[u8]) -> usize {
    let document = DwgReader::from_stream(Cursor::new(bytes.to_vec()))
        .read()
        .unwrap();
    document.entities().count()
}

/// The strict read must fail; returns its message.
fn strict_error(bytes: &[u8]) -> String {
    match DwgReader::from_stream(Cursor::new(bytes.to_vec())).read() {
        Err(error) => error.to_string(),
        Ok(document) => panic!("read {} entities", document.entities().count()),
    }
}

/// The error message of a strict read, or its diagnostics when the read succeeded without
/// the bad section.
fn strict_outcome(bytes: &[u8]) -> String {
    match DwgReader::from_stream(Cursor::new(bytes.to_vec())).read_with_stats() {
        Err(error) => error.to_string(),
        Ok(outcome) => diagnostics(&outcome.stats.diagnostics),
    }
}

fn failsafe_diagnostics(bytes: &[u8]) -> String {
    let outcome = DwgReader::from_stream_with_options(
        Cursor::new(bytes.to_vec()),
        DwgReadOptions::failsafe(),
    )
    .read_with_stats()
    .unwrap();
    diagnostics(&outcome.stats.diagnostics)
}

fn diagnostics(list: &[acadrust::io::read::ReadDiagnostic]) -> String {
    list.iter()
        .map(|d| d.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn i32_at(bytes: &[u8], at: usize) -> i32 {
    i32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn put_i32(bytes: &mut [u8], at: usize, value: i32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// Offset of the AC18 system page of `kind`: a 20-byte header (type, decompressed size,
/// compressed size, compression 2, checksum) followed by the compressed data.
fn system_page(bytes: &[u8], kind: u32) -> usize {
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
    hits[0]
}

/// Decompresses the AC18 section map, passes every section descriptor (name, 96-byte fixed
/// part followed by 16 bytes per page) through `edit`, and writes the recompressed map back
/// in place of the old one.
fn rewrite_section_map(bytes: &[u8], edit: impl Fn(&str, Vec<u8>) -> Vec<u8>) -> Vec<u8> {
    try_rewrite_section_map(bytes, edit).expect("rewritten section map does not fit")
}

/// `rewrite_section_map`, or `None` when the recompressed map is larger than the old one.
fn try_rewrite_section_map(
    bytes: &[u8],
    edit: impl Fn(&str, Vec<u8>) -> Vec<u8>,
) -> Option<Vec<u8>> {
    let at = system_page(bytes, SECTION_MAP);
    let size = i32_at(bytes, at + 4) as usize;
    let room = i32_at(bytes, at + 8) as usize;
    let map = decompress_ac18(&bytes[at + 20..at + 20 + room], size);
    let mut out = map[..20].to_vec();
    let mut pos = 20;
    for _ in 0..i32_at(&map, 0) {
        let end = pos + 96 + 16 * i32_at(&map, pos + 8) as usize;
        let name = map[pos + 32..pos + 96]
            .split(|&b| b == 0)
            .next()
            .unwrap_or_default();
        out.extend(edit(&String::from_utf8_lossy(name), map[pos..end].to_vec()));
        pos = end;
    }
    out.extend_from_slice(&map[pos..]);
    let packed = DwgLZ77AC18Compressor::new().compress(&out, 0, out.len());
    if packed.len() > room {
        return None;
    }
    let mut bytes = bytes.to_vec();
    put_i32(&mut bytes, at + 4, out.len() as i32);
    put_i32(&mut bytes, at + 8, packed.len() as i32);
    bytes[at + 20..at + 20 + packed.len()].copy_from_slice(&packed);
    Some(bytes)
}

/// Makes `section` claim `data_size` bytes with a page count of -1, so that no page
/// descriptor is parsed for it while the declared count reads as `u64::MAX`.
fn inflate_section(bytes: &[u8], section: &str, data_size: u64) -> Vec<u8> {
    rewrite_section_map(bytes, |name, descriptor| {
        if name != section {
            return descriptor;
        }
        let mut fixed = descriptor[..96].to_vec();
        fixed[..8].copy_from_slice(&data_size.to_le_bytes());
        put_i32(&mut fixed, 8, -1);
        fixed
    })
}

/// Makes a section declare a small amount of padding beyond the sum of its parsed page
/// capacities. AC1032 files in the wild use this form for the objects section. `None` when
/// the edited map no longer compresses into the old map's room.
fn overshoot_section(bytes: &[u8], section: &str, extra: u64) -> Option<Vec<u8>> {
    try_rewrite_section_map(bytes, |name, descriptor| {
        if name != section {
            return descriptor;
        }
        let page_count = i32_at(&descriptor, 8).max(0) as u64;
        let page_size = i32_at(&descriptor, 12).max(0) as u64;
        let capacity = page_count.saturating_mul(page_size);
        let mut descriptor = descriptor;
        descriptor[..8].copy_from_slice(&capacity.saturating_add(extra).to_le_bytes());
        descriptor
    })
}

fn rs_index(i: usize) -> usize {
    AC21_HEADER + i / RS_DATA + (i % RS_DATA) * RS_BLOCKS
}

/// Decodes the AC21 file header metadata, applies `edit` and stores the metadata back
/// uncompressed (the reader accepts both forms).
fn with_ac21_metadata(bytes: &[u8], edit: impl FnOnce(&mut Dwg21CompressedMetadata)) -> Vec<u8> {
    let mut decoded: Vec<u8> = (0..RS_BLOCKS * RS_DATA)
        .map(|i| bytes[rs_index(i)])
        .collect();
    let len = i32_at(&decoded, 0x18);
    let mut raw = vec![0u8; 0x110];
    if len > 0 {
        decompress_ac21(&decoded, 0x20, len as u32, &mut raw);
    } else {
        raw.copy_from_slice(&decoded[0x20..0x20 + 0x110]);
    }
    let mut metadata = Dwg21CompressedMetadata::from_bytes(&raw).unwrap();
    edit(&mut metadata);
    let raw = metadata.to_bytes();
    put_i32(&mut decoded, 0x18, -(raw.len() as i32));
    decoded[0x20..0x20 + raw.len()].copy_from_slice(&raw);
    let mut bytes = bytes.to_vec();
    for (i, byte) in decoded.into_iter().enumerate() {
        bytes[rs_index(i)] = byte;
    }
    bytes
}

#[test]
fn ac18_page_and_section_map_sizes_are_checked_before_allocation() {
    for version in [
        DxfVersion::AC1018,
        DxfVersion::AC1024,
        DxfVersion::AC1027,
        DxfVersion::AC1032,
    ] {
        let bytes = dwg(version);
        assert_eq!(entity_count(&bytes), 3, "{version:?}");
        for (kind, name) in [(PAGE_MAP, "page map"), (SECTION_MAP, "section map")] {
            let at = system_page(&bytes, kind);
            for size in [-1, 0, i32::MAX] {
                let mut bad = bytes.clone();
                put_i32(&mut bad, at + 4, size);
                let expected = format!("AC18 {name} decompressed size {size} is outside");
                let message = strict_error(&bad);
                assert!(
                    message.contains(&expected) && message.contains("16777216"),
                    "{version:?} {name} {size}: {message}"
                );
                let failsafe = failsafe_diagnostics(&bad);
                assert!(failsafe.contains(&expected), "{version:?}: {failsafe}");
            }
        }
    }
}

#[test]
fn ac18_section_size_is_bounded_by_its_parsed_pages() {
    let bytes = dwg(DxfVersion::AC1032);
    let same = rewrite_section_map(&bytes, |_, descriptor| descriptor);
    assert_eq!(entity_count(&same), 3);
    for (data_size, expected) in [(1u64 << 40, "1073741824"), (512 << 20, "0 page(s)")] {
        let bad = inflate_section(&bytes, "AcDb:Header", data_size);
        let text = strict_outcome(&bad);
        assert!(
            text.contains(&format!("'AcDb:Header' size {data_size} ")) && text.contains(expected),
            "header section of {data_size} bytes: {text}"
        );
    }
}

#[test]
fn ac18_small_section_capacity_overshoot_keeps_objects_readable() {
    let bytes = dwg(DxfVersion::AC1032);
    // Any overshoot under a page will do: the first whose edited section map still
    // compresses into the old map's room.
    let padded = (1..=255)
        .find_map(|extra| overshoot_section(&bytes, "AcDb:AcDbObjects", extra))
        .expect("an overshoot whose section map fits");
    assert_eq!(entity_count(&padded), 3);
}

#[test]
fn ac21_map_sizes_are_checked_before_allocation() {
    let bytes = dwg(DxfVersion::AC1021);
    assert_eq!(entity_count(&bytes), 3);
    assert_eq!(entity_count(&with_ac21_metadata(&bytes, |_| {})), 3);
    type Set = fn(&mut Dwg21CompressedMetadata, u64);
    let fields: [(&str, Set); 4] = [
        ("AC21 page map decompressed size", |m, v| {
            m.pages_map_size_uncompressed = v;
        }),
        ("AC21 page map compressed size", |m, v| {
            m.pages_map_size_compressed = v;
        }),
        ("AC21 section map decompressed size", |m, v| {
            m.sections_map_size_uncompressed = v;
        }),
        ("AC21 section map compressed size", |m, v| {
            m.sections_map_size_compressed = v;
        }),
    ];
    for (field, set) in fields {
        for value in [0, 1 << 40, u64::MAX] {
            let message = strict_error(&with_ac21_metadata(&bytes, |m| set(m, value)));
            assert!(
                message.contains(&format!("{field} {value} is outside"))
                    && message.contains("16777216"),
                "{field} {value}: {message}"
            );
        }
    }
}

#[test]
fn ac21_header_min_compr_len_does_not_overflow() {
    let bytes = dwg(DxfVersion::AC1021);
    let mut decoded: Vec<u8> = (0..RS_BLOCKS * RS_DATA)
        .map(|i| bytes[rs_index(i)])
        .collect();
    put_i32(&mut decoded, 0x18, i32::MIN);
    let mut bad_bytes = bytes.clone();
    for (i, byte) in decoded.into_iter().enumerate() {
        bad_bytes[rs_index(i)] = byte;
    }
    let _ = DwgReader::from_stream(Cursor::new(bad_bytes)).read();
}
