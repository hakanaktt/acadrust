//! DWG file writer — top-level orchestrator.
//!
//! Mirrors ACadSharp's `DwgWriter`.
//!
//! This module ties together all section writers (header, classes, objects,
//! handles, preview, app-info, aux-header, summary-info) and the file
//! header writer to produce a complete DWG binary file from a `CadDocument`.

use crate::document::CadDocument;
use crate::error::Result;
use crate::io::dwg::constants::section_names;
use crate::io::dwg::header_handles::DwgHeaderHandlesCollection;
use crate::io::dwg::section_io::SectionIO;
use crate::io::dwg::writer::app_info_writer::DwgAppInfoWriter;
use crate::io::dwg::writer::aux_header_writer::DwgAuxHeaderWriter;
use crate::io::dwg::writer::classes_writer::DwgClassesWriter;
use crate::io::dwg::writer::file_header_writer::{
    DwgFileHeaderWriterAC15, DwgFileHeaderWriterAC18, IDwgFileHeaderWriter,
};
use crate::io::dwg::writer::handle_writer::DwgHandleWriter;
use crate::io::dwg::writer::header_writer::DwgHeaderWriter;
use crate::io::dwg::writer::object_writer::DwgObjectWriter;
use crate::io::dwg::writer::preview_writer::DwgPreviewWriter;
use crate::io::dwg::writer::summary_info_writer::DwgSummaryInfoWriter;
use crate::summary_info::CadSummaryInfo;
use crate::types::DxfVersion;

/// DWG file writer.
///
/// # Usage
/// ```no_run
/// use acadrust::document::CadDocument;
/// use acadrust::io::dwg::writer::dwg_writer::DwgWriter;
///
/// let doc = CadDocument::new(); // or read from existing file
/// let bytes = DwgWriter::write(&doc).unwrap();
/// std::fs::write("output.dwg", &bytes).unwrap();
/// ```
pub struct DwgWriter;

impl DwgWriter {
    /// Write a `CadDocument` to DWG binary format, returning the complete
    /// file contents as a byte vector.
    pub fn write(doc: &CadDocument) -> Result<Vec<u8>> {
        Self::write_with_info(doc, &CadSummaryInfo::default())
    }

