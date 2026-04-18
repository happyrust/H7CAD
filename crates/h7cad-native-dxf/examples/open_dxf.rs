use h7cad_native_dxf::read_dxf_bytes;
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let path = match env::args().nth(1) {
        Some(p) => PathBuf::from(p),
        None => {
            eprintln!("usage: open_dxf <path-to-dxf>");
            return ExitCode::from(2);
        }
    };
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("failed to read {}: {}", path.display(), e);
            return ExitCode::from(1);
        }
    };
    println!("file: {} ({} bytes)", path.display(), bytes.len());

    let doc = match read_dxf_bytes(&bytes) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("DXF parse failed: {e}");
            return ExitCode::from(1);
        }
    };

    println!("version   : {:?}", doc.header.version);
    println!("layers    : {}", doc.tables.layer.entries.len());
    println!("blocks    : {}", doc.tables.block_record.entries.len());
    println!("entities  : {}", doc.entities.len());

    let mut counts = std::collections::BTreeMap::<&'static str, usize>::new();
    for e in &doc.entities {
        let name = match &e.data {
            h7cad_native_model::EntityData::Line { .. } => "LINE",
            h7cad_native_model::EntityData::Circle { .. } => "CIRCLE",
            h7cad_native_model::EntityData::Arc { .. } => "ARC",
            h7cad_native_model::EntityData::Text { .. } => "TEXT",
            h7cad_native_model::EntityData::LwPolyline { .. } => "LWPOLYLINE",
            h7cad_native_model::EntityData::Polyline { .. } => "POLYLINE",
            h7cad_native_model::EntityData::Ellipse { .. } => "ELLIPSE",
            h7cad_native_model::EntityData::Point { .. } => "POINT",
            h7cad_native_model::EntityData::Insert { .. } => "INSERT",
            h7cad_native_model::EntityData::Spline { .. } => "SPLINE",
            h7cad_native_model::EntityData::Hatch { .. } => "HATCH",
            h7cad_native_model::EntityData::Unknown { .. } => "UNKNOWN",
            _ => "OTHER",
        };
        *counts.entry(name).or_insert(0) += 1;
    }
    for (k, v) in &counts {
        println!("  {k:<10}: {v}");
    }

    println!("OK: DXF opened successfully");
    ExitCode::SUCCESS
}
