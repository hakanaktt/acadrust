//! Table entity implementation.
//!
//! The Table entity represents a grid of cells that can contain text,
//! blocks, or formulas, with extensive styling and formatting options.

use crate::entities::{Entity, EntityCommon};
use crate::types::{BoundingBox3D, Color, Handle, LineWeight, Transparency, Vector3};

use bitflags::bitflags;

// ============================================================================
// Enums
// ============================================================================

/// Cell content type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
pub struct CellBorder {
    /// Border type.
    pub border_type: BorderType,
    /// Border color.
    pub color: Color,
    /// Border line weight.
    pub line_weight: LineWeight,
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
pub struct CellValue {
    /// Value type.
    pub value_type: CellValueType,
    /// Unit type.
    pub unit_type: ValueUnitType,
    /// Value flags.
    pub flags: i32,
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
}

impl CellValue {
    /// Creates an empty cell value.
    pub fn new() -> Self {
        Self {
            value_type: CellValueType::Unknown,
            unit_type: ValueUnitType::NoUnits,
            flags: 0,
            text: String::new(),
            format: String::new(),
            formatted_value: String::new(),
            numeric_value: 0.0,
            handle_value: None,
        }
    }

    /// Creates a text value.
    pub fn text(s: &str) -> Self {
        Self {
            value_type: CellValueType::String,
            unit_type: ValueUnitType::NoUnits,
            flags: 0,
            text: s.to_string(),
            format: String::new(),
            formatted_value: s.to_string(),
            numeric_value: 0.0,
            handle_value: None,
        }
    }

    /// Creates a numeric value.
    pub fn number(n: f64) -> Self {
        Self {
            value_type: CellValueType::Double,
            unit_type: ValueUnitType::NoUnits,
            flags: 0,
            text: String::new(),
            format: String::new(),
            formatted_value: n.to_string(),
            numeric_value: n,
            handle_value: None,
        }
    }

