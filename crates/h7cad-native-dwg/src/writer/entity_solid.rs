//! AC1015 SOLID entity body writer (F5.M5.E7).
//!
//! Inverse of [`crate::read_solid_geometry`]. SOLID encodes the 4
//! corners as XY pairs sharing a single BD elevation:
//!
//! ```text
//!   BT   thickness
//!   BD   elevation
//!   2RD  corner1 (x,y)
//!   2RD  corner2 (x,y)
//!   2RD  corner3 (x,y)
//!   2RD  corner4 (x,y)
//!   BE   extrusion
//! ```
//!
//! The shared-elevation invariant is a wire-format hard requirement:
//! the reader will assign `corner_i[2] = elevation` for every i, so a
//! caller-supplied `SolidGeometry` with mismatched z values cannot be
//! round-tripped. Such input is treated as a caller bug and surfaced
//! as [`DwgWriteError::InvalidValue`] rather than silently coerced.

use crate::bit_writer::BitWriter;
use crate::entity_solid::SolidGeometry;
use crate::DwgWriteError;

/// Write the AC1015 SOLID-specific payload for `geom` into `writer`.
///
/// Returns [`DwgWriteError::InvalidValue`] if the four corners do not
/// share a single z component.
pub fn write_solid_geometry(
    geom: SolidGeometry,
    writer: &mut BitWriter,
) -> Result<(), DwgWriteError> {
    let elevation = geom.corners[0][2];
    for c in &geom.corners[1..] {
        if c[2] != elevation {
            return Err(DwgWriteError::InvalidValue(format!(
                "SOLID corners must share elevation; got z={elevation} vs z={}",
                c[2]
            )));
        }
    }
    writer.write_bit_thickness_r2000_plus(geom.thickness)?;
    writer.write_bit_double(elevation)?;
    for c in &geom.corners {
        writer.write_raw_f64_le(c[0])?;
        writer.write_raw_f64_le(c[1])?;
    }
    writer.write_bit_extrusion_r2000_plus(geom.extrusion)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read_solid_geometry, BitReader, BitWriter, DwgWriteError};

    fn round_trip(geom: SolidGeometry) -> SolidGeometry {
        let mut writer = BitWriter::new();
        write_solid_geometry(geom, &mut writer).expect("writes");
        let bytes = writer.into_bytes();
        let mut reader = BitReader::new(&bytes);
        read_solid_geometry(&mut reader).expect("reader recovers")
    }

    #[test]
    fn solid_geometry_round_trips_unit_quad() {
        let geom = SolidGeometry {
            corners: [
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            thickness: 0.0,
            extrusion: [0.0, 0.0, 1.0],
        };
        assert_eq!(round_trip(geom), geom);
    }

    #[test]
    fn solid_geometry_round_trips_translated_nonzero_elevation() {
        let geom = SolidGeometry {
            corners: [
                [10.0, 20.0, 5.0],
                [11.0, 20.0, 5.0],
                [11.0, 21.0, 5.0],
                [10.0, 21.0, 5.0],
            ],
            thickness: 1.0,
            extrusion: [1.0, 0.0, 0.0],
        };
        assert_eq!(round_trip(geom), geom);
    }

    #[test]
    fn solid_geometry_rejects_mismatched_corner_elevations() {
        let geom = SolidGeometry {
            corners: [
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 1.0],
            ],
            thickness: 0.0,
            extrusion: [0.0, 0.0, 1.0],
        };
        let mut writer = BitWriter::new();
        let err = write_solid_geometry(geom, &mut writer)
            .expect_err("mismatched elevation must be rejected");
        match err {
            DwgWriteError::InvalidValue(msg) => {
                assert!(
                    msg.contains("z=0") && msg.contains("z=1"),
                    "error must surface both z values; got `{msg}`"
                );
                assert!(
                    msg.contains("SOLID"),
                    "error must name the offending entity type; got `{msg}`"
                );
            }
            other => panic!("expected InvalidValue, got {other:?}"),
        }
    }
}
