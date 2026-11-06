//! Genuine DXF R12 (AC1009) output.
//!
//! R12 predates the handle-based object model of R13: it has no CLASSES and
//! no OBJECTS section, table records and entities carry no handles, owner
//! pointers or subclass markers, and many entity types do not exist yet. The
//! regular [`SectionWriter`](super::SectionWriter) always emits R13+
//! structures, so documents are written as R13 when they carry `AC1009`
//! (see `SectionWriter::writable_version`). This module is used instead when
//! R12 is requested explicitly with [`DxfWriter::with_version`](super::DxfWriter::with_version).
//!
//! Conversion rules:
//!
//! * R12 entities (POINT, LINE, CIRCLE, ARC, SOLID, TRACE, 3DFACE, TEXT,
//!   INSERT with ATTRIBs, ATTDEF, POLYLINE/VERTEX incl. polyface and polygon
//!   meshes, DIMENSION) are written natively.
//! * LWPOLYLINE becomes a 2D POLYLINE with bulges and widths.
//! * RAY and XLINE become long LINEs through the drawing extents.
//! * Every other entity is decomposed with [`EntityType::explode`] until only
//!   R12 entities remain (MTEXT -> TEXT, ELLIPSE/SPLINE -> lines,
//!   HATCH -> lines, MULTILEADER -> lines/text, ...). Entities that cannot be
//!   decomposed (3D solids, images, OLE, ...) are left out.
//! * Table, block and linetype names are mapped to valid R12 names
//!   (letters, digits, `$`, `-`, `_`, at most 31 characters).

use std::collections::{HashMap, HashSet};

use super::{DxfStreamWriter, DxfStreamWriterExt};
use crate::document::CadDocument;
use crate::entities::explode::{nurbs_points, spline_points};
use crate::entities::*;
use crate::error::Result;
use crate::types::{Color, Handle, Vector3};

/// Upper limit of pattern segments per hatch; denser patterns fall back to the boundary.
const MAX_HATCH_SEGMENTS: usize = 200_000;

/// Nesting limit for `explode` chains.
const MAX_EXPLODE_DEPTH: usize = 8;

/// Writes `document` as an ASCII/text DXF R12 file.
pub(super) fn write_r12<W: DxfStreamWriter>(writer: &mut W, document: &CadDocument) -> Result<()> {
    let mut r12 = R12Writer::new(writer, document);
    r12.write_header()?;
    r12.write_tables()?;
    r12.write_blocks()?;
    r12.write_entities()?;
    r12.writer.write_string(0, "EOF")?;
    Ok(())
}

// ----------------------------------------------------------------------------- names

/// Maps names to valid, unique R12 symbol names (case-insensitive like ACAD).
#[derive(Default)]
struct NameMap {
    map: HashMap<String, String>,
    used: HashSet<String>,
}

impl NameMap {
    fn sanitize(name: &str) -> String {
        let mut s: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '$' | '-' | '_') {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        if s.is_empty() {
            s.push('_');
        }
        s.truncate(31);
        s
    }

    /// Registers `name` and returns its R12 form.
    fn add(&mut self, name: &str) -> String {
        let key = name.to_ascii_uppercase();
        if let Some(v) = self.map.get(&key) {
            return v.clone();
        }
        let base = Self::sanitize(name);
        let mut candidate = base.clone();
        let mut n = 1;
        while self.used.contains(&candidate.to_ascii_uppercase()) {
            let suffix = format!("_{n}");
            let keep = 31usize.saturating_sub(suffix.len()).min(base.len());
            candidate = format!("{}{suffix}", &base[..keep]);
            n += 1;
        }
        self.used.insert(candidate.to_ascii_uppercase());
        self.map.insert(key, candidate.clone());
        candidate
    }

    fn get(&self, name: &str) -> Option<&str> {
        self.map.get(&name.to_ascii_uppercase()).map(String::as_str)
    }
}

// ----------------------------------------------------------------------------- writer

struct R12Writer<'a, W: DxfStreamWriter> {
    writer: &'a mut W,
    doc: &'a CadDocument,
    layers: NameMap,
    linetypes: NameMap,
    styles: NameMap,
    blocks: NameMap,
    dimstyles: NameMap,
    /// half length of the LINEs that replace RAY/XLINE
    reach: f64,
    /// anonymous `*X` blocks holding the hatch geometry, by hatch handle
    hatch_blocks: HashMap<Handle, String>,
}

fn is_model_space(name: &str) -> bool {
    name.eq_ignore_ascii_case("*Model_Space")
}

fn is_paper_space(name: &str) -> bool {
    name.len() >= 12 && name[..12].eq_ignore_ascii_case("*Paper_Space")
}

fn degrees(radians: f64) -> f64 {
    super::degrees(radians)
}

impl<'a, W: DxfStreamWriter> R12Writer<'a, W> {
    fn new(writer: &'a mut W, doc: &'a CadDocument) -> Self {
        let mut layers = NameMap::default();
        layers.add("0");
        for layer in doc.layers.iter() {
            layers.add(&layer.name);
        }
        let mut linetypes = NameMap::default();
        linetypes.add("CONTINUOUS");
        for lt in doc.line_types.iter() {
            if !Self::is_pseudo_linetype(&lt.name) {
                linetypes.add(&lt.name);
            }
        }
        let mut styles = NameMap::default();
        styles.add("STANDARD");
        for st in doc.text_styles.iter() {
            if !st.name.is_empty() {
                styles.add(&st.name);
            }
        }
        let mut blocks = NameMap::default();
        for br in doc.block_records.iter() {
            if !is_model_space(&br.name) && !is_paper_space(&br.name) {
                // anonymous names keep their '*' prefix (*D1, *U2, *X3, ...)
                if let Some(rest) = br.name.strip_prefix('*') {
                    let mapped = format!("*{}", NameMap::sanitize(rest));
                    blocks
                        .map
                        .insert(br.name.to_ascii_uppercase(), mapped.clone());
                    blocks.used.insert(mapped.to_ascii_uppercase());
                } else {
                    blocks.add(&br.name);
                }
            }
        }
        let mut dimstyles = NameMap::default();
        dimstyles.add("STANDARD");
        for ds in doc.dim_styles.iter() {
            dimstyles.add(&ds.name);
        }
        let h = &doc.header;
        let (min, max) = (h.model_space_extents_min, h.model_space_extents_max);
        let reach = if max.x >= min.x && max.y >= min.y {
            let diag =
                ((max.x - min.x).powi(2) + (max.y - min.y).powi(2) + (max.z - min.z).powi(2))
                    .sqrt();
            let far = [min.x.abs(), min.y.abs(), max.x.abs(), max.y.abs()]
                .into_iter()
                .fold(0.0, f64::max);
            (diag * 2.0 + far).max(1000.0)
        } else {
            1.0e6
        };
        Self {
            writer,
            doc,
            layers,
            linetypes,
            styles,
            blocks,
            dimstyles,
            reach,
            hatch_blocks: HashMap::new(),
        }
    }

    /// BYLAYER/BYBLOCK are entity keywords in R12, not LTYPE records.
    fn is_pseudo_linetype(name: &str) -> bool {
        name.eq_ignore_ascii_case("ByLayer") || name.eq_ignore_ascii_case("ByBlock")
    }

    // ------------------------------------------------------------------- header

