//! AC1015 ELLIPSE entity geometry decoder.
//!
//! On-disk layout (R2000+, per ACadSharp `DwgEntityReader.ReadEllipse`):
//!
//! ```text
//!   3BD  center
//!   3BD  major_axis_endpoint (SM_axis, endpoint relative to center)
//!   3BD  extrusion (normal)
//!   BD   axis_ratio (minor/major)
//!   BD   start_angle (radians, 0 = full ellipse start)
//!   BD   end_angle   (radians, 2*PI = full ellipse end)
//! ```
//!
//! Unlike CIRCLE/ARC the extrusion is NOT encoded with the R2000+
//! short-circuit `BE` format — it is a plain `3BD` triple.

use crate::bit_reader::BitReader;
use crate::DwgReadError;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EllipseGeometry {
    pub center: [f64; 3],
    pub major_axis: [f64; 3],
    pub extrusion: [f64; 3],
    pub ratio: f64,
    pub start_param: f64,
    pub end_param: f64,
}

pub fn read_ellipse_geometry(reader: &mut BitReader<'_>) -> Result<EllipseGeometry, DwgReadError> {
    let center = reader.read_3bit_double()?;
    let major_axis = reader.read_3bit_double()?;
    let extrusion = reader.read_3bit_double()?;
    let ratio = reader.read_bit_double()?;
    let start_param = reader.read_bit_double()?;
    let end_param = reader.read_bit_double()?;
    Ok(EllipseGeometry {
        center,
        major_axis,
        extrusion,
        ratio,
        start_param,
        end_param,
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
    fn ellipse_shortest_encoding() {
        // 3×3BD all prefix 10 (18 bits) + 3×BD prefix 01 (6 bits) = 24 bits
        let mut bytes = Vec::new();
        let mut cursor = 0usize;
        for _ in 0..9 {
            emit_bits(&mut bytes, &mut cursor, 0b10, 2);
        }
        for _ in 0..3 {
            emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        }
        let mut reader = BitReader::new(&bytes);
        let geom = read_ellipse_geometry(&mut reader).unwrap();
        assert_eq!(geom.center, [0.0, 0.0, 0.0]);
        assert_eq!(geom.major_axis, [0.0, 0.0, 0.0]);
        assert_eq!(geom.extrusion, [0.0, 0.0, 0.0]);
        assert_eq!(geom.ratio, 1.0);
        assert_eq!(geom.start_param, 1.0);
        assert_eq!(geom.end_param, 1.0);
    }

    #[test]
    fn ellipse_reports_eof_on_empty() {
        let err = read_ellipse_geometry(&mut BitReader::new(&[])).unwrap_err();
        assert!(matches!(err, DwgReadError::UnexpectedEof { .. }));
    }
}
