//! ACIS/SAT data types for solid modeler geometry.
//!
//! These types represent the parsed structure of ACIS SAT format data,
//! including the header, entity records, and the B-rep topology/geometry.

use std::fmt;

use super::parser::SatParser;
use super::writer::SatWriter;

// ============================================================================
// SAT Version
// ============================================================================

/// ACIS SAT version number.
///
/// Common versions:
/// - `(4, 0, 0)` → SAT version 400 (ACIS 4.0)
/// - `(7, 0, 0)` → SAT version 700 (ACIS 7.0)
/// - `(21, 0, 0)` → SAT version 21800 (ACIS 21.0)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SatVersion {
    /// Major version (e.g. 7 for ACIS 7.0).
    pub major: u32,
    /// Minor version (e.g. 0).
    pub minor: u32,
    /// Patch / sub-minor.
    pub patch: u32,
}

impl SatVersion {
    /// Creates a new SAT version.
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }

    /// ACIS 4.0 — legacy format.
    pub const V4_0: Self = Self { major: 4, minor: 0, patch: 0 };
    /// ACIS 7.0 — introduced explicit indices and asmheader.
    pub const V7_0: Self = Self { major: 7, minor: 0, patch: 0 };
    /// ACIS 21.0 — modern format.
    pub const V21_0: Self = Self { major: 21, minor: 0, patch: 0 };

    /// Returns the SAT version number used in the header line (e.g. 700).
    pub fn sat_version_number(&self) -> u32 {
        self.major * 100 + self.minor * 10 + self.patch
    }

    /// Creates a version from the SAT version number (e.g. 700 → 7.0.0).
    pub fn from_sat_number(num: u32) -> Self {
        Self {
            major: num / 100,
            minor: (num % 100) / 10,
            patch: num % 10,
        }
    }

    /// Returns true if this version uses explicit record indices (7.0+).
    pub fn has_explicit_indices(&self) -> bool {
        self.major >= 7
    }

    /// Returns true if this version uses `@`-prefixed counted strings (7.0+).
    pub fn has_counted_strings(&self) -> bool {
        self.major >= 7
    }

    /// Returns true if this version requires an `asmheader` entity (7.0+).
    pub fn has_asm_header(&self) -> bool {
        self.major >= 7
    }
}

impl Default for SatVersion {
    fn default() -> Self {
        Self::V7_0
    }
}

impl fmt::Display for SatVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

// ============================================================================
// SAT Header
// ============================================================================

/// Header of a SAT file.
///
/// The first 3 lines of a SAT file contain version info, product info,
/// and modeler tolerances.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SatHeader {
    /// ACIS version.
    pub version: SatVersion,
    /// Number of entity records.
    pub num_records: usize,
    /// Number of bodies.
    pub num_bodies: usize,
    /// Whether history data is present.
    pub has_history: bool,
    /// Product identifier string.
    pub product_id: String,
    /// Product version string.
    pub product_version: String,
    /// File creation date string.
    pub date: String,
    /// Spatial resolution (minimum edge length, typically 1e-06).
    pub spatial_resolution: f64,
    /// Normal tolerance (angular tolerance in radians, typically ~1e-07).
    pub normal_tolerance: f64,
    /// Fit tolerance for approximation (ACIS 7.0+, typically 1e-10).
    pub resfit_tolerance: Option<f64>,
}

impl SatHeader {
    /// Creates a default header for ACIS 7.0.
    pub fn new() -> Self {
        Self {
            version: SatVersion::V7_0,
            num_records: 0,
            num_bodies: 0,
            has_history: false,
            product_id: "acadrust".to_string(),
            product_version: "ACIS 7.0".to_string(),
            date: "Thu Jan 01 00:00:00 2023".to_string(),
            spatial_resolution: 10.0,
            normal_tolerance: 9.9999999999999995e-07,
            resfit_tolerance: Some(1e-10),
        }
    }

    /// Creates a header with the specified version.
    pub fn with_version(version: SatVersion) -> Self {
        let mut header = Self::new();
        header.version = version;
        header.product_version = format!("ACIS {}.{}", version.major, version.minor);
        header
    }
}

impl Default for SatHeader {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Entity Pointer
// ============================================================================

/// A pointer/reference to another SAT entity record.
///
/// In SAT text, pointers appear as `$<index>` (e.g. `$3`) or `$-1` for null.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SatPointer(pub i32);

impl SatPointer {
    /// Null pointer (`$-1`).
    pub const NULL: Self = Self(-1);

    /// Creates a pointer to the given record index.
    pub fn new(index: i32) -> Self {
        Self(index)
    }

    /// Returns true if this is a null pointer.
    pub fn is_null(&self) -> bool {
        self.0 < 0
    }

    /// Returns the index, or `None` if null.
    pub fn index(&self) -> Option<usize> {
        if self.0 >= 0 {
            Some(self.0 as usize)
        } else {
            None
        }
    }
}

impl Default for SatPointer {
    fn default() -> Self {
        Self::NULL
    }
}

impl fmt::Display for SatPointer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${}", self.0)
    }
}

// ============================================================================
// SAT Token
// ============================================================================

/// A single token in a SAT entity record.
///
/// SAT records consist of a sequence of tokens separated by spaces.
/// Tokens can be entity-type identifiers, pointers, numbers, strings,
/// or enum values.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SatToken {
    /// An identifier/keyword (entity type, sense, etc.).
    Ident(String),
    /// A pointer reference (`$<index>`).
    Pointer(SatPointer),
    /// An integer value.
    Integer(i64),
    /// A floating-point value.
    Float(f64),
    /// A counted string (`@<len> <text>`).
    String(String),
    /// A position/point value `(x y z)`.
    Position(f64, f64, f64),
    /// The boolean true literal.
    True,
    /// The boolean false literal.
    False,
    /// The record terminator `#`.
    Terminator,
    /// An enum-like keyword (forward, reversed, single, double, etc.).
    Enum(String),
}

impl SatToken {
    /// Returns the token as a string if it is an identifier.
    pub fn as_ident(&self) -> Option<&str> {
        match self {
            SatToken::Ident(s) | SatToken::Enum(s) => Some(s),
            _ => None,
        }
    }