    fn write_header(&mut self) -> Result<()> {
        let h = &self.doc.header;
        let w = &mut *self.writer;
        w.write_section_start("HEADER")?;
        w.write_string(9, "$ACADVER")?;
        w.write_string(1, "AC1009")?;
        if !h.code_page.is_empty() {
            w.write_string(9, "$DWGCODEPAGE")?;
            w.write_string(3, &h.code_page)?;
        }
        w.write_string(9, "$INSBASE")?;
        w.write_point3d(10, h.model_space_insertion_base)?;
        w.write_string(9, "$EXTMIN")?;
        w.write_point3d(10, h.model_space_extents_min)?;
        w.write_string(9, "$EXTMAX")?;
        w.write_point3d(10, h.model_space_extents_max)?;
        w.write_string(9, "$LIMMIN")?;
        w.write_point2d(10, h.model_space_limits_min)?;
        w.write_string(9, "$LIMMAX")?;
        w.write_point2d(10, h.model_space_limits_max)?;
        w.write_string(9, "$ORTHOMODE")?;
        w.write_i16(70, h.ortho_mode as i16)?;
        w.write_string(9, "$FILLMODE")?;
        w.write_i16(70, h.fill_mode as i16)?;
        w.write_string(9, "$LTSCALE")?;
        w.write_double(
            40,
            if h.linetype_scale > 0.0 {
                h.linetype_scale
            } else {
                1.0
            },
        )?;
        w.write_string(9, "$TEXTSIZE")?;
        w.write_double(
            40,
            if h.text_height > 0.0 {
                h.text_height
            } else {
                2.5
            },
        )?;
        w.write_string(9, "$CLAYER")?;
        let layer = self
            .layers
            .get(&h.current_layer_name)
            .unwrap_or("0")
            .to_string();
        w.write_string(8, &layer)?;
        w.write_string(9, "$TEXTSTYLE")?;
        let style = self
            .styles
            .get(&h.current_text_style_name)
            .unwrap_or("STANDARD")
            .to_string();
        w.write_string(7, &style)?;
        w.write_string(9, "$DIMSTYLE")?;
        let dimstyle = self
            .dimstyles
            .get(&h.current_dimstyle_name)
            .unwrap_or("STANDARD")
            .to_string();
        w.write_string(2, &dimstyle)?;
        w.write_string(9, "$DIMSCALE")?;
        w.write_double(40, if h.dim_scale > 0.0 { h.dim_scale } else { 1.0 })?;
        w.write_string(9, "$LUNITS")?;
        w.write_i16(70, h.linear_unit_format.clamp(1, 5))?;
        w.write_string(9, "$LUPREC")?;
        w.write_i16(70, h.linear_unit_precision.clamp(0, 8))?;
        w.write_string(9, "$AUNITS")?;
        w.write_i16(70, h.angular_unit_format.clamp(0, 4))?;
        w.write_string(9, "$AUPREC")?;
        w.write_i16(70, h.angular_unit_precision.clamp(0, 8))?;
        w.write_string(9, "$ANGBASE")?;
        w.write_double(50, degrees(h.angle_base))?;
        w.write_string(9, "$ANGDIR")?;
        w.write_i16(70, h.angle_direction)?;
        w.write_string(9, "$PDMODE")?;
        w.write_i16(70, h.point_display_mode)?;
        w.write_string(9, "$PDSIZE")?;
        w.write_double(40, h.point_display_size)?;
        // no entity handles are written
        w.write_string(9, "$HANDLING")?;
        w.write_i16(70, 0)?;
        w.write_section_end()?;
        Ok(())
    }

    // ------------------------------------------------------------------- tables

    fn table_start(&mut self, name: &str, count: usize) -> Result<()> {
        self.writer.write_string(0, "TABLE")?;
        self.writer.write_string(2, name)?;
        self.writer
            .write_i16(70, count.min(i16::MAX as usize) as i16)?;
        Ok(())
    }

    fn table_end(&mut self) -> Result<()> {
        self.writer.write_string(0, "ENDTAB")
    }

    fn write_tables(&mut self) -> Result<()> {
        self.writer.write_section_start("TABLES")?;
        self.write_vport_table()?;
        self.write_ltype_table()?;
        self.write_layer_table()?;
        self.write_style_table()?;
        self.table_start("VIEW", 0)?;
        self.table_end()?;
        self.table_start("UCS", 0)?;
        self.table_end()?;
        self.write_appid_table()?;
        self.write_dimstyle_table()?;
        self.writer.write_section_end()?;
        Ok(())
    }

    fn write_vport_table(&mut self) -> Result<()> {
        let active = self
            .doc
            .vports
            .iter()
            .find(|v| v.name.eq_ignore_ascii_case("*Active"));
        self.table_start("VPORT", 1)?;
        let w = &mut *self.writer;
        w.write_string(0, "VPORT")?;
        w.write_string(2, "*ACTIVE")?;
        w.write_i16(70, 0)?;
        match active {
            Some(v) => {
                w.write_point2d(10, v.lower_left)?;
                w.write_point2d(11, v.upper_right)?;
                w.write_point2d(12, v.view_center)?;
                w.write_point2d(13, v.snap_base)?;
                w.write_point2d(14, v.snap_spacing)?;
                w.write_point2d(15, v.grid_spacing)?;
                w.write_point3d(16, v.view_direction)?;
                w.write_point3d(17, v.view_target)?;
                w.write_double(
                    40,
                    if v.view_height > 0.0 {
                        v.view_height
                    } else {
                        1.0
                    },
                )?;
                w.write_double(
                    41,
                    if v.aspect_ratio > 0.0 {
                        v.aspect_ratio
                    } else {
                        1.0
                    },
                )?;
                w.write_double(
                    42,
                    if v.lens_length > 0.0 {
                        v.lens_length
                    } else {
                        50.0
                    },
                )?;
                w.write_double(43, v.front_clip)?;
                w.write_double(44, v.back_clip)?;
                w.write_double(50, degrees(v.snap_rotation))?;
                w.write_double(51, degrees(v.view_twist))?;
                w.write_i16(71, 0)?;
                w.write_i16(72, v.circle_zoom.max(1))?;
                w.write_i16(73, v.fast_zoom as i16)?;
                w.write_i16(74, 3)?;
                w.write_i16(75, v.snap_on as i16)?;
                w.write_i16(76, v.grid_on as i16)?;
                w.write_i16(77, v.snap_style as i16)?;
                w.write_i16(78, v.snap_isopair)?;
            }
            None => {
                let h = &self.doc.header;
                let (min, max) = (h.model_space_extents_min, h.model_space_extents_max);
                let (cx, cy, height) = if max.x > min.x && max.y > min.y {
                    (
                        (min.x + max.x) * 0.5,
                        (min.y + max.y) * 0.5,
                        (max.y - min.y) * 1.1,
                    )
                } else {
                    (0.0, 0.0, 100.0)
                };
                w.write_double(10, 0.0)?;
                w.write_double(20, 0.0)?;
                w.write_double(11, 1.0)?;
                w.write_double(21, 1.0)?;
                w.write_double(12, cx)?;
                w.write_double(22, cy)?;
                w.write_double(16, 0.0)?;
                w.write_double(26, 0.0)?;
                w.write_double(36, 1.0)?;
                w.write_double(40, height)?;
                w.write_double(41, 1.5)?;
                w.write_double(42, 50.0)?;
                w.write_i16(71, 0)?;
                w.write_i16(72, 100)?;
                w.write_i16(73, 1)?;
                w.write_i16(74, 3)?;
            }
        }
        self.table_end()
    }

    fn write_ltype_table(&mut self) -> Result<()> {
        let records: Vec<&crate::tables::LineType> = self
            .doc
            .line_types
            .iter()
            .filter(|lt| !Self::is_pseudo_linetype(&lt.name))
            .collect();
        let has_continuous = records
            .iter()
            .any(|lt| lt.name.eq_ignore_ascii_case("CONTINUOUS"));
        self.table_start("LTYPE", records.len() + usize::from(!has_continuous))?;
        if !has_continuous {
            let w = &mut *self.writer;
            w.write_string(0, "LTYPE")?;
            w.write_string(2, "CONTINUOUS")?;
            w.write_i16(70, 0)?;
            w.write_string(3, "Solid line")?;
            w.write_i16(72, 65)?;
            w.write_i16(73, 0)?;
            w.write_double(40, 0.0)?;
        }
        for lt in records {
            let name = self
                .linetypes
                .get(&lt.name)
                .unwrap_or("CONTINUOUS")
                .to_string();
            let w = &mut *self.writer;
            w.write_string(0, "LTYPE")?;
            w.write_string(2, &name)?;
            w.write_i16(70, 0)?;
            w.write_string(3, &lt.description)?;
            w.write_i16(72, 65)?;
            // complex elements (text/shape) did not exist in R12; their dash length is kept
            w.write_i16(73, lt.elements.len().min(12) as i16)?;
            let total: f64 = lt.elements.iter().take(12).map(|e| e.length.abs()).sum();
            w.write_double(
                40,
                if lt.pattern_length > 0.0 {
                    lt.pattern_length
                } else {
                    total
                },
            )?;
            for e in lt.elements.iter().take(12) {
                w.write_double(49, e.length)?;
            }
        }
        self.table_end()
    }

