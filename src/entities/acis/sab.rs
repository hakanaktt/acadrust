//! SAB (ACIS Binary) format converter.
//!
//! Converts between SAT text format (used pre-AC1027) and SAB binary format
//! (used in AC1027 / R2013 and later). The SAB format stores the same ACIS
//! topology/geometry data but uses binary tags instead of text tokens.
//!
//! # SAB Tag Bytes
//!
//! | Tag  | Meaning           | Data               |
//! |------|-------------------|---------------------|
//! | 0x02 | Character value   | 1 byte signed       |
//! | 0x03 | Short value       | 2 bytes LE i16      |
//! | 0x04 | Integer value     | 4 bytes LE i32      |
//! | 0x05 | Float value       | 4 bytes IEEE        |
//! | 0x06 | Double value      | 8 bytes LE f64      |
//! | 0x07 | String literal    | 1-byte len + bytes  |
//! | 0x08 | String literal    | 2-byte len + bytes  |
//! | 0x09 | String literal    | 4-byte len + bytes  |
//! | 0x0A | False / Reversed  | (no data)           |
//! | 0x0B | True / Forward    | (no data)           |
//! | 0x0C | Entity pointer    | 4 bytes LE i32      |
//! | 0x0D | Entity type name  | 1-byte len + bytes  |
//! | 0x0E | Subtype prefix    | 1-byte len + bytes  |
//! | 0x11 | End of record     | (no data)           |
//! | 0x13 | Position (x,y,z)  | 24 bytes (3×f64 LE) |
//! | 0x14 | Direction (x,y,z) | 24 bytes (3×f64 LE) |
//! | 0x15 | Enum value        | 4 bytes LE i32      |
//! | 0x16 | U-V vector        | 16 bytes (2×f64 LE) |
//! | 0x17 | 64-bit integer    | 8 bytes LE i64      |

use super::types::*;

/// SAB binary tag constants.
pub mod tags {
    /// Signed character value.
    pub const CHARACTER: u8 = 0x02;
    /// Signed short value.
    pub const SHORT: u8 = 0x03;
    /// Integer value (plain int, not entity pointer).
    pub const INTEGER: u8 = 0x04;
    /// Single-precision float.
    pub const FLOAT: u8 = 0x05;
    /// Double-precision float.
    pub const DOUBLE: u8 = 0x06;
    /// String literal with length prefix.
    pub const STRING: u8 = 0x07;
    /// String literal with a 2-byte length prefix.
    pub const SHORT_STRING: u8 = 0x08;
    /// String literal with a 4-byte length prefix.
    pub const LONG_STRING: u8 = 0x09;
    /// Boolean false / reversed / double-sided.
    pub const FALSE: u8 = 0x0A;
    /// Boolean true / forward / single-sided.
    pub const TRUE: u8 = 0x0B;
    /// Entity pointer reference (like `$n` in SAT).
    pub const POINTER: u8 = 0x0C;
    /// Entity type name.
    pub const ENTITY_TYPE: u8 = 0x0D;
    /// Subtype prefix (for compound types like `plane-surface`).
    pub const SUBTYPE: u8 = 0x0E;
    /// Subtype block start (`{` in SAT) — opens a nested subrecord, e.g. the
    /// `skin_spl_sur` block inside a lofted spline-surface.
    pub const SUBTYPE_START: u8 = 0x0F;
    /// Subtype block end (`}` in SAT) — closes a nested subrecord.
    pub const SUBTYPE_END: u8 = 0x10;
    /// End of record marker.
    pub const END_OF_RECORD: u8 = 0x11;
    /// Newer ASM long string literal (4-byte u32 length prefix + bytes).
    pub const ASM_LONG_STRING: u8 = 0x12;
    /// Position (3 doubles: x, y, z).
    pub const POSITION: u8 = 0x13;
    /// Direction (3 doubles: x, y, z).
    pub const DIRECTION: u8 = 0x14;
    /// Enumerated value (4-byte int). Emitted by ASM / ShapeManager records
    /// (AutoCAD 2013+).
    pub const ENUM: u8 = 0x15;
    /// U-V vector (2 doubles).
    pub const UV: u8 = 0x16;
    /// Signed 64-bit integer used by newer ASM / ShapeManager records.
    pub const INTEGER64: u8 = 0x17;
}

/// SAB header magic string.
const SAB_MAGIC: &[u8] = b"ACIS BinaryFile";

// ============================================================================
// SAT → SAB Writer
// ============================================================================

/// Converts a [`SatDocument`] to SAB binary format.
pub struct SabWriter;

impl SabWriter {
    /// Convert a SAT document to SAB binary data.
    pub fn write(doc: &SatDocument) -> Vec<u8> {
        let completed = doc.completed_for_restore();
        let doc = completed.as_ref().unwrap_or(doc);
        let mut buf = Vec::with_capacity(8192);

        // Header
        Self::write_header(&mut buf, &doc.header);

        // Entity records
        for record in &doc.records {
            Self::write_record(&mut buf, record);
        }

        // End marker, with no end-of-record tag, written as tagged components
        // (`End-of-ASM-data` for ShapeManager bodies, `End-of-ACIS-data`
        // before); the reference application rejects a 22300 body closed with
        // a single-string marker.
        let kernel = if Self::is_asm(&doc.header) { "ASM" } else { "ACIS" };
        for part in ["End", "of", kernel] {
            Self::write_subtype(&mut buf, part);
        }
        Self::write_entity_type(&mut buf, "data");

        buf
    }

    fn is_asm(header: &SatHeader) -> bool {
        header.version.sat_version_number() >= 21800
    }

    fn write_header(buf: &mut Vec<u8>, header: &SatHeader) {
        // Magic
        buf.extend_from_slice(SAB_MAGIC);

        // Version number (4 bytes LE)
        let ver = header.version.sat_version_number();
        buf.extend_from_slice(&ver.to_le_bytes());

        // num_records field (4 bytes LE) — always 0 for ACIS 7.0+
        let num_records: u32 = if header.version.has_explicit_indices() {
            0
        } else {
            header.num_records as u32
        };
        buf.extend_from_slice(&num_records.to_le_bytes());

        // num_bodies (4 bytes LE)
        buf.extend_from_slice(&(header.num_bodies as u32).to_le_bytes());

        // Flags (4 bytes LE): bit 0 is history. A ShapeManager body also needs
        // bits 2 and 3 (12, as the reference application's own 21800 bodies
        // carry); measured: 21800 bodies are rejected with 0/1/4/8 and
        // restored with 12, 22300 bodies restore with 12 too.
        let mut history: u32 = if header.has_history { 1 } else { 0 };
        if Self::is_asm(header) {
            history |= 12;
        }
        buf.extend_from_slice(&history.to_le_bytes());

        // Product info strings
        Self::write_string(buf, &header.product_id);
        Self::write_string(buf, &header.product_version);
        Self::write_string(buf, &header.date);

        // Tolerances
        Self::write_double(buf, header.spatial_resolution);
        Self::write_double(buf, header.normal_tolerance);
        if let Some(resfit) = header.resfit_tolerance {
            Self::write_double(buf, resfit);
        }
    }

    fn write_record(buf: &mut Vec<u8>, record: &SatRecord) {
        // Entity type — may be compound with multiple hyphens.
        // In SAB, each level of the class hierarchy is a separate tag:
        //   "plane-surface"               → 0x0E("plane") + 0x0D("surface")
        //   "fmesh-eye-attrib"            → 0x0E("fmesh") + 0x0E("eye") + 0x0D("attrib")
        //   "persubent-acadSolidHistory-attrib" → 0x0E("persubent") + 0x0E("acadSolidHistory") + 0x0D("attrib")
        // The last segment is always the base type (0x0D ENTITY_TYPE);
        // all preceding segments are subtype prefixes (0x0E SUBTYPE).
        if record.entity_type.contains('-') {
            let parts: Vec<&str> = record.entity_type.split('-').collect();
            // All parts except the last are subtypes
            for &part in &parts[..parts.len() - 1] {
                Self::write_subtype(buf, part);
            }
            // Last part is the base entity type
            Self::write_entity_type(buf, parts[parts.len() - 1]);
        } else {
            Self::write_entity_type(buf, &record.entity_type);
        }

        // Attribute pointer
        Self::write_pointer(buf, record.attribute.0);

        // Subtype ID (plain integer, not pointer)
        Self::write_integer(buf, record.subtype_id);

        if record.entity_type == "transform"
            && !record
                .tokens
                .iter()
                .any(|t| matches!(t, SatToken::Sab { .. }))
        {
            let tokens: Vec<_> = record
                .tokens
                .iter()
                .filter(|t| !matches!(t, SatToken::Pointer(_)))
                .collect();
            for row in tokens[..tokens.len().min(12)].chunks(3) {
                if row.len() == 3 {
                    Self::write_direction(
                        buf,
                        row[0].as_float().unwrap_or(0.0),
                        row[1].as_float().unwrap_or(0.0),
                        row[2].as_float().unwrap_or(0.0),
                    );
                }
            }
            if let Some(scale) = tokens.get(12) {
                Self::write_double(buf, scale.as_float().unwrap_or(1.0));
            }
            for flag in tokens.iter().skip(13) {
                Self::write_token(buf, flag, false);
            }
            buf.push(tags::END_OF_RECORD);
            return;
        }

        // Remaining tokens — with entity-type-aware coordinate grouping.
        // In SAT text, coordinates are individual Float tokens, but SAB uses
        // composite position(0x13)/direction(0x14) tags for coordinate triplets.
        let layout = CoordLayout::for_entity(&record.entity_type);
        let ints_as_doubles = Self::integers_are_doubles(&record.entity_type);
        let mut contextual;
        let tokens = if base_entity_type(&record.entity_type) == "face" {
            contextual = Self::encode_face_boolean_roles(&record.tokens);
            &contextual
        } else if base_entity_type(&record.entity_type) == "wire" {
            // The wire side is TRUE for "out", unlike the generic mapping.
            contextual = record
                .tokens
                .iter()
                .map(|token| match token.as_ident() {
                    Some("out") => SatToken::True,
                    Some("in") => SatToken::False,
                    _ => token.clone(),
                })
                .collect();
            &contextual
        } else if matches!(
            record.entity_type.as_str(),
            "intcurve-curve" | "spline-surface" | "pcurve"
        ) {
            contextual = Self::encode_spline_numeric_roles(&record.entity_type, &record.tokens);
            &contextual
        } else if record.entity_type == "eye_refinement" {
            // Labelled fields: the tolerances are doubles even when SAT
            // writes them as whole numbers; the grid counts stay integers.
            contextual = record.tokens.clone();
            for index in 1..contextual.len() {
                let label = match &contextual[index - 1] {
                    SatToken::String(label) | SatToken::Ident(label) => label.as_str(),
                    _ => continue,
                };
                if matches!(
                    label,
                    "stol" | "ntol" | "dsil" | "flatness" | "pixarea" | "hmax" | "gridar"
                ) {
                    if let SatToken::Integer(value) = contextual[index] {
                        contextual[index] = SatToken::Float(value as f64);
                    }
                }
            }
            &contextual
        } else {
            &record.tokens
        };
        Self::write_tokens_with_coord_grouping(buf, tokens, &layout, ints_as_doubles);

        // End of record
        buf.push(tags::END_OF_RECORD);
    }

