//! DWG object type codes.
//!
//! Some object types have fixed numeric values in the DWG format.
//! This enum contains those values, mirroring ACadSharp's `ObjectType`.
//!
//! Custom/proxy objects use class numbers above 500.

/// Fixed DWG object type codes.
///
/// These are the numeric type identifiers written into each object in the
/// AcDb:AcDbObjects section. Values above ~0x1F3 are class-based and looked
/// up in the AcDb:Classes section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum DwgObjectType {
    Unlisted = -999,
    Invalid = -1,
    Undefined = 0,
    Text = 1,
    Attrib = 2,
    Attdef = 3,
    Block = 4,
    Endblk = 5,
    Seqend = 6,
    Insert = 7,
    Minsert = 8,
    Unknown9 = 9,
    Vertex2D = 0x0A,
    Vertex3D = 0x0B,
    VertexMesh = 0x0C,
    VertexPface = 0x0D,
    VertexPfaceFace = 0x0E,
    Polyline2D = 0x0F,
    Polyline3D = 0x10,
    Arc = 0x11,
    Circle = 0x12,
    Line = 0x13,
    DimensionOrdinate = 0x14,
    DimensionLinear = 0x15,
    DimensionAligned = 0x16,
    DimensionAng3Pt = 0x17,
    DimensionAng2Ln = 0x18,
    DimensionRadius = 0x19,
    DimensionDiameter = 0x1A,
    Point = 0x1B,
    Face3D = 0x1C,
    PolylinePface = 0x1D,
    PolylineMesh = 0x1E,
    Solid = 0x1F,
    Trace = 0x20,
    Shape = 0x21,
    Viewport = 0x22,
    Ellipse = 0x23,
    Spline = 0x24,
    Region = 0x25,
    Solid3D = 0x26,
    Body = 0x27,
    Ray = 0x28,
    Xline = 0x29,
    Dictionary = 0x2A,
    OleFrame = 0x2B,
    Mtext = 0x2C,
    Leader = 0x2D,
    Tolerance = 0x2E,
    Mline = 0x2F,
    BlockControlObj = 0x30,
    BlockHeader = 0x31,
    LayerControlObj = 0x32,
    Layer = 0x33,
    StyleControlObj = 0x34,
    Style = 0x35,
    Unknown36 = 0x36,
    Unknown37 = 0x37,
    LtypeControlObj = 0x38,
    Ltype = 0x39,
    Unknown3A = 0x3A,
    Unknown3B = 0x3B,
    ViewControlObj = 0x3C,
    View = 0x3D,
    UcsControlObj = 0x3E,
    Ucs = 0x3F,
    VportControlObj = 0x40,
    Vport = 0x41,
    AppidControlObj = 0x42,
    Appid = 0x43,
    DimstyleControlObj = 0x44,
    Dimstyle = 0x45,
    VpEntHdrCtrlObj = 0x46,
    VpEntHdr = 0x47,
    Group = 0x48,
    MlineStyle = 0x49,
    Ole2Frame = 0x4A,
    Dummy = 0x4B,
    LongTransaction = 0x4C,
    LwPolyline = 0x4D,
    Hatch = 0x4E,
    XRecord = 0x4F,
    AcDbPlaceholder = 0x50,
    VbaProject = 0x51,
    Layout = 0x52,
    AcadProxyEntity = 0x1F2,
    AcadProxyObject = 0x1F3,
}

