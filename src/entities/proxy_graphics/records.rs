//! Typed metafile commands. Coordinates are source values, never tessellated or transformed here.
use super::{ProxyGraphicErrorReason as Error, Vector3};

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ProxyGraphicCommand {
    Extents([Vector3; 2]),
    Circle {
        center: Vector3,
        radius: f64,
        normal: Vector3,
    },
    Circle3([Vector3; 3]),
    CircularArc {
        center: Vector3,
        radius: f64,
        normal: Vector3,
        start_direction: Vector3,
        sweep_angle: f64,
        arc_type: i32,
    },
    CircularArc3 {
        points: [Vector3; 3],
        arc_type: i32,
    },
    Polyline(Vec<Vector3>),
    Polygon(Vec<Vector3>),
    PolylineWithNormal {
        vertices: Vec<Vector3>,
        normal: Vector3,
    },
    /// Sixteen row-major source matrix values, without applying them to geometry.
    PushTransform([f64; 16]),
    PopTransform,
    ColorIndex(u32),
    LayerIndex(u32),
    LineTypeIndex(u32),
    SelectionMarker(u32),
    Fill(bool),
    EncodedColor(u32),
    LineWeight(i32),
    LineTypeScale(f64),
    Thickness(f64),
    StyledUnicodeText(Box<ProxyStyledUnicodeText>),
    LegacyText(Box<ProxyLegacyText>),
    /// Exact type51/u32(0) compatibility control; no renderer no-op claim.
    Compatibility51Zero,
}

impl ProxyGraphicCommand {
    /// Metafile record code; it identifies a source record, not an entity.
    pub fn record_type(&self) -> u32 {
        use ProxyGraphicCommand as C;
        match self {
            C::Extents(_) => 1,
            C::Circle { .. } => 2,
            C::Circle3(_) => 3,
            C::CircularArc { .. } => 4,
            C::CircularArc3 { .. } => 5,
            C::Polyline(_) => 6,
            C::Polygon(_) => 7,
            C::ColorIndex(_) => 14,
            C::LayerIndex(_) => 16,
            C::LineTypeIndex(_) => 18,
            C::SelectionMarker(_) => 19,
            C::Fill(_) => 20,
            C::EncodedColor(_) => 22,
            C::LineWeight(_) => 23,
            C::LineTypeScale(_) => 24,
            C::Thickness(_) => 25,
            C::PushTransform(_) => 29,
            C::PopTransform => 31,
            C::PolylineWithNormal { .. } => 32,
            C::StyledUnicodeText(_) => 38,
            C::LegacyText(text) => {
                if text.styling.is_some() {
                    11
                } else {
                    10
                }
            }
            C::Compatibility51Zero => 51,
        }
    }
}

