//! AC1015 `AcDb:Template` section composer (F5.M2.T3.5).
//!
//! Template carries optional template-file hints; the reader does not
//! depend on it for the AC1015 baseline pipeline. Empty payload is
//! the defensible minimum and is emitted infallibly.

/// Emit the minimal AC1015 `AcDb:Template` payload (empty).
pub fn write_ac1015_template_section() -> Vec<u8> {
    Vec::new()
}
