//! AC1015 LWPOLYLINE entity body writer (F5.M5.E4).
//!
//! Inverse of [`crate::read_lwpolyline_geometry`]. This is the first
//! variable-length entity writer: flags announce which optional arrays
//! are present, the first vertex uses raw doubles, and later vertices
//! use BitDouble-with-default against the previous coordinate.

use crate::bit_writer::BitWriter;
use crate::entity_lwpolyline::LwPolylineGeometry;
use crate::DwgWriteError;

const FLAG_HAS_NORMAL: i16 = 0x1;
const FLAG_HAS_THICKNESS: i16 = 0x2;
const FLAG_HAS_CONSTANT_WIDTH: i16 = 0x4;
const FLAG_HAS_ELEVATION: i16 = 0x8;
const FLAG_HAS_BULGES: i16 = 0x10;
const FLAG_HAS_WIDTHS: i16 = 0x20;
const FLAG_CLOSED: i16 = 0x200;

/// Write the AC1015 LWPOLYLINE-specific payload for `geom` into `writer`.
pub fn write_lwpolyline_geometry(
    geom: LwPolylineGeometry,
    writer: &mut BitWriter,
) -> Result<(), DwgWriteError> {
    let has_constant_width = geom.constant_width != 0.0;
    let has_elevation = geom.elevation != 0.0;
    let has_thickness = geom.thickness != 0.0;
    let has_normal = geom.extrusion != [0.0, 0.0, 1.0];
    let has_bulges = geom.vertices.iter().any(|v| v.bulge != 0.0);
    let has_widths = geom
        .vertices
        .iter()
        .any(|v| v.start_width != 0.0 || v.end_width != 0.0);

    let mut flag = 0i16;
    if has_normal {
        flag |= FLAG_HAS_NORMAL;
    }
    if has_thickness {
        flag |= FLAG_HAS_THICKNESS;
    }
    if has_constant_width {
        flag |= FLAG_HAS_CONSTANT_WIDTH;
    }
    if has_elevation {
        flag |= FLAG_HAS_ELEVATION;
    }
    if has_bulges {
        flag |= FLAG_HAS_BULGES;
    }
    if has_widths {
        flag |= FLAG_HAS_WIDTHS;
    }
    if geom.closed {
        flag |= FLAG_CLOSED;
    }

    writer.write_bit_short(flag)?;
    if has_constant_width {
        writer.write_bit_double(geom.constant_width)?;
    }
    if has_elevation {
        writer.write_bit_double(geom.elevation)?;
    }
    if has_thickness {
        writer.write_bit_thickness_r2000_plus(geom.thickness)?;
    }
    if has_normal {
        writer.write_bit_extrusion_r2000_plus(geom.extrusion)?;
    }

    let vertex_count = count_to_bit_long(geom.vertices.len(), "LWPOLYLINE vertex count")?;
    writer.write_bit_long(vertex_count)?;
    if has_bulges {
        writer.write_bit_long(vertex_count)?;
    }
    if has_widths {
        writer.write_bit_long(vertex_count)?;
    }

    if let Some(first) = geom.vertices.first() {
        writer.write_raw_f64_le(first.x)?;
        writer.write_raw_f64_le(first.y)?;

        let mut previous_x = first.x;
        let mut previous_y = first.y;
        for vertex in geom.vertices.iter().skip(1) {
            writer.write_bit_double_with_default(vertex.x, previous_x)?;
            writer.write_bit_double_with_default(vertex.y, previous_y)?;
            previous_x = vertex.x;
            previous_y = vertex.y;
        }
    }

    if has_bulges {
        for vertex in &geom.vertices {
            writer.write_bit_double(vertex.bulge)?;
        }
    }

    if has_widths {
        for vertex in &geom.vertices {
            writer.write_bit_double(vertex.start_width)?;
            writer.write_bit_double(vertex.end_width)?;
        }
    }

    Ok(())
}

fn count_to_bit_long(count: usize, field: &'static str) -> Result<i32, DwgWriteError> {
    i32::try_from(count)
        .map_err(|_| DwgWriteError::InvalidValue(format!("{field} exceeds i32::MAX: {count}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read_lwpolyline_geometry, BitReader, BitWriter};
    use h7cad_native_model::LwVertex;

    fn round_trip(geom: LwPolylineGeometry) -> LwPolylineGeometry {
        let mut writer = BitWriter::new();
        write_lwpolyline_geometry(geom, &mut writer).expect("writes");
        let bytes = writer.into_bytes();
        let mut reader = BitReader::new(&bytes);
        read_lwpolyline_geometry(&mut reader).expect("reader recovers")
    }

    #[test]
    fn round_trips_minimal_open_polyline() {
        let geom = LwPolylineGeometry {
            vertices: vec![
                LwVertex {
                    x: 1.0,
                    y: 2.0,
                    bulge: 0.0,
                    start_width: 0.0,
                    end_width: 0.0,
                },
                LwVertex {
                    x: 3.0,
                    y: 4.0,
                    bulge: 0.0,
                    start_width: 0.0,
                    end_width: 0.0,
                },
            ],
            closed: false,
            constant_width: 0.0,
            elevation: 0.0,
            thickness: 0.0,
            extrusion: [0.0, 0.0, 1.0],
        };
        assert_eq!(round_trip(geom.clone()), geom);
    }

    #[test]
    fn round_trips_closed_polyline_with_optional_arrays() {
        let geom = LwPolylineGeometry {
            vertices: vec![
                LwVertex {
                    x: 1.0,
                    y: 2.0,
                    bulge: 0.25,
                    start_width: 0.1,
                    end_width: 0.2,
                },
                LwVertex {
                    x: 3.0,
                    y: 4.0,
                    bulge: 0.0,
                    start_width: 0.3,
                    end_width: 0.4,
                },
            ],
            closed: true,
            constant_width: 0.5,
            elevation: 6.0,
            thickness: 0.75,
            extrusion: [1.0, 0.0, 0.0],
        };
        assert_eq!(round_trip(geom.clone()), geom);
    }

    #[test]
    fn writes_empty_polyline_payload() {
        let geom = LwPolylineGeometry {
            vertices: Vec::new(),
            closed: false,
            constant_width: 0.0,
            elevation: 0.0,
            thickness: 0.0,
            extrusion: [0.0, 0.0, 1.0],
        };
        assert_eq!(round_trip(geom.clone()), geom);
    }
}
