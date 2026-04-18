//! AC1015 CIRCLE entity geometry decoder.
//!
//! Complements [`entity_line`](crate::entity_line) with the CIRCLE
//! (object_type = 18) geometric payload. `sample_AC1015.dwg` only
//! reports a small number of true CIRCLE records versus LINE/TEXT —
//! the incremental entity count
//! is small — but the decoder serves as the first validation that the
//! LINE pattern (dedicated `read_<kind>_geometry` + pure `<Kind>Geometry`
//! struct) generalises across entity classes without per-type
//! bit-stream scaffolding.
//!
//! On-disk layout (R2000+, per ACadSharp `DwgEntityReader.ReadCircle`):
//!
//! ```text
//!   3BD  center
//!   BD   radius
//!   BT   thickness
//!   BE   extrusion (normal)
//! ```
//!
//! There is no `z_is_zero` short-circuit (unlike LINE) because CIRCLE
//! always carries its full 3D center; AutoCAD emits a genuine 3BD
//! triple regardless of whether the drawing is flat.
//!
//! Scope rules match `entity_line`: AC1015 only, entity-specific
//! payload only (common entity header must have been skipped by
//! [`skip_ac1015_entity_common_main_stream`](crate::skip_ac1015_entity_common_main_stream)).

use crate::bit_reader::BitReader;
use crate::DwgReadError;

/// Decoded geometric fields of a DWG AC1015 CIRCLE entity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CircleGeometry {
    pub center: [f64; 3],
    pub radius: f64,
    /// Thickness extrudes the circle perpendicular to `extrusion`,
    /// producing a 2D ring. `0.0` for a pure circle.
    pub thickness: f64,
    /// OCS normal. `[0, 0, 1]` for world-XY circles.
    pub extrusion: [f64; 3],
}

/// Read the AC1015 CIRCLE payload from `reader`.
///
/// Contract mirrors [`read_line_geometry`](crate::read_line_geometry):
/// caller must have positioned `reader` right after the common entity
/// preamble; on success the reader sits immediately after the BE
/// extrusion. Truncation surfaces as [`DwgReadError::UnexpectedEof`].
pub fn read_circle_geometry(reader: &mut BitReader<'_>) -> Result<CircleGeometry, DwgReadError> {
    let center = reader.read_3bit_double()?;
    let radius = reader.read_bit_double()?;
    let thickness = reader.read_bit_thickness_r2000_plus()?;
    let extrusion = reader.read_bit_extrusion_r2000_plus()?;
    Ok(CircleGeometry {
        center,
        radius,
        thickness,
        extrusion,
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
    fn circle_geometry_reads_shortest_encoding_with_defaults() {
        // Shortest possible CIRCLE payload:
        //   3BD all literal 0.0 via prefix 10 (2 bits × 3 = 6 bits)
        //   BD radius = 1.0 via prefix 01 (2 bits)
        //   BT flag 1 = 0.0 (1 bit)
        //   BE flag 1 = default (0,0,1) (1 bit)
        // Total = 10 bits.
        let mut bytes = Vec::new();
        let mut cursor = 0usize;
        for _ in 0..3 {
            emit_bits(&mut bytes, &mut cursor, 0b10, 2);
        }
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        emit_bits(&mut bytes, &mut cursor, 1, 1);
        emit_bits(&mut bytes, &mut cursor, 1, 1);

        let mut reader = BitReader::new(&bytes);
        let geom = read_circle_geometry(&mut reader).unwrap();
        assert_eq!(geom.center, [0.0, 0.0, 0.0]);
        assert_eq!(geom.radius, 1.0);
        assert_eq!(geom.thickness, 0.0);
        assert_eq!(geom.extrusion, [0.0, 0.0, 1.0]);
        assert_eq!(reader.position_in_bits(), 10);
    }

    #[test]
    fn circle_geometry_decodes_nontrivial_center_and_thickness() {
        // center = (1, 1, 0), radius = 1, thickness = 1, extrusion = (1, 1, 1)
        let mut bytes = Vec::new();
        let mut cursor = 0usize;
        // center.x = 1 via BD prefix 01 (2 bits)
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        // center.y = 1 via BD prefix 01
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        // center.z = 0 via BD prefix 10
        emit_bits(&mut bytes, &mut cursor, 0b10, 2);
        // radius = 1 via BD prefix 01
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        // thickness flag 0 + BD prefix 01 = 1.0
        emit_bits(&mut bytes, &mut cursor, 0, 1);
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        // extrusion flag 0 + 3BD each prefix 01 = 1
        emit_bits(&mut bytes, &mut cursor, 0, 1);
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);

        let mut reader = BitReader::new(&bytes);
        let geom = read_circle_geometry(&mut reader).unwrap();
        assert_eq!(geom.center, [1.0, 1.0, 0.0]);
        assert_eq!(geom.radius, 1.0);
        assert_eq!(geom.thickness, 1.0);
        assert_eq!(geom.extrusion, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn circle_geometry_reports_eof_on_truncated_payload() {
        let err = read_circle_geometry(&mut BitReader::new(&[0x00])).unwrap_err();
        assert!(matches!(err, DwgReadError::UnexpectedEof { .. }));
    }
}
