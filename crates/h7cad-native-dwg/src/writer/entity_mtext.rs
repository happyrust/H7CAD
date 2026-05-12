//! AC1015 MTEXT entity body writer (F5.M5.E11).
//!
//! Inverse of [`crate::read_mtext_geometry`]:
//!
//! ```text
//!   3BD  insertion
//!   3BD  extrusion
//!   3BD  x_direction
//!   BD   rect_width
//!   BD   rect_height
//!   BD   text_height
//!   BS   attachment_point
//!   BS   drawing_direction
//!   BD   ext_height          (reader-discarded)
//!   BD   ext_width           (reader-discarded)
//!   T    value
//!   BS   line_spacing_style  (reader-discarded)
//!   BD   line_spacing_factor
//!   B    unknown_bit         (reader-discarded)
//!   (handle stream) hard-owner style_handle
//! ```
//!
//! Field choices made by this writer (none round-trip-breaking, but
//! they pin behaviour the reader will observe):
//!
//! - **`ext_height` / `ext_width` / `line_spacing_style` / `unknown_bit`**:
//!   the reader discards these (`_`-prefixed locals in
//!   `read_mtext_geometry`), so the writer always emits `0.0` / `0`.
//!   They cannot influence round-trip equivalence.
//! - **`x_direction` vs `rotation`**: `EntityData::MText` stores only
//!   `rotation` (f64). The reader derives it from `x_direction` via
//!   `atan2(x[1], x[0])`. The writer reverses this:
//!   `x_direction = [cos(rotation), sin(rotation), 0.0]`. Note that
//!   the `cos/sin → atan2` chain is not bit-exact in f64, so callers
//!   that need lossless rotation round-trip must compare with an
//!   ε-tolerance (≤ 1e-12 for ordinary inputs).

use crate::bit_writer::BitWriter;
use crate::entity_mtext::MTextGeometry;
use crate::object_header::HANDLE_CODE_HARD_OWNER;
use crate::DwgWriteError;