    fn write_layer_table(&mut self) -> Result<()> {
        let has_zero = self.doc.layers.iter().any(|l| l.name == "0");
        let count = self.doc.layers.iter().count() + usize::from(!has_zero);
        self.table_start("LAYER", count)?;
        if !has_zero {
            let w = &mut *self.writer;
            w.write_string(0, "LAYER")?;
            w.write_string(2, "0")?;
            w.write_i16(70, 0)?;
            w.write_i16(62, 7)?;
            w.write_string(6, "CONTINUOUS")?;
        }
        for layer in self.doc.layers.iter() {
            let name = self.layers.get(&layer.name).unwrap_or("0").to_string();
            let linetype =
                if layer.line_type.is_empty() || Self::is_pseudo_linetype(&layer.line_type) {
                    "CONTINUOUS".to_string()
                } else {
                    self.linetypes
                        .get(&layer.line_type)
                        .unwrap_or("CONTINUOUS")
                        .to_string()
                };
            let mut flags = 0i16;
            if layer.flags.frozen {
                flags |= 1;
            }
            if layer.flags.locked {
                flags |= 4;
            }
            let index = match layer.color {
                Color::Index(i) if i > 0 => i as i16,
                Color::Rgb { .. } => layer.color.approximate_index().clamp(1, 255),
                _ => 7,
            };
            let w = &mut *self.writer;
            w.write_string(0, "LAYER")?;
            w.write_string(2, &name)?;
            w.write_i16(70, flags)?;
            // a negative color index switches the layer off
            w.write_i16(62, if layer.flags.off { -index } else { index })?;
            w.write_string(6, &linetype)?;
        }
        self.table_end()
    }

    fn write_style_table(&mut self) -> Result<()> {
        let records: Vec<&crate::tables::TextStyle> = self
            .doc
            .text_styles
            .iter()
            .filter(|s| !s.name.is_empty())
            .collect();
        let has_standard = records
            .iter()
            .any(|s| s.name.eq_ignore_ascii_case("STANDARD"));
        self.table_start("STYLE", records.len() + usize::from(!has_standard))?;
        if !has_standard {
            let w = &mut *self.writer;
            w.write_string(0, "STYLE")?;
            w.write_string(2, "STANDARD")?;
            w.write_i16(70, 0)?;
            w.write_double(40, 0.0)?;
            w.write_double(41, 1.0)?;
            w.write_double(50, 0.0)?;
            w.write_i16(71, 0)?;
            w.write_double(42, 2.5)?;
            w.write_string(3, "txt")?;
            w.write_string(4, "")?;
        }
        for st in records {
            let name = self.styles.get(&st.name).unwrap_or("STANDARD").to_string();
            let mut generation = 0i16;
            if st.flags.backward {
                generation |= 2;
            }
            if st.flags.upside_down {
                generation |= 4;
            }
            // R12 only knows SHX fonts; TrueType names are kept as the font file
            let font = if !st.font_file.is_empty() {
                st.font_file.clone()
            } else if !st.true_type_font.is_empty() {
                st.true_type_font.clone()
            } else {
                "txt".to_string()
            };
            let w = &mut *self.writer;
            w.write_string(0, "STYLE")?;
            w.write_string(2, &name)?;
            w.write_i16(
                70,
                if st.is_shape_file { 1 } else { 0 } | if st.is_vertical { 4 } else { 0 },
            )?;
            w.write_double(40, st.height)?;
            w.write_double(
                41,
                if st.width_factor > 0.0 {
                    st.width_factor
                } else {
                    1.0
                },
            )?;
            w.write_double(50, degrees(st.oblique_angle))?;
            w.write_i16(71, generation)?;
            w.write_double(
                42,
                if st.last_height > 0.0 {
                    st.last_height
                } else {
                    2.5
                },
            )?;
            w.write_string(3, &font)?;
            w.write_string(4, &st.big_font_file)?;
        }
        self.table_end()
    }

    fn write_appid_table(&mut self) -> Result<()> {
        // XDATA is not written, so only the mandatory ACAD record is needed
        self.table_start("APPID", 1)?;
        let w = &mut *self.writer;
        w.write_string(0, "APPID")?;
        w.write_string(2, "ACAD")?;
        w.write_i16(70, 0)?;
        self.table_end()
    }

    fn write_dimstyle_table(&mut self) -> Result<()> {
        let records: Vec<&crate::tables::DimStyle> = self.doc.dim_styles.iter().collect();
        let has_standard = records
            .iter()
            .any(|d| d.name.eq_ignore_ascii_case("STANDARD"));
        self.table_start("DIMSTYLE", records.len() + usize::from(!has_standard))?;
        if !has_standard {
            let w = &mut *self.writer;
            w.write_string(0, "DIMSTYLE")?;
            w.write_string(2, "STANDARD")?;
            w.write_i16(70, 0)?;
        }
        for ds in records {
            let name = self
                .dimstyles
                .get(&ds.name)
                .unwrap_or("STANDARD")
                .to_string();
            let w = &mut *self.writer;
            w.write_string(0, "DIMSTYLE")?;
            w.write_string(2, &name)?;
            w.write_i16(70, 0)?;
            w.write_string(3, &ds.dimpost)?;
            w.write_string(4, &ds.dimapost)?;
            w.write_double(40, if ds.dimscale > 0.0 { ds.dimscale } else { 1.0 })?;
            w.write_double(41, ds.dimasz)?;
            w.write_double(42, ds.dimexo)?;
            w.write_double(43, ds.dimdli)?;
            w.write_double(44, ds.dimexe)?;
            w.write_double(45, ds.dimrnd)?;
            w.write_double(46, ds.dimdle)?;
            w.write_double(47, ds.dimtp)?;
            w.write_double(48, ds.dimtm)?;
            w.write_double(140, ds.dimtxt)?;
            w.write_double(141, ds.dimcen)?;
            w.write_double(142, ds.dimtsz)?;
            w.write_double(143, ds.dimaltf)?;
            w.write_double(144, if ds.dimlfac != 0.0 { ds.dimlfac } else { 1.0 })?;
            w.write_double(145, ds.dimtvp)?;
            w.write_double(146, if ds.dimtfac > 0.0 { ds.dimtfac } else { 1.0 })?;
            w.write_double(147, ds.dimgap)?;
            w.write_i16(71, ds.dimtol as i16)?;
            w.write_i16(72, ds.dimlim as i16)?;
            w.write_i16(73, ds.dimtih as i16)?;
            w.write_i16(74, ds.dimtoh as i16)?;
            w.write_i16(75, ds.dimse1 as i16)?;
            w.write_i16(76, ds.dimse2 as i16)?;
            w.write_i16(77, ds.dimtad)?;
            w.write_i16(78, ds.dimzin)?;
            w.write_i16(170, ds.dimalt as i16)?;
            w.write_i16(171, ds.dimaltd)?;
            w.write_i16(172, ds.dimtofl as i16)?;
            w.write_i16(173, ds.dimsah as i16)?;
            w.write_i16(174, ds.dimtix as i16)?;
            w.write_i16(175, ds.dimsoxd as i16)?;
            w.write_i16(176, ds.dimclrd)?;
            w.write_i16(177, ds.dimclre)?;
            w.write_i16(178, ds.dimclrt)?;
        }
        self.table_end()
    }

    // ------------------------------------------------------------------- blocks

    fn write_blocks(&mut self) -> Result<()> {
        self.writer.write_section_start("BLOCKS")?;
        // ACAD R12 writes the two layout blocks, empty
        for name in ["$MODEL_SPACE", "$PAPER_SPACE"] {
            self.block_start(name, 0, Vector3::ZERO)?;
            self.block_end()?;
        }
        // hatch blocks first, so hatches inside block definitions can use them too
        self.write_hatch_blocks()?;
        for br in self.doc.block_records.iter() {
            if is_model_space(&br.name)
                || is_paper_space(&br.name)
                || br.flags.is_xref
                || br.flags.is_xref_overlay
            {
                continue;
            }
            let name = self.blocks.get(&br.name).unwrap_or(&br.name).to_string();
            let entities: Vec<&EntityType> = br
                .entity_handles
                .iter()
                .filter_map(|h| self.doc.get_entity(*h))
                .collect();
            let base = match self.doc.get_entity(br.block_entity_handle) {
                Some(EntityType::Block(b)) => b.base_point,
                _ => br.base_point,
            };
            let mut flags = 0i16;
            if br.name.starts_with('*') {
                flags |= 1;
            }
            if entities
                .iter()
                .any(|e| matches!(e, EntityType::AttributeDefinition(_)))
            {
                flags |= 2;
            }
            self.block_start(&name, flags, base)?;
            for e in entities {
                self.write_entity(e, false, 0)?;
            }
            self.block_end()?;
        }
        self.writer.write_section_end()?;
        Ok(())
    }

