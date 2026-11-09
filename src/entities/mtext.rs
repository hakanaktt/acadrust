//! Multi-line text entity

use super::{Entity, EntityCommon, LineSpacingStyle};
use crate::types::{BoundingBox3D, Color, Vector3};

/// Attachment point for MText
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AttachmentPoint {
    /// Top left
    TopLeft = 1,
    /// Top center
    TopCenter = 2,
    /// Top right
    TopRight = 3,
    /// Middle left
    MiddleLeft = 4,
    /// Middle center
    MiddleCenter = 5,
    /// Middle right
    MiddleRight = 6,
    /// Bottom left
    BottomLeft = 7,
    /// Bottom center
    BottomCenter = 8,
    /// Bottom right
    BottomRight = 9,
}

impl From<i16> for AttachmentPoint {
    /// DXF group 71 / embedded object group 70; unknown codes are top left.
    fn from(code: i16) -> Self {
        match code {
            2 => Self::TopCenter,
            3 => Self::TopRight,
            4 => Self::MiddleLeft,
            5 => Self::MiddleCenter,
            6 => Self::MiddleRight,
            7 => Self::BottomLeft,
            8 => Self::BottomCenter,
            9 => Self::BottomRight,
            _ => Self::TopLeft,
        }
    }
}

/// Drawing direction for MText
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DrawingDirection {
    /// Left to right
    LeftToRight = 1,
    /// Top to bottom (DXF 72 = 3 — the TEXT-era code set skips 2 and 4)
    TopToBottom = 3,
    /// By style (DXF 72 = 5)
    ByStyle = 5,
}

/// Column layout for an [`MText`] entity (stored in R2018+ DWG, non-annotative).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MTextColumnData {
    /// Column type: 0 = no columns, 1 = static columns, 2 = dynamic columns.
    pub column_type: i16,
    /// Number of columns. For dynamic, non-auto-height columns this is the
    /// number of per-column [`heights`](Self::heights); the DWG writer derives
    /// the on-disk count from `heights.len()` in that case to keep the object
    /// stream in sync, so keep the two consistent for dynamic columns.
    pub column_count: i32,
    /// Whether the column flow is reversed.
    pub flow_reversed: bool,
    /// Whether the column height is computed automatically.
    pub auto_height: bool,
    /// Column width.
    pub width: f64,
    /// Gutter width between columns.
    pub gutter: f64,
    /// Per-column heights. Only stored for dynamic, non-auto-height columns.
    pub heights: Vec<f64>,
}

impl MTextColumnData {
    /// Create empty (no-columns) column data.
    pub fn new() -> Self {
        MTextColumnData {
            column_type: 0,
            column_count: 0,
            flow_reversed: false,
            auto_height: false,
            width: 0.0,
            gutter: 0.0,
            heights: Vec::new(),
        }
    }
}

impl MTextColumnData {
    /// The ACAD extended data AutoCAD keeps R2007 to R2013 columns in:
    /// `ACAD_MTEXT_COLUMN_INFO_BEGIN`, pairs of a group code and its value
    /// (code 50 is followed by the height count and then the heights), and
    /// `ACAD_MTEXT_COLUMN_INFO_END`. None when there are no columns.
    pub(crate) fn to_acad_xdata(&self) -> Option<crate::xdata::ExtendedDataRecord> {
        use crate::xdata::{ExtendedDataRecord, XDataValue};
        if self.column_type == 0 {
            return None;
        }
        let manual_heights = self.column_type == 2 && !self.auto_height;
        let count = if manual_heights {
            self.heights.len()
        } else {
            self.column_count.max(0) as usize
        };
        let i = XDataValue::Integer16;
        let mut record = ExtendedDataRecord::new("ACAD");
        record.values = vec![
            XDataValue::String("ACAD_MTEXT_COLUMN_INFO_BEGIN".into()),
            i(75),
            i(self.column_type),
            i(79),
            i(i16::from(self.auto_height)),
            i(76),
            i(count.min(i16::MAX as usize) as i16),
            i(78),
            i(i16::from(self.flow_reversed)),
            i(48),
            XDataValue::Real(self.width),
            i(49),
            XDataValue::Real(self.gutter),
            i(50),
            i(if manual_heights {
                count.min(i16::MAX as usize) as i16
            } else {
                0
            }),
        ];
        if manual_heights {
            record.values.extend(
                self.heights
                    .iter()
                    .take(count)
                    .map(|h| XDataValue::Real(*h)),
            );
        }
        record
            .values
            .push(XDataValue::String("ACAD_MTEXT_COLUMN_INFO_END".into()));
        Some(record)
    }

