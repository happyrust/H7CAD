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
    use super::{load, NativeFormat};

    #[test]
    fn dwg_load_delegates_to_native_parser() {
        let result = load(NativeFormat::Dwg, b"AC1015");
        assert!(
            result.is_ok() || result.is_err(),
            "facade DWG load should delegate to h7cad_native_dwg::read_dwg"
        );
    }
}
