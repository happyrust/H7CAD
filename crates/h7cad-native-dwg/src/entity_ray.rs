//! AC1015 RAY / XLINE entity geometry decoder.
//!
//! RAY (object_type = 38) and XLINE (object_type = 40) share the same
//! on-disk layout:
//!
//! ```text
//!   3BD  origin
//!   3BD  direction
//! ```

use crate::bit_reader::BitReader;
use crate::DwgReadError;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayGeometry {
    pub origin: [f64; 3],
    pub direction: [f64; 3],
}

pub fn read_ray_geometry(reader: &mut BitReader<'_>) -> Result<RayGeometry, DwgReadError> {
    let origin = reader.read_3bit_double()?;
    let direction = reader.read_3bit_double()?;
    Ok(RayGeometry { origin, direction })
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
    fn ray_reads_origin_and_direction() {
        let mut bytes = Vec::new();
        let mut cursor = 0usize;
        // origin all zero (prefix 10), direction all 1.0 (prefix 01)
        for _ in 0..3 {
            emit_bits(&mut bytes, &mut cursor, 0b10, 2);
        }
        for _ in 0..3 {
            emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        }
        let mut reader = BitReader::new(&bytes);
        let geom = read_ray_geometry(&mut reader).unwrap();
        assert_eq!(geom.origin, [0.0, 0.0, 0.0]);
        assert_eq!(geom.direction, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn ray_reports_eof_on_empty() {
        let err = read_ray_geometry(&mut BitReader::new(&[])).unwrap_err();
        assert!(matches!(err, DwgReadError::UnexpectedEof { .. }));
    }
}