impl DwgObjectType {
    /// Create an `DwgObjectType` from a raw i16 code.
    ///
    /// Returns `Unlisted` for unknown codes (class-based objects).
    pub fn from_raw(value: i16) -> Self {
        match value {
            -1 => Self::Invalid,
            0 => Self::Undefined,
            1 => Self::Text,
            2 => Self::Attrib,
            3 => Self::Attdef,
            4 => Self::Block,
            5 => Self::Endblk,
            6 => Self::Seqend,
            7 => Self::Insert,
            8 => Self::Minsert,
            9 => Self::Unknown9,
            0x0A => Self::Vertex2D,
            0x0B => Self::Vertex3D,
            0x0C => Self::VertexMesh,
            0x0D => Self::VertexPface,
            0x0E => Self::VertexPfaceFace,
            0x0F => Self::Polyline2D,
            0x10 => Self::Polyline3D,
            0x11 => Self::Arc,
            0x12 => Self::Circle,
            0x13 => Self::Line,
            0x14 => Self::DimensionOrdinate,
            0x15 => Self::DimensionLinear,
            0x16 => Self::DimensionAligned,
            0x17 => Self::DimensionAng3Pt,
            0x18 => Self::DimensionAng2Ln,
            0x19 => Self::DimensionRadius,
            0x1A => Self::DimensionDiameter,
            0x1B => Self::Point,
            0x1C => Self::Face3D,
            0x1D => Self::PolylinePface,
            0x1E => Self::PolylineMesh,
            0x1F => Self::Solid,
            0x20 => Self::Trace,
            0x21 => Self::Shape,
            0x22 => Self::Viewport,
            0x23 => Self::Ellipse,
            0x24 => Self::Spline,
            0x25 => Self::Region,
            0x26 => Self::Solid3D,
            0x27 => Self::Body,
            0x28 => Self::Ray,
            0x29 => Self::Xline,
            0x2A => Self::Dictionary,
            0x2B => Self::OleFrame,
            0x2C => Self::Mtext,
            0x2D => Self::Leader,
            0x2E => Self::Tolerance,
            0x2F => Self::Mline,
            0x30 => Self::BlockControlObj,
            0x31 => Self::BlockHeader,
            0x32 => Self::LayerControlObj,
            0x33 => Self::Layer,
            0x34 => Self::StyleControlObj,
            0x35 => Self::Style,
            0x36 => Self::Unknown36,
            0x37 => Self::Unknown37,
            0x38 => Self::LtypeControlObj,
            0x39 => Self::Ltype,
            0x3A => Self::Unknown3A,
            0x3B => Self::Unknown3B,
            0x3C => Self::ViewControlObj,
            0x3D => Self::View,
            0x3E => Self::UcsControlObj,
            0x3F => Self::Ucs,
            0x40 => Self::VportControlObj,
            0x41 => Self::Vport,
            0x42 => Self::AppidControlObj,
            0x43 => Self::Appid,
            0x44 => Self::DimstyleControlObj,
            0x45 => Self::Dimstyle,
            0x46 => Self::VpEntHdrCtrlObj,
            0x47 => Self::VpEntHdr,
            0x48 => Self::Group,
            0x49 => Self::MlineStyle,
            0x4A => Self::Ole2Frame,
            0x4B => Self::Dummy,
            0x4C => Self::LongTransaction,
            0x4D => Self::LwPolyline,
            0x4E => Self::Hatch,
            0x4F => Self::XRecord,
            0x50 => Self::AcDbPlaceholder,
            0x51 => Self::VbaProject,
            0x52 => Self::Layout,
            0x1F2 => Self::AcadProxyEntity,
            0x1F3 => Self::AcadProxyObject,
            _ => Self::Unlisted,
        }
    }

    /// Get the raw i16 value.
    pub fn as_raw(self) -> i16 {
        self as i16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_raw_known() {
        assert_eq!(DwgObjectType::from_raw(0x13), DwgObjectType::Line);
        assert_eq!(DwgObjectType::from_raw(0x2A), DwgObjectType::Dictionary);
        assert_eq!(DwgObjectType::from_raw(0x52), DwgObjectType::Layout);
    }

    #[test]
    fn test_from_raw_unknown() {
        assert_eq!(DwgObjectType::from_raw(999), DwgObjectType::Unlisted);
    }

    #[test]
    fn test_as_raw() {
        assert_eq!(DwgObjectType::Line.as_raw(), 0x13);
        assert_eq!(DwgObjectType::AcadProxyEntity.as_raw(), 0x1F2);
    }
}
