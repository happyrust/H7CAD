//! AC1015 SOLID / 3DFACE entity geometry decoder.
//!
//! SOLID (object_type = 31) on-disk layout (R2000+):
//!
//! ```text
//!   BT   thickness
//!   BD   elevation
//!   2RD  corner1 (x,y)
//!   2RD  corner2 (x,y)
//!   2RD  corner3 (x,y)
//!   2RD  corner4 (x,y)
//!   BE   extrusion
//! ```
//!
//! 3DFACE (object_type = 28) on-disk layout:
//!
//! ```text
//!   B    has_no_flags
//!   BS   invisible_edges  (only if has_no_flags == 0)
//!   3BD  corner1
//!   3BD  corner2
//!   3BD  corner3
//!   3BD  corner4
//! ```

use crate::bit_reader::BitReader;
use crate::DwgReadError;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolidGeometry {
    pub corners: [[f64; 3]; 4],
    pub thickness: f64,
    pub extrusion: [f64; 3],
}

pub fn read_solid_geometry(reader: &mut BitReader<'_>) -> Result<SolidGeometry, DwgReadError> {
    let thickness = reader.read_bit_thickness_r2000_plus()?;
    let elevation = reader.read_bit_double()?;
    let c1 = reader.read_2raw_double()?;
    let c2 = reader.read_2raw_double()?;
    let c3 = reader.read_2raw_double()?;
    let c4 = reader.read_2raw_double()?;
    let extrusion = reader.read_bit_extrusion_r2000_plus()?;
    Ok(SolidGeometry {
        corners: [
            [c1[0], c1[1], elevation],
            [c2[0], c2[1], elevation],
            [c3[0], c3[1], elevation],
            [c4[0], c4[1], elevation],
        ],
        thickness,
        extrusion,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Face3DGeometry {
    pub corners: [[f64; 3]; 4],
    pub invisible_edges: i16,
}

pub fn read_face3d_geometry(reader: &mut BitReader<'_>) -> Result<Face3DGeometry, DwgReadError> {
    let has_no_flags = reader.read_bit()?;
    let invisible_edges = if has_no_flags == 0 {
        reader.read_bit_short()?
    } else {
        0
    };
    let c1 = reader.read_3bit_double()?;
    let c2 = reader.read_3bit_double()?;
    let c3 = reader.read_3bit_double()?;
    let c4 = reader.read_3bit_double()?;
    Ok(Face3DGeometry {
        corners: [c1, c2, c3, c4],
        invisible_edges,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn face3d_reports_eof_on_empty() {
        let err = read_face3d_geometry(&mut BitReader::new(&[])).unwrap_err();
        assert!(matches!(err, DwgReadError::UnexpectedEof { .. }));
    }

    #[test]
    fn solid_reports_eof_on_empty() {
        let err = read_solid_geometry(&mut BitReader::new(&[])).unwrap_err();
        assert!(matches!(err, DwgReadError::UnexpectedEof { .. }));
    }
}
