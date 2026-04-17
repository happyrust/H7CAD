//! AC1015 common object/entity preamble decoding.
//!
//! `read_ac1015_object_header` leaves the caller at the first bit of
//! the object body after `[BS type][RL main_size_bits][H self]`.
//! For AC1015 that body is split into:
//!
//! ```text
//! |--- main stream ---|--- handle stream ---|
//! ```
//!
//! The helpers in this module serve two purposes:
//!
//! 1. preserve the old "main-stream skip only" behaviour used by the
//!    early LINE/CIRCLE enrichment bricks, and
//! 2. provide richer decoders that keep the entity/non-entity common
//!    metadata needed by M3-B/M3-C.

use crate::bit_reader::BitReader;
use crate::DwgReadError;
use h7cad_native_model::Handle;

const DWG_LINEWEIGHT_VALUES: [i16; 24] = [
    0, 5, 9, 13, 15, 18, 20, 25, 30, 35, 40, 50, 53, 60, 70, 80, 90, 100, 106, 120, 140,
    158, 200, 211,
];

#[derive(Debug, Clone, PartialEq)]
pub struct Ac1015EntityCommonData {
    pub owner_handle: Handle,
    pub layer_handle: Handle,
    pub linetype_handle: Handle,
    /// 0=ByLayer, 1=ByBlock, 2=Continuous, 3=explicit handle.
    pub linetype_flags: u8,
    pub color_index: i16,
    pub linetype_scale: f64,
    pub lineweight: i16,
    pub invisible: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ac1015NonEntityCommonData {
    pub owner_handle: Handle,
}

fn skip_extended_entity_data(reader: &mut BitReader<'_>) -> Result<(), DwgReadError> {
    loop {
        let size = reader.read_bit_short()?;
        if size <= 0 {
            break;
        }
        // AC1015 entity EED carries the application handle inline in
        // the main stream.
        let _ = reader.read_handle()?;
        for _ in 0..size as usize {
            reader.read_raw_u8()?;
        }
    }
    Ok(())
}

fn skip_extended_non_entity_data(reader: &mut BitReader<'_>) -> Result<(), DwgReadError> {
    loop {
        let size = reader.read_bit_short()?;
        if size <= 0 {
            break;
        }
        for _ in 0..size as usize {
            reader.read_raw_u8()?;
        }
    }
    Ok(())
}

fn read_resolved_handle(
    handle_reader: &mut BitReader<'_>,
    reference_handle: Handle,
) -> Result<Handle, DwgReadError> {
    Ok(Handle::new(
        handle_reader.read_handle_relative(reference_handle.value())?,
    ))
}

fn consume_optional_handle(
    handle_reader: &mut BitReader<'_>,
    reference_handle: Handle,
) -> Result<Option<Handle>, DwgReadError> {
    let handle = read_resolved_handle(handle_reader, reference_handle)?;
    if handle == Handle::NULL {
        Ok(None)
    } else {
        Ok(Some(handle))
    }
}

#[inline]
fn safe_count(raw: i32) -> usize {
    raw.max(0).min(100_000) as usize
}

pub fn dwg_lineweight_from_index(index: u8) -> i16 {
    match index {
        28 | 29 => -1,
        30 => -2,
        31 => -3,
        i if (i as usize) < DWG_LINEWEIGHT_VALUES.len() => DWG_LINEWEIGHT_VALUES[i as usize],
        _ => -3,
    }
}

/// Advance `reader` past the AC1015 common entity preamble while
/// ignoring the handle stream.
pub fn skip_ac1015_entity_common_main_stream(
    reader: &mut BitReader<'_>,
) -> Result<(), DwgReadError> {
    skip_extended_entity_data(reader)?;

    let has_graphic = reader.read_bit()? == 1;
    if has_graphic {
        let graphic_size = reader.read_raw_u32_le()? as usize;
        for _ in 0..graphic_size {
            reader.read_raw_u8()?;
        }
    }

    let _entity_mode = reader.read_bits(2)?;
    let _reactor_count = reader.read_bit_long()?;
    let _nolinks = reader.read_bit()?;
    let _color = reader.read_bit_short()?;
    let _linetype_scale = reader.read_bit_double()?;
    let _linetype_flags = reader.read_bits(2)?;
    let _plotstyle_flags = reader.read_bits(2)?;
    let _invisible = reader.read_bit_short()?;
    let _lineweight = reader.read_raw_u8()?;

    Ok(())
}

pub fn parse_ac1015_entity_common(
    main_reader: &mut BitReader<'_>,
    handle_reader: &mut BitReader<'_>,
    object_handle: Handle,
) -> Result<Ac1015EntityCommonData, DwgReadError> {
    skip_extended_entity_data(main_reader)?;

    let has_graphic = main_reader.read_bit()? == 1;
    if has_graphic {
        let graphic_size = main_reader.read_raw_u32_le()? as usize;
        for _ in 0..graphic_size {
            main_reader.read_raw_u8()?;
        }
    }

    let entity_mode = main_reader.read_bits(2)? as u8;
    let owner_handle = if entity_mode == 0 {
        read_resolved_handle(handle_reader, object_handle)?
    } else {
        Handle::NULL
    };

    let reactor_count = safe_count(main_reader.read_bit_long()?);
    for _ in 0..reactor_count {
        let _ = read_resolved_handle(handle_reader, object_handle)?;
    }

    let _xdictionary_handle = consume_optional_handle(handle_reader, object_handle)?;

    let nolinks = main_reader.read_bit()? == 1;
    if !nolinks {
        let _ = read_resolved_handle(handle_reader, object_handle)?;
        let _ = read_resolved_handle(handle_reader, object_handle)?;
    }

    let color_index = main_reader.read_bit_short()?;
    let linetype_scale = main_reader.read_bit_double()?;

    let layer_handle = read_resolved_handle(handle_reader, object_handle)?;

    let linetype_flags = main_reader.read_bits(2)? as u8;
    let linetype_handle = if linetype_flags == 0b11 {
        read_resolved_handle(handle_reader, object_handle)?
    } else {
        Handle::NULL
    };

    let plotstyle_flags = main_reader.read_bits(2)? as u8;
    if plotstyle_flags == 0b11 {
        let _ = read_resolved_handle(handle_reader, object_handle)?;
    }

    let invisible = main_reader.read_bit_short()? != 0;
    let lineweight = dwg_lineweight_from_index(main_reader.read_raw_u8()?);

    Ok(Ac1015EntityCommonData {
        owner_handle,
        layer_handle,
        linetype_handle,
        linetype_flags,
        color_index,
        linetype_scale,
        lineweight,
        invisible,
    })
}

pub fn parse_ac1015_non_entity_common(
    main_reader: &mut BitReader<'_>,
    handle_reader: &mut BitReader<'_>,
    object_handle: Handle,
) -> Result<Ac1015NonEntityCommonData, DwgReadError> {
    skip_extended_non_entity_data(main_reader)?;

    let owner_handle = read_resolved_handle(handle_reader, object_handle)?;
    let reactor_count = safe_count(main_reader.read_bit_long()?);
    for _ in 0..reactor_count {
        let _ = read_resolved_handle(handle_reader, object_handle)?;
    }
    let _xdictionary_handle = consume_optional_handle(handle_reader, object_handle)?;

    Ok(Ac1015NonEntityCommonData { owner_handle })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// MSB-first emitter shared with the entity decoder tests.
    fn emit_bits(out: &mut Vec<u8>, cursor: &mut usize, value: u64, count: u8) {
        for bit in (0..count).rev() {
            let byte_idx = *cursor / 8;
            let bit_idx = 7 - (*cursor % 8);
            while out.len() <= byte_idx {
                out.push(0);
            }
            if ((value >> bit) & 1) == 1 {
                out[byte_idx] |= 1 << bit_idx;
            }
            *cursor += 1;
        }
    }

    fn emit_bit_double_zero(out: &mut Vec<u8>, cursor: &mut usize) {
        emit_bits(out, cursor, 0b10, 2);
    }

    fn emit_bit_short_zero(out: &mut Vec<u8>, cursor: &mut usize) {
        emit_bits(out, cursor, 0b10, 2);
    }

    fn emit_bit_long_zero(out: &mut Vec<u8>, cursor: &mut usize) {
        emit_bits(out, cursor, 0b10, 2);
    }

    /// Build a synthetic common-entity preamble that represents the
    /// most permissive case: no EED blocks, no graphic, entity_mode
    /// 01 (by parent), zero reactors, nolinks, color 0, lt_scale 0,
    /// linetype bylayer, plotstyle bylayer, visible, lineweight 0.
    fn synth_minimal_preamble() -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut cursor = 0usize;
        emit_bit_short_zero(&mut bytes, &mut cursor);
        emit_bits(&mut bytes, &mut cursor, 0, 1);
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        emit_bit_long_zero(&mut bytes, &mut cursor);
        emit_bits(&mut bytes, &mut cursor, 1, 1);
        emit_bit_short_zero(&mut bytes, &mut cursor);
        emit_bit_double_zero(&mut bytes, &mut cursor);
        emit_bits(&mut bytes, &mut cursor, 0b00, 2);
        emit_bits(&mut bytes, &mut cursor, 0b00, 2);
        emit_bit_short_zero(&mut bytes, &mut cursor);
        emit_bits(&mut bytes, &mut cursor, 0, 8);
        bytes
    }

