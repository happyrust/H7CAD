//! AC1015 POINT entity geometry decoder.
//!
//! `sample_AC1015.dwg` reports 34 POINTs, the second-highest geometric
//! entity count after LINE (82). The decoder follows the same pattern
//! as [`entity_line`](crate::entity_line) and [`entity_circle`](crate::entity_circle)
//! so `lib::try_decode_entity_body` can dispatch on `object_type == 27`
//! with a single extra arm.
//!
//! On-disk layout (R2000+, per ACadSharp `DwgEntityReader.ReadPoint`):
//!
//! ```text
//!   3BD  location
//!   BT   thickness
//!   BE   extrusion (normal)
//!   BD   x_axis_angle
//! ```
//!
//! DWG stores POINT with an optional x_axis_angle used by the PDMODE
//! renderer for glyph orientation. The native model's
//! `EntityData::Point` only carries `position`; the other three
//! fields are retained on the decoder struct for future schema
//! growth but discarded when the enrich pipeline wraps it as an
//! `EntityData::Point`.

use crate::bit_reader::BitReader;
use crate::DwgReadError;

/// Decoded geometric fields of a DWG AC1015 POINT entity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointGeometry {
    pub position: [f64; 3],
    pub thickness: f64,
    pub extrusion: [f64; 3],
    /// OCS X-axis rotation used by PDMODE glyph renderers.
    pub x_axis_angle: f64,
}

/// Read the AC1015 POINT payload from `reader`.
///
/// Contract mirrors [`read_line_geometry`](crate::read_line_geometry):
/// caller must have skipped the common entity preamble before
/// calling. Truncation surfaces as [`DwgReadError::UnexpectedEof`].
pub fn read_point_geometry(reader: &mut BitReader<'_>) -> Result<PointGeometry, DwgReadError> {
    let position = reader.read_3bit_double()?;
    let thickness = reader.read_bit_thickness_r2000_plus()?;
    let extrusion = reader.read_bit_extrusion_r2000_plus()?;
    let x_axis_angle = reader.read_bit_double()?;
    Ok(PointGeometry {
        position,
        thickness,
        extrusion,
        x_axis_angle,
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
    fn point_geometry_shortest_encoding_all_defaults() {
        // 3BD all prefix 10 (literal 0) + BT flag=1 + BE flag=1 + BD prefix 10.
        //   3 × 2 + 1 + 1 + 2 = 10 bits.
        let mut bytes = Vec::new();
        let mut cursor = 0usize;
        for _ in 0..3 {
            emit_bits(&mut bytes, &mut cursor, 0b10, 2);
        }
        emit_bits(&mut bytes, &mut cursor, 1, 1);
        emit_bits(&mut bytes, &mut cursor, 1, 1);
        emit_bits(&mut bytes, &mut cursor, 0b10, 2);

        let mut reader = BitReader::new(&bytes);
        let geom = read_point_geometry(&mut reader).unwrap();
        assert_eq!(geom.position, [0.0, 0.0, 0.0]);
        assert_eq!(geom.thickness, 0.0);
        assert_eq!(geom.extrusion, [0.0, 0.0, 1.0]);
        assert_eq!(geom.x_axis_angle, 0.0);
        assert_eq!(reader.position_in_bits(), 10);
    }

    #[test]
    fn point_geometry_reads_nontrivial_position_and_angle() {
        let mut bytes = Vec::new();
        let mut cursor = 0usize;
        // position = (1, 1, 0)
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        emit_bits(&mut bytes, &mut cursor, 0b10, 2);
        // thickness 0
        emit_bits(&mut bytes, &mut cursor, 1, 1);
        // extrusion default
        emit_bits(&mut bytes, &mut cursor, 1, 1);
        // x_axis_angle = 1.0
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);

        let mut reader = BitReader::new(&bytes);
        let geom = read_point_geometry(&mut reader).unwrap();
        assert_eq!(geom.position, [1.0, 1.0, 0.0]);
        assert_eq!(geom.x_axis_angle, 1.0);
    }

    #[test]
    fn point_geometry_reports_eof_on_empty_stream() {
        let err = read_point_geometry(&mut BitReader::new(&[])).unwrap_err();
        assert!(matches!(err, DwgReadError::UnexpectedEof { .. }));
    }
}