/// Codepage-dependent text bytes. Decode them only with the owning drawing's
/// explicit encoding; these bytes are not implicitly UTF8 or replacement text.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProxyLegacyText {
    pub position: Vector3,
    pub normal: Vector3,
    pub direction: Vector3,
    pub text: Vec<u8>,
    pub height: f64,
    pub width_factor: f64,
    pub oblique_angle: f64,
    pub styling: Option<ProxyLegacyTextStyle>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProxyLegacyTextStyle {
    pub declared_length: i32,
    pub processing_mode: i32,
    pub tracking: f64,
    pub text_flags: [bool; 5],
    pub font_file: Vec<u8>,
    pub big_font_file: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProxyStyledUnicodeText {
    pub position: Vector3,
    pub normal: Vector3,
    pub direction: Vector3,
    pub text: String,
    pub declared_length: i32,
    pub processing_mode: i32,
    pub height: f64,
    pub width_factor: f64,
    pub oblique_angle: f64,
    pub tracking: f64,
    /// Backwards, upside-down, vertical, underline, overline, in source order.
    pub text_flags: [bool; 5],
    pub bold: bool,
    pub italic: bool,
    pub charset: u32,
    pub pitch_family: u32,
    pub family: String,
    pub font_file: String,
    pub big_font_file: String,
}

struct Input<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl Input<'_> {
    fn bytes<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        let end = self.offset.checked_add(N).ok_or(Error::Overflow)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(Error::PayloadLength)?;
        self.offset = end;
        value.try_into().map_err(|_| Error::PayloadLength)
    }
    fn word(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(self.bytes()?))
    }
    fn number(&mut self) -> Result<f64, Error> {
        let value = f64::from_le_bytes(self.bytes()?);
        if value.is_finite() {
            Ok(value)
        } else {
            Err(Error::InvalidValue)
        }
    }
    fn point(&mut self) -> Result<Vector3, Error> {
        Ok(Vector3::new(self.number()?, self.number()?, self.number()?))
    }
    fn flag(&mut self) -> Result<bool, Error> {
        match self.word()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::InvalidValue),
        }
    }
    fn text(&mut self) -> Result<String, Error> {
        let mut units = Vec::new();
        loop {
            let unit = u16::from_le_bytes(self.bytes()?);
            if unit == 0 {
                break;
            }
            units.push(unit);
        }
        while self.offset % 4 != 0 {
            // Alignment bytes in CAD-authored records can contain arbitrary
            // allocator residue. They are not characters or source semantics.
            self.bytes::<1>()?;
        }
        String::from_utf16(&units).map_err(|_| Error::InvalidString)
    }
    fn byte_text(&mut self) -> Result<Vec<u8>, Error> {
        let mut bytes = Vec::new();
        loop {
            let [byte] = self.bytes::<1>()?;
            if byte == 0 {
                break;
            }
            bytes.push(byte);
        }
        while self.offset % 4 != 0 {
            self.bytes::<1>()?;
        }
        Ok(bytes)
    }
}

pub(super) fn decode(kind: u32, bytes: &[u8]) -> Result<Option<ProxyGraphicCommand>, Error> {
    use ProxyGraphicCommand as C;
    let mut input = Input { bytes, offset: 0 };
    let command = match kind {
        1 => C::Extents([input.point()?, input.point()?]),
        2 => C::Circle {
            center: input.point()?,
            radius: input.number()?,
            normal: input.point()?,
        },
        3 => C::Circle3([input.point()?, input.point()?, input.point()?]),
        4 => C::CircularArc {
            center: input.point()?,
            radius: input.number()?,
            normal: input.point()?,
            start_direction: input.point()?,
            sweep_angle: input.number()?,
            arc_type: input.word()? as i32,
        },
        5 => C::CircularArc3 {
            points: [input.point()?, input.point()?, input.point()?],
            arc_type: input.word()? as i32,
        },
        6 | 7 | 32 => {
            let count = input.word()? as usize;
            if count > bytes.len().saturating_sub(4) / 24 {
                return Err(Error::PayloadLength);
            }
            let mut points = Vec::with_capacity(count);
            for _ in 0..count {
                points.push(input.point()?);
            }
            if kind == 32 {
                C::PolylineWithNormal {
                    vertices: points,
                    normal: input.point()?,
                }
            } else if kind == 6 {
                C::Polyline(points)
            } else {
                C::Polygon(points)
            }
        }
        14 => {
            let value = input.word()?;
            if value > 256 {
                return Err(Error::InvalidValue);
            }
            C::ColorIndex(value)
        }
        16 => C::LayerIndex(input.word()?),
        18 => C::LineTypeIndex(input.word()?),
        19 => C::SelectionMarker(input.word()?),
        20 => C::Fill(match input.word()? {
            1 => true,
            2 => false,
            _ => return Err(Error::InvalidValue),
        }),
        22 => C::EncodedColor(input.word()?),
        23 => C::LineWeight(input.word()? as i32),
        24 => C::LineTypeScale(input.number()?),
        25 => C::Thickness(input.number()?),
        29 => {
            let mut matrix = [0.; 16];
            for value in &mut matrix {
                *value = input.number()?;
            }
            C::PushTransform(matrix)
        }
        31 => C::PopTransform,
        10 | 11 => {
            let position = input.point()?;
            let normal = input.point()?;
            let direction = input.point()?;
            let (text, height, width_factor, oblique_angle, styling) = if kind == 10 {
                let height = input.number()?;
                let width_factor = input.number()?;
                let oblique_angle = input.number()?;
                (
                    input.byte_text()?,
                    height,
                    width_factor,
                    oblique_angle,
                    None,
                )
            } else {
                let text = input.byte_text()?;
                let declared_length = input.word()? as i32;
                let processing_mode = input.word()? as i32;
                if declared_length < -1 || !matches!(processing_mode, 0 | 1) {
                    return Err(Error::InvalidValue);
                }
                let height = input.number()?;
                let width_factor = input.number()?;
                let oblique_angle = input.number()?;
                let tracking = input.number()?;
                let text_flags = [
                    input.flag()?,
                    input.flag()?,
                    input.flag()?,
                    input.flag()?,
                    input.flag()?,
                ];
                let font_file = input.byte_text()?;
                let big_font_file = input.byte_text()?;
                (
                    text,
                    height,
                    width_factor,
                    oblique_angle,
                    Some(ProxyLegacyTextStyle {
                        declared_length,
                        processing_mode,
                        tracking,
                        text_flags,
                        font_file,
                        big_font_file,
                    }),
                )
            };
            C::LegacyText(Box::new(ProxyLegacyText {
                position,
                normal,
                direction,
                text,
                height,
                width_factor,
                oblique_angle,
                styling,
            }))
        }
        38 => {
            let position = input.point()?;
            let normal = input.point()?;
            let direction = input.point()?;
            let text = input.text()?;
            let declared_length = input.word()? as i32;
            let processing_mode = input.word()? as i32;
            if !matches!(processing_mode, 0 | 1) || declared_length < -1 {
                return Err(Error::InvalidValue);
            }
            if declared_length >= 0 && declared_length as usize != text.encode_utf16().count() {
                return Err(Error::InvalidString);
            }
            let height = input.number()?;
            let width_factor = input.number()?;
            let oblique_angle = input.number()?;
            let tracking = input.number()?;
            let text_flags = [
                input.flag()?,
                input.flag()?,
                input.flag()?,
                input.flag()?,
                input.flag()?,
            ];
            let bold = input.flag()?;
            let italic = input.flag()?;
            let charset = input.word()?;
            let pitch_family = input.word()?;
            let family = input.text()?;
            let font_file = input.text()?;
            let big_font_file = input.text()?;
            C::StyledUnicodeText(Box::new(ProxyStyledUnicodeText {
                position,
                normal,
                direction,
                text,
                declared_length,
                processing_mode,
                height,
                width_factor,
                oblique_angle,
                tracking,
                text_flags,
                bold,
                italic,
                charset,
                pitch_family,
                family,
                font_file,
                big_font_file,
            }))
        }
        51 if bytes == [0, 0, 0, 0] => {
            input.offset = 4;
            C::Compatibility51Zero
        }
        _ => return Ok(None),
    };
    if input.offset != bytes.len() {
        return Err(Error::PayloadLength);
    }
    Ok(Some(command))
}

