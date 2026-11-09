//! TableStyle object implementation.
//!
//! Defines the visual properties and cell formatting for Table entities.

use crate::types::{Color, Handle, LineWeight};

use bitflags::bitflags;

// ============================================================================
// Enums
// ============================================================================

/// Flow direction for table content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(i16)]
pub enum TableFlowDirection {
    /// Content flows from top to bottom (default).
    #[default]
    Down = 0,
    /// Content flows from bottom to top.
    Up = 1,
}

impl From<i16> for TableFlowDirection {
    fn from(value: i16) -> Self {
        match value {
            1 => Self::Up,
            _ => Self::Down,
        }
    }
}

/// Cell alignment within the cell bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(i16)]
pub enum CellAlignment {
    /// Top left alignment.
    TopLeft = 1,
    /// Top center alignment.
    TopCenter = 2,
    /// Top right alignment.
    TopRight = 3,
    /// Middle left alignment.
    MiddleLeft = 4,
    /// Middle center alignment (default).
    #[default]
    MiddleCenter = 5,
    /// Middle right alignment.
    MiddleRight = 6,
    /// Bottom left alignment.
    BottomLeft = 7,
    /// Bottom center alignment.
    BottomCenter = 8,
    /// Bottom right alignment.
    BottomRight = 9,
}

impl From<i16> for CellAlignment {
    fn from(value: i16) -> Self {
        match value {
            1 => Self::TopLeft,
            2 => Self::TopCenter,
            3 => Self::TopRight,
            4 => Self::MiddleLeft,
            5 => Self::MiddleCenter,
            6 => Self::MiddleRight,
            7 => Self::BottomLeft,
            8 => Self::BottomCenter,
            9 => Self::BottomRight,
            _ => Self::MiddleCenter,
        }
    }
}

impl CellAlignment {
    /// Returns true if alignment is top-aligned.
    pub fn is_top(&self) -> bool {
        matches!(self, Self::TopLeft | Self::TopCenter | Self::TopRight)
    }

    /// Returns true if alignment is middle-aligned.
    pub fn is_middle(&self) -> bool {
        matches!(
            self,
            Self::MiddleLeft | Self::MiddleCenter | Self::MiddleRight
        )
    }

    /// Returns true if alignment is bottom-aligned.
    pub fn is_bottom(&self) -> bool {
        matches!(
            self,
            Self::BottomLeft | Self::BottomCenter | Self::BottomRight
        )
    }

    /// Returns true if alignment is left-aligned.
    pub fn is_left(&self) -> bool {
        matches!(self, Self::TopLeft | Self::MiddleLeft | Self::BottomLeft)
    }

    /// Returns true if alignment is center-aligned.
    pub fn is_center(&self) -> bool {
        matches!(
            self,
            Self::TopCenter | Self::MiddleCenter | Self::BottomCenter
        )
    }

    /// Returns true if alignment is right-aligned.
    pub fn is_right(&self) -> bool {
        matches!(self, Self::TopRight | Self::MiddleRight | Self::BottomRight)
    }
}

/// Border type for cell borders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(i16)]
pub enum TableBorderType {
    /// Single line border (default).
    #[default]
    Single = 1,
    /// Double line border.
    Double = 2,
}

impl From<i16> for TableBorderType {
    fn from(value: i16) -> Self {
        match value {
            2 => Self::Double,
            _ => Self::Single,
        }
    }
}

// ============================================================================
// Flags
// ============================================================================

bitflags! {
    /// Property override flags for table cell styles.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct TableCellStylePropertyFlags: i32 {
        /// No properties overridden.
        const NONE = 0x0;
        /// Data type override.
        const DATA_TYPE = 0x1;
        /// Data format override.
        const DATA_FORMAT = 0x2;
        /// Rotation override.
        const ROTATION = 0x4;
        /// Block scale override.
        const BLOCK_SCALE = 0x8;
        /// Alignment override.
        const ALIGNMENT = 0x10;
        /// Content color override.
        const CONTENT_COLOR = 0x20;
        /// Text style override.
        const TEXT_STYLE = 0x40;
        /// Text height override.
        const TEXT_HEIGHT = 0x80;
        /// Auto scale override.
        const AUTO_SCALE = 0x100;
        /// Background color override.
        const BACKGROUND_COLOR = 0x200;
        /// Left margin override.
        const MARGIN_LEFT = 0x400;
        /// Top margin override.
        const MARGIN_TOP = 0x800;
        /// Right margin override.
        const MARGIN_RIGHT = 0x1000;
        /// Bottom margin override.
        const MARGIN_BOTTOM = 0x2000;
        /// Content layout override.
        const CONTENT_LAYOUT = 0x4000;
        /// Merge all.
        const MERGE_ALL = 0x8000;
        /// Flow direction bottom to top.
        const FLOW_DIRECTION_BOTTOM_TO_TOP = 0x10000;
        /// Horizontal spacing override.
        const MARGIN_HORIZONTAL_SPACING = 0x20000;
        /// Vertical spacing override.
        const MARGIN_VERTICAL_SPACING = 0x40000;
    }
}

bitflags! {
    /// Border property override flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct TableBorderPropertyFlags: i32 {
        /// No properties overridden.
        const NONE = 0x0;
        /// Line weight override.
        const LINE_WEIGHT = 0x1;
        /// Line type override.
        const LINE_TYPE = 0x2;
        /// Color override.
        const COLOR = 0x4;
        /// Visibility override.
        const VISIBILITY = 0x8;
        /// Double line spacing override.
        const DOUBLE_LINE_SPACING = 0x10;
    }
}

bitflags! {
    /// Table style flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct TableStyleFlags: i16 {
        /// No flags.
        const NONE = 0;
        /// Title row is suppressed.
        const TITLE_SUPPRESSED = 1;
        /// Header row is suppressed.
        const HEADER_SUPPRESSED = 2;
    }
}

// ============================================================================
// Cell Border
// ============================================================================

/// Border definition for a cell edge.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableCellBorder {
    /// Property override flags.
    pub property_flags: TableBorderPropertyFlags,

    /// Border type (single or double).
    pub border_type: TableBorderType,

    /// Line weight.
    pub line_weight: LineWeight,

    /// Border color.
    pub color: Color,

    /// Is invisible/hidden.
    pub is_invisible: bool,

    /// Spacing for double line borders.
    pub double_line_spacing: f64,
}