    /// Returns the token as a pointer if it is one.
    pub fn as_pointer(&self) -> Option<SatPointer> {
        match self {
            SatToken::Pointer(p) => Some(*p),
            _ => None,
        }
    }

    /// Returns the token as an integer if it is one.
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            SatToken::Integer(v) => Some(*v),
            _ => None,
        }
    }

    /// Returns the token as a float if it is one.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            SatToken::Float(v) => Some(*v),
            SatToken::Integer(v) => Some(*v as f64),
            _ => None,
        }
    }

    /// Returns the token as a string value.
    pub fn as_string(&self) -> Option<&str> {
        match self {
            SatToken::String(s) => Some(s),
            _ => None,
        }
    }
}

impl fmt::Display for SatToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SatToken::Ident(s) => write!(f, "{}", s),
            SatToken::Pointer(p) => write!(f, "{}", p),
            SatToken::Integer(v) => write!(f, "{}", v),
            SatToken::Float(v) => {
                if v.fract() == 0.0 && !v.is_infinite() && !v.is_nan() {
                    write!(f, "{:.1}", v)
                } else {
                    write!(f, "{}", v)
                }
            }
            SatToken::String(s) => write!(f, "@{} {}", s.len(), s),
            SatToken::Position(x, y, z) => write!(f, "{} {} {}", x, y, z),
            SatToken::True => write!(f, "TRUE"),
            SatToken::False => write!(f, "FALSE"),
            SatToken::Terminator => write!(f, "#"),
            SatToken::Enum(s) => write!(f, "{}", s),
        }
    }
}

// ============================================================================
// SAT Entity Record
// ============================================================================

/// A single entity record in a SAT file.
///
/// Each record represents a topological or geometric entity in the B-rep model.
/// Records are terminated by `#` in the text format.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SatRecord {
    /// Record index (explicit in ACIS 7.0+, implicit/sequential in earlier).
    pub index: i32,
    /// Entity type name (e.g. "body", "face", "plane-surface").
    pub entity_type: String,
    /// Entity sub-type (for entities like "xxx-surface", "xxx-curve").
    pub sub_type: Option<String>,
    /// Attribute pointer (first pointer after entity type).
    pub attribute: SatPointer,
    /// Subtype/ID field (integer after attribute, always -1 for standard entities).
    pub subtype_id: i32,
    /// Remaining tokens in the record (after entity type, attribute, and subtype_id).
    pub tokens: Vec<SatToken>,
    /// Raw text of the record (preserved for roundtrip fidelity).
    pub raw_text: Option<String>,
}

impl SatRecord {
    /// Creates a new empty record.
    pub fn new(index: i32, entity_type: &str) -> Self {
        Self {
            index,
            entity_type: entity_type.to_string(),
            sub_type: None,
            attribute: SatPointer::NULL,
            subtype_id: -1,
            tokens: Vec::new(),
            raw_text: None,
        }
    }

    /// Returns all pointer references in this record.
    pub fn pointers(&self) -> Vec<SatPointer> {
        let mut ptrs = vec![self.attribute];
        for token in &self.tokens {
            if let SatToken::Pointer(p) = token {
                ptrs.push(*p);
            }
        }
        ptrs
    }

    /// Returns the token at the given position (0-based, after the attribute).
    pub fn token(&self, index: usize) -> Option<&SatToken> {
        self.tokens.get(index)
    }

    /// Returns the integer value of the token at the given position.
    pub fn token_integer(&self, index: usize) -> Option<i64> {
        self.tokens.get(index).and_then(|t| t.as_integer())
    }

    /// Returns the float value of the token at the given position.
    pub fn token_float(&self, index: usize) -> Option<f64> {
        // Coordinate-aware flat-float indexing. SAT text stores a point/vector
        // as three separate float tokens, so the geometry accessors read
        // px,py,pz / nx,ny,nz at consecutive indices. SAB/ASM (AutoCAD 2013+)
        // packs each coordinate triple into ONE Position/Direction token, so
        // expand those into three consecutive float slots here. Records with
        // no Position tokens (all SAT, most SAB scalars) index 1:1 as before.
        let mut slot = 0usize;
        for t in &self.tokens {
            if let SatToken::Position(x, y, z) = t {
                for &c in &[*x, *y, *z] {
                    if slot == index {
                        return Some(c);
                    }
                    slot += 1;
                }
            } else {
                if slot == index {
                    return t.as_float();
                }
                slot += 1;
            }
        }
        None
    }

    /// Returns the pointer at the given token position.
    pub fn token_pointer(&self, index: usize) -> Option<SatPointer> {
        self.tokens.get(index).and_then(|t| t.as_pointer())
    }

    /// Pointer by ordinal: the `index`-th pointer token, skipping interleaved
    /// scalar tokens. Most accessors use absolute positions (`token_pointer`)
    /// because ACIS keeps parameters in fixed slots, but a few records gained
    /// an extra scalar field in ASM (ShapeManager, AutoCAD 2013+) — e.g. a
    /// vertex's tolerance int between its edge and point — where ordinal
    /// indexing stays correct for both ACIS and ASM.
    pub fn nth_pointer(&self, index: usize) -> Option<SatPointer> {
        self.tokens
            .iter()
            .filter_map(|t| t.as_pointer())
            .nth(index)
    }

    /// Returns the string value at the given token position.
    pub fn token_string(&self, index: usize) -> Option<&str> {
        self.tokens.get(index).and_then(|t| match t {
            SatToken::String(s) | SatToken::Ident(s) | SatToken::Enum(s) => Some(s.as_str()),
            _ => None,
        })
    }
}

impl fmt::Display for SatRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}", self.index, self.entity_type, self.attribute)?;
        for token in &self.tokens {
            write!(f, " {}", token)?;
        }
        write!(f, " #")
    }
}

// ============================================================================
// SAT Entity Types (Typed Accessors)
// ============================================================================

/// The sense of a surface normal or curve direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Sense {
    /// Forward sense.
    Forward,
    /// Reversed sense.
    Reversed,
}

impl Sense {
    /// Parse from SAT token.
    pub fn from_str(s: &str) -> Self {
        match s {
            "reversed" | "REVERSED" => Self::Reversed,
            _ => Self::Forward,
        }
    }

