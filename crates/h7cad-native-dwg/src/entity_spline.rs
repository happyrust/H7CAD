//! AC1015 SPLINE entity decoder.
//!
//! On-disk layout (R2000, per ACadSharp `DwgEntityReader.ReadSpline`):
//!
//! ```text
//!   BL   scenario
//!   BL   degree             (if scenario == 2)
//!   BD   fit_tolerance
//!   3BD  start_tangent
//!   3BD  end_tangent
//!   BL   num_fit_points
//!   3BD × num_fit_points
//!   B    rational
//!   B    closed
//!   B    periodic
//!   BD   knot_tolerance
//!   BD   control_tolerance
//!   BL   num_knots
//!   BD  × num_knots
//!   BL   num_control_points
//!   (3BD + BD weight if rational) × num_control_points
//! ```
//!
//! When `scenario == 1` (fit-point spline), the spec says to read
//! `degree` from a different field (BL at position after scenario);
//! the above is the typical case.

use crate::bit_reader::BitReader;
use crate::DwgReadError;

const MAX_KNOTS: usize = 16384;
const MAX_CONTROL_POINTS: usize = 16384;
const MAX_FIT_POINTS: usize = 16384;

#[derive(Debug, Clone, PartialEq)]
pub struct SplineGeometry {
    pub degree: i32,
    pub closed: bool,
    pub knots: Vec<f64>,
    pub control_points: Vec<[f64; 3]>,
    pub weights: Vec<f64>,
    pub fit_points: Vec<[f64; 3]>,
    pub start_tangent: [f64; 3],
    pub end_tangent: [f64; 3],
}

pub fn read_spline_geometry(reader: &mut BitReader<'_>) -> Result<SplineGeometry, DwgReadError> {
    let scenario = reader.read_bit_long()?;

    let degree = if scenario == 2 {
        reader.read_bit_long()?
    } else if scenario == 1 {
        reader.read_bit_long()?
    } else {
        reader.read_bit_long()?
    };

    let mut fit_points = Vec::new();
    let mut start_tangent = [0.0; 3];
    let mut end_tangent = [0.0; 3];

    if scenario == 2 {
        let _fit_tolerance = reader.read_bit_double()?;
        start_tangent = reader.read_3bit_double()?;
        end_tangent = reader.read_3bit_double()?;
        let num_fit_points = safe_count(reader.read_bit_long()?, MAX_FIT_POINTS);
        fit_points.reserve(num_fit_points);
        for _ in 0..num_fit_points {
            fit_points.push(reader.read_3bit_double()?);
        }
    }

    let rational = reader.read_bit()? == 1;
    let closed = reader.read_bit()? == 1;
    let _periodic = reader.read_bit()? == 1;

    let _knot_tolerance = reader.read_bit_double()?;
    let _control_tolerance = reader.read_bit_double()?;

    let num_knots = safe_count(reader.read_bit_long()?, MAX_KNOTS);
    let mut knots = Vec::with_capacity(num_knots);
    for _ in 0..num_knots {
        knots.push(reader.read_bit_double()?);
    }

    let num_control_points = safe_count(reader.read_bit_long()?, MAX_CONTROL_POINTS);
    let mut control_points = Vec::with_capacity(num_control_points);
    let mut weights = Vec::new();
    if rational {
        weights.reserve(num_control_points);
    }
    for _ in 0..num_control_points {
        control_points.push(reader.read_3bit_double()?);
        if rational {
            weights.push(reader.read_bit_double()?);
        }
    }

    Ok(SplineGeometry {
        degree,
        closed,
        knots,
        control_points,
        weights,
        fit_points,
        start_tangent,
        end_tangent,
    })
}

fn safe_count(value: i32, max: usize) -> usize {
    if value < 0 {
        0
    } else {
        (value as usize).min(max)
    }
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
    fn spline_reports_eof_on_empty() {
        let err = read_spline_geometry(&mut BitReader::new(&[])).unwrap_err();
        assert!(matches!(err, DwgReadError::UnexpectedEof { .. }));
    }

    #[test]
    fn spline_scenario1_minimal() {
        let mut bytes = Vec::new();
        let mut cursor = 0usize;
        // scenario = 1 (BL prefix 01 => next byte = 1)
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        emit_bits(&mut bytes, &mut cursor, 1, 8);
        // degree = 3 (BL prefix 01 => next byte = 3)
        emit_bits(&mut bytes, &mut cursor, 0b01, 2);
        emit_bits(&mut bytes, &mut cursor, 3, 8);
        // rational = 0, closed = 0, periodic = 0
        emit_bits(&mut bytes, &mut cursor, 0, 1);
        emit_bits(&mut bytes, &mut cursor, 0, 1);
        emit_bits(&mut bytes, &mut cursor, 0, 1);
        // knot_tolerance = 0 (BD prefix 10)
        emit_bits(&mut bytes, &mut cursor, 0b10, 2);
        // control_tolerance = 0
        emit_bits(&mut bytes, &mut cursor, 0b10, 2);
        // num_knots = 0 (BL prefix 10)
        emit_bits(&mut bytes, &mut cursor, 0b10, 2);
        // num_control_points = 0
        emit_bits(&mut bytes, &mut cursor, 0b10, 2);

        let mut reader = BitReader::new(&bytes);
        let geom = read_spline_geometry(&mut reader).unwrap();
        assert_eq!(geom.degree, 3);
        assert!(!geom.closed);
        assert!(geom.knots.is_empty());
        assert!(geom.control_points.is_empty());
        assert!(geom.fit_points.is_empty());
    }
}
