use h7cad_native_model::CadDocument;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeFormat {
    Dxf,
    Dwg,
}

pub fn load(format: NativeFormat, bytes: &[u8]) -> Result<CadDocument, String> {
    match format {
        NativeFormat::Dxf => {
            let text = std::str::from_utf8(bytes)
                .map_err(|e| format!("invalid UTF-8 in DXF: {e}"))?;
            h7cad_native_dxf::read_dxf(text).map_err(|e| e.to_string())
        }
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