    /// R12 has no HATCH entity: like ACAD R12, every hatch becomes an
    /// anonymous `*X` block with its pattern lines (or SOLID strips for fills),
    /// inserted where the hatch was.
    fn write_hatch_blocks(&mut self) -> Result<()> {
        let doc = self.doc;
        let mut hatches: Vec<&Hatch> = Vec::new();
        for br in doc.block_records.iter() {
            if br.flags.is_xref || br.flags.is_xref_overlay {
                continue;
            }
            for h in &br.entity_handles {
                if let Some(EntityType::Hatch(hatch)) = doc.get_entity(*h) {
                    if !hatch.common.handle.is_null() {
                        hatches.push(hatch);
                    }
                }
            }
        }
        let mut n = 0usize;
        for hatch in hatches {
            let parts = hatch_entities(hatch);
            if parts.is_empty() {
                continue;
            }
            let name = loop {
                n += 1;
                let candidate = format!("*X{n}");
                if !self.blocks.used.contains(&candidate.to_ascii_uppercase()) {
                    break candidate;
                }
            };
            self.blocks.used.insert(name.to_ascii_uppercase());
            self.block_start(&name, 1, Vector3::ZERO)?;
            for part in &parts {
                self.write_entity(part, false, 0)?;
            }
            self.block_end()?;
            self.hatch_blocks.insert(hatch.common.handle, name);
        }
        Ok(())
    }

    fn block_start(&mut self, name: &str, flags: i16, base: Vector3) -> Result<()> {
        let w = &mut *self.writer;
        w.write_string(0, "BLOCK")?;
        w.write_string(8, "0")?;
        w.write_string(2, name)?;
        w.write_i16(70, flags)?;
        w.write_point3d(10, base)?;
        w.write_string(3, name)?;
        Ok(())
    }

    fn block_end(&mut self) -> Result<()> {
        self.writer.write_string(0, "ENDBLK")?;
        self.writer.write_string(8, "0")
    }

    // ------------------------------------------------------------------- entities

    fn write_entities(&mut self) -> Result<()> {
        self.writer.write_section_start("ENTITIES")?;
        let doc = self.doc;
        for (space, paper) in [("*Model_Space", false), ("*Paper_Space", true)] {
            if let Some(br) = doc.block_records.get(space) {
                for h in &br.entity_handles {
                    if let Some(e) = doc.get_entity(*h) {
                        // R12 paper space has no VIEWPORT records of the R13 kind
                        if paper && matches!(e, EntityType::Viewport(_)) {
                            continue;
                        }
                        self.write_entity(e, paper, 0)?;
                    }
                }
            }
        }
        self.writer.write_section_end()?;
        Ok(())
    }

    fn layer(&self, name: &str) -> String {
        if name.is_empty() {
            return "0".into();
        }
        self.layers
            .get(name)
            .map(str::to_string)
            .unwrap_or_else(|| NameMap::sanitize(name))
    }

    fn common(&mut self, c: &EntityCommon, paper: bool) -> Result<()> {
        let layer = self.layer(&c.layer);
        self.writer.write_string(8, &layer)?;
        if c.has_linetype() {
            if c.linetype.eq_ignore_ascii_case("ByBlock") {
                self.writer.write_string(6, "BYBLOCK")?;
            } else if let Some(lt) = self.linetypes.get(&c.linetype).map(str::to_string) {
                self.writer.write_string(6, &lt)?;
            }
        }
        if !matches!(c.color, Color::ByLayer | Color::None) {
            self.writer.write_color(62, c.color)?;
        }
        if paper {
            self.writer.write_i16(67, 1)?;
        }
        Ok(())
    }

    fn thickness(&mut self, t: f64) -> Result<()> {
        if t != 0.0 {
            self.writer.write_double(39, t)?;
        }
        Ok(())
    }

    fn normal(&mut self, n: Vector3) -> Result<()> {
        if n.x.abs() > 1e-12 || n.y.abs() > 1e-12 || (n.z - 1.0).abs() > 1e-12 {
            self.writer.write_double(210, n.x)?;
            self.writer.write_double(220, n.y)?;
            self.writer.write_double(230, n.z)?;
        }
        Ok(())
    }

    fn style(&self, name: &str) -> String {
        self.styles.get(name).unwrap_or("STANDARD").to_string()
    }

    fn write_entity(&mut self, e: &EntityType, paper: bool, depth: usize) -> Result<()> {
        match e {
            EntityType::Point(p) => {
                self.writer.write_entity_type("POINT")?;
                self.common(&p.common, paper)?;
                self.writer.write_point3d(10, p.location)?;
                self.thickness(p.thickness)?;
                self.normal(p.normal)?;
                if p.x_axis_angle != 0.0 {
                    self.writer.write_double(50, degrees(p.x_axis_angle))?;
                }
            }
            EntityType::Line(l) => {
                self.writer.write_entity_type("LINE")?;
                self.common(&l.common, paper)?;
                self.writer.write_point3d(10, l.start)?;
                self.writer.write_point3d(11, l.end)?;
                self.thickness(l.thickness)?;
                self.normal(l.normal)?;
            }
            EntityType::Circle(c) => {
                self.writer.write_entity_type("CIRCLE")?;
                self.common(&c.common, paper)?;
                self.writer.write_point3d(10, c.center)?;
                self.writer.write_double(40, c.radius)?;
                self.thickness(c.thickness)?;
                self.normal(c.normal)?;
            }
            EntityType::Arc(a) => {
                self.writer.write_entity_type("ARC")?;
                self.common(&a.common, paper)?;
                self.writer.write_point3d(10, a.center)?;
                self.writer.write_double(40, a.radius)?;
                self.writer.write_double(50, degrees(a.start_angle))?;
                self.writer.write_double(51, degrees(a.end_angle))?;
                self.thickness(a.thickness)?;
                self.normal(a.normal)?;
            }
            EntityType::Solid(s) => {
                self.writer
                    .write_entity_type(if s.is_trace { "TRACE" } else { "SOLID" })?;
                self.common(&s.common, paper)?;
                self.writer.write_point3d(10, s.first_corner)?;
                self.writer.write_point3d(11, s.second_corner)?;
                self.writer.write_point3d(12, s.third_corner)?;
                self.writer.write_point3d(13, s.fourth_corner)?;
                self.thickness(s.thickness)?;
                self.normal(s.normal)?;
            }
            EntityType::Face3D(f) => {
                self.writer.write_entity_type("3DFACE")?;
                self.common(&f.common, paper)?;
                self.writer.write_point3d(10, f.first_corner)?;
                self.writer.write_point3d(11, f.second_corner)?;
                self.writer.write_point3d(12, f.third_corner)?;
                self.writer.write_point3d(13, f.fourth_corner)?;
                let bits = f.invisible_edges.bits();
                if bits != 0 {
                    self.writer.write_i16(70, bits as i16)?;
                }
            }
            EntityType::Text(t) => self.write_text(t, paper)?,
            EntityType::Insert(i) => self.write_insert(i, paper)?,
            EntityType::AttributeDefinition(a) => self.write_attdef(a, paper)?,
            EntityType::AttributeEntity(a) => self.write_attrib(a, paper)?,
            EntityType::LwPolyline(p) => self.write_lwpolyline(p, paper)?,
            EntityType::Polyline2D(p) => self.write_polyline2d(p, paper)?,
            EntityType::Polyline(p) => self.write_polyline3d_old(p, paper)?,
            EntityType::Polyline3D(p) => self.write_polyline3d(p, paper)?,
            EntityType::PolyfaceMesh(m) => self.write_polyface(m, paper)?,
            EntityType::PolygonMesh(m) => self.write_polygon_mesh(m, paper)?,
            EntityType::Dimension(d) => {
                if self.write_dimension(d, paper)? {
                    return Ok(());
                }
                // kinds R12 does not have: their graphics block, else the decomposition
                let block = d.base().block_name.clone();
                if !block.is_empty() && self.blocks.get(&block).is_some() {
                    let mut insert = Insert::new(block, Vector3::ZERO);
                    insert.common = d.base().common.clone();
                    return self.write_insert(&insert, paper);
                }
                return self.write_exploded(e, paper, depth);
            }
            EntityType::Hatch(h) => {
                if let Some(name) = self.hatch_blocks.get(&h.common.handle).cloned() {
                    self.writer.write_entity_type("INSERT")?;
                    self.common(&h.common, paper)?;
                    self.writer.write_string(2, &name)?;
                    self.writer.write_point3d(10, Vector3::ZERO)?;
                } else {
                    for part in hatch_entities(h) {
                        self.write_entity(&part, paper, depth + 1)?;
                    }
                }
            }
            EntityType::Spline(s) => {
                let points = spline_points(s);
                self.write_point_polyline(&s.common, paper, &points, false, s.normal)?;
            }
            EntityType::Helix(h) => {
                let points = spline_points(&h.spline);
                self.write_point_polyline(
                    &h.spline.common,
                    paper,
                    &points,
                    false,
                    Vector3::new(1.0, 0.0, 0.0),
                )?;
            }
            EntityType::Ellipse(el) => {
                let points = ellipse_points(el);
                let closed =
                    points.len() > 2 && (points[0] - points[points.len() - 1]).length() < 1e-9;
                let points = if closed {
                    &points[..points.len() - 1]
                } else {
                    &points[..]
                };
                self.write_point_polyline(&el.common, paper, points, closed, el.normal)?;
            }
            EntityType::Ray(r) => {
                let line = Line {
                    common: r.common.clone(),
                    start: r.base_point,
                    end: r.base_point + r.direction.normalize() * self.reach,
                    ..Line::new()
                };
                self.write_entity(&EntityType::Line(line), paper, depth)?;
            }
            EntityType::XLine(x) => {
                let d = x.direction.normalize() * self.reach;
                let line = Line {
                    common: x.common.clone(),
                    start: x.base_point - d,
                    end: x.base_point + d,
                    ..Line::new()
                };
                self.write_entity(&EntityType::Line(line), paper, depth)?;
            }
            // structural records are produced by this writer itself
            EntityType::Block(_) | EntityType::BlockEnd(_) | EntityType::Seqend(_) => {}
            _ => return self.write_exploded(e, paper, depth),
        }
        Ok(())
    }

