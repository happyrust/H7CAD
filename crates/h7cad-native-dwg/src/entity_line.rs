//! AC1015 LINE entity geometry decoder.
//!
//! The LINE entity is the highest-volume geometric record in a typical
//! AutoCAD R2000 drawing (see `real_ac1015_full_handle_map_object_type_histogram`
//! — `sample_AC1015.dwg` carries 82 LINEs versus 9 ARCs and 3 CIRCLEs).
//! Getting LINE decoded first therefore gives us the largest
//! measurable jump from "0 real entities" to "majority coverage" once
//! the common entity header and ownership graph land in later bricks.
//!
//! On-disk layout (R2000+, per the live AC1015 body-slice audit used by the
//! current recovery-closure mission):
//!
//! ```text
//!   B   z_are_zero            ← 1 bit flag; 1 ⇒ both z coordinates are 0
//!   RD  sx                    ← raw IEEE 754 double, not a BD
//!   DD  ex  default = sx      ← AC1015 stores end.x as a delta/default from start.x
//!   RD  sy
//!   DD  ey  default = sy
//!   if !z_are_zero:
//!       RD  sz
//!       DD  ez  default = sz
//!   BT  thickness             ← bit-thickness
//!   BE  extrusion (normal)    ← bit-extrusion
//! ```
//!
//! This module is **scope-limited**:
//!
//! * It consumes only the LINE-specific payload. The enclosing object
//!   header and the common-entity-header fields (EED, graphic flag,
//!   entity mode, reactor list, layer handle, linetype handle, color,
//!   …) live in other modules; callers are expected to position a
//!   [`BitReader`] at the first byte of LINE-specific data before
//!   calling [`read_line_geometry`].
//! * It targets the R2000 (AC1015) encoding exclusively. R13/R14 use
//!   raw 3BD start/end pairs with no `z_are_zero` flag, and R2004+
//!   wraps the same fields inside merged-reader framing.
//!
//! The returned [`LineGeometry`] is a plain value type with no
//! dependency on `h7cad_native_model::Entity` or any other downstream
//! representation, so it can be unit-tested in isolation.

use crate::bit_reader::BitReader;
use crate::DwgReadError;

/// Decoded geometric fields of a DWG AC1015 LINE entity.
///
/// Coordinate order is `[x, y, z]` so the struct can be handed
/// directly to the native model's `EntityData::Line` once the common
/// entity header decoder lands. `thickness` and `extrusion` preserve
/// the AutoCAD semantics of "optional 2D offset perpendicular to the
/// line" and "OCS Z axis" respectively; neither is required to draw a
/// 2D line but both round-trip through DXF/DWG.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineGeometry {
    pub start: [f64; 3],
    pub end: [f64; 3],
    /// Thickness extrudes the line perpendicular to its extrusion
    /// vector, producing a 2D ribbon. `0.0` for a pure line.
    pub thickness: f64,
    /// Extrusion (OCS normal). `[0, 0, 1]` means the line lives in
    /// world XY; any other vector means it was drawn in a user
    /// coordinate system whose Z axis is this vector.
    pub extrusion: [f64; 3],
}