    /// Write a `CadDocument` with explicit summary info.
    pub fn write_with_info(doc: &CadDocument, summary_info: &CadSummaryInfo) -> Result<Vec<u8>> {
        let version = doc.version;
        let sio = SectionIO::new(version);
        let maintenance_version: u8 = 0;
        let code_page = Self::code_page_for_version(version);
        let version_string = version.as_str();

        // Build the handles collection from the document.
        let handles = Self::build_header_handles(doc);

        // Create the version-appropriate file header writer.
        let mut file_writer: Box<dyn IDwgFileHeaderWriter> =
            Self::create_file_header_writer(version, version_string, code_page, maintenance_version);

        // -------------------------------------------------------------------
        // 1. Header section (AcDb:Header)
        // -------------------------------------------------------------------
        let header_data = DwgHeaderWriter::new(version)
            .write(&doc.header, &handles, maintenance_version)?;
        file_writer.add_section(section_names::HEADER, header_data, sio.r2004_plus, 0)?;

        // -------------------------------------------------------------------
        // 2. Classes section (AcDb:Classes)
        // -------------------------------------------------------------------
        let classes: Vec<_> = doc.classes.iter().cloned().collect();
        let classes_data = DwgClassesWriter::new(version)
            .write(&classes, maintenance_version)?;
        file_writer.add_section(section_names::CLASSES, classes_data, sio.r2004_plus, 0)?;

        // -------------------------------------------------------------------
        // 3. Objects section (AcDb:AcDbObjects) + Handles section (AcDb:Handles)
        // -------------------------------------------------------------------
        let obj_writer = DwgObjectWriter::new(version, doc);
        let (objects_data, handle_map) = obj_writer.write(doc)?;

        file_writer.add_section(
            section_names::ACDB_OBJECTS,
            objects_data,
            sio.r2004_plus,
            0,
        )?;

        let section_offset = file_writer.handle_section_offset();
        let handles_data = DwgHandleWriter::new(version)
            .write(&handle_map, section_offset)?;
        file_writer.add_section(section_names::HANDLES, handles_data, sio.r2004_plus, 0)?;

        // -------------------------------------------------------------------
        // 4. Preview section (AcDb:Preview) — write empty placeholder
        // -------------------------------------------------------------------
        let preview_data = DwgPreviewWriter::new(version).write_empty()?;
        file_writer.add_section(section_names::PREVIEW, preview_data, false, 0)?;

        // -------------------------------------------------------------------
        // 5. R2004+ only sections
        // -------------------------------------------------------------------
        if sio.r2004_plus {
            // App Info (AcDb:AppInfo)
            let app_info_data = DwgAppInfoWriter::new(version).write()?;
            file_writer.add_section(section_names::APP_INFO, app_info_data, true, 0)?;

            // Aux Header (AcDb:AuxHeader)
            let aux_header_data = DwgAuxHeaderWriter::new(version)
                .write(&doc.header, maintenance_version as i16)?;
            file_writer.add_section(section_names::AUX_HEADER, aux_header_data, true, 0)?;

            // Summary Info (AcDb:SummaryInfo)
            let summary_data = DwgSummaryInfoWriter::new(version)
                .write(summary_info)?;
            file_writer.add_section(section_names::SUMMARY_INFO, summary_data, true, 0)?;
        }

        // -------------------------------------------------------------------
        // 6. Assemble final file
        // -------------------------------------------------------------------
        file_writer.write_file()
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Create the appropriate file header writer for the given version.
    fn create_file_header_writer(
        version: DxfVersion,
        version_string: &str,
        code_page: u16,
        maintenance_version: u8,
    ) -> Box<dyn IDwgFileHeaderWriter> {
        match version {
            DxfVersion::AC1012 | DxfVersion::AC1014 | DxfVersion::AC1015 => {
                Box::new(DwgFileHeaderWriterAC15::new(
                    version,
                    version_string,
                    code_page,
                    maintenance_version,
                ))
            }
            _ => Box::new(DwgFileHeaderWriterAC18::new(
                version,
                version_string,
                code_page,
                maintenance_version,
            )),
        }
    }

    /// Build the header handles collection from the document.
    ///
    /// The header writer needs handle references for table controls,
    /// current settings, and well-known blocks.
    fn build_header_handles(doc: &CadDocument) -> DwgHeaderHandlesCollection {
        let mut h = DwgHeaderHandlesCollection::new();
        let hdr = &doc.header;

        // Current settings
        h.set_clayer(Some(hdr.current_layer_handle.value()));
        h.set_textstyle(Some(hdr.current_text_style_handle.value()));
        h.set_celtype(Some(hdr.current_linetype_handle.value()));
        h.set_dimstyle(Some(hdr.current_dimstyle_handle.value()));
        h.set_cmlstyle(Some(0)); // no multiline style stored yet

        // Block/layout handles
        h.set_paper_space(Some(hdr.paper_space_block_handle.value()));
        h.set_model_space(Some(hdr.model_space_block_handle.value()));
        h.set_bylayer(Some(hdr.bylayer_linetype_handle.value()));
        h.set_byblock(Some(hdr.byblock_linetype_handle.value()));
        h.set_continuous(Some(hdr.continuous_linetype_handle.value()));

        // Table control object handles
        h.set_block_control_object(Some(hdr.block_control_handle.value()));
        h.set_layer_control_object(Some(hdr.layer_control_handle.value()));
        h.set_style_control_object(Some(hdr.style_control_handle.value()));
        h.set_linetype_control_object(Some(hdr.linetype_control_handle.value()));
        h.set_view_control_object(Some(hdr.view_control_handle.value()));
        h.set_ucs_control_object(Some(hdr.ucs_control_handle.value()));
        h.set_vport_control_object(Some(hdr.vport_control_handle.value()));
        h.set_appid_control_object(Some(hdr.appid_control_handle.value()));
        h.set_dimstyle_control_object(Some(hdr.dimstyle_control_handle.value()));

        // Dictionary handles
        h.set_dictionary_named_objects(Some(hdr.named_objects_dict_handle.value()));
        h.set_dictionary_acad_group(Some(hdr.acad_group_dict_handle.value()));
        h.set_dictionary_acad_mlinestyle(Some(hdr.acad_mlinestyle_dict_handle.value()));
        h.set_dictionary_layouts(Some(hdr.acad_layout_dict_handle.value()));
        h.set_dictionary_plotsettings(Some(hdr.acad_plotsettings_dict_handle.value()));
        h.set_dictionary_plotstyles(Some(hdr.acad_plotstylename_dict_handle.value()));

        h
    }

    /// Return the code page number for the given version.
    /// The standard code page for ANSI_1252 is 30 in DWG files.
    fn code_page_for_version(_version: DxfVersion) -> u16 {
        30 // ANSI_1252 / Western European
    }
}
