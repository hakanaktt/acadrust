//! DXF writer module

mod stream_writer;
mod text_writer;
mod binary_writer;
mod section_writer;

pub use stream_writer::{DxfStreamWriter, DxfStreamWriterExt, value_type_for_code};
pub use text_writer::DxfTextWriter;
pub use binary_writer::DxfBinaryWriter;
pub use section_writer::SectionWriter;

use crate::document::CadDocument;
use crate::error::Result;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// DXF file writer
pub struct DxfWriter {
    document: CadDocument,
    /// Whether to write binary DXF format
    pub binary: bool,
}

impl DxfWriter {
    /// Create a new DXF writer for ASCII output
    pub fn new(document: CadDocument) -> Self {
        Self {
            document,
            binary: false,
        }
    }

    /// Create a new DXF writer for binary output
    pub fn new_binary(document: CadDocument) -> Self {
        Self {
            document,
            binary: true,
        }
    }

    /// Set whether to write binary format
    pub fn set_binary(&mut self, binary: bool) {
        self.binary = binary;
    }
    
    /// Write to a file
    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        self.write_to_writer(writer)
    }

    /// Write to any writer
    pub fn write_to_writer<W: Write>(&self, writer: W) -> Result<()> {
        if self.binary {
            let mut stream_writer = DxfBinaryWriter::new(writer)?;
            self.write_dxf(&mut stream_writer)?;
            stream_writer.flush()?;
        } else {
            let mut stream_writer = DxfTextWriter::new(writer);
            self.write_dxf(&mut stream_writer)?;
            stream_writer.flush()?;
        }
        Ok(())
    }

    /// Write to a byte vector (useful for testing)
    pub fn write_to_vec(&self) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        self.write_to_writer(&mut buffer)?;
        Ok(buffer)
    }

    /// Write DXF content to a stream writer
    fn write_dxf<W: DxfStreamWriter>(&self, writer: &mut W) -> Result<()> {
        let mut section_writer = SectionWriter::new(writer);

        // Write all sections
        section_writer.write_header(&self.document)?;
        section_writer.write_classes(&self.document)?;
        section_writer.write_tables(&self.document)?;
        section_writer.write_blocks(&self.document)?;
        section_writer.write_entities(&self.document)?;
        section_writer.write_objects(&self.document)?;

        // Write EOF
        writer.write_string(0, "EOF")?;

        Ok(())
    }

    /// Get a reference to the document
    pub fn document(&self) -> &CadDocument {
        &self.document
    }
}

/// Convenience function to write a document to a file
pub fn write_dxf<P: AsRef<Path>>(document: &CadDocument, path: P) -> Result<()> {
    // Clone the document for writing
    let writer = DxfWriter::new(document.clone());
    writer.write_to_file(path)
}

/// Convenience function to write a document to a binary DXF file
pub fn write_binary_dxf<P: AsRef<Path>>(document: &CadDocument, path: P) -> Result<()> {
    let writer = DxfWriter::new_binary(document.clone());
    writer.write_to_file(path)
}
