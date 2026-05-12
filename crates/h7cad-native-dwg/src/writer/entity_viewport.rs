//! AC1015 VIEWPORT entity body writer (F5.M5.E21).
//!
//! Inverse of [`crate::read_viewport_geometry`]. The current reader
//! only extracts center / width / height from the full VIEWPORT
//! payload (view direction, twist, lens length, frozen layers, ...
//! are all skipped). The writer mirrors that minimal shape:
//!
//! ```text
//!   3BD  center
//!   BD   width
//!   BD   height
//! ```
//!
//! **Known limitation**: callers building docs from scratch will
//! produce VIEWPORT objects with no view direction / twist / frozen
//! layers / etc. AutoCAD and other CAD apps reading these files may
//! see a partial viewport that is structurally valid but lacking
//! standard viewport metadata. Expanding to the full spec is tracked
//! at the entity-recovery level — the writer can only emit what the
//! model carries.

use crate::bit_writer::BitWriter;
use crate::entity_viewport::ViewportGeometry;
use crate::DwgWriteError;

/// Write the AC1015 VIEWPORT-specific payload for `geom` into `writer`.
pub fn write_viewport_geometry(
    geom: ViewportGeometry,
    writer: &mut BitWriter,
) -> Result<(), DwgWriteError> {
    writer.write_3bit_double(geom.center)?;
    writer.write_bit_double(geom.width)?;
    writer.write_bit_double(geom.height)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read_viewport_geometry, BitReader, BitWriter};

    fn round_trip(geom: ViewportGeometry) -> ViewportGeometry {
        let mut writer = BitWriter::new();
        write_viewport_geometry(geom, &mut writer).expect("writes");
        let bytes = writer.into_bytes();
        let mut reader = BitReader::new(&bytes);
        read_viewport_geometry(&mut reader).expect("reader recovers")
    }

    #[test]
    fn viewport_round_trips_origin_unit_size() {
        let geom = ViewportGeometry {
            center: [0.0, 0.0, 0.0],
            width: 1.0,
            height: 1.0,
        };
        assert_eq!(round_trip(geom), geom);
    }

    #[test]
    fn viewport_round_trips_offset_rectangle() {
        let geom = ViewportGeometry {
            center: [42.0, -17.5, 3.25],
            width: 100.0,
            height: 50.0,
        };
        assert_eq!(round_trip(geom), geom);
    }

    #[test]
    fn viewport_round_trips_zero_dimensions() {
        // Degenerate viewport: zero width/height. The reader/writer
        // pair must still round-trip without complaint.
        let geom = ViewportGeometry {
            center: [10.0, 20.0, 0.0],
            width: 0.0,
            height: 0.0,
        };
        assert_eq!(round_trip(geom), geom);
    }
}
