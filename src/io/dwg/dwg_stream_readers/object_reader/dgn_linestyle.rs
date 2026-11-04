use crate::io::dwg::dwg_stream_readers::bit_reader::DwgBitReader;
use crate::io::dwg::dwg_stream_readers::merged_reader::DwgMergedReader;
use crate::objects::{
    DgnLineStyleData, DgnLsComponentData, DgnLsComponentType, DgnLsCompoundComponent,
    DgnLsCompoundEntry, DgnLsInternalComponent, DgnLsPhaseMode, DgnLsPointComponent, DgnLsStroke,
    DgnLsStrokePattern, DgnLsSymbolComponent, DgnLsSymbolReference, ProxyPayload,
};
use crate::types::Handle;

pub fn read_dgn_line_style_data(
    reader: &mut DwgMergedReader,
    dxf_name: &str,
) -> Option<DgnLineStyleData> {
    let description = reader.read_variable_text();
    let version = reader.read_bit_long();
    let component_type = reader.read_bit_long();
    let component_uid = read_uid(reader);

    if dxf_name.eq_ignore_ascii_case("LSDEFINITION") {
        let (fields, undecoded_fields) = read_definition_fields(reader);
        return Some(DgnLineStyleData::Definition {
            description,
            version,
            style_number: component_type,
            component_uid,
            is_continuous: fields.is_continuous,
            unit_definition: fields.unit_definition,
            unit_scale: fields.unit_scale,
            units_type: fields.units_type,
            is_element: fields.is_element,
            is_physical: fields.is_physical,
            is_scale_independent: fields.is_scale_independent,
            is_snappable: fields.is_snappable,
            root_component: Handle::from(reader.read_handle()),
            undecoded_fields,
            properties: Vec::new(),
        });
    }

    let kind = DgnLsComponentType::from_code(component_type)?;
    let scale = reader.read_bit_double();
    let property_flags = read_byte(reader);
    let component = match kind {
        DgnLsComponentType::Symbol => DgnLsComponentData::Symbol(DgnLsSymbolComponent {
            stored_unit_scale: reader.read_bit_double(),
            unit_scale: reader.read_bit_double(),
            has_unit_scale: reader.read_bit(),
            is_3d: reader.read_bit(),
            block: Handle::from(reader.read_handle()),
        }),
        DgnLsComponentType::Compound => {
            let count = safe_count(reader.read_bit_long(), reader);
            let offsets = (0..count)
                .map(|_| reader.read_bit_double())
                .collect::<Vec<_>>();
            let entries = offsets
                .into_iter()
                .map(|offset| DgnLsCompoundEntry {
                    component: Handle::from(reader.read_handle()),
                    offset,
                })
                .collect();
            DgnLsComponentData::Compound(DgnLsCompoundComponent { entries })
        }
        DgnLsComponentType::Stroke => DgnLsComponentData::Stroke(read_stroke_pattern(reader)),
        DgnLsComponentType::Point => {
            let count = safe_count(reader.read_bit_long(), reader);
            let mut symbols = Vec::with_capacity(count as usize);
            for _ in 0..count {
                symbols.push(DgnLsSymbolReference {
                    symbol_component: Handle::NULL,
                    partial_strokes: reader.read_bit(),
                    clip_partial: reader.read_bit(),
                    allow_stretch: reader.read_bit(),
                    partial_projected: reader.read_bit(),
                    use_symbol_color: reader.read_bit(),
                    use_symbol_lineweight: reader.read_bit(),
                    justify: reader.read_bit_long(),
                    rotation_type: reader.read_bit_long(),
                    vertex_mask: reader.read_bit_long(),
                    x_offset: reader.read_bit_double(),
                    y_offset: reader.read_bit_double(),
                    angle: reader.read_bit_double(),
                    stroke_number: reader.read_bit_long(),
                });
            }
            let stroke_component = Handle::from(reader.read_handle());
            for symbol in &mut symbols {
                symbol.symbol_component = Handle::from(reader.read_handle());
            }
            DgnLsComponentData::Point(DgnLsPointComponent {
                stroke_component,
                symbols,
            })
        }
        DgnLsComponentType::Internal => {
            let pattern = read_stroke_pattern(reader);
            DgnLsComponentData::Internal(DgnLsInternalComponent {
                pattern,
                internal_version: reader.read_bit_long(),
                hardware_style: reader.read_bit_long(),
                is_hardware_style: reader.read_bit(),
                line_code: reader.read_bit_long(),
            })
        }
    };

    Some(DgnLineStyleData::Component {
        kind,
        description,
        version,
        component_uid,
        scale,
        property_flags,
        component,
        properties: Vec::new(),
    })
}

