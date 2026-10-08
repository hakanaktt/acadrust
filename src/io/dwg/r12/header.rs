//! Fixed-layout AC1009 drawing and table headers.

use crate::document::HeaderVariables;
use crate::error::{DxfError, Result};
use crate::io::dwg::crc;
use crate::types::{Color, Vector2, Vector3};

pub const HEADER_VARIABLES_END: usize = 0x6bd;
pub const ENTITIES_START: u32 = 0x6cf;

pub const ENTITIES_BEGIN: [u8; 16] = [
    0xc4, 0x6e, 0x68, 0x54, 0xf8, 0x6e, 0x33, 0x30, 0x63, 0x3e, 0xc1, 0x85, 0x2a, 0xdc, 0x94, 0x01,
];
pub const ENTITIES_END: [u8; 16] = [
    0x3b, 0x91, 0x97, 0xab, 0x07, 0x91, 0xcc, 0xcf, 0x9c, 0xc1, 0x3e, 0x7a, 0xd5, 0x23, 0x6b, 0xfe,
];
pub const BLOCK_ENTITIES_BEGIN: [u8; 16] = [
    0x72, 0x2b, 0x7d, 0xec, 0x3e, 0x8c, 0x88, 0x6c, 0x7a, 0x72, 0x0a, 0xfd, 0xc8, 0x6c, 0x84, 0x26,
];
pub const BLOCK_ENTITIES_END: [u8; 16] = [
    0x8d, 0xd4, 0x82, 0x13, 0xc1, 0x73, 0x77, 0x93, 0x85, 0x8d, 0xf5, 0x02, 0x37, 0x93, 0x7b, 0xd9,
];
pub const EXTRA_ENTITIES_BEGIN: [u8; 16] = [
    0xd5, 0xf9, 0xd3, 0xbb, 0x0a, 0xa9, 0x69, 0xa6, 0xcd, 0x1c, 0x87, 0xc7, 0xee, 0x80, 0x4b, 0x17,
];
pub const EXTRA_ENTITIES_END: [u8; 16] = [
    0x2a, 0x06, 0x2c, 0x44, 0xf5, 0x56, 0x96, 0x59, 0x32, 0xe3, 0x78, 0x38, 0x11, 0x7f, 0xb4, 0xe8,
];
pub const AUXHEADER_BEGIN: [u8; 16] = [
    0x29, 0x8d, 0xd1, 0x49, 0xa9, 0x73, 0x1f, 0xea, 0x99, 0xde, 0x32, 0xf9, 0x4d, 0x0a, 0xe0, 0x19,
];
pub const AUXHEADER_END: [u8; 16] = [
    0xd6, 0x72, 0x2e, 0xb6, 0x56, 0x8c, 0xe0, 0x15, 0x66, 0x21, 0xcd, 0x06, 0xb2, 0xf5, 0x1f, 0xe6,
];

