//! Stub object types for DXF objects that need basic round-trip support.
//!
//! These are minimal representations of DXF objects that ACadSharp supports
//! but that don't require full rich data models for typical usage.

use crate::types::Handle;

/// Trait for minimal stub objects that only need handle + owner fields.
/// Used by the generic `read_stub_object` reader.
pub trait StubObject {
    /// Create a new default instance
    fn new_stub() -> Self;
    /// Set the object handle
    fn set_handle(&mut self, handle: Handle);
    /// Set the owner handle
    fn set_owner(&mut self, owner: Handle);
    /// Get the object handle
    fn handle(&self) -> Handle;
}

/// VisualStyle object — named visual rendering style
#[derive(Debug, Clone)]
pub struct VisualStyle {
    /// Unique handle
    pub handle: Handle,
    /// Owner handle
    pub owner: Handle,
    /// Description / name
    pub description: String,
    /// Style type (code 70)
    pub style_type: i16,
    /// Face lighting model (code 71)
    pub face_lighting_model: i16,
    /// Face lighting quality (code 72)
    pub face_lighting_quality: i16,
    /// Face color mode (code 73)
    pub face_color_mode: i16,
    /// Face modifier (code 90)
    pub face_modifier: i32,
    /// Edge model (code 91)
    pub edge_model: i32,
    /// Edge style (code 92)
    pub edge_style: i32,
    /// Internal use only flag (code 291)
    pub internal_use_only: bool,
}

impl VisualStyle {
    /// Create a new VisualStyle with defaults
    pub fn new() -> Self {
        VisualStyle {
            handle: Handle::NULL,
            owner: Handle::NULL,
            description: String::new(),
            style_type: 0,
            face_lighting_model: 0,
            face_lighting_quality: 0,
            face_color_mode: 0,
            face_modifier: 0,
            edge_model: 0,
            edge_style: 0,
            internal_use_only: false,
        }
    }
}

impl Default for VisualStyle {
    fn default() -> Self { Self::new() }
}

/// Material object — named material for 3D rendering
#[derive(Debug, Clone)]
pub struct Material {
    /// Unique handle
    pub handle: Handle,
    /// Owner handle
    pub owner: Handle,
    /// Material name
    pub name: String,
    /// Description
    pub description: String,
}

impl Material {
    /// Create a new Material with defaults
    pub fn new() -> Self {
        Material {
            handle: Handle::NULL,
            owner: Handle::NULL,
            name: String::new(),
            description: String::new(),
        }
    }
}

impl Default for Material {
    fn default() -> Self { Self::new() }
}

/// GeoData — geographic location data for a drawing
#[derive(Debug, Clone)]
pub struct GeoData {
    /// Unique handle
    pub handle: Handle,
    /// Owner handle
    pub owner: Handle,
    /// Object version (code 90)
    pub version: i32,
    /// Coordinate type (code 70): 0 = unknown, 1 = local grid, 2 = projected grid, 3 = geographic
    pub coordinate_type: i16,
}

impl GeoData {
    /// Create a new GeoData
    pub fn new() -> Self {
        GeoData {
            handle: Handle::NULL,
            owner: Handle::NULL,
            version: 2,
            coordinate_type: 0,
        }
    }
}

impl Default for GeoData {
    fn default() -> Self { Self::new() }
}

/// SpatialFilter — clip boundary for external references
#[derive(Debug, Clone)]
pub struct SpatialFilter {
    /// Unique handle
    pub handle: Handle,
    /// Owner handle
    pub owner: Handle,
}

impl SpatialFilter {
    /// Create a new SpatialFilter
    pub fn new() -> Self {
        SpatialFilter {
            handle: Handle::NULL,
            owner: Handle::NULL,
        }
    }
}

impl Default for SpatialFilter {
    fn default() -> Self { Self::new() }
}

