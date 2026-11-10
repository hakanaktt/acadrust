//! Table entity implementation.
//!
//! The Table entity represents a grid of cells that can contain text,
//! blocks, or formulas, with extensive styling and formatting options.

use crate::entities::{Entity, EntityCommon};
use crate::types::{BoundingBox3D, Color, Handle, LineWeight, Vector3};

use bitflags::bitflags;

// ============================================================================
// Enums
// ============================================================================

/// Cell content type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum CellType {
    /// Text content.
    #[default]
    Text = 1,
    /// Block reference content.
    Block = 2,
}

impl From<u8> for CellType {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Text,
            2 => Self::Block,
            _ => Self::Text,
        }
    }
}

/// Cell value data type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u32)]
pub enum CellValueType {
    /// Unknown type.
    #[default]
    Unknown = 0,
    /// Long integer.
    Long = 1,
    /// Double precision float.
    Double = 2,
    /// Text string.
    String = 4,
    /// Date value.
    Date = 8,
    /// 2D point.
    Point2D = 0x10,
    /// 3D point.
    Point3D = 0x20,
    /// Object handle reference.
    Handle = 0x40,
    /// Binary buffer.
    Buffer = 0x80,
    /// Result buffer.
    ResultBuffer = 0x100,
    /// General value.
    General = 0x200,
}

impl From<u32> for CellValueType {
    fn from(value: u32) -> Self {
        match value {
            1 => Self::Long,
            2 => Self::Double,
            4 => Self::String,
            8 => Self::Date,
            0x10 => Self::Point2D,
            0x20 => Self::Point3D,
            0x40 => Self::Handle,
            0x80 => Self::Buffer,
            0x100 => Self::ResultBuffer,
            0x200 => Self::General,
            _ => Self::Unknown,
        }
    }
}

/// Value unit type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u32)]
pub enum ValueUnitType {
    /// No units.
    #[default]
    NoUnits = 0,
    /// Distance units.
    Distance = 1,
    /// Angle units.
    Angle = 2,
    /// Area units.
    Area = 4,
    /// Volume units.
    Volume = 8,
    /// Currency units.
    Currency = 0x10,
    /// Percentage.
    Percentage = 0x20,
}

impl From<u32> for ValueUnitType {
    fn from(value: u32) -> Self {
        match value {
            1 => Self::Distance,
            2 => Self::Angle,
            4 => Self::Area,
            8 => Self::Volume,
            0x10 => Self::Currency,
            0x20 => Self::Percentage,
            _ => Self::NoUnits,
        }
    }
}

/// Border type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(i16)]
pub enum BorderType {
    /// Single line border.
    #[default]
    Single = 1,
    /// Double line border.
    Double = 2,
}

impl From<i16> for BorderType {
    fn from(value: i16) -> Self {
        match value {
            1 => Self::Single,
            2 => Self::Double,
            _ => Self::Single,
        }
    }
}

/// Cell content type for table content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum TableCellContentType {
    /// Unknown content.
    #[default]
    Unknown = 0,
    /// Value content.
    Value = 1,
    /// Field content.
    Field = 2,
    /// Block content.
    Block = 4,
}

impl From<u8> for TableCellContentType {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Value,
            2 => Self::Field,
            4 => Self::Block,
            _ => Self::Unknown,
        }
    }
}

/// Cell style type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum CellStyleType {
    /// Cell style.
    #[default]
    Cell = 1,
    /// Row style.
    Row = 2,
    /// Column style.
    Column = 3,
    /// Formatted table data style.
    FormattedTableData = 4,
    /// Table style.
    Table = 5,
}

impl From<u8> for CellStyleType {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Cell,
            2 => Self::Row,
            3 => Self::Column,
            4 => Self::FormattedTableData,
            5 => Self::Table,
            _ => Self::Cell,
        }
    }
}

/// Break flow direction for table breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum BreakFlowDirection {
    /// Break to the right.
    #[default]
    Right = 1,
    /// Break vertically.
    Vertical = 2,
    /// Break to the left.
    Left = 4,
}

impl From<u8> for BreakFlowDirection {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Right,
            2 => Self::Vertical,
            4 => Self::Left,
            _ => Self::Right,
        }
    }
}

// ============================================================================
// Bitflags
// ============================================================================

bitflags! {
    /// Cell edge flags indicating which borders to affect.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct CellEdgeFlags: u32 {
        /// No edges.
        const NONE = 0;
        /// Top edge.
        const TOP = 1;
        /// Right edge.
        const RIGHT = 2;
        /// Bottom edge.
        const BOTTOM = 4;
        /// Left edge.
        const LEFT = 8;
        /// Inside vertical edges.
        const INSIDE_VERTICAL = 16;
        /// Inside horizontal edges.
        const INSIDE_HORIZONTAL = 32;
        /// All edges.
        const ALL = 63;
    }
}

bitflags! {
    /// Cell state flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct CellStateFlags: u32 {
        /// No state.
        const NONE = 0;
        /// Content is locked.
        const CONTENT_LOCKED = 1;
        /// Content is read-only.
        const CONTENT_READ_ONLY = 2;
        /// Cell is linked.
        const LINKED = 4;
        /// Content modified after update.
        const CONTENT_MODIFIED_AFTER_UPDATE = 8;
        /// Format is locked.
        const FORMAT_LOCKED = 16;
        /// Format is read-only.
        const FORMAT_READ_ONLY = 32;
        /// Format modified after update.
        const FORMAT_MODIFIED_AFTER_UPDATE = 64;
    }
}

bitflags! {
    /// Cell style property override flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct CellStylePropertyFlags: u32 {
        /// No properties.
        const NONE = 0;
        /// Data type.
        const DATA_TYPE = 1;
        /// Data format.
        const DATA_FORMAT = 2;
        /// Rotation.
        const ROTATION = 4;
        /// Block scale.
        const BLOCK_SCALE = 8;
        /// Alignment.
        const ALIGNMENT = 16;
        /// Content color.
        const CONTENT_COLOR = 32;
        /// Text style.
        const TEXT_STYLE = 64;
        /// Text height.
        const TEXT_HEIGHT = 128;
        /// Auto scale.
        const AUTO_SCALE = 256;
        /// Background color.
        const BACKGROUND_COLOR = 512;
        /// Left margin.
        const MARGIN_LEFT = 1024;
        /// Top margin.
        const MARGIN_TOP = 2048;
        /// Right margin.
        const MARGIN_RIGHT = 4096;
        /// Bottom margin.
        const MARGIN_BOTTOM = 8192;
        /// Content layout.
        const CONTENT_LAYOUT = 16384;
        /// Merge all.
        const MERGE_ALL = 32768;
        /// Flow direction bottom to top.
        const FLOW_DIRECTION_BOTTOM_TO_TOP = 65536;
        /// Horizontal spacing.
        const MARGIN_HORIZONTAL_SPACING = 131072;
        /// Vertical spacing.
        const MARGIN_VERTICAL_SPACING = 262144;
    }
}

bitflags! {
    /// Border property override flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct BorderPropertyFlags: u32 {
        /// No properties.
        const NONE = 0;
        /// Border type.
        const BORDER_TYPE = 1;
        /// Line weight.
        const LINE_WEIGHT = 2;
        /// Line type.
        const LINE_TYPE = 4;
        /// Color.
        const COLOR = 8;
        /// Invisibility.
        const INVISIBILITY = 16;
        /// Double line spacing.
        const DOUBLE_LINE_SPACING = 32;
        /// All properties.
        const ALL = 63;
    }
}

bitflags! {
    /// Content layout flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct ContentLayoutFlags: u32 {
        /// No layout.
        const NONE = 0;
        /// Flow layout.
        const FLOW = 1;
        /// Stacked horizontal.
        const STACKED_HORIZONTAL = 2;
        /// Stacked vertical.
        const STACKED_VERTICAL = 4;
    }
}

bitflags! {
    /// Break option flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct BreakOptionFlags: u32 {
        /// No options.
        const NONE = 0;
        /// Enable breaks.
        const ENABLE_BREAKS = 1;
        /// Repeat top labels.
        const REPEAT_TOP_LABELS = 2;
        /// Repeat bottom labels.
        const REPEAT_BOTTOM_LABELS = 4;
        /// Allow manual positions.
        const ALLOW_MANUAL_POSITIONS = 8;
        /// Allow manual heights.
        const ALLOW_MANUAL_HEIGHTS = 16;
    }
}

// ============================================================================
// Cell Border
// ============================================================================

/// Border definition for a cell edge.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CellBorder {
    /// Border type.
    pub border_type: BorderType,
    /// Border color.
    pub color: Color,
    /// Border line weight.
    pub line_weight: LineWeight,
    /// Line type object used for the border.
    pub line_type_handle: Option<Handle>,
    /// Whether the border is invisible.
    pub invisible: bool,
    /// Double line spacing.
    pub double_spacing: f64,
    /// Override flags.
    pub override_flags: BorderPropertyFlags,
}

impl CellBorder {
    /// Creates a default cell border.
    pub fn new() -> Self {
        Self {
            border_type: BorderType::Single,
            color: Color::ByBlock,
            line_weight: LineWeight::ByLayer,
            line_type_handle: None,
            invisible: false,
            double_spacing: 0.0,
            override_flags: BorderPropertyFlags::NONE,
        }
    }

    /// Creates a visible border with the given color.
    pub fn with_color(color: Color) -> Self {
        let mut border = Self::new();
        border.color = color;
        border.override_flags |= BorderPropertyFlags::COLOR;
        border
    }

    /// Creates an invisible border.
    pub fn invisible() -> Self {
        let mut border = Self::new();
        border.invisible = true;
        border
    }
}

impl Default for CellBorder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Cell Value
// ============================================================================

/// Value stored in a table cell.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CellValue {
    /// Value type.
    pub value_type: CellValueType,
    /// Exact DWG/DXF type code. Preserves vendor-defined values that map to
    /// [`CellValueType::Unknown`].
    #[cfg_attr(feature = "serde", serde(default))]
    pub raw_type_code: i32,
    /// Unit type.
    pub unit_type: ValueUnitType,
    /// Exact DWG/DXF unit code, including code 12 whose value-string is absent.
    #[cfg_attr(feature = "serde", serde(default))]
    pub raw_unit_type_code: i32,
    /// Value flags.
    pub flags: i32,
    /// Stored size field used by Date, Point and General values.
    #[cfg_attr(feature = "serde", serde(default))]
    pub data_size: i32,
    /// Text string value.
    pub text: String,
    /// Format string.
    pub format: String,
    /// Formatted display value.
    pub formatted_value: String,
    /// Numeric value (for Long/Double types).
    pub numeric_value: f64,
    /// Handle value (for Handle type).
    pub handle_value: Option<Handle>,
    /// Point value for Point2D/Point3D cell values.
    pub point_value: Vector3,
    /// Date/buffer/result-buffer payload.
    pub binary_value: Vec<u8>,
}

