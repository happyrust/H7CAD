//! AC1015 ATTRIB entity body writer (F5.M5.E6).
//!
//! ATTRIB is TEXT-like with trailing `tag`, `field_length`, and flags.
//! The native model currently keeps tag/value/insertion/height plus the
//! common entity thickness/extrusion, so non-modeled TEXT options are
//! emitted as DWG defaults.

use crate::bit_writer::BitWriter;
use crate::entity_attrib::AttribGeometry;
use crate::object_header::HANDLE_CODE_HARD_OWNER;
use crate::DwgWriteError;

const FLAG_ELEVATION_DEFAULT: u8 = 0x01;
const FLAG_ALIGNMENT_ABSENT: u8 = 0x02;
const FLAG_OBLIQUE_DEFAULT: u8 = 0x04;
const FLAG_ROTATION_DEFAULT: u8 = 0x08;
const FLAG_WIDTH_FACTOR_DEFAULT: u8 = 0x10;
const FLAG_GENERATION_DEFAULT: u8 = 0x20;
const FLAG_HORIZONTAL_DEFAULT: u8 = 0x40;
const FLAG_VERTICAL_DEFAULT: u8 = 0x80;

/// Write the AC1015 ATTRIB-specific payload for `geom`.
pub fn write_attrib_geometry(
    geom: AttribGeometry,
    main_writer: &mut BitWriter,
    handle_writer: &mut BitWriter,
) -> Result<(), DwgWriteError> {
    let elevation = geom.insertion[2];
    let has_elevation = elevation != 0.0;
    let has_rotation = geom.rotation != 0.0;

    let mut data_flags = FLAG_ALIGNMENT_ABSENT
        | FLAG_OBLIQUE_DEFAULT
        | FLAG_WIDTH_FACTOR_DEFAULT
        | FLAG_GENERATION_DEFAULT
        | FLAG_HORIZONTAL_DEFAULT
        | FLAG_VERTICAL_DEFAULT;
    if !has_elevation {
        data_flags |= FLAG_ELEVATION_DEFAULT;
    }
    if !has_rotation {
        data_flags |= FLAG_ROTATION_DEFAULT;
    }

    main_writer.write_raw_u8(data_flags)?;
    if has_elevation {
        main_writer.write_raw_f64_le(elevation)?;
    }
    main_writer.write_raw_f64_le(geom.insertion[0])?;
    main_writer.write_raw_f64_le(geom.insertion[1])?;
    main_writer.write_bit_extrusion_r2000_plus(geom.extrusion)?;
    main_writer.write_bit_thickness_r2000_plus(geom.thickness)?;
    if has_rotation {
        main_writer.write_raw_f64_le(geom.rotation)?;
    }
    main_writer.write_raw_f64_le(geom.height)?;
    main_writer.write_text_ascii(&geom.value)?;
    main_writer.write_text_ascii(&geom.tag)?;
    main_writer.write_bit_short(0)?;
    main_writer.write_raw_u8(0)?;

    handle_writer.write_handle(HANDLE_CODE_HARD_OWNER, geom.style_handle.value())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read_attrib_geometry, BitReader, BitWriter};
    use h7cad_native_model::Handle;

    fn round_trip(geom: AttribGeometry, object_handle: Handle) -> AttribGeometry {
        let mut main = BitWriter::new();
        let mut handle = BitWriter::new();
        write_attrib_geometry(geom, &mut main, &mut handle).expect("writes");
        let main_bytes = main.into_bytes();
        let handle_bytes = handle.into_bytes();
        let mut main_reader = BitReader::new(&main_bytes);
        let mut handle_reader = BitReader::new(&handle_bytes);
        read_attrib_geometry(&mut main_reader, &mut handle_reader, object_handle)
            .expect("reader recovers")
    }

    #[test]
    fn round_trips_minimal_attrib() {
        let geom = AttribGeometry {
            tag: "TAG".to_string(),
            value: "VALUE".to_string(),
            insertion: [1.0, 2.0, 0.0],
            height: 2.5,
            extrusion: [0.0, 0.0, 1.0],
            thickness: 0.0,
            rotation: 0.0,
            style_handle: Handle::new(0x21),
        };
        assert_eq!(round_trip(geom.clone(), Handle::new(0x10)), geom);
    }

    #[test]
    fn round_trips_nontrivial_attrib() {
        let geom = AttribGeometry {
            tag: "PIPE_ID".to_string(),
            value: "P-101".to_string(),
            insertion: [1.0, 2.0, 3.0],
            height: 3.5,
            extrusion: [1.0, 0.0, 0.0],
            thickness: 0.25,
            rotation: 0.5,
            style_handle: Handle::new(0x22),
        };
        assert_eq!(round_trip(geom.clone(), Handle::new(0x20)), geom);
    }
}