    /// Write tokens with coordinate grouping based on entity type layout.
    ///
    /// For geometric entities (surfaces, curves, points), consecutive Float
    /// tokens that represent coordinates are grouped into Position/Direction
    /// composite tags. The layout describes the exact sequence of triplets
    /// and scalars for each entity type.
    fn write_tokens_with_coord_grouping(
        buf: &mut Vec<u8>,
        tokens: &[SatToken],
        layout: &CoordLayout,
        ints_as_doubles: bool,
    ) {
        let mut i = 0;
        let mut step_index = 0; // tracks position in layout.steps
        let mut subtype_depth = 0usize;
        let mut active_steps = layout.steps;

        // Skip the first Pointer token (v700 unknown/$-1) to count geometry tokens
        let geom_start = tokens.iter().position(|t| Self::is_numeric(t));

        while i < tokens.len() {
            // ACIS subtype names and geometry keywords use identifier tags,
            // not counted strings. Keep explicit String tokens as strings.
            if subtype_depth > 0 {
                if let SatToken::Ident(name) = &tokens[i] {
                    if name != "{" && name != "}" && Self::string_to_boolean(name).is_none() {
                        // Explicit UV curves and intersection curves embed
                        // analytic surface geometry inside their subtype.
                        // Its vectors need the same binary grouping as a
                        // standalone surface, without grouping UV controls.
                        let inline_layout = match name.as_str() {
                            "plane" | "cone" => Some(CoordLayout::POS_DIR_DIR),
                            "sphere" => Some(CoordLayout::POS_S_DIR_DIR),
                            "torus" => Some(CoordLayout::POS_DIR_SS_DIR),
                            _ => None,
                        };
                        if let Some(inline_layout) = inline_layout {
                            active_steps = inline_layout.steps;
                            step_index = 0;
                        }
                        Self::write_entity_type(buf, name);
                        i += 1;
                        continue;
                    }
                }
            }
            // A token decoded from SAB already carries its original tag and
            // payload. Re-emit it byte-for-byte instead of applying SAT
            // coordinate grouping or numeric coercion.
            if matches!(&tokens[i], SatToken::Sab { .. }) {
                if matches!(
                    &tokens[i],
                    SatToken::Sab {
                        tag: tags::POSITION | tags::DIRECTION,
                        ..
                    }
                ) {
                    step_index += 1;
                }
                Self::write_token(buf, &tokens[i], ints_as_doubles && subtype_depth == 0);
                i += 1;
                continue;
            }

            // Explicit semantic positions are already grouped.
            if matches!(&tokens[i], SatToken::Position(_, _, _)) {
                Self::write_token(buf, &tokens[i], ints_as_doubles && subtype_depth == 0);
                step_index += 1;
                i += 1;
                continue;
            }

            // Are we in the geometry section of the token stream?
            let in_geom = geom_start.map(|gs| i >= gs).unwrap_or(false);

            if in_geom && step_index < active_steps.len() {
                match active_steps[step_index] {
                    Some(tag) => {
                        // This step expects a coordinate triplet (3 floats → Position/Direction)
                        if i + 2 < tokens.len()
                            && Self::is_numeric(&tokens[i])
                            && Self::is_numeric(&tokens[i + 1])
                            && Self::is_numeric(&tokens[i + 2])
                        {
                            let x = Self::numeric_value(&tokens[i]);
                            let y = Self::numeric_value(&tokens[i + 1]);
                            let z = Self::numeric_value(&tokens[i + 2]);

                            buf.push(tag);
                            buf.extend_from_slice(&x.to_le_bytes());
                            buf.extend_from_slice(&y.to_le_bytes());
                            buf.extend_from_slice(&z.to_le_bytes());

                            i += 3;
                            step_index += 1;
                        } else {
                            // Not enough numeric tokens for a triplet — write individually
                            Self::write_token(
                                buf,
                                &tokens[i],
                                ints_as_doubles && subtype_depth == 0,
                            );
                            i += 1;
                        }
                    }
                    None => {
                        // This step expects a scalar double (single float value)
                        Self::write_token(buf, &tokens[i], ints_as_doubles && subtype_depth == 0);
                        i += 1;
                        step_index += 1;
                    }
                }
            } else {
                Self::write_token(buf, &tokens[i], ints_as_doubles && subtype_depth == 0);
                i += 1;
            }

            match tokens[i - 1].as_ident() {
                Some("{") => subtype_depth += 1,
                Some("}") => subtype_depth = subtype_depth.saturating_sub(1),
                _ => {}
            }
        }
    }

    /// Check if a token is a numeric value (Float or Integer).
    fn is_numeric(token: &SatToken) -> bool {
        matches!(token, SatToken::Float(_) | SatToken::Integer(_))
    }

    /// Extract numeric value from a Float or Integer token.
    fn numeric_value(token: &SatToken) -> f64 {
        match token {
            SatToken::Float(v) => *v,
            SatToken::Integer(v) => *v as f64,
            _ => 0.0,
        }
    }

    fn write_token(buf: &mut Vec<u8>, token: &SatToken, ints_as_doubles: bool) {
        match token {
            SatToken::Pointer(p) => Self::write_pointer(buf, p.0),
            // Direct geometry parameters such as edge start/end values are
            // doubles. NURBS subtype fields are routed here with `false` so
            // their degree and multiplicities stay integer-tagged in SAB.
            SatToken::Integer(v) => {
                if ints_as_doubles {
                    Self::write_double(buf, *v as f64);
                } else {
                    Self::write_integer(buf, *v as i32);
                }
            }
            SatToken::Float(v) => Self::write_double(buf, *v),
            // String tokens from @-counted SAT format may be boolean keywords
            // (e.g., @9 reverse_v, @9 forward_v). Map them to TRUE/FALSE tags.
            SatToken::String(s) => {
                if let Some(val) = Self::string_to_boolean(s) {
                    buf.push(if val { tags::TRUE } else { tags::FALSE });
                } else {
                    Self::write_string(buf, s);
                }
            }
            SatToken::Position(x, y, z) => Self::write_position(buf, *x, *y, *z),
            SatToken::True => buf.push(tags::TRUE),
            SatToken::False => buf.push(tags::FALSE),
            SatToken::Terminator => buf.push(tags::END_OF_RECORD),
            SatToken::Ident(s) => Self::write_ident_token(buf, s),
            SatToken::Enum(s) => Self::write_enum_token(buf, s),
            SatToken::Sab { tag, data } => {
                buf.push(*tag);
                buf.extend_from_slice(data);
            }
        }
    }

    /// Check if a string value is a known ACIS boolean keyword.
    /// Returns `Some(true)` for forward/positive, `Some(false)` for reversed/negative.
    fn string_to_boolean(s: &str) -> Option<bool> {
        match s {
            "forward_v" | "I" | "forward" | "single" | "in" | "no_rotate" | "no_reflect"
            | "no_shear" | "surf2" => Some(true),
            "reverse_v" | "reversed_v" | "reversed" | "double" | "out" | "F" | "rotate"
            | "reflect" | "shear" | "surf1" => Some(false),
            _ => None,
        }
    }

    fn write_ident_token(buf: &mut Vec<u8>, ident: &str) {
        match ident {
            "{" => buf.push(tags::SUBTYPE_START),
            "}" => buf.push(tags::SUBTYPE_END),
            _ => {
                // Map known boolean identifiers to True/False tags
                if let Some(val) = Self::string_to_boolean(ident) {
                    buf.push(if val { tags::TRUE } else { tags::FALSE });
                } else {
                    Self::write_string(buf, ident);
                }
            }
        }
    }

    fn write_enum_token(buf: &mut Vec<u8>, name: &str) {
        if let Some(value) = Self::string_to_boolean(name) {
            buf.push(if value { tags::TRUE } else { tags::FALSE });
            return;
        }
        match name {
            "full" | "open" | "none" | "closed" | "periodic" | "UNEXTENDED" => {
                let value: i32 = match name {
                    "closed" => 1,
                    "periodic" => 2,
                    _ => 0,
                };
                buf.push(tags::ENUM);
                buf.extend_from_slice(&value.to_le_bytes());
            }
            // "unknown" and other enum values → string
            _ => Self::write_string(buf, name),
        }
    }

