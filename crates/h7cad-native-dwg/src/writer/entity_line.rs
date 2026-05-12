//! AC1015 LINE entity body writer (F5.M4.D).
//!
//! Inverse of [`crate::read_line_geometry`]. Mirrors the on-disk
//! layout documented in `entity_line.rs`:
//!
//! ```text
//!   B   z_are_zero
//!   RD  sx
//!   DD  ex (default = sx)
//!   RD  sy
//!   DD  ey (default = sy)
//!   if !z_are_zero:
//!       RD  sz
//!       DD  ez (default = sz)
//!   BT  thickness
//!   BE  extrusion (normal)
//! ```
//!
//! z_are_zero is auto-detected: when `start[2]` and `end[2]` both
//! compare equal to `0.0`, the writer takes the compact path that
//! omits the sz/ez pair (saves 80+ bits per LINE). Note that
//! `-0.0 == 0.0` in IEEE 754, so a LINE with `-0.0` z components
//! also collapses to the compact path; the reader will recover the
//! z values as `+0.0`. AutoCAD treats `-0.0` and `+0.0` as
//! geometrically identical, so this matches the documented
//! semantics.
//!
//! The writer assumes the caller has already emitted the common
//! entity header (M4.C) into the same `BitWriter`; on exit the
//! writer sits immediately after the BE extrusion and the caller
//! can attach trailing common-entity bits or hand the body to
//! [`crate::compose_ac1015_object_slice`] for slice composition.

use crate::bit_writer::BitWriter;
use crate::entity_line::LineGeometry;
use crate::DwgWriteError;

/// Write the AC1015 LINE-specific payload for `geom` into `writer`.
pub fn write_line_geometry(
    geom: LineGeometry,
    writer: &mut BitWriter,
) -> Result<(), DwgWriteError> {
    let z_are_zero = geom.start[2] == 0.0 && geom.end[2] == 0.0;
    writer.write_bit(if z_are_zero { 1 } else { 0 })?;

    writer.write_raw_f64_le(geom.start[0])?;
    writer.write_bit_double_with_default(geom.end[0], geom.start[0])?;
    writer.write_raw_f64_le(geom.start[1])?;
    writer.write_bit_double_with_default(geom.end[1], geom.start[1])?;

    if !z_are_zero {
        writer.write_raw_f64_le(geom.start[2])?;
        writer.write_bit_double_with_default(geom.end[2], geom.start[2])?;
    }

    writer.write_bit_thickness_r2000_plus(geom.thickness)?;
    writer.write_bit_extrusion_r2000_plus(geom.extrusion)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read_line_geometry, BitReader, BitWriter};

    fn round_trip(geom: LineGeometry) -> LineGeometry {
        let mut writer = BitWriter::new();
        write_line_geometry(geom, &mut writer).expect("writes");
        let bytes = writer.into_bytes();
        let mut reader = BitReader::new(&bytes);
        read_line_geometry(&mut reader).expect("reader recovers")
    }

    #[test]
    fn round_trips_2d_synthesis() {
        // Mirror of reader-side `line_geometry_round_trips_2d_synthesis`:
        // 2D LINE with z = 0, thickness = 0, default extrusion.
        // Compact path on every front.
        let geom = LineGeometry {
            start: [1.0, 2.0, 0.0],
            end: [4.0, 5.0, 0.0],
            thickness: 0.0,
            extrusion: [0.0, 0.0, 1.0],
        };
        assert_eq!(round_trip(geom), geom);
    }

    #[test]
    fn round_trips_with_z_when_not_zero() {
        // Mirror of reader-side `line_geometry_reads_z_when_not_zero_flag`:
        // 3D LINE forces z_are_zero = 0 and emits sz/ez.
        let geom = LineGeometry {
            start: [1.0, 3.0, 5.0],
            end: [2.0, 4.0, 6.0],
            thickness: 0.0,
            extrusion: [0.0, 0.0, 1.0],
        };
        assert_eq!(round_trip(geom), geom);
    }

    #[test]
    fn dd_default_path_used_when_end_equals_start() {
        // Mirror of reader-side
        // `line_geometry_uses_start_coordinate_default_for_bit_double_prefix_zero`:
        // A LINE with end == start should emit the DD `00` prefix
        // (no payload), making the encoded LINE 16+ bits shorter
        // than the explicit case.
        let geom = LineGeometry {
            start: [3.5, -2.0, 0.0],
            end: [3.5, -2.0, 0.0],
            thickness: 0.0,
            extrusion: [0.0, 0.0, 1.0],
        };
        let mut writer = BitWriter::new();
        write_line_geometry(geom, &mut writer).expect("writes");
        let total_bits = writer.position_in_bits();
        // Layout when end == start (z_are_zero compact path):
        //   1 (z_are_zero) + 64 (sx) + 2 (DD prefix 00) +
        //   64 (sy) + 2 (DD prefix 00) +
        //   1 (thickness=0 compact) + 1 (extrusion default) = 135
        assert_eq!(total_bits, 135);
        assert_eq!(round_trip(geom), geom);
    }

    #[test]
    fn round_trips_nontrivial_thickness_and_extrusion() {
        // Mirror of reader-side
        // `line_geometry_decodes_nontrivial_thickness_and_extrusion`.
        let geom = LineGeometry {
            start: [0.0, 0.0, 0.0],
            end: [0.0, 0.0, 0.0],
            thickness: 1.0,
            extrusion: [1.0, 1.0, 1.0],
        };
        assert_eq!(round_trip(geom), geom);
    }

    #[test]
    fn negative_zero_z_collapses_to_compact_path() {
        // IEEE 754: -0.0 == +0.0 returns true, so a LINE with -0.0
        // z components takes the compact path. The reader recovers
        // z as +0.0 (the encoded path doesn't carry sign for the
        // omitted z fields). Lock this as documented behaviour.
        let geom = LineGeometry {
            start: [1.0, 2.0, -0.0],
            end: [3.0, 4.0, -0.0],
            thickness: 0.0,
            extrusion: [0.0, 0.0, 1.0],
        };
        let recovered = round_trip(geom);
        assert_eq!(recovered.start[0], geom.start[0]);
        assert_eq!(recovered.start[1], geom.start[1]);
        assert_eq!(recovered.start[2], 0.0); // bit pattern may flip from -0.0 to +0.0
        assert_eq!(recovered.end[0], geom.end[0]);
        assert_eq!(recovered.end[1], geom.end[1]);
        assert_eq!(recovered.end[2], 0.0);
        assert_eq!(recovered.thickness, geom.thickness);
        assert_eq!(recovered.extrusion, geom.extrusion);
    }
}
