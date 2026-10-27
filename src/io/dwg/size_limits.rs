//! Limits for the sizes a DWG file declares for its own structures.
//!
//! The reader allocates buffers whose sizes come straight from the file: system pages (the
//! AC18 and AC21 page map and section map), data pages and whole sections. A damaged or
//! hostile file can declare gigabytes, or a negative 32-bit size that turns into nearly
//! `usize::MAX`; the allocation then aborts the process and no caller can catch it. Each such
//! size goes through [`size_from_file`] before anything is allocated, in strict and failsafe
//! mode alike. Buffers sized from file data are reserved with [`try_zeroed_vec`] /
//! [`try_vec_with_capacity`], so an allocation failure becomes a `DxfError`. Callers can also
//! cap the total across a whole read with
//! [`DwgReadOptions::max_decompressed_bytes`](crate::DwgReadOptions::max_decompressed_bytes).

use std::fmt::Display;
use std::ops::RangeInclusive;

use crate::error::DxfError;

/// Largest compressed or decompressed size of a system page: the AC18 and AC21 page map and
/// section map. Real files use a few KiB (5,212 bytes was the largest in 181 drawings).
pub(crate) const MAX_SYSTEM_PAGE_SIZE: usize = 16 * 1024 * 1024;

/// Largest size of one data page. ACAD pages AC18 sections at 0x7400 bytes and AC21
/// sections at 0xF800; single-page sections such as previews reach about 1 MiB.
pub(crate) const MAX_DATA_PAGE_SIZE: usize = 4 * 1024 * 1024;

/// Largest decompressed size of one section; it also bounds the sum of the section's pages.
pub(crate) const MAX_SECTION_SIZE: usize = 1024 * 1024 * 1024;

/// Allocates a zero-filled buffer, reporting an allocation failure as an error.
///
/// Buffers sized from file-declared values use this instead of `vec![0; size]`: the latter
/// aborts the process through `handle_alloc_error`, which neither `Result` nor
/// `catch_unwind` can recover from.
pub(crate) fn try_zeroed_vec(len: usize, what: impl Display) -> Result<Vec<u8>, DxfError> {
    let mut bytes = try_vec_with_capacity(len, what)?;
    bytes.resize(len, 0);
    Ok(bytes)
}

/// Allocates an empty `Vec<u8>` with the requested capacity, reporting an allocation
/// failure as an error instead of aborting.
pub(crate) fn try_vec_with_capacity(
    capacity: usize,
    what: impl Display,
) -> Result<Vec<u8>, DxfError> {
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(capacity).map_err(|error| {
        DxfError::Custom(format!(
            "failed to allocate {capacity} bytes for {what}: {error}"
        ))
    })?;
    Ok(bytes)
}

