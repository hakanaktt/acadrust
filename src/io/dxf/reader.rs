//! DXF file reader

mod stream_reader;
mod text_reader;
mod binary_reader;
mod section_reader;

pub use stream_reader::DxfStreamReader;
pub use text_reader::DxfTextReader;
pub use binary_reader::DxfBinaryReader;

use section_reader::SectionReader;

use crate::document::CadDocument;
use crate::error::Result;
use crate::types::DxfVersion;
use std::fs::File;
use std::io::{BufReader, Read, Seek};
use std::path::Path;

/// DXF file reader
pub struct DxfReader {
    reader: Box<dyn DxfStreamReader>,
    version: DxfVersion,
}

impl DxfReader {
    /// Create a new DXF reader from any reader
    pub fn from_reader<R: Read + Seek + 'static>(reader: R) -> Result<Self> {
        let mut buf_reader = BufReader::new(reader);

        // Detect if binary
        let is_binary = Self::is_binary(&mut buf_reader)?;

        // Create appropriate reader
        let reader: Box<dyn DxfStreamReader> = if is_binary {
            Box::new(DxfBinaryReader::new(buf_reader)?)
        } else {
            // Seek back to start for text DXF files
            buf_reader.seek(std::io::SeekFrom::Start(0))?;
            Box::new(DxfTextReader::new(buf_reader)?)
        };

        Ok(Self {
            reader,
            version: DxfVersion::Unknown,
        })
    }

    /// Create a new DXF reader from a file path
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path)?;
        let mut buf_reader = BufReader::new(file);
        
        // Detect if binary
        let is_binary = Self::is_binary(&mut buf_reader)?;
        
        // Create appropriate reader
        let reader: Box<dyn DxfStreamReader> = if is_binary {
            Box::new(DxfBinaryReader::new(buf_reader)?)
        } else {
            // Seek back to start for text DXF files
            buf_reader.seek(std::io::SeekFrom::Start(0))?;
            Box::new(DxfTextReader::new(buf_reader)?)
        };
        
        Ok(Self {
            reader,
            version: DxfVersion::Unknown,
        })
    }
    
    /// Check if a stream contains binary DXF data
    fn is_binary<R: Read + Seek>(reader: &mut R) -> Result<bool> {
        const SENTINEL: &[u8] = b"AutoCAD Binary DXF";
        let mut buffer = vec![0u8; SENTINEL.len()];
        
        // Try to read the sentinel bytes
        let bytes_read = reader.read(&mut buffer)?;
        
        // Always seek back to start after checking
        reader.seek(std::io::SeekFrom::Start(0))?;
        
        // If file is too small or doesn't match, it's not binary
        if bytes_read < SENTINEL.len() {
            return Ok(false);
        }
        
        Ok(buffer == SENTINEL)
    }
    
    /// Read a DXF file and return a CadDocument
    pub fn read(mut self) -> Result<CadDocument> {
        // Find and read version from header
        self.read_version()?;

        // Create document
        let mut document = CadDocument::new();
        
        // Read all sections
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "SECTION" {
                // Read section name
                if let Some(section_pair) = self.reader.read_pair()? {
                    if section_pair.code == 2 {
                        match section_pair.value_string.as_str() {
                            "HEADER" => self.read_header_section(&mut document)?,
                            "CLASSES" => self.read_classes_section(&mut document)?,
                            "TABLES" => self.read_tables_section(&mut document)?,
                            "BLOCKS" => self.read_blocks_section(&mut document)?,
                            "ENTITIES" => self.read_entities_section(&mut document)?,
                            "OBJECTS" => self.read_objects_section(&mut document)?,
                            _ => {
                                // Skip unknown section
                                self.skip_section()?;
                            }
                        }
                    }
                }
            } else if pair.code == 0 && pair.value_string == "EOF" {
                break;
            }
        }
        
        Ok(document)
    }
    
    /// Read the AutoCAD version from the header
    fn read_version(&mut self) -> Result<()> {
        // Find HEADER section
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "SECTION" {
                if let Some(section_pair) = self.reader.read_pair()? {
                    if section_pair.code == 2 && section_pair.value_string == "HEADER" {
                        // Look for $ACADVER
                        while let Some(header_pair) = self.reader.read_pair()? {
                            if header_pair.code == 0 && header_pair.value_string == "ENDSEC" {
                                break;
                            }
                            if header_pair.code == 9 && header_pair.value_string == "$ACADVER" {
                                if let Some(version_pair) = self.reader.read_pair()? {
                                    if version_pair.code == 1 {
                                        self.version = DxfVersion::from_version_string(&version_pair.value_string);
                                        return Ok(());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // If version not found, use Unknown
        self.version = DxfVersion::Unknown;
        Ok(())
    }
    
    /// Read the HEADER section
    fn read_header_section(&mut self, document: &mut CadDocument) -> Result<()> {
        let mut section_reader = SectionReader::new(&mut self.reader);
        section_reader.read_header(document)
    }

    /// Read the CLASSES section
    fn read_classes_section(&mut self, document: &mut CadDocument) -> Result<()> {
        let mut section_reader = SectionReader::new(&mut self.reader);
        section_reader.read_classes(document)
    }

    /// Read the TABLES section
    fn read_tables_section(&mut self, document: &mut CadDocument) -> Result<()> {
        let mut section_reader = SectionReader::new(&mut self.reader);
        section_reader.read_tables(document)
    }

    /// Read the BLOCKS section
    fn read_blocks_section(&mut self, document: &mut CadDocument) -> Result<()> {
        let mut section_reader = SectionReader::new(&mut self.reader);
        section_reader.read_blocks(document)
    }

    /// Read the ENTITIES section
    fn read_entities_section(&mut self, document: &mut CadDocument) -> Result<()> {
        let mut section_reader = SectionReader::new(&mut self.reader);
        section_reader.read_entities(document)
    }

    /// Read the OBJECTS section
    fn read_objects_section(&mut self, document: &mut CadDocument) -> Result<()> {
        let mut section_reader = SectionReader::new(&mut self.reader);
        section_reader.read_objects(document)
    }
    
    /// Skip the current section
    fn skip_section(&mut self) -> Result<()> {
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDSEC" {
                break;
            }
        }
        Ok(())
    }
}


