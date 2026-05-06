//! Three-way color policy mirroring ODA's `ColorPolicy` device property.
//!
//! Shared by SVG export (R40) and PDF export (R41) so the two pipelines
//! agree byte-for-byte on what `Full` / `Monochrome` / `Grayscale` mean.
//!
//! Per-format helpers (`effective_color_policy` for SVG,
//! `effective_color_policy_pdf` for PDF) live next to their respective
//! `*ExportOptions` structs because they read distinct option fields, but
//! both call into [`apply_color_policy`] below for the actual RGB mapping.

/// Three-way color policy mirroring ODA's `ColorPolicy` device property:
///
/// | ODA value | H7CAD variant | Behaviour |
/// |-----------|---------------|-----------|
/// | `0`       | `Full`        | Colors are unchanged |
/// | `1`       | `Monochrome`  | All strokes forced to black (DEFAULT) |
/// | `2`       | `Grayscale`   | Every color luminance-mapped to gray |
///
/// Ordering of `Default` is deliberate: `Monochrome` keeps byte-level parity
/// with every pre-R40 SVG export and pre-R41 PDF export.  `Grayscale` is the
/// missing third arm filled in by R40 (SVG) / R41 (PDF).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorPolicy {
    /// Keep ACI / true-color strokes (ODA `ColorPolicy=0`).
    Full,
    /// Force all strokes to black (ODA `ColorPolicy=1` — default).
    #[default]
    Monochrome,
    /// Convert every color to gray via luminance `0.299R + 0.587G + 0.114B`
    /// (ODA `ColorPolicy=2`).
    Grayscale,
}

/// Map an RGB triplet through the resolved [`ColorPolicy`].
///
/// * `Full` returns the input unchanged.
/// * `Monochrome` collapses to `(0.0, 0.0, 0.0)`.
/// * `Grayscale` applies ITU-R BT.601 luminance (`0.299R + 0.587G + 0.114B`)
///   to produce a perceptually correct gray ramp.
///
/// BT.601 (vs. BT.709) is chosen because engineering drawings are usually
/// printed in black and white where the BT.601 weights (red 76 / green 150 /
/// blue 29 out of 255) match the legacy AutoCAD / PlotStyle CTB grayscale
/// behaviour, so a `Monochrome → Grayscale` switch is a smooth gradient
/// rather than a perceptual jump.
#[inline]
pub fn apply_color_policy(policy: ColorPolicy, r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    match policy {
        ColorPolicy::Full => (r, g, b),
        ColorPolicy::Monochrome => (0.0, 0.0, 0.0),
        ColorPolicy::Grayscale => {
            let y = 0.299 * r + 0.587 * g + 0.114 * b;
            (y, y, y)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_monochrome() {
        assert_eq!(ColorPolicy::default(), ColorPolicy::Monochrome);
    }

    #[test]
    fn full_is_identity() {
        let (r, g, b) = apply_color_policy(ColorPolicy::Full, 0.7, 0.3, 0.9);
        assert_eq!((r, g, b), (0.7, 0.3, 0.9));
    }

    #[test]
    fn monochrome_collapses_to_black() {
        let (r, g, b) = apply_color_policy(ColorPolicy::Monochrome, 0.9, 0.5, 0.2);
        assert_eq!((r, g, b), (0.0, 0.0, 0.0));
        let (r, g, b) = apply_color_policy(ColorPolicy::Monochrome, 1.0, 1.0, 1.0);
        assert_eq!((r, g, b), (0.0, 0.0, 0.0));
    }

    #[test]
    fn grayscale_uses_bt601_luminance() {
        let (r, _, _) = apply_color_policy(ColorPolicy::Grayscale, 1.0, 0.0, 0.0);
        assert!((r - 0.299).abs() < 1e-6, "pure red luminance must be 0.299");
        let (r, _, _) = apply_color_policy(ColorPolicy::Grayscale, 0.0, 1.0, 0.0);
        assert!((r - 0.587).abs() < 1e-6, "pure green luminance must be 0.587");
        let (r, _, _) = apply_color_policy(ColorPolicy::Grayscale, 0.0, 0.0, 1.0);
        assert!((r - 0.114).abs() < 1e-6, "pure blue luminance must be 0.114");
        let (r, _, _) = apply_color_policy(ColorPolicy::Grayscale, 1.0, 1.0, 1.0);
        assert!((r - 1.0).abs() < 1e-6, "white must stay white");
        let (r, _, _) = apply_color_policy(ColorPolicy::Grayscale, 0.0, 0.0, 0.0);
        assert!(r.abs() < 1e-6, "black must stay black");
    }

    #[test]
    fn grayscale_returns_three_equal_components() {
        let (r, g, b) = apply_color_policy(ColorPolicy::Grayscale, 0.4, 0.6, 0.2);
        assert_eq!(r, g);
        assert_eq!(g, b);
    }

    #[test]
    fn json_roundtrip_three_variants() {
        // snake_case rename matches the SVG R40 contract used by --options JSON.
        let p: ColorPolicy = serde_json::from_str("\"full\"").unwrap();
        assert_eq!(p, ColorPolicy::Full);
        let p: ColorPolicy = serde_json::from_str("\"monochrome\"").unwrap();
        assert_eq!(p, ColorPolicy::Monochrome);
        let p: ColorPolicy = serde_json::from_str("\"grayscale\"").unwrap();
        assert_eq!(p, ColorPolicy::Grayscale);
    }
}