    /// Returns the SAT string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Forward => "forward",
            Self::Reversed => "reversed",
        }
    }
}

impl Default for Sense {
    fn default() -> Self {
        Self::Forward
    }
}

/// Surface sidedness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Sidedness {
    /// Single-sided surface.
    Single,
    /// Double-sided surface.
    Double,
}

impl Sidedness {
    /// Parse from SAT token.
    pub fn from_str(s: &str) -> Self {
        match s {
            "double" | "DOUBLE" => Self::Double,
            _ => Self::Single,
        }
    }

    /// Returns the SAT string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Double => "double",
        }
    }
}

impl Default for Sidedness {
    fn default() -> Self {
        Self::Single
    }
}

// ============================================================================
// Typed Entity Accessors
// ============================================================================

/// Accessor for a `body` entity record.
///
/// Body record layout: `body $<attrib> <id> $<next_body> $<lump> $<wire> $<transform>`
#[derive(Debug, Clone)]
pub struct SatBody<'a> {
    record: &'a SatRecord,
}

impl<'a> SatBody<'a> {
    /// Wraps a record as a body entity. Returns None if not a body.
    pub fn from_record(record: &'a SatRecord) -> Option<Self> {
        if record.entity_type == "body" {
            Some(Self { record })
        } else {
            None
        }
    }

    /// Pointer to the next body.
    pub fn next_body(&self) -> SatPointer {
        self.record.token_pointer(0).unwrap_or(SatPointer::NULL)
    }

    /// Pointer to the first lump.
    pub fn lump(&self) -> SatPointer {
        self.record.token_pointer(1).unwrap_or(SatPointer::NULL)
    }

    /// Pointer to the wire body (if any).
    pub fn wire_body(&self) -> SatPointer {
        self.record.token_pointer(2).unwrap_or(SatPointer::NULL)
    }

    /// Pointer to the transform.
    pub fn transform(&self) -> SatPointer {
        self.record.token_pointer(3).unwrap_or(SatPointer::NULL)
    }
}

/// Accessor for a `lump` entity record.
///
/// Lump record layout: `lump $<attrib> <id> $<next_lump> $<unknown> $<shell> $<body>`
#[derive(Debug, Clone)]
pub struct SatLump<'a> {
    record: &'a SatRecord,
}

impl<'a> SatLump<'a> {
    /// Wraps a record as a lump entity. Returns None if not a lump.
    pub fn from_record(record: &'a SatRecord) -> Option<Self> {
        if record.entity_type == "lump" {
            Some(Self { record })
        } else {
            None
        }
    }

    /// Pointer to the next lump.
    pub fn next_lump(&self) -> SatPointer {
        self.record.token_pointer(0).unwrap_or(SatPointer::NULL)
    }

    /// Pointer to the shell.
    pub fn shell(&self) -> SatPointer {
        self.record.token_pointer(2).unwrap_or(SatPointer::NULL)
    }

    /// Pointer to the owner body.
    pub fn body(&self) -> SatPointer {
        self.record.token_pointer(3).unwrap_or(SatPointer::NULL)
    }
}

/// Accessor for a `shell` entity record.
///
/// Shell record layout: `shell $<attrib> <id> $<next_shell> $<subshell> $<unknown> $<face> $<wire> $<lump>`
#[derive(Debug, Clone)]
pub struct SatShell<'a> {
    record: &'a SatRecord,
}

impl<'a> SatShell<'a> {
    /// Wraps a record as a shell entity. Returns None if not a shell.
    pub fn from_record(record: &'a SatRecord) -> Option<Self> {
        if record.entity_type == "shell" {
            Some(Self { record })
        } else {
            None
        }
    }

    /// Pointer to the next shell.
    pub fn next_shell(&self) -> SatPointer {
        self.record.token_pointer(0).unwrap_or(SatPointer::NULL)
    }

    /// Pointer to the subshell.
    pub fn subshell(&self) -> SatPointer {
        self.record.token_pointer(1).unwrap_or(SatPointer::NULL)
    }

    /// Pointer to the first face.
    pub fn face(&self) -> SatPointer {
        self.record.token_pointer(3).unwrap_or(SatPointer::NULL)
    }

    /// Pointer to the wire.
    pub fn wire(&self) -> SatPointer {
        self.record.token_pointer(4).unwrap_or(SatPointer::NULL)
    }

    /// Pointer to the owner lump.
    pub fn lump(&self) -> SatPointer {
        self.record.token_pointer(5).unwrap_or(SatPointer::NULL)
    }
}

/// Accessor for a `face` entity record.
///
/// Face record layout: `face $<attrib> <id> $<unknown> $<next_face> $<loop> $<shell> $<subshell> $<surface> <sense> <sidedness>`
#[derive(Debug, Clone)]
pub struct SatFace<'a> {
    record: &'a SatRecord,
}

impl<'a> SatFace<'a> {
    /// Wraps a record as a face entity.
    pub fn from_record(record: &'a SatRecord) -> Option<Self> {
        if record.entity_type == "face" {
            Some(Self { record })
        } else {
            None
        }
    }

    /// Pointer to the next face.
    pub fn next_face(&self) -> SatPointer {
        self.record.token_pointer(1).unwrap_or(SatPointer::NULL)
    }

    /// Pointer to the first loop.
    pub fn first_loop(&self) -> SatPointer {
        self.record.token_pointer(2).unwrap_or(SatPointer::NULL)
    }

    /// Pointer to the owner shell.
    pub fn shell(&self) -> SatPointer {
        self.record.token_pointer(3).unwrap_or(SatPointer::NULL)
    }

    /// Pointer to the subshell.
    pub fn subshell(&self) -> SatPointer {
        self.record.token_pointer(4).unwrap_or(SatPointer::NULL)
    }

    /// Pointer to the surface geometry.
    pub fn surface(&self) -> SatPointer {
        self.record.token_pointer(5).unwrap_or(SatPointer::NULL)
    }

    /// Surface sense.
    pub fn sense(&self) -> Sense {
        self.record
            .token_string(6)
            .map(Sense::from_str)
            .unwrap_or_default()
    }