    /// Writes the R12 decomposition of an entity that R12 does not know.
    fn write_exploded(&mut self, e: &EntityType, paper: bool, depth: usize) -> Result<()> {
        if depth >= MAX_EXPLODE_DEPTH {
            return Ok(());
        }
        for part in e.flatten() {
            self.write_entity(&part, paper, depth + 1)?;
        }
        Ok(())
    }

    fn write_text(&mut self, t: &Text, paper: bool) -> Result<()> {
        self.writer.write_entity_type("TEXT")?;
        self.common(&t.common, paper)?;
        self.writer.write_point3d(10, t.insertion_point)?;
        self.writer.write_double(40, t.height)?;
        self.writer.write_string(1, &t.value)?;
        if t.rotation != 0.0 {
            self.writer.write_double(50, degrees(t.rotation))?;
        }
        if t.width_factor != 1.0 && t.width_factor > 0.0 {
            self.writer.write_double(41, t.width_factor)?;
        }
        if t.oblique_angle != 0.0 {
            self.writer.write_double(51, degrees(t.oblique_angle))?;
        }
        let style = self.style(&t.style);
        self.writer.write_string(7, &style)?;
        if t.generation_flags != 0 {
            self.writer.write_i16(71, t.generation_flags)?;
        }
        let h = t.horizontal_alignment as i16;
        let v = t.vertical_alignment as i16;
        if h != 0 {
            self.writer.write_i16(72, h)?;
        }
        if h != 0 || v != 0 {
            self.writer
                .write_point3d(11, t.alignment_point.unwrap_or(t.insertion_point))?;
        }
        self.thickness(t.thickness)?;
        self.normal(t.normal)?;
        if v != 0 {
            self.writer.write_i16(73, v)?;
        }
        Ok(())
    }

    fn write_insert(&mut self, i: &Insert, paper: bool) -> Result<()> {
        let Some(name) = self.blocks.get(&i.block_name).map(str::to_string) else {
            // reference to a block that is not written (xref, missing)
            return Ok(());
        };
        self.writer.write_entity_type("INSERT")?;
        self.common(&i.common, paper)?;
        if !i.attributes.is_empty() {
            self.writer.write_i16(66, 1)?;
        }
        self.writer.write_string(2, &name)?;
        self.writer.write_point3d(10, i.insert_point)?;
        for (code, s) in [(41, i.x_scale()), (42, i.y_scale()), (43, i.z_scale())] {
            if s != 1.0 {
                self.writer.write_double(code, s)?;
            }
        }
        if i.rotation != 0.0 {
            self.writer.write_double(50, degrees(i.rotation))?;
        }
        if i.column_count > 1 {
            self.writer.write_i16(70, i.column_count as i16)?;
            self.writer.write_double(44, i.column_spacing)?;
        }
        if i.row_count > 1 {
            self.writer.write_i16(71, i.row_count as i16)?;
            self.writer.write_double(45, i.row_spacing)?;
        }
        self.normal(i.normal)?;
        if !i.attributes.is_empty() {
            for a in &i.attributes {
                self.write_attrib(a, paper)?;
            }
            self.seqend(&i.common, paper)?;
        }
        Ok(())
    }

    fn attribute_flags(f: &AttributeFlags) -> i16 {
        (f.invisible as i16)
            | (f.constant as i16) << 1
            | (f.verify as i16) << 2
            | (f.preset as i16) << 3
    }

    #[allow(clippy::too_many_arguments)]
    fn write_attribute_text(
        &mut self,
        insertion: Vector3,
        alignment: Vector3,
        height: f64,
        rotation: f64,
        width: f64,
        oblique: f64,
        style: &str,
        generation: i16,
        h: i16,
        v: i16,
    ) -> Result<()> {
        if rotation != 0.0 {
            self.writer.write_double(50, degrees(rotation))?;
        }
        if width != 1.0 && width > 0.0 {
            self.writer.write_double(41, width)?;
        }
        if oblique != 0.0 {
            self.writer.write_double(51, degrees(oblique))?;
        }
        let style = self.style(style);
        self.writer.write_string(7, &style)?;
        if generation != 0 {
            self.writer.write_i16(71, generation)?;
        }
        if h != 0 {
            self.writer.write_i16(72, h)?;
        }
        if h != 0 || v != 0 {
            self.writer.write_point3d(
                11,
                if alignment == Vector3::ZERO {
                    insertion
                } else {
                    alignment
                },
            )?;
        }
        if v != 0 {
            // ATTRIB/ATTDEF use 74 for the vertical alignment
            self.writer.write_i16(74, v)?;
        }
        let _ = height;
        Ok(())
    }

    fn write_attdef(&mut self, a: &AttributeDefinition, paper: bool) -> Result<()> {
        self.writer.write_entity_type("ATTDEF")?;
        self.common(&a.common, paper)?;
        self.thickness(a.thickness)?;
        self.writer.write_point3d(10, a.insertion_point)?;
        self.writer.write_double(40, a.height)?;
        self.writer.write_string(1, &a.default_value)?;
        self.writer.write_string(3, &a.prompt)?;
        self.writer.write_string(2, &NameMap::sanitize(&a.tag))?;
        self.writer.write_i16(70, Self::attribute_flags(&a.flags))?;
        if a.field_length != 0 {
            self.writer.write_i16(73, a.field_length)?;
        }
        self.write_attribute_text(
            a.insertion_point,
            a.alignment_point,
            a.height,
            a.rotation,
            a.width_factor,
            a.oblique_angle,
            &a.text_style,
            a.text_generation_flags,
            a.horizontal_alignment as i16,
            a.vertical_alignment as i16,
        )?;
        self.normal(a.normal)?;
        Ok(())
    }

    fn write_attrib(&mut self, a: &AttributeEntity, paper: bool) -> Result<()> {
        self.writer.write_entity_type("ATTRIB")?;
        self.common(&a.common, paper)?;
        self.thickness(a.thickness)?;
        self.writer.write_point3d(10, a.insertion_point)?;
        self.writer.write_double(40, a.height)?;
        self.writer.write_string(1, &a.value)?;
        self.writer.write_string(2, &NameMap::sanitize(&a.tag))?;
        self.writer.write_i16(70, Self::attribute_flags(&a.flags))?;
        if a.field_length != 0 {
            self.writer.write_i16(73, a.field_length)?;
        }
        self.write_attribute_text(
            a.insertion_point,
            a.alignment_point,
            a.height,
            a.rotation,
            a.width_factor,
            a.oblique_angle,
            &a.text_style,
            a.text_generation_flags,
            a.horizontal_alignment as i16,
            a.vertical_alignment as i16,
        )?;
        self.normal(a.normal)?;
        Ok(())
    }