impl CellValue {
    /// Creates an empty cell value.
    pub fn new() -> Self {
        Self {
            value_type: CellValueType::Unknown,
            raw_type_code: 0,
            unit_type: ValueUnitType::NoUnits,
            raw_unit_type_code: 0,
            flags: 0,
            data_size: 0,
            text: String::new(),
            format: String::new(),
            formatted_value: String::new(),
            numeric_value: 0.0,
            handle_value: None,
            point_value: Vector3::ZERO,
            binary_value: Vec::new(),
        }
    }

    /// Creates a text value.
    pub fn text(s: &str) -> Self {
        Self {
            value_type: CellValueType::String,
            raw_type_code: CellValueType::String as i32,
            unit_type: ValueUnitType::NoUnits,
            raw_unit_type_code: 0,
            flags: 0,
            data_size: 0,
            text: s.to_string(),
            format: String::new(),
            formatted_value: s.to_string(),
            numeric_value: 0.0,
            handle_value: None,
            point_value: Vector3::ZERO,
            binary_value: Vec::new(),
        }
    }

    /// Creates a numeric value.
    pub fn number(n: f64) -> Self {
        Self {
            value_type: CellValueType::Double,
            raw_type_code: CellValueType::Double as i32,
            unit_type: ValueUnitType::NoUnits,
            raw_unit_type_code: 0,
            flags: 0,
            data_size: 0,
            text: String::new(),
            format: String::new(),
            formatted_value: n.to_string(),
            numeric_value: n,
            handle_value: None,
            point_value: Vector3::ZERO,
            binary_value: Vec::new(),
        }
    }

    /// Creates an integer value.
    pub fn integer(n: i64) -> Self {
        Self {
            value_type: CellValueType::Long,
            raw_type_code: CellValueType::Long as i32,
            unit_type: ValueUnitType::NoUnits,
            raw_unit_type_code: 0,
            flags: 0,
            data_size: 0,
            text: String::new(),
            format: String::new(),
            formatted_value: n.to_string(),
            numeric_value: n as f64,
            handle_value: None,
            point_value: Vector3::ZERO,
            binary_value: Vec::new(),
        }
    }

    /// Returns true if the value is empty.
    pub fn is_empty(&self) -> bool {
        self.value_type == CellValueType::Unknown && self.text.is_empty()
    }

    /// Returns the display string.
    pub fn display(&self) -> &str {
        if !self.formatted_value.is_empty() {
            &self.formatted_value
        } else if !self.text.is_empty() {
            &self.text
        } else {
            ""
        }
    }

    /// Type code to serialize after accounting for API-side type changes.
    pub fn type_code(&self) -> i32 {
        if CellValueType::from(self.raw_type_code as u32) == self.value_type {
            self.raw_type_code
        } else {
            self.value_type as i32
        }
    }

    /// Unit code to serialize after accounting for API-side unit changes.
    pub fn unit_type_code(&self) -> i32 {
        if ValueUnitType::from(self.raw_unit_type_code as u32) == self.unit_type {
            self.raw_unit_type_code
        } else {
            self.unit_type as i32
        }
    }
}

impl Default for CellValue {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Cell Content
// ============================================================================

/// Content within a table cell.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CellContent {
    /// Content type.
    pub content_type: TableCellContentType,
    /// Cell value.
    pub value: CellValue,
    /// Block content handle (for block type).
    pub block_handle: Option<Handle>,
    /// Text style handle.
    pub text_style_handle: Option<Handle>,
    /// Text style name used by legacy DXF table cells.
    #[cfg_attr(feature = "serde", serde(default))]
    pub text_style_name: String,
    /// Content color.
    pub color: Color,
    /// Rotation angle in radians.
    pub rotation: f64,
    /// Scale factor (for blocks).
    pub scale: f64,
    /// Text height.
    pub text_height: f64,
    /// Referenced FIELD object for field content.
    pub field_handle: Option<Handle>,
    /// Block attribute values attached to block content.
    pub attributes: Vec<TableAttribute>,
    /// Content-format override flags.
    pub format_override_flags: i32,
    pub format_property_flags: i32,
    pub format_value_data_type: i32,
    pub format_value_unit_type: i32,
    pub value_format: String,
    pub alignment: i32,
    /// Cached content geometry.
    pub geometry: Option<CellContentGeometry>,
}

impl CellContent {
    pub fn sets(&self, property: CellStylePropertyFlags) -> bool {
        let bits = (self.format_property_flags | self.format_override_flags) as u32;
        bits & property.bits() == property.bits()
    }

    /// Creates empty cell content.
    pub fn new() -> Self {
        Self {
            content_type: TableCellContentType::Unknown,
            value: CellValue::new(),
            block_handle: None,
            text_style_handle: None,
            text_style_name: String::new(),
            color: Color::ByBlock,
            rotation: 0.0,
            scale: 1.0,
            text_height: 0.18,
            field_handle: None,
            attributes: Vec::new(),
            format_override_flags: 0,
            format_property_flags: 0,
            format_value_data_type: 0,
            format_value_unit_type: 0,
            value_format: String::new(),
            alignment: 0,
            geometry: None,
        }
    }

    /// Creates text content.
    pub fn text(s: &str) -> Self {
        Self {
            content_type: TableCellContentType::Value,
            value: CellValue::text(s),
            block_handle: None,
            text_style_handle: None,
            text_style_name: String::new(),
            color: Color::ByBlock,
            rotation: 0.0,
            scale: 1.0,
            text_height: 0.18,
            field_handle: None,
            attributes: Vec::new(),
            format_override_flags: 0,
            format_property_flags: 0,
            format_value_data_type: 0,
            format_value_unit_type: 0,
            value_format: String::new(),
            alignment: 0,
            geometry: None,
        }
    }

    /// Creates block content.
    pub fn block(block_handle: Handle) -> Self {
        Self {
            content_type: TableCellContentType::Block,
            value: CellValue::new(),
            block_handle: Some(block_handle),
            text_style_handle: None,
            text_style_name: String::new(),
            color: Color::ByBlock,
            rotation: 0.0,
            scale: 1.0,
            text_height: 0.18,
            field_handle: None,
            attributes: Vec::new(),
            format_override_flags: 0,
            format_property_flags: 0,
            format_value_data_type: 0,
            format_value_unit_type: 0,
            value_format: String::new(),
            alignment: 0,
            geometry: None,
        }
    }
}

impl Default for CellContent {
    fn default() -> Self {
        Self::new()
    }
}

/// Attribute value attached to block content in a table cell.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableAttribute {
    pub definition_handle: Handle,
    pub value: String,
    pub index: i32,
}

/// Cached table-cell content geometry.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CellContentGeometry {
    pub distance_to_top_left: Vector3,
    pub distance_to_center: Vector3,
    pub width: f64,
    pub height: f64,
    pub outer_width: f64,
    pub outer_height: f64,
    pub flags: i32,
}

/// Named custom data value used by tables, rows, columns and cells.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableCustomData {
    pub name: String,
    pub value: CellValue,
}

// ============================================================================
// Cell Style
// ============================================================================

/// Style applied to a table cell.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CellStyle {
    /// Style type.
    pub style_type: CellStyleType,
    /// Property override flags.
    pub property_flags: CellStylePropertyFlags,
    /// Complete style override flag word.
    pub override_flags: i32,
    #[cfg_attr(feature = "serde", serde(default))]
    pub legacy_override_bits: bool,
    /// Nested content-format metadata.
    pub content_format_override_flags: i32,
    pub content_property_flags: i32,
    pub value_data_type: i32,
    pub value_unit_type: i32,
    pub value_format: String,
    /// Background fill color.
    pub background_color: Color,
    /// Content color.
    pub content_color: Color,
    /// Text style handle.
    pub text_style_handle: Option<Handle>,
    /// Text style name used by legacy DXF table cells.
    #[cfg_attr(feature = "serde", serde(default))]
    pub text_style_name: String,
    /// Text height.
    pub text_height: f64,
    /// Rotation angle.
    pub rotation: f64,
    /// Scale factor.
    pub scale: f64,
    /// Alignment value.
    pub alignment: i32,
    /// Background fill enabled.
    pub fill_enabled: bool,
    /// Content layout flags.
    pub layout_flags: ContentLayoutFlags,
    /// Exact margin-override flag word stored by DWG.
    #[cfg_attr(feature = "serde", serde(default))]
    pub margin_override_flags: i16,
    /// Margins.
    pub margin_left: f64,
    pub margin_top: f64,
    pub margin_right: f64,
    pub margin_bottom: f64,
    /// Spacing.
    pub horizontal_spacing: f64,
    pub vertical_spacing: f64,
    /// Borders.
    pub top_border: CellBorder,
    pub right_border: CellBorder,
    pub bottom_border: CellBorder,
    pub left_border: CellBorder,
    /// Border edge records actually present in the DWG cell style.
    #[cfg_attr(feature = "serde", serde(default))]
    pub applied_border_edges: CellEdgeFlags,
    /// Borders not covered by the four outer edges (inside horizontal/vertical
    /// and vendor-defined edge masks).
    pub additional_borders: Vec<(u32, CellBorder)>,
}

impl CellStyle {
    /// Legacy override bits (pre-R2010 DWG cell, DXF cell group 91) for the
    /// overrides set through the R2010+ property flags, as the readers and
    /// editors set them.
    pub fn property_override_bits(&self) -> i32 {
        let mut flags = 0;
        for (property, bits) in [
            (CellStylePropertyFlags::ALIGNMENT, 0x01),
            (CellStylePropertyFlags::BACKGROUND_COLOR, 0x02 | 0x04),
            (CellStylePropertyFlags::CONTENT_COLOR, 0x08),
            (CellStylePropertyFlags::TEXT_STYLE, 0x10),
            (CellStylePropertyFlags::TEXT_HEIGHT, 0x20),
        ] {
            if self.property_flags.contains(property) {
                flags |= bits;
            }
        }
        flags
    }

    /// The stored legacy override bits plus `property_override_bits`.
    pub fn legacy_override_flags(&self) -> i32 {
        self.override_flags | self.property_override_bits()
    }

    pub fn overridden(&self) -> CellStylePropertyFlags {
        if self.legacy_override_bits {
            self.property_flags
        } else {
            self.property_flags
                | CellStylePropertyFlags::from_bits_truncate(self.override_flags as u32)
        }
    }

    pub fn sets(&self, property: CellStylePropertyFlags) -> bool {
        self.overridden().contains(property)
    }