    /// Surface sidedness.
    pub fn sidedness(&self) -> Sidedness {
        self.record
            .token_string(7)
            .map(Sidedness::from_str)
            .unwrap_or_default()
    }
}

/// Accessor for a `loop` entity record.
///
/// Loop record layout: `loop $<attrib> <id> $<next_loop> $<unknown> $<coedge> $<face>`
#[derive(Debug, Clone)]
pub struct SatLoop<'a> {
    record: &'a SatRecord,
}

impl<'a> SatLoop<'a> {
    /// Wraps a record as a loop entity.
    pub fn from_record(record: &'a SatRecord) -> Option<Self> {
        if record.entity_type == "loop" {
            Some(Self { record })
        } else {
            None
        }
    }

    /// Pointer to the next loop in the face's loop list (the inner/hole loops
    /// follow the outer boundary here). The first token is a leading pointer
    /// (loop kind / unused); the next-loop link is the second.
    pub fn next_loop(&self) -> SatPointer {
        self.record.token_pointer(1).unwrap_or(SatPointer::NULL)
    }

    /// Pointer to the first coedge.
    pub fn first_coedge(&self) -> SatPointer {
        self.record.token_pointer(2).unwrap_or(SatPointer::NULL)
    }

    /// Pointer to the owner face.
    pub fn face(&self) -> SatPointer {
        self.record.token_pointer(3).unwrap_or(SatPointer::NULL)
    }
}

/// Accessor for a `coedge` entity record.
///
/// Coedge record layout: `coedge $<attrib> $<next> $<prev> $<partner> $<edge> <sense> $<loop>`
#[derive(Debug, Clone)]
pub struct SatCoedge<'a> {
    record: &'a SatRecord,
}

impl<'a> SatCoedge<'a> {
    /// Wraps a record as a coedge entity.
    pub fn from_record(record: &'a SatRecord) -> Option<Self> {
        if record.entity_type == "coedge" {
            Some(Self { record })
        } else {
            None
        }
    }

    /// Pointer to the next coedge in the loop.
    pub fn next(&self) -> SatPointer {
        self.record.token_pointer(1).unwrap_or(SatPointer::NULL)
    }

    /// Pointer to the previous coedge in the loop.
    pub fn prev(&self) -> SatPointer {
        self.record.token_pointer(2).unwrap_or(SatPointer::NULL)
    }

    /// Pointer to the partner coedge (on adjacent face).
    pub fn partner(&self) -> SatPointer {
        self.record.token_pointer(3).unwrap_or(SatPointer::NULL)
    }

    /// Pointer to the edge.
    pub fn edge(&self) -> SatPointer {
        self.record.token_pointer(4).unwrap_or(SatPointer::NULL)
    }

    /// Sense of this coedge relative to the edge.
    pub fn sense(&self) -> Sense {
        self.record
            .token_string(5)
            .map(Sense::from_str)
            .unwrap_or_default()
    }

    /// Pointer to the owner loop.
    pub fn owner_loop(&self) -> SatPointer {
        self.record.token_pointer(6).unwrap_or(SatPointer::NULL)
    }
}

/// Accessor for an `edge` entity record.
///
/// Edge record layout: `edge $<attrib> $<start_vertex> $<end_vertex> $<coedge> $<curve> <sense>`
#[derive(Debug, Clone)]
pub struct SatEdge<'a> {
    record: &'a SatRecord,
}

impl<'a> SatEdge<'a> {
    /// Wraps a record as an edge entity.
    pub fn from_record(record: &'a SatRecord) -> Option<Self> {
        if record.entity_type == "edge" {
            Some(Self { record })
        } else {
            None
        }
    }

    /// Pointer to the start vertex.
    pub fn start_vertex(&self) -> SatPointer {
        self.record.token_pointer(1).unwrap_or(SatPointer::NULL)
    }

    /// Start parameter along the curve.
    pub fn start_param(&self) -> f64 {
        self.record.token_float(2).unwrap_or(0.0)
    }

    /// Pointer to the end vertex.
    pub fn end_vertex(&self) -> SatPointer {
        self.record.token_pointer(3).unwrap_or(SatPointer::NULL)
    }

    /// End parameter along the curve.
    pub fn end_param(&self) -> f64 {
        self.record.token_float(4).unwrap_or(0.0)
    }

    /// Pointer to the first coedge using this edge.
    pub fn coedge(&self) -> SatPointer {
        self.record.token_pointer(5).unwrap_or(SatPointer::NULL)
    }

    /// Pointer to the curve geometry.
    pub fn curve(&self) -> SatPointer {
        self.record.token_pointer(6).unwrap_or(SatPointer::NULL)
    }

    /// Edge sense.
    pub fn sense(&self) -> Sense {
        self.record
            .token_string(7)
            .map(Sense::from_str)
            .unwrap_or_default()
    }
}

/// Accessor for a `vertex` entity record.
///
/// Vertex record layout: `vertex $<attrib> $<edge> $<point>`
#[derive(Debug, Clone)]
pub struct SatVertex<'a> {
    record: &'a SatRecord,
}

impl<'a> SatVertex<'a> {
    /// Wraps a record as a vertex entity.
    pub fn from_record(record: &'a SatRecord) -> Option<Self> {
        if record.entity_type == "vertex" {
            Some(Self { record })
        } else {
            None
        }
    }

    /// Pointer to the edge.
    pub fn edge(&self) -> SatPointer {
        self.record.nth_pointer(1).unwrap_or(SatPointer::NULL)
    }

    /// Pointer to the point geometry.
    ///
    /// Uses pointer-ordinal indexing so the ASM (AutoCAD 2013+) vertex layout
    /// `$attr $edge <tolerance:int> $point` resolves the same as the classic
    /// ACIS `$attr $edge $point`.
    pub fn point(&self) -> SatPointer {
        self.record.nth_pointer(2).unwrap_or(SatPointer::NULL)
    }
}

// ============================================================================
// Geometry Entity Accessors
// ============================================================================

/// Accessor for a `point` entity record.
///
/// Point record layout: `point $<attrib> <x> <y> <z>`
#[derive(Debug, Clone)]
pub struct SatPoint<'a> {
    record: &'a SatRecord,
}

