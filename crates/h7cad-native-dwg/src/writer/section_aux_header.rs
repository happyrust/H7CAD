//! AC1015 `AcDb:AuxHeader` section composer (F5.M2.T3.6).
//!
//! AuxHeader is a recovery snapshot the reader does not require for
//! the baseline read path. Empty payload is the defensible minimum
//! and is emitted infallibly.

/// Emit the minimal AC1015 `AcDb:AuxHeader` payload (empty).
pub fn write_ac1015_aux_header_section() -> Vec<u8> {
    Vec::new()
}