    fn encode_face_boolean_roles(tokens: &[SatToken]) -> Vec<SatToken> {
        let mut result = tokens.to_vec();
        let semantic: Vec<_> = result
            .iter()
            .enumerate()
            .filter(|(_, token)| {
                matches!(token, SatToken::True | SatToken::False)
                    || token.as_ident().is_some_and(|name| {
                        matches!(
                            name,
                            "forward" | "reversed" | "single" | "double" | "in" | "out"
                        )
                    })
            })
            .map(|(index, _)| index)
            .collect();
        let start = semantic.len().saturating_sub(3);
        for (role, &index) in semantic[start..].iter().enumerate() {
            let raw = match &result[index] {
                SatToken::True => true,
                SatToken::False => false,
                token => match (role, token.as_ident()) {
                    (0, Some("forward")) | (1, Some("single")) => true,
                    (0, Some("reversed")) | (1, Some("double")) => false,
                    (2, Some("out")) => true,
                    (2, Some("in")) => false,
                    _ => continue,
                },
            };
            result[index] = if raw { SatToken::True } else { SatToken::False };
        }
        result
    }

    fn write_entity_type(buf: &mut Vec<u8>, name: &str) {
        buf.push(tags::ENTITY_TYPE);
        buf.push(name.len() as u8);
        buf.extend_from_slice(name.as_bytes());
    }

    // SAT has no numeric type tags: a double such as 1.0 is commonly written
    // as "1". Recover the roles of explicit NURBS fields before emitting SAB.
    fn encode_spline_numeric_roles(entity_type: &str, tokens: &[SatToken]) -> Vec<SatToken> {
        let mut result = tokens.to_vec();
        let mut doubles = Vec::new();
        let mut enums = Vec::new();
        let mut subtypes = Vec::new();
        // Per open subtype block: where an interpolated curve's pcurves end.
        let mut pcurve_ends: Vec<Option<usize>> = Vec::new();
        let in_int_cur =
            |subtypes: &[Option<&str>]| matches!(subtypes.last(), Some(Some(name)) if name.ends_with("cur"));
        for (index, token) in tokens.iter().enumerate() {
            match token.as_ident() {
                Some("{") => {
                    subtypes.push(tokens.get(index + 1).and_then(SatToken::as_ident));
                    pcurve_ends.push(None);
                }
                Some("}") => {
                    // The pcurves are followed by two logicals and the
                    // curve's discontinuity lists.
                    if let (true, Some(Some(end))) = (in_int_cur(&subtypes), pcurve_ends.last()) {
                        if tokens.get(*end).is_some_and(Self::is_logical)
                            && tokens.get(end + 1).is_some_and(Self::is_logical)
                        {
                            doubles.extend(Self::discontinuity_doubles(tokens, end + 2, 3));
                        }
                    }
                    subtypes.pop();
                    pcurve_ends.pop();
                }
                Some("nullbs") if in_int_cur(&subtypes) => {
                    if let Some(end) = pcurve_ends.last_mut() {
                        *end = Some(index + 1);
                    }
                }
                _ => {}
            }
            if matches!(token.as_ident(), Some("nurbs" | "nubs")) {
                let previous = index.checked_sub(1).and_then(|i| tokens[i].as_ident());
                let parent = index.checked_sub(2).and_then(|i| tokens[i].as_ident());
                // ShapeManager curves write `exact_int_cur <version> full`;
                // the other interpolated curves (`surfintcur`, ...) open with
                // the same approximating `full nubs` spline.
                let parent = match subtypes.last() {
                    Some(Some(name)) if name.ends_with("cur") => Some("exactcur"),
                    _ => parent,
                };
                let pcurve = in_int_cur(&subtypes) && previous != Some("full");
                let dimensions = match (parent, previous) {
                    (Some("exactcur"), Some("full")) => Some((3, false)),
                    (Some("exactsur"), Some("full")) => Some((3, true)),
                    (_, Some("exppc")) => Some((2, false)),
                    // A 2D pcurve of an interpolated curve: no fit tolerance.
                    _ if pcurve => Some((2, false)),
                    _ => None,
                };
                if let Some((dimensions, surface)) = dimensions {
                    if let Some(fields) =
                        Self::nurbs_double_fields(tokens, index, dimensions, surface, !pcurve)
                    {
                        let end = fields.iter().max().map(|last| last + 1);
                        if pcurve {
                            if let Some(slot) = pcurve_ends.last_mut() {
                                *slot = end;
                            }
                        } else if surface {
                            // u and v discontinuity lists follow the fit tolerance.
                            if let Some(end) = end {
                                doubles.extend(Self::discontinuity_doubles(tokens, end, 6));
                            }
                        }
                        doubles.extend(fields);
                        if previous == Some("full") {
                            enums.push(index - 1);
                        }
                        if surface {
                            let closure =
                                index + 3 + usize::from(token.as_ident() == Some("nurbs"));
                            enums.extend(closure..closure + 4);
                        } else {
                            enums.push(index + 2);
                        }
                    }
                }
            }
            if (token.as_ident() == Some("F") || matches!(token, SatToken::False))
                && index + 1 < tokens.len()
                && (matches!(
                    subtypes.last(),
                    None | Some(Some("exactcur" | "exact_int_cur" | "exactsur" | "exppc"))
                ) || in_int_cur(&subtypes))
            {
                doubles.push(index + 1);
            }
            if token.as_ident() == Some("UNEXTENDED") && !subtypes.is_empty() {
                enums.push(index);
            }
            // An explicit pcurve ends with an inline analytic support surface.
            // All numbers in that geometry (including bounded intervals) are doubles.
            if entity_type == "pcurve"
                && subtypes.last() == Some(&Some("exppc"))
                && matches!(
                    token.as_ident(),
                    Some("plane" | "cone" | "sphere" | "torus")
                )
            {
                doubles.extend(
                    (index + 1..tokens.len()).take_while(|&i| tokens[i].as_ident() != Some("}")),
                );
            }
            // An interpolated curve stores its analytic support surfaces
            // inline after the spline; their numbers are doubles, while the
            // trailing fields after the `nullbs` pcurves stay integers.
            if entity_type == "intcurve-curve"
                && matches!(subtypes.last(), Some(Some(name)) if name.ends_with("cur"))
                && matches!(
                    token.as_ident(),
                    Some("plane" | "cone" | "sphere" | "torus")
                )
            {
                doubles.extend((index + 1..tokens.len()).take_while(|&i| {
                    !matches!(
                        tokens[i].as_ident(),
                        Some(
                            "plane" | "cone" | "sphere" | "torus" | "spline" | "nullbs" | "nubs"
                                | "nurbs" | "{" | "}"
                        )
                    )
                }));
            }
        }
        if entity_type == "pcurve" && tokens.len() >= 2 {
            doubles.extend(tokens.len() - 2..tokens.len());
        }
        for index in doubles {
            if let SatToken::Integer(value) = result[index] {
                result[index] = SatToken::Float(value as f64);
            }
        }
        for index in enums {
            if let SatToken::Ident(name) = &result[index] {
                if matches!(
                    name.as_str(),
                    "full" | "open" | "closed" | "periodic" | "none" | "UNEXTENDED"
                ) {
                    result[index] = SatToken::Enum(name.clone());
                }
            }
        }
        result
    }

    fn nurbs_double_fields(
        tokens: &[SatToken],
        start: usize,
        dimensions: usize,
        surface: bool,
        fit_tolerance: bool,
    ) -> Option<Vec<usize>> {
        let integer = |index: usize| usize::try_from(tokens.get(index)?.as_integer()?).ok();
        let rational = tokens.get(start)?.as_ident()? == "nurbs";
        let degree_u = integer(start + 1)?;
        let (degrees, counts, mut position) = if surface {
            let degree_v = integer(start + 2)?;
            let count_index = start + 7 + usize::from(rational);
            (
                vec![degree_u, degree_v],
                vec![integer(count_index)?, integer(count_index + 1)?],
                count_index + 2,
            )
        } else {
            (vec![degree_u], vec![integer(start + 3)?], start + 4)
        };
        let mut doubles = Vec::new();
        let mut controls = 1usize;
        for (degree, count) in degrees.into_iter().zip(counts) {
            if count < 2 || count > tokens.len().saturating_sub(position) / 2 {
                return None;
            }
            let mut multiplicities = 0usize;
            for _ in 0..count {
                tokens.get(position)?.as_float()?;
                doubles.push(position);
                multiplicities = multiplicities.checked_add(integer(position + 1)?)?;
                position += 2;
            }
            // ACIS omits one repetition at each clamped endpoint.
            let axis_controls = multiplicities.checked_add(1)?.checked_sub(degree)?;
            controls = controls.checked_mul(axis_controls)?;
        }
        let values = controls
            .checked_mul(dimensions + usize::from(rational))?
            .checked_add(usize::from(fit_tolerance))?;
        let end = position.checked_add(values)?;
        for (offset, token) in tokens.get(position..end)?.iter().enumerate() {
            token.as_float()?;
            doubles.push(position + offset);
        }
        Some(doubles)
    }

    fn is_logical(token: &SatToken) -> bool {
        matches!(token, SatToken::True | SatToken::False)
            || matches!(token.as_ident(), Some("I" | "F" | "T"))
    }

    fn discontinuity_doubles(tokens: &[SatToken], start: usize, groups: usize) -> Vec<usize> {
        let mut doubles = Vec::new();
        let mut position = start;
        for _ in 0..groups {
            let Some(count) = tokens
                .get(position)
                .and_then(SatToken::as_integer)
                .and_then(|count| usize::try_from(count).ok())
            else {
                return Vec::new();
            };
            let values = position + 1..position + 1 + count;
            if tokens.get(values.clone()).map_or(true, |values| {
                values.iter().any(|value| value.as_float().is_none())
            }) {
                return Vec::new();
            }
            doubles.extend(values);
            position += 1 + count;
        }
        doubles
    }

