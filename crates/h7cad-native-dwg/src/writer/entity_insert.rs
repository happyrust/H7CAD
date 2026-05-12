//! AC1015 INSERT entity body writer (F5.M5.E12).
//!
//! Inverse of [`crate::read_insert_geometry`]:
//!
//! ```text
//!   3BD  insertion
//!   2B   scale_flag      (compact / unit / single / DD)
//!   ?    scale_x / scale_y / scale_z  (varies by scale_flag)
//!   BD   rotation
//!   3BD  extrusion
//!   B    has_attribs
//!   (handle stream) H block_header  (hard-owner)
//! ```
//!
//! Scale flag selection (matches the reader's 4 branches):
//!
//! | flag | semantics | writer condition |
//! |------|-----------|------------------|
//! | `01` | unit scale `(1,1,1)`           | `scale == [1.0, 1.0, 1.0]` |
//! | `10` | single x → broadcast to y/z    | `scale[0] == scale[1] == scale[2]` and not unit |
//! | `00` | RD x, DD y default x, DD z default x | otherwise |
//! | `11` | reader catch-all, same as `00` | writer never emits |
//!
//! has_attribs known limitation: the reader's `read_insert_geometry`
//! only pulls `block_header_handle` from the handle stream; it
//! never reads `first_attrib / last_attrib / seqend` even when
//! `has_attribs` is set. The model's `EntityData::Insert.attribs`
//! is always empty after a fresh read. Writer therefore forces
//! `has_attribs = false` to keep the handle stream contract tight
//! — supporting `has_attribs = true` would require also serialising
//! ATTRIB chain handles which is outside M5.E12's scope.

use crate::bit_writer::BitWriter;
use crate::entity_insert::InsertGeometry;
use crate::object_header::HANDLE_CODE_HARD_OWNER;
use crate::DwgWriteError;

const SCALE_FLAG_DD: u8 = 0b00;
const SCALE_FLAG_UNIT: u8 = 0b01;
const SCALE_FLAG_SINGLE: u8 = 0b10;

/// Write the AC1015 INSERT-specific payload for `geom`.
pub fn write_insert_geometry(
    geom: &InsertGeometry,
    main_writer: &mut BitWriter,
    handle_writer: &mut BitWriter,
) -> Result<(), DwgWriteError> {
    main_writer.write_3bit_double(geom.insertion)?;

    let [sx, sy, sz] = geom.scale;
    if sx == 1.0 && sy == 1.0 && sz == 1.0 {
        main_writer.write_bits(SCALE_FLAG_UNIT as u64, 2)?;
    } else if sx == sy && sx == sz {
        main_writer.write_bits(SCALE_FLAG_SINGLE as u64, 2)?;
        main_writer.write_raw_f64_le(sx)?;
    } else {
        main_writer.write_bits(SCALE_FLAG_DD as u64, 2)?;
        main_writer.write_raw_f64_le(sx)?;
        main_writer.write_bit_double_with_default(sy, sx)?;
        main_writer.write_bit_double_with_default(sz, sx)?;
    }

    main_writer.write_bit_double(geom.rotation)?;
    main_writer.write_3bit_double(geom.extrusion)?;
    // M5.E12 known limitation: the model carries `has_attribs` as a
    // boolean but the reader's `read_insert_geometry` never reads
    // first/last/seqend handles, and `EntityData::Insert.attribs`
    // is always empty after a fresh read. Forcing `false` here
    // keeps the handle-stream contract tight; lossless attrib-chain
    // round-trip is out of scope for this milestone.
    main_writer.write_bit(0)?;

    handle_writer.write_handle(HANDLE_CODE_HARD_OWNER, geom.block_header_handle.value())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read_insert_geometry, BitReader, BitWriter};
    use h7cad_native_model::Handle;

    fn round_trip(geom: InsertGeometry, object_handle: Handle) -> InsertGeometry {
        let mut main = BitWriter::new();
        let mut handle = BitWriter::new();
        write_insert_geometry(&geom, &mut main, &mut handle).expect("writes");
        let main_bytes = main.into_bytes();
        let handle_bytes = handle.into_bytes();
        let mut main_reader = BitReader::new(&main_bytes);
        let mut handle_reader = BitReader::new(&handle_bytes);
        read_insert_geometry(&mut main_reader, &mut handle_reader, object_handle)
            .expect("reader recovers")
    }

    #[test]
    fn round_trips_unit_scale_insert() {
        // scale_flag = 01 path: no scale bytes emitted.
        let geom = InsertGeometry {
            insertion: [10.0, 20.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            rotation: 0.0,
            extrusion: [0.0, 0.0, 1.0],
            has_attribs: false,
            block_header_handle: Handle::new(0x33),
        };
        assert_eq!(round_trip(geom.clone(), Handle::new(0x10)), geom);
    }

    #[test]
    fn round_trips_single_scale_insert() {
        // scale_flag = 10 path: one f64 emitted then broadcast.
        let geom = InsertGeometry {
            insertion: [5.0, -5.0, 2.0],
            scale: [2.5, 2.5, 2.5],
            rotation: 1.25,
            extrusion: [0.0, 0.0, 1.0],
            has_attribs: false,
            block_header_handle: Handle::new(0x44),
        };
        assert_eq!(round_trip(geom.clone(), Handle::new(0x20)), geom);
    }

    #[test]
    fn round_trips_dd_scale_insert() {
        // scale_flag = 00 path: x raw + y DD-against-x + z DD-against-x.
        let geom = InsertGeometry {
            insertion: [0.0, 0.0, 0.0],
            scale: [2.0, 3.0, 4.0],
            rotation: 0.5,
            extrusion: [0.0, 0.0, 1.0],
            has_attribs: false,
            block_header_handle: Handle::new(0x55),
        };
        assert_eq!(round_trip(geom.clone(), Handle::new(0x30)), geom);
    }

    #[test]
    fn round_trips_dd_scale_with_x_y_equal() {
        // scale_flag = 00 path where y matches x (DD-default fires
        // for y) but z differs. Exercises the compact DD encoding
        // mid-row.
        let geom = InsertGeometry {
            insertion: [0.0, 0.0, 0.0],
            scale: [2.0, 2.0, 4.0],
            rotation: 0.0,
            extrusion: [0.0, 0.0, 1.0],
            has_attribs: false,
            block_header_handle: Handle::new(0x66),
        };
        assert_eq!(round_trip(geom.clone(), Handle::new(0x40)), geom);
    }

    #[test]
    fn writer_forces_has_attribs_false_known_limitation() {
        // M5.E12 known limitation: writer always emits has_attribs=0
        // regardless of caller-supplied value. The round-tripped
        // geom always reports has_attribs = false.
        let geom = InsertGeometry {
            insertion: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            rotation: 0.0,
            extrusion: [0.0, 0.0, 1.0],
            has_attribs: true, // caller claims true, writer forces false
            block_header_handle: Handle::new(0x77),
        };
        let got = round_trip(geom, Handle::new(0x50));
        assert!(!got.has_attribs);
        assert_eq!(got.block_header_handle, Handle::new(0x77));
    }
}