    /// Creates an integer value.
    pub fn integer(n: i64) -> Self {
        Self {
            value_type: CellValueType::Long,
            unit_type: ValueUnitType::NoUnits,
            flags: 0,
            text: String::new(),
            format: String::new(),
            formatted_value: n.to_string(),
            numeric_value: n as f64,
            handle_value: None,
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
pub struct CellContent {
    /// Content type.
    pub content_type: TableCellContentType,
    /// Cell value.
    pub value: CellValue,
    /// Block content handle (for block type).
    pub block_handle: Option<Handle>,
    /// Text style handle.
    pub text_style_handle: Option<Handle>,
    /// Content color.
    pub color: Color,
    /// Rotation angle in radians.
    pub rotation: f64,
    /// Scale factor (for blocks).
    pub scale: f64,
    /// Text height.
    pub text_height: f64,
}

impl CellContent {
    /// Creates empty cell content.
    pub fn new() -> Self {
        Self {
            content_type: TableCellContentType::Unknown,
            value: CellValue::new(),
            block_handle: None,
            text_style_handle: None,
            color: Color::ByBlock,
            rotation: 0.0,
            scale: 1.0,
            text_height: 0.18,
        }
    }

    /// Creates text content.
    pub fn text(s: &str) -> Self {
        Self {
            content_type: TableCellContentType::Value,
            value: CellValue::text(s),
            block_handle: None,
            text_style_handle: None,
            color: Color::ByBlock,
            rotation: 0.0,
            scale: 1.0,
            text_height: 0.18,
        }
    }

    /// Creates block content.
    pub fn block(block_handle: Handle) -> Self {
        Self {
            content_type: TableCellContentType::Block,
            value: CellValue::new(),
            block_handle: Some(block_handle),
            text_style_handle: None,
            color: Color::ByBlock,
            rotation: 0.0,
            scale: 1.0,
            text_height: 0.18,
        }
    }
}

impl Default for CellContent {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Cell Style
// ============================================================================

/// Style applied to a table cell.
#[derive(Debug, Clone, PartialEq)]
pub struct CellStyle {
    /// Style type.
    pub style_type: CellStyleType,
    /// Property override flags.
    pub property_flags: CellStylePropertyFlags,
    /// Background fill color.
    pub background_color: Color,
    /// Content color.
    pub content_color: Color,
    /// Text style handle.
    pub text_style_handle: Option<Handle>,
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
}

impl CellStyle {
    /// Creates a default cell style.
    pub fn new() -> Self {
        Self {
            style_type: CellStyleType::Cell,
            property_flags: CellStylePropertyFlags::NONE,
            background_color: Color::ByBlock,
            content_color: Color::ByBlock,
            text_style_handle: None,
            text_height: 0.18,
            rotation: 0.0,
            scale: 1.0,
            alignment: 0,
            fill_enabled: false,
            layout_flags: ContentLayoutFlags::FLOW,
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

/// A cell in a table.
#[derive(Debug, Clone, PartialEq)]
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
            auto_fit: false,
            merge_width: 1,
            merge_height: 1,
            flag: 0,
            merged: 0,
            virtual_edge: 0,
            has_linked_data: false,
            custom_data: 0,
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
        self.contents.first()
            .map(|c| c.value.display())
            .unwrap_or("")
    }

    /// Sets the text value.
    pub fn set_text(&mut self, s: &str) {
        self.contents.clear();
        self.contents.push(CellContent::text(s));
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
pub struct TableRow {
    /// Row height.
    pub height: f64,
    /// Cells in this row.
    pub cells: Vec<TableCell>,
    /// Row style override.
    pub style: Option<CellStyle>,
    /// Custom data value.
    pub custom_data: i32,
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
pub struct TableColumn {
    /// Column name.
    pub name: String,
    /// Column width.
    pub width: f64,
    /// Column style override.
    pub style: Option<CellStyle>,
    /// Custom data value.
    pub custom_data: i32,
}

impl TableColumn {
    /// Creates a new column with default width.
    pub fn new() -> Self {
        Self {
            name: String::new(),
            width: 2.5,
            style: None,
            custom_data: 0,
        }
    }

    /// Creates a column with the given width.
    pub fn with_width(width: f64) -> Self {
        Self {
            name: String::new(),
            width,
            style: None,
            custom_data: 0,
        }
    }

    /// Creates a named column.
    pub fn named(name: &str, width: f64) -> Self {
        Self {
            name: name.to_string(),
            width,
            style: None,
            custom_data: 0,
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
        row >= self.top_row && row <= self.bottom_row &&
        col >= self.left_col && col <= self.right_col
    }
}

impl Default for CellRange {
    fn default() -> Self {
        Self::cell(0, 0)
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
    /// Table data version.
    pub data_version: i16,
    /// Table value flags.
    pub value_flags: i32,
    /// Flag overrides.
    pub override_flag: bool,
    pub override_border_color: bool,
    pub override_border_line_weight: bool,
    pub override_border_visibility: bool,
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
}

impl Table {
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
            data_version: 0,
            value_flags: 0,
            override_flag: false,
            override_border_color: false,
            override_border_line_weight: false,
            override_border_visibility: false,
            rows,
            columns,
            break_options: BreakOptionFlags::NONE,
            break_flow_direction: BreakFlowDirection::Right,
            break_spacing: 0.0,
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
            self.rows.insert(index, TableRow::new(num_cols));
        }
    }

    /// Inserts a column at the given index.
    pub fn insert_column(&mut self, index: usize, width: f64) {
        if index <= self.columns.len() {
            self.columns.insert(index, TableColumn::with_width(width));
            for row in &mut self.rows {
                row.cells.insert(index, TableCell::new());
            }
        }
    }

    /// Removes a row.
    pub fn remove_row(&mut self, index: usize) -> Option<TableRow> {
        if index < self.rows.len() {
            Some(self.rows.remove(index))
        } else {
            None
        }
    }

    /// Removes a column.
    pub fn remove_column(&mut self, index: usize) -> Option<TableColumn> {
        if index < self.columns.len() {
            let col = self.columns.remove(index);
            for row in &mut self.rows {
                if index < row.cells.len() {
                    row.cells.remove(index);
                }
            }
            Some(col)
        } else {
            None
        }
    }

    /// Merges cells in the given range.
    pub fn merge_cells(&mut self, range: CellRange) {
        if let Some(cell) = self.cell_mut(range.top_row, range.left_col) {
            cell.merge_width = range.col_count() as i32;
            cell.merge_height = range.row_count() as i32;
        }
    }

    /// Unmerges a cell.
    pub fn unmerge_cell(&mut self, row: usize, col: usize) {
        if let Some(cell) = self.cell_mut(row, col) {
            cell.merge_width = 1;
            cell.merge_height = 1;
        }
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
    fn handle(&self) -> Handle {
        self.common.handle
    }

    fn set_handle(&mut self, handle: Handle) {
        self.common.handle = handle;
    }

    fn layer(&self) -> &str {
        &self.common.layer
    }

    fn set_layer(&mut self, layer: String) {
        self.common.layer = layer;
    }

    fn color(&self) -> Color {
        self.common.color
    }

    fn set_color(&mut self, color: Color) {
        self.common.color = color;
    }

    fn line_weight(&self) -> LineWeight {
        self.common.line_weight
    }

    fn set_line_weight(&mut self, line_weight: LineWeight) {
        self.common.line_weight = line_weight;
    }

    fn transparency(&self) -> Transparency {
        self.common.transparency
    }

    fn set_transparency(&mut self, transparency: Transparency) {
        self.common.transparency = transparency;
    }

    fn is_invisible(&self) -> bool {
        self.common.invisible
    }

    fn set_invisible(&mut self, invisible: bool) {
        self.common.invisible = invisible;
    }

    fn bounding_box(&self) -> BoundingBox3D {
        let min = self.insertion_point;
        let max = Vector3::new(
            min.x + self.total_width(),
            min.y + self.total_height(),
            min.z,
        );
        BoundingBox3D::new(min, max)
    }

    fn translate(&mut self, offset: Vector3) {
        self.insertion_point = self.insertion_point + offset;
    }

    fn entity_type(&self) -> &'static str {
        "ACAD_TABLE"
    }
    
    fn apply_transform(&mut self, transform: &crate::types::Transform) {
        // Transform insertion point
        self.insertion_point = transform.apply(self.insertion_point);
        
        // Transform direction vectors
        self.horizontal_direction = transform.apply_rotation(self.horizontal_direction).normalize();
        self.normal = transform.apply_rotation(self.normal).normalize();
        
        // Scale column widths and row heights
        let unit_x = Vector3::new(1.0, 0.0, 0.0);
        let transformed_unit = transform.apply_rotation(unit_x);
        let scale_factor = transformed_unit.length();
        
        for col in &mut self.columns {
            col.width *= scale_factor;
        }
        for row in &mut self.rows {
            row.height *= scale_factor;
        }
    }
}

// ============================================================================
// Builder
// ============================================================================

/// Builder for Table entities.
#[derive(Debug, Clone)]
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

        let bbox = table.bounding_box();
        assert_eq!(bbox.min.x, 5.0);
        assert_eq!(bbox.min.y, 10.0);
        assert_eq!(bbox.max.x, 11.0); // 5 + 3*2
        assert_eq!(bbox.max.y, 12.0); // 10 + 2*1
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
}