    /// Determine whether direct geometry integer literals are scalar doubles.
    ///
    /// NURBS subtype blocks are handled separately in
    /// [`Self::write_tokens_with_coord_grouping`]: their degree, knot counts,
    /// and knot multiplicities are SAB integers even when the enclosing
    /// entity is a curve, surface, or pcurve.
    fn integers_are_doubles(entity_type: &str) -> bool {
        matches!(
            entity_type,
            "point"
                | "straight-curve"
                | "ellipse-curve"
                | "plane-surface"
                | "cone-surface"
                | "sphere-surface"
                | "torus-surface"
                | "edge"
                // Tolerant edges and coedges keep their parameter range.
                | "tedge-edge"
                | "tcoedge-coedge"
        )
    }

    fn write_subtype(buf: &mut Vec<u8>, name: &str) {
        buf.push(tags::SUBTYPE);
        buf.push(name.len() as u8);
        buf.extend_from_slice(name.as_bytes());
    }

    fn write_pointer(buf: &mut Vec<u8>, value: i32) {
        buf.push(tags::POINTER);
        buf.extend_from_slice(&value.to_le_bytes());
    }

    fn write_integer(buf: &mut Vec<u8>, value: i32) {
        buf.push(tags::INTEGER);
        buf.extend_from_slice(&value.to_le_bytes());
    }

    fn write_double(buf: &mut Vec<u8>, value: f64) {
        buf.push(tags::DOUBLE);
        buf.extend_from_slice(&value.to_le_bytes());
    }

    fn write_string(buf: &mut Vec<u8>, s: &str) {
        if s.len() > u16::MAX as usize {
            buf.push(tags::LONG_STRING);
            buf.extend_from_slice(&(s.len() as u32).to_le_bytes());
        } else if s.len() > u8::MAX as usize {
            buf.push(tags::SHORT_STRING);
            buf.extend_from_slice(&(s.len() as u16).to_le_bytes());
        } else {
            buf.push(tags::STRING);
            buf.push(s.len() as u8);
        }
        buf.extend_from_slice(s.as_bytes());
    }

    fn write_position(buf: &mut Vec<u8>, x: f64, y: f64, z: f64) {
        buf.push(tags::POSITION);
        buf.extend_from_slice(&x.to_le_bytes());
        buf.extend_from_slice(&y.to_le_bytes());
        buf.extend_from_slice(&z.to_le_bytes());
    }

    #[allow(dead_code)]
    fn write_direction(buf: &mut Vec<u8>, x: f64, y: f64, z: f64) {
        buf.push(tags::DIRECTION);
        buf.extend_from_slice(&x.to_le_bytes());
        buf.extend_from_slice(&y.to_le_bytes());
        buf.extend_from_slice(&z.to_le_bytes());
    }
}

// ============================================================================
// Coordinate layout for entity-type-aware SAB encoding
// ============================================================================

/// Describes the layout of coordinate triplets and scalars in a geometry record.
///
/// In SAT text, positions and directions are both written as three individual floats.
/// In SAB binary, positions use tag `0x13` and directions use tag `0x14`, each
/// encoding three f64 values as a single composite token.
///
/// Some entities (like `sphere-surface` and `torus-surface`) have scalar float
/// values interleaved between coordinate triplets. The layout must describe
/// the exact sequence to group correctly.
struct CoordLayout {
    /// Sequence of geometry tokens after the initial v700 $-1 pointer.
    ///
    /// - `Some(tag)` = group next 3 floats as a composite Position/Direction.
    /// - `None` = write next float as an individual DOUBLE scalar.
    steps: &'static [Option<u8>],
}

impl CoordLayout {
    const EMPTY: Self = Self { steps: &[] };

    /// Position only (e.g., `point`)
    const POS: Self = Self {
        steps: &[Some(tags::POSITION)],
    };

    /// Position + direction (e.g., `straight-curve`)
    const POS_DIR: Self = Self {
        steps: &[Some(tags::POSITION), Some(tags::DIRECTION)],
    };

    /// Position + direction + direction (e.g., `plane-surface`)
    const POS_DIR_DIR: Self = Self {
        steps: &[
            Some(tags::POSITION),
            Some(tags::DIRECTION),
            Some(tags::DIRECTION),
        ],
    };

    /// Position + direction + position (e.g., future entity types where
    /// the 3rd triplet's magnitude carries meaning).
    #[allow(dead_code)]
    const POS_DIR_POS: Self = Self {
        steps: &[
            Some(tags::POSITION),
            Some(tags::DIRECTION),
            Some(tags::POSITION),
        ],
    };

    /// Position + scalar + direction + direction (e.g., `sphere-surface`)
    ///
    /// sphere-surface: center(pos) radius(scalar) u_dir(dir) pole(dir)
    const POS_S_DIR_DIR: Self = Self {
        steps: &[
            Some(tags::POSITION),
            None, // radius (scalar double)
            Some(tags::DIRECTION),
            Some(tags::DIRECTION),
        ],
    };

    /// Position + direction + scalar + scalar + direction (e.g., `torus-surface`)
    ///
    /// torus-surface: center(pos) normal(dir) major_r(scalar) minor_r(scalar) u_dir(dir)
    const POS_DIR_SS_DIR: Self = Self {
        steps: &[
            Some(tags::POSITION),
            Some(tags::DIRECTION),
            None, // major_radius (scalar)
            None, // minor_radius (scalar)
            Some(tags::DIRECTION),
        ],
    };

    /// Determine the coordinate layout for a given entity type.
    fn for_entity(entity_type: &str) -> Self {
        match entity_type {
            "point" => Self::POS,
            "straight-curve" => Self::POS_DIR,
            "plane-surface" => Self::POS_DIR_DIR,
            "cone-surface" => Self::POS_DIR_DIR,
            "sphere-surface" => Self::POS_S_DIR_DIR,
            "torus-surface" => Self::POS_DIR_SS_DIR,
            "ellipse-curve" => Self::POS_DIR_DIR,
            _ => Self::EMPTY,
        }
    }
}

// ============================================================================
// SAB → SAT Reader
// ============================================================================

/// Error type for SAB parsing.
#[derive(Debug)]
pub enum SabError {
    /// Unexpected end of data.
    UnexpectedEof,
    /// Invalid magic header.
    InvalidMagic,
    /// Unknown tag byte.
    UnknownTag(u8, usize),
    /// Invalid string encoding.
    InvalidString,
}

impl std::fmt::Display for SabError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SabError::UnexpectedEof => write!(f, "Unexpected end of SAB data"),
            SabError::InvalidMagic => write!(f, "Invalid SAB magic header"),
            SabError::UnknownTag(tag, pos) => {
                write!(f, "Unknown SAB tag 0x{:02X} at position {}", tag, pos)
            }
            SabError::InvalidString => write!(f, "Invalid UTF-8 string in SAB data"),
        }
    }
}

impl std::error::Error for SabError {}

/// Reads SAB binary data and produces a [`SatDocument`].
pub struct SabReader;

impl SabReader {
    /// Parse SAB binary data into a SAT document.
    pub fn read(data: &[u8]) -> Result<SatDocument, SabError> {
        Self::read_with_consumed(data).map(|(doc, _)| doc)
    }

    /// Like [`read`](Self::read), also returning how many input bytes the
    /// payload occupied (through the End-of-ACIS-data record). R2004–R2006
    /// DWGs embed the SAB with no length prefix, so the caller measures the
    /// blob by parsing it.
    pub fn read_with_consumed(data: &[u8]) -> Result<(SatDocument, usize), SabError> {
        // Two header magics: classic ACIS ("ACIS BinaryFile", 15 bytes) and the
        // newer Autodesk ShapeManager ("ASM BinaryFile", 14 bytes) emitted by
        // AutoCAD 2013+ and carried in the AcDs data store. In both the version
        // u32 begins 15 bytes in: ACIS's magic is 15 bytes; ASM's 14-byte magic
        // is followed by one trailing byte before the header ints.
        const SAB_MAGIC_ASM: &[u8] = b"ASM BinaryFile";
        let mut pos = if data.starts_with(SAB_MAGIC) {
            SAB_MAGIC.len()
        } else if data.starts_with(SAB_MAGIC_ASM) {
            SAB_MAGIC.len() // 15 = 14-byte ASM magic + 1 trailing byte
        } else {
            return Err(SabError::InvalidMagic);
        };

        // Header ints (4 × u32 LE)
        let version_num = read_u32(data, &mut pos)?;
        let num_records = read_u32(data, &mut pos)? as usize;
        let num_bodies = read_u32(data, &mut pos)? as usize;
        let has_history = read_u32(data, &mut pos)? & 1 != 0;

        let version = SatVersion::from_sat_number(version_num);

        // Header strings (3 tagged strings)
        let product_id = read_tagged_string(data, &mut pos)?;
        let product_version = read_tagged_string(data, &mut pos)?;
        let date = read_tagged_string(data, &mut pos)?;

        // Tolerances (2 or 3 tagged doubles)
        let spatial_resolution = read_tagged_double(data, &mut pos)?;
        let normal_tolerance = read_tagged_double(data, &mut pos)?;

        // Third tolerance (resfit) — officially ACIS 7.0+ only, but some
        // writers (e.g. Open Design Alliance ACIS Builder) include it in
        // older versions too.  Peek at the next byte: if it's a DOUBLE
        // tag we read it; otherwise skip.
        let resfit_tolerance = if pos < data.len() && data[pos] == tags::DOUBLE {
            Some(read_tagged_double(data, &mut pos)?)
        } else {
            None
        };

        let header = SatHeader {
            version,
            num_records,
            num_bodies,
            has_history,
            product_id,
            product_version,
            date,
            spatial_resolution,
            normal_tolerance,
            resfit_tolerance,
        };

        // Parse entity records
        let mut records = Vec::new();
        let mut record_index: i32 = 0;

        while pos < data.len() {
            let tag = data[pos];
            if tag == tags::ENTITY_TYPE || tag == tags::SUBTYPE {
                let (record, new_pos) = Self::read_record(data, pos, record_index)?;

                // Check for ACIS/ASM end marker — consume it so the
                // reported length covers the full payload.
                if matches!(
                    record.entity_type.as_str(),
                    "End-of-ACIS-data" | "End-of-ASM-data"
                ) {
                    pos = new_pos;
                    break;
                }

                pos = new_pos;
                records.push(record);
                record_index += 1;
            } else {
                return Err(SabError::UnknownTag(tag, pos));
            }
        }

        // Normalize pre-7.0 SAB records to v700 token layout by inserting
        // synthetic sentinel pointers, just like the SAT parser does.
        // SAB v600 records have the same layout as v400 SAT text (no
        // sentinel pointers).  The SatWriter's skip_index logic expects
        // the v700 layout, so we must add sentinels here.
        if version.major < 7 {
            for record in &mut records {
                super::parser::normalize_v400_tokens(&record.entity_type, &mut record.tokens);
            }
        }

        // Convert SAB boolean tags (TRUE/FALSE) to ACIS SAT keywords.
        // SAB uses binary TRUE(0x0B)/FALSE(0x0A) tags, but SAT text
        // uses context-dependent keywords:
        //   face:    sense (forward/reversed), side (single/double)
        //   coedge:  sense (forward/reversed)
        //   edge:    sense (forward/reversed)
        //   surface: sense (forward_v/reversed_v), bounds (I/F)
        for record in &mut records {
            convert_sab_booleans(&record.entity_type, &mut record.tokens);
            name_curve_enums(&mut record.tokens);
            name_spline_keywords(&record.entity_type, &mut record.tokens);
        }

        Ok((SatDocument { header, records }, pos))
    }