impl TableCellBorder {
    /// Creates a new cell border with default values.
    pub fn new() -> Self {
        TableCellBorder {
            property_flags: TableBorderPropertyFlags::NONE,
            border_type: TableBorderType::Single,
            line_weight: LineWeight::ByBlock,
            color: Color::ByBlock,
            is_invisible: false,
            double_line_spacing: 0.0,
        }
    }

    /// Creates an invisible border.
    pub fn invisible() -> Self {
        TableCellBorder {
            is_invisible: true,
            ..Self::new()
        }
    }

    /// Creates a border with specific color and weight.
    pub fn with_style(color: Color, line_weight: LineWeight) -> Self {
        TableCellBorder {
            color,
            line_weight,
            ..Self::new()
        }
    }
}

impl Default for TableCellBorder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Row Cell Style
// ============================================================================

/// Cell style definition for a row type (data, header, title).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RowCellStyle {
    /// Text style name.
    pub text_style_name: String,

    /// Text style handle.
    pub text_style_handle: Option<Handle>,

    /// Text height.
    /// DXF code: 140
    pub text_height: f64,

    /// Cell alignment.
    /// DXF code: 170
    pub alignment: CellAlignment,

    /// Text color.
    /// DXF code: 62
    pub text_color: Color,

    /// Background/fill color.
    /// DXF code: 63
    pub fill_color: Color,

    /// Whether background fill is enabled.
    /// DXF code: 283
    pub fill_enabled: bool,

    /// Cell data type.
    /// DXF code: 90
    pub data_type: i32,

    /// Cell unit type.
    /// DXF code: 91
    pub unit_type: i32,

    /// Format string.
    /// DXF code: 1
    pub format_string: String,

    // Borders
    /// Left border.
    pub left_border: TableCellBorder,
    /// Right border.
    pub right_border: TableCellBorder,
    /// Top border.
    pub top_border: TableCellBorder,
    /// Bottom border.
    pub bottom_border: TableCellBorder,
    /// Horizontal inside border.
    pub horizontal_inside_border: TableCellBorder,
    /// Vertical inside border.
    pub vertical_inside_border: TableCellBorder,
}

impl RowCellStyle {
    /// Creates a new row cell style with default values.
    pub fn new() -> Self {
        RowCellStyle {
            text_style_name: "Standard".to_string(),
            text_style_handle: None,
            text_height: 0.18,
            alignment: CellAlignment::MiddleCenter,
            text_color: Color::ByBlock,
            fill_color: Color::Index(7), // White/Black
            fill_enabled: false,
            data_type: 512,
            unit_type: 0,
            format_string: String::new(),
            left_border: TableCellBorder::new(),
            right_border: TableCellBorder::new(),
            top_border: TableCellBorder::new(),
            bottom_border: TableCellBorder::new(),
            horizontal_inside_border: TableCellBorder::new(),
            vertical_inside_border: TableCellBorder::new(),
        }
    }

    /// Creates a data row style (smaller text, top-centered, as the
    /// default table style of the reference templates).
    pub fn data_row() -> Self {
        RowCellStyle {
            text_height: 0.18,
            alignment: CellAlignment::TopCenter,
            ..Self::new()
        }
    }

    /// Creates a header row style (larger text, middle-centered).
    pub fn header_row() -> Self {
        RowCellStyle {
            text_height: 0.25,
            alignment: CellAlignment::MiddleCenter,
            ..Self::new()
        }
    }

    /// Creates a title row style (larger text, middle-centered).
    pub fn title_row() -> Self {
        RowCellStyle {
            text_height: 0.25,
            alignment: CellAlignment::MiddleCenter,
            ..Self::new()
        }
    }

    /// Sets all borders to the same style.
    pub fn set_all_borders(&mut self, border: TableCellBorder) {
        self.left_border = border.clone();
        self.right_border = border.clone();
        self.top_border = border.clone();
        self.bottom_border = border.clone();
        self.horizontal_inside_border = border.clone();
        self.vertical_inside_border = border;
    }

    /// Sets all borders invisible.
    pub fn set_all_borders_invisible(&mut self) {
        self.set_all_borders(TableCellBorder::invisible());
    }
}

impl Default for RowCellStyle {
    fn default() -> Self {
        Self::new()
    }
}

/// Complete R2010+ content-format payload used by TABLESTYLE cell styles.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableContentFormat {
    pub property_override_flags: i32,
    pub property_flags: i32,
    pub value_data_type: i32,
    pub value_unit_type: i32,
    pub value_format_string: String,
    pub rotation: f64,
    pub block_scale: f64,
    pub cell_alignment: i32,
    pub content_color: Color,
    pub text_style: Handle,
    pub text_height: f64,
}

