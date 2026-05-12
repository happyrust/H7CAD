//! AC1015 `AcDb:Header` section composer (F5.M2.T3.1).
//!
//! The reader's [`crate::build_pending_document`] feeds the Header
//! payload to [`crate::lib::classify_section_records_for_section`],
//! which short-circuits on empty payload (`Ok(Vec::new())`). For the
//! M2 milestone we therefore emit an empty payload — the smallest
//! possible byte sequence the reader still classifies as a valid
//! Header section. Real `CadHeader` variable serialisation lands in
//! F5.M5 once the entity body writers create demand for the supporting
//! header fields (`HANDSEED`, `INSBASE`, etc.).
//!
//! The 16-byte start/end sentinel from `KnownSection::Header` is
//! intentionally not emitted at this milestone. The native reader
//! ignores it; ACadSharp interoperability via `acadrust` is gated on
//! M6, when sentinel emission becomes part of a focused vertical TDD
//! slice.

use crate::DwgWriteError;
use h7cad_native_model::CadDocument;

/// Emit the minimal AC1015 `AcDb:Header` payload for the given
/// document. M2 milestone returns an empty `Vec`; future milestones
/// add real header-variable serialisation.
pub fn write_ac1015_header_section(_doc: &CadDocument) -> Result<Vec<u8>, DwgWriteError> {
    Ok(Vec::new())
}
