//! AC1015 LWPOLYLINE entity decoder.

use crate::bit_reader::BitReader;
use crate::DwgReadError;
use h7cad_native_model::LwVertex;

#[derive(Debug, Clone, PartialEq)]
pub struct LwPolylineGeometry {
    pub vertices: Vec<LwVertex>,
    pub closed: bool,
    pub constant_width: f64,
    pub elevation: f64,
    pub thickness: f64,
    pub extrusion: [f64; 3],
}

#[inline]
fn safe_count(raw: i32) -> usize {
    raw.max(0).min(100_000) as usize
}

pub fn read_lwpolyline_geometry(
    reader: &mut BitReader<'_>,
) -> Result<LwPolylineGeometry, DwgReadError> {
    let flag = reader.read_bit_short()?;
    let has_constant_width = (flag & 0x4) != 0;
    let has_elevation = (flag & 0x8) != 0;
    let has_thickness = (flag & 0x2) != 0;
    let has_normal = (flag & 0x1) != 0;
    let has_bulges = (flag & 0x10) != 0;
    let has_widths = (flag & 0x20) != 0;

    let constant_width = if has_constant_width {
        reader.read_bit_double()?
    } else {
        0.0
    };
    let elevation = if has_elevation {
        reader.read_bit_double()?
    } else {
        0.0
    };
    let thickness = if has_thickness {
        reader.read_bit_thickness_r2000_plus()?
    } else {
        0.0
    };
    let extrusion = if has_normal {
        reader.read_bit_extrusion_r2000_plus()?
    } else {
        [0.0, 0.0, 1.0]
    };

    let num_pts = safe_count(reader.read_bit_long()?);
    let num_bulges = if has_bulges {
        safe_count(reader.read_bit_long()?)
    } else {
        0
    };
    let has_vertex_ids = (flag & 0x400) != 0;
    let num_vertex_ids = if has_vertex_ids {
        safe_count(reader.read_bit_long()?)
    } else {
        0
    };
    let num_widths = if has_widths {
        safe_count(reader.read_bit_long()?)
    } else {
        0
    };

    let mut xs = Vec::with_capacity(num_pts);
    let mut ys = Vec::with_capacity(num_pts);
    if num_pts > 0 {
        xs.push(reader.read_raw_f64_le()?);
        ys.push(reader.read_raw_f64_le()?);
        for i in 1..num_pts {
            let px = xs[i - 1];
            let py = ys[i - 1];
            xs.push(reader.read_bit_double_with_default(px)?);
            ys.push(reader.read_bit_double_with_default(py)?);
        }
    }

    let mut bulges = vec![0.0f64; num_pts];
    if has_bulges {
        for i in 0..num_bulges.min(num_pts) {
            bulges[i] = reader.read_bit_double()?;
        }
        for _ in num_bulges.min(num_pts)..num_bulges {
            let _ = reader.read_bit_double()?;
        }
    }

    if has_vertex_ids {
        for _ in 0..num_vertex_ids {
            let _ = reader.read_bit_long()?;
        }
    }

    let mut start_widths = vec![0.0f64; num_pts];
    let mut end_widths = vec![0.0f64; num_pts];
    if has_widths {
        for i in 0..num_widths.min(num_pts) {
            start_widths[i] = reader.read_bit_double()?;
            end_widths[i] = reader.read_bit_double()?;
        }
        for _ in num_widths.min(num_pts)..num_widths {
            let _ = reader.read_bit_double()?;
            let _ = reader.read_bit_double()?;
        }
    }

    let vertices = (0..num_pts)
        .map(|i| LwVertex {
            x: xs[i],
            y: ys[i],
            bulge: bulges[i],
            start_width: start_widths[i],
            end_width: end_widths[i],
        })
        .collect();

    Ok(LwPolylineGeometry {
        vertices,
        closed: (flag & 0x200) != 0,
        constant_width,
        elevation,
        thickness,
        extrusion,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn emit_raw_f64(out: &mut Vec<u8>, cursor: &mut usize, value: f64) {
        for byte in value.to_le_bytes() {
            emit_bits(out, cursor, byte as u64, 8);
        }
    }

    #[test]
    fn lwpolyline_decodes_two_vertices_minimal_payload() {
        let mut bytes = Vec::new();
        let mut cursor = 0usize;
        emit_bits(&mut bytes, &mut cursor, 0b10, 2); // flag = 0
        emit_bits(&mut bytes, &mut cursor, 0b01, 2); // num_pts uses u8
        emit_bits(&mut bytes, &mut cursor, 2, 8);
        emit_raw_f64(&mut bytes, &mut cursor, 1.0);
        emit_raw_f64(&mut bytes, &mut cursor, 2.0);
        emit_bits(&mut bytes, &mut cursor, 0b11, 2);
        emit_raw_f64(&mut bytes, &mut cursor, 3.0);
        emit_bits(&mut bytes, &mut cursor, 0b11, 2);
        emit_raw_f64(&mut bytes, &mut cursor, 4.0);

        let mut reader = BitReader::new(&bytes);
        let poly = read_lwpolyline_geometry(&mut reader).unwrap();
        assert_eq!(poly.vertices.len(), 2);
        assert_eq!(poly.vertices[0].x, 1.0);
        assert_eq!(poly.vertices[0].y, 2.0);
        assert_eq!(poly.vertices[1].x, 3.0);
        assert_eq!(poly.vertices[1].y, 4.0);
        assert!(!poly.closed);
        assert_eq!(poly.extrusion, [0.0, 0.0, 1.0]);
    }
}