    /// The margin flag word to store: the stored word, with bit 0 (the six
    /// margin values follow) set whenever the style overrides a margin or a
    /// content spacing, so an override made through the property flags is
    /// written out instead of dropped.
    pub fn margin_flags_to_write(&self) -> i16 {
        use CellStylePropertyFlags as P;
        let margins = P::MARGIN_LEFT
            | P::MARGIN_TOP
            | P::MARGIN_RIGHT
            | P::MARGIN_BOTTOM
            | P::MARGIN_HORIZONTAL_SPACING
            | P::MARGIN_VERTICAL_SPACING;
        if self.property_flags.intersects(margins) {
            self.margin_override_flags | 1
        } else {
            self.margin_override_flags
        }
    }

    /// Creates a default cell style.
    pub fn new() -> Self {
        Self {
            style_type: CellStyleType::Cell,
            property_flags: CellStylePropertyFlags::NONE,
            override_flags: 0,
            legacy_override_bits: false,
            content_format_override_flags: 0,
            content_property_flags: 0,
            value_data_type: 0,
            value_unit_type: 0,
            value_format: String::new(),
            background_color: Color::ByBlock,
            content_color: Color::ByBlock,
            text_style_handle: None,
            text_style_name: String::new(),
            text_height: 0.18,
            rotation: 0.0,
            scale: 1.0,
            alignment: 0,
            fill_enabled: false,
            layout_flags: ContentLayoutFlags::FLOW,
            margin_override_flags: 0,
            margin_left: 0.06,
            margin_top: 0.06,
            margin_right: 0.06,
            margin_bottom: 0.06,
            horizontal_spacing: 0.0,
            vertical_spacing: 0.0,
            top_border: CellBorder::new(),
            right_border: CellBorder::new(),
            bottom_border: CellBorder::new(),
            left_border: CellBorder::new(),
            applied_border_edges: CellEdgeFlags::NONE,
            additional_borders: Vec::new(),
        }
    }

    /// Sets uniform margins.
    pub fn set_margins(&mut self, margin: f64) {
        self.margin_left = margin;
        self.margin_top = margin;
        self.margin_right = margin;
        self.margin_bottom = margin;
    }

    /// Sets all borders to the same style.
    pub fn set_border_color(&mut self, color: Color) {
        self.top_border.color = color;
        self.right_border.color = color;
        self.bottom_border.color = color;
        self.left_border.color = color;
    }
}

impl Default for CellStyle {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Table Cell
// ============================================================================

#[cfg(feature = "serde")]
fn default_cell_geometry_scale() -> f64 {
    1.0
}

/// A cell in a table.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableCell {
    /// Cell type (text or block).
    pub cell_type: CellType,
    /// Cell state flags.
    pub state: CellStateFlags,
    /// Cell contents (can have multiple).
    pub contents: Vec<CellContent>,
    /// Cell style override.
    pub style: Option<CellStyle>,
    /// Cell tooltip.
    pub tooltip: String,
    /// Rotation angle.
    pub rotation: f64,
    /// R2010+ cached cell-geometry scale.
    #[cfg_attr(feature = "serde", serde(default = "default_cell_geometry_scale"))]
    pub geometry_scale: f64,
    /// Auto-fit content.
    pub auto_fit: bool,
    /// Merged cell width (number of columns).
    pub merge_width: i32,
    /// Merged cell height (number of rows).
    pub merge_height: i32,
    /// Flag value.
    pub flag: i32,
    /// Merged value (encoding of merge info).
    pub merged: i32,
    /// Virtual edge flag.
    pub virtual_edge: i16,
    /// Has linked data.
    pub has_linked_data: bool,
    /// Custom data value.
    pub custom_data: i32,
    /// Pre-R2010 edge flag byte.
    pub edge_flags: u8,
    /// Pre-R2010 text/block value handle.
    pub value_handle: Option<Handle>,
    /// Pre-R2010 block scale.
    pub block_scale: f64,
    /// Named custom data entries.
    pub custom_data_items: Vec<TableCustomData>,
    /// Linked data object and its stored range dimensions.
    pub data_link_handle: Option<Handle>,
    pub data_link_rows: i32,
    pub data_link_columns: i32,
    pub data_link_unknown: i32,
    /// Cell style identifier.
    pub style_id: i32,
    /// Cached geometry metadata.
    #[cfg_attr(feature = "serde", serde(default))]
    pub geometry_data_flag: i32,
    #[cfg_attr(feature = "serde", serde(default))]
    pub geometry_width_with_gap: f64,
    #[cfg_attr(feature = "serde", serde(default))]
    pub geometry_height_with_gap: f64,
    pub geometry_flags: i32,
    pub geometry_handle: Option<Handle>,
    pub geometry: Option<CellContentGeometry>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub geometries: Vec<CellContentGeometry>,
}

impl TableCell {
    /// Creates an empty cell.
    pub fn new() -> Self {
        Self {
            cell_type: CellType::Text,
            state: CellStateFlags::NONE,
            contents: Vec::new(),
            style: None,
            tooltip: String::new(),
            rotation: 0.0,
            geometry_scale: 1.0,
            auto_fit: false,
            merge_width: 1,
            merge_height: 1,
            flag: 0,
            merged: 0,
            virtual_edge: 0,
            has_linked_data: false,
            custom_data: 0,
            edge_flags: 0,
            value_handle: None,
            block_scale: 1.0,
            custom_data_items: Vec::new(),
            data_link_handle: None,
            data_link_rows: 0,
            data_link_columns: 0,
            data_link_unknown: 0,
            style_id: 0,
            geometry_data_flag: 0,
            geometry_width_with_gap: 0.0,
            geometry_height_with_gap: 0.0,
            geometry_flags: 0,
            geometry_handle: None,
            geometry: None,
            geometries: Vec::new(),
        }
    }

    /// Creates a cell with text content.
    pub fn text(s: &str) -> Self {
        let mut cell = Self::new();
        cell.contents.push(CellContent::text(s));
        cell
    }

    /// Creates a cell with block content.
    pub fn block(block_handle: Handle) -> Self {
        let mut cell = Self::new();
        cell.cell_type = CellType::Block;
        cell.contents.push(CellContent::block(block_handle));
        cell
    }

    /// Gets the first content (convenience method).
    pub fn content(&self) -> Option<&CellContent> {
        self.contents.first()
    }

    /// Gets the text value of the first content.
    pub fn text_value(&self) -> &str {
        self.contents
            .first()
            .map(|c| c.value.display())
            .unwrap_or("")
    }

    pub fn binary_layout(&self) -> std::borrow::Cow<'_, TableCell> {
        use CellStylePropertyFlags as P;
        const CONTENT: P = P::TEXT_HEIGHT.union(P::TEXT_STYLE).union(P::CONTENT_COLOR);
        let legacy = self.style.as_ref().is_some_and(|s| s.legacy_override_bits);
        if !legacy {
            return std::borrow::Cow::Borrowed(self);
        }
        let own = self.style.clone();
        let untyped = self.contents.iter().any(|c| c.format_value_data_type == 0);
        if own.is_none() && !untyped {
            return std::borrow::Cow::Borrowed(self);
        }
        let mut cell = self.clone();
        for content in &mut cell.contents {
            if content.format_value_data_type == 0 {
                if content.value.text.is_empty() && content.field_handle.is_none() {
                    content.format_value_data_type = 512;
                } else {
                    content.format_value_data_type = 4;
                    content.format_override_flags |= 3;
                }
            }
        }
        if let Some(own) = own {
            let p = own.overridden();
            for content in &mut cell.contents {
                if p.contains(P::TEXT_HEIGHT) {
                    content.format_override_flags |= P::TEXT_HEIGHT.bits() as i32;
                    content.text_height = own.text_height;
                }
                if p.contains(P::TEXT_STYLE) {
                    content.format_override_flags |= P::TEXT_STYLE.bits() as i32;
                    content.text_style_handle = own.text_style_handle;
                    content.text_style_name = own.text_style_name.clone();
                }
                if p.contains(P::CONTENT_COLOR) {
                    content.format_override_flags |= P::CONTENT_COLOR.bits() as i32;
                    content.color = own.content_color;
                }
            }
            if let Some(style) = cell.style.as_mut() {
                if p.contains(P::BACKGROUND_COLOR) && !style.fill_enabled {
                    style.background_color = Color::ByBlock;
                }
                // Each edge's colour, line weight and visibility have their
                // own legacy bits (0x40, 0x400, 0x4000, shifted by the edge).
                for (i, (edge, border)) in [
                    (CellEdgeFlags::TOP, &mut style.top_border),
                    (CellEdgeFlags::RIGHT, &mut style.right_border),
                    (CellEdgeFlags::BOTTOM, &mut style.bottom_border),
                    (CellEdgeFlags::LEFT, &mut style.left_border),
                ]
                .into_iter()
                .enumerate()
                {
                    for (bit, property) in [
                        (0x40 << i, BorderPropertyFlags::COLOR),
                        (0x400 << i, BorderPropertyFlags::LINE_WEIGHT),
                        (0x4000 << i, BorderPropertyFlags::INVISIBILITY),
                    ] {
                        if own.override_flags & bit != 0 {
                            border.override_flags |= property;
                            style.applied_border_edges |= edge;
                        }
                    }
                }
                style.override_flags = (p & !CONTENT).bits() as i32;
                style.property_flags = P::NONE;
                style.legacy_override_bits = false;
            }
        }
        std::borrow::Cow::Owned(cell)
    }

    pub fn set_text(&mut self, s: &str) {
        // Typed text keeps the content's format and is stored as a string
        // value with the flags the reference writes for typed text (6).
        let mut content = self
            .contents
            .first()
            .filter(|c| c.content_type == TableCellContentType::Value)
            .cloned()
            .unwrap_or_else(|| CellContent::text(s));
        content.value = CellValue::text(s);
        content.value.flags = 6;
        content.field_handle = None;
        self.contents.clear();
        self.contents.push(content);
        self.cell_type = CellType::Text;
    }

    /// Returns true if this cell spans multiple cells.
    pub fn is_merged(&self) -> bool {
        self.merge_width > 1 || self.merge_height > 1
    }

    /// Returns true if the cell has content.
    pub fn has_content(&self) -> bool {
        !self.contents.is_empty()
    }

    /// Returns true if this cell has multiple contents.
    pub fn has_multiple_contents(&self) -> bool {
        self.contents.len() > 1
    }
}

impl Default for TableCell {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Table Row
// ============================================================================

/// A row in a table.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableRow {
    /// Row height.
    pub height: f64,
    /// Cells in this row.
    pub cells: Vec<TableCell>,
    /// Row style override.
    pub style: Option<CellStyle>,
    /// Custom data value.
    pub custom_data: i32,
    pub custom_data_items: Vec<TableCustomData>,
    pub style_id: i32,
}

impl TableRow {
    /// Creates a new row with the given number of cells.
    pub fn new(num_cells: usize) -> Self {
        let cells = (0..num_cells).map(|_| TableCell::new()).collect();
        Self {
            height: 0.25,
            cells,
            style: None,
            custom_data: 0,
            custom_data_items: Vec::new(),
            style_id: 0,
        }
    }