    fn read_record(
        data: &[u8],
        mut pos: usize,
        index: i32,
    ) -> Result<(SatRecord, usize), SabError> {
        // Read entity type — may have multiple subtype prefixes.
        // In SAB, compound types like "fmesh-eye-attrib" are encoded as:
        //   0x0E("fmesh") + 0x0E("eye") + 0x0D("attrib")
        // We collect all 0x0E subtypes, then read the final 0x0D base type,
        // and join them with hyphens to reconstruct the SAT entity type name.
        let mut subtype_parts: Vec<String> = Vec::new();
        let entity_type;

        // Collect all subtype prefixes (0x0E)
        while pos < data.len() && data[pos] == tags::SUBTYPE {
            pos += 1;
            let (sub, new_pos) = read_length_string(data, pos)?;
            subtype_parts.push(sub);
            pos = new_pos;
        }

        // Read the base entity type (0x0D)
        if pos >= data.len() || data[pos] != tags::ENTITY_TYPE {
            return Err(SabError::UnknownTag(
                if pos < data.len() { data[pos] } else { 0 },
                pos,
            ));
        }
        pos += 1;
        let (base_type, new_pos) = read_length_string(data, pos)?;
        pos = new_pos;

        // Reconstruct compound name: subtypes joined with hyphens + base type
        if subtype_parts.is_empty() {
            entity_type = base_type;
        } else {
            subtype_parts.push(base_type);
            entity_type = subtype_parts.join("-");
        }

        let subtype_name = if entity_type.contains('-') {
            Some(entity_type.split('-').next().unwrap().to_string())
        } else {
            None
        };

        // Check for ACIS/ASM end marker (no record body)
        if matches!(entity_type.as_str(), "End-of-ACIS-data" | "End-of-ASM-data") {
            return Ok((
                SatRecord {
                    index,
                    entity_type,
                    sub_type: subtype_name,
                    attribute: SatPointer::NULL,
                    subtype_id: -1,
                    tokens: Vec::new(),
                    raw_text: None,
                },
                pos,
            ));
        }

        // Attribute pointer
        let attribute = if pos < data.len() && data[pos] == tags::POINTER {
            pos += 1;
            let val = read_i32(data, &mut pos)?;
            SatPointer::new(val)
        } else {
            SatPointer::NULL
        };

        // Subtype ID (plain integer)
        let subtype_id = if pos < data.len() && data[pos] == tags::INTEGER {
            pos += 1;
            read_i32(data, &mut pos)?
        } else {
            -1
        };

        // Remaining tokens until END_OF_RECORD
        let mut tokens = Vec::new();
        while pos < data.len() {
            let tag = data[pos];
            if tag == tags::END_OF_RECORD {
                pos += 1;
                break;
            }
            let (token, new_pos) = Self::read_token(data, pos)?;
            tokens.push(token);
            pos = new_pos;
        }

        Ok((
            SatRecord {
                index,
                entity_type,
                sub_type: subtype_name,
                attribute,
                subtype_id,
                tokens,
                raw_text: None,
            },
            pos,
        ))
    }

    fn read_token(data: &[u8], pos: usize) -> Result<(SatToken, usize), SabError> {
        if pos >= data.len() {
            return Err(SabError::UnexpectedEof);
        }

        let tag = data[pos];
        let mut pos = pos + 1;

        match tag {
            tags::POINTER => {
                let val = read_i32(data, &mut pos)?;
                Ok((SatToken::Pointer(SatPointer::new(val)), pos))
            }
            tags::TRUE => Ok((SatToken::True, pos)),
            tags::FALSE => Ok((SatToken::False, pos)),
            tags::END_OF_RECORD => Ok((SatToken::Terminator, pos)),
            tags::CHARACTER => Self::read_raw_fixed(data, tag, pos, 1),
            tags::SHORT => Self::read_raw_fixed(data, tag, pos, 2),
            tags::INTEGER | tags::FLOAT | tags::ENUM => Self::read_raw_fixed(data, tag, pos, 4),
            tags::DOUBLE | tags::INTEGER64 => Self::read_raw_fixed(data, tag, pos, 8),
            tags::POSITION | tags::DIRECTION => Self::read_raw_fixed(data, tag, pos, 24),
            tags::UV => Self::read_raw_fixed(data, tag, pos, 16),
            tags::STRING | tags::ENTITY_TYPE | tags::SUBTYPE => {
                Self::read_raw_string(data, tag, pos, 1)
            }
            tags::SHORT_STRING => Self::read_raw_string(data, tag, pos, 2),
            tags::LONG_STRING | tags::ASM_LONG_STRING => Self::read_raw_string(data, tag, pos, 4),
            // Keep nested subtype delimiters as their original zero-payload
            // SAB tags. `SatToken::as_ident` exposes them as `{` and `}` to
            // geometry consumers without losing binary identity.
            tags::SUBTYPE_START | tags::SUBTYPE_END => Ok((
                SatToken::Sab {
                    tag,
                    data: Vec::new(),
                },
                pos,
            )),
            _ => Err(SabError::UnknownTag(tag, pos - 1)),
        }
    }

    fn read_raw_fixed(
        data: &[u8],
        tag: u8,
        pos: usize,
        len: usize,
    ) -> Result<(SatToken, usize), SabError> {
        let end = pos.checked_add(len).ok_or(SabError::UnexpectedEof)?;
        if end > data.len() {
            return Err(SabError::UnexpectedEof);
        }
        Ok((
            SatToken::Sab {
                tag,
                data: data[pos..end].to_vec(),
            },
            end,
        ))
    }

    fn read_raw_string(
        data: &[u8],
        tag: u8,
        pos: usize,
        prefix_len: usize,
    ) -> Result<(SatToken, usize), SabError> {
        let prefix_end = pos.checked_add(prefix_len).ok_or(SabError::UnexpectedEof)?;
        if prefix_end > data.len() {
            return Err(SabError::UnexpectedEof);
        }
        let len = match prefix_len {
            1 => data[pos] as usize,
            2 => u16::from_le_bytes([data[pos], data[pos + 1]]) as usize,
            4 => u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                as usize,
            _ => return Err(SabError::UnexpectedEof),
        };
        let end = prefix_end.checked_add(len).ok_or(SabError::UnexpectedEof)?;
        if end > data.len() {
            return Err(SabError::UnexpectedEof);
        }
        Ok((
            SatToken::Sab {
                tag,
                data: data[pos..end].to_vec(),
            },
            end,
        ))
    }
}

// ============================================================================
// SAB boolean → SAT keyword conversion
// ============================================================================

fn name_curve_enums(tokens: &mut [SatToken]) {
    let mut depth = 0usize;
    let mut role = None;
    let mut parcur = false;
    for index in 0..tokens.len() {
        match tokens[index].as_ident() {
            Some("{") => {
                depth += 1;
                if depth == 1 {
                    parcur = tokens.get(index + 1).and_then(SatToken::as_ident) == Some("parcur");
                }
                // Every interpolated curve (`exactcur`, `surfintcur`,
                // `parcur`, ...) opens with the same `full nubs` spline.
                if tokens
                    .get(index + 1)
                    .and_then(SatToken::as_ident)
                    .is_some_and(|name| name.ends_with("cur"))
                    && depth == 1
                {
                    role = Some(0);
                }
                continue;
            }
            Some("}") => {
                // A `parcur` ends with the logical naming its surface.
                if depth == 1 && role.is_some() && index > 0 {
                    let surface = match &tokens[index - 1] {
                        SatToken::Enum(value) if value == "F" => Some("surf1"),
                        SatToken::Enum(value) if value == "I" => Some("surf2"),
                        _ => None,
                    };
                    if let (true, Some(surface)) = (parcur, surface) {
                        tokens[index - 1] = SatToken::Ident(surface.to_string());
                    }
                }
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    role = None;
                }
                continue;
            }
            // The curve spline, then any 2D pcurve spline: closure follows.
            Some("nubs" | "nurbs") if role.is_some_and(|role| role >= 1) => {
                role = Some(2);
                continue;
            }
            _ => {}
        }
        // Only the curve's own fields; nested subtypes keep their values.
        if depth != 1 {
            continue;
        }
        let (Some(current), SatToken::Sab { tag: 0x15, data }) = (role, &tokens[index]) else {
            continue;
        };
        let value = data
            .get(..4)
            .map(|bytes| i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
        let name = match (current, value) {
            (0, Some(0)) => Some("full"),
            (2, Some(0)) => Some("open"),
            (2, Some(1)) => Some("closed"),
            (2, Some(2)) => Some("periodic"),
            (3.., Some(0)) => Some("UNEXTENDED"),
            _ => None,
        };
        if let Some(name) = name {
            tokens[index] = SatToken::Enum(name.to_string());
        }
        role = Some(current + 1);
    }
}

