//! AC1015 TEXT entity decoder.

use crate::bit_reader::BitReader;
use crate::DwgReadError;
use h7cad_native_model::Handle;

#[derive(Debug, Clone, PartialEq)]
pub struct TextGeometry {
    pub insertion: [f64; 3],
    pub alignment_point: Option<[f64; 3]>,
    pub extrusion: [f64; 3],
    pub thickness: f64,
    pub oblique_angle: f64,
    pub rotation: f64,
    pub height: f64,
    pub width_factor: f64,
    pub value: String,
    pub horizontal_alignment: i16,
    pub vertical_alignment: i16,
    pub style_handle: Handle,
}

pub fn read_text_geometry(
    main_reader: &mut BitReader<'_>,
    handle_reader: &mut BitReader<'_>,
    object_handle: Handle,
) -> Result<TextGeometry, DwgReadError> {
    let data_flags = main_reader.read_raw_u8()?;
    let elevation = if (data_flags & 0x01) == 0 {
        main_reader.read_raw_f64_le()?
    } else {
        0.0
    };
    let ix = main_reader.read_raw_f64_le()?;
    let iy = main_reader.read_raw_f64_le()?;
    let alignment_point = if (data_flags & 0x02) == 0 {
        let ax = main_reader.read_bit_double_with_default(ix)?;
        let ay = main_reader.read_bit_double_with_default(iy)?;
        Some([ax, ay, elevation])
    } else {
        None
    };
    let extrusion = main_reader.read_bit_extrusion_r2000_plus()?;
    let thickness = main_reader.read_bit_thickness_r2000_plus()?;
    let oblique_angle = if (data_flags & 0x04) == 0 {
        main_reader.read_raw_f64_le()?
    } else {
        0.0
    };
    let rotation = if (data_flags & 0x08) == 0 {
        main_reader.read_raw_f64_le()?
    } else {
        0.0
    };
    let height = main_reader.read_raw_f64_le()?;
    let width_factor = if (data_flags & 0x10) == 0 {
        main_reader.read_raw_f64_le()?
    } else {
        1.0
    };
    let value = main_reader.read_text_ascii()?;
    let _generation = if (data_flags & 0x20) == 0 {
        main_reader.read_bit_short()?
    } else {
        0
    };
    let horizontal_alignment = if (data_flags & 0x40) == 0 {
        main_reader.read_bit_short()?
    } else {
        0
    };
    let vertical_alignment = if (data_flags & 0x80) == 0 {
        main_reader.read_bit_short()?
    } else {
        0
    };
    let style_handle = Handle::new(handle_reader.read_handle_relative(object_handle.value())?);

    Ok(TextGeometry {
        insertion: [ix, iy, elevation],
        alignment_point,
        extrusion,
        thickness,
        oblique_angle,
        rotation,
        height,
        width_factor,
        value,
        horizontal_alignment,
        vertical_alignment,
        style_handle,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn emit_raw_f64(out: &mut Vec<u8>, cursor: &mut usize, value: f64) {
        for byte in value.to_le_bytes() {
            emit_bits(out, cursor, byte as u64, 8);
        }
    }

    #[test]
    fn text_geometry_decodes_minimal_r2000_payload() {
        let mut main = Vec::new();
        let mut cursor = 0usize;
        emit_bits(&mut main, &mut cursor, 0b1_1111_111, 8); // data_flags = 0xFF
        emit_raw_f64(&mut main, &mut cursor, 1.0);
        emit_raw_f64(&mut main, &mut cursor, 2.0);
        emit_bits(&mut main, &mut cursor, 1, 1); // extrusion default
        emit_bits(&mut main, &mut cursor, 1, 1); // thickness 0
        emit_raw_f64(&mut main, &mut cursor, 3.0); // height
        emit_bits(&mut main, &mut cursor, 0b01, 2); // text len => next byte
        emit_bits(&mut main, &mut cursor, 2, 8);
        emit_bits(&mut main, &mut cursor, b'A' as u64, 8);
        emit_bits(&mut main, &mut cursor, 0, 8);

        let handles = [0x51, 0x21];
        let mut main_reader = BitReader::new(&main);
        let mut handle_reader = BitReader::new(&handles);
        let text = read_text_geometry(&mut main_reader, &mut handle_reader, Handle::new(0x10))
            .unwrap();
        assert_eq!(text.insertion, [1.0, 2.0, 0.0]);
        assert_eq!(text.height, 3.0);
        assert_eq!(text.value, "A");
        assert_eq!(text.style_handle, Handle::new(0x21));
        assert_eq!(text.extrusion, [0.0, 0.0, 1.0]);
        assert_eq!(text.width_factor, 1.0);
    }
}
