use h7cad_native_model::CadDocument;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeFormat {
    Dxf,
    Dwg,
}

pub fn load(format: NativeFormat, bytes: &[u8]) -> Result<CadDocument, String> {
    match format {
        NativeFormat::Dxf => h7cad_native_dxf::read_dxf_bytes(bytes).map_err(|e| e.to_string()),
        NativeFormat::Dwg => Err("native DWG reader not implemented yet".to_string()),
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
    fn dwg_runtime_load_is_unavailable() {
        let err = load(NativeFormat::Dwg, b"AC1015")
            .expect_err("DWG runtime load should remain unavailable on the facade");
        assert_eq!(err, "native DWG reader not implemented yet");
    }
}
