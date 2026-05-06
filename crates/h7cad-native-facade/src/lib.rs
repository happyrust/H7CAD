//! Unified load/save entry for native CAD formats (DXF + DWG read; DWG write planned).
//!
//! DWG read delegates to [`h7cad_native_dwg::read_dwg`]. DWG write is tracked in
//! repository file `docs/DEVELOPMENT-PLAN.md` (phase P2).

use h7cad_native_model::CadDocument;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeFormat {
    Dxf,
    Dwg,
}

/// Read a CAD document from in-memory bytes through the native facade.
///
/// Both DXF and DWG paths return a `h7cad_native_model::CadDocument`
/// directly. The DWG arm bridges to `h7cad_native_dwg::read_dwg` —
/// today that crate covers AC1015 only and rejects everything else as
/// `UnsupportedVersion` / `UnsupportedHeaderLayout`; runtime callers
/// that need wider coverage continue to use the acadrust path through
/// `src/io::load_file_native_blocking`. See `docs/plans/
/// 2026-04-28-r48-facade-and-build-cleanup-plan.md` for the rationale.
pub fn load(format: NativeFormat, bytes: &[u8]) -> Result<CadDocument, String> {
    match format {
        NativeFormat::Dxf => h7cad_native_dxf::read_dxf_bytes(bytes).map_err(|e| e.to_string()),
        NativeFormat::Dwg => h7cad_native_dwg::read_dwg(bytes).map_err(|e| e.to_string()),
    }
}

/// Serialize a CAD document back to bytes through the native facade.
///
/// DXF writes through `h7cad_native_dxf::write_dxf`. DWG **save** is
/// intentionally unimplemented: there is no native DWG writer yet, and
/// the facade explicitly surfaces that gap rather than silently falling
/// back to acadrust. Production callers wishing to write DWG must use
/// `src/io::save_dwg`, which routes through `acadrust::DwgWriter`.
pub fn save(format: NativeFormat, doc: &CadDocument) -> Result<Vec<u8>, String> {
    match format {
        NativeFormat::Dxf => {
            let text = h7cad_native_dxf::write_dxf(doc)?;
            Ok(text.into_bytes())
        }
        NativeFormat::Dwg => Err("native DWG writer not implemented yet".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::{load, save, NativeFormat};
    use h7cad_native_model::CadDocument;

    #[test]
    fn dwg_runtime_load_rejects_truncated_signature_with_real_error() {
        // R48-DWG-FACADE-AND-BUILD: facade DWG load is now wired through
        // `h7cad_native_dwg::read_dwg`, so a short / malformed signature
        // surfaces a real reader error rather than the legacy
        // "not implemented yet" placeholder. The exact message text is
        // implementation-defined; we only assert the placeholder string
        // is gone and a non-empty error came back.
        let err = load(NativeFormat::Dwg, b"AC1015")
            .expect_err("DWG runtime load on a 6-byte signature must surface an error");
        assert_ne!(
            err, "native DWG reader not implemented yet",
            "facade DWG load should no longer return the legacy placeholder"
        );
        assert!(
            !err.is_empty(),
            "DWG reader error message should be non-empty"
        );
    }

    #[test]
    fn dwg_runtime_save_is_unavailable() {
        // No native DWG writer yet (see module-level docs for the rationale);
        // facade `save` must keep surfacing that gap explicitly.
        let doc = CadDocument::new();
        let err = save(NativeFormat::Dwg, &doc)
            .expect_err("DWG runtime save should remain unavailable on the facade");
        assert_eq!(err, "native DWG writer not implemented yet");
    }
}