pub const TABLE_NAMES: [&str; 10] = [
    "BLOCK", "LAYER", "STYLE", "LTYPE", "VIEW", "UCS", "VPORT", "APPID", "DIMSTYLE", "VX",
];
const TABLE_IDS: [u16; 10] = [1, 2, 3, 5, 6, 7, 8, 9, 10, 11];
const TABLE_OFFSETS: [usize; 10] = [
    0x2c, 0x36, 0x40, 0x4a, 0x54, 0x3ef, 0x500, 0x512, 0x522, 0x69f,
];
pub const TABLE_BEGIN: [[u8; 16]; 10] = [
    [
        0xdb, 0xef, 0xb3, 0xf0, 0xc7, 0x3e, 0x6d, 0xa6, 0xc9, 0xb6, 0x24, 0x5c, 0x4c, 0x6f, 0x32,
        0xcb,
    ],
    [
        0x0e, 0xc4, 0x64, 0x6f, 0xbb, 0x1d, 0xd3, 0x8b, 0x00, 0x49, 0xc2, 0xef, 0x18, 0xea, 0x6f,
        0xfb,
    ],
    [
        0xe2, 0x3e, 0xc1, 0x82, 0x43, 0x9f, 0x61, 0x77, 0x50, 0xab, 0xc7, 0x66, 0x96, 0x00, 0x06,
        0x18,
    ],
    [
        0xac, 0x90, 0x1a, 0xca, 0x1c, 0xbd, 0x95, 0x15, 0x16, 0x16, 0x4c, 0x14, 0xce, 0x18, 0x88,
        0xaf,
    ],
    [
        0xc1, 0x3c, 0xaa, 0x56, 0x68, 0xf4, 0xb4, 0x1e, 0x4b, 0x74, 0xf4, 0x08, 0x42, 0x4d, 0xbf,
        0xa5,
    ],
    [
        0x60, 0x4a, 0xfa, 0x3d, 0x84, 0x90, 0xcc, 0x5b, 0xef, 0xe7, 0xd6, 0xa5, 0x7f, 0x1e, 0x61,
        0xcd,
    ],
    [
        0xf6, 0xed, 0x44, 0x61, 0x2a, 0xdc, 0xe4, 0x7b, 0x4e, 0xb9, 0x2b, 0xbb, 0x66, 0x60, 0x63,
        0x8d,
    ],
    [
        0xe1, 0x25, 0xc2, 0x50, 0x36, 0x68, 0x6c, 0x0c, 0x3b, 0xd3, 0x5d, 0x56, 0xc1, 0x79, 0x1c,
        0x3a,
    ],
    [
        0xb4, 0x18, 0x3e, 0x42, 0xc9, 0x9f, 0xff, 0xe5, 0xb6, 0xe2, 0xcb, 0xb3, 0x75, 0xc3, 0xc3,
        0xb0,
    ],
    [
        0xe0, 0xca, 0x36, 0x7c, 0xce, 0xe7, 0x58, 0x6f, 0x2b, 0x7d, 0x74, 0x55, 0x05, 0xf1, 0x44,
        0x7f,
    ],
];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TableLocator {
    pub record_size: u16,
    pub count: u16,
    pub flags: u16,
    pub address: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeaderLayout {
    pub entities_start: u32,
    pub entities_end: u32,
    pub blocks_start: u32,
    pub blocks_size: u32,
    pub extras_start: u32,
    pub extras_size: u32,
}

impl Default for HeaderLayout {
    fn default() -> Self {
        Self {
            entities_start: ENTITIES_START,
            entities_end: ENTITIES_START,
            blocks_start: 0,
            blocks_size: 0,
            extras_start: 0,
            extras_size: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HeaderReferences {
    pub layer_index: i16,
    pub text_style_index: i16,
    pub linetype_index: i16,
    pub ucs_index: i16,
    pub paper_ucs_index: i16,
}

#[derive(Debug, Clone)]
pub struct R12Header {
    pub variables: HeaderVariables,
    pub layout: HeaderLayout,
    pub tables: [TableLocator; 10],
    pub references: HeaderReferences,
    pub maintenance_version: u8,
    pub num_header_variables: u16,
    /// Retains header fields that have no counterpart in `HeaderVariables`.
    pub raw: Vec<u8>,
}

pub fn read_header(data: &[u8]) -> Result<R12Header> {
    if data.len() < 0x6cd || !matches!(data.get(..6), Some(b"AC1009" | b"AD1009")) {
        return Err(DxfError::InvalidFormat("Invalid AC1009 header".into()));
    }
    let num_header_variables = u16::from_le_bytes(data[0x11..0x13].try_into().unwrap());
    if !matches!(num_header_variables, 204 | 205) {
        return Err(DxfError::InvalidFormat(format!(
            "Unsupported AC1009 header variable count: {num_header_variables}"
        )));
    }
    let variables_end = if num_header_variables == 204 {
        0x6bb
    } else {
        HEADER_VARIABLES_END
    };
    let stored_crc = u16::from_le_bytes(data[variables_end..variables_end + 2].try_into().unwrap());
    if stored_crc != crc::crc16(crc::CRC16_SEED, &data[..variables_end]) {
        return Err(DxfError::InvalidFormat("AC1009 header CRC mismatch".into()));
    }
    if data.get(variables_end + 2..variables_end + 18) != Some(ENTITIES_BEGIN.as_slice()) {
        return Err(DxfError::InvalidFormat(
            "Missing AC1009 entity sentinel".into(),
        ));
    }
    let mut header = R12Header {
        variables: HeaderVariables::default(),
        layout: HeaderLayout::default(),
        tables: [TableLocator::default(); 10],
        references: HeaderReferences::default(),
        maintenance_version: data[0x0b],
        num_header_variables,
        raw: data[..variables_end].to_vec(),
    };
    let mut c = Codec::reading(data[..variables_end].to_vec());
    c.pos = 0x14;
    c.long(&mut header.layout.entities_start);
    c.long(&mut header.layout.entities_end);
    c.long(&mut header.layout.blocks_start);
    c.long(&mut header.layout.blocks_size);
    c.long(&mut header.layout.extras_start);
    c.long(&mut header.layout.extras_size);
    header.layout.blocks_size &= 0x3fffffff;
    header.layout.extras_size &= 0x3fffffff;
    for (table, offset) in header.tables.iter_mut().zip(TABLE_OFFSETS) {
        c.pos = offset;
        c.locator(table);
        let end = u64::from(table.address) + u64::from(table.record_size) * u64::from(table.count);
        if table.count != 0 && (table.record_size == 0 || end > data.len() as u64) {
            return Err(DxfError::InvalidFormat(
                "AC1009 table exceeds file bounds".into(),
            ));
        }
    }
    c.pos = 0x5e;
    variables(
        &mut c,
        &mut header.variables,
        &mut header.references,
        num_header_variables,
    )?;
    if header.layout.entities_start < (variables_end + 18) as u32
        || header.layout.entities_end < header.layout.entities_start
        || header.layout.entities_end as usize > data.len()
    {
        return Err(DxfError::InvalidFormat(
            "Invalid AC1009 entity bounds".into(),
        ));
    }
    Ok(header)
}

pub fn write_header(
    header: &HeaderVariables,
    layout: HeaderLayout,
    tables: &[TableLocator; 10],
    references: HeaderReferences,
    maintenance_version: u8,
    source_raw: Option<&[u8]>,
) -> Result<Vec<u8>> {
    let mut c = Codec::writing(source_raw);
    c.data[..6].copy_from_slice(b"AC1009");
    c.data[6..11].fill(0);
    c.data[0x0b] = maintenance_version;
    c.data[0x0c] = 1;
    c.pos = 0x0d;
    c.short(&mut 3);
    c.short(&mut 5);
    c.short(&mut 205);
    c.byte(&mut 0);
    c.long(&mut layout.entities_start.clone());
    c.long(&mut layout.entities_end.clone());
    c.long(&mut layout.blocks_start.clone());
    c.long(&mut (layout.blocks_size | 0x40000000));
    c.long(&mut layout.extras_start.clone());
    c.long(&mut (layout.extras_size | 0x80000000));
    for (table, offset) in tables.iter().zip(TABLE_OFFSETS) {
        c.pos = offset;
        let mut table = *table;
        c.locator(&mut table);
    }
    c.pos = 0x5e;
    variables(&mut c, &mut header.clone(), &mut references.clone(), 205)?;
    let checksum = crc::crc16(crc::CRC16_SEED, &c.data);
    c.data.extend_from_slice(&checksum.to_le_bytes());
    c.data.extend_from_slice(&ENTITIES_BEGIN);
    Ok(c.data)
}

pub fn write_auxheader(
    layout: HeaderLayout,
    tables: &[TableLocator; 10],
    handle_seed: u64,
    address: u32,
) -> Vec<u8> {
    let mut data = Vec::with_capacity(170);
    data.extend_from_slice(&AUXHEADER_BEGIN);
    data.extend_from_slice(&16u16.to_le_bytes());
    data.extend_from_slice(&138u16.to_le_bytes());
    for value in [
        layout.entities_start,
        layout.entities_end,
        layout.blocks_start,
        layout.extras_start,
    ] {
        data.extend_from_slice(&value.to_le_bytes());
    }
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&handle_seed.to_be_bytes());
    data.extend_from_slice(&10u16.to_le_bytes());
    for (table, id) in tables.iter().zip(TABLE_IDS) {
        data.extend_from_slice(&id.to_le_bytes());
        data.extend_from_slice(&table.record_size.to_le_bytes());
        data.extend_from_slice(&table.count.to_le_bytes());
        data.extend_from_slice(&table.address.to_le_bytes());
    }
    data.extend_from_slice(&address.to_le_bytes());
    let checksum = crc::crc16(crc::CRC16_SEED, &data[16..]);
    data.extend_from_slice(&checksum.to_le_bytes());
    data.extend_from_slice(&AUXHEADER_END);
    data
}

struct Codec {
    data: Vec<u8>,
    pos: usize,
    read: bool,
    encoding: &'static encoding_rs::Encoding,
}

impl Codec {
    fn reading(data: Vec<u8>) -> Self {
        Self {
            data,
            pos: 0,
            read: true,
            encoding: encoding_rs::WINDOWS_1252,
        }
    }

    fn writing(raw: Option<&[u8]>) -> Self {
        let mut data = vec![0; HEADER_VARIABLES_END];
        if let Some(raw) = raw {
            let count = raw.len().min(data.len());
            data[..count].copy_from_slice(&raw[..count]);
        }
        Self {
            data,
            pos: 0,
            read: false,
            encoding: encoding_rs::WINDOWS_1252,
        }
    }

    fn bytes<const N: usize>(&mut self, value: &mut [u8; N]) {
        if self.read {
            value.copy_from_slice(&self.data[self.pos..self.pos + N]);
        } else {
            self.data[self.pos..self.pos + N].copy_from_slice(value);
        }
        self.pos += N;
    }

    fn byte(&mut self, value: &mut u8) {
        let mut bytes = [*value];
        self.bytes(&mut bytes);
        *value = bytes[0];
    }

    fn short(&mut self, value: &mut i16) {
        let mut bytes = value.to_le_bytes();
        self.bytes(&mut bytes);
        *value = i16::from_le_bytes(bytes);
    }

    fn long(&mut self, value: &mut u32) {
        let mut bytes = value.to_le_bytes();
        self.bytes(&mut bytes);
        *value = u32::from_le_bytes(bytes);
    }

    fn double(&mut self, value: &mut f64) {
        let mut bytes = value.to_le_bytes();
        self.bytes(&mut bytes);
        *value = f64::from_le_bytes(bytes);
    }

    fn flag(&mut self, value: &mut bool) {
        let mut number = i16::from(*value);
        self.short(&mut number);
        *value = number != 0;
    }

    fn byte_flag(&mut self, value: &mut bool) {
        let mut number = u8::from(*value);
        self.byte(&mut number);
        *value = number != 0;
    }

    fn byte_short(&mut self, value: &mut i16) {
        let mut number = *value as u8;
        self.byte(&mut number);
        *value = i16::from(number);
    }

    fn color(&mut self, value: &mut Color) {
        let mut number = value.approximate_index();
        self.short(&mut number);
        *value = Color::from_index(number);
    }

    fn point2(&mut self, value: &mut Vector2) {
        self.double(&mut value.x);
        self.double(&mut value.y);
    }

    fn point3(&mut self, value: &mut Vector3) {
        self.double(&mut value.x);
        self.double(&mut value.y);
        self.double(&mut value.z);
    }

    fn text(&mut self, value: &mut String, length: usize) {
        if self.read {
            let bytes = &self.data[self.pos..self.pos + length];
            let end = bytes.iter().position(|byte| *byte == 0).unwrap_or(length);
            *value = self.encoding.decode(&bytes[..end]).0.into_owned();
        } else {
            let bytes = self.encoding.encode(value).0;
            let count = bytes.len().min(length.saturating_sub(1));
            self.data[self.pos..self.pos + length].fill(0);
            self.data[self.pos..self.pos + count].copy_from_slice(&bytes[..count]);
        }
        self.pos += length;
    }

    fn timer(&mut self, value: &mut f64) {
        let mut day = value.floor() as u32;
        let mut milliseconds = ((value.fract() * 86_400_000.0).round().max(0.0)) as u32;
        self.long(&mut day);
        self.long(&mut milliseconds);
        *value = f64::from(day) + f64::from(milliseconds) / 86_400_000.0;
    }

    fn locator(&mut self, value: &mut TableLocator) {
        let mut size = value.record_size as i16;
        let mut count = value.count as i16;
        let mut flags = value.flags as i16;
        self.short(&mut size);
        self.short(&mut count);
        self.short(&mut flags);
        self.long(&mut value.address);
        value.record_size = size as u16;
        value.count = count as u16;
        value.flags = flags as u16;
    }

    fn skip(&mut self, length: usize) {
        self.pos += length;
    }
}

fn variables(
    c: &mut Codec,
    h: &mut HeaderVariables,
    r: &mut HeaderReferences,
    count: u16,
) -> Result<()> {
    c.point3(&mut h.model_space_insertion_base);
    c.flag(&mut h.polyline_linetype_generation);
    c.point3(&mut h.model_space_extents_min);
    c.point3(&mut h.model_space_extents_max);
    c.point2(&mut h.model_space_limits_min);
    c.point2(&mut h.model_space_limits_max);
    c.skip(24 + 8 + 2 + 16 + 16 + 8 + 2 + 2 + 2 + 16); // active view, snap and grid
    c.flag(&mut h.ortho_mode);
    c.flag(&mut h.regen_mode);
    c.flag(&mut h.fill_mode);
    c.flag(&mut h.quick_text_mode);
    c.short(&mut h.drag_mode);
    c.double(&mut h.linetype_scale);
    c.double(&mut h.text_height);
    c.double(&mut h.trace_width);
    c.short(&mut r.layer_index);
    c.skip(8 + 2);
    c.flag(&mut h.paper_space_linetype_scaling);
    c.short(&mut h.tree_depth);
    c.skip(2 + 8);
    c.short(&mut h.linear_unit_format);
    c.short(&mut h.linear_unit_precision);
    c.skip(2 + 16);
    c.double(&mut h.sketch_increment);
    c.double(&mut h.fillet_radius);
    c.short(&mut h.angular_unit_format);
    c.short(&mut h.angular_unit_precision);
    c.short(&mut r.text_style_index);
    let mut osmode = h.object_snap_mode as i16;
    c.short(&mut osmode);
    h.object_snap_mode = i32::from(osmode);
    c.short(&mut h.attribute_visibility);
    c.text(&mut h.menu_name, 15);
    c.double(&mut h.dim_scale);
    c.double(&mut h.dim_arrow_size);
    c.double(&mut h.dim_ext_line_offset);
    c.double(&mut h.dim_line_increment);
    c.double(&mut h.dim_ext_line_extension);
    c.double(&mut h.dim_tolerance_plus);
    c.double(&mut h.dim_tolerance_minus);
    c.double(&mut h.dim_text_height);
    c.double(&mut h.dim_center_mark);
    c.double(&mut h.dim_tick_size);
    c.byte_flag(&mut h.dim_tolerance);
    c.byte_flag(&mut h.dim_limits);
    c.byte_flag(&mut h.dim_text_inside_horizontal);
    c.byte_flag(&mut h.dim_text_outside_horizontal);
    c.byte_flag(&mut h.dim_suppress_ext1);
    c.byte_flag(&mut h.dim_suppress_ext2);
    c.byte_short(&mut h.dim_text_above);
    c.byte_flag(&mut h.limit_check);
    c.skip(46);
    c.double(&mut h.elevation);
    c.double(&mut h.thickness);
    c.skip(24 * 7 + 2);
    c.flag(&mut h.blip_mode);
    c.byte_short(&mut h.dim_zero_suppression);
    c.double(&mut h.dim_rounding);
    c.double(&mut h.dim_line_extension);
    c.text(&mut h.dim_arrow_block, 33);
    c.skip(2);
    c.short(&mut h.coords_mode);
    c.color(&mut h.current_entity_color);
    c.short(&mut r.linetype_index);
    c.timer(&mut h.create_date_julian);
    c.timer(&mut h.update_date_julian);
    c.timer(&mut h.total_editing_time);
    c.timer(&mut h.user_elapsed_time);
    c.flag(&mut h.user_timer);
    c.skip(2);
    let mut skpoly = h.sketch_type != 0;
    c.flag(&mut skpoly);
    h.sketch_type = i16::from(skpoly);
    c.skip(14);
    c.double(&mut h.angle_base);
    c.short(&mut h.angle_direction);
    c.short(&mut h.point_display_mode);
    c.double(&mut h.point_display_size);
    c.double(&mut h.polyline_width);
    c.short(&mut h.user_int1);
    c.short(&mut h.user_int2);
    c.short(&mut h.user_int3);
    c.short(&mut h.user_int4);
    c.short(&mut h.user_int5);
    c.double(&mut h.user_real1);
    c.double(&mut h.user_real2);
    c.double(&mut h.user_real3);
    c.double(&mut h.user_real4);
    c.double(&mut h.user_real5);
    c.byte_flag(&mut h.dim_alternate_units);
    c.byte_short(&mut h.dim_alt_decimal_places);
    c.byte_flag(&mut h.associate_dimensions);
    c.byte_flag(&mut h.update_dimensions_while_dragging);
    c.text(&mut h.dim_post, 16);
    c.text(&mut h.dim_alt_post, 16);
    c.double(&mut h.dim_alt_scale);
    c.double(&mut h.dim_linear_scale);
    c.short(&mut h.spline_segments);
    c.flag(&mut h.spline_frame);
    c.flag(&mut h.attribute_request);
    c.flag(&mut h.attribute_dialog);
    c.double(&mut h.chamfer_distance_a);
    c.double(&mut h.chamfer_distance_b);
    c.flag(&mut h.mirror_text);
    if c.pos != 0x3ef {
        return Err(DxfError::InvalidFormat(format!(
            "AC1009 header layout error at {:x}",
            c.pos
        )));
    }
    c.skip(10);
    let mut page = crate::io::dxf::code_page::dwg_code_page_index(&h.code_page) as i16;
    c.short(&mut page);
    h.code_page = crate::io::dxf::code_page::dwg_code_page_name(page as u16).to_owned();
    c.encoding = crate::io::dxf::code_page::encoding_from_dwg_code_page(page as u16);
    c.point3(&mut h.model_space_ucs_origin);
    c.point3(&mut h.model_space_ucs_x_axis);
    c.point3(&mut h.model_space_ucs_y_axis);
    c.skip(24);
    c.double(&mut h.lens_length);
    c.double(&mut h.view_twist);
    c.skip(8 + 8 + 2);
    c.byte_flag(&mut h.dim_force_line_inside);
    c.text(&mut h.dim_arrow_block1, 33);
    c.text(&mut h.dim_arrow_block2, 33);
    c.byte_flag(&mut h.dim_separate_arrows);
    c.byte_flag(&mut h.dim_force_text_inside);
    c.byte_flag(&mut h.dim_suppress_outside_ext);
    c.double(&mut h.dim_text_vertical_pos);
    c.skip(33);
    c.short(&mut 1);
    let mut seed = h.handle_seed.to_be_bytes();
    c.bytes(&mut seed);
    h.handle_seed = u64::from_be_bytes(seed);
    c.short(&mut h.surface_u_density);
    c.short(&mut h.surface_v_density);
    c.short(&mut h.surface_type);
    c.short(&mut h.surface_tab1);
    c.short(&mut h.surface_tab2);
    if c.pos != 0x500 {
        return Err(DxfError::InvalidFormat(format!(
            "AC1009 VPORT layout error at {:x}",
            c.pos
        )));
    }
    c.skip(10 + 2);
    c.short(&mut h.spline_type);
    c.skip(2);
    c.short(&mut r.ucs_index);
    c.skip(10);
    c.flag(&mut h.world_view);
    c.skip(4 + 10 + 5);
    c.color(&mut h.dim_line_color);
    c.color(&mut h.dim_ext_line_color);
    c.color(&mut h.dim_text_color);
    c.short(&mut h.shade_edge);
    c.short(&mut h.shade_diffuse);
    c.skip(2 + 2 + 32 + 128);
    c.double(&mut h.dim_tolerance_scale);
    c.point3(&mut h.paper_space_ucs_origin);
    c.point3(&mut h.paper_space_ucs_x_axis);
    c.point3(&mut h.paper_space_ucs_y_axis);
    c.short(&mut r.paper_ucs_index);
    c.flag(&mut h.show_model_space);
    c.flag(&mut h.paper_space_limit_check);
    c.skip(2);
    c.point3(&mut h.paper_space_extents_min);
    c.point3(&mut h.paper_space_extents_max);
    c.point2(&mut h.paper_space_limits_min);
    c.point2(&mut h.paper_space_limits_max);
    c.point3(&mut h.paper_space_insertion_base);
    if c.pos != 0x69f {
        return Err(DxfError::InvalidFormat(format!(
            "AC1009 VX layout error at {:x}",
            c.pos
        )));
    }
    c.skip(10);
    c.short(&mut h.max_active_viewports);
    c.double(&mut h.dim_line_gap);
    c.double(&mut h.paper_elevation);
    if count >= 205 {
        c.flag(&mut h.retain_xref_visibility);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_roundtrip_preserves_variables_and_locators() {
        let mut h = HeaderVariables::default();
        h.model_space_insertion_base = Vector3::new(12.0, 34.0, 56.0);
        h.linetype_scale = 2.5;
        h.user_int3 = -24;
        h.dim_arrow_block1 = "ARROW".into();
        h.handle_seed = 0x12345678;
        h.code_page = "ANSI_1252".into();
        let layout = HeaderLayout::default();
        let tables = [TableLocator::default(); 10];
        let refs = HeaderReferences {
            layer_index: 7,
            text_style_index: 3,
            linetype_index: -1,
            ucs_index: -1,
            paper_ucs_index: -1,
        };
        let encoded = write_header(&h, layout, &tables, refs, 0, None).unwrap();
        assert_eq!(encoded.len(), ENTITIES_START as usize);
        let decoded = read_header(&encoded).unwrap();
        assert_eq!(
            decoded.variables.model_space_insertion_base,
            h.model_space_insertion_base
        );
        assert_eq!(decoded.variables.linetype_scale, h.linetype_scale);
        assert_eq!(decoded.variables.user_int3, h.user_int3);
        assert_eq!(decoded.variables.dim_arrow_block1, h.dim_arrow_block1);
        assert_eq!(decoded.variables.handle_seed, h.handle_seed);
        assert_eq!(decoded.layout, layout);
        assert_eq!(decoded.references, refs);
        assert_eq!(decoded.tables, tables);
        let mut corrupt = encoded;
        corrupt[0x100] ^= 1;
        assert!(read_header(&corrupt).is_err());
    }

    #[test]
    fn auxheader_has_native_size_and_crc() {
        let encoded = write_auxheader(
            HeaderLayout::default(),
            &[TableLocator::default(); 10],
            42,
            2000,
        );
        assert_eq!(encoded.len(), 170);
        assert_eq!(&encoded[..16], &AUXHEADER_BEGIN);
        assert_eq!(&encoded[154..], &AUXHEADER_END);
        assert_eq!(
            u16::from_le_bytes(encoded[152..154].try_into().unwrap()),
            crc::crc16(crc::CRC16_SEED, &encoded[16..152])
        );
    }
}