    fn seqend(&mut self, c: &EntityCommon, paper: bool) -> Result<()> {
        self.writer.write_entity_type("SEQEND")?;
        let layer = self.layer(&c.layer);
        self.writer.write_string(8, &layer)?;
        if paper {
            self.writer.write_i16(67, 1)?;
        }
        Ok(())
    }

    /// POLYLINE header: entity data, "vertices follow" flag and the elevation point.
    fn polyline_start(
        &mut self,
        c: &EntityCommon,
        paper: bool,
        flags: i16,
        elevation: f64,
    ) -> Result<()> {
        self.writer.write_entity_type("POLYLINE")?;
        self.common(c, paper)?;
        self.writer.write_i16(66, 1)?;
        self.writer
            .write_point3d(10, Vector3::new(0.0, 0.0, elevation))?;
        self.writer.write_i16(70, flags)?;
        Ok(())
    }

    fn vertex_start(&mut self, c: &EntityCommon, paper: bool, location: Vector3) -> Result<()> {
        self.writer.write_entity_type("VERTEX")?;
        let layer = self.layer(&c.layer);
        self.writer.write_string(8, &layer)?;
        if paper {
            self.writer.write_i16(67, 1)?;
        }
        self.writer.write_point3d(10, location)?;
        Ok(())
    }

    /// Polyline through WCS points: 2D when the points lie in the plane of an
    /// unrotated OCS (`normal` = Z), otherwise a 3D polyline.
    fn write_point_polyline(
        &mut self,
        common: &EntityCommon,
        paper: bool,
        points: &[Vector3],
        closed: bool,
        normal: Vector3,
    ) -> Result<()> {
        if points.len() < 2 {
            return Ok(());
        }
        let flat_z = (normal - Vector3::new(0.0, 0.0, 1.0)).length() < 1e-9
            && points.iter().all(|p| (p.z - points[0].z).abs() < 1e-9);
        let flags = if closed { 1 } else { 0 } | if flat_z { 0 } else { 8 };
        self.polyline_start(common, paper, flags, if flat_z { points[0].z } else { 0.0 })?;
        for p in points {
            self.vertex_start(common, paper, *p)?;
            self.writer.write_i16(70, if flat_z { 0 } else { 32 })?;
        }
        self.seqend(common, paper)
    }

    fn write_lwpolyline(&mut self, p: &LwPolyline, paper: bool) -> Result<()> {
        let mut flags = 0i16;
        if p.is_closed {
            flags |= 1;
        }
        if p.plinegen {
            flags |= 128;
        }
        self.polyline_start(&p.common, paper, flags, p.elevation)?;
        if p.constant_width != 0.0 {
            self.writer.write_double(40, p.constant_width)?;
            self.writer.write_double(41, p.constant_width)?;
        }
        self.thickness(p.thickness)?;
        self.normal(p.normal)?;
        for v in &p.vertices {
            self.vertex_start(
                &p.common,
                paper,
                Vector3::new(v.location.x, v.location.y, p.elevation),
            )?;
            if v.start_width != 0.0 || v.end_width != 0.0 {
                self.writer.write_double(40, v.start_width)?;
                self.writer.write_double(41, v.end_width)?;
            }
            if v.bulge != 0.0 {
                self.writer.write_double(42, v.bulge)?;
            }
            self.writer.write_i16(70, 0)?;
        }
        self.seqend(&p.common, paper)
    }

    fn write_polyline2d(&mut self, p: &Polyline2D, paper: bool) -> Result<()> {
        // only the 2D flags; mesh/3D bits belong to the other polyline kinds
        let flags = (p.flags.bits() & (1 | 2 | 4 | 128)) as i16;
        self.polyline_start(&p.common, paper, flags, p.elevation)?;
        if p.start_width != 0.0 || p.end_width != 0.0 {
            self.writer.write_double(40, p.start_width)?;
            self.writer.write_double(41, p.end_width)?;
        }
        self.thickness(p.thickness)?;
        self.normal(p.normal)?;
        for v in &p.vertices {
            self.vertex_start(&p.common, paper, v.location)?;
            if v.start_width != 0.0 || v.end_width != 0.0 {
                self.writer.write_double(40, v.start_width)?;
                self.writer.write_double(41, v.end_width)?;
            }
            if v.bulge != 0.0 {
                self.writer.write_double(42, v.bulge)?;
            }
            let vflags = (v.flags.bits() & (1 | 2 | 8 | 16)) as i16;
            self.writer.write_i16(70, vflags)?;
            if vflags & 2 != 0 {
                self.writer.write_double(50, degrees(v.curve_tangent))?;
            }
        }
        self.seqend(&p.common, paper)
    }

    fn write_polyline3d_old(&mut self, p: &Polyline, paper: bool) -> Result<()> {
        let flags = ((p.flags.bits() & (1 | 4 | 128)) | 8) as i16;
        self.polyline_start(&p.common, paper, flags, 0.0)?;
        for v in &p.vertices {
            self.vertex_start(&p.common, paper, v.location)?;
            self.writer
                .write_i16(70, ((v.flags.bits() & (1 | 8 | 16)) | 32) as i16)?;
        }
        self.seqend(&p.common, paper)
    }

    fn write_polyline3d(&mut self, p: &Polyline3D, paper: bool) -> Result<()> {
        let f = &p.flags;
        let mut flags = 8i16;
        if f.closed {
            flags |= 1;
        }
        if f.spline_fit {
            flags |= 4;
        }
        if f.linetype_continuous {
            flags |= 128;
        }
        self.polyline_start(&p.common, paper, flags, 0.0)?;
        for v in &p.vertices {
            self.vertex_start(&p.common, paper, v.position)?;
            self.writer
                .write_i16(70, ((v.flags as i16) & (1 | 8 | 16)) | 32)?;
        }
        self.seqend(&p.common, paper)
    }

    fn write_polyface(&mut self, m: &PolyfaceMesh, paper: bool) -> Result<()> {
        self.polyline_start(&m.common, paper, 64, m.elevation)?;
        self.writer
            .write_i16(71, m.vertices.len().min(i16::MAX as usize) as i16)?;
        self.writer
            .write_i16(72, m.faces.len().min(i16::MAX as usize) as i16)?;
        self.normal(m.normal)?;
        for v in &m.vertices {
            self.vertex_start(&m.common, paper, v.location)?;
            self.writer.write_i16(70, 128 | 64)?;
        }
        for f in &m.faces {
            self.vertex_start(&m.common, paper, Vector3::ZERO)?;
            if let Some(c) = f.color {
                if !matches!(c, Color::ByLayer | Color::None) {
                    self.writer.write_color(62, c)?;
                }
            }
            self.writer.write_i16(70, 128)?;
            for (code, index) in [
                (71, f.index1),
                (72, f.index2),
                (73, f.index3),
                (74, f.index4),
            ] {
                if index != 0 {
                    self.writer.write_i16(code, index)?;
                }
            }
        }
        self.seqend(&m.common, paper)
    }

    fn write_polygon_mesh(&mut self, m: &PolygonMeshEntity, paper: bool) -> Result<()> {
        let flags = (m.flags.bits() & (1 | 4 | 32)) | 16;
        self.polyline_start(&m.common, paper, flags, m.elevation)?;
        self.writer.write_i16(71, m.m_vertex_count)?;
        self.writer.write_i16(72, m.n_vertex_count)?;
        if m.m_smooth_density != 0 || m.n_smooth_density != 0 {
            self.writer.write_i16(73, m.m_smooth_density)?;
            self.writer.write_i16(74, m.n_smooth_density)?;
        }
        self.normal(m.normal)?;
        for v in &m.vertices {
            self.vertex_start(&m.common, paper, v.location)?;
            self.writer.write_i16(70, 64)?;
        }
        self.seqend(&m.common, paper)
    }

