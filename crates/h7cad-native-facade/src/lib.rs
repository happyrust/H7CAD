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

pub fn load(format: NativeFormat, bytes: &[u8]) -> Result<CadDocument, String> {
    match format {
        NativeFormat::Dxf => h7cad_native_dxf::read_dxf_bytes(bytes).map_err(|e| e.to_string()),
        NativeFormat::Dwg => h7cad_native_dwg::read_dwg(bytes).map_err(|e| e.to_string()),
    }
}

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
        let err = load(NativeFormat::Dwg, b"AC1015")
            .expect_err("DWG runtime load on a 6-byte signature must surface an error");
        assert_ne!(
            err, "native DWG reader not implemented yet",
            "facade DWG load should no longer return the legacy placeholder"
        );
        assert!(!err.is_empty(), "DWG reader error message should be non-empty");
    }

    #[test]
    fn dwg_runtime_save_is_unavailable() {
        let doc = CadDocument::new();
        let err = save(NativeFormat::Dwg, &doc)
            .expect_err("DWG runtime save should remain unavailable on the facade");
        assert_eq!(err, "native DWG writer not implemented yet");
    }
}