fn point(output: &mut Vec<u8>, value: Vector3) {
    for value in [value.x, value.y, value.z] {
        output.extend_from_slice(&value.to_le_bytes());
    }
}
fn text(output: &mut Vec<u8>, value: &str) -> Option<()> {
    if value.contains('\0') {
        return None;
    }
    for unit in value.encode_utf16().chain(std::iter::once(0)) {
        output.extend_from_slice(&unit.to_le_bytes());
    }
    while output.len() % 4 != 0 {
        output.push(0);
    }
    Some(())
}

fn byte_text(output: &mut Vec<u8>, value: &[u8]) -> Option<()> {
    if value.contains(&0) {
        return None;
    }
    output.extend_from_slice(value);
    output.push(0);
    while output.len() % 4 != 0 {
        output.push(0);
    }
    Some(())
}

pub(super) fn encode(command: &ProxyGraphicCommand) -> Option<(u32, Vec<u8>)> {
    use ProxyGraphicCommand as C;
    let mut bytes = Vec::new();
    let kind = match command {
        C::Extents(points) => {
            for value in points {
                point(&mut bytes, *value);
            }
            1
        }
        C::Circle {
            center,
            radius,
            normal,
        } => {
            point(&mut bytes, *center);
            bytes.extend_from_slice(&radius.to_le_bytes());
            point(&mut bytes, *normal);
            2
        }
        C::Circle3(points) => {
            for value in points {
                point(&mut bytes, *value);
            }
            3
        }
        C::CircularArc {
            center,
            radius,
            normal,
            start_direction,
            sweep_angle,
            arc_type,
        } => {
            point(&mut bytes, *center);
            bytes.extend_from_slice(&radius.to_le_bytes());
            point(&mut bytes, *normal);
            point(&mut bytes, *start_direction);
            bytes.extend_from_slice(&sweep_angle.to_le_bytes());
            bytes.extend_from_slice(&arc_type.to_le_bytes());
            4
        }
        C::CircularArc3 { points, arc_type } => {
            for value in points {
                point(&mut bytes, *value);
            }
            bytes.extend_from_slice(&arc_type.to_le_bytes());
            5
        }
        C::Polyline(points) | C::Polygon(points) => {
            bytes.extend_from_slice(&u32::try_from(points.len()).ok()?.to_le_bytes());
            for value in points {
                point(&mut bytes, *value);
            }
            if matches!(command, C::Polyline(_)) {
                6
            } else {
                7
            }
        }
        C::PolylineWithNormal { vertices, normal } => {
            bytes.extend_from_slice(&u32::try_from(vertices.len()).ok()?.to_le_bytes());
            for value in vertices {
                point(&mut bytes, *value);
            }
            point(&mut bytes, *normal);
            32
        }
        C::PushTransform(matrix) => {
            for value in matrix {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            29
        }
        C::PopTransform => 31,
        C::ColorIndex(v)
        | C::LayerIndex(v)
        | C::LineTypeIndex(v)
        | C::SelectionMarker(v)
        | C::EncodedColor(v) => {
            bytes.extend_from_slice(&v.to_le_bytes());
            match command {
                C::ColorIndex(_) => 14,
                C::LayerIndex(_) => 16,
                C::LineTypeIndex(_) => 18,
                C::SelectionMarker(_) => 19,
                _ => 22,
            }
        }
        C::Fill(v) => {
            bytes.extend_from_slice(&(if *v { 1u32 } else { 2 }).to_le_bytes());
            20
        }
        C::LineWeight(v) => {
            bytes.extend_from_slice(&v.to_le_bytes());
            23
        }
        C::LineTypeScale(v) | C::Thickness(v) => {
            bytes.extend_from_slice(&v.to_le_bytes());
            if matches!(command, C::LineTypeScale(_)) {
                24
            } else {
                25
            }
        }
        C::Compatibility51Zero => {
            bytes.extend_from_slice(&0u32.to_le_bytes());
            51
        }
        C::StyledUnicodeText(t) => {
            for value in [t.position, t.normal, t.direction] {
                point(&mut bytes, value);
            }
            text(&mut bytes, &t.text)?;
            bytes.extend_from_slice(&t.declared_length.to_le_bytes());
            bytes.extend_from_slice(&t.processing_mode.to_le_bytes());
            for value in [t.height, t.width_factor, t.oblique_angle, t.tracking] {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            for flag in t.text_flags.into_iter().chain([t.bold, t.italic]) {
                bytes.extend_from_slice(&u32::from(flag).to_le_bytes());
            }
            bytes.extend_from_slice(&t.charset.to_le_bytes());
            bytes.extend_from_slice(&t.pitch_family.to_le_bytes());
            for value in [&t.family, &t.font_file, &t.big_font_file] {
                text(&mut bytes, value)?;
            }
            38
        }
        C::LegacyText(t) => {
            for value in [t.position, t.normal, t.direction] {
                point(&mut bytes, value);
            }
            if let Some(style) = &t.styling {
                byte_text(&mut bytes, &t.text)?;
                bytes.extend_from_slice(&style.declared_length.to_le_bytes());
                bytes.extend_from_slice(&style.processing_mode.to_le_bytes());
                for value in [t.height, t.width_factor, t.oblique_angle, style.tracking] {
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
                for flag in style.text_flags {
                    bytes.extend_from_slice(&u32::from(flag).to_le_bytes());
                }
                byte_text(&mut bytes, &style.font_file)?;
                byte_text(&mut bytes, &style.big_font_file)?;
                11
            } else {
                for value in [t.height, t.width_factor, t.oblique_angle] {
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
                byte_text(&mut bytes, &t.text)?;
                10
            }
        }
    };
    Some((kind, bytes))
}
