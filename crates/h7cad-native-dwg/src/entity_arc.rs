//! AC1015 ARC entity geometry decoder.
//!
//! `sample_AC1015.dwg` reports a small ARC set. The payload is CIRCLE plus two
//! trailing bit-doubles for the sweep angles.
//!
//! On-disk layout (R2000+, per ACadSharp `DwgEntityReader.ReadArc`):
//!
//! ```text
//!   3BD  center
//!   BD   radius
//!   BT   thickness
//!   BE   extrusion (normal)
//!   BD   start_angle
//!   BD   end_angle
//! ```
//!
//! AutoCAD stores both angles in radians, measured counter-clockwise
//! from the OCS +X axis. The native model's `EntityData::Arc` uses
//! the same convention, so no conversion is needed when wrapping the
//! decoded struct.

use crate::bit_reader::BitReader;
use crate::DwgReadError;

/// Decoded geometric fields of a DWG AC1015 ARC entity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArcGeometry {
    pub center: [f64; 3],
    pub radius: f64,
    pub thickness: f64,
    pub extrusion: [f64; 3],
    /// Start angle in radians, measured counter-clockwise from the
    /// OCS +X axis.
    pub start_angle: f64,
    /// End angle in radians; `start_angle` -> `end_angle` sweeps
    /// counter-clockwise (AutoCAD convention).
    pub end_angle: f64,
}

/// Read the AC1015 ARC payload from `reader`.
///
/// Contract mirrors the other `entity_*` readers: caller must have
/// skipped the common entity preamble. Truncation surfaces as
/// [`DwgReadError::UnexpectedEof`].
pub fn read_arc_geometry(reader: &mut BitReader<'_>) -> Result<ArcGeometry, DwgReadError> {
    let center = reader.read_3bit_double()?;
    let radius = reader.read_bit_double()?;
    let thickness = reader.read_bit_thickness_r2000_plus()?;
    let extrusion = reader.read_bit_extrusion_r2000_plus()?;
    let start_angle = reader.read_bit_double()?;
    let end_angle = reader.read_bit_double()?;
    Ok(ArcGeometry {
        center,
        radius,
        thickness,
        extrusion,
        start_angle,
        end_angle,
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
    fn arc_geometry_shortest_encoding_all_defaults() {
        // 3BD all prefix 10 + BD radius prefix 01 + BT flag 1 + BE flag 1 +
        // 2 × BD prefix 10.  Total = 6 + 2 + 1 + 1 + 4 = 14 bits.
        let mut bytes = Vec::new();
        let mut cursor = 0usize;
        for _ in 0..3 {
            emit_bits(&mut bytes, &mut cursor, 0b10, 2);
        }
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        emit_bits(&mut bytes, &mut cursor, 1, 1);
        emit_bits(&mut bytes, &mut cursor, 1, 1);
        emit_bits(&mut bytes, &mut cursor, 0b10, 2);
        emit_bits(&mut bytes, &mut cursor, 0b10, 2);

        let mut reader = BitReader::new(&bytes);
        let geom = read_arc_geometry(&mut reader).unwrap();
        assert_eq!(geom.center, [0.0, 0.0, 0.0]);
        assert_eq!(geom.radius, 1.0);
        assert_eq!(geom.thickness, 0.0);
        assert_eq!(geom.extrusion, [0.0, 0.0, 1.0]);
        assert_eq!(geom.start_angle, 0.0);
        assert_eq!(geom.end_angle, 0.0);
        assert_eq!(reader.position_in_bits(), 14);
    }

    #[test]
    fn arc_geometry_reads_nontrivial_sweep() {
        // center = (1, 1, 0), radius = 1, thickness = 0, extrusion = default,
        // start_angle = 0, end_angle = 1.
        let mut bytes = Vec::new();
        let mut cursor = 0usize;
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        emit_bits(&mut bytes, &mut cursor, 0b10, 2);
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        emit_bits(&mut bytes, &mut cursor, 1, 1);
        emit_bits(&mut bytes, &mut cursor, 1, 1);
        emit_bits(&mut bytes, &mut cursor, 0b10, 2);
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);

        let mut reader = BitReader::new(&bytes);
        let geom = read_arc_geometry(&mut reader).unwrap();
        assert_eq!(geom.center, [1.0, 1.0, 0.0]);
        assert_eq!(geom.radius, 1.0);
        assert_eq!(geom.start_angle, 0.0);
        assert_eq!(geom.end_angle, 1.0);
    }

    #[test]
    fn arc_geometry_reports_eof_on_empty_stream() {
        let err = read_arc_geometry(&mut BitReader::new(&[])).unwrap_err();
        assert!(matches!(err, DwgReadError::UnexpectedEof { .. }));
    }
}
