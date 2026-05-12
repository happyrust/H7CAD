//! AC1015 SPLINE entity body writer (F5.M5.E10).
//!
//! Inverse of [`crate::read_spline_geometry`]:
//!
//! ```text
//!   BL   scenario          (1 = control-point, 2 = fit-point)
//!   BL   degree
//!   if scenario == 2:
//!     BD   fit_tolerance
//!     3BD  start_tangent
//!     3BD  end_tangent
//!     BL   num_fit_points
//!     3BD × num_fit_points
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
//! Field choices made by this writer (none round-trip-breaking, but
//! they pin behaviour the reader will then observe):
//!
//! - `scenario` is set to `2` whenever any fit-point data is
//!   non-default (`!fit_points.is_empty() || start_tangent != [0,0,0]
//!   || end_tangent != [0,0,0]`), otherwise `1`. The reader sets
//!   `fit_points / start_tangent / end_tangent` to their default empty/
//!   zero values when scenario is 1, so this rule is the minimum
//!   sufficient condition for full round-trip.
//! - `rational` is derived from `!weights.is_empty()`. When the
//!   geometry claims weights but its length disagrees with
//!   `control_points`, the writer returns
//!   [`DwgWriteError::InvalidValue`] rather than silently truncating.
//! - `periodic` is always `0`: the reader discards this bit
//!   (`_periodic` in `read_spline_geometry`) so there is no source
//!   to round-trip.
//! - `fit_tolerance`, `knot_tolerance`, and `control_tolerance` are
//!   always `0.0` for the same reason — they are also discarded by
//!   the reader.

use crate::bit_writer::BitWriter;
use crate::entity_spline::SplineGeometry;
use crate::DwgWriteError;

const PERIODIC_FLAG: u8 = 0;
const ZERO_TOLERANCE: f64 = 0.0;
const SCENARIO_FIT_POINT: i32 = 2;
const SCENARIO_CONTROL_POINT: i32 = 1;

fn fit_data_is_nondefault(geom: &SplineGeometry) -> bool {
    !geom.fit_points.is_empty()
        || geom.start_tangent != [0.0, 0.0, 0.0]
        || geom.end_tangent != [0.0, 0.0, 0.0]
}