#[derive(Default)]
struct DefinitionFields {
    is_continuous: bool,
    unit_definition: f64,
    unit_scale: f64,
    units_type: i32,
    is_element: bool,
    is_physical: bool,
    is_scale_independent: bool,
    is_snappable: bool,
}

/// The fields of an `AcDbLSDefinition` after its UID.
///
/// They are read in the order the writer uses, from a copy of the bits that
/// are left in the main stream. Some drawings store bits that do not fit
/// that order (the record ends inside a field), and the order they do fit is
/// not known. Those bits are returned as they are, so the definition still
/// reads and is written back unchanged, and the fields keep their defaults.
fn read_definition_fields(
    reader: &mut DwgMergedReader,
) -> (DefinitionFields, Option<ProxyPayload>) {
    let bit_count = reader.main_remaining_bits().max(0);
    let mut data = vec![0u8; (bit_count as usize).div_ceil(8)];
    for bit_index in 0..bit_count as usize {
        if reader.read_bit() {
            data[bit_index / 8] |= 0x80 >> (bit_index % 8);
        }
    }

    let mut bits = DwgBitReader::new(data.clone(), reader.main().version(), reader.dxf_version());
    bits.set_end_bit(bit_count);
    let fields = DefinitionFields {
        is_continuous: bits.read_bit(),
        unit_definition: bits.read_bit_double(),
        unit_scale: bits.read_bit_double(),
        units_type: bits.read_bit_long(),
        is_element: bits.read_bit(),
        is_physical: bits.read_bit(),
        is_scale_independent: bits.read_bit(),
        is_snappable: bits.read_bit(),
    };
    if bits.check_error().is_ok() {
        (fields, None)
    } else {
        (
            DefinitionFields::default(),
            Some(ProxyPayload::from_bits(&data, bit_count as u32)),
        )
    }
}

fn read_uid(reader: &mut DwgMergedReader) -> [u8; 16] {
    let bytes = reader.read_bytes(16);
    let mut uid = [0; 16];
    uid[..bytes.len().min(16)].copy_from_slice(&bytes[..bytes.len().min(16)]);
    uid
}

fn read_byte(reader: &mut DwgMergedReader) -> u8 {
    reader.read_bytes(1).first().copied().unwrap_or(0)
}

fn safe_count(value: i32, reader: &DwgMergedReader) -> i32 {
    reader.checked_count(i64::from(value)) as i32
}

fn read_stroke_pattern(reader: &mut DwgMergedReader) -> DgnLsStrokePattern {
    let has_iteration_limit = reader.read_bit();
    let is_single_segment = reader.read_bit();
    let iteration_limit = reader.read_bit_long();
    // A BL in DWG, as AutoCAD writes it; read as a BD, a value of 1 left
    // eight bits to the fields after it.
    let auto_phase = f64::from(reader.read_bit_long());
    let phase = reader.read_bit_double();
    let phase_mode =
        DgnLsPhaseMode::from_code(((reader.read_bit() as u8) << 1) | reader.read_bit() as u8);
    let count = safe_count(reader.read_bit_long(), reader);
    let mut strokes = Vec::with_capacity(count as usize);
    for _ in 0..count {
        strokes.push(DgnLsStroke {
            is_dash: reader.read_bit(),
            bypass_corner: reader.read_bit(),
            can_be_scaled: reader.read_bit(),
            invert_at_origin: reader.read_bit(),
            invert_at_end: reader.read_bit(),
            length: reader.read_bit_double(),
            start_width: reader.read_bit_double(),
            end_width: reader.read_bit_double(),
            width_mode: reader.read_bit_long(),
            cap_mode: reader.read_bit_long(),
        });
    }
    DgnLsStrokePattern {
        has_iteration_limit,
        is_single_segment,
        iteration_limit,
        auto_phase,
        phase,
        phase_mode,
        strokes,
    }
}