/// Write the AC1015 MTEXT-specific payload for `geom`.
pub fn write_mtext_geometry(
    geom: &MTextGeometry,
    main_writer: &mut BitWriter,
    handle_writer: &mut BitWriter,
) -> Result<(), DwgWriteError> {
    main_writer.write_3bit_double(geom.insertion)?;
    main_writer.write_3bit_double(geom.extrusion)?;
    main_writer.write_3bit_double(geom.x_direction)?;
    main_writer.write_bit_double(geom.rect_width)?;
    main_writer.write_bit_double(geom.rect_height)?;
    main_writer.write_bit_double(geom.height)?;
    main_writer.write_bit_short(geom.attachment_point)?;
    main_writer.write_bit_short(geom.drawing_direction)?;
    main_writer.write_bit_double(0.0)?;
    main_writer.write_bit_double(0.0)?;
    main_writer.write_text_ascii(&geom.value)?;
    main_writer.write_bit_short(0)?;
    main_writer.write_bit_double(geom.line_spacing_factor)?;
    main_writer.write_bit(0)?;

    handle_writer.write_handle(HANDLE_CODE_HARD_OWNER, geom.style_handle.value())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read_mtext_geometry, BitReader, BitWriter};
    use h7cad_native_model::Handle;

    fn round_trip(geom: MTextGeometry, object_handle: Handle) -> MTextGeometry {
        let mut main = BitWriter::new();
        let mut handle = BitWriter::new();
        write_mtext_geometry(&geom, &mut main, &mut handle).expect("writes");
        let main_bytes = main.into_bytes();
        let handle_bytes = handle.into_bytes();
        let mut main_reader = BitReader::new(&main_bytes);
        let mut handle_reader = BitReader::new(&handle_bytes);
        read_mtext_geometry(&mut main_reader, &mut handle_reader, object_handle)
            .expect("reader recovers")
    }

    fn approx_eq(left: f64, right: f64, label: &str) {
        let diff = (left - right).abs();
        assert!(
            diff < 1e-12,
            "{label}: {left} vs {right} (Δ {diff} exceeds 1e-12)"
        );
    }

    #[test]
    fn round_trips_canonical_axis_mtext() {
        // Canonical x_direction (1,0,0) → rotation = 0; round-trips
        // exactly with no f64 approximation.
        let geom = MTextGeometry {
            insertion: [1.0, 2.0, 3.0],
            extrusion: [0.0, 0.0, 1.0],
            x_direction: [1.0, 0.0, 0.0],
            rect_width: 100.0,
            rect_height: 0.0,
            height: 2.5,
            attachment_point: 1,
            drawing_direction: 5,
            value: "hello".to_string(),
            line_spacing_factor: 1.0,
            style_handle: Handle::new(0x21),
            rotation: 0.0,
        };
        let got = round_trip(geom.clone(), Handle::new(0x10));
        assert_eq!(got.insertion, geom.insertion);
        assert_eq!(got.extrusion, geom.extrusion);
        assert_eq!(got.x_direction, geom.x_direction);
        assert_eq!(got.rect_width, geom.rect_width);
        assert_eq!(got.rect_height, geom.rect_height);
        assert_eq!(got.height, geom.height);
        assert_eq!(got.attachment_point, geom.attachment_point);
        assert_eq!(got.drawing_direction, geom.drawing_direction);
        assert_eq!(got.value, geom.value);
        assert_eq!(got.line_spacing_factor, geom.line_spacing_factor);
        assert_eq!(got.style_handle, geom.style_handle);
        assert_eq!(got.rotation, 0.0);
    }

    #[test]
    fn round_trips_rotated_mtext_within_epsilon() {
        // Non-canonical x_direction: writer emits exactly; reader's
        // `atan2(x[1], x[0])` then derives a rotation that is close
        // (but not bit-equal) to the original `atan2`. The
        // x_direction triple itself round-trips exactly because the
        // wire format is 3 × BD with no transcendental in the path.
        let dir = [3.0_f64.cos(), 3.0_f64.sin(), 0.0];
        let geom = MTextGeometry {
            insertion: [-1.0, 4.0, 0.0],
            extrusion: [0.0, 0.0, 1.0],
            x_direction: dir,
            rect_width: 50.0,
            rect_height: 12.0,
            height: 3.5,
            attachment_point: 2,
            drawing_direction: 1,
            value: "rotated mtext".to_string(),
            line_spacing_factor: 1.5,
            style_handle: Handle::new(0x22),
            rotation: dir[1].atan2(dir[0]),
        };
        let got = round_trip(geom.clone(), Handle::new(0x20));
        assert_eq!(got.x_direction, geom.x_direction);
        assert_eq!(got.rect_height, geom.rect_height);
        assert_eq!(got.attachment_point, geom.attachment_point);
        assert_eq!(got.value, geom.value);
        approx_eq(got.rotation, geom.rotation, "rotation");
    }

    #[test]
    fn round_trips_empty_value_and_zero_rect_height() {
        // Stress the BS-length=0 ASCII path and reader's
        // `rect_height > 0.0 ? Some : None` semantic.
        let geom = MTextGeometry {
            insertion: [0.0, 0.0, 0.0],
            extrusion: [0.0, 0.0, 1.0],
            x_direction: [1.0, 0.0, 0.0],
            rect_width: 1.0,
            rect_height: 0.0,
            height: 1.0,
            attachment_point: 0,
            drawing_direction: 0,
            value: String::new(),
            line_spacing_factor: 1.0,
            style_handle: Handle::new(0x23),
            rotation: 0.0,
        };
        let got = round_trip(geom.clone(), Handle::new(0x30));
        assert_eq!(got.value, "");
        assert_eq!(got.rect_height, 0.0);
        assert_eq!(got.attachment_point, 0);
        assert_eq!(got.line_spacing_factor, 1.0);
    }
}
