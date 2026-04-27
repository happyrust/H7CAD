//! AC1015 MTEXT entity decoder.
//!
//! On-disk layout (R2000, per ACadSharp `DwgEntityReader.ReadMText`):
//!
//! ```text
//!   3BD  insertion
//!   3BD  extrusion
//!   3BD  x_direction
//!   BD   rect_width
//!   BD   rect_height        (R2007+, 0.0 for R2000)
//!   BD   text_height
//!   BS   attachment
//!   BS   drawing_direction
//!   BD   ext_height          (not used in model)
//!   BD   ext_width           (not used in model)
//!   T    text_value
//!   BS   line_spacing_style
//!   BD   line_spacing_factor
//!   B    unknown_bit
//!   ...  handles: style
//! ```

use crate::bit_reader::BitReader;
use crate::DwgReadError;
use h7cad_native_model::Handle;

#[derive(Debug, Clone, PartialEq)]
pub struct MTextGeometry {
    pub insertion: [f64; 3],
    pub extrusion: [f64; 3],
    pub x_direction: [f64; 3],
    pub rect_width: f64,
    pub rect_height: f64,
    pub height: f64,
    pub attachment_point: i16,
    pub drawing_direction: i16,
    pub value: String,
    pub line_spacing_factor: f64,
    pub style_handle: Handle,
    pub rotation: f64,
}

pub fn read_mtext_geometry(
    main_reader: &mut BitReader<'_>,
    handle_reader: &mut BitReader<'_>,
    object_handle: Handle,
) -> Result<MTextGeometry, DwgReadError> {
    let insertion = main_reader.read_3bit_double()?;
    let extrusion = main_reader.read_3bit_double()?;
    let x_direction = main_reader.read_3bit_double()?;
    let rect_width = main_reader.read_bit_double()?;
    let rect_height = main_reader.read_bit_double()?;
    let height = main_reader.read_bit_double()?;
    let attachment_point = main_reader.read_bit_short()?;
    let drawing_direction = main_reader.read_bit_short()?;
    let _ext_height = main_reader.read_bit_double()?;
    let _ext_width = main_reader.read_bit_double()?;
    let value = main_reader.read_text_ascii()?;
    let _line_spacing_style = main_reader.read_bit_short()?;
    let line_spacing_factor = main_reader.read_bit_double()?;
    let _unknown_bit = main_reader.read_bit()?;

    let style_handle = Handle::new(handle_reader.read_handle_relative(object_handle.value())?);

    let rotation = x_direction[1].atan2(x_direction[0]);

    Ok(MTextGeometry {
        insertion,
        extrusion,
        x_direction,
        rect_width,
        rect_height,
        height,
        attachment_point,
        drawing_direction,
        value,
        line_spacing_factor,
        style_handle,
        rotation,
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

    #[test]
    fn mtext_reports_eof_on_empty() {
        let handles = [0x00; 4];
        let err = read_mtext_geometry(
            &mut BitReader::new(&[]),
            &mut BitReader::new(&handles),
            Handle::new(0x10),
        )
        .unwrap_err();
        assert!(matches!(err, DwgReadError::UnexpectedEof { .. }));
    }

    #[test]
    fn mtext_reads_shortest_encoding() {
        let mut bytes = Vec::new();
        let mut cursor = 0usize;
        // insertion 3BD zeros (prefix 10 × 3)
        for _ in 0..3 {
            emit_bits(&mut bytes, &mut cursor, 0b10, 2);
        }
        // extrusion 3BD zeros
        for _ in 0..3 {
            emit_bits(&mut bytes, &mut cursor, 0b10, 2);
        }
        // x_direction = (1, 0, 0)
        emit_bits(&mut bytes, &mut cursor, 0b01, 2); // 1.0
        emit_bits(&mut bytes, &mut cursor, 0b10, 2); // 0.0
        emit_bits(&mut bytes, &mut cursor, 0b10, 2); // 0.0
        // rect_width = 1.0
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        // rect_height = 0.0
        emit_bits(&mut bytes, &mut cursor, 0b10, 2);
        // height = 1.0
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        // attachment = 0 (BS prefix 01 => 0)
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        emit_bits(&mut bytes, &mut cursor, 0, 8);
        // drawing_direction = 0
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        emit_bits(&mut bytes, &mut cursor, 0, 8);
        // ext_height = 0
        emit_bits(&mut bytes, &mut cursor, 0b10, 2);
        // ext_width = 0
        emit_bits(&mut bytes, &mut cursor, 0b10, 2);
        // text = "" (BS len = 0)
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        emit_bits(&mut bytes, &mut cursor, 0, 8);
        // line_spacing_style = 0
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        emit_bits(&mut bytes, &mut cursor, 0, 8);
        // line_spacing_factor = 1
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        // unknown_bit = 0
        emit_bits(&mut bytes, &mut cursor, 0, 1);

        let handles = [0x51, 0x21];
        let mut main_reader = BitReader::new(&bytes);
        let mut handle_reader = BitReader::new(&handles);
        let mtext =
            read_mtext_geometry(&mut main_reader, &mut handle_reader, Handle::new(0x10)).unwrap();
        assert_eq!(mtext.insertion, [0.0, 0.0, 0.0]);
        assert_eq!(mtext.height, 1.0);
        assert_eq!(mtext.rect_width, 1.0);
        assert_eq!(mtext.value, "");
        assert_eq!(mtext.style_handle, Handle::new(0x21));
    }
}