/// Write the AC1015 SPLINE-specific payload for `geom` into `writer`.
pub fn write_spline_geometry(
    geom: &SplineGeometry,
    writer: &mut BitWriter,
) -> Result<(), DwgWriteError> {
    let rational = !geom.weights.is_empty();
    if rational && geom.weights.len() != geom.control_points.len() {
        return Err(DwgWriteError::InvalidValue(format!(
            "rational SPLINE weights ({}) must match control_points ({})",
            geom.weights.len(),
            geom.control_points.len()
        )));
    }

    let scenario = if fit_data_is_nondefault(geom) {
        SCENARIO_FIT_POINT
    } else {
        SCENARIO_CONTROL_POINT
    };

    writer.write_bit_long(scenario)?;
    writer.write_bit_long(geom.degree)?;

    if scenario == SCENARIO_FIT_POINT {
        writer.write_bit_double(ZERO_TOLERANCE)?;
        writer.write_3bit_double(geom.start_tangent)?;
        writer.write_3bit_double(geom.end_tangent)?;
        let num_fit_points = i32::try_from(geom.fit_points.len()).map_err(|_| {
            DwgWriteError::InvalidValue(format!(
                "SPLINE fit_points count {} exceeds i32::MAX",
                geom.fit_points.len()
            ))
        })?;
        writer.write_bit_long(num_fit_points)?;
        for fp in &geom.fit_points {
            writer.write_3bit_double(*fp)?;
        }
    }

    writer.write_bit(if rational { 1 } else { 0 })?;
    writer.write_bit(if geom.closed { 1 } else { 0 })?;
    writer.write_bit(PERIODIC_FLAG)?;

    writer.write_bit_double(ZERO_TOLERANCE)?;
    writer.write_bit_double(ZERO_TOLERANCE)?;

    let num_knots = i32::try_from(geom.knots.len()).map_err(|_| {
        DwgWriteError::InvalidValue(format!(
            "SPLINE knots count {} exceeds i32::MAX",
            geom.knots.len()
        ))
    })?;
    writer.write_bit_long(num_knots)?;
    for k in &geom.knots {
        writer.write_bit_double(*k)?;
    }

    let num_control_points = i32::try_from(geom.control_points.len()).map_err(|_| {
        DwgWriteError::InvalidValue(format!(
            "SPLINE control_points count {} exceeds i32::MAX",
            geom.control_points.len()
        ))
    })?;
    writer.write_bit_long(num_control_points)?;
    for (idx, cp) in geom.control_points.iter().enumerate() {
        writer.write_3bit_double(*cp)?;
        if rational {
            writer.write_bit_double(geom.weights[idx])?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read_spline_geometry, BitReader, BitWriter, DwgWriteError};

    fn round_trip(geom: SplineGeometry) -> SplineGeometry {
        let mut writer = BitWriter::new();
        write_spline_geometry(&geom, &mut writer).expect("writes");
        let bytes = writer.into_bytes();
        let mut reader = BitReader::new(&bytes);
        read_spline_geometry(&mut reader).expect("reader recovers")
    }

    fn empty_spline(degree: i32) -> SplineGeometry {
        SplineGeometry {
            degree,
            closed: false,
            knots: Vec::new(),
            control_points: Vec::new(),
            weights: Vec::new(),
            fit_points: Vec::new(),
            start_tangent: [0.0, 0.0, 0.0],
            end_tangent: [0.0, 0.0, 0.0],
        }
    }

    #[test]
    fn spline_round_trips_scenario1_minimal() {
        // Empty spline → scenario 1 path; no fit-point block emitted.
        let geom = empty_spline(3);
        assert_eq!(round_trip(geom.clone()), geom);
    }

    #[test]
    fn spline_round_trips_scenario1_with_control_points_and_knots() {
        // Non-trivial closed spline, control-point flavour. weights
        // stays empty (rational = false).
        let geom = SplineGeometry {
            degree: 3,
            closed: true,
            knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            control_points: vec![
                [0.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [2.0, 1.0, 0.0],
                [3.0, 0.0, 0.0],
            ],
            weights: Vec::new(),
            fit_points: Vec::new(),
            start_tangent: [0.0, 0.0, 0.0],
            end_tangent: [0.0, 0.0, 0.0],
        };
        assert_eq!(round_trip(geom.clone()), geom);
    }

    #[test]
    fn spline_round_trips_scenario2_fit_points_with_tangents() {
        // Fit-point flavour: forces scenario 2 because fit_points is
        // non-empty AND tangents are non-zero.
        let geom = SplineGeometry {
            degree: 3,
            closed: false,
            knots: Vec::new(),
            control_points: Vec::new(),
            weights: Vec::new(),
            fit_points: vec![
                [0.0, 0.0, 0.0],
                [1.0, 2.0, 0.0],
                [3.0, 1.0, 0.0],
            ],
            start_tangent: [1.0, 0.0, 0.0],
            end_tangent: [-1.0, 0.0, 0.0],
        };
        assert_eq!(round_trip(geom.clone()), geom);
    }

    #[test]
    fn spline_round_trips_rational_with_weights() {
        // Rational spline: weights non-empty, length matches control_points.
        let geom = SplineGeometry {
            degree: 2,
            closed: false,
            knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            control_points: vec![[0.0, 0.0, 0.0], [1.0, 1.0, 0.0], [2.0, 0.0, 0.0]],
            weights: vec![1.0, 2.0, 1.0],
            fit_points: Vec::new(),
            start_tangent: [0.0, 0.0, 0.0],
            end_tangent: [0.0, 0.0, 0.0],
        };
        assert_eq!(round_trip(geom.clone()), geom);
    }

    #[test]
    fn spline_rejects_weights_length_mismatch() {
        let geom = SplineGeometry {
            degree: 2,
            closed: false,
            knots: Vec::new(),
            control_points: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
            // 2 control points but only 1 weight: caller bug.
            weights: vec![1.0],
            fit_points: Vec::new(),
            start_tangent: [0.0, 0.0, 0.0],
            end_tangent: [0.0, 0.0, 0.0],
        };
        let mut writer = BitWriter::new();
        let err = write_spline_geometry(&geom, &mut writer)
            .expect_err("mismatched weights must be rejected");
        match err {
            DwgWriteError::InvalidValue(msg) => assert!(
                msg.contains("weights") && msg.contains("control_points"),
                "error must surface both length sources; got `{msg}`"
            ),
            other => panic!("expected InvalidValue, got {other:?}"),
        }
    }
}
