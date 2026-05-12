//! AC1015 TEXT entity body writer (F5.M5.E5).
//!
//! Inverse of [`crate::read_text_geometry`]. Data flags are set when a
//! field can use the DWG default and cleared when the value is emitted.
//! The style handle lives in the handle stream.

use crate::bit_writer::BitWriter;
use crate::entity_text::TextGeometry;
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

/// Write the AC1015 TEXT-specific payload for `geom`.
pub fn write_text_geometry(
    geom: TextGeometry,
    main_writer: &mut BitWriter,
    handle_writer: &mut BitWriter,
) -> Result<(), DwgWriteError> {
    let elevation = geom.insertion[2];
    let has_elevation = elevation != 0.0;
    let has_alignment = geom.alignment_point.is_some();
    let has_oblique = geom.oblique_angle != 0.0;
    let has_rotation = geom.rotation != 0.0;
    let has_width_factor = geom.width_factor != 1.0;
    let has_horizontal_alignment = geom.horizontal_alignment != 0;
    let has_vertical_alignment = geom.vertical_alignment != 0;

    if let Some(alignment_point) = geom.alignment_point {
        if alignment_point[2] != elevation {
            return Err(DwgWriteError::InvalidValue(format!(
                "TEXT alignment point z ({}) must match insertion elevation ({})",
                alignment_point[2], elevation
            )));
        }
    }

    let mut data_flags = FLAG_GENERATION_DEFAULT;
    if !has_elevation {
        data_flags |= FLAG_ELEVATION_DEFAULT;
    }
    if !has_alignment {
        data_flags |= FLAG_ALIGNMENT_ABSENT;
    }
    if !has_oblique {
        data_flags |= FLAG_OBLIQUE_DEFAULT;
    }
    if !has_rotation {
        data_flags |= FLAG_ROTATION_DEFAULT;
    }
    if !has_width_factor {
        data_flags |= FLAG_WIDTH_FACTOR_DEFAULT;
    }
    if !has_horizontal_alignment {
        data_flags |= FLAG_HORIZONTAL_DEFAULT;
    }
    if !has_vertical_alignment {
        data_flags |= FLAG_VERTICAL_DEFAULT;
    }

    main_writer.write_raw_u8(data_flags)?;
    if has_elevation {
        main_writer.write_raw_f64_le(elevation)?;
    }
    main_writer.write_raw_f64_le(geom.insertion[0])?;
    main_writer.write_raw_f64_le(geom.insertion[1])?;
    if let Some(alignment_point) = geom.alignment_point {
        main_writer.write_bit_double_with_default(alignment_point[0], geom.insertion[0])?;
        main_writer.write_bit_double_with_default(alignment_point[1], geom.insertion[1])?;
    }
    main_writer.write_bit_extrusion_r2000_plus(geom.extrusion)?;
    main_writer.write_bit_thickness_r2000_plus(geom.thickness)?;
    if has_oblique {
        main_writer.write_raw_f64_le(geom.oblique_angle)?;
    }
    if has_rotation {
        main_writer.write_raw_f64_le(geom.rotation)?;
    }
    main_writer.write_raw_f64_le(geom.height)?;
    if has_width_factor {
        main_writer.write_raw_f64_le(geom.width_factor)?;
    }
    main_writer.write_text_ascii(&geom.value)?;
    if has_horizontal_alignment {
        main_writer.write_bit_short(geom.horizontal_alignment)?;
    }
    if has_vertical_alignment {
        main_writer.write_bit_short(geom.vertical_alignment)?;
    }

    handle_writer.write_handle(HANDLE_CODE_HARD_OWNER, geom.style_handle.value())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read_text_geometry, BitReader, BitWriter};
    use h7cad_native_model::Handle;

    fn round_trip(geom: TextGeometry, object_handle: Handle) -> TextGeometry {
        let mut main = BitWriter::new();
        let mut handle = BitWriter::new();
        write_text_geometry(geom, &mut main, &mut handle).expect("writes");
        let main_bytes = main.into_bytes();
        let handle_bytes = handle.into_bytes();
        let mut main_reader = BitReader::new(&main_bytes);
        let mut handle_reader = BitReader::new(&handle_bytes);
        read_text_geometry(&mut main_reader, &mut handle_reader, object_handle)
            .expect("reader recovers")
    }

    #[test]
    fn round_trips_minimal_text() {
        let geom = TextGeometry {
            insertion: [1.0, 2.0, 0.0],
            alignment_point: None,
            extrusion: [0.0, 0.0, 1.0],
            thickness: 0.0,
            oblique_angle: 0.0,
            rotation: 0.0,
            height: 3.0,
            width_factor: 1.0,
            value: "A".to_string(),
            horizontal_alignment: 0,
            vertical_alignment: 0,
            style_handle: Handle::new(0x21),
        };
        assert_eq!(round_trip(geom.clone(), Handle::new(0x10)), geom);
    }

    #[test]
    fn round_trips_nontrivial_text_fields() {
        let geom = TextGeometry {
            insertion: [1.0, 2.0, 3.0],
            alignment_point: Some([4.0, 5.0, 3.0]),
            extrusion: [1.0, 0.0, 0.0],
            thickness: 0.25,
            oblique_angle: 0.1,
            rotation: 0.5,
            height: 2.5,
            width_factor: 0.75,
            value: "Label-1".to_string(),
            horizontal_alignment: 1,
            vertical_alignment: 2,
            style_handle: Handle::new(0x22),
        };
        assert_eq!(round_trip(geom.clone(), Handle::new(0x20)), geom);
    }

    #[test]
    fn rejects_alignment_point_with_different_z() {
        let geom = TextGeometry {
            insertion: [1.0, 2.0, 3.0],
            alignment_point: Some([4.0, 5.0, 6.0]),
            extrusion: [0.0, 0.0, 1.0],
            thickness: 0.0,
            oblique_angle: 0.0,
            rotation: 0.0,
            height: 2.5,
            width_factor: 1.0,
            value: "A".to_string(),
            horizontal_alignment: 0,
            vertical_alignment: 0,
            style_handle: Handle::new(0x21),
        };
        let err = write_text_geometry(geom, &mut BitWriter::new(), &mut BitWriter::new())
            .expect_err("z mismatch must reject");
        assert!(matches!(err, DwgWriteError::InvalidValue(_)));
    }
}