    /// Native R12 DIMENSION; returns false for kinds R12 does not have.
    fn write_dimension(&mut self, d: &Dimension, paper: bool) -> Result<bool> {
        let (definition, type_flags): (Vector3, i16) = match d {
            Dimension::Linear(x) => (x.definition_point, 0),
            Dimension::Aligned(x) => (x.definition_point, 1),
            Dimension::Angular2Ln(x) => (x.definition_point, 2),
            Dimension::Diameter(x) => (x.definition_point, 3),
            Dimension::Radius(x) => (x.angle_vertex, 4),
            Dimension::Angular3Pt(x) => (x.definition_point, 5),
            Dimension::Ordinate(x) => (
                x.definition_point,
                6 | if x.is_ordinate_type_x { 0x40 } else { 0 },
            ),
            Dimension::Arc(_) | Dimension::LargeRadial(_) => return Ok(false),
        };
        let base = d.base();
        let block = self
            .blocks
            .get(&base.block_name)
            .unwrap_or(&base.block_name)
            .to_string();
        self.writer.write_entity_type("DIMENSION")?;
        self.common(&base.common, paper)?;
        self.writer.write_string(2, &block)?;
        self.writer.write_point3d(10, definition)?;
        self.writer.write_point3d(11, base.text_middle_point)?;
        if base.insertion_point != Vector3::ZERO {
            self.writer.write_point3d(12, base.insertion_point)?;
        }
        let user_positioned = if base.text_user_positioned { 0x80 } else { 0 };
        self.writer.write_i16(70, type_flags | user_positioned)?;
        if let Some(text) = base.text_override() {
            self.writer.write_string(1, text)?;
        }
        let style = self
            .dimstyles
            .get(&base.style_name)
            .unwrap_or("STANDARD")
            .to_string();
        self.writer.write_string(3, &style)?;
        match d {
            Dimension::Linear(x) => {
                self.writer.write_point3d(13, x.first_point)?;
                self.writer.write_point3d(14, x.second_point)?;
                self.writer.write_double(50, degrees(x.rotation))?;
                if x.ext_line_rotation != 0.0 {
                    self.writer.write_double(52, degrees(x.ext_line_rotation))?;
                }
            }
            Dimension::Aligned(x) => {
                self.writer.write_point3d(13, x.first_point)?;
                self.writer.write_point3d(14, x.second_point)?;
            }
            Dimension::Angular2Ln(x) => {
                self.writer.write_point3d(13, x.first_point)?;
                self.writer.write_point3d(14, x.second_point)?;
                self.writer.write_point3d(15, x.angle_vertex)?;
                self.writer.write_point3d(16, x.dimension_arc)?;
            }
            Dimension::Diameter(x) => {
                self.writer.write_point3d(15, x.angle_vertex)?;
                self.writer.write_double(40, x.leader_length)?;
            }
            Dimension::Radius(x) => {
                self.writer.write_point3d(15, x.definition_point)?;
                self.writer.write_double(40, x.leader_length)?;
            }
            Dimension::Angular3Pt(x) => {
                self.writer.write_point3d(13, x.first_point)?;
                self.writer.write_point3d(14, x.second_point)?;
                self.writer.write_point3d(15, x.angle_vertex)?;
            }
            Dimension::Ordinate(x) => {
                self.writer.write_point3d(13, x.feature_location)?;
                self.writer.write_point3d(14, x.leader_endpoint)?;
            }
            Dimension::Arc(_) | Dimension::LargeRadial(_) => {}
        }
        if base.text_rotation != 0.0 {
            self.writer.write_double(53, degrees(base.text_rotation))?;
        }
        if base.horizontal_direction.abs() > 1e-12 {
            self.writer
                .write_double(51, degrees(base.horizontal_direction))?;
        }
        self.normal(base.normal)?;
        Ok(true)
    }
}

// ----------------------------------------------------------------------------- geometry

/// Points of an ellipse or elliptical arc (WCS).
fn ellipse_points(el: &Ellipse) -> Vec<Vector3> {
    let major = el.major_axis;
    let n = el.normal.normalize();
    let minor = n.cross(&major) * el.minor_axis_ratio;
    let (t0, mut t1) = (el.start_parameter, el.end_parameter);
    while t1 <= t0 {
        t1 += std::f64::consts::TAU;
    }
    let steps = (((t1 - t0) / std::f64::consts::TAU) * 96.0)
        .ceil()
        .clamp(8.0, 512.0) as usize;
    (0..=steps)
        .map(|i| {
            let t = t0 + (t1 - t0) * i as f64 / steps as f64;
            el.center + major * t.cos() + minor * t.sin()
        })
        .collect()
}

/// ACAD arbitrary axis algorithm: OCS point -> WCS.
fn ocs_to_wcs(normal: Vector3, p: Vector3) -> Vector3 {
    let n = normal.normalize();
    if (n - Vector3::new(0.0, 0.0, 1.0)).length() < 1e-12 {
        return p;
    }
    let ax = if n.x.abs() < 1.0 / 64.0 && n.y.abs() < 1.0 / 64.0 {
        Vector3::new(0.0, 1.0, 0.0).cross(&n).normalize()
    } else {
        Vector3::new(0.0, 0.0, 1.0).cross(&n).normalize()
    };
    let ay = n.cross(&ax).normalize();
    ax * p.x + ay * p.y + n * p.z
}