    #[test]
    fn skip_minimal_preamble_advances_the_reader_without_panic() {
        let bytes = synth_minimal_preamble();
        let mut reader = BitReader::new(&bytes);
        skip_ac1015_entity_common_main_stream(&mut reader).expect("minimal preamble must decode");
        assert!(reader.position_in_bits() > 0);
    }

    #[test]
    fn skip_preamble_leaves_reader_at_position_after_all_eleven_fields() {
        let bytes = synth_minimal_preamble();
        let mut reader = BitReader::new(&bytes);
        skip_ac1015_entity_common_main_stream(&mut reader).unwrap();
        assert_eq!(reader.position_in_bits(), 26);
    }

    #[test]
    fn skip_preamble_drains_a_single_eed_block() {
        let mut bytes = Vec::new();
        let mut cursor = 0usize;
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        emit_bits(&mut bytes, &mut cursor, 3, 8);
        emit_bits(&mut bytes, &mut cursor, 0x00, 8);
        emit_bits(&mut bytes, &mut cursor, 0xAA, 8);
        emit_bits(&mut bytes, &mut cursor, 0xBB, 8);
        emit_bits(&mut bytes, &mut cursor, 0xCC, 8);
        emit_bit_short_zero(&mut bytes, &mut cursor);
        emit_bits(&mut bytes, &mut cursor, 0, 1);
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        emit_bit_long_zero(&mut bytes, &mut cursor);
        emit_bits(&mut bytes, &mut cursor, 1, 1);
        emit_bit_short_zero(&mut bytes, &mut cursor);
        emit_bit_double_zero(&mut bytes, &mut cursor);
        emit_bits(&mut bytes, &mut cursor, 0b00, 2);
        emit_bits(&mut bytes, &mut cursor, 0b00, 2);
        emit_bit_short_zero(&mut bytes, &mut cursor);
        emit_bits(&mut bytes, &mut cursor, 0, 8);

        let mut reader = BitReader::new(&bytes);
        skip_ac1015_entity_common_main_stream(&mut reader).unwrap();
        assert_eq!(reader.position_in_bits(), 68);
    }