impl<'a> SatPoint<'a> {
    /// Wraps a record as a point entity.
    pub fn from_record(record: &'a SatRecord) -> Option<Self> {
        if record.entity_type == "point" {
            Some(Self { record })
        } else {
            None
        }
    }

    /// Returns the coordinates as (x, y, z).
    pub fn position(&self) -> (f64, f64, f64) {
        let x = self.record.token_float(1).unwrap_or(0.0);
        let y = self.record.token_float(2).unwrap_or(0.0);
        let z = self.record.token_float(3).unwrap_or(0.0);
        (x, y, z)
    }
}

/// Accessor for a `straight-curve` entity record.
///
/// Straight-curve layout: `straight-curve $<attrib> <px> <py> <pz> <dx> <dy> <dz> ...`
#[derive(Debug, Clone)]
pub struct SatStraightCurve<'a> {
    record: &'a SatRecord,
}

impl<'a> SatStraightCurve<'a> {
    /// Wraps a record as a straight-curve entity.
    pub fn from_record(record: &'a SatRecord) -> Option<Self> {
        if record.entity_type == "straight-curve" {
            Some(Self { record })
        } else {
            None
        }
    }

    /// Root point (position on the line).
    pub fn root_point(&self) -> (f64, f64, f64) {
        let x = self.record.token_float(1).unwrap_or(0.0);
        let y = self.record.token_float(2).unwrap_or(0.0);
        let z = self.record.token_float(3).unwrap_or(0.0);
        (x, y, z)
    }

    /// Direction vector.
    pub fn direction(&self) -> (f64, f64, f64) {
        let x = self.record.token_float(4).unwrap_or(0.0);
        let y = self.record.token_float(5).unwrap_or(0.0);
        let z = self.record.token_float(6).unwrap_or(1.0);
        (x, y, z)
    }
}

/// Accessor for an `ellipse-curve` entity record.
///
/// Ellipse-curve layout: `ellipse-curve $<attrib> <cx> <cy> <cz> <nx> <ny> <nz> <major_x> <major_y> <major_z> <ratio> ...`
#[derive(Debug, Clone)]
pub struct SatEllipseCurve<'a> {
    record: &'a SatRecord,
}

impl<'a> SatEllipseCurve<'a> {
    /// Wraps a record as an ellipse-curve entity.
    pub fn from_record(record: &'a SatRecord) -> Option<Self> {
        if record.entity_type == "ellipse-curve" {
            Some(Self { record })
        } else {
            None
        }
    }

    /// Center point.
    pub fn center(&self) -> (f64, f64, f64) {
        let x = self.record.token_float(1).unwrap_or(0.0);
        let y = self.record.token_float(2).unwrap_or(0.0);
        let z = self.record.token_float(3).unwrap_or(0.0);
        (x, y, z)
    }

    /// Normal vector.
    pub fn normal(&self) -> (f64, f64, f64) {
        let x = self.record.token_float(4).unwrap_or(0.0);
        let y = self.record.token_float(5).unwrap_or(0.0);
        let z = self.record.token_float(6).unwrap_or(1.0);
        (x, y, z)
    }

    /// Major axis direction.
    pub fn major_axis(&self) -> (f64, f64, f64) {
        let x = self.record.token_float(7).unwrap_or(1.0);
        let y = self.record.token_float(8).unwrap_or(0.0);
        let z = self.record.token_float(9).unwrap_or(0.0);
        (x, y, z)
    }

    /// Ratio of minor to major axis.
    pub fn ratio(&self) -> f64 {
        self.record.token_float(10).unwrap_or(1.0)
    }
}

/// Accessor for a `plane-surface` entity record.
///
/// Plane-surface layout: `plane-surface $<attrib> <px> <py> <pz> <nx> <ny> <nz> <ux> <uy> <uz> ...`
#[derive(Debug, Clone)]
pub struct SatPlaneSurface<'a> {
    record: &'a SatRecord,
}

impl<'a> SatPlaneSurface<'a> {
    /// Wraps a record as a plane-surface entity.
    pub fn from_record(record: &'a SatRecord) -> Option<Self> {
        if record.entity_type == "plane-surface" {
            Some(Self { record })
        } else {
            None
        }
    }

    /// Root point on the plane.
    pub fn root_point(&self) -> (f64, f64, f64) {
        let x = self.record.token_float(1).unwrap_or(0.0);
        let y = self.record.token_float(2).unwrap_or(0.0);
        let z = self.record.token_float(3).unwrap_or(0.0);
        (x, y, z)
    }

    /// Normal vector.
    pub fn normal(&self) -> (f64, f64, f64) {
        let x = self.record.token_float(4).unwrap_or(0.0);
        let y = self.record.token_float(5).unwrap_or(0.0);
        let z = self.record.token_float(6).unwrap_or(1.0);
        (x, y, z)
    }

    /// U direction on the surface.
    pub fn u_direction(&self) -> (f64, f64, f64) {
        let x = self.record.token_float(7).unwrap_or(1.0);
        let y = self.record.token_float(8).unwrap_or(0.0);
        let z = self.record.token_float(9).unwrap_or(0.0);
        (x, y, z)
    }
}

/// Accessor for a `cone-surface` entity record.
///
/// Cone-surface layout (v700):
/// `cone-surface $<attrib> -1 $-1 <cx> <cy> <cz> <ax_x> <ax_y> <ax_z> <rx> <ry> <rz> <ratio> I I <sin_half_angle> <cos_half_angle> <radius> forward_v I I I I`
///
/// Tokens 11–12 are spline continuation markers (`I`), so sine/cosine
/// sit at positions 13 and 14. For a cylinder, sin=0 and cos=1.
#[derive(Debug, Clone)]
pub struct SatConeSurface<'a> {
    record: &'a SatRecord,
}

impl<'a> SatConeSurface<'a> {
    /// Wraps a record as a cone-surface entity.
    pub fn from_record(record: &'a SatRecord) -> Option<Self> {
        if record.entity_type == "cone-surface" {
            Some(Self { record })
        } else {
            None
        }
    }

