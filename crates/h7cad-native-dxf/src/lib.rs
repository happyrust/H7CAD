use h7cad_native_model::CadDocument;

pub fn read_dxf(_input: &str) -> Result<CadDocument, String> {
    Err("native DXF reader not implemented yet".to_string())
}

pub fn write_dxf(_doc: &CadDocument) -> Result<String, String> {
    Err("native DXF writer not implemented yet".to_string())
}
