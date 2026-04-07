use h7cad_native_model::CadDocument;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeFormat {
    Dxf,
    Dwg,
}

pub fn load(_format: NativeFormat, _bytes: &[u8]) -> Result<CadDocument, String> {
    Err("native facade not implemented yet".to_string())
}

pub fn save(_format: NativeFormat, _doc: &CadDocument) -> Result<Vec<u8>, String> {
    Err("native facade not implemented yet".to_string())
}