    /// Creates a row from cell values.
    pub fn from_texts(texts: &[&str]) -> Self {
        let cells = texts.iter().map(|t| TableCell::text(t)).collect();
        Self {
            height: 0.25,
            cells,
            style: None,
            custom_data: 0,
            custom_data_items: Vec::new(),
            style_id: 0,
        }
    }

    /// Returns the number of cells.
    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    /// Gets a cell by index.
    pub fn cell(&self, index: usize) -> Option<&TableCell> {
        self.cells.get(index)
    }

    /// Gets a mutable cell by index.
    pub fn cell_mut(&mut self, index: usize) -> Option<&mut TableCell> {
        self.cells.get_mut(index)
    }
}

impl Default for TableRow {
    fn default() -> Self {
        Self::new(0)
    }
}

// ============================================================================
// Table Column
// ============================================================================

/// A column definition in a table.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableColumn {
    /// Column name.
    pub name: String,
    /// Column width.
    pub width: f64,
    /// Column style override.
    pub style: Option<CellStyle>,
    /// Custom data value.
    pub custom_data: i32,
    pub custom_data_items: Vec<TableCustomData>,
    pub style_id: i32,
}

impl TableColumn {
    /// Creates a new column with default width.
    pub fn new() -> Self {
        Self {
            name: String::new(),
            width: 2.5,
            style: None,
            custom_data: 0,
            custom_data_items: Vec::new(),
            style_id: 0,
        }
    }

    /// Creates a column with the given width.
    pub fn with_width(width: f64) -> Self {
        Self {
            name: String::new(),
            width,
            style: None,
            custom_data: 0,
            custom_data_items: Vec::new(),
            style_id: 0,
        }
    }

    /// Creates a named column.
    pub fn named(name: &str, width: f64) -> Self {
        Self {
            name: name.to_string(),
            width,
            style: None,
            custom_data: 0,
            custom_data_items: Vec::new(),
            style_id: 0,
        }
    }
}

impl Default for TableColumn {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Cell Range
// ============================================================================

/// A range of cells in a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CellRange {
    /// Top row index.
    pub top_row: usize,
    /// Left column index.
    pub left_col: usize,
    /// Bottom row index (inclusive).
    pub bottom_row: usize,
    /// Right column index (inclusive).
    pub right_col: usize,
}

impl CellRange {
    /// Creates a range for a single cell.
    pub fn cell(row: usize, col: usize) -> Self {
        Self {
            top_row: row,
            left_col: col,
            bottom_row: row,
            right_col: col,
        }
    }

    /// Creates a range of cells.
    pub fn new(top_row: usize, left_col: usize, bottom_row: usize, right_col: usize) -> Self {
        Self {
            top_row,
            left_col,
            bottom_row,
            right_col,
        }
    }

    /// Returns the number of rows in the range.
    pub fn row_count(&self) -> usize {
        self.bottom_row - self.top_row + 1
    }

    /// Returns the number of columns in the range.
    pub fn col_count(&self) -> usize {
        self.right_col - self.left_col + 1
    }

    /// Returns the total number of cells in the range.
    pub fn cell_count(&self) -> usize {
        self.row_count() * self.col_count()
    }

    /// Returns true if this range contains the given cell.
    pub fn contains(&self, row: usize, col: usize) -> bool {
        row >= self.top_row
            && row <= self.bottom_row
            && col >= self.left_col
            && col <= self.right_col
    }
}

impl Default for CellRange {
    fn default() -> Self {
        Self::cell(0, 0)
    }
}

// Laying merged ranges over a table's cells costs each range its area, so ranges that overlap
// again and again (a file can list millions) cost far more than the table has cells. Ranges
// that do not overlap cost at most one visit per cell and one per row they span
// (`MERGE_VISITS_PER_CELL` leaves room for some overlap) plus a constant, so a well-formed
// table never reaches the budget; only ranges past it are treated differently (see
// `Table::set_merged_ranges` and `MergedCellIndex`). A range past the budget is skipped and
// the ranges after it are still laid while they fit.
const MERGE_VISITS_PER_CELL: usize = 4;
const MERGE_VISIT_SLACK: usize = 1 << 16;

/// Cell visits allowed for laying merged ranges over `cells` cells.
fn merge_visit_budget(cells: usize) -> usize {
    cells
        .saturating_mul(MERGE_VISITS_PER_CELL)
        .saturating_add(MERGE_VISIT_SLACK)
}

/// The index of each row's first cell in a table's cells taken row after row, followed by
/// the number of cells.
fn row_starts(rows: &[TableRow]) -> Vec<usize> {
    let mut starts = Vec::with_capacity(rows.len() + 1);
    let mut cells = 0usize;
    for row in rows {
        starts.push(cells);
        cells = cells.saturating_add(row.cells.len());
    }
    starts.push(cells);
    starts
}

/// The visits a range costs, an upper bound of what a loop over it touches:
/// one per row it spans, plus its cells, counted at most as its area and at most as the cells
/// those rows hold. Rows can hold fewer cells than the table has columns, and the area alone
/// would make a range over short rows cost far more than it touches. `row_starts` comes from
/// [`row_starts`].
fn merge_visits(range: &CellRange, row_starts: &[usize]) -> usize {
    let rows = row_starts.len().saturating_sub(1);
    if range.top_row >= rows {
        return 1;
    }
    let bottom = range.bottom_row.min(rows - 1);
    let spanned = bottom.saturating_sub(range.top_row).saturating_add(1);
    let columns = range
        .right_col
        .saturating_sub(range.left_col)
        .saturating_add(1);
    let held = row_starts[bottom + 1].saturating_sub(row_starts[range.top_row]);
    spanned.saturating_add(held.min(spanned.saturating_mul(columns)))
}

/// For every cell of a table, the first of its merged ranges that contains it, which the DXF
/// writer used to find by searching every range for every cell (cells × ranges). Built once:
/// the ranges are laid over the cells in order, each claiming the cells no earlier range
/// claimed, within `merge_visit_budget`. The answers are those of the search whenever the
/// ranges fit the budget, which ranges that do not overlap always do; once overlapping ranges
/// spend it, a range that no longer fits claims no cells and the ranges after it go on
/// claiming while they fit.
pub(crate) struct MergedCellIndex {
    /// Index in `claims` of each row's first cell, then the number of cells.
    row_starts: Vec<usize>,
    /// The range claiming each cell (rows one after the other), or `UNCLAIMED`.
    claims: Vec<u32>,
}

const UNCLAIMED: u32 = u32::MAX;

impl MergedCellIndex {
    pub(crate) fn new(table: &Table, ranges: &[CellRange]) -> Self {
        let row_starts = row_starts(&table.rows);
        let cells = row_starts.last().copied().unwrap_or(0);
        let mut claims = vec![UNCLAIMED; cells];
        let mut budget = merge_visit_budget(cells);
        for (index, range) in ranges.iter().enumerate() {
            let Ok(index) = u32::try_from(index) else {
                break;
            };
            if index == UNCLAIMED {
                break;
            }
            // A range past the budget is skipped: a later range, disjoint from it, may still
            // fit.
            let visits = merge_visits(range, &row_starts);
            if visits > budget {
                continue;
            }
            budget -= visits;
            let rows = table
                .rows
                .iter()
                .zip(&row_starts)
                .take(range.bottom_row.saturating_add(1))
                .skip(range.top_row);
            for (row, start) in rows {
                let end = row.cells.len().min(range.right_col.saturating_add(1));
                for column in range.left_col..end {
                    let claim = &mut claims[start + column];
                    if *claim == UNCLAIMED {
                        *claim = index;
                    }
                }
            }
        }
        Self { row_starts, claims }
    }

    /// Index of the first range containing cell (`row`, `column`).
    pub(crate) fn first(&self, row: usize, column: usize) -> Option<usize> {
        let start = *self.row_starts.get(row)?;
        let end = self
            .row_starts
            .get(row + 1)
            .copied()
            .unwrap_or(self.claims.len());
        let slot = start.checked_add(column).filter(|slot| *slot < end)?;
        match self.claims[slot] {
            UNCLAIMED => None,
            index => Some(index as usize),
        }
    }
}

/// One manual table-break position.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableBreakData {
    pub position: Vector3,
    pub height: f64,
    pub flags: i32,
}

/// Cached range attached to a table break.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableBreakRange {
    pub position: Vector3,
    pub start_row: i32,
    pub end_row: i32,
}

/// Pre-R2010 table-wide style overrides.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LegacyTableStyleOverride {
    pub flags: i32,
    pub title_suppressed: Option<bool>,
    /// Header suppression retained from legacy DXF.
    #[cfg_attr(feature = "serde", serde(default))]
    pub header_suppressed: Option<bool>,
    pub flow_direction: Option<i16>,
    pub horizontal_cell_margin: Option<f64>,
    pub vertical_cell_margin: Option<f64>,
    pub row_colors: Vec<Color>,
    pub row_fill_none: Vec<bool>,
    pub row_fill_colors: Vec<Color>,
    pub row_alignments: Vec<i16>,
    /// Text-style names retained from legacy DXF.
    #[cfg_attr(feature = "serde", serde(default))]
    pub text_style_names: Vec<String>,
    pub text_style_handles: Vec<Handle>,
    pub row_heights: Vec<f64>,
}

/// Pre-R2010 border overrides. Values follow set-bit order from bit 0 through 17.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LegacyBorderOverrides<T> {
    pub flags: i32,
    pub values: Vec<T>,
}

impl<T> Default for LegacyBorderOverrides<T> {
    fn default() -> Self {
        Self {
            flags: 0,
            values: Vec::new(),
        }
    }
}

// ============================================================================
// Table Entity
// ============================================================================

