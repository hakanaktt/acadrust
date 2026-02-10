//! CAD document summary information.
//!
//! Holds general metadata about a CAD document, such as title, author,
//! timestamps, and custom properties.

use std::collections::HashMap;

/// Summary metadata for a CAD document.
///
/// Corresponds to the `AcDb:SummaryInfo` section in DWG files and the
/// `HEADER` metadata fields in DXF files.
#[derive(Debug, Clone)]
pub struct CadSummaryInfo {
    /// Title of the document.
    pub title: String,
    /// A short description or subject for the document.
    pub subject: String,
    /// Name of the person or organization that created the document.
    pub author: String,
    /// Keywords to help categorize or search for the document.
    pub keywords: String,
    /// Any notes or comments about the document.
    pub comments: String,
    /// Name of the last person who saved the document.
    pub last_saved_by: String,
    /// Revision number, useful for tracking changes or versions.
    pub revision_number: String,
    /// Base URL for hyperlinks in the document.
    pub hyperlink_base: String,
    /// When the document was first created (Julian date).
    pub created_date: f64,
    /// When the document was last modified (Julian date).
    pub modified_date: f64,
    /// Custom properties defined by the user or application.
    pub properties: HashMap<String, String>,
}

impl Default for CadSummaryInfo {
    fn default() -> Self {
        Self {
            title: String::new(),
            subject: String::new(),
            author: String::new(),
            keywords: String::new(),
            comments: String::new(),
            last_saved_by: String::new(),
            revision_number: String::new(),
            hyperlink_base: String::new(),
            created_date: 0.0,
            modified_date: 0.0,
            properties: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_summary_info_default() {
        let info = CadSummaryInfo::default();
        assert!(info.title.is_empty());
        assert!(info.properties.is_empty());
        assert_eq!(info.created_date, 0.0);
    }

    #[test]
    fn test_summary_info_properties() {
        let mut info = CadSummaryInfo::default();
        info.title = "Test Drawing".to_string();
        info.author = "John Doe".to_string();
        info.properties.insert("CustomProp".to_string(), "CustomValue".to_string());
        assert_eq!(info.title, "Test Drawing");
        assert_eq!(info.properties.len(), 1);
        assert_eq!(info.properties["CustomProp"], "CustomValue");
    }
}