    /// Center point (apex or center of base circle).
    pub fn center(&self) -> (f64, f64, f64) {
        let x = self.record.token_float(1).unwrap_or(0.0);
        let y = self.record.token_float(2).unwrap_or(0.0);
        let z = self.record.token_float(3).unwrap_or(0.0);
        (x, y, z)
    }

    /// Axis direction.
    pub fn axis(&self) -> (f64, f64, f64) {
        let x = self.record.token_float(4).unwrap_or(0.0);
        let y = self.record.token_float(5).unwrap_or(0.0);
        let z = self.record.token_float(6).unwrap_or(1.0);
        (x, y, z)
    }

    /// Major radius direction.
    pub fn major_axis(&self) -> (f64, f64, f64) {
        let x = self.record.token_float(7).unwrap_or(1.0);
        let y = self.record.token_float(8).unwrap_or(0.0);
        let z = self.record.token_float(9).unwrap_or(0.0);
        (x, y, z)
    }

    /// Ratio of minor to major radius.
    pub fn ratio(&self) -> f64 {
        self.record.token_float(10).unwrap_or(1.0)
    }

    /// Sine of half angle (position 13, after two `I` continuation tokens).
    /// For a cylinder this is 0.0.
    pub fn sin_half_angle(&self) -> f64 {
        self.record.token_float(13).unwrap_or(0.0)
    }

    /// Cosine of half angle (position 14, after two `I` continuation tokens).
    /// For a cylinder this is 1.0.
    pub fn cos_half_angle(&self) -> f64 {
        self.record.token_float(14).unwrap_or(1.0)
    }

    /// Radius at the reference cross-section (position 15).
    pub fn radius(&self) -> f64 {
        self.record.token_float(15).unwrap_or(1.0)
    }
}

/// Accessor for a `sphere-surface` entity record.
///
/// Sphere-surface layout: `sphere-surface $<attrib> <cx> <cy> <cz> <radius> <ux> <uy> <uz> <pole_x> <pole_y> <pole_z> ...`
#[derive(Debug, Clone)]
pub struct SatSphereSurface<'a> {
    record: &'a SatRecord,
}

impl<'a> SatSphereSurface<'a> {
    /// Wraps a record as a sphere-surface entity.
    pub fn from_record(record: &'a SatRecord) -> Option<Self> {
        if record.entity_type == "sphere-surface" {
            Some(Self { record })
        } else {
            None
        }
    }

    /// Center point.
    pub fn center(&self) -> (f64, f64, f64) {
        let x = self.record.token_float(1).unwrap_or(0.0);
        let y = self.record.token_float(2).unwrap_or(0.0);
        let z = self.record.token_float(3).unwrap_or(0.0);
        (x, y, z)
    }

    /// Radius.
    pub fn radius(&self) -> f64 {
        self.record.token_float(4).unwrap_or(1.0)
    }

    /// U direction.
    pub fn u_direction(&self) -> (f64, f64, f64) {
        let x = self.record.token_float(5).unwrap_or(1.0);
        let y = self.record.token_float(6).unwrap_or(0.0);
        let z = self.record.token_float(7).unwrap_or(0.0);
        (x, y, z)
    }

    /// Pole direction.
    pub fn pole(&self) -> (f64, f64, f64) {
        let x = self.record.token_float(8).unwrap_or(0.0);
        let y = self.record.token_float(9).unwrap_or(0.0);
        let z = self.record.token_float(10).unwrap_or(1.0);
        (x, y, z)
    }
}

/// Accessor for a `torus-surface` entity record.
///
/// Torus-surface layout: `torus-surface $<attrib> <cx> <cy> <cz> <nx> <ny> <nz> <major_r> <minor_r> <ux> <uy> <uz> ...`
#[derive(Debug, Clone)]
pub struct SatTorusSurface<'a> {
    record: &'a SatRecord,
}

impl<'a> SatTorusSurface<'a> {
    /// Wraps a record as a torus-surface entity.
    pub fn from_record(record: &'a SatRecord) -> Option<Self> {
        if record.entity_type == "torus-surface" {
            Some(Self { record })
        } else {
            None
        }
    }

    /// Center point.
    pub fn center(&self) -> (f64, f64, f64) {
        let x = self.record.token_float(1).unwrap_or(0.0);
        let y = self.record.token_float(2).unwrap_or(0.0);
        let z = self.record.token_float(3).unwrap_or(0.0);
        (x, y, z)
    }

    /// Normal (axis of revolution).
    pub fn normal(&self) -> (f64, f64, f64) {
        let x = self.record.token_float(4).unwrap_or(0.0);
        let y = self.record.token_float(5).unwrap_or(0.0);
        let z = self.record.token_float(6).unwrap_or(1.0);
        (x, y, z)
    }

    /// Major radius (distance from center to tube center).
    pub fn major_radius(&self) -> f64 {
        self.record.token_float(7).unwrap_or(1.0)
    }

    /// Minor radius (tube radius).
    pub fn minor_radius(&self) -> f64 {
        self.record.token_float(8).unwrap_or(0.5)
    }

    /// U direction.
    pub fn u_direction(&self) -> (f64, f64, f64) {
        let x = self.record.token_float(9).unwrap_or(1.0);
        let y = self.record.token_float(10).unwrap_or(0.0);
        let z = self.record.token_float(11).unwrap_or(0.0);
        (x, y, z)
    }
}

/// Accessor for a `transform` entity record.
///
/// Transform layout (3x3 + translation + scale):
/// `transform $<attrib> <r00> <r01> <r02> <r10> <r11> <r12> <r20> <r21> <r22> <tx> <ty> <tz> <scale> <is_rotation> <is_reflection> <is_shear>`
#[derive(Debug, Clone)]
pub struct SatTransform<'a> {
    record: &'a SatRecord,
}

impl<'a> SatTransform<'a> {
    /// Wraps a record as a transform entity.
    pub fn from_record(record: &'a SatRecord) -> Option<Self> {
        if record.entity_type == "transform" {
            Some(Self { record })
        } else {
            None
        }
    }

    /// Rotation matrix row 0 (X axis).
    pub fn row0(&self) -> (f64, f64, f64) {
        (
            self.record.token_float(1).unwrap_or(1.0),
            self.record.token_float(2).unwrap_or(0.0),
            self.record.token_float(3).unwrap_or(0.0),
        )
    }

