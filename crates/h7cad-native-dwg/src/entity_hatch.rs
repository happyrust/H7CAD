//! AC1015 HATCH entity decoder.

use crate::bit_reader::BitReader;
use crate::DwgReadError;
use h7cad_native_model::{Handle, HatchBoundaryPath, HatchEdge};

#[derive(Debug, Clone, PartialEq)]
pub struct HatchGeometry {
    pub pattern_name: String,
    pub solid_fill: bool,
    pub boundary_paths: Vec<HatchBoundaryPath>,
    pub extrusion: [f64; 3],
}

#[derive(Debug, Clone, PartialEq)]
struct ParsedBoundaryPath {
    path: HatchBoundaryPath,
    boundary_handle_count: usize,
    has_derived: bool,
}

#[inline]
fn safe_count(raw: i32) -> usize {
    raw.max(0).min(100_000) as usize
}

fn read_boundary_path(reader: &mut BitReader<'_>) -> Result<ParsedBoundaryPath, DwgReadError> {
    let flags = reader.read_bit_long()?;
    let is_polyline = (flags & 2) != 0;
    let has_derived = (flags & 4) != 0;

    let mut edges = Vec::new();
    if !is_polyline {
        let num_edges = safe_count(reader.read_bit_long()?);
        for _ in 0..num_edges {
            match reader.read_raw_u8()? {
                1 => {
                    let start = reader.read_2raw_double()?;
                    let end = reader.read_2raw_double()?;
                    edges.push(HatchEdge::Line { start, end });
                }
                2 => {
                    let center = reader.read_2raw_double()?;
                    let radius = reader.read_bit_double()?;
                    let start_angle = reader.read_bit_double()?;
                    let end_angle = reader.read_bit_double()?;
                    let is_ccw = reader.read_bit()? == 1;
                    edges.push(HatchEdge::CircularArc {
                        center,
                        radius,
                        start_angle,
                        end_angle,
                        is_ccw,
                    });
                }
                3 => {
                    let center = reader.read_2raw_double()?;
                    let major_endpoint = reader.read_2raw_double()?;
                    let minor_ratio = reader.read_bit_double()?;
                    let start_angle = reader.read_bit_double()?;
                    let end_angle = reader.read_bit_double()?;
                    let is_ccw = reader.read_bit()? == 1;
                    edges.push(HatchEdge::EllipticArc {
                        center,
                        major_endpoint,
                        minor_ratio,
                        start_angle,
                        end_angle,
                        is_ccw,
                    });
                }
                4 => {
                    let _degree = reader.read_bit_long()?;
                    let rational = reader.read_bit()? == 1;
                    let _periodic = reader.read_bit()? == 1;
                    let num_knots = safe_count(reader.read_bit_long()?);
                    let num_ctrl = safe_count(reader.read_bit_long()?);
                    for _ in 0..num_knots {
                        let _ = reader.read_bit_double()?;
                    }
                    for _ in 0..num_ctrl {
                        let _ = reader.read_2raw_double()?;
                        if rational {
                            let _ = reader.read_bit_double()?;
                        }
                    }
                    // AC1015 hatch spline edges do not carry fit-point
                    // data; we intentionally drop them from the native
                    // edge list for now.
                }
                _ => {}
            }
        }
    } else {
        let has_bulge = reader.read_bit()? == 1;
        let closed = reader.read_bit()? == 1;
        let num_vertices = safe_count(reader.read_bit_long()?);
        let mut vertices = Vec::with_capacity(num_vertices);
        for _ in 0..num_vertices {
            let point = reader.read_2raw_double()?;
            let bulge = if has_bulge {
                reader.read_bit_double()?
            } else {
                0.0
            };
            vertices.push([point[0], point[1], bulge]);
        }
        edges.push(HatchEdge::Polyline { closed, vertices });
    }

    let boundary_handle_count = safe_count(reader.read_bit_long()?);
    Ok(ParsedBoundaryPath {
        path: HatchBoundaryPath { flags, edges },
        boundary_handle_count,
        has_derived,
    })
}