    /// Whether `xdata` already holds an ACAD column block.
    pub(crate) fn in_acad_xdata(xdata: &crate::xdata::ExtendedData) -> bool {
        Self::acad_column_record(xdata).is_some()
    }

    fn acad_column_record(
        xdata: &crate::xdata::ExtendedData,
    ) -> Option<&crate::xdata::ExtendedDataRecord> {
        use crate::xdata::XDataValue;
        xdata.records().iter().find(|r| {
            r.application_name == "ACAD"
                && r.values.iter().any(
                    |v| matches!(v, XDataValue::String(s) if s == "ACAD_MTEXT_COLUMN_INFO_BEGIN"),
                )
        })
    }

    /// The columns of an ACAD column block (see [`Self::to_acad_xdata`]).
    pub(crate) fn from_acad_xdata(xdata: &crate::xdata::ExtendedData) -> Option<Self> {
        use crate::xdata::XDataValue;
        let record = Self::acad_column_record(xdata)?;
        let mut values = record
            .values
            .iter()
            .skip_while(
                |v| !matches!(v, XDataValue::String(s) if s == "ACAD_MTEXT_COLUMN_INFO_BEGIN"),
            )
            .skip(1);
        let mut columns = Self::new();
        while let Some(code) = values.next() {
            let XDataValue::Integer16(code) = code else {
                break; // the END marker, or something this block does not hold
            };
            match (code, values.next()?) {
                (75, XDataValue::Integer16(v)) => columns.column_type = *v,
                (79, XDataValue::Integer16(v)) => columns.auto_height = *v != 0,
                (76, XDataValue::Integer16(v)) => columns.column_count = i32::from(*v),
                (78, XDataValue::Integer16(v)) => columns.flow_reversed = *v != 0,
                (48, XDataValue::Real(v)) => columns.width = *v,
                (49, XDataValue::Real(v)) => columns.gutter = *v,
                (50, XDataValue::Integer16(v)) => {
                    for _ in 0..(*v).max(0) {
                        if let Some(XDataValue::Real(h)) = values.next() {
                            columns.heights.push(*h);
                        }
                    }
                }
                _ => {}
            }
        }
        (columns.column_type != 0).then_some(columns)
    }
}

impl Default for MTextColumnData {
    fn default() -> Self {
        Self::new()
    }
}

/// A multi-line text entity
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MText {
    /// Common entity data
    pub common: EntityCommon,
    /// Text content (may contain formatting codes)
    pub value: String,
    /// Insertion point
    pub insertion_point: Vector3,
    /// Text height
    pub height: f64,
    /// Reference rectangle width
    pub rectangle_width: f64,
    /// Reference rectangle height (optional)
    pub rectangle_height: Option<f64>,
    /// Rotation angle in radians
    pub rotation: f64,
    /// Exact DWG X-axis direction used to derive `rotation`. Kept only while
    /// it still describes the public rotation, avoiding needless last-bit
    /// drift from an `atan2` followed by `sin`/`cos`.
    #[cfg_attr(feature = "serde", serde(default))]
    pub dwg_x_direction: Option<Vector3>,
    /// Text style name
    pub style: String,
    /// Attachment point
    pub attachment_point: AttachmentPoint,
    /// Drawing direction
    pub drawing_direction: DrawingDirection,
    /// Line spacing factor
    pub line_spacing_factor: f64,
    /// Line spacing style (DXF 73): AtLeast (1) or Exactly (2).
    pub line_spacing_style: LineSpacingStyle,
    /// Normal vector
    pub normal: Vector3,
    /// Background fill flags (BL 90): bit 0x01 = use background fill color,
    /// 0x02 = use drawing window color, 0x10 = text frame (R2018+).
    pub background_fill_flags: i32,
    /// Background fill scale factor (BD 45). Default 1.5.
    pub background_scale: f64,
    /// Background fill color (CMC 63).
    pub background_color: Color,
    /// Background fill transparency (BL 441).
    pub background_transparency: i32,
    /// Whether this MTEXT is annotative (R2018+). When `false`, the DWG stores
    /// a block of redundant fields followed by column data.
    pub is_annotative: bool,
    /// Column layout data (R2018+).
    pub column_data: MTextColumnData,
    /// Horizontal extent of the laid-out text (DXF 42, output-only; DWG
    /// "extents width"). 0 when never laid out.
    #[cfg_attr(feature = "serde", serde(default))]
    pub extents_width: f64,
    /// Vertical extent of the laid-out text (DXF 43, output-only; DWG
    /// "extents height"). 0 when never laid out.
    #[cfg_attr(feature = "serde", serde(default))]
    pub extents_height: f64,
}