fn name_spline_keywords(entity_type: &str, tokens: &mut [SatToken]) {
    if !matches!(entity_type, "intcurve-curve" | "spline-surface" | "pcurve") {
        return;
    }
    let logical = |token: &SatToken| match token {
        SatToken::True => Some(true),
        SatToken::False => Some(false),
        SatToken::Enum(name) => match name.as_str() {
            "I" | "forward_v" | "forward" => Some(true),
            "F" | "reverse_v" | "reversed" => Some(false),
            _ => None,
        },
        _ => None,
    };
    // (forward name, reversed name, logicals to skip before the sense)
    let mut sense: Option<(&str, &str, usize)> = (entity_type != "intcurve-curve")
        .then_some(("forward", "reversed", 0));
    // Per open block: (subtype, enumeration role).
    let mut blocks: Vec<(String, usize)> = Vec::new();
    for index in 0..tokens.len() {
        match tokens[index].as_ident() {
            Some("{") => {
                let name = tokens.get(index + 1).and_then(SatToken::as_ident).unwrap_or("");
                blocks.push((name.to_string(), 0));
                continue;
            }
            Some("}") => {
                blocks.pop();
                continue;
            }
            Some("nubs" | "nurbs") => {
                if let Some((_, role)) = blocks.last_mut() {
                    *role = 1;
                }
                continue;
            }
            Some("plane" | "sphere" | "torus") if !blocks.is_empty() => {
                sense = Some(("forward_v", "reverse_v", 0));
                continue;
            }
            Some("cone") if !blocks.is_empty() => {
                sense = Some(("forward", "reversed", 2));
                continue;
            }
            Some("spline") if !blocks.is_empty() => {
                sense = Some(("forward", "reversed", 0));
                continue;
            }
            _ => {}
        }
        if let (Some((forward, reversed, skip)), Some(value)) = (sense, logical(&tokens[index])) {
            if skip > 0 {
                sense = Some((forward, reversed, skip - 1));
            } else {
                tokens[index] = SatToken::Enum(if value { forward } else { reversed }.to_string());
                sense = None;
            }
            continue;
        }
        let Some((block, role)) = blocks.last_mut() else { continue };
        let SatToken::Sab { tag: 0x15, data } = &tokens[index] else { continue };
        let value = data
            .get(..4)
            .map(|bytes| i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
        let name = match (block.as_str(), *role, value) {
            ("exactsur", 0, Some(0)) => Some("full"),
            ("exactsur", 1..=2, Some(0)) | ("exppc", 1, Some(0)) => Some("open"),
            ("exactsur", 1..=2, Some(1)) | ("exppc", 1, Some(1)) => Some("closed"),
            ("exactsur", 1..=2, Some(2)) | ("exppc", 1, Some(2)) => Some("periodic"),
            ("exactsur", 3..=4, Some(0)) => Some("none"),
            _ => None,
        };
        if let Some(name) = name {
            tokens[index] = SatToken::Enum(name.to_string());
        }
        if *role > 0 {
            *role += 1;
        }
    }
    // The remaining logicals of a pcurve (its inline surface's bounds) are
    // infinite/finite flags.
    if entity_type == "pcurve" {
        for token in tokens.iter_mut() {
            if let Some(infinite) = match token {
                SatToken::True => Some(true),
                SatToken::False => Some(false),
                _ => None,
            } {
                *token = SatToken::Enum(if infinite { "I" } else { "F" }.to_string());
            }
        }
    }
}

/// Convert SAB TRUE/FALSE tokens to ACIS SAT keywords based on entity context.
///
/// SAB binary uses generic TRUE(0x0B)/FALSE(0x0A) tags for all boolean fields.
/// SAT text uses context-dependent keywords:
///   - face sense: `forward` / `reversed`
///   - face side: `single` / `double`
///   - coedge sense: `forward` / `reversed`
///   - edge sense: `forward` / `reversed`
///   - surface sense: `forward_v` / `reversed_v`
///   - surface bounds: `I` (infinite) / `F` (finite)
fn convert_sab_booleans(entity_type: &str, tokens: &mut Vec<SatToken>) {
    match base_entity_type(entity_type) {
        "face" => {
            // Modern faces end in sense, sidedness, and containment. The
            // containment bit is inverted from the generic in/out mapping.
            let bools: Vec<_> = tokens
                .iter()
                .enumerate()
                .filter(|(_, token)| matches!(token, SatToken::True | SatToken::False))
                .map(|(index, _)| index)
                .collect();
            let start = bools.len().saturating_sub(3);
            for (role, &index) in bools[start..].iter().enumerate() {
                let value = matches!(tokens[index], SatToken::True);
                let name = match role {
                    0 => {
                        if value {
                            "forward"
                        } else {
                            "reversed"
                        }
                    }
                    1 => {
                        if value {
                            "single"
                        } else {
                            "double"
                        }
                    }
                    _ => {
                        if value {
                            "out"
                        } else {
                            "in"
                        }
                    }
                };
                tokens[index] = SatToken::Enum(name.to_string());
            }
        }
        "transform" => {
            let bools: Vec<_> = tokens
                .iter()
                .enumerate()
                .filter(|(_, token)| matches!(token, SatToken::True | SatToken::False))
                .map(|(index, _)| index)
                .collect();
            let start = bools.len().saturating_sub(3);
            for (role, &index) in bools[start..].iter().enumerate() {
                let value = matches!(tokens[index], SatToken::True);
                let name = match role {
                    0 => {
                        if value {
                            "no_rotate"
                        } else {
                            "rotate"
                        }
                    }
                    1 => {
                        if value {
                            "no_reflect"
                        } else {
                            "reflect"
                        }
                    }
                    _ => {
                        if value {
                            "no_shear"
                        } else {
                            "shear"
                        }
                    }
                };
                tokens[index] = SatToken::Ident(name.to_string());
            }
        }
        "wire" => {
            // wire: ... $shell $subshell side #. TRUE is "out", as in the
            // reference application's SAT text of a polyline profile body.
            if let Some(token) = tokens
                .iter_mut()
                .rev()
                .find(|token| matches!(token, SatToken::True | SatToken::False))
            {
                let out = matches!(token, SatToken::True);
                *token = SatToken::Enum(if out { "out" } else { "in" }.to_string());
            }
        }
        "coedge" => {
            // coedge: ... $edge sense $loop $pcurve #
            // Find the first True/False token and convert to sense
            for token in tokens.iter_mut() {
                if matches!(token, SatToken::True | SatToken::False) {
                    let is_forward = matches!(token, SatToken::True);
                    *token =
                        SatToken::Enum(if is_forward { "forward" } else { "reversed" }.to_string());
                    break; // only the first boolean is sense
                }
            }
        }
        "edge" => {
            // edge: ... $coedge $curve sense convexity unknown #
            // Find True/False tokens: first is sense
            for token in tokens.iter_mut() {
                if matches!(token, SatToken::True | SatToken::False) {
                    let is_forward = matches!(token, SatToken::True);
                    *token =
                        SatToken::Enum(if is_forward { "forward" } else { "reversed" }.to_string());
                    break;
                }
            }
        }
        _ if entity_type == "cone-surface" => {
            // cone-surface in v400: bool layout is position-dependent.
            // SAB stores booleans at the same structural positions as SAT, but
            // v600 and v400 interpret them differently:
            //   bool 0,1 → bounds (I/F)
            //   bool 2   → sense (forward/reversed)
            //   bool 3+  → bounds (I/F)
            let mut bool_idx = 0u32;
            for token in tokens.iter_mut() {
                if matches!(token, SatToken::True | SatToken::False) {
                    let is_true = matches!(token, SatToken::True);
                    *token = if bool_idx == 2 {
                        SatToken::Enum(if is_true { "forward" } else { "reversed" }.to_string())
                    } else {
                        SatToken::Enum(if is_true { "I" } else { "F" }.to_string())
                    };
                    bool_idx += 1;
                }
            }
        }
        _ if entity_type.ends_with("-surface") => {
            // Generic surface: first bool = sense (forward_v/reverse_v),
            // remaining bools = bound infinity (I/F).
            // Note: v400 ACIS uses "reverse_v" (not "reversed_v").
            let mut first = true;
            for token in tokens.iter_mut() {
                if matches!(token, SatToken::True | SatToken::False) {
                    if first {
                        let is_forward = matches!(token, SatToken::True);
                        *token = SatToken::Enum(
                            if is_forward { "forward_v" } else { "reverse_v" }.to_string(),
                        );
                        first = false;
                    } else {
                        let is_infinite = matches!(token, SatToken::True);
                        *token = SatToken::Enum(if is_infinite { "I" } else { "F" }.to_string());
                    }
                }
            }
        }
        _ if entity_type.ends_with("-curve") => {
            // Curve entities have varying boolean layouts:
            //
            // straight-curve, ellipse-curve: all booleans are bounds (I/F).
            //   In v400, ellipse-curve has no sense — just 2 bounds.
            //
            // intcurve-curve, bs2-curve, bs3-curve: first bool = sense, rest = bounds.
            let has_sense = entity_type != "straight-curve" && entity_type != "ellipse-curve";
            let mut found_sense = false;
            for token in tokens.iter_mut() {
                if matches!(token, SatToken::True | SatToken::False) {
                    if has_sense && !found_sense {
                        // First boolean is sense for curves that have it
                        let is_forward = matches!(token, SatToken::True);
                        *token = SatToken::Enum(
                            if is_forward { "forward" } else { "reversed" }.to_string(),
                        );
                        found_sense = true;
                    } else {
                        // Bound: True=Infinite(I), False=Finite(F)
                        let is_infinite = matches!(token, SatToken::True);
                        *token = SatToken::Enum(if is_infinite { "I" } else { "F" }.to_string());
                    }
                }
            }
        }
        _ => {}
    }
}

// ============================================================================
// Binary reading helpers
// ============================================================================

fn read_u32(data: &[u8], pos: &mut usize) -> Result<u32, SabError> {
    if *pos + 4 > data.len() {
        return Err(SabError::UnexpectedEof);
    }
    let val = u32::from_le_bytes([data[*pos], data[*pos + 1], data[*pos + 2], data[*pos + 3]]);
    *pos += 4;
    Ok(val)
}

fn read_i32(data: &[u8], pos: &mut usize) -> Result<i32, SabError> {
    if *pos + 4 > data.len() {
        return Err(SabError::UnexpectedEof);
    }
    let val = i32::from_le_bytes([data[*pos], data[*pos + 1], data[*pos + 2], data[*pos + 3]]);
    *pos += 4;
    Ok(val)
}

fn read_f64(data: &[u8], pos: &mut usize) -> Result<f64, SabError> {
    if *pos + 8 > data.len() {
        return Err(SabError::UnexpectedEof);
    }
    let val = f64::from_le_bytes([
        data[*pos],
        data[*pos + 1],
        data[*pos + 2],
        data[*pos + 3],
        data[*pos + 4],
        data[*pos + 5],
        data[*pos + 6],
        data[*pos + 7],
    ]);
    *pos += 8;
    Ok(val)
}

fn read_length_string(data: &[u8], pos: usize) -> Result<(String, usize), SabError> {
    if pos >= data.len() {
        return Err(SabError::UnexpectedEof);
    }
    let len = data[pos] as usize;
    let start = pos + 1;
    if start + len > data.len() {
        return Err(SabError::UnexpectedEof);
    }
    let s = std::str::from_utf8(&data[start..start + len])
        .map_err(|_| SabError::InvalidString)?
        .to_string();
    Ok((s, start + len))
}

fn read_tagged_string(data: &[u8], pos: &mut usize) -> Result<String, SabError> {
    if *pos >= data.len() {
        return Err(SabError::UnknownTag(0, *pos));
    }
    let tag = data[*pos];
    *pos += 1;
    let prefix_len = match tag {
        tags::STRING => 1,
        tags::SHORT_STRING => 2,
        tags::LONG_STRING | tags::ASM_LONG_STRING => 4,
        _ => return Err(SabError::UnknownTag(tag, *pos - 1)),
    };
    let prefix_end = (*pos)
        .checked_add(prefix_len)
        .ok_or(SabError::UnexpectedEof)?;
    if prefix_end > data.len() {
        return Err(SabError::UnexpectedEof);
    }
    let len = match prefix_len {
        1 => data[*pos] as usize,
        2 => u16::from_le_bytes([data[*pos], data[*pos + 1]]) as usize,
        4 => u32::from_le_bytes([data[*pos], data[*pos + 1], data[*pos + 2], data[*pos + 3]])
            as usize,
        _ => return Err(SabError::UnexpectedEof),
    };
    let end = prefix_end.checked_add(len).ok_or(SabError::UnexpectedEof)?;
    if end > data.len() {
        return Err(SabError::UnexpectedEof);
    }
    let value = std::str::from_utf8(&data[prefix_end..end])
        .map_err(|_| SabError::InvalidString)?
        .to_string();
    *pos = end;
    Ok(value)
}

fn read_tagged_double(data: &[u8], pos: &mut usize) -> Result<f64, SabError> {
    if *pos >= data.len() || data[*pos] != tags::DOUBLE {
        return Err(SabError::UnknownTag(
            if *pos < data.len() { data[*pos] } else { 0 },
            *pos,
        ));
    }
    *pos += 1;
    read_f64(data, pos)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asm_flags_preserve_the_history_bit() {
        for version in [21800, 22300] {
            for history in [false, true] {
                let mut doc = SatDocument::new();
                doc.header.version = SatVersion::from_sat_number(version);
                doc.header.has_history = history;
                let bytes = SabWriter::write(&doc);
                assert_eq!(u32::from_le_bytes(bytes[27..31].try_into().unwrap()), 12 | u32::from(history));
                let loaded = SabReader::read(&bytes).unwrap();
                assert_eq!(loaded.header.has_history, history);
                assert_eq!(SabWriter::write(&loaded), bytes);
            }
        }
    }

    #[test]
    fn test_sat_to_sab_header() {
        let mut doc = SatDocument::new();
        doc.header.product_id = "TestApp".to_string();
        doc.header.product_version = "ACIS 7.0".to_string();
        doc.header.date = "Thu Jan 01 00:00:00 2023".to_string();
        doc.header.spatial_resolution = 10.0;
        doc.header.normal_tolerance = 1e-06;
        doc.header.resfit_tolerance = Some(1e-10);

        let sab = SabWriter::write(&doc);

        // Check magic
        assert_eq!(&sab[..15], b"ACIS BinaryFile");
        // Check version
        let ver = u32::from_le_bytes([sab[15], sab[16], sab[17], sab[18]]);
        assert_eq!(ver, 700);
        // The End-of-ACIS-data marker closes the body as tagged components.
        let mut end = Vec::new();
        for part in ["End", "of", "ACIS"] {
            SabWriter::write_subtype(&mut end, part);
        }
        SabWriter::write_entity_type(&mut end, "data");
        assert!(sab.ends_with(&end));
    }

    #[test]
    fn test_sat_to_sab_roundtrip() {
        let sat_text = "700 0 1 0\n\
            @8 acadrust @8 ACIS 7.0 @24 Thu Jan 01 00:00:00 2023\n\
            10 9.9999999999999995e-007 1e-010\n\
            body $-1 -1 $-1 $1 $-1 $-1 #\n\
            lump $-1 -1 $-1 $-1 $2 $0 #\n\
            shell $-1 -1 $-1 $-1 $-1 $3 $-1 $1 #\n\
            face $-1 -1 $-1 $-1 $-1 $2 $-1 $4 forward single #\n\
            plane-surface $-1 -1 $-1 0 0 5 0 0 1 1 0 0 forward_v I I I I #\n\
            End-of-ACIS-data\n";

        let doc = SatDocument::parse(sat_text).unwrap();
        let sab = SabWriter::write(&doc);
        let roundtrip = SabReader::read(&sab).unwrap();

        assert_eq!(roundtrip.header.version, doc.header.version);
        assert_eq!(roundtrip.records.len(), doc.records.len());
        assert_eq!(roundtrip.records[0].entity_type, "body");
        assert_eq!(roundtrip.records[3].entity_type, "face");
        assert_eq!(roundtrip.records[4].entity_type, "plane-surface");
    }

    #[test]
    fn test_compound_entity_types() {
        let sat_text = "700 0 1 0\n\
            @8 acadrust @8 ACIS 7.0 @24 Thu Jan 01 00:00:00 2023\n\
            10 9.9999999999999995e-007 1e-010\n\
            plane-surface $-1 -1 $-1 0 0 5 0 0 1 1 0 0 forward_v I I I I #\n\
            straight-curve $-1 -1 $-1 0 0 0 1 0 0 I I #\n\
            End-of-ACIS-data\n";

        let doc = SatDocument::parse(sat_text).unwrap();
        let sab = SabWriter::write(&doc);

        // Verify subtype tags are present
        assert!(sab.contains(&tags::SUBTYPE));

        // Roundtrip
        let roundtrip = SabReader::read(&sab).unwrap();
        assert_eq!(roundtrip.records[0].entity_type, "plane-surface");
        assert_eq!(roundtrip.records[1].entity_type, "straight-curve");
    }

    #[test]
    fn test_sab_boolean_mapping() {
        let sat_text = "700 0 1 0\n\
            @8 acadrust @8 ACIS 7.0 @24 Thu Jan 01 00:00:00 2023\n\
            10 9.9999999999999995e-007 1e-010\n\
            face $-1 -1 $-1 $-1 $-1 $-1 $-1 $-1 forward single #\n\
            face $-1 -1 $-1 $-1 $-1 $-1 $-1 $-1 reversed double #\n\
            End-of-ACIS-data\n";

        let doc = SatDocument::parse(sat_text).unwrap();
        let sab = SabWriter::write(&doc);
        let roundtrip = SabReader::read(&sab).unwrap();

        // forward/single → Enum("forward"), Enum("single")
        let face1 = &roundtrip.records[0];
        let last_two: Vec<_> = face1.tokens.iter().rev().take(2).collect();
        assert_eq!(last_two[0], &SatToken::Enum("single".to_string()));
        assert_eq!(last_two[1], &SatToken::Enum("forward".to_string()));

        // reversed/double → Enum("reversed"), Enum("double")
        let face2 = &roundtrip.records[1];
        let last_two: Vec<_> = face2.tokens.iter().rev().take(2).collect();
        assert_eq!(last_two[0], &SatToken::Enum("double".to_string()));
        assert_eq!(last_two[1], &SatToken::Enum("reversed".to_string()));
    }

    #[test]
    fn modern_face_and_transform_boolean_roles_roundtrip() {
        let sat = "21200 2 1 26\n0  0  0 \n1 0.000001 0.0000000001\n\
            face $-1 -1 $-1 $-1 $-1 $-1 forward double out #\n\
            transform $-1 -1 1 0 0 0 1 0 0 0 1 5 6 7 1 no_rotate no_reflect no_shear #\n\
            End-of-ACIS-data\n";
        let document = SatDocument::parse(sat).unwrap();
        let roundtrip = SabReader::read(&SabWriter::write(&document)).unwrap();
        assert_eq!(roundtrip.records[0].tokens, document.records[0].tokens);
        assert_eq!(roundtrip.placement(), document.placement());
        let transform = roundtrip
            .records
            .iter()
            .find(|record| record.entity_type == "transform")
            .unwrap();
        assert_eq!(
            transform.tokens[transform.tokens.len() - 3..]
                .iter()
                .filter_map(SatToken::as_ident)
                .collect::<Vec<_>>(),
            ["no_rotate", "no_reflect", "no_shear"]
        );
        let text = roundtrip.to_sat_string();
        assert!(text.starts_with("21200 2 1 1\n"));
        assert!(text.contains(" forward double out #\n"));
        assert!(text.contains(" no_rotate no_reflect no_shear #\n"));
    }

    #[test]
    fn sab_preserves_nurbs_integer_fields() {
        let mut doc = SatDocument::new_body();
        let knots = [(0.0, 3), (1.0, 3)];
        let _curve = doc.add_spline_curve(
            true,
            2,
            false,
            &knots,
            &[[0.0, 0.0, 0.0], [1.0, 1.0, 0.0], [2.0, 0.0, 0.0]],
            Some(&[1.0, 0.5, 1.0]),
            1.0e-9,
        );
        let surface = doc.add_spline_surface(
            false,
            true,
            1,
            1,
            false,
            false,
            &[(0.0, 2), (1.0, 2)],
            &[(0.0, 2), (1.0, 2)],
            &[
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [1.0, 1.0, 0.0],
            ],
            Some(&[1.0, 0.5, 0.75, 1.0]),
            1.0e-9,
        );
        let _pcurve = doc.add_pcurve(
            false,
            1,
            false,
            &[(0.0, 2), (1.0, 2)],
            &[[0.0, 0.0], [1.0, 1.0]],
            None,
            1.0e-9,
            surface,
            (0.0, 0.0),
        );

        let roundtrip = SabReader::read(&SabWriter::write(&doc)).unwrap();
        let curve = SatIntCurve::from_record(roundtrip.records_of_type("intcurve-curve")[0])
            .expect("NURBS curve record");
        assert_eq!(curve.bspline().expect("decoded NURBS curve").0, 2);
        let surface = SatSplineSurface::from_record(roundtrip.records_of_type("spline-surface")[0])
            .expect("NURBS surface record");
        let decoded_surface = surface.bspline(&roundtrip).expect("decoded NURBS surface");
        assert_eq!((decoded_surface.degree_u, decoded_surface.degree_v), (1, 1));
        assert_eq!(
            (
                decoded_surface.control_count_u,
                decoded_surface.control_count_v
            ),
            (2, 2)
        );
        assert_eq!(decoded_surface.u_knots, vec![0.0, 0.0, 1.0, 1.0]);
        assert_eq!(
            decoded_surface.control_points,
            vec![
                [0.0, 0.0, 0.0, 1.0],
                [0.5, 0.0, 0.0, 0.5],
                [0.0, 0.75, 0.0, 0.75],
                [1.0, 1.0, 0.0, 1.0]
            ]
        );
        let tokens = &roundtrip.records_of_type("spline-surface")[0].tokens;
        let block = tokens
            .iter()
            .position(|t| t.as_ident() == Some("nurbs"))
            .unwrap();
        assert!(matches!(
            tokens[block],
            SatToken::Sab {
                tag: tags::ENTITY_TYPE,
                ..
            }
        ));
        assert!(matches!(
            tokens[block + 1],
            SatToken::Sab {
                tag: tags::INTEGER,
                ..
            }
        ));
        assert_eq!(tokens[block + 3].as_ident(), Some("both"));
        assert_eq!(tokens[block + 4], SatToken::Enum("open".to_string()));
        assert!(matches!(
            tokens[block + 10],
            SatToken::Sab {
                tag: tags::DOUBLE,
                ..
            }
        ));
        assert!(matches!(
            tokens[block + 11],
            SatToken::Sab {
                tag: tags::INTEGER,
                ..
            }
        ));
        let pcurve = SatPCurve::from_record(roundtrip.records_of_type("pcurve")[0])
            .expect("NURBS pcurve record");
        assert_eq!(
            pcurve
                .bspline_in(&roundtrip)
                .expect("decoded NURBS pcurve")
                .0,
            1
        );
    }

    #[test]
    fn sat_integer_spline_coordinates_survive_sab() {
        let mut doc = SatDocument::new_body();
        doc.add_spline_curve(
            false,
            2,
            false,
            &[(0.0, 3), (1.0, 3)],
            &[[0.0, 0.0, 0.0], [1.0, 1.0, 0.0], [2.0, 0.0, 0.0]],
            None,
            0.0,
        );

        // A permissive reader accepts integer coordinates, but ACIS requires
        // DOUBLE tags. Compare against the binary output of the typed builder.
        let parsed_text = SatDocument::parse(&doc.to_sat_string()).unwrap();
        assert_eq!(SabWriter::write(&parsed_text), SabWriter::write(&doc));
        let roundtrip = SabReader::read(&SabWriter::write(&parsed_text)).unwrap();
        let curve = SatIntCurve::from_record(roundtrip.records_of_type("intcurve-curve")[0])
            .expect("NURBS curve record");
        let (_, _, controls) = curve.bspline().expect("decoded NURBS curve");
        assert_eq!(controls[1], [1.0, 1.0, 0.0, 1.0]);
    }

    #[test]
    fn explicit_pcurve_sense_survives_sat_and_sab() {
        for sense in [Sense::Forward, Sense::Reversed] {
            let mut doc = SatDocument::new_body();
            let plane = doc.add_plane_surface([0.; 3], [0., 0., 1.], [1., 0., 0.]);
            let index = doc.add_pcurve(
                false,
                1,
                false,
                &[(0., 2), (1., 2)],
                &[[0., 0.], [1., 0.]],
                None,
                1e-9,
                plane,
                (0., 0.),
            );
            doc.records[index as usize].tokens[2] = SatToken::Ident(
                if sense == Sense::Reversed {
                    "reversed"
                } else {
                    "forward"
                }
                .into(),
            );
            for read in [
                SatDocument::parse(&doc.to_sat_string()).unwrap(),
                SabReader::read(&SabWriter::write(&doc)).unwrap(),
            ] {
                let curve = SatPCurve::from_record(&read.records[index as usize]).unwrap();
                assert_eq!(curve.sense(), sense);
            }
        }
    }

    #[test]
    fn sat_nurbs_fields_retain_binary_roles() {
        for rational in [false, true] {
            let mut doc = SatDocument::new_body();
            doc.add_spline_curve(
                rational,
                1,
                false,
                &[(0., 2), (1., 2)],
                &[[0., 0., 0.], [1., 1., 0.]],
                Some(&[1., 1.]),
                0.,
            );
            let spline = doc.add_spline_surface(
                false,
                rational,
                1,
                1,
                false,
                false,
                &[(0., 2), (1., 2)],
                &[(0., 2), (1., 2)],
                &[[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [1., 1., 0.]],
                Some(&[1.; 4]),
                0.,
            );
            let plane = doc.add_plane_surface([0.; 3], [0., 0., 1.], [1., 0., 0.]);
            let cone = doc.add_cone_surface([0.; 3], [0., 0., 1.], [2., 0., 0.], 1., 1., 0.);
            let sphere = doc.add_sphere_surface([0.; 3], 2., [1., 0., 0.], [0., 0., 1.]);
            let torus = doc.add_torus_surface([0.; 3], [0., 0., 1.], 3., 1., [1., 0., 0.]);
            for support in [spline, plane, cone, sphere, torus] {
                doc.add_pcurve(
                    rational,
                    1,
                    false,
                    &[(0., 2), (1., 2)],
                    &[[0., 0.], [1., 1.]],
                    Some(&[1., 1.]),
                    0.,
                    support,
                    (1., 0.),
                );
            }
            let parsed = SatDocument::parse(&doc.to_sat_string()).unwrap();
            let expected = SabWriter::write(&doc);
            assert_eq!(SabWriter::write(&parsed), expected, "rational={rational}");
            let raw = SabReader::read(&expected).unwrap();
            assert_eq!(
                SabWriter::write(&raw),
                expected,
                "raw SAB must remain unchanged"
            );
            for version in [
                crate::DxfVersion::AC1018,
                crate::DxfVersion::AC1021,
                crate::DxfVersion::AC1024,
                crate::DxfVersion::AC1027,
                crate::DxfVersion::AC1032,
            ] {
                let mut drawing = crate::CadDocument::with_version(version);
                let solid = crate::entities::Solid3D::from_sat(&doc.to_sat_string());
                drawing
                    .add_entity(crate::EntityType::Solid3D(solid))
                    .unwrap();
                let bytes = crate::DwgWriter::write_to_vec(&drawing).unwrap();
                let read = crate::DwgReader::from_stream(std::io::Cursor::new(bytes))
                    .read()
                    .unwrap();
                let crate::EntityType::Solid3D(solid) = read.entities().next().unwrap() else {
                    panic!("missing solid");
                };
                assert_eq!(solid.acis_data.sab_data, expected, "DWG {version:?}");
            }
        }
    }
}