/// Converts a size read from a DWG file to `usize`. Negative values and values outside
/// `accepted` are rejected with an error naming the field and the accepted range.
pub(crate) fn size_from_file(
    field: impl Display,
    value: impl Into<i128>,
    accepted: RangeInclusive<usize>,
) -> Result<usize, DxfError> {
    let value = value.into();
    match usize::try_from(value) {
        Ok(size) if accepted.contains(&size) => Ok(size),
        _ => Err(DxfError::InvalidFormat(format!(
            "{field} {value} is outside the accepted range {}..={} bytes",
            accepted.start(),
            accepted.end()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::io::Cursor;

    use super::*;
    use crate::io::dwg::decompressor_ac18::decompress_ac18;
    use crate::io::dwg::dwg_reader::{DwgFileHeaderInfo, DwgPageCrcInfo, DwgSectionInfo};
    use crate::io::dwg::{DwgReadOptions, DwgReader, DwgVersion};

    #[test]
    fn sizes_in_range_are_converted() {
        assert_eq!(size_from_file("size", 0i32, 0..=8).unwrap(), 0);
        assert_eq!(size_from_file("size", 8u64, 0..=8).unwrap(), 8);
        assert_eq!(size_from_file("size", 1i64, 1..=8).unwrap(), 1);
    }

    #[test]
    fn negative_zero_and_oversized_values_are_rejected() {
        let cases: [(i128, RangeInclusive<usize>); 5] = [
            (-1, 0..=8),
            (i128::from(i64::MIN), 0..=usize::MAX),
            (0, 1..=8),
            (9, 0..=8),
            (i128::from(u64::MAX) + 1, 0..=usize::MAX),
        ];
        for (value, accepted) in cases {
            assert!(
                size_from_file("size", value, accepted.clone()).is_err(),
                "{value} in {accepted:?}"
            );
        }
        let message = size_from_file("AC18 page map decompressed size", -1i32, 1..=16)
            .unwrap_err()
            .to_string();
        assert!(
            message.contains(
                "AC18 page map decompressed size -1 is outside the accepted range 1..=16 bytes"
            ),
            "{message}"
        );
    }

    #[test]
    fn decompress_ac18_does_not_reserve_the_declared_size() {
        // A 4-byte literal run (opcode 0x01) followed by the end marker.
        let out = decompress_ac18(&[0x01, 1, 2, 3, 4, 0x11], usize::MAX);
        assert_eq!(out, [1, 2, 3, 4]);
        assert!(out.capacity() <= MAX_DATA_PAGE_SIZE);
    }

    fn header_info(
        ac18: bool,
        section: DwgSectionInfo,
        page_records: HashMap<i32, (i64, i64)>,
    ) -> DwgFileHeaderInfo {
        let (version_string, version) = if ac18 {
            ("AC1018", DwgVersion::AC18)
        } else {
            ("AC1021", DwgVersion::AC21)
        };
        DwgFileHeaderInfo {
            version_string: version_string.to_owned(),
            version,
            acad_maintenance_version: 0,
            preview_address: 0,
            dwg_version: 0,
            app_release_version: 0,
            code_page: 0,
            security_type: 0,
            summary_info_addr: 0,
            vba_project_addr: 0,
            ac21_metadata: None,
            ac21_header_crc: None,
            ac21_unknown_key: None,
            ac21_compressed_data_crc: None,
            page_records,
            section_descriptors: vec![section],
            section_locators: HashMap::new(),
            objects_base_offset: 0,
            is_ac18_format: ac18,
        }
    }

    /// A header section declaring `data_size` bytes in pages of up to `page_size` bytes.
    fn section(
        data_size: u64,
        page_size: u64,
        page_count: u64,
        pages: Vec<DwgPageCrcInfo>,
    ) -> DwgSectionInfo {
        DwgSectionInfo {
            name: "AcDb:Header".to_owned(),
            compressed_size: data_size,
            decompressed_size: page_size,
            encrypted: 0,
            hash_code: 0,
            encoding: 4,
            page_count,
            pages,
        }
    }

    fn page(page_number: i64, decompressed_size: u64, compressed_size: u64) -> DwgPageCrcInfo {
        DwgPageCrcInfo {
            page_number,
            offset: 0,
            size: 0,
            decompressed_size,
            compressed_size,
            checksum: 0,
            crc: 0,
        }
    }

    /// Reading the section must fail with `expected` in strict and in failsafe mode.
    fn rejected(info: &DwgFileHeaderInfo, expected: &str) {
        for options in [DwgReadOptions::default(), DwgReadOptions::failsafe()] {
            let failsafe = options.failsafe;
            let stream = Cursor::new(vec![0u8; 64]);
            let mut reader = DwgReader::from_stream_with_options(stream, options);
            // Never print the buffer: it may be as large as the declared size.
            let message = match reader.get_section_buffer("AcDb:Header", info) {
                Err(error) => error.to_string(),
                Ok(bytes) => panic!("failsafe={failsafe}: read {} bytes", bytes.len()),
            };
            assert!(message.contains(expected), "failsafe={failsafe}: {message}");
        }
    }

    #[test]
    fn ac18_section_is_bounded_by_the_pages_actually_parsed() {
        // A page count of -1 reads as u64::MAX, while no page descriptor follows it.
        let info = header_info(
            true,
            section(512 << 20, 0x7400, u64::MAX, vec![]),
            HashMap::new(),
        );
        rejected(&info, "size 536870912 exceeds its 0 page(s)");
        let info = header_info(
            true,
            section(1 << 40, 0x7400, u64::MAX, vec![]),
            HashMap::new(),
        );
        rejected(
            &info,
            "size 1099511627776 is outside the accepted range 0..=1073741824",
        );
    }

    #[test]
    fn ac18_page_size_is_limited() {
        let too_big = MAX_DATA_PAGE_SIZE as u64 + 1;
        let info = header_info(
            true,
            section(8, too_big, 1, vec![page(1, 0, 0)]),
            HashMap::new(),
        );
        rejected(&info, "page size 4194305 is outside");
        // A negative 32-bit page size is stored sign-extended.
        let info = header_info(
            true,
            section(8, u64::MAX, 1, vec![page(1, 0, 0)]),
            HashMap::new(),
        );
        rejected(&info, "page size 18446744073709551615 is outside");
    }

    #[test]
    fn ac21_page_sizes_are_checked_before_any_page_is_read() {
        let records = HashMap::from([(1, (0i64, 64i64))]);
        let info = header_info(
            false,
            section(8, 0xF800, 1, vec![page(1, 1 << 40, 8)]),
            records.clone(),
        );
        rejected(&info, "page 1 decompressed size 1099511627776 is outside");
        let info = header_info(
            false,
            section(8, 0xF800, 1, vec![page(1, 8, 1 << 40)]),
            records.clone(),
        );
        rejected(&info, "page 1 compressed size 1099511627776 is outside");
        // Failsafe reads zero-filled the declared size of a page missing from the page map.
        let info = header_info(
            false,
            section(8, 0xF800, 1, vec![page(7, u64::MAX, 8)]),
            records,
        );
        rejected(
            &info,
            "page 7 decompressed size 18446744073709551615 is outside",
        );
    }

    #[test]
    fn ac21_section_size_and_page_total_are_limited() {
        let info = header_info(false, section(1 << 40, 0xF800, 0, vec![]), HashMap::new());
        rejected(
            &info,
            "size 1099511627776 is outside the accepted range 0..=1073741824",
        );
        // 300 pages of 4 MiB add up to more than one section may hold.
        let pages = (1..=300)
            .map(|n| page(n, MAX_DATA_PAGE_SIZE as u64, 8))
            .collect();
        let info = header_info(false, section(8, 0xF800, 300, pages), HashMap::new());
        rejected(&info, "page total 1258291200 is outside");
    }

    /// Reading the section under `budget` must fail with "budget" in strict and failsafe mode.
    fn rejected_by_budget(info: &DwgFileHeaderInfo, section_name: &str, budget: usize) {
        for options in [DwgReadOptions::default(), DwgReadOptions::failsafe()] {
            let failsafe = options.failsafe;
            let mut reader = DwgReader::from_stream_with_options(
                Cursor::new(Vec::new()),
                options.with_max_decompressed_bytes(budget),
            );
            let message = match reader.get_section_buffer(section_name, info) {
                Err(error) => error.to_string(),
                Ok(bytes) => panic!("failsafe={failsafe}: read {} bytes", bytes.len()),
            };
            assert!(message.contains("budget"), "failsafe={failsafe}: {message}");
        }
    }

    #[test]
    fn ac18_section_respects_the_total_decompressed_budget() {
        let info = header_info(true, section(8, 8, 1, vec![page(7, 8, 0)]), HashMap::new());
        rejected_by_budget(&info, "AcDb:Header", 7);
    }

    #[test]
    fn ac21_section_respects_the_total_decompressed_budget() {
        let info = header_info(false, section(8, 0xF800, 0, vec![]), HashMap::new());
        rejected_by_budget(&info, "AcDb:Header", 7);
    }

    #[test]
    fn a_section_that_fits_the_budget_is_read() {
        let info = header_info(true, section(8, 8, 1, vec![page(7, 8, 0)]), HashMap::new());
        let mut reader = DwgReader::from_stream_with_options(
            Cursor::new(Vec::new()),
            DwgReadOptions::failsafe().with_max_decompressed_bytes(8),
        );
        assert_eq!(
            reader
                .get_section_buffer("AcDb:Header", &info)
                .unwrap()
                .len(),
            8
        );
    }

    #[test]
    fn decompressed_budget_is_shared_by_all_sections() {
        let mut info = header_info(true, section(8, 8, 1, vec![page(7, 8, 0)]), HashMap::new());
        let mut classes = section(8, 8, 1, vec![page(7, 8, 0)]);
        classes.name = "AcDb:Classes".to_owned();
        info.section_descriptors.push(classes);

        let mut reader = DwgReader::from_stream_with_options(
            Cursor::new(Vec::new()),
            DwgReadOptions::failsafe().with_max_decompressed_bytes(10),
        );
        assert_eq!(
            reader
                .get_section_buffer("AcDb:Header", &info)
                .unwrap()
                .len(),
            8
        );
        let message = reader
            .get_section_buffer("AcDb:Classes", &info)
            .unwrap_err()
            .to_string();
        assert!(message.contains("budget"), "{message}");
    }

    #[test]
    fn allocation_failures_are_reported_as_errors() {
        let message = try_zeroed_vec(usize::MAX, "test buffer")
            .unwrap_err()
            .to_string();
        assert!(message.contains("allocate"), "{message}");
    }
}