/// Table entity.
///
/// A table is a grid of cells containing text, blocks, or formulas,
/// with extensive styling and formatting options.
///
/// # Example
///
/// ```ignore
/// use acadrust::entities::{Table, TableRow};
/// use acadrust::types::Vector3;
///
/// // Create a 3x4 table
/// let mut table = Table::new(Vector3::new(0.0, 0.0, 0.0), 3, 4);
///
/// // Set header row
/// table.set_cell_text(0, 0, "Name");
/// table.set_cell_text(0, 1, "Value");
/// table.set_cell_text(0, 2, "Unit");
/// table.set_cell_text(0, 3, "Notes");
///
/// // Add data rows
/// table.set_cell_text(1, 0, "Length");
/// table.set_cell_text(1, 1, "100.0");
/// table.set_cell_text(1, 2, "mm");
/// ```
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Table {
    /// Common entity data.
    pub common: EntityCommon,
    /// Insertion point.
    pub insertion_point: Vector3,
    /// Horizontal direction vector.
    pub horizontal_direction: Vector3,
    /// Normal vector.
    pub normal: Vector3,
    /// Table style handle.
    pub table_style_handle: Option<Handle>,
    /// Block record handle (table is based on block).
    pub block_record_handle: Option<Handle>,
    /// Anonymous block name stored by the legacy DXF INSERT base.
    #[cfg_attr(feature = "serde", serde(default))]
    pub block_name: String,
    /// Table data version.
    pub data_version: i16,
    /// Table value flags.
    pub value_flags: i32,
    /// Flag overrides.
    pub override_flag: bool,
    pub override_border_color: bool,
    pub override_border_line_weight: bool,
    pub override_border_visibility: bool,
    /// Native pre-R2010 table and border overrides.
    pub legacy_style_override: Option<LegacyTableStyleOverride>,
    pub legacy_border_colors: Option<LegacyBorderOverrides<Color>>,
    pub legacy_border_line_weights: Option<LegacyBorderOverrides<LineWeight>>,
    pub legacy_border_visibility: Option<LegacyBorderOverrides<bool>>,
    /// Rows.
    pub rows: Vec<TableRow>,
    /// Columns.
    pub columns: Vec<TableColumn>,
    /// Break options.
    pub break_options: BreakOptionFlags,
    /// Break flow direction.
    pub break_flow_direction: BreakFlowDirection,
    /// Break spacing.
    pub break_spacing: f64,
    /// Linked table-data name and description.
    pub name: String,
    pub description: String,
    /// FIELD objects referenced by the table content.
    pub field_handles: Vec<Handle>,
    /// Base style applied to the table.
    pub base_style: Option<CellStyle>,
    /// Merged cell ranges.
    pub merged_ranges: Vec<CellRange>,
    /// Manual break positions and cached break ranges.
    pub break_data: Vec<TableBreakData>,
    pub break_ranges: Vec<TableBreakRange>,
    /// Undocumented header values retained by the native DWG codec.
    pub dwg_unknown_byte: u8,
    pub dwg_unknown_handle: Option<Handle>,
    pub dwg_unknown_long1: i32,
    /// R2010-only header bit. New tables use the native default `true`.
    /// Kept separate from the R2013+ long, whose native default is zero.
    #[cfg_attr(feature = "serde", serde(default))]
    pub dwg_r2010_unknown_bit: Option<bool>,
    pub dwg_unknown_long2: i32,
    pub dwg_unknown_short: i16,
}

fn visit_table_value_handles(value: &mut CellValue, visit: &mut impl FnMut(&mut Handle)) {
    if let Some(handle) = value.handle_value.as_mut() {
        visit(handle);
    }
}

fn visit_table_border_handles(value: &mut CellBorder, visit: &mut impl FnMut(&mut Handle)) {
    if let Some(handle) = value.line_type_handle.as_mut() {
        visit(handle);
    }
}

fn visit_table_style_handles(value: &mut CellStyle, visit: &mut impl FnMut(&mut Handle)) {
    if let Some(handle) = value.text_style_handle.as_mut() {
        visit(handle);
    }
    visit_table_border_handles(&mut value.top_border, visit);
    visit_table_border_handles(&mut value.right_border, visit);
    visit_table_border_handles(&mut value.bottom_border, visit);
    visit_table_border_handles(&mut value.left_border, visit);
    for (_, border) in &mut value.additional_borders {
        visit_table_border_handles(border, visit);
    }
}

fn visit_table_custom_data_handles(
    values: &mut [TableCustomData],
    visit: &mut impl FnMut(&mut Handle),
) {
    for value in values {
        visit_table_value_handles(&mut value.value, visit);
    }
}

fn visit_table_cell_handles(value: &mut TableCell, visit: &mut impl FnMut(&mut Handle)) {
    for content in &mut value.contents {
        visit_table_value_handles(&mut content.value, visit);
        if let Some(handle) = content.block_handle.as_mut() {
            visit(handle);
        }
        if let Some(handle) = content.text_style_handle.as_mut() {
            visit(handle);
        }
        if let Some(handle) = content.field_handle.as_mut() {
            visit(handle);
        }
        for attribute in &mut content.attributes {
            visit(&mut attribute.definition_handle);
        }
    }
    if let Some(style) = value.style.as_mut() {
        visit_table_style_handles(style, visit);
    }
    if let Some(handle) = value.value_handle.as_mut() {
        visit(handle);
    }
    visit_table_custom_data_handles(&mut value.custom_data_items, visit);
    if let Some(handle) = value.data_link_handle.as_mut() {
        visit(handle);
    }
    if let Some(handle) = value.geometry_handle.as_mut() {
        visit(handle);
    }
}

impl Table {
    pub fn sync_merged_ranges_from_cells(&mut self) {
        let ranges = self.canonical_merged_ranges();
        self.merged_ranges = ranges;
    }

    pub(crate) fn sync_cells_from_merged_ranges(&mut self) {
        let ranges = self.canonical_merged_ranges();
        self.set_merged_ranges(ranges);
    }

    pub(crate) fn canonical_merged_ranges(&self) -> Vec<CellRange> {
        let row_count = self.rows.len();
        let column_count = self.columns.len();
        if row_count == 0 || column_count == 0 {
            return Vec::new();
        }

        // The kept ranges are hashed as they are kept, so finding a duplicate takes constant
        // time instead of a search of every kept range (quadratic in the number of ranges a
        // file lists: minutes for a million). Same ranges, same order.
        let key = |range: &CellRange| {
            (
                range.top_row,
                range.left_col,
                range.bottom_row,
                range.right_col,
            )
        };
        let mut seen = std::collections::HashSet::with_capacity(self.merged_ranges.len());
        let mut ranges = Vec::new();
        for range in &self.merged_ranges {
            if range.top_row >= row_count
                || range.left_col >= column_count
                || range.top_row > range.bottom_row
                || range.left_col > range.right_col
            {
                continue;
            }
            let range = CellRange {
                top_row: range.top_row,
                left_col: range.left_col,
                bottom_row: range.bottom_row.min(row_count - 1),
                right_col: range.right_col.min(column_count - 1),
            };
            if range.cell_count() > 1 && seen.insert(key(&range)) {
                ranges.push(range);
            }
        }
        for (row_index, row) in self.rows.iter().enumerate() {
            for (column_index, cell) in row.cells.iter().take(column_count).enumerate() {
                let height = cell.merge_height.max(1) as usize;
                let width = cell.merge_width.max(1) as usize;
                if height == 1 && width == 1 {
                    continue;
                }
                let range = CellRange {
                    top_row: row_index,
                    left_col: column_index,
                    bottom_row: row_index.saturating_add(height - 1).min(row_count - 1),
                    right_col: column_index.saturating_add(width - 1).min(column_count - 1),
                };
                if range.cell_count() > 1 && seen.insert(key(&range)) {
                    ranges.push(range);
                }
            }
        }
        ranges
    }

    fn set_merged_ranges(&mut self, ranges: Vec<CellRange>) {
        for row in &mut self.rows {
            for cell in &mut row.cells {
                cell.merge_width = 1;
                cell.merge_height = 1;
                cell.merged = 0;
            }
        }
        self.merged_ranges = ranges;
        // Marking the cells a range covers costs what `merge_visits` counts: its rows and the
        // cells they hold within it. Within `merge_visit_budget`, which ranges that do not
        // overlap never reach, every range marks exactly as before; past it (millions of large
        // overlapping ranges took hours) a range that no longer fits still sets its anchor but
        // leaves the cells it covers as the earlier ranges left them, and the ranges after it
        // are marked while they fit.
        let row_starts = row_starts(&self.rows);
        let mut budget = merge_visit_budget(row_starts.last().copied().unwrap_or(0));
        for range in &self.merged_ranges {
            let visits = merge_visits(range, &row_starts);
            if visits <= budget {
                budget -= visits;
                for row in self
                    .rows
                    .iter_mut()
                    .take(range.bottom_row + 1)
                    .skip(range.top_row)
                {
                    for cell in row
                        .cells
                        .iter_mut()
                        .take(range.right_col + 1)
                        .skip(range.left_col)
                    {
                        cell.merged = 1;
                    }
                }
            }
            if let Some(cell) = self
                .rows
                .get_mut(range.top_row)
                .and_then(|row| row.cells.get_mut(range.left_col))
            {
                cell.merged = 0;
                cell.merge_width = range.col_count() as i32;
                cell.merge_height = range.row_count() as i32;
            }
        }
    }

    pub(crate) fn visit_object_handles_mut(&mut self, visit: &mut impl FnMut(&mut Handle)) {
        if let Some(handle) = self.common.linetype_handle.as_mut() {
            visit(handle);
        }
        for handle in &mut self.common.reactors {
            visit(handle);
        }
        if let Some(handle) = self.common.xdictionary_handle.as_mut() {
            visit(handle);
        }
        visit(&mut self.common.owner_handle);
        if let Some(handle) = self.common.color_book_handle.as_mut() {
            visit(handle);
        }
        if let Some(handle) = self.common.full_visual_style_handle.as_mut() {
            visit(handle);
        }
        if let Some(handle) = self.common.face_visual_style_handle.as_mut() {
            visit(handle);
        }
        if let Some(handle) = self.common.edge_visual_style_handle.as_mut() {
            visit(handle);
        }
        if let Some(handle) = self.common.material_handle.as_mut() {
            visit(handle);
        }
        if let Some(handle) = self.common.plotstyle_handle.as_mut() {
            visit(handle);
        }
        if let Some(handle) = self.table_style_handle.as_mut() {
            visit(handle);
        }
        if let Some(handle) = self.block_record_handle.as_mut() {
            visit(handle);
        }
        if let Some(style) = self.legacy_style_override.as_mut() {
            for handle in &mut style.text_style_handles {
                visit(handle);
            }
        }
        for row in &mut self.rows {
            if let Some(style) = row.style.as_mut() {
                visit_table_style_handles(style, visit);
            }
            visit_table_custom_data_handles(&mut row.custom_data_items, visit);
            for cell in &mut row.cells {
                visit_table_cell_handles(cell, visit);
            }
        }
        for column in &mut self.columns {
            if let Some(style) = column.style.as_mut() {
                visit_table_style_handles(style, visit);
            }
            visit_table_custom_data_handles(&mut column.custom_data_items, visit);
        }
        for handle in &mut self.field_handles {
            visit(handle);
        }
        if let Some(style) = self.base_style.as_mut() {
            visit_table_style_handles(style, visit);
        }
        if let Some(handle) = self.dwg_unknown_handle.as_mut() {
            visit(handle);
        }
    }