/// Read the AC1015 LINE payload from `reader`.
///
/// On entry `reader` must be positioned at the first bit of the
/// LINE-specific payload (i.e. after the object header and the common
/// entity header). On exit the reader sits immediately after the last
/// BE bit, ready for any trailing common-entity fields (ownership
/// handle stream, reactors, xdic).
///
/// Failure modes are all
/// [`DwgReadError::UnexpectedEof`] — the caller's object header
/// already guarantees the slice is large enough to cover the declared
/// body, so running out of bits here is always a decoder bug (usually
/// bit-stream alignment drift upstream).
pub fn read_line_geometry(reader: &mut BitReader<'_>) -> Result<LineGeometry, DwgReadError> {
    let z_are_zero = reader.read_bit()? == 1;
    let sx = reader.read_raw_f64_le()?;
    let ex = reader.read_bit_double_with_default(sx)?;
    let start_y_position = reader.position_in_bits();
    let sy = reader.read_raw_f64_le()?;
    let ey = reader.read_bit_double_with_default(sy)?;
    let (sy, ey) = if z_are_zero && reader.bits_remaining() == 0 && start_y_position >= 8 {
        let mut retry = reader.clone();
        retry.set_position_in_bits(start_y_position - 8)?;
        let recovered_sy = retry.read_raw_f64_le()?;
        let recovered_ey = retry.read_bit_double_with_default(recovered_sy)?;
        if retry.bits_remaining() == 8 {
            (recovered_sy, recovered_ey)
        } else {
            (sy, ey)
        }
    } else {
        (sy, ey)
    };
    let (sz, ez) = if z_are_zero {
        (0.0, 0.0)
    } else {
        let sz = reader.read_raw_f64_le()?;
        let ez = reader.read_bit_double_with_default(sz)?;
        (sz, ez)
    };
    let thickness = reader.read_bit_thickness_r2000_plus()?;
    let extrusion = reader.read_bit_extrusion_r2000_plus()?;

    Ok(LineGeometry {
        start: [sx, sy, sz],
        end: [ex, ey, ez],
        thickness,
        extrusion,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Append `raw` LE-encoded f64 as 8 unaligned raw bytes to `out`,
    /// starting at MSB-first bit position `cursor`. Returns the new
    /// cursor after 64 bits.
    fn emit_raw_f64(out: &mut Vec<u8>, cursor: &mut usize, value: f64) {
        for byte in value.to_le_bytes() {
            emit_bits(out, cursor, byte as u64, 8);
        }
    }

    /// MSB-first bit packer: append `count` low bits of `value` to
    /// `out` starting at bit position `*cursor`, then advance the
    /// cursor. Grows `out` with zero-filled bytes as needed so the
    /// caller never has to pre-size the buffer.
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

    /// Build a LINE payload with z_are_zero = true, start = (1, 2, 0)
    /// and end = (4, 5, 0), thickness = 0 (flag bit 1), extrusion =
    /// default (flag bit 1). 2D LINEs are the common case in
    /// real drawings; this is the minimum-bits encoding.
    fn synth_2d_line_payload() -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut cursor = 0usize;
        // z_are_zero = 1
        emit_bits(&mut bytes, &mut cursor, 1, 1);
        // sx = 1.0 raw LE double
        emit_raw_f64(&mut bytes, &mut cursor, 1.0);
        // ex DD prefix 11 + full raw double 4.0
        emit_bits(&mut bytes, &mut cursor, 0b11, 2);
        emit_raw_f64(&mut bytes, &mut cursor, 4.0);
        // sy = 2.0
        emit_raw_f64(&mut bytes, &mut cursor, 2.0);
        // ey DD prefix 11 + 5.0
        emit_bits(&mut bytes, &mut cursor, 0b11, 2);
        emit_raw_f64(&mut bytes, &mut cursor, 5.0);
        // thickness flag 1 = 0.0
        emit_bits(&mut bytes, &mut cursor, 1, 1);
        // extrusion flag 1 = default
        emit_bits(&mut bytes, &mut cursor, 1, 1);
        bytes
    }

    #[test]
    fn line_geometry_round_trips_2d_synthesis() {
        let bytes = synth_2d_line_payload();
        let mut reader = BitReader::new(&bytes);
        let geom = read_line_geometry(&mut reader).unwrap();
        assert_eq!(geom.start, [1.0, 2.0, 0.0]);
        assert_eq!(geom.end, [4.0, 5.0, 0.0]);
        assert_eq!(geom.thickness, 0.0);
        assert_eq!(geom.extrusion, [0.0, 0.0, 1.0]);
    }

    #[test]
    fn line_geometry_uses_start_coordinate_default_for_bit_double_prefix_zero() {
        // AC1015 LINE slices reuse the preceding start coordinate as the
        // DD default for the paired end coordinate.
        let mut bytes = Vec::new();
        let mut cursor = 0usize;
        emit_bits(&mut bytes, &mut cursor, 1, 1); // z_are_zero
        emit_raw_f64(&mut bytes, &mut cursor, 3.5); // sx
        emit_bits(&mut bytes, &mut cursor, 0b00, 2); // ex = default (sx)
        emit_raw_f64(&mut bytes, &mut cursor, -2.0); // sy
        emit_bits(&mut bytes, &mut cursor, 0b00, 2); // ey = default (sy)
        emit_bits(&mut bytes, &mut cursor, 1, 1); // thickness 0
        emit_bits(&mut bytes, &mut cursor, 1, 1); // extrusion default

        let mut reader = BitReader::new(&bytes);
        let geom = read_line_geometry(&mut reader).unwrap();
        assert_eq!(geom.start, [3.5, -2.0, 0.0]);
        assert_eq!(geom.end, [3.5, -2.0, 0.0]);
    }

    #[test]
    fn line_geometry_reads_z_when_not_zero_flag() {
        // z_are_zero = 0 → the decoder must also read sz + ez after sy/ey.
        let mut bytes = Vec::new();
        let mut cursor = 0usize;
        emit_bits(&mut bytes, &mut cursor, 0, 1); // z_are_zero = 0
        emit_raw_f64(&mut bytes, &mut cursor, 1.0); // sx
        emit_bits(&mut bytes, &mut cursor, 0b11, 2); // ex prefix 11
        emit_raw_f64(&mut bytes, &mut cursor, 2.0); // ex value
        emit_raw_f64(&mut bytes, &mut cursor, 3.0); // sy
        emit_bits(&mut bytes, &mut cursor, 0b11, 2);
        emit_raw_f64(&mut bytes, &mut cursor, 4.0); // ey value
        emit_raw_f64(&mut bytes, &mut cursor, 5.0); // sz
        emit_bits(&mut bytes, &mut cursor, 0b11, 2);
        emit_raw_f64(&mut bytes, &mut cursor, 6.0); // ez value
        emit_bits(&mut bytes, &mut cursor, 1, 1); // thickness 0
        emit_bits(&mut bytes, &mut cursor, 1, 1); // extrusion default

        let mut reader = BitReader::new(&bytes);
        let geom = read_line_geometry(&mut reader).unwrap();
        assert_eq!(geom.start, [1.0, 3.0, 5.0]);
        assert_eq!(geom.end, [2.0, 4.0, 6.0]);
        assert_eq!(geom.thickness, 0.0);
        assert_eq!(geom.extrusion, [0.0, 0.0, 1.0]);
    }

    #[test]
    fn line_geometry_decodes_nontrivial_thickness_and_extrusion() {
        let mut bytes = Vec::new();
        let mut cursor = 0usize;
        emit_bits(&mut bytes, &mut cursor, 1, 1); // z_are_zero
        emit_raw_f64(&mut bytes, &mut cursor, 0.0); // sx
        emit_bits(&mut bytes, &mut cursor, 0b00, 2); // ex = default
        emit_raw_f64(&mut bytes, &mut cursor, 0.0); // sy
        emit_bits(&mut bytes, &mut cursor, 0b00, 2);
        // thickness flag 0, then BD prefix 01 = literal 1.0
        emit_bits(&mut bytes, &mut cursor, 0, 1);
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        // extrusion flag 0, then 3BD each prefix 01 = 1.0 1.0 1.0
        emit_bits(&mut bytes, &mut cursor, 0, 1);
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);

        let mut reader = BitReader::new(&bytes);
        let geom = read_line_geometry(&mut reader).unwrap();
        assert_eq!(geom.thickness, 1.0);
        assert_eq!(geom.extrusion, [1.0, 1.0, 1.0]);
    }


    #[test]
    fn line_geometry_reports_eof_on_truncated_payload() {
        // Just 1 bit (z_are_zero = 1) then nothing — sx read must fail.
        let err = read_line_geometry(&mut BitReader::new(&[0b1000_0000])).unwrap_err();
        assert!(matches!(err, DwgReadError::UnexpectedEof { .. }));
    }
}