    /// Rotation matrix row 1 (Y axis).
    pub fn row1(&self) -> (f64, f64, f64) {
        (
            self.record.token_float(4).unwrap_or(0.0),
            self.record.token_float(5).unwrap_or(1.0),
            self.record.token_float(6).unwrap_or(0.0),
        )
    }

    /// Rotation matrix row 2 (Z axis).
    pub fn row2(&self) -> (f64, f64, f64) {
        (
            self.record.token_float(7).unwrap_or(0.0),
            self.record.token_float(8).unwrap_or(0.0),
            self.record.token_float(9).unwrap_or(1.0),
        )
    }

    /// Translation vector.
    pub fn translation(&self) -> (f64, f64, f64) {
        (
            self.record.token_float(10).unwrap_or(0.0),
            self.record.token_float(11).unwrap_or(0.0),
            self.record.token_float(12).unwrap_or(0.0),
        )
    }

    /// Scale factor.
    pub fn scale(&self) -> f64 {
        self.record.token_float(13).unwrap_or(1.0)
    }
}

// ============================================================================
// SAT Document
// ============================================================================

/// A parsed SAT document representing complete ACIS solid model data.
///
/// This is the main entry point for working with ACIS geometry. Parse raw
/// SAT text from a Solid3D/Region/Body entity's `AcisData`, inspect or
/// modify the B-rep topology, and write back to SAT text.
///
/// # Example
///
/// ```rust
/// use acadrust::entities::acis::SatDocument;
///
/// let sat_text = "700 0 1 0\n\
///     @8 acadrust @8 ACIS 7.0 @24 Thu Jan 01 00:00:00 2023\n\
///     1e-06 9.9999999999999995e-07\n\
///     -0 asmheader $-1 -1 @12 700 7 0 0 @5 ACIS @3 7.0 @24 Thu Jan 01 00:00:00 2023 #\n\
///     -1 body $-1 $-1 $-1 $-1 #\n\
///     End-of-ACIS-data\n";
///
/// let doc = SatDocument::parse(sat_text).unwrap();
/// assert!(doc.records.len() >= 2);
/// ```
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SatDocument {
    /// SAT file header.
    pub header: SatHeader,
    /// Entity records in the document.
    pub records: Vec<SatRecord>,
}

impl SatDocument {
    /// Creates a new empty SAT document with ACIS 7.0 header.
    pub fn new() -> Self {
        Self {
            header: SatHeader::new(),
            records: Vec::new(),
        }
    }

    /// Creates a document with the specified header.
    pub fn with_header(header: SatHeader) -> Self {
        Self {
            header,
            records: Vec::new(),
        }
    }

    /// Parses SAT text into a structured document.
    pub fn parse(text: &str) -> Result<Self, SatParseError> {
        SatParser::parse(text)
    }

    /// Writes the document back to SAT text format.
    pub fn to_sat_string(&self) -> String {
        SatWriter::write(self)
    }

    /// Returns the number of entity records.
    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    /// Returns a record by index.
    pub fn record(&self, index: usize) -> Option<&SatRecord> {
        self.records.iter().find(|r| r.index == index as i32)
    }

    /// Returns a mutable record by index.
    pub fn record_mut(&mut self, index: usize) -> Option<&mut SatRecord> {
        self.records.iter_mut().find(|r| r.index == index as i32)
    }

    /// Returns all records of a given entity type.
    pub fn records_of_type(&self, entity_type: &str) -> Vec<&SatRecord> {
        self.records
            .iter()
            .filter(|r| r.entity_type == entity_type)
            .collect()
    }