    /// Creates a new table with the given dimensions.
    pub fn new(insertion_point: Vector3, num_rows: usize, num_cols: usize) -> Self {
        let rows = (0..num_rows).map(|_| TableRow::new(num_cols)).collect();
        let columns = (0..num_cols).map(|_| TableColumn::new()).collect();

        Self {
            common: EntityCommon::default(),
            insertion_point,
            horizontal_direction: Vector3::new(1.0, 0.0, 0.0),
            normal: Vector3::new(0.0, 0.0, 1.0),
            table_style_handle: None,
            block_record_handle: None,
            block_name: String::new(),
            data_version: 0,
            value_flags: 0,
            override_flag: false,
            override_border_color: false,
            override_border_line_weight: false,
            override_border_visibility: false,
            legacy_style_override: None,
            legacy_border_colors: None,
            legacy_border_line_weights: None,
            legacy_border_visibility: None,
            rows,
            columns,
            break_options: BreakOptionFlags::NONE,
            break_flow_direction: BreakFlowDirection::Right,
            break_spacing: 0.0,
            name: String::new(),
            description: String::new(),
            field_handles: Vec::new(),
            base_style: None,
            merged_ranges: Vec::new(),
            break_data: Vec::new(),
            break_ranges: Vec::new(),
            dwg_unknown_byte: 0,
            dwg_unknown_handle: None,
            dwg_unknown_long1: 0,
            dwg_r2010_unknown_bit: None,
            dwg_unknown_long2: 0,
            dwg_unknown_short: 38,
        }
    }

    /// Returns the number of rows.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Returns the number of columns.
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    /// Gets a cell at the given position.
    pub fn cell(&self, row: usize, col: usize) -> Option<&TableCell> {
        self.rows.get(row).and_then(|r| r.cells.get(col))
    }

    /// Gets a mutable cell at the given position.
    pub fn cell_mut(&mut self, row: usize, col: usize) -> Option<&mut TableCell> {
        self.rows.get_mut(row).and_then(|r| r.cells.get_mut(col))
    }

    /// Sets the text content of a cell.
    pub fn set_cell_text(&mut self, row: usize, col: usize, text: &str) -> bool {
        if let Some(cell) = self.cell_mut(row, col) {
            cell.set_text(text);
            true
        } else {
            false
        }
    }

    /// Gets the text content of a cell.
    pub fn cell_text(&self, row: usize, col: usize) -> Option<&str> {
        self.cell(row, col).map(|c| c.text_value())
    }

    /// Sets the height of a row.
    pub fn set_row_height(&mut self, row: usize, height: f64) {
        if let Some(r) = self.rows.get_mut(row) {
            r.height = height;
        }
    }

    /// Sets the width of a column.
    pub fn set_column_width(&mut self, col: usize, width: f64) {
        if let Some(c) = self.columns.get_mut(col) {
            c.width = width;
        }
    }

    /// Gets the total table width.
    pub fn total_width(&self) -> f64 {
        self.columns.iter().map(|c| c.width).sum()
    }

    /// Gets the total table height.
    pub fn total_height(&self) -> f64 {
        self.rows.iter().map(|r| r.height).sum()
    }

    /// Adds a new row at the end.
    pub fn add_row(&mut self) -> &mut TableRow {
        let num_cols = self.columns.len();
        self.rows.push(TableRow::new(num_cols));
        self.rows.last_mut().unwrap()
    }

    /// Adds a new column at the end.
    pub fn add_column(&mut self, width: f64) {
        self.columns.push(TableColumn::with_width(width));
        // Add a cell to each row
        for row in &mut self.rows {
            row.cells.push(TableCell::new());
        }
    }

    /// Inserts a row at the given index.
    pub fn insert_row(&mut self, index: usize) {
        let num_cols = self.columns.len();
        if index <= self.rows.len() {
            let ranges = self
                .canonical_merged_ranges()
                .into_iter()
                .map(|mut range| {
                    if index <= range.top_row {
                        range.top_row += 1;
                        range.bottom_row += 1;
                    } else if index <= range.bottom_row {
                        range.bottom_row += 1;
                    }
                    range
                })
                .collect();
            self.rows.insert(index, TableRow::new(num_cols));
            for range in &mut self.break_ranges {
                if index as i32 <= range.start_row {
                    range.start_row += 1;
                    range.end_row += 1;
                } else if index as i32 <= range.end_row {
                    range.end_row += 1;
                }
            }
            self.set_merged_ranges(ranges);
        }
    }

    /// Inserts a column at the given index.
    pub fn insert_column(&mut self, index: usize, width: f64) {
        if index <= self.columns.len() {
            let ranges = self
                .canonical_merged_ranges()
                .into_iter()
                .map(|mut range| {
                    if index <= range.left_col {
                        range.left_col += 1;
                        range.right_col += 1;
                    } else if index <= range.right_col {
                        range.right_col += 1;
                    }
                    range
                })
                .collect();
            self.columns.insert(index, TableColumn::with_width(width));
            for row in &mut self.rows {
                row.cells.insert(index, TableCell::new());
            }
            self.set_merged_ranges(ranges);
        }
    }

    /// Removes a row.
    pub fn remove_row(&mut self, index: usize) -> Option<TableRow> {
        if index < self.rows.len() {
            let ranges = self
                .canonical_merged_ranges()
                .into_iter()
                .filter_map(|mut range| {
                    if index < range.top_row {
                        range.top_row -= 1;
                        range.bottom_row -= 1;
                    } else if index <= range.bottom_row {
                        if range.top_row == range.bottom_row {
                            return None;
                        }
                        range.bottom_row -= 1;
                    }
                    (range.cell_count() > 1).then_some(range)
                })
                .collect();
            let removed = self.rows.remove(index);
            self.break_ranges.retain_mut(|range| {
                if (index as i32) < range.start_row {
                    range.start_row -= 1;
                    range.end_row -= 1;
                } else if index as i32 <= range.end_row {
                    if range.start_row == range.end_row {
                        return false;
                    }
                    range.end_row -= 1;
                }
                true
            });
            self.set_merged_ranges(ranges);
            Some(removed)
        } else {
            None
        }
    }

    /// Removes a column.
    pub fn remove_column(&mut self, index: usize) -> Option<TableColumn> {
        if index < self.columns.len() {
            let ranges = self
                .canonical_merged_ranges()
                .into_iter()
                .filter_map(|mut range| {
                    if index < range.left_col {
                        range.left_col -= 1;
                        range.right_col -= 1;
                    } else if index <= range.right_col {
                        if range.left_col == range.right_col {
                            return None;
                        }
                        range.right_col -= 1;
                    }
                    (range.cell_count() > 1).then_some(range)
                })
                .collect();
            let col = self.columns.remove(index);
            for row in &mut self.rows {
                if index < row.cells.len() {
                    row.cells.remove(index);
                }
            }
            self.set_merged_ranges(ranges);
            Some(col)
        } else {
            None
        }
    }

    /// Merges cells in the given range.
    pub fn merge_cells(&mut self, range: CellRange) {
        if range.top_row > range.bottom_row
            || range.left_col > range.right_col
            || range.bottom_row >= self.rows.len()
            || range.right_col >= self.columns.len()
            || range.cell_count() <= 1
        {
            return;
        }
        let mut ranges = self.canonical_merged_ranges();
        ranges.retain(|current| {
            current.bottom_row < range.top_row
                || current.top_row > range.bottom_row
                || current.right_col < range.left_col
                || current.left_col > range.right_col
        });
        ranges.push(range);
        self.set_merged_ranges(ranges);
    }

    /// Unmerges a cell.
    pub fn unmerge_cell(&mut self, row: usize, col: usize) {
        let ranges = self
            .canonical_merged_ranges()
            .into_iter()
            .filter(|range| !range.contains(row, col))
            .collect();
        self.set_merged_ranges(ranges);
    }

    /// Sets uniform row height for all rows.
    pub fn set_uniform_row_height(&mut self, height: f64) {
        for row in &mut self.rows {
            row.height = height;
        }
    }

    /// Sets uniform column width for all columns.
    pub fn set_uniform_column_width(&mut self, width: f64) {
        for col in &mut self.columns {
            col.width = width;
        }
    }

    /// Clears all cell content but keeps structure.
    pub fn clear_content(&mut self) {
        for row in &mut self.rows {
            for cell in &mut row.cells {
                cell.contents.clear();
            }
        }
    }
}

impl Default for Table {
    fn default() -> Self {
        Self::new(Vector3::ZERO, 3, 3)
    }
}

impl Entity for Table {
    fn common(&self) -> &super::EntityCommon {
        &self.common
    }

    fn common_mut(&mut self) -> &mut super::EntityCommon {
        &mut self.common
    }

    fn bounding_box(&self) -> BoundingBox3D {
        let horizontal = if self.horizontal_direction.length() > 1.0e-12 {
            self.horizontal_direction.normalize()
        } else {
            Vector3::UNIT_X
        };
        let normal = if self.normal.length() > 1.0e-12 {
            self.normal.normalize()
        } else {
            Vector3::UNIT_Z
        };
        let down = {
            let candidate = horizontal.cross(&normal);
            if candidate.length() > 1.0e-12 {
                candidate.normalize()
            } else {
                Vector3::new(0.0, -1.0, 0.0)
            }
        };
        let width = self.total_width();
        let height = self.total_height();
        let p0 = self.insertion_point;
        let p1 = p0 + horizontal * width;
        let p2 = p0 + down * height;
        let p3 = p1 + down * height;
        BoundingBox3D::from_points(&[p0, p1, p2, p3])
            .unwrap_or_else(|| BoundingBox3D::from_point(self.insertion_point))
    }

    fn translate(&mut self, offset: Vector3) {
        super::translate::translate_table(self, offset);
    }

    fn entity_type(&self) -> &'static str {
        "ACAD_TABLE"
    }

    fn apply_transform(&mut self, transform: &crate::types::Transform) {
        super::transform::transform_table(self, transform);
    }
}

// ============================================================================
// Builder
// ============================================================================

/// Builder for Table entities.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableBuilder {
    table: Table,
}

impl TableBuilder {
    /// Creates a new builder.
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            table: Table::new(Vector3::ZERO, rows, cols),
        }
    }

    /// Sets the insertion point.
    pub fn at(mut self, point: Vector3) -> Self {
        self.table.insertion_point = point;
        self
    }

    /// Sets uniform row height.
    pub fn row_height(mut self, height: f64) -> Self {
        self.table.set_uniform_row_height(height);
        self
    }

    /// Sets uniform column width.
    pub fn column_width(mut self, width: f64) -> Self {
        self.table.set_uniform_column_width(width);
        self
    }

    /// Sets the layer.
    pub fn layer(mut self, layer: &str) -> Self {
        self.table.common.layer = layer.to_string();
        self
    }

    /// Sets text in a cell.
    pub fn cell_text(mut self, row: usize, col: usize, text: &str) -> Self {
        self.table.set_cell_text(row, col, text);
        self
    }

    /// Sets a header row (first row) with texts.
    pub fn header(mut self, headers: &[&str]) -> Self {
        for (col, text) in headers.iter().enumerate() {
            self.table.set_cell_text(0, col, text);
        }
        self
    }

    /// Builds the table.
    pub fn build(self) -> Table {
        self.table
    }
}

