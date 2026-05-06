//! AC1015 VIEWPORT entity decoder.
//!
//! On-disk layout (R2000, per ACadSharp `DwgEntityReader.ReadViewport`):
//!
//! ```text
//!   3BD  center
//!   BD   width
//!   BD   height
//!   ... many additional fields (view direction, twist, lens length,
//!       frozen layers, etc.) which we skip for now.
//! ```
//!
//! The full VIEWPORT spec is extensive. This minimal decoder only
//! extracts center, width, and height — enough to represent the
//! viewport rectangle in the native model.

use crate::bit_reader::BitReader;
use crate::DwgReadError;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportGeometry {
    pub center: [f64; 3],
    pub width: f64,
    pub height: f64,
}

pub fn read_viewport_geometry(
    reader: &mut BitReader<'_>,
) -> Result<ViewportGeometry, DwgReadError> {
    let center = reader.read_3bit_double()?;
    let width = reader.read_bit_double()?;
    let height = reader.read_bit_double()?;
    Ok(ViewportGeometry {
        center,
        width,
        height,
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
    fn viewport_reads_center_and_size() {
        let mut bytes = Vec::new();
        let mut cursor = 0usize;
        // center = (0, 0, 0) via 3BD prefix 10
        for _ in 0..3 {
            emit_bits(&mut bytes, &mut cursor, 0b10, 2);
        }
        // width = 1.0 via BD prefix 01
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        // height = 1.0
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);

        let mut reader = BitReader::new(&bytes);
        let geom = read_viewport_geometry(&mut reader).unwrap();
        assert_eq!(geom.center, [0.0, 0.0, 0.0]);
        assert_eq!(geom.width, 1.0);
        assert_eq!(geom.height, 1.0);
    }

    #[test]
    fn viewport_reports_eof_on_empty() {
        let err = read_viewport_geometry(&mut BitReader::new(&[])).unwrap_err();
        assert!(matches!(err, DwgReadError::UnexpectedEof { .. }));
    }
}