    /// Returns all body records.
    pub fn bodies(&self) -> Vec<SatBody<'_>> {
        self.records
            .iter()
            .filter_map(SatBody::from_record)
            .collect()
    }

    /// Returns all face records.
    pub fn faces(&self) -> Vec<SatFace<'_>> {
        self.records
            .iter()
            .filter_map(SatFace::from_record)
            .collect()
    }

    /// Returns all edge records.
    pub fn edges(&self) -> Vec<SatEdge<'_>> {
        self.records
            .iter()
            .filter_map(SatEdge::from_record)
            .collect()
    }

    /// Returns all vertex records.
    pub fn vertices(&self) -> Vec<SatVertex<'_>> {
        self.records
            .iter()
            .filter_map(SatVertex::from_record)
            .collect()
    }

    /// Adds a record and returns its index.
    pub fn add_record(&mut self, mut record: SatRecord) -> i32 {
        let index = self.records.len() as i32;
        record.index = index;
        self.records.push(record);
        self.header.num_records = self.records.len();
        index
    }

    /// Returns the record that a pointer refers to.
    pub fn resolve(&self, ptr: SatPointer) -> Option<&SatRecord> {
        if ptr.is_null() {
            None
        } else {
            self.record(ptr.0 as usize)
        }
    }

    /// Validates the document structure.
    ///
    /// Checks that all pointers reference valid records and that
    /// the topology is consistent.
    pub fn validate(&self) -> Vec<SatValidationError> {
        let mut errors = Vec::new();
        let max_index = self.records.len() as i32;

        for record in &self.records {
            // Check attribute pointer
            if !record.attribute.is_null() {
                if let Some(idx) = record.attribute.index() {
                    if idx as i32 >= max_index {
                        errors.push(SatValidationError::InvalidPointer {
                            record_index: record.index,
                            pointer_value: record.attribute.0,
                            context: "attribute".to_string(),
                        });
                    }
                }
            }

            // Check all token pointers
            for (i, token) in record.tokens.iter().enumerate() {
                if let SatToken::Pointer(p) = token {
                    if !p.is_null() {
                        if let Some(idx) = p.index() {
                            if idx as i32 >= max_index {
                                errors.push(SatValidationError::InvalidPointer {
                                    record_index: record.index,
                                    pointer_value: p.0,
                                    context: format!("token[{}]", i),
                                });
                            }
                        }
                    }
                }
            }
        }

        errors
    }

    /// Strip non-geometry entities for SAB binary encoding.
    ///
    /// AutoCAD/IntelliCAD's ACIS SAB format only includes core geometric
    /// entities. Custom attribute entities (like `eye_refinement`,
    /// `vertex_template`, `*-attrib`) cause "NOT THAT KIND OF CLASS"
    /// errors in the ACIS kernel when encountered in SAB binary data.
    ///
    /// This method removes all non-core entities and remaps pointer
    /// references in the remaining records.
    pub fn strip_for_sab(&mut self) {
        // Determine which records to keep.
        // Core ACIS base types (last segment after hyphen split):
        let keep: Vec<bool> = self
            .records
            .iter()
            .map(|r| Self::is_core_geometry_type(&r.entity_type))
            .collect();

        let kept_count = keep.iter().filter(|&&k| k).count();
        if kept_count == self.records.len() {
            return; // nothing to strip
        }

        // Build old-index → new-index mapping.
        // Removed records map to -1 (null pointer).
        let mut index_map = vec![-1i32; self.records.len()];
        let mut new_idx: i32 = 0;
        for (old_idx, &kept) in keep.iter().enumerate() {
            if kept {
                index_map[old_idx] = new_idx;
                new_idx += 1;
            }
        }

        // Remap a single pointer value
        let remap = |p: i32| -> i32 {
            if p < 0 || (p as usize) >= index_map.len() {
                -1
            } else {
                index_map[p as usize]
            }
        };

        // Filter and remap records
        let mut new_records = Vec::with_capacity(kept_count);
        let old_records = std::mem::take(&mut self.records);
        for (old_idx, record) in old_records.into_iter().enumerate() {
            if !keep[old_idx] {
                continue;
            }
            let mut rec = record;
            rec.index = index_map[old_idx];

            // Remap attribute pointer — always null since all attribs are stripped
            rec.attribute = SatPointer::new(remap(rec.attribute.0));

            // Remap all pointer tokens
            for token in &mut rec.tokens {
                if let SatToken::Pointer(p) = token {
                    p.0 = remap(p.0);
                }
            }

            new_records.push(rec);
        }

        self.records = new_records;
        self.header.num_records = self.records.len();

        // Normalize spatial_resolution to 1.0 for SAB output.
        // IntelliCAD/AutoCAD always use 1.0 in native SAB data.
        // Source files from older ACIS versions may use different values
        // (e.g. 10.0) which can cause compatibility issues.
        self.header.spatial_resolution = 1.0;
    }

    /// Check if an entity type is a core ACIS geometry type that should
    /// be preserved in SAB output.
    fn is_core_geometry_type(entity_type: &str) -> bool {
        // Get the base type (last segment after hyphen split)
        let base = if let Some(pos) = entity_type.rfind('-') {
            &entity_type[pos + 1..]
        } else {
            entity_type
        };

        matches!(
            base,
            "body"
                | "lump"
                | "shell"
                | "subshell"
                | "face"
                | "loop"
                | "coedge"
                | "edge"
                | "vertex"
                | "wire"
                | "point"
                | "curve"
                | "surface"
                | "pcurve"
                | "transform"
                | "asmheader"
        )
    }
}

impl Default for SatDocument {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Errors
// ============================================================================

/// Error during SAT parsing.
#[derive(Debug, Clone, PartialEq)]
pub enum SatParseError {
    /// The input text is empty or too short.
    EmptyInput,
    /// Failed to parse the header line.
    InvalidHeader(String),
    /// Failed to parse the product info line.
    InvalidProductInfo(String),
    /// Failed to parse the tolerance line.
    InvalidTolerances(String),
    /// Failed to parse an entity record.
    InvalidRecord {
        line: usize,
        message: String,
    },
    /// Unexpected token.
    UnexpectedToken {
        line: usize,
        token: String,
    },
}

impl fmt::Display for SatParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "SAT data is empty"),
            Self::InvalidHeader(msg) => write!(f, "Invalid SAT header: {}", msg),
            Self::InvalidProductInfo(msg) => write!(f, "Invalid product info: {}", msg),
            Self::InvalidTolerances(msg) => write!(f, "Invalid tolerances: {}", msg),
            Self::InvalidRecord { line, message } => {
                write!(f, "Invalid SAT record at line {}: {}", line, message)
            }
            Self::UnexpectedToken { line, token } => {
                write!(f, "Unexpected token '{}' at line {}", token, line)
            }
        }
    }
}

impl std::error::Error for SatParseError {}

/// Validation error in a SAT document.
#[derive(Debug, Clone, PartialEq)]
pub enum SatValidationError {
    /// A pointer references a non-existent record.
    InvalidPointer {
        record_index: i32,
        pointer_value: i32,
        context: String,
    },
}

impl fmt::Display for SatValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPointer {
                record_index,
                pointer_value,
                context,
            } => write!(
                f,
                "Record {} has invalid pointer ${} in {}",
                record_index, pointer_value, context
            ),
        }
    }
}

// ============================================================================
// Helper: SAT Entity Type Classification
// ============================================================================

/// Classification of SAT entity types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SatEntityCategory {
    /// Assembly header (asmheader).
    Header,
    /// Topological entity (body, lump, shell, face, loop, coedge, edge, vertex).
    Topology,
    /// Geometric entity (point, curve, surface).
    Geometry,
    /// Transform entity.
    Transform,
    /// Attribute entity.
    Attribute,
    /// Unknown entity type.
    Unknown,
}

/// Classify a SAT entity type name.
pub fn classify_entity_type(entity_type: &str) -> SatEntityCategory {
    match entity_type {
        "asmheader" => SatEntityCategory::Header,
        "body" | "lump" | "shell" | "subshell" | "face" | "loop" | "coedge" | "edge"
        | "vertex" | "wire" => SatEntityCategory::Topology,
        "point" | "straight-curve" | "ellipse-curve" | "intcurve-curve" | "bs3-curve"
        | "plane-surface" | "cone-surface" | "sphere-surface" | "torus-surface"
        | "spline-surface" | "meshsurf-surface" | "bs3-surface" => SatEntityCategory::Geometry,
        "transform" => SatEntityCategory::Transform,
        _ if entity_type.ends_with("-attrib") || entity_type.starts_with("attrib") => {
            SatEntityCategory::Attribute
        }
        _ => SatEntityCategory::Unknown,
    }
}
