use h7cad_native_model::CadDocument;

pub fn read_dwg(_bytes: &[u8]) -> Result<CadDocument, String> {
    Err("native DWG reader reserved for milestone 3".to_string())
}