/// Arc points from `a0` to `a1` (radians, counter-clockwise).
fn arc_xy(cx: f64, cy: f64, r: f64, a0: f64, mut a1: f64) -> Vec<(f64, f64)> {
    while a1 <= a0 {
        a1 += std::f64::consts::TAU;
    }
    let steps = (((a1 - a0) / std::f64::consts::TAU) * 96.0)
        .ceil()
        .clamp(2.0, 512.0) as usize;
    (0..=steps)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f64 / steps as f64;
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

/// Boundary loops of a hatch as closed OCS polygons.
fn hatch_loops(hatch: &Hatch) -> Vec<Vec<(f64, f64)>> {
    let mut loops = Vec::new();
    for path in &hatch.paths {
        let mut pts: Vec<(f64, f64)> = Vec::new();
        let push = |pts: &mut Vec<(f64, f64)>, p: (f64, f64)| {
            if pts
                .last()
                .is_none_or(|q| (q.0 - p.0).abs() > 1e-12 || (q.1 - p.1).abs() > 1e-12)
            {
                pts.push(p);
            }
        };
        for edge in &path.edges {
            match edge {
                BoundaryEdge::Line(l) => {
                    push(&mut pts, (l.start.x, l.start.y));
                    push(&mut pts, (l.end.x, l.end.y));
                }
                BoundaryEdge::CircularArc(a) => {
                    let arc = if a.counter_clockwise {
                        arc_xy(a.center.x, a.center.y, a.radius, a.start_angle, a.end_angle)
                    } else {
                        // clockwise edges store mirrored angles
                        let mut v =
                            arc_xy(a.center.x, a.center.y, a.radius, a.start_angle, a.end_angle);
                        for p in &mut v {
                            p.1 = 2.0 * a.center.y - p.1;
                        }
                        v
                    };
                    for p in arc {
                        push(&mut pts, p);
                    }
                }
                BoundaryEdge::EllipticArc(e) => {
                    let (mx, my) = (e.major_axis_endpoint.x, e.major_axis_endpoint.y);
                    let (nx, ny) = (-my * e.minor_axis_ratio, mx * e.minor_axis_ratio);
                    let (t0, mut t1) = (e.start_angle, e.end_angle);
                    // clockwise edges store mirrored parameters
                    let sign = if e.counter_clockwise { 1.0 } else { -1.0 };
                    while t1 <= t0 {
                        t1 += std::f64::consts::TAU;
                    }
                    let steps = (((t1 - t0) / std::f64::consts::TAU) * 96.0)
                        .ceil()
                        .clamp(2.0, 512.0) as usize;
                    for i in 0..=steps {
                        let t = sign * (t0 + (t1 - t0) * i as f64 / steps as f64);
                        push(
                            &mut pts,
                            (
                                e.center.x + mx * t.cos() + nx * t.sin(),
                                e.center.y + my * t.cos() + ny * t.sin(),
                            ),
                        );
                    }
                }
                BoundaryEdge::Spline(s) => {
                    let ctrl: Vec<Vector3> = s
                        .control_points
                        .iter()
                        .map(|c| Vector3::new(c.x, c.y, 0.0))
                        .collect();
                    let weights: Vec<f64> = if s.rational {
                        s.control_points.iter().map(|c| c.z).collect()
                    } else {
                        Vec::new()
                    };
                    let curve = if ctrl.len() >= 2 {
                        nurbs_points(
                            usize::try_from(s.degree).unwrap_or(3).max(1),
                            &ctrl,
                            &weights,
                            &s.knots,
                            16,
                        )
                    } else {
                        s.fit_points
                            .iter()
                            .map(|f| Vector3::new(f.x, f.y, 0.0))
                            .collect()
                    };
                    for p in curve {
                        push(&mut pts, (p.x, p.y));
                    }
                }
                BoundaryEdge::Polyline(pl) => {
                    let v = &pl.vertices;
                    let count = if pl.is_closed {
                        v.len()
                    } else {
                        v.len().saturating_sub(1)
                    };
                    for i in 0..v.len() {
                        push(&mut pts, (v[i].x, v[i].y));
                        if i < count {
                            let (a, b) = (v[i], v[(i + 1) % v.len()]);
                            let bulge = a.z;
                            if bulge.abs() > 1e-12 {
                                for p in bulge_points((a.x, a.y), (b.x, b.y), bulge) {
                                    push(&mut pts, p);
                                }
                            }
                        }
                    }
                }
            }
        }
        if pts.len() > 1 && pts[0] == pts[pts.len() - 1] {
            pts.pop();
        }
        if pts.len() >= 3 {
            loops.push(pts);
        }
    }
    loops
}

/// Intermediate points of a bulged segment (start and end excluded).
fn bulge_points(a: (f64, f64), b: (f64, f64), bulge: f64) -> Vec<(f64, f64)> {
    let theta = 4.0 * bulge.atan();
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let chord = (dx * dx + dy * dy).sqrt();
    if chord < 1e-12 {
        return Vec::new();
    }
    let r = chord / (2.0 * (theta / 2.0).sin());
    let h = r * (theta / 2.0).cos();
    let (mx, my) = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let (cx, cy) = (mx - h * dy / chord, my + h * dx / chord);
    let a0 = (a.1 - cy).atan2(a.0 - cx);
    let steps = ((theta.abs() / std::f64::consts::TAU) * 96.0)
        .ceil()
        .clamp(2.0, 256.0) as usize;
    (1..steps)
        .map(|i| {
            let ang = a0 + theta * i as f64 / steps as f64;
            (cx + r.abs() * ang.cos(), cy + r.abs() * ang.sin())
        })
        .collect()
}

/// Hatch geometry as R12 entities: SOLID strips for fills, LINEs/POINTs of the
/// pattern clipped to the boundary (even-odd), or the boundary itself when the
/// pattern is empty or too dense.
fn hatch_entities(hatch: &Hatch) -> Vec<EntityType> {
    let loops = hatch_loops(hatch);
    if loops.is_empty() {
        return Vec::new();
    }
    let z = hatch.elevation;
    let normal = hatch.normal;
    let common = EntityCommon {
        handle: Handle::NULL,
        ..hatch.common.clone()
    };
    let edges: Vec<((f64, f64), (f64, f64))> = loops
        .iter()
        .flat_map(|l| (0..l.len()).map(move |i| (l[i], l[(i + 1) % l.len()])))
        .collect();
    let mut out = Vec::new();

    let solid = hatch.is_solid
        || hatch.pattern.lines.is_empty() && hatch.pattern.name.eq_ignore_ascii_case("SOLID");
    if solid {
        // trapezoid strips between the distinct vertex heights
        let mut ys: Vec<f64> = edges.iter().map(|e| e.0 .1).collect();
        ys.sort_by(|a, b| a.total_cmp(b));
        ys.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
        for band in ys.windows(2) {
            let (y0, y1) = (band[0], band[1]);
            let ym = (y0 + y1) / 2.0;
            let mut xs: Vec<(f64, f64, f64)> = edges
                .iter()
                .filter(|(a, b)| (a.1 <= ym) != (b.1 <= ym))
                .map(|(a, b)| {
                    let at = |y: f64| a.0 + (b.0 - a.0) * (y - a.1) / (b.1 - a.1);
                    (at(ym), at(y0), at(y1))
                })
                .collect();
            xs.sort_by(|a, b| a.0.total_cmp(&b.0));
            for pair in xs.chunks_exact(2) {
                let (l, r) = (pair[0], pair[1]);
                out.push(EntityType::Solid(Solid {
                    common: common.clone(),
                    // SOLID corner order: 1-2 bottom, 3-4 top (3 below 1, 4 below 2 in drawing order)
                    first_corner: Vector3::new(l.1, y0, z),
                    second_corner: Vector3::new(r.1, y0, z),
                    third_corner: Vector3::new(l.2, y1, z),
                    fourth_corner: Vector3::new(r.2, y1, z),
                    normal,
                    thickness: 0.0,
                    is_trace: false,
                }));
            }
        }
        return out;
    }

    let line = |a: (f64, f64), b: (f64, f64)| {
        EntityType::Line(Line {
            common: common.clone(),
            start: ocs_to_wcs(normal, Vector3::new(a.0, a.1, z)),
            end: ocs_to_wcs(normal, Vector3::new(b.0, b.1, z)),
            ..Line::new()
        })
    };
    let mut segments = 0usize;
    'pattern: for pl in &hatch.pattern.lines {
        let (dx, dy) = (pl.angle.cos(), pl.angle.sin());
        let (ox, oy) = (pl.offset.x, pl.offset.y);
        // distance between neighbouring lines of the family
        let spacing = dx * oy - dy * ox;
        if spacing.abs() < 1e-9 {
            continue;
        }
        let (bx, by) = (pl.base_point.x, pl.base_point.y);
        let perp = |p: (f64, f64)| (p.1 - by) * dx - (p.0 - bx) * dy;
        let (lo, hi) = edges.iter().fold((f64::MAX, f64::MIN), |(lo, hi), e| {
            (lo.min(perp(e.0)), hi.max(perp(e.0)))
        });
        let (k0, k1) = (
            (lo / spacing).min(hi / spacing).floor() as i64,
            (lo / spacing).max(hi / spacing).ceil() as i64,
        );
        if (k1 - k0) as usize > MAX_HATCH_SEGMENTS {
            segments = usize::MAX;
            break;
        }
        let period: f64 = pl.dash_lengths.iter().map(|d| d.abs()).sum();
        for k in k0..=k1 {
            let (qx, qy) = (bx + ox * k as f64, by + oy * k as f64);
            // parameters along the line where it crosses the boundary
            let mut ts: Vec<f64> = edges
                .iter()
                .filter_map(|(a, b)| {
                    let (ex, ey) = (b.0 - a.0, b.1 - a.1);
                    let den = dx * ey - dy * ex;
                    if den.abs() < 1e-15 {
                        return None;
                    }
                    let (wx, wy) = (a.0 - qx, a.1 - qy);
                    let s = (wx * dy - wy * dx) / den;
                    if !(0.0..1.0).contains(&s) {
                        return None;
                    }
                    Some((wx * ey - wy * ex) / den)
                })
                .collect();
            ts.sort_by(|a, b| a.total_cmp(b));
            for span in ts.chunks_exact(2) {
                let (t0, t1) = (span[0], span[1]);
                let at = |t: f64| (qx + dx * t, qy + dy * t);
                if pl.dash_lengths.is_empty() || period <= 1e-12 {
                    out.push(line(at(t0), at(t1)));
                    segments += 1;
                } else {
                    // dash pattern measured from the line origin
                    let mut t = (t0 / period).floor() * period;
                    'dashes: loop {
                        for d in &pl.dash_lengths {
                            let (s, e) = (t, t + d.abs());
                            if s >= t1 {
                                break 'dashes;
                            }
                            if *d >= 0.0 && e > t0 {
                                if *d == 0.0 {
                                    let p = at(s);
                                    out.push(EntityType::Point(Point {
                                        common: common.clone(),
                                        location: ocs_to_wcs(normal, Vector3::new(p.0, p.1, z)),
                                        ..Point::new()
                                    }));
                                } else {
                                    out.push(line(at(s.max(t0)), at(e.min(t1))));
                                }
                                segments += 1;
                            }
                            t = e;
                        }
                        if segments > MAX_HATCH_SEGMENTS {
                            break 'pattern;
                        }
                    }
                }
            }
            if segments > MAX_HATCH_SEGMENTS {
                break 'pattern;
            }
        }
    }
    if segments > MAX_HATCH_SEGMENTS || out.is_empty() {
        // too dense (or no pattern): keep at least the boundary
        out.clear();
        for (a, b) in &edges {
            out.push(line(*a, *b));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::NameMap;

    #[test]
    fn names_are_valid_and_unique() {
        let mut m = NameMap::default();
        assert_eq!(m.add("Layer 1"), "Layer_1");
        assert_eq!(m.add("Layer_1"), "Layer_1_1");
        assert_eq!(m.add("layer 1"), "Layer_1");
        assert_eq!(m.add("Ä/Ö"), "___");
        let long = "x".repeat(40);
        assert_eq!(m.add(&long).len(), 31);
    }
}