impl Default for TableContentFormat {
    fn default() -> Self {
        Self {
            property_override_flags: 0,
            property_flags: 0,
            value_data_type: 512,
            value_unit_type: 0,
            value_format_string: String::new(),
            rotation: 0.0,
            block_scale: 1.0,
            cell_alignment: 5,
            content_color: Color::ByBlock,
            text_style: Handle::NULL,
            text_height: 0.18,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableGridFormat {
    pub index_mask: i32,
    pub border: TableCellBorder,
    pub line_type: Handle,
}

impl Default for TableGridFormat {
    fn default() -> Self {
        Self {
            index_mask: 0,
            border: TableCellBorder::default(),
            line_type: Handle::NULL,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableCellStyleData {
    pub style_type: i32,
    pub data_flags: i16,
    pub property_override_flags: i32,
    pub merge_flags: i32,
    pub background_color: Color,
    pub content_layout: i32,
    pub content_format: TableContentFormat,
    pub margin_override_flags: i16,
    pub vertical_margin: f64,
    pub horizontal_margin: f64,
    pub bottom_margin: f64,
    pub right_margin: f64,
    pub horizontal_spacing: f64,
    pub vertical_spacing: f64,
    pub borders: Vec<TableGridFormat>,
}

#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NamedTableCellStyle {
    pub cell_style: TableCellStyleData,
    pub id: i32,
    pub style_type: i32,
    pub name: String,
}

/// Grid edges of a cell style, in the order the R2010+ record stores them.
pub const TABLE_GRID_EDGES: [i32; 6] = [1, 2, 4, 8, 16, 32];

impl TableCellStyleData {
    /// "Merge cells on row/column creation".
    pub const MERGE_ALL: i32 = 0x8000;
    /// On the "Table" base cell style: rows run bottom to top.
    pub const FLOW_UP: i32 = 0x10000;

    pub fn merge_all(&self) -> bool {
        self.merge_flags & Self::MERGE_ALL != 0
    }

    pub fn set_merge_all(&mut self, on: bool) {
        self.merge_flags = if on {
            self.merge_flags | Self::MERGE_ALL
        } else {
            self.merge_flags & !Self::MERGE_ALL
        };
    }

    /// Horizontal and vertical margins; `None` when the cell style keeps
    /// the table's.
    pub fn margins(&self) -> Option<(f64, f64)> {
        (self.margin_override_flags != 0).then_some((self.horizontal_margin, self.vertical_margin))
    }

    /// Sets the left/right (`horizontal`) and top/bottom (`vertical`) margins.
    pub fn set_margins(&mut self, horizontal: f64, vertical: f64) {
        if self.margin_override_flags == 0 {
            // A cell style that took the table's margins starts its spacing
            // at its text height, as a new style does.
            self.horizontal_spacing = self.content_format.text_height;
            self.vertical_spacing = self.content_format.text_height;
        }
        self.margin_override_flags = 1;
        self.horizontal_margin = horizontal;
        self.right_margin = horizontal;
        self.vertical_margin = vertical;
        self.bottom_margin = vertical;
    }

    /// The grid record of an edge (`TABLE_GRID_EDGES`), if the style has one.
    pub fn grid(&self, edge: i32) -> Option<&TableGridFormat> {
        self.borders.iter().find(|grid| grid.index_mask == edge)
    }

    /// The grid record of an edge, added (visible, by block) when missing.
    pub fn grid_mut(&mut self, edge: i32) -> &mut TableGridFormat {
        if let Some(index) = self.borders.iter().position(|grid| grid.index_mask == edge) {
            return &mut self.borders[index];
        }
        let spacing = 0.25 * self.content_format.text_height;
        let at = self
            .borders
            .iter()
            .position(|grid| grid.index_mask > edge)
            .unwrap_or(self.borders.len());
        self.borders.insert(
            at,
            TableGridFormat {
                index_mask: edge,
                border: TableCellBorder {
                    // The record's flag is set for a hidden edge; the field
                    // holds its inverse (see `edge_hidden`).
                    is_invisible: true,
                    double_line_spacing: spacing,
                    ..TableCellBorder::default()
                },
                line_type: Handle::NULL,
            },
        );
        &mut self.borders[at]
    }

    /// Whether an edge is hidden. The grid record stores a "hidden" flag,
    /// read into `is_invisible` inverted.
    pub fn edge_hidden(&self, edge: i32) -> bool {
        self.grid(edge).is_some_and(|grid| !grid.border.is_invisible)
    }

    pub fn set_edge_hidden(&mut self, edge: i32, hidden: bool) {
        self.grid_mut(edge).border.is_invisible = !hidden;
    }

    /// The pre-R2010 row style this cell style describes (text, fill and
    /// the six edges, with `is_invisible` meaning hidden).
    pub fn to_row_style(&self) -> RowCellStyle {
        let mut row = RowCellStyle::new();
        row.text_style_name.clear();
        apply_cell_style_to_row(&mut row, self);
        row
    }
}

/// Names of the cell styles the title, header and data rows use.
pub const TITLE_CELL_STYLE: &str = "_TITLE";
pub const HEADER_CELL_STYLE: &str = "_HEADER";
pub const DATA_CELL_STYLE: &str = "_DATA";

/// Cell style class: a label cell (title, header) or a data cell.
pub const CELL_CLASS_LABEL: i32 = 1;
pub const CELL_CLASS_DATA: i32 = 2;

/// The R2010+ cell style a pre-R2010 row style describes.
pub(crate) fn row_cell_style(
    row: &RowCellStyle,
    // Horizontal and vertical margins, and the spacing (the data text
    // height in every cell style of the style).
    margins: (f64, f64, f64),
    text_style: Handle,
    merge_flags: i32,
) -> TableCellStyleData {
    // R2010+ edge order; the grid flag is set for a hidden edge, the
    // reverse of the row border's `is_invisible`.
    let borders = [
        (1, &row.top_border),
        (2, &row.horizontal_inside_border),
        (4, &row.bottom_border),
        (8, &row.left_border),
        (16, &row.vertical_inside_border),
        (32, &row.right_border),
    ]
    .into_iter()
    .map(|(index_mask, border)| TableGridFormat {
        index_mask,
        border: TableCellBorder {
            is_invisible: !border.is_invisible,
            ..border.clone()
        },
        line_type: Handle::NULL,
    })
    .collect();
    TableCellStyleData {
        style_type: 5,
        data_flags: 1,
        merge_flags,
        background_color: if row.fill_enabled {
            row.fill_color
        } else {
            Color::None
        },
        content_layout: 1,
        content_format: TableContentFormat {
            value_data_type: row.data_type,
            value_unit_type: row.unit_type,
            value_format_string: row.format_string.clone(),
            block_scale: 1.0,
            cell_alignment: row.alignment as i32,
            content_color: row.text_color,
            text_style,
            text_height: row.text_height,
            ..TableContentFormat::default()
        },
        margin_override_flags: 1,
        horizontal_margin: margins.0,
        right_margin: margins.0,
        vertical_margin: margins.1,
        bottom_margin: margins.1,
        horizontal_spacing: margins.2,
        vertical_spacing: margins.2,
        borders,
        ..TableCellStyleData::default()
    }
}

/// The pre-R2010 row style an R2010+ cell style describes.
pub(crate) fn apply_cell_style_to_row(row: &mut RowCellStyle, style: &TableCellStyleData) {
    let format = &style.content_format;
    row.data_type = format.value_data_type;
    row.unit_type = format.value_unit_type;
    row.format_string = format.value_format_string.clone();
    row.alignment = CellAlignment::from(format.cell_alignment as i16);
    row.text_color = format.content_color;
    row.text_style_handle = (!format.text_style.is_null()).then_some(format.text_style);
    row.text_height = format.text_height;
    // No background color means an unfilled row; the row keeps its default
    // fill color.
    row.fill_enabled = style.background_color != Color::None;
    if row.fill_enabled {
        row.fill_color = style.background_color;
    }
    // Grid edges in R2010+ order: top, horizontal inside, bottom, left,
    // vertical inside, right. The grid's stored flag is set for a hidden
    // edge, the reverse of what `is_invisible` holds for it.
    for grid in &style.borders {
        let mut border = grid.border.clone();
        border.is_invisible = !border.is_invisible;
        match grid.index_mask {
            1 => row.top_border = border,
            2 => row.horizontal_inside_border = border,
            4 => row.bottom_border = border,
            8 => row.left_border = border,
            16 => row.vertical_inside_border = border,
            32 => row.right_border = border,
            _ => {}
        }
    }
}

// ============================================================================
// TableStyle
// ============================================================================

/// Table style object.
///
/// Defines the visual properties and cell formatting for Table entities.
///
/// # DXF Information
/// - Object type: TABLESTYLE
/// - Subclass marker: AcDbTableStyle
///
/// # Example
///
/// ```ignore
/// use acadrust::objects::TableStyle;
///
/// let mut style = TableStyle::new("MyStyle");
/// style.horizontal_margin = 0.1;
/// style.vertical_margin = 0.1;
/// style.data_row_style.text_height = 0.2;
/// ```
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableStyle {
    /// Object handle.
    pub handle: Handle,

    /// Owner handle.
    pub owner_handle: Handle,

    /// Style name.
    /// DXF code: 3
    pub name: String,

    /// Style description.
    pub description: String,

    /// Version flag.
    /// DXF code: 280
    pub version: i16,

    /// Flow direction.
    /// DXF code: 70
    pub flow_direction: TableFlowDirection,

    /// Style flags.
    /// DXF code: 71
    pub flags: TableStyleFlags,

    /// Horizontal cell margin.
    /// DXF code: 40
    pub horizontal_margin: f64,

    /// Vertical cell margin.
    /// DXF code: 41
    pub vertical_margin: f64,

    /// Whether title row is suppressed.
    /// DXF code: 280
    pub title_suppressed: bool,

    /// Whether header row is suppressed.
    /// DXF code: 281
    pub header_suppressed: bool,

    /// Data row cell style.
    pub data_row_style: RowCellStyle,

    /// Header row cell style.
    pub header_row_style: RowCellStyle,

    /// Title row cell style.
    pub title_row_style: RowCellStyle,
    /// R2010+ object metadata and complete named cell-style payload.
    pub modern_unknown_byte: u8,
    pub modern_unknown_long1: i32,
    pub modern_unknown_long2: i32,
    pub modern_cell_style_handle: Handle,
    pub modern_style: Option<NamedTableCellStyle>,
    pub modern_overrides: Vec<(i32, NamedTableCellStyle)>,
    /// Annotative: tables using this style scale with the annotation scale.
    /// Persisted as XDATA under the `AcadAnnotative` application:
    /// `AnnotativeData { 1 <flag> }`.
    pub annotative: bool,
    /// Original DXF group-code payload for lossless DXF round-trips.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub raw_dxf_codes: Option<Vec<(i32, String)>>,
}

impl TableStyle {
    /// Object type name.
    pub const OBJECT_NAME: &'static str = "TABLESTYLE";

    /// Subclass marker.
    pub const SUBCLASS_MARKER: &'static str = "AcDbTableStyle";

    /// Default style name.
    pub const STANDARD: &'static str = "Standard";

    /// Creates a new TableStyle with default values.
    pub fn new(name: &str) -> Self {
        TableStyle {
            handle: Handle::NULL,
            owner_handle: Handle::NULL,
            name: name.to_string(),
            description: String::new(),
            version: 0,
            flow_direction: TableFlowDirection::Down,
            flags: TableStyleFlags::NONE,
            horizontal_margin: 0.06,
            vertical_margin: 0.06,
            title_suppressed: false,
            header_suppressed: false,
            data_row_style: RowCellStyle::data_row(),
            header_row_style: RowCellStyle::header_row(),
            title_row_style: RowCellStyle::title_row(),
            modern_unknown_byte: 0,
            modern_unknown_long1: 8,
            modern_unknown_long2: 101,
            modern_cell_style_handle: Handle::NULL,
            modern_style: None,
            modern_overrides: Vec::new(),
            annotative: false,
            raw_dxf_codes: None,
        }
    }

    /// Creates the standard TableStyle.
    pub fn standard() -> Self {
        Self::new(Self::STANDARD)
    }

    /// Sets both margins uniformly.
    pub fn set_margins(&mut self, margin: f64) {
        self.horizontal_margin = margin;
        self.vertical_margin = margin;
    }

    /// Sets the text height for all row types.
    pub fn set_all_text_heights(&mut self, height: f64) {
        self.data_row_style.text_height = height;
        self.header_row_style.text_height = height;
        self.title_row_style.text_height = height;
    }

    /// Sets the text style for all row types.
    pub fn set_all_text_styles(&mut self, name: &str, handle: Option<Handle>) {
        self.data_row_style.text_style_name = name.to_string();
        self.data_row_style.text_style_handle = handle;
        self.header_row_style.text_style_name = name.to_string();
        self.header_row_style.text_style_handle = handle;
        self.title_row_style.text_style_name = name.to_string();
        self.title_row_style.text_style_handle = handle;
    }

    /// Sets the text color for all row types.
    pub fn set_all_text_colors(&mut self, color: Color) {
        self.data_row_style.text_color = color;
        self.header_row_style.text_color = color;
        self.title_row_style.text_color = color;
    }

    /// Sets the fill color for all row types.
    pub fn set_all_fill_colors(&mut self, color: Color) {
        self.data_row_style.fill_color = color;
        self.header_row_style.fill_color = color;
        self.title_row_style.fill_color = color;
    }

    /// Enables background fill for all row types.
    pub fn set_all_fill_enabled(&mut self, enabled: bool) {
        self.data_row_style.fill_enabled = enabled;
        self.header_row_style.fill_enabled = enabled;
        self.title_row_style.fill_enabled = enabled;
    }

    /// Returns true if title row is visible.
    pub fn has_title_row(&self) -> bool {
        !self.title_suppressed && !self.flags.contains(TableStyleFlags::TITLE_SUPPRESSED)
    }

    /// Returns true if header row is visible.
    pub fn has_header_row(&self) -> bool {
        !self.header_suppressed && !self.flags.contains(TableStyleFlags::HEADER_SUPPRESSED)
    }

    /// Sets title row visibility.
    pub fn set_title_visible(&mut self, visible: bool) {
        self.title_suppressed = !visible;
        self.flags.set(TableStyleFlags::TITLE_SUPPRESSED, !visible);
    }

    /// Sets header row visibility.
    pub fn set_header_visible(&mut self, visible: bool) {
        self.header_suppressed = !visible;
        self.flags.set(TableStyleFlags::HEADER_SUPPRESSED, !visible);
    }

    pub(crate) fn effective_flags(&self) -> TableStyleFlags {
        let mut flags = self.flags;
        flags.set(TableStyleFlags::TITLE_SUPPRESSED, !self.has_title_row());
        flags.set(TableStyleFlags::HEADER_SUPPRESSED, !self.has_header_row());
        flags
    }

    pub(crate) fn sync_suppression_flags(&mut self) {
        self.flags = self.effective_flags();
        self.title_suppressed = self.flags.contains(TableStyleFlags::TITLE_SUPPRESSED);
        self.header_suppressed = self.flags.contains(TableStyleFlags::HEADER_SUPPRESSED);
    }

    /// The style's starting table (a `TableTemplate` object), if it has one.
    pub fn template(&self) -> Option<Handle> {
        (!self.modern_cell_style_handle.is_null()).then_some(self.modern_cell_style_handle)
    }

    /// The style's named cell styles (title, header, data and the user's).
    pub fn cell_styles(&self) -> impl Iterator<Item = &NamedTableCellStyle> {
        self.modern_overrides.iter().map(|(_, style)| style)
    }

    pub fn cell_style(&self, name: &str) -> Option<&NamedTableCellStyle> {
        self.cell_styles()
            .find(|style| style.name.eq_ignore_ascii_case(name))
    }

    pub fn cell_style_mut(&mut self, name: &str) -> Option<&mut NamedTableCellStyle> {
        self.modern_overrides
            .iter_mut()
            .map(|(_, style)| style)
            .find(|style| style.name.eq_ignore_ascii_case(name))
    }

    pub fn cell_style_by_id(&self, id: i32) -> Option<&NamedTableCellStyle> {
        self.cell_styles().find(|style| style.id == id)
    }

    /// Gives a style that only has the pre-R2010 title, header and data
    /// rows (a new style, or one read from an older file) the named cell
    /// styles the rows describe. `text_style` resolves a row's text style.
    pub fn ensure_cell_styles(&mut self, text_style: impl Fn(&RowCellStyle) -> Handle) {
        if !self.modern_overrides.is_empty() {
            return;
        }
        let margins = (
            self.horizontal_margin,
            self.vertical_margin,
            self.data_row_style.text_height,
        );
        let rows = [
            (&self.title_row_style, 1, CELL_CLASS_LABEL, TITLE_CELL_STYLE, TableCellStyleData::MERGE_ALL),
            (&self.header_row_style, 2, CELL_CLASS_LABEL, HEADER_CELL_STYLE, 0),
            (&self.data_row_style, 3, CELL_CLASS_DATA, DATA_CELL_STYLE, 0),
        ];
        self.modern_overrides = rows
            .into_iter()
            .map(|(row, id, class, name, merge)| {
                (
                    id,
                    NamedTableCellStyle {
                        cell_style: row_cell_style(row, margins, text_style(row), merge),
                        id,
                        style_type: class,
                        name: name.to_string(),
                    },
                )
            })
            .collect();
    }

    /// Projects the title, header and data cell styles and the base cell
    /// style's direction onto the pre-R2010 fields, which the DXF record
    /// and older files carry. Call after editing the cell styles.
    pub fn sync_rows_from_cell_styles(&mut self) {
        // The DXF groups the style was read with no longer describe it.
        self.raw_dxf_codes = None;
        let cells: Vec<(String, TableCellStyleData)> = self
            .cell_styles()
            .map(|style| (style.name.clone(), style.cell_style.clone()))
            .collect();
        for (name, cell) in cells {
            let row = match name.as_str() {
                TITLE_CELL_STYLE => &mut self.title_row_style,
                HEADER_CELL_STYLE => &mut self.header_row_style,
                DATA_CELL_STYLE => {
                    if let Some((horizontal, vertical)) = cell.margins() {
                        self.horizontal_margin = horizontal;
                        self.vertical_margin = vertical;
                    }
                    &mut self.data_row_style
                }
                _ => continue,
            };
            apply_cell_style_to_row(row, &cell);
        }
    }

    /// A cell style name not in use, from `base` ("Cell style", "Cell style
    /// (2)", ...).
    pub fn unique_cell_style_name(&self, base: &str) -> String {
        if self.cell_style(base).is_none() {
            return base.to_string();
        }
        (2..)
            .map(|n| format!("{base} ({n})"))
            .find(|name| self.cell_style(name).is_none())
            .unwrap_or_default()
    }

    /// Adds a cell style named `name`, a copy of `source` or else a blank
    /// data cell style. User cell styles are numbered from 101. Returns its
    /// id, or `None` when the name is empty or taken.
    pub fn create_cell_style(&mut self, name: &str, source: Option<&str>) -> Option<i32> {
        let name = name.trim();
        if name.is_empty() || self.cell_style(name).is_some() {
            return None;
        }
        let id = self
            .cell_styles()
            .map(|style| style.id)
            .filter(|id| *id > 100)
            .max()
            .unwrap_or(100)
            + 1;
        let mut style = match source.and_then(|source| self.cell_style(source)) {
            Some(source) => source.clone(),
            None => NamedTableCellStyle {
                cell_style: TableCellStyleData {
                    style_type: 5,
                    data_flags: 1,
                    background_color: Color::None,
                    content_layout: 1,
                    content_format: TableContentFormat {
                        cell_alignment: CellAlignment::TopLeft as i32,
                        ..TableContentFormat::default()
                    },
                    ..TableCellStyleData::default()
                },
                style_type: CELL_CLASS_DATA,
                ..NamedTableCellStyle::default()
            },
        };
        style.id = id;
        style.name = name.to_string();
        self.modern_overrides.push((id, style));
        Some(id)
    }

    /// Renames a user cell style; the title, header and data cell styles
    /// keep their names.
    pub fn rename_cell_style(&mut self, name: &str, new_name: &str) -> bool {
        let new_name = new_name.trim();
        if new_name.is_empty()
            || is_row_cell_style(name)
            || self
                .cell_style(new_name)
                .is_some_and(|other| !other.name.eq_ignore_ascii_case(name))
        {
            return false;
        }
        match self.cell_style_mut(name) {
            Some(style) => {
                style.name = new_name.to_string();
                true
            }
            None => false,
        }
    }

    /// Deletes a user cell style; the title, header and data cell styles
    /// stay.
    pub fn delete_cell_style(&mut self, name: &str) -> bool {
        if is_row_cell_style(name) {
            return false;
        }
        let before = self.modern_overrides.len();
        self.modern_overrides
            .retain(|(_, style)| !style.name.eq_ignore_ascii_case(name));
        self.modern_overrides.len() != before
    }
}

/// The title, header and data cell styles every table style has.
pub fn is_row_cell_style(name: &str) -> bool {
    [TITLE_CELL_STYLE, HEADER_CELL_STYLE, DATA_CELL_STYLE]
        .iter()
        .any(|row| row.eq_ignore_ascii_case(name))
}

impl Default for TableStyle {
    fn default() -> Self {
        Self::standard()
    }
}

const CELL_STYLE_MAP_KEY: &str = "ACAD_ROUNDTRIP_2008_TABLESTYLE_CELLSTYLEMAP";
/// A DXF file keeps a table style's starting table in its extension
/// dictionary under this key; a DWG file in the style record.
const TEMPLATE_KEY: &str = "ACAD_ROUNDTRIP_2008_TABLESTYLE_TEMPLATE";

/// Moves the starting tables (`TableTemplate`) to where the output keeps
/// them: in DXF (and pre-R2010 DWG, whose style record has no field for
/// it) under the style's extension dictionary, in R2010+ DWG owned by the
/// style record itself.
pub(crate) fn prepare_table_style_templates(
    document: &mut std::borrow::Cow<'_, crate::document::CadDocument>,
    in_dictionary: bool,
) {
    use crate::objects::ObjectType;
    let mut changes = Vec::new();
    for (handle, object) in &document.objects {
        let ObjectType::TableStyle(style) = object else {
            continue;
        };
        let Some(template) = style.template() else {
            continue;
        };
        let Some(ObjectType::TableTemplate(table)) = document.objects.get(&template) else {
            continue;
        };
        let entry = document
            .extension_dictionary_handle(*handle)
            .and_then(|dictionary| match document.objects.get(&dictionary) {
                Some(ObjectType::Dictionary(entries)) => {
                    entries.get(TEMPLATE_KEY).map(|entry| (dictionary, entry))
                }
                _ => None,
            });
        let placed = if in_dictionary {
            entry.is_some_and(|(dictionary, entry)| {
                entry == template && table.common.owner_handle == dictionary
            })
        } else {
            entry.is_none() && table.common.owner_handle == *handle
        };
        if !placed {
            changes.push((*handle, template));
        }
    }
    if changes.is_empty() {
        return;
    }
    let document = document.to_mut();
    document.classes.ensure_default("TABLETEMPLATE");
    for (style, template) in changes {
        let owner = if in_dictionary {
            let dictionary = document.ensure_extension_dictionary(style);
            if let Some(ObjectType::Dictionary(entries)) = document.objects.get_mut(&dictionary) {
                entries.remove(TEMPLATE_KEY);
                entries.add_entry(TEMPLATE_KEY, template);
                entries.hard_owner_entries.push(TEMPLATE_KEY.to_string());
            }
            dictionary
        } else {
            if let Some(dictionary) = document.extension_dictionary_handle(style) {
                if let Some(ObjectType::Dictionary(entries)) =
                    document.objects.get_mut(&dictionary)
                {
                    entries.remove(TEMPLATE_KEY);
                }
            }
            style
        };
        if let Some(ObjectType::TableTemplate(table)) = document.objects.get_mut(&template) {
            table.common.owner_handle = owner;
            table.common.reactors = vec![owner];
        }
    }
}
const ROUNDTRIP_RECORD_KEY: &str = "ACAD_XREC_ROUNDTRIP";
const PRE2007_MARKER: &str = "ACAD_ROUNDTRIP_PRE2007_TABLESTYLE";

fn legacy_rows_mut(style: &mut TableStyle) -> [&mut RowCellStyle; 3] {
    [
        &mut style.data_row_style,
        &mut style.title_row_style,
        &mut style.header_row_style,
    ]
}

pub(crate) fn restore_table_style_roundtrip(document: &mut crate::document::CadDocument) {
    use crate::objects::{DataObjectData, ObjectType, XRecordValue};
    // A starting table kept in the extension dictionary (DXF, pre-R2010).
    let templates: Vec<(Handle, Handle)> = document
        .objects
        .iter()
        .filter_map(|(handle, object)| match object {
            ObjectType::TableStyle(style) if style.template().is_none() => {
                let dictionary = document.extension_dictionary_handle(*handle)?;
                let Some(ObjectType::Dictionary(entries)) = document.objects.get(&dictionary)
                else {
                    return None;
                };
                let template = entries.get(TEMPLATE_KEY)?;
                matches!(document.objects.get(&template), Some(ObjectType::TableTemplate(_)))
                    .then_some((*handle, template))
            }
            _ => None,
        })
        .collect();
    for (style, template) in templates {
        if let Some(ObjectType::TableStyle(style)) = document.objects.get_mut(&style) {
            style.modern_cell_style_handle = template;
        }
    }
    let styles: Vec<_> = document
        .objects
        .iter()
        .filter_map(|(handle, object)| match object {
            ObjectType::TableStyle(style) if style.modern_overrides.is_empty() => Some(*handle),
            _ => None,
        })
        .collect();
    for handle in styles {
        let Some(dictionary) = document.extension_dictionary_handle(handle) else {
            continue;
        };
        let Some(ObjectType::Dictionary(dictionary)) = document.objects.get(&dictionary) else {
            continue;
        };
        let cells =
            dictionary
                .get(CELL_STYLE_MAP_KEY)
                .and_then(|map| match document.objects.get(&map) {
                    Some(ObjectType::DataObject(object)) => match &object.data {
                        DataObjectData::CellStyleMap(map) => Some(map.cells.clone()),
                        _ => None,
                    },
                    _ => None,
                });
        let mut row_types: [Option<(i32, i32, String)>; 3] = Default::default();
        if let Some(Some(ObjectType::XRecord(record))) = dictionary
            .get(ROUNDTRIP_RECORD_KEY)
            .map(|record| document.objects.get(&record))
        {
            let mut entries = record
                .entries
                .iter()
                .skip_while(|entry| entry.value.as_string() != Some(PRE2007_MARKER))
                .skip(1)
                .take_while(|entry| entry.code != 102);
            for slot in &mut row_types {
                let (Some(data_type), Some(unit_type), Some(format)) =
                    (entries.next(), entries.next(), entries.next())
                else {
                    break;
                };
                if let (XRecordValue::Int32(data_type), XRecordValue::Int32(unit_type)) =
                    (&data_type.value, &unit_type.value)
                {
                    *slot = Some((
                        *data_type,
                        *unit_type,
                        format.value.as_string().unwrap_or_default().to_string(),
                    ));
                }
            }
        }
        let Some(ObjectType::TableStyle(style)) = document.objects.get_mut(&handle) else {
            continue;
        };
        if let Some(cells) = cells {
            style.modern_overrides = cells.into_iter().map(|cell| (cell.id, cell)).collect();
        }
        for (row, types) in legacy_rows_mut(style).into_iter().zip(row_types) {
            if let Some((data_type, unit_type, format)) = types {
                row.data_type = data_type;
                row.unit_type = unit_type;
                row.format_string = format;
            }
        }
    }
}

pub(crate) fn store_table_style_roundtrip(document: &mut crate::document::CadDocument) {
    use crate::types::DxfVersion;
    if document.version >= DxfVersion::AC1024 {
        return;
    }
    store_cell_style_maps(document);
}

/// A DXF file keeps a table style's named cell styles in its extension
/// dictionary, whatever its version: bring those maps up to date with the
/// cell styles (a style edited or made after reading has a stale or no map).
pub(crate) fn prepare_table_style_cell_maps(
    document: &mut std::borrow::Cow<'_, crate::document::CadDocument>,
) {
    use crate::objects::{DataObjectData, ObjectType};
    let stale = document.objects.iter().any(|(handle, object)| {
        let ObjectType::TableStyle(style) = object else {
            return false;
        };
        if style.modern_overrides.is_empty() {
            return false;
        }
        let stored = document
            .extension_dictionary_handle(*handle)
            .and_then(|dictionary| match document.objects.get(&dictionary) {
                Some(ObjectType::Dictionary(entries)) => entries.get(CELL_STYLE_MAP_KEY),
                _ => None,
            })
            .and_then(|map| match document.objects.get(&map) {
                Some(ObjectType::DataObject(object)) => match &object.data {
                    DataObjectData::CellStyleMap(map) => Some(&map.cells),
                    _ => None,
                },
                _ => None,
            });
        stored.is_none_or(|cells| {
            !cells.iter().eq(style.modern_overrides.iter().map(|(_, cell)| cell))
        })
    });
    if stale {
        store_cell_style_maps(document.to_mut());
    }
}

fn store_cell_style_maps(document: &mut crate::document::CadDocument) {
    use crate::objects::{
        CellStyleMap, DataObject, DataObjectData, ObjectType, XRecordEntry, XRecordValue,
    };
    use crate::types::DxfVersion;
    let with_row_types = document.version < DxfVersion::AC1021;
    let mut styles: Vec<_> = document
        .objects
        .iter()
        .filter_map(|(handle, object)| match object {
            ObjectType::TableStyle(style) if !style.modern_overrides.is_empty() => Some((
                *handle,
                style
                    .modern_overrides
                    .iter()
                    .map(|(_, cell)| cell.clone())
                    .collect::<Vec<_>>(),
                [
                    &style.data_row_style,
                    &style.title_row_style,
                    &style.header_row_style,
                ]
                .map(|row| (row.data_type, row.unit_type, row.format_string.clone())),
            )),
            _ => None,
        })
        .collect();
    // Objects are created in handle order so the output is deterministic.
    styles.sort_by_key(|style| style.0);
    for (handle, cells, row_types) in styles {
        let dictionary = document.ensure_extension_dictionary(handle);
        let existing = match document.objects.get(&dictionary) {
            Some(ObjectType::Dictionary(entries)) => entries.get(CELL_STYLE_MAP_KEY),
            _ => None,
        };
        match existing.and_then(|map| document.objects.get_mut(&map)) {
            Some(ObjectType::DataObject(DataObject {
                data: DataObjectData::CellStyleMap(map),
                ..
            })) => map.cells = cells,
            _ => {
                // The map takes its type code from the CELLSTYLEMAP class;
                // without it the writer leaves the map out.
                document.classes.ensure_default("CELLSTYLEMAP");
                let map = document.allocate_handle();
                let mut object =
                    DataObject::new(DataObjectData::CellStyleMap(CellStyleMap { cells }));
                object.handle = map;
                object.owner = dictionary;
                object.reactors = vec![dictionary];
                document.objects.insert(map, ObjectType::DataObject(object));
                if let Some(ObjectType::Dictionary(entries)) = document.objects.get_mut(&dictionary)
                {
                    entries
                        .entries
                        .retain(|(name, _)| name != CELL_STYLE_MAP_KEY);
                    entries.add_entry(CELL_STYLE_MAP_KEY, map);
                }
            }
        }
        if !with_row_types {
            continue;
        }
        let record = document.ensure_xrecord(handle, ROUNDTRIP_RECORD_KEY);
        let Some(ObjectType::XRecord(record)) = document.objects.get_mut(&record) else {
            continue;
        };
        let start = record
            .entries
            .iter()
            .position(|entry| entry.value.as_string() == Some(PRE2007_MARKER));
        if let Some(start) = start {
            let end = record.entries[start + 1..]
                .iter()
                .position(|entry| entry.code == 102)
                .map_or(record.entries.len(), |offset| start + 1 + offset);
            record.entries.drain(start..end);
        }
        let mut group = vec![XRecordEntry {
            code: 102,
            value: XRecordValue::String(PRE2007_MARKER.to_string()),
        }];
        for (index, (data_type, unit_type, format)) in row_types.into_iter().enumerate() {
            let index = index as i32;
            group.push(XRecordEntry {
                code: 90 + 2 * index,
                value: XRecordValue::Int32(data_type),
            });
            group.push(XRecordEntry {
                code: 91 + 2 * index,
                value: XRecordValue::Int32(unit_type),
            });
            group.push(XRecordEntry {
                code: 1 + index,
                value: XRecordValue::String(format),
            });
        }
        let at = start.unwrap_or(record.entries.len());
        record.entries.splice(at..at, group);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tablestyle_creation() {
        let style = TableStyle::new("TestStyle");
        assert_eq!(style.name, "TestStyle");
        assert!(style.description.is_empty());
    }

    #[test]
    fn test_standard_style() {
        let style = TableStyle::standard();
        assert_eq!(style.name, "Standard");
    }

    #[test]
    fn test_default_values() {
        let style = TableStyle::default();
        assert!((style.horizontal_margin - 0.06).abs() < 1e-10);
        assert!((style.vertical_margin - 0.06).abs() < 1e-10);
        assert!(!style.title_suppressed);
        assert!(!style.header_suppressed);
        assert_eq!(style.flow_direction, TableFlowDirection::Down);
    }

    #[test]
    fn test_data_row_style() {
        let style = TableStyle::default();
        assert!((style.data_row_style.text_height - 0.18).abs() < 1e-10);
        assert_eq!(style.data_row_style.alignment, CellAlignment::TopCenter);
    }

    #[test]
    fn test_header_row_style() {
        let style = TableStyle::default();
        assert!((style.header_row_style.text_height - 0.25).abs() < 1e-10);
        assert_eq!(style.header_row_style.alignment, CellAlignment::MiddleCenter);
    }

    #[test]
    fn test_title_row_style() {
        let style = TableStyle::default();
        assert!((style.title_row_style.text_height - 0.25).abs() < 1e-10);
    }

    #[test]
    fn test_flow_direction_enum() {
        assert_eq!(TableFlowDirection::from(0), TableFlowDirection::Down);
        assert_eq!(TableFlowDirection::from(1), TableFlowDirection::Up);
    }

    #[test]
    fn test_cell_alignment_enum() {
        assert_eq!(CellAlignment::from(1), CellAlignment::TopLeft);
        assert_eq!(CellAlignment::from(5), CellAlignment::MiddleCenter);
        assert_eq!(CellAlignment::from(9), CellAlignment::BottomRight);
        assert_eq!(CellAlignment::from(99), CellAlignment::MiddleCenter);
    }

    #[test]
    fn test_cell_alignment_helpers() {
        let top_left = CellAlignment::TopLeft;
        assert!(top_left.is_top());
        assert!(top_left.is_left());
        assert!(!top_left.is_center());
        assert!(!top_left.is_bottom());

        let middle_center = CellAlignment::MiddleCenter;
        assert!(middle_center.is_middle());
        assert!(middle_center.is_center());

        let bottom_right = CellAlignment::BottomRight;
        assert!(bottom_right.is_bottom());
        assert!(bottom_right.is_right());
    }

    #[test]
    fn test_set_margins() {
        let mut style = TableStyle::new("Test");
        style.set_margins(0.15);

        assert_eq!(style.horizontal_margin, 0.15);
        assert_eq!(style.vertical_margin, 0.15);
    }

    #[test]
    fn test_set_all_text_heights() {
        let mut style = TableStyle::new("Test");
        style.set_all_text_heights(0.3);

        assert_eq!(style.data_row_style.text_height, 0.3);
        assert_eq!(style.header_row_style.text_height, 0.3);
        assert_eq!(style.title_row_style.text_height, 0.3);
    }

    #[test]
    fn test_set_all_text_colors() {
        let mut style = TableStyle::new("Test");
        style.set_all_text_colors(Color::Index(1));

        assert_eq!(style.data_row_style.text_color, Color::Index(1));
        assert_eq!(style.header_row_style.text_color, Color::Index(1));
        assert_eq!(style.title_row_style.text_color, Color::Index(1));
    }

    #[test]
    fn test_set_all_fill_enabled() {
        let mut style = TableStyle::new("Test");
        style.set_all_fill_enabled(true);

        assert!(style.data_row_style.fill_enabled);
        assert!(style.header_row_style.fill_enabled);
        assert!(style.title_row_style.fill_enabled);
    }

    #[test]
    fn test_row_visibility() {
        let mut style = TableStyle::new("Test");

        assert!(style.has_title_row());
        assert!(style.has_header_row());

        style.set_title_visible(false);
        style.set_header_visible(false);

        assert!(!style.has_title_row());
        assert!(!style.has_header_row());
    }

    #[test]
    fn test_cell_border_creation() {
        let border = TableCellBorder::new();
        assert!(!border.is_invisible);
        assert_eq!(border.border_type, TableBorderType::Single);
    }

    #[test]
    fn test_cell_border_invisible() {
        let border = TableCellBorder::invisible();
        assert!(border.is_invisible);
    }

    #[test]
    fn test_cell_border_with_style() {
        let border = TableCellBorder::with_style(Color::Index(1), LineWeight::W0_50);
        assert_eq!(border.color, Color::Index(1));
        assert_eq!(border.line_weight, LineWeight::W0_50);
    }

    #[test]
    fn test_set_all_borders() {
        let mut style = RowCellStyle::new();
        let border = TableCellBorder::with_style(Color::Index(3), LineWeight::W1_00);
        style.set_all_borders(border);

        assert_eq!(style.left_border.color, Color::Index(3));
        assert_eq!(style.right_border.color, Color::Index(3));
        assert_eq!(style.top_border.color, Color::Index(3));
        assert_eq!(style.bottom_border.color, Color::Index(3));
    }

    #[test]
    fn test_set_all_borders_invisible() {
        let mut style = RowCellStyle::new();
        style.set_all_borders_invisible();

        assert!(style.left_border.is_invisible);
        assert!(style.right_border.is_invisible);
        assert!(style.top_border.is_invisible);
        assert!(style.bottom_border.is_invisible);
    }

    #[test]
    fn test_cell_style_property_flags() {
        let flags = TableCellStylePropertyFlags::ALIGNMENT
            | TableCellStylePropertyFlags::TEXT_HEIGHT
            | TableCellStylePropertyFlags::BACKGROUND_COLOR;

        assert!(flags.contains(TableCellStylePropertyFlags::ALIGNMENT));
        assert!(flags.contains(TableCellStylePropertyFlags::TEXT_HEIGHT));
        assert!(!flags.contains(TableCellStylePropertyFlags::ROTATION));
    }

    #[test]
    fn test_border_property_flags() {
        let flags = TableBorderPropertyFlags::COLOR | TableBorderPropertyFlags::LINE_WEIGHT;

        assert!(flags.contains(TableBorderPropertyFlags::COLOR));
        assert!(flags.contains(TableBorderPropertyFlags::LINE_WEIGHT));
        assert!(!flags.contains(TableBorderPropertyFlags::VISIBILITY));
    }

    #[test]
    fn test_table_style_flags() {
        let flags = TableStyleFlags::TITLE_SUPPRESSED;

        assert!(flags.contains(TableStyleFlags::TITLE_SUPPRESSED));
        assert!(!flags.contains(TableStyleFlags::HEADER_SUPPRESSED));
    }

    #[test]
    fn test_border_type_enum() {
        assert_eq!(TableBorderType::from(1), TableBorderType::Single);
        assert_eq!(TableBorderType::from(2), TableBorderType::Double);
    }
}