    #[test]
    fn skip_preamble_reports_eof_on_truncated_stream() {
        let err = skip_ac1015_entity_common_main_stream(&mut BitReader::new(&[0])).unwrap_err();
        assert!(matches!(err, DwgReadError::UnexpectedEof { .. }));
    }

    #[test]
    fn parse_common_entity_reads_handles_and_flags() {
        let main = synth_minimal_preamble();
        let handle_bytes = [
            0x00, // xdict = none
            0x51, 0x20, // layer handle
        ];

        let mut main_reader = BitReader::new(&main);
        let mut handle_reader = BitReader::new(&handle_bytes);
        let common = parse_ac1015_entity_common(
            &mut main_reader,
            &mut handle_reader,
            Handle::new(0x40),
        )
        .unwrap();

        assert_eq!(common.owner_handle, Handle::NULL);
        assert_eq!(common.layer_handle, Handle::new(0x20));
        assert_eq!(common.linetype_handle, Handle::NULL);
        assert_eq!(common.linetype_flags, 0);
        assert_eq!(common.color_index, 0);
        assert_eq!(common.linetype_scale, 0.0);
        assert_eq!(common.lineweight, 0);
        assert!(!common.invisible);
    }

    #[test]
    fn parse_common_entity_reads_owned_mode_and_explicit_linetype() {
        let mut main = Vec::new();
        let mut cursor = 0usize;
        emit_bit_short_zero(&mut main, &mut cursor); // eed terminator
        emit_bits(&mut main, &mut cursor, 0, 1); // no graphic
        emit_bits(&mut main, &mut cursor, 0b00, 2); // entity_mode owned
        emit_bit_long_zero(&mut main, &mut cursor); // no reactors
        emit_bits(&mut main, &mut cursor, 1, 1); // nolinks
        emit_bits(&mut main, &mut cursor, 0b01, 2); // color = next byte
        emit_bits(&mut main, &mut cursor, 7, 8);
        emit_bit_double_zero(&mut main, &mut cursor); // ltscale 0
        emit_bits(&mut main, &mut cursor, 0b11, 2); // explicit ltype handle
        emit_bits(&mut main, &mut cursor, 0b00, 2); // plotstyle bylayer
        emit_bit_short_zero(&mut main, &mut cursor); // visible
        emit_bits(&mut main, &mut cursor, 29, 8); // ByLayer lineweight

        let handle_bytes = [
            0x51, 0x10, // owner
            0x00, // xdict
            0x51, 0x20, // layer handle
            0x51, 0x30, // linetype handle
        ];

        let mut main_reader = BitReader::new(&main);
        let mut handle_reader = BitReader::new(&handle_bytes);
        let common = parse_ac1015_entity_common(
            &mut main_reader,
            &mut handle_reader,
            Handle::new(0x40),
        )
        .unwrap();

        assert_eq!(common.owner_handle, Handle::new(0x10));
        assert_eq!(common.layer_handle, Handle::new(0x20));
        assert_eq!(common.linetype_handle, Handle::new(0x30));
        assert_eq!(common.linetype_flags, 0b11);
        assert_eq!(common.color_index, 7);
        assert_eq!(common.lineweight, -1);
    }

    #[test]
    fn parse_non_entity_common_reads_owner_from_handle_stream() {
        let main = [0xA0]; // EED terminator BS=0, then BL reactor_count=0
        let handles = [
            0x51, 0x77, // owner
            0x00, // xdict
        ];

        let mut main_reader = BitReader::new(&main);
        let mut handle_reader = BitReader::new(&handles);
        let common = parse_ac1015_non_entity_common(
            &mut main_reader,
            &mut handle_reader,
            Handle::new(0x40),
        )
        .unwrap();
        assert_eq!(common.owner_handle, Handle::new(0x77));
    }

    #[test]
    fn lineweight_index_maps_special_values() {
        assert_eq!(dwg_lineweight_from_index(29), -1);
        assert_eq!(dwg_lineweight_from_index(30), -2);
        assert_eq!(dwg_lineweight_from_index(31), -3);
        assert_eq!(dwg_lineweight_from_index(7), 25);
    }
}