/// RasterVariables — global raster image settings
#[derive(Debug, Clone)]
pub struct RasterVariables {
    /// Unique handle
    pub handle: Handle,
    /// Owner handle
    pub owner: Handle,
    /// Class version (code 90)
    pub class_version: i32,
    /// Image frame display (code 70): 0 = no frame, 1 = display frame
    pub display_image_frame: i16,
    /// Image quality (code 71): 0 = draft, 1 = high
    pub image_quality: i16,
    /// Units (code 72): 0 = none, 1 = mm, 2 = cm, 3 = m, 4 = km, 5 = in, 6 = ft, 7 = yd, 8 = mi
    pub units: i16,
}

impl RasterVariables {
    /// Create new RasterVariables
    pub fn new() -> Self {
        RasterVariables {
            handle: Handle::NULL,
            owner: Handle::NULL,
            class_version: 0,
            display_image_frame: 1,
            image_quality: 1,
            units: 0,
        }
    }
}

impl Default for RasterVariables {
    fn default() -> Self { Self::new() }
}

/// BookColor (DBCOLOR) — named color definition
#[derive(Debug, Clone)]
pub struct BookColor {
    /// Unique handle
    pub handle: Handle,
    /// Owner handle
    pub owner: Handle,
    /// Color name (code 1)
    pub color_name: String,
    /// Book name (code 2)
    pub book_name: String,
}

impl BookColor {
    /// Create a new BookColor
    pub fn new() -> Self {
        BookColor {
            handle: Handle::NULL,
            owner: Handle::NULL,
            color_name: String::new(),
            book_name: String::new(),
        }
    }
}

impl Default for BookColor {
    fn default() -> Self { Self::new() }
}

/// AcDbPlaceHolder — placeholder object (no data beyond handle)
#[derive(Debug, Clone)]
pub struct PlaceHolder {
    /// Unique handle
    pub handle: Handle,
    /// Owner handle
    pub owner: Handle,
}

impl PlaceHolder {
    /// Create a new PlaceHolder
    pub fn new() -> Self {
        PlaceHolder {
            handle: Handle::NULL,
            owner: Handle::NULL,
        }
    }
}

impl Default for PlaceHolder {
    fn default() -> Self { Self::new() }
}

/// DictionaryWithDefault — dictionary with a default entry handle
#[derive(Debug, Clone)]
pub struct DictionaryWithDefault {
    /// Unique handle
    pub handle: Handle,
    /// Owner handle
    pub owner: Handle,
    /// Dictionary entries (key -> handle)
    pub entries: Vec<(String, Handle)>,
    /// Default entry handle (code 340)
    pub default_handle: Handle,
    /// Duplicate record cloning flag (code 281)
    pub duplicate_cloning: i16,
    /// Hard owner flag (code 280)
    pub hard_owner: bool,
}

impl DictionaryWithDefault {
    /// Create a new DictionaryWithDefault
    pub fn new() -> Self {
        DictionaryWithDefault {
            handle: Handle::NULL,
            owner: Handle::NULL,
            entries: Vec::new(),
            default_handle: Handle::NULL,
            duplicate_cloning: 1,
            hard_owner: false,
        }
    }
}

impl Default for DictionaryWithDefault {
    fn default() -> Self { Self::new() }
}

/// WipeoutVariables — global wipeout display settings
#[derive(Debug, Clone)]
pub struct WipeoutVariables {
    /// Unique handle
    pub handle: Handle,
    /// Owner handle
    pub owner: Handle,
    /// Display image frame (code 70): 0 = no, 1 = yes
    pub display_frame: i16,
}

impl WipeoutVariables {
    /// Create new WipeoutVariables
    pub fn new() -> Self {
        WipeoutVariables {
            handle: Handle::NULL,
            owner: Handle::NULL,
            display_frame: 0,
        }
    }
}

impl Default for WipeoutVariables {
    fn default() -> Self { Self::new() }
}

// StubObject implementations for types that only need handle + owner parsing

macro_rules! impl_stub_object {
    ($ty:ident) => {
        impl StubObject for $ty {
            fn new_stub() -> Self { Self::new() }
            fn set_handle(&mut self, handle: Handle) { self.handle = handle; }
            fn set_owner(&mut self, owner: Handle) { self.owner = owner; }
            fn handle(&self) -> Handle { self.handle }
        }
    };
}

impl_stub_object!(GeoData);
impl_stub_object!(SpatialFilter);
impl_stub_object!(PlaceHolder);