impl Default for TableBuilder {
    fn default() -> Self {
        Self::new(3, 3)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_creation() {
        let table = Table::new(Vector3::ZERO, 3, 4);
        assert_eq!(table.row_count(), 3);
        assert_eq!(table.column_count(), 4);
    }

    #[test]
    fn test_table_cell_access() {
        let mut table = Table::new(Vector3::ZERO, 2, 2);
        table.set_cell_text(0, 0, "A1");
        table.set_cell_text(1, 1, "B2");

        assert_eq!(table.cell_text(0, 0), Some("A1"));
        assert_eq!(table.cell_text(1, 1), Some("B2"));
        assert_eq!(table.cell_text(0, 1), Some(""));
    }

    #[test]
    fn test_table_dimensions() {
        let mut table = Table::new(Vector3::ZERO, 2, 3);
        table.set_uniform_row_height(1.0);
        table.set_uniform_column_width(2.0);

        assert_eq!(table.total_height(), 2.0);
        assert_eq!(table.total_width(), 6.0);
    }

    #[test]
    fn test_table_add_row() {
        let mut table = Table::new(Vector3::ZERO, 2, 2);
        table.add_row();
        assert_eq!(table.row_count(), 3);
        assert_eq!(table.rows[2].cell_count(), 2);
    }

    #[test]
    fn test_table_add_column() {
        let mut table = Table::new(Vector3::ZERO, 2, 2);
        table.add_column(3.0);
        assert_eq!(table.column_count(), 3);
        assert_eq!(table.columns[2].width, 3.0);
        assert_eq!(table.rows[0].cell_count(), 3);
    }

    #[test]
    fn test_table_remove_row() {
        let mut table = Table::new(Vector3::ZERO, 3, 2);
        table.remove_row(1);
        assert_eq!(table.row_count(), 2);
    }

    #[test]
    fn test_table_remove_column() {
        let mut table = Table::new(Vector3::ZERO, 2, 3);
        table.remove_column(1);
        assert_eq!(table.column_count(), 2);
        assert_eq!(table.rows[0].cell_count(), 2);
    }

    #[test]
    fn test_cell_merge() {
        let mut table = Table::new(Vector3::ZERO, 3, 3);
        table.merge_cells(CellRange::new(0, 0, 1, 2));

        let cell = table.cell(0, 0).unwrap();
        assert!(cell.is_merged());
        assert_eq!(cell.merge_width, 3);
        assert_eq!(cell.merge_height, 2);
    }

    #[test]
    fn test_cell_value() {
        let text = CellValue::text("Hello");
        assert_eq!(text.display(), "Hello");
        assert!(!text.is_empty());

        let number = CellValue::number(42.5);
        assert_eq!(number.numeric_value, 42.5);
    }

    #[test]
    fn test_cell_range() {
        let range = CellRange::new(1, 2, 3, 4);
        assert_eq!(range.row_count(), 3);
        assert_eq!(range.col_count(), 3);
        assert_eq!(range.cell_count(), 9);
        assert!(range.contains(2, 3));
        assert!(!range.contains(0, 0));
    }

    #[test]
    fn test_table_builder() {
        let table = TableBuilder::new(2, 3)
            .at(Vector3::new(10.0, 20.0, 0.0))
            .row_height(0.5)
            .column_width(2.0)
            .header(&["A", "B", "C"])
            .cell_text(1, 0, "Data")
            .build();

        assert_eq!(table.insertion_point.x, 10.0);
        assert_eq!(table.cell_text(0, 0), Some("A"));
        assert_eq!(table.cell_text(1, 0), Some("Data"));
    }

    #[test]
    fn test_table_bounding_box() {
        let mut table = Table::new(Vector3::new(5.0, 10.0, 0.0), 2, 3);
        table.set_uniform_row_height(1.0);
        table.set_uniform_column_width(2.0);

        // The insertion point is the table's top-left corner: with the
        // default horizontal (+X) and normal (+Z), rows flow downward (-Y),
        // so the box spans y in [insert.y - height, insert.y] and
        // x in [insert.x, insert.x + width].
        let bbox = table.bounding_box();
        assert_eq!(bbox.min.x, 5.0);
        assert_eq!(bbox.max.x, 11.0); // 5 + 3*2
        assert_eq!(bbox.max.y, 10.0); // insertion point = top edge
        assert_eq!(bbox.min.y, 8.0); // 10 - 2*1
    }

    #[test]
    fn test_table_translate() {
        let mut table = Table::new(Vector3::new(0.0, 0.0, 0.0), 2, 2);
        table.translate(Vector3::new(10.0, 20.0, 5.0));
        assert_eq!(table.insertion_point, Vector3::new(10.0, 20.0, 5.0));
    }

    #[test]
    fn test_cell_content() {
        let text = CellContent::text("Hello");
        assert_eq!(text.content_type, TableCellContentType::Value);

        let block = CellContent::block(Handle::new(0x100));
        assert_eq!(block.content_type, TableCellContentType::Block);
        assert!(block.block_handle.is_some());
    }

    #[test]
    fn test_cell_border() {
        let border = CellBorder::with_color(Color::from_index(1));
        assert!(!border.invisible);
        assert!(border.override_flags.contains(BorderPropertyFlags::COLOR));
    }

    #[test]
    fn test_cell_style() {
        let mut style = CellStyle::new();
        style.set_margins(0.1);
        style.set_border_color(Color::from_index(2));

        assert_eq!(style.margin_left, 0.1);
        assert_eq!(style.top_border.color, Color::from_index(2));
    }

    #[test]
    fn test_table_row() {
        let row = TableRow::from_texts(&["A", "B", "C"]);
        assert_eq!(row.cell_count(), 3);
        assert_eq!(row.cell(0).unwrap().text_value(), "A");
    }

    #[test]
    fn test_table_column() {
        let col = TableColumn::named("Width", 5.0);
        assert_eq!(col.name, "Width");
        assert_eq!(col.width, 5.0);
    }

    #[test]
    fn test_insert_operations() {
        let mut table = Table::new(Vector3::ZERO, 2, 2);
        table.insert_row(1);
        assert_eq!(table.row_count(), 3);

        table.insert_column(1, 3.0);
        assert_eq!(table.column_count(), 3);
        assert_eq!(table.rows[0].cell_count(), 3);
    }

    #[test]
    fn test_cell_state_flags() {
        let flags = CellStateFlags::CONTENT_LOCKED | CellStateFlags::FORMAT_LOCKED;
        assert!(flags.contains(CellStateFlags::CONTENT_LOCKED));
        assert!(!flags.contains(CellStateFlags::LINKED));
    }

    // `canonical_merged_ranges` deduplicates in linear time. The tests below hold it to the
    // previous quadratic algorithm, kept here as the reference, and to a time bound.

    /// The previous algorithm: one `Vec::contains` over the kept ranges per range.
    fn reference_canonical_merged_ranges(table: &Table) -> Vec<CellRange> {
        let row_count = table.rows.len();
        let column_count = table.columns.len();
        if row_count == 0 || column_count == 0 {
            return Vec::new();
        }

        let mut ranges = Vec::new();
        for range in &table.merged_ranges {
            if range.top_row >= row_count
                || range.left_col >= column_count
                || range.top_row > range.bottom_row
                || range.left_col > range.right_col
            {
                continue;
            }
            let range = CellRange {
                top_row: range.top_row,
                left_col: range.left_col,
                bottom_row: range.bottom_row.min(row_count - 1),
                right_col: range.right_col.min(column_count - 1),
            };
            if range.cell_count() > 1 && !ranges.contains(&range) {
                ranges.push(range);
            }
        }
        for (row_index, row) in table.rows.iter().enumerate() {
            for (column_index, cell) in row.cells.iter().take(column_count).enumerate() {
                let height = cell.merge_height.max(1) as usize;
                let width = cell.merge_width.max(1) as usize;
                if height == 1 && width == 1 {
                    continue;
                }
                let range = CellRange {
                    top_row: row_index,
                    left_col: column_index,
                    bottom_row: row_index.saturating_add(height - 1).min(row_count - 1),
                    right_col: column_index.saturating_add(width - 1).min(column_count - 1),
                };
                if range.cell_count() > 1 && !ranges.contains(&range) {
                    ranges.push(range);
                }
            }
        }
        ranges
    }

    /// Deterministic xorshift64* numbers for the comparison inputs.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            self.0 = x;
            x.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }

