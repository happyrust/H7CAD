//! AC1015 `AcDb:ObjFreeSpace` section composer (F5.M2.T3.4).
//!
//! The reader treats the ObjFreeSpace payload as opaque — it routes
//! through [`crate::lib::classify_section_records_for_section`] and
//! is generally ignored on read. Empty payload is therefore the
//! defensible minimum and is emitted infallibly.

/// Emit the minimal AC1015 `AcDb:ObjFreeSpace` payload (empty).
pub fn write_ac1015_obj_free_space_section() -> Vec<u8> {
    Vec::new()
}
