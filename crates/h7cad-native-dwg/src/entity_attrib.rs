//! AC1015 ATTRIB / ATTDEF entity decoders.
//!
//! ATTRIB (type 2) is a TEXT-like entity attached to INSERT blocks.
//! ATTDEF (type 3) is the template definition in block definitions.
//!
//! Both share a similar prefix with TEXT (data_flags + elevation +
//! insertion + alignment + extrusion + thickness + angles + height +
//! width_factor + value/tag). ATTDEF adds a prompt and field_length.
//!
//! On-disk layout (R2000): same as TEXT with additional trailing fields.

use crate::bit_reader::BitReader;
use crate::DwgReadError;
use h7cad_native_model::Handle;

#[derive(Debug, Clone, PartialEq)]
pub struct AttribGeometry {
    pub tag: String,
    pub value: String,
    pub insertion: [f64; 3],
    pub height: f64,
    pub extrusion: [f64; 3],
    pub thickness: f64,
    pub rotation: f64,
    pub style_handle: Handle,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttDefGeometry {
    pub tag: String,
    pub prompt: String,
    pub default_value: String,
    pub insertion: [f64; 3],
    pub height: f64,
    pub extrusion: [f64; 3],
    pub thickness: f64,
    pub style_handle: Handle,
}

pub fn read_attrib_geometry(
    main_reader: &mut BitReader<'_>,
    handle_reader: &mut BitReader<'_>,
    object_handle: Handle,
) -> Result<AttribGeometry, DwgReadError> {
    let data_flags = main_reader.read_raw_u8()?;
    let elevation = if (data_flags & 0x01) == 0 {
        main_reader.read_raw_f64_le()?
    } else {
        0.0
    };
    let ix = main_reader.read_raw_f64_le()?;
    let iy = main_reader.read_raw_f64_le()?;
    if (data_flags & 0x02) == 0 {
        let _ = main_reader.read_bit_double_with_default(ix)?;
        let _ = main_reader.read_bit_double_with_default(iy)?;
    }
    let extrusion = main_reader.read_bit_extrusion_r2000_plus()?;
    let thickness = main_reader.read_bit_thickness_r2000_plus()?;
    if (data_flags & 0x04) == 0 {
        let _ = main_reader.read_raw_f64_le()?; // oblique
    }
    let rotation = if (data_flags & 0x08) == 0 {
        main_reader.read_raw_f64_le()?
    } else {
        0.0
    };
    let height = main_reader.read_raw_f64_le()?;
    if (data_flags & 0x10) == 0 {
        let _ = main_reader.read_raw_f64_le()?; // width_factor
    }
    let value = main_reader.read_text_ascii()?;
    if (data_flags & 0x20) == 0 {
        let _ = main_reader.read_bit_short()?; // generation
    }
    if (data_flags & 0x40) == 0 {
        let _ = main_reader.read_bit_short()?; // horizontal_alignment
    }
    if (data_flags & 0x80) == 0 {
        let _ = main_reader.read_bit_short()?; // vertical_alignment
    }

    let tag = main_reader.read_text_ascii()?;
    let _field_length = main_reader.read_bit_short()?;
    let _flags = main_reader.read_raw_u8()?;

    let style_handle = Handle::new(handle_reader.read_handle_relative(object_handle.value())?);

    Ok(AttribGeometry {
        tag,
        value,
        insertion: [ix, iy, elevation],
        height,
        extrusion,
        thickness,
        rotation,
        style_handle,
    })
}

pub fn read_attdef_geometry(
    main_reader: &mut BitReader<'_>,
    handle_reader: &mut BitReader<'_>,
    object_handle: Handle,
) -> Result<AttDefGeometry, DwgReadError> {
    let data_flags = main_reader.read_raw_u8()?;
    let elevation = if (data_flags & 0x01) == 0 {
        main_reader.read_raw_f64_le()?
    } else {
        0.0
    };
    let ix = main_reader.read_raw_f64_le()?;
    let iy = main_reader.read_raw_f64_le()?;
    if (data_flags & 0x02) == 0 {
        let _ = main_reader.read_bit_double_with_default(ix)?;
        let _ = main_reader.read_bit_double_with_default(iy)?;
    }
    let extrusion = main_reader.read_bit_extrusion_r2000_plus()?;
    let thickness = main_reader.read_bit_thickness_r2000_plus()?;
    if (data_flags & 0x04) == 0 {
        let _ = main_reader.read_raw_f64_le()?; // oblique
    }
    if (data_flags & 0x08) == 0 {
        let _ = main_reader.read_raw_f64_le()?; // rotation
    }
    let height = main_reader.read_raw_f64_le()?;
    if (data_flags & 0x10) == 0 {
        let _ = main_reader.read_raw_f64_le()?; // width_factor
    }
    let default_value = main_reader.read_text_ascii()?;
    if (data_flags & 0x20) == 0 {
        let _ = main_reader.read_bit_short()?; // generation
    }
    if (data_flags & 0x40) == 0 {
        let _ = main_reader.read_bit_short()?; // horizontal_alignment
    }
    if (data_flags & 0x80) == 0 {
        let _ = main_reader.read_bit_short()?; // vertical_alignment
    }

    let tag = main_reader.read_text_ascii()?;
    let _field_length = main_reader.read_bit_short()?;
    let _flags = main_reader.read_raw_u8()?;
    let prompt = main_reader.read_text_ascii()?;

    let style_handle = Handle::new(handle_reader.read_handle_relative(object_handle.value())?);

    Ok(AttDefGeometry {
        tag,
        prompt,
        default_value,
        insertion: [ix, iy, elevation],
        height,
        extrusion,
        thickness,
        style_handle,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attrib_reports_eof_on_empty() {
        let handles = [0x00; 4];
        let err = read_attrib_geometry(
            &mut BitReader::new(&[]),
            &mut BitReader::new(&handles),
            Handle::new(0x10),
        )
        .unwrap_err();
        assert!(matches!(err, DwgReadError::UnexpectedEof { .. }));
    }

    #[test]
    fn attdef_reports_eof_on_empty() {
        let handles = [0x00; 4];
        let err = read_attdef_geometry(
            &mut BitReader::new(&[]),
            &mut BitReader::new(&handles),
            Handle::new(0x10),
        )
        .unwrap_err();
        assert!(matches!(err, DwgReadError::UnexpectedEof { .. }));
    }
}
