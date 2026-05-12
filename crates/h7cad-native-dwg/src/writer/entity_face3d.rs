//! AC1015 3DFACE entity body writer (F5.M5.E7).
//!
//! Inverse of [`crate::read_face3d_geometry`]:
//!
//! ```text
//!   B    has_no_flags
//!   BS   invisible_edges  (only if has_no_flags == 0)
//!   3BD  corner1
//!   3BD  corner2
//!   3BD  corner3
//!   3BD  corner4
//! ```
//!
//! This writer always emits `has_no_flags = 0` so the BS
//! `invisible_edges` value round-trips through the reader regardless
//! of whether it is zero. The reader's alternative branch
//! (`has_no_flags == 1`) is a wire-format shortcut some samples use
//! to omit a zero `invisible_edges`; emitting it would couple the
//! writer to a "are all edges visible?" predicate and obscure intent.
//! M5.E7 keeps the path predictable and uniform with the rest of
//! M5.E1…E6.

use crate::bit_writer::BitWriter;
use crate::entity_solid::Face3DGeometry;
use crate::DwgWriteError;

/// Write the AC1015 3DFACE-specific payload for `geom` into `writer`.
pub fn write_face3d_geometry(
    geom: Face3DGeometry,
    writer: &mut BitWriter,
) -> Result<(), DwgWriteError> {
    writer.write_bit(0)?;
    writer.write_bit_short(geom.invisible_edges)?;
    for c in &geom.corners {
        writer.write_3bit_double(*c)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read_face3d_geometry, BitReader, BitWriter};

    fn round_trip(geom: Face3DGeometry) -> Face3DGeometry {
        let mut writer = BitWriter::new();
        write_face3d_geometry(geom, &mut writer).expect("writes");
        let bytes = writer.into_bytes();
        let mut reader = BitReader::new(&bytes);
        read_face3d_geometry(&mut reader).expect("reader recovers")
    }

    #[test]
    fn face3d_geometry_round_trips_planar_quad() {
        let geom = Face3DGeometry {
            corners: [
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            invisible_edges: 0,
        };
        assert_eq!(round_trip(geom), geom);
    }

    #[test]
    fn face3d_geometry_round_trips_nonplanar_with_invisible_edges() {
        let geom = Face3DGeometry {
            corners: [
                [0.0, 0.0, 0.0],
                [2.0, 0.0, 1.0],
                [2.0, 2.0, 2.0],
                [0.0, 2.0, 1.0],
            ],
            // bits set for edges 1 and 3 (0-indexed); reader returns the
            // raw i16 so a non-zero mask is the clearest signal that
            // `has_no_flags = 0` was respected on write.
            invisible_edges: 0b1010,
        };
        assert_eq!(round_trip(geom), geom);
    }

    #[test]
    fn face3d_geometry_round_trips_zero_invisible_edges() {
        let geom = Face3DGeometry {
            corners: [
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            invisible_edges: 0,
        };
        let mut writer = BitWriter::new();
        write_face3d_geometry(geom, &mut writer).expect("writes");
        // Writer must always pay the 1-bit `has_no_flags=0` + BS overhead,
        // even when `invisible_edges == 0`. If the writer ever shortcuts
        // to `has_no_flags=1` to save those bits the bit count would
        // drop below this floor. We do not pin an upper bound — the BS
        // encoding of a small i16 is variable-width — but the lower
        // bound proves the shortcut path is not silently taken.
        // 1 (has_no_flags) + 2 (BS short tag for 0) + 4 * 6 (3BD zeros) = 27 bits min.
        assert!(
            writer.position_in_bits() >= 27,
            "writer must always emit the has_no_flags=0 + BS path; got {} bits",
            writer.position_in_bits()
        );
        assert_eq!(round_trip(geom), geom);
    }
}
