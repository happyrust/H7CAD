//! AC1015 RAY / XLINE entity body writer (F5.M5.E8).
//!
//! Inverse of [`crate::read_ray_geometry`]. RAY (object_type = 38)
//! and XLINE (object_type = 40) share the same body layout, so a
//! single writer covers both — the entity type only differs at the
//! object_slice header dispatched by `writer/document.rs`.
//!
//! ```text
//!   3BD  origin
//!   3BD  direction
//! ```

use crate::bit_writer::BitWriter;
use crate::entity_ray::RayGeometry;
use crate::DwgWriteError;

/// Write the AC1015 RAY/XLINE-specific payload for `geom` into `writer`.
pub fn write_ray_geometry(geom: RayGeometry, writer: &mut BitWriter) -> Result<(), DwgWriteError> {
    writer.write_3bit_double(geom.origin)?;
    writer.write_3bit_double(geom.direction)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read_ray_geometry, BitReader, BitWriter};

    fn round_trip(geom: RayGeometry) -> RayGeometry {
        let mut writer = BitWriter::new();
        write_ray_geometry(geom, &mut writer).expect("writes");
        let bytes = writer.into_bytes();
        let mut reader = BitReader::new(&bytes);
        read_ray_geometry(&mut reader).expect("reader recovers")
    }

    #[test]
    fn ray_geometry_round_trips_canonical_axis() {
        let geom = RayGeometry {
            origin: [0.0, 0.0, 0.0],
            direction: [1.0, 0.0, 0.0],
        };
        assert_eq!(round_trip(geom), geom);
    }

    #[test]
    fn ray_geometry_round_trips_offset_origin_diagonal_direction() {
        let geom = RayGeometry {
            origin: [3.5, -2.25, 7.0],
            direction: [1.0, 1.0, 1.0],
        };
        assert_eq!(round_trip(geom), geom);
    }

    #[test]
    fn ray_geometry_round_trips_negative_direction() {
        let geom = RayGeometry {
            origin: [0.0, 0.0, 0.0],
            direction: [-1.0, -2.0, -3.0],
        };
        assert_eq!(round_trip(geom), geom);
    }
}