        /// A number in `0..n` (`n` > 0).
        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }

        /// A cell span: mostly 1, else 0, negative, small, past the grid or huge.
        fn span(&mut self) -> i32 {
            match self.below(8) {
                0 => 0,
                1 => -(self.below(3) as i32) - 1,
                2 => i32::MAX,
                3 | 4 => self.below(12) as i32 + 1,
                _ => 1,
            }
        }
    }

    /// A range from `rng`: coordinates up to 10 (some outside the grid, inverted or one
    /// cell), or a duplicate, a range nested in or adjacent to an earlier one.
    fn random_range(rng: &mut Rng, earlier: &[CellRange]) -> CellRange {
        let pick = |rng: &mut Rng| earlier[rng.below(earlier.len())];
        match rng.below(6) {
            0 if !earlier.is_empty() => pick(rng),
            1 if !earlier.is_empty() => {
                let r = pick(rng);
                CellRange::new(
                    r.top_row + rng.below(2),
                    r.left_col + rng.below(2),
                    r.bottom_row.saturating_sub(rng.below(2)),
                    r.right_col.saturating_sub(rng.below(2)),
                )
            }
            2 if !earlier.is_empty() => {
                let r = pick(rng);
                if rng.below(2) == 0 {
                    CellRange::new(r.top_row, r.right_col + 1, r.bottom_row, r.right_col + 2)
                } else {
                    CellRange::new(r.bottom_row + 1, r.left_col, r.bottom_row + 2, r.right_col)
                }
            }
            _ => CellRange::new(rng.below(11), rng.below(11), rng.below(11), rng.below(11)),
        }
    }

    /// A table of up to 8 × 8 cells, rows holding more or fewer cells than there are
    /// columns, with random merged ranges and cell spans.
    fn random_table(rng: &mut Rng) -> Table {
        let mut table = Table::new(Vector3::ZERO, rng.below(9), rng.below(9));
        for row in &mut table.rows {
            match rng.below(6) {
                0 => {
                    let keep = rng.below(row.cells.len() + 1);
                    row.cells.truncate(keep);
                }
                1 => row.cells.push(TableCell::new()),
                _ => {}
            }
            for cell in &mut row.cells {
                if rng.below(3) == 0 {
                    cell.merge_width = rng.span();
                    cell.merge_height = rng.span();
                }
            }
        }
        for _ in 0..rng.below(40) {
            let range = random_range(rng, &table.merged_ranges);
            table.merged_ranges.push(range);
        }
        table
    }

    #[test]
    fn canonical_merged_ranges_match_the_original_algorithm() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let mut kept = 0;
        for case in 0..5_000 {
            let table = random_table(&mut rng);
            let expected = reference_canonical_merged_ranges(&table);
            kept += expected.len();
            assert_eq!(
                table.canonical_merged_ranges(),
                expected,
                "case {case}: {:?}",
                table.merged_ranges
            );
        }
        assert!(kept > 10_000, "the inputs keep ranges: {kept}");
    }

    /// Every range of two cells or more in a `rows` × `columns` grid, row by row.
    fn every_range(rows: usize, columns: usize) -> Vec<CellRange> {
        let mut out = Vec::new();
        for top in 0..rows {
            for left in 0..columns {
                for bottom in top..rows {
                    for right in left..columns {
                        let range = CellRange::new(top, left, bottom, right);
                        if range.cell_count() > 1 {
                            out.push(range);
                        }
                    }
                }
            }
        }
        out
    }

    #[test]
    fn canonical_merged_ranges_take_linear_time() {
        // 215,325 distinct ranges of a 30 × 30 table, plus a span on every cell: the
        // original compared each range with every kept one, about 2.3e10 comparisons
        // (minutes in a debug build).
        let mut table = Table::new(Vector3::ZERO, 30, 30);
        let ranges = every_range(30, 30);
        assert_eq!(ranges.len(), 215_325);
        table.merged_ranges = ranges.clone();
        for row in &mut table.rows {
            for cell in &mut row.cells {
                cell.merge_width = 2;
            }
        }
        let start = std::time::Instant::now();
        let got = table.canonical_merged_ranges();
        let elapsed = start.elapsed();
        assert_eq!(got, ranges, "every span repeats a listed range");
        assert!(
            elapsed < std::time::Duration::from_secs(10),
            "{elapsed:?} for {} ranges",
            ranges.len()
        );
    }

    // `set_merged_ranges` and `MergedCellIndex` lay merged ranges over the cells within a
    // visit budget. The tests below hold them to the previous algorithms, kept here as the
    // references, and to time bounds.

    /// The previous `set_merged_ranges`: every range marks all of its cells.
    fn reference_set_merged_ranges(table: &mut Table, ranges: Vec<CellRange>) {
        for row in &mut table.rows {
            for cell in &mut row.cells {
                cell.merge_width = 1;
                cell.merge_height = 1;
                cell.merged = 0;
            }
        }
        table.merged_ranges = ranges;
        for range in &table.merged_ranges {
            for row in table
                .rows
                .iter_mut()
                .take(range.bottom_row + 1)
                .skip(range.top_row)
            {
                for cell in row
                    .cells
                    .iter_mut()
                    .take(range.right_col + 1)
                    .skip(range.left_col)
                {
                    cell.merged = 1;
                }
            }
            if let Some(cell) = table
                .rows
                .get_mut(range.top_row)
                .and_then(|row| row.cells.get_mut(range.left_col))
            {
                cell.merged = 0;
                cell.merge_width = range.col_count() as i32;
                cell.merge_height = range.row_count() as i32;
            }
        }
    }

    /// The previous DXF writer search: the first range containing the cell.
    fn reference_first_range(ranges: &[CellRange], row: usize, column: usize) -> Option<usize> {
        ranges.iter().position(|range| range.contains(row, column))
    }

    /// (merged, merge width, merge height) of every cell, row by row.
    fn merge_state(table: &Table) -> Vec<Vec<(i32, i32, i32)>> {
        table
            .rows
            .iter()
            .map(|row| {
                row.cells
                    .iter()
                    .map(|cell| (cell.merged, cell.merge_width, cell.merge_height))
                    .collect()
            })
            .collect()
    }

    #[test]
    fn set_merged_ranges_match_the_original_algorithm() {
        let mut rng = Rng(0xD1B5_4A32_D192_ED03);
        let mut overlapping = 0;
        for case in 0..5_000 {
            let table = random_table(&mut rng);
            let ranges = table.canonical_merged_ranges();
            let cells: usize = ranges.iter().map(CellRange::cell_count).sum();
            if cells > table.rows.len() * table.columns.len() {
                overlapping += 1;
            }
            let mut expected = table.clone();
            reference_set_merged_ranges(&mut expected, ranges.clone());
            let mut got = table.clone();
            got.set_merged_ranges(ranges);
            assert_eq!(merge_state(&got), merge_state(&expected), "case {case}");
            assert_eq!(got.merged_ranges, expected.merged_ranges, "case {case}");
        }
        assert!(overlapping > 1_000, "the inputs overlap: {overlapping}");
    }

    #[test]
    fn merged_cell_index_matches_the_original_search() {
        let mut rng = Rng(0x94D0_49BB_1331_11EB);
        for case in 0..5_000 {
            let table = random_table(&mut rng);
            let ranges = table.canonical_merged_ranges();
            let index = MergedCellIndex::new(&table, &ranges);
            for (r, row) in table.rows.iter().enumerate() {
                for c in 0..row.cells.len() {
                    assert_eq!(
                        index.first(r, c),
                        reference_first_range(&ranges, r, c),
                        "case {case}, cell ({r}, {c}): {ranges:?}"
                    );
                }
            }
        }
    }

    /// 1,048,576 distinct ranges of a 200 × 200 table, each at least 136 × 136 cells: they all
    /// overlap.
    fn overlapping_ranges() -> Vec<CellRange> {
        let mut out = Vec::with_capacity(1 << 20);
        for top in 0..32 {
            for left in 0..32 {
                for bottom in 168..200 {
                    for right in 168..200 {
                        out.push(CellRange::new(top, left, bottom, right));
                    }
                }
            }
        }
        out
    }

    #[test]
    fn overlapping_merged_ranges_take_linear_time() {
        // The original marked every cell of every range: about 2.4e10 cell writes.
        let mut table = Table::new(Vector3::ZERO, 200, 200);
        table.merged_ranges = overlapping_ranges();
        let start = std::time::Instant::now();
        table.sync_cells_from_merged_ranges();
        let elapsed = start.elapsed();
        assert_eq!(table.merged_ranges.len(), 1 << 20);
        let corner = &table.rows[0].cells[0];
        assert_eq!(
            (corner.merged, corner.merge_width, corner.merge_height),
            (0, 200, 200),
            "anchors take the last range they anchor"
        );
        assert_eq!(table.rows[150].cells[150].merged, 1, "covered");
        assert!(elapsed < std::time::Duration::from_secs(30), "{elapsed:?}");
        let start = std::time::Instant::now();
        let index = MergedCellIndex::new(&table, &table.merged_ranges);
        let elapsed = start.elapsed();
        assert_eq!(index.first(150, 150), Some(0), "claimed by the first range");
        assert_eq!(
            index.first(199, 199),
            None,
            "only ranges past the budget reach it"
        );
        assert!(elapsed < std::time::Duration::from_secs(30), "{elapsed:?}");
    }

    /// A range past the budget does not stop the ranges after it. In a
    /// 300 × 300 table the first four of ten nested ranges spend most of the budget and the
    /// other six do not fit; a range in the bottom row, which overlaps none of them, is still
    /// merged, in the cells and in the writer's index alike.
    #[test]
    fn a_disjoint_range_after_an_over_budget_one_is_still_merged() {
        let mut table = Table::new(Vector3::ZERO, 300, 300);
        table.merged_ranges = (0..10)
            .map(|k| CellRange::new(0, 0, 298, 299 - k))
            .chain([CellRange::new(299, 0, 299, 1)])
            .collect();
        table.sync_cells_from_merged_ranges();
        let anchor = &table.rows[299].cells[0];
        assert_eq!(
            (anchor.merged, anchor.merge_width, anchor.merge_height),
            (0, 2, 1)
        );
        assert_eq!(table.rows[299].cells[1].merged, 1, "covered");
        let index = MergedCellIndex::new(&table, &table.merged_ranges);
        assert_eq!(index.first(299, 0), Some(10));
        assert_eq!(index.first(299, 1), Some(10));
        assert_eq!(index.first(299, 2), None);
    }

    /// A range over rows shorter than the column count costs the cells those rows hold, not
    /// its area, so one range over 1,000 one-cell rows of a 10,000-column table is laid exactly
    /// as before.
    #[test]
    fn ranges_over_short_rows_cost_the_cells_they_hold() {
        let mut table = Table::new(Vector3::ZERO, 1_000, 1);
        table.columns.resize_with(10_000, TableColumn::default);
        table.merged_ranges = vec![CellRange::new(0, 0, 999, 9_999)];
        let ranges = table.canonical_merged_ranges();
        let mut expected = table.clone();
        reference_set_merged_ranges(&mut expected, ranges.clone());
        let mut got = table.clone();
        got.set_merged_ranges(ranges.clone());
        let (got, expected) = (merge_state(&got), merge_state(&expected));
        assert_eq!(got.len(), expected.len());
        let differs = got.iter().zip(&expected).position(|(a, b)| a != b);
        assert_eq!(differs, None, "first row laid differently");
        let index = MergedCellIndex::new(&table, &ranges);
        for row in 0..1_000 {
            assert_eq!(
                index.first(row, 0),
                reference_first_range(&ranges, row, 0),
                "row {row}"
            );
        }
    }
}

pub fn legacy_override_properties(flags: i32) -> CellStylePropertyFlags {
    let mut properties = CellStylePropertyFlags::NONE;
    for (bit, property) in [
        (0x01, CellStylePropertyFlags::ALIGNMENT),
        (0x02, CellStylePropertyFlags::BACKGROUND_COLOR),
        (0x04, CellStylePropertyFlags::BACKGROUND_COLOR),
        (0x08, CellStylePropertyFlags::CONTENT_COLOR),
        (0x10, CellStylePropertyFlags::TEXT_STYLE),
        (0x20, CellStylePropertyFlags::TEXT_HEIGHT),
    ] {
        if flags & bit != 0 {
            properties |= property;
        }
    }
    properties
}

pub fn cell_override_bit(code: i32) -> Option<i32> {
    Some(match code {
        170 => 0x01,
        283 => 0x02,
        63 => 0x04,
        64 => 0x08,
        7 => 0x10,
        140 => 0x20,
        69 => 0x40,
        65 => 0x80,
        66 => 0x100,
        68 => 0x200,
        279 => 0x400,
        275 => 0x800,
        276 => 0x1000,
        278 => 0x2000,
        289 => 0x4000,
        285 => 0x8000,
        286 => 0x10000,
        288 => 0x20000,
        _ => return None,
    })
}
