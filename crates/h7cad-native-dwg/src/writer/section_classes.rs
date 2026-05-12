//! AC1015 `AcDb:Classes` section composer (F5.M2.T3.2).
//!
//! Mirrors [`section_header`](super::section_header) at this milestone:
//! emit an empty payload, deferring `DxfClass` serialisation to
//! F5.M5 when entity bodies that reference custom classes
//! (`AcDbHatch` etc.) come online.

use crate::DwgWriteError;
use h7cad_native_model::CadDocument;

/// Emit the minimal AC1015 `AcDb:Classes` payload.
pub fn write_ac1015_classes_section(_doc: &CadDocument) -> Result<Vec<u8>, DwgWriteError> {
    Ok(Vec::new())
}
