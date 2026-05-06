//! AC1015 DIMENSION entity decoder.
//!
//! DWG encodes seven dimension sub-types as separate object_type codes
//! (20–26), but all share a common "dimension base" preamble. The
//! sub-type-specific suffix reads extra point/angle fields depending
//! on the object_type.
//!
//! Common base (R2000, per ACadSharp `DwgEntityReader.ReadCommonDimensionData`):
//!
//! ```text
//!   BL   version (3 = R2010+, 2 = R2007, else R2000)
//!   3BD  extrusion
//!   2RD  text_midpoint (x,y)
//!   BD   elevation
//!   RC   flags
//!   T    text_override
//!   BD   text_rotation
//!   BD   horizontal_direction
//!   3BD  ins_scale
//!   BD   ins_rotation
//!   --- R2000-specific:
//!   B    attachment_point_bit2
//!   B    attachment_point_bit1
//!   B    dim_text_attachment
//!   --- handles:
//!   H    dimstyle
//!   H    anonymous_block
//! ```

use crate::bit_reader::BitReader;
use crate::DwgReadError;
use h7cad_native_model::Handle;

#[derive(Debug, Clone, PartialEq)]
pub struct DimensionGeometry {
    pub dim_type: i16,
    pub extrusion: [f64; 3],
    pub text_midpoint: [f64; 3],
    pub definition_point: [f64; 3],
    pub first_point: [f64; 3],
    pub second_point: [f64; 3],
    pub angle_vertex: [f64; 3],
    pub dimension_arc: [f64; 3],
    pub text_override: String,
    pub text_rotation: f64,
    pub horizontal_direction: f64,
    pub measurement: f64,
    pub leader_length: f64,
    pub rotation: f64,
    pub ext_line_rotation: f64,
    pub attachment_point: i16,
    pub flip_arrow1: bool,
    pub flip_arrow2: bool,
    pub style_handle: Handle,
    pub block_handle: Handle,
}

pub fn read_dimension_geometry(
    object_type: i16,
    main_reader: &mut BitReader<'_>,
    handle_reader: &mut BitReader<'_>,
    object_handle: Handle,
) -> Result<DimensionGeometry, DwgReadError> {
    let _version = main_reader.read_bit_long()?;
    let extrusion = main_reader.read_3bit_double()?;
    let text_mid_xy = main_reader.read_2raw_double()?;
    let elevation = main_reader.read_bit_double()?;
    let flags = main_reader.read_raw_u8()?;
    let text_override = main_reader.read_text_ascii()?;
    let text_rotation = main_reader.read_bit_double()?;
    let horizontal_direction = main_reader.read_bit_double()?;

    let _ins_scale = main_reader.read_3bit_double()?;
    let _ins_rotation = main_reader.read_bit_double()?;

    let ap_bit2 = main_reader.read_bit()? as i16;
    let ap_bit1 = main_reader.read_bit()? as i16;
    let _dim_text_attachment = main_reader.read_bit()?;
    let attachment_point = (ap_bit2 << 1) | ap_bit1;

    let flip_arrow1 = (flags & 0x01) != 0;
    let flip_arrow2 = (flags & 0x02) != 0;

    let dim_type = match object_type {
        20 => 6, // DIMENSION_ORDINATE
        21 => 0, // DIMENSION_LINEAR
        22 => 1, // DIMENSION_ALIGNED
        23 => 5, // DIMENSION_ANG3PT
        24 => 2, // DIMENSION_ANG2LN
        25 => 4, // DIMENSION_RADIUS
        26 => 3, // DIMENSION_DIAMETER
        _ => 0,
    };

    let mut definition_point = [0.0; 3];
    let mut first_point = [0.0; 3];
    let mut second_point = [0.0; 3];
    let mut angle_vertex = [0.0; 3];
    let mut dimension_arc = [0.0; 3];
    let mut leader_length = 0.0;
    let mut rotation = 0.0;
    let mut ext_line_rotation = 0.0;

    match object_type {
        21 => {
            // LINEAR
            let p13 = main_reader.read_2raw_double()?;
            let p14 = main_reader.read_2raw_double()?;
            let p10 = main_reader.read_3bit_double()?;
            rotation = main_reader.read_bit_double()?;
            ext_line_rotation = main_reader.read_bit_double()?;
            first_point = [p13[0], p13[1], elevation];
            second_point = [p14[0], p14[1], elevation];
            definition_point = p10;
        }
        22 => {
            // ALIGNED
            let p13 = main_reader.read_2raw_double()?;
            let p14 = main_reader.read_2raw_double()?;
            let p10 = main_reader.read_3bit_double()?;
            ext_line_rotation = main_reader.read_bit_double()?;
            first_point = [p13[0], p13[1], elevation];
            second_point = [p14[0], p14[1], elevation];
            definition_point = p10;
        }
        23 => {
            // ANGULAR 3PT
            let p10 = main_reader.read_3bit_double()?;
            let p13 = main_reader.read_3bit_double()?;
            let p14 = main_reader.read_3bit_double()?;
            let p15 = main_reader.read_3bit_double()?;
            definition_point = p10;
            first_point = p13;
            second_point = p14;
            angle_vertex = p15;
        }
        24 => {
            // ANGULAR 2-LINE
            let p16 = main_reader.read_2raw_double()?;
            let p13 = main_reader.read_2raw_double()?;
            let p14 = main_reader.read_2raw_double()?;
            let p15 = main_reader.read_3bit_double()?;
            let p10 = main_reader.read_3bit_double()?;
            dimension_arc = [p16[0], p16[1], elevation];
            first_point = [p13[0], p13[1], elevation];
            second_point = [p14[0], p14[1], elevation];
            angle_vertex = p15;
            definition_point = p10;
        }
        25 => {
            // RADIUS
            let p10 = main_reader.read_3bit_double()?;
            let p15 = main_reader.read_3bit_double()?;
            leader_length = main_reader.read_bit_double()?;
            definition_point = p10;
            angle_vertex = p15;
        }
        26 => {
            // DIAMETER
            let p10 = main_reader.read_3bit_double()?;
            let p15 = main_reader.read_3bit_double()?;
            leader_length = main_reader.read_bit_double()?;
            definition_point = p10;
            angle_vertex = p15;
        }
        20 => {
            // ORDINATE
            let p10 = main_reader.read_3bit_double()?;
            let p13 = main_reader.read_3bit_double()?;
            let p14 = main_reader.read_3bit_double()?;
            definition_point = p10;
            first_point = p13;
            second_point = p14;
        }
        _ => {}
    }

    let style_handle = Handle::new(handle_reader.read_handle_relative(object_handle.value())?);
    let block_handle = Handle::new(handle_reader.read_handle_relative(object_handle.value())?);

    Ok(DimensionGeometry {
        dim_type,
        extrusion,
        text_midpoint: [text_mid_xy[0], text_mid_xy[1], elevation],
        definition_point,
        first_point,
        second_point,
        angle_vertex,
        dimension_arc,
        text_override,
        text_rotation,
        horizontal_direction,
        measurement: 0.0,
        leader_length,
        rotation,
        ext_line_rotation,
        attachment_point,
        flip_arrow1,
        flip_arrow2,
        style_handle,
        block_handle,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimension_reports_eof_on_empty() {
        let handles = [0x00; 8];
        let err = read_dimension_geometry(
            21,
            &mut BitReader::new(&[]),
            &mut BitReader::new(&handles),
            Handle::new(0x10),
        )
        .unwrap_err();
        assert!(matches!(err, DwgReadError::UnexpectedEof { .. }));
    }
}