impl MText {
    /// Create a new MText entity
    pub fn new() -> Self {
        MText {
            common: EntityCommon::new(),
            value: String::new(),
            insertion_point: Vector3::ZERO,
            height: 1.0,
            rectangle_width: 10.0,
            rectangle_height: None,
            rotation: 0.0,
            dwg_x_direction: None,
            style: "Standard".to_string(),
            attachment_point: AttachmentPoint::TopLeft,
            drawing_direction: DrawingDirection::LeftToRight,
            line_spacing_factor: 1.0,
            line_spacing_style: LineSpacingStyle::AtLeast,
            normal: Vector3::UNIT_Z,
            background_fill_flags: 0,
            background_scale: 1.5,
            background_color: Color::ByLayer,
            background_transparency: 0,
            // Non-annotative by default. Real annotativeness is carried by the
            // annotation context / text style (and, from R2018 DWG on, an inline
            // entity bit the reader sets explicitly) — not by every fresh MTEXT.
            // DXF has no entity-level annotative flag at all, so without this
            // default it would mark every imported MTEXT annotative.
            is_annotative: false,
            column_data: MTextColumnData::new(),
            extents_width: 0.0,
            extents_height: 0.0,
        }
    }

    /// Create a new MText with value and position
    pub fn with_value(value: impl Into<String>, position: Vector3) -> Self {
        MText {
            value: value.into(),
            insertion_point: position,
            ..Self::new()
        }
    }

    /// Set the text height
    pub fn with_height(mut self, height: f64) -> Self {
        self.height = height;
        self
    }

    /// Set the rectangle width
    pub fn with_width(mut self, width: f64) -> Self {
        self.rectangle_width = width;
        self
    }
}

impl Default for MText {
    fn default() -> Self {
        Self::new()
    }
}

impl Entity for MText {
    fn common(&self) -> &super::EntityCommon {
        &self.common
    }

    fn common_mut(&mut self) -> &mut super::EntityCommon {
        &mut self.common
    }

    fn bounding_box(&self) -> BoundingBox3D {
        let height = self.rectangle_height.unwrap_or(self.height * 2.0);
        BoundingBox3D::new(
            self.insertion_point,
            Vector3::new(
                self.insertion_point.x + self.rectangle_width,
                self.insertion_point.y + height,
                self.insertion_point.z,
            ),
        )
    }

    fn translate(&mut self, offset: Vector3) {
        super::translate::translate_mtext(self, offset);
    }

    fn entity_type(&self) -> &'static str {
        "MTEXT"
    }

    fn apply_transform(&mut self, transform: &crate::types::Transform) {
        super::transform::transform_mtext(self, transform);
    }

    fn apply_mirror(&mut self, transform: &crate::types::Transform) {
        super::mirror::mirror_mtext(self, transform);
    }

    fn apply_mirror_with_options(
        &mut self,
        transform: &crate::types::Transform,
        options: super::MirrorOptions,
    ) {
        super::mirror::mirror_mtext_with_options(self, transform, options);
    }
}