pub fn read_hatch_geometry(
    main_reader: &mut BitReader<'_>,
    handle_reader: &mut BitReader<'_>,
    object_handle: Handle,
) -> Result<HatchGeometry, DwgReadError> {
    let _elevation = main_reader.read_bit_double()?;
    let extrusion = main_reader.read_3bit_double()?;
    let pattern_name = main_reader.read_text_ascii()?;
    let solid_fill = main_reader.read_bit()? == 1;
    let _is_associative = main_reader.read_bit()? == 1;

    let num_paths = safe_count(main_reader.read_bit_long()?);
    let mut boundary_paths = Vec::with_capacity(num_paths);
    let mut has_derived = false;
    let mut boundary_handle_total = 0usize;
    for _ in 0..num_paths {
        let parsed = read_boundary_path(main_reader)?;
        has_derived |= parsed.has_derived;
        boundary_handle_total += parsed.boundary_handle_count;
        boundary_paths.push(parsed.path);
    }

    let _style = main_reader.read_bit_short()?;
    let _pattern_type = main_reader.read_bit_short()?;

    if !solid_fill {
        let _pattern_angle = main_reader.read_bit_double()?;
        let _pattern_scale = main_reader.read_bit_double()?;
        let _is_double = main_reader.read_bit()? == 1;
        let num_lines = safe_count(main_reader.read_bit_short()? as i32);
        for _ in 0..num_lines {
            let _ = main_reader.read_bit_double()?;
            let _ = main_reader.read_2bit_double()?;
            let _ = main_reader.read_2bit_double()?;
            let num_dashes = safe_count(main_reader.read_bit_short()? as i32);
            for _ in 0..num_dashes {
                let _ = main_reader.read_bit_double()?;
            }
        }
    }

    if has_derived {
        let _ = main_reader.read_bit_double()?;
    }

    let num_seeds = safe_count(main_reader.read_bit_long()?);
    for _ in 0..num_seeds {
        let _ = main_reader.read_2raw_double()?;
    }

    for _ in 0..boundary_handle_total {
        let _ = handle_reader.read_handle_relative(object_handle.value())?;
    }

    Ok(HatchGeometry {
        pattern_name,
        solid_fill,
        boundary_paths,
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

    #[test]
    fn hatch_geometry_decodes_empty_solid_payload() {
        let mut bytes = Vec::new();
        let mut cursor = 0usize;
        emit_bits(&mut bytes, &mut cursor, 0b10, 2); // elevation 0
        emit_bits(&mut bytes, &mut cursor, 0b10, 2); // normal.x 0
        emit_bits(&mut bytes, &mut cursor, 0b10, 2); // normal.y 0
        emit_bits(&mut bytes, &mut cursor, 0b01, 2); // normal.z 1
        emit_bits(&mut bytes, &mut cursor, 0b10, 2); // empty pattern name
        emit_bits(&mut bytes, &mut cursor, 1, 1); // solid
        emit_bits(&mut bytes, &mut cursor, 0, 1); // non-assoc
        emit_bits(&mut bytes, &mut cursor, 0b10, 2); // num_paths = 0
        emit_bits(&mut bytes, &mut cursor, 0b10, 2); // style = 0
        emit_bits(&mut bytes, &mut cursor, 0b10, 2); // pattern_type = 0
        emit_bits(&mut bytes, &mut cursor, 0b10, 2); // num_seeds = 0

        let mut main_reader = BitReader::new(&bytes);
        let mut handle_reader = BitReader::new(&[]);
        let hatch =
            read_hatch_geometry(&mut main_reader, &mut handle_reader, Handle::new(0x10)).unwrap();
        assert!(hatch.solid_fill);
        assert!(hatch.boundary_paths.is_empty());
        assert_eq!(hatch.extrusion, [0.0, 0.0, 1.0]);
    }
}
