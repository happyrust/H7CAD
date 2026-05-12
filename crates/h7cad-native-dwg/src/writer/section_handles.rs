//! AC1015 `AcDb:Handles` section composer (F5.M2.T3.3 → F5.M3).
//!
//! Reader contract (see [`crate::build_pending_document`]):
//! - empty payload → handle_offsets stays empty; reader skips silently.
//! - non-empty payload → fed to [`crate::parse_handle_map`] which
//!   decodes a delta-encoded `(handle, offset)` list.
//!
//! F5.M3 milestone: real chunk-based encoding via
//! [`super::handle_map::write_ac1015_handle_map_payload`]. Empty input
//! still yields the documented empty terminator chunk (`0x00 0x02`),
//! which is round-trippable but not byte-for-byte empty — the
//! [`super::write_dwg`] glue is updated in lockstep so the
//! `write_dwg_section_payloads_are_byte_for_byte_empty` invariant
//! continues to apply only to the *empty-input* case.

use crate::writer::handle_map::write_ac1015_handle_map_payload;
use crate::{DwgWriteError, HandleMapEntry};

/// Emit the AC1015 `AcDb:Handles` payload for the given handle map
/// entries.
///
/// Empty `entries` returns an empty `Vec` (zero bytes) so callers
/// that want a strictly empty section payload — currently the
/// `write_dwg(empty CadDocument)` tracer bullet — keep their
/// invariant. Non-empty `entries` flow through the M3 chunk encoder.
pub fn write_ac1015_handles_section(entries: &[HandleMapEntry]) -> Result<Vec<u8>, DwgWriteError> {
    if entries.is_empty() {
        return Ok(Vec::new());
    }
    write_ac1015_handle_map_payload(entries)
}
