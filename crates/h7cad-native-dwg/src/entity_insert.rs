//! AC1015 INSERT entity decoder.
//!
//! On-disk layout (R2000, per ACadSharp `DwgEntityReader.ReadInsert`):
//!
//! ```text
//!   3BD  insertion
//!   DD   scale_x      (default 1.0)
//!   DD   scale_y      (default scale_x)
//!   DD   scale_z      (default scale_x)
//!   BD   rotation
//!   3BD  extrusion
//!   B    has_attribs
//! ```
//!
//! Handle section:
//! ```text
//!   H    block_header  (hard pointer)
//!   H    first_attrib  (if has_attribs)
//!   H    last_attrib   (if has_attribs)
//!   H    seqend        (if has_attribs)
//! ```

use crate::bit_reader::BitReader;
use crate::DwgReadError;
use h7cad_native_model::Handle;

#[derive(Debug, Clone, PartialEq)]
pub struct InsertGeometry {
    pub insertion: [f64; 3],
    pub scale: [f64; 3],
    pub rotation: f64,
    pub extrusion: [f64; 3],
    pub has_attribs: bool,
    pub block_header_handle: Handle,
}

pub fn read_insert_geometry(
    main_reader: &mut BitReader<'_>,
    handle_reader: &mut BitReader<'_>,
    object_handle: Handle,
) -> Result<InsertGeometry, DwgReadError> {
    let insertion = main_reader.read_3bit_double()?;

    let scale_flag = main_reader.read_bits(2)? as u8;
    let scale = match scale_flag {
        0b00 => {
            let x = main_reader.read_raw_f64_le()?;
            let dd_y = main_reader.read_bit_double_with_default(x)?;
            let dd_z = main_reader.read_bit_double_with_default(x)?;
            [x, dd_y, dd_z]
        }
        0b01 => [1.0, 1.0, 1.0],
        0b10 => {
            let x = main_reader.read_raw_f64_le()?;
            [x, x, x]
        }
        _ => {
            let x = main_reader.read_raw_f64_le()?;
            let dd_y = main_reader.read_bit_double_with_default(x)?;
            let dd_z = main_reader.read_bit_double_with_default(x)?;
            [x, dd_y, dd_z]
        }
    };

    let rotation = main_reader.read_bit_double()?;
    let extrusion = main_reader.read_3bit_double()?;
    let has_attribs = main_reader.read_bit()? == 1;

    let block_header_handle =
        Handle::new(handle_reader.read_handle_relative(object_handle.value())?);

    Ok(InsertGeometry {
        insertion,
        scale,
        rotation,
        extrusion,
        has_attribs,
        block_header_handle,
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
    fn insert_reports_eof_on_empty() {
        let handles = [0x00; 4];
        let err = read_insert_geometry(
            &mut BitReader::new(&[]),
            &mut BitReader::new(&handles),
            Handle::new(0x10),
        )
        .unwrap_err();
        assert!(matches!(err, DwgReadError::UnexpectedEof { .. }));
    }

    #[test]
    fn insert_reads_unit_scale() {
        let mut bytes = Vec::new();
        let mut cursor = 0usize;
        // insertion 3BD zeros
        for _ in 0..3 {
            emit_bits(&mut bytes, &mut cursor, 0b10, 2);
        }
        // scale flag = 01 (unit scale)
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        // rotation = 0
        emit_bits(&mut bytes, &mut cursor, 0b10, 2);
        // extrusion 3BD zeros
        for _ in 0..3 {
            emit_bits(&mut bytes, &mut cursor, 0b10, 2);
        }
        // has_attribs = 0
        emit_bits(&mut bytes, &mut cursor, 0, 1);

        let handles = [0x51, 0x42];
        let mut main_reader = BitReader::new(&bytes);
        let mut handle_reader = BitReader::new(&handles);
        let insert =
            read_insert_geometry(&mut main_reader, &mut handle_reader, Handle::new(0x10)).unwrap();
        assert_eq!(insert.insertion, [0.0, 0.0, 0.0]);
        assert_eq!(insert.scale, [1.0, 1.0, 1.0]);
        assert_eq!(insert.rotation, 0.0);
        assert!(!insert.has_attribs);
        assert_eq!(insert.block_header_handle, Handle::new(0x42));
    }
}
