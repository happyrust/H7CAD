//! Integration tests for DXF HEADER drawing-environment variables
//! expanded in the 2026-04-21 header-drawing-vars plan.
//!
//! Covers read, write, round-trip, defaults, and legacy-file behaviour
//! for the 15 new variables: ORTHOMODE / GRIDMODE / SNAPMODE / FILLMODE
//! / MIRRTEXT / ATTMODE / CLAYER / CECOLOR / CELTYPE / CELWEIGHT /
//! CELTSCALE / CETRANSPARENCY / ANGBASE / ANGDIR / PSLTSCALE.

use h7cad_native_dxf::{read_dxf, write_dxf};
use h7cad_native_model::CadDocument;

fn dxf_with_all_drawing_vars() -> String {
    concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  9\n$ORTHOMODE\n 70\n     1\n",
        "  9\n$GRIDMODE\n 70\n     1\n",
        "  9\n$SNAPMODE\n 70\n     1\n",
        "  9\n$FILLMODE\n 70\n     0\n",
        "  9\n$MIRRTEXT\n 70\n     1\n",
        "  9\n$ATTMODE\n 70\n     2\n",
        "  9\n$CLAYER\n  8\nMyLayer\n",
        "  9\n$CECOLOR\n 62\n     3\n",
        "  9\n$CELTYPE\n  6\nDASHED\n",
        "  9\n$CELWEIGHT\n370\n    25\n",
        "  9\n$CELTSCALE\n 40\n2.5\n",
        "  9\n$CETRANSPARENCY\n440\n128\n",
        "  9\n$ANGBASE\n 50\n1.5707963267948966\n",
        "  9\n$ANGDIR\n 70\n     1\n",
        "  9\n$PSLTSCALE\n 70\n     0\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    )
    .to_string()
}

#[test]
fn header_reads_all_15_drawing_vars() {
    let doc = read_dxf(&dxf_with_all_drawing_vars()).expect("parse succeeds");

    assert!(doc.header.orthomode);
    assert!(doc.header.gridmode);
    assert!(doc.header.snapmode);
    assert!(!doc.header.fillmode);
    assert!(doc.header.mirrtext);
    assert_eq!(doc.header.attmode, 2);

    assert_eq!(doc.header.clayer, "MyLayer");
    assert_eq!(doc.header.cecolor, 3);
    assert_eq!(doc.header.celtype, "DASHED");
    assert_eq!(doc.header.celweight, 25);
    assert!((doc.header.celtscale - 2.5).abs() < 1e-12);
    assert_eq!(doc.header.cetransparency, 128);

    assert!((doc.header.angbase - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    assert!(doc.header.angdir);

    assert!(!doc.header.psltscale);
}

#[test]
fn header_writes_all_15_drawing_vars() {
    let mut doc = CadDocument::new();
    doc.header.orthomode = true;
    doc.header.gridmode = true;
    doc.header.snapmode = true;
    doc.header.fillmode = false;
    doc.header.mirrtext = true;
    doc.header.attmode = 2;
    doc.header.clayer = "LayerZ".into();
    doc.header.cecolor = 5;
    doc.header.celtype = "CENTER".into();
    doc.header.celweight = 50;
    doc.header.celtscale = 3.25;
    doc.header.cetransparency = 99;
    doc.header.angbase = 1.0;
    doc.header.angdir = true;
    doc.header.psltscale = false;

    let text = write_dxf(&doc).expect("write_dxf");

    assert_var_i16(&text, "$ORTHOMODE", 70, 1);
    assert_var_i16(&text, "$GRIDMODE", 70, 1);
    assert_var_i16(&text, "$SNAPMODE", 70, 1);
    assert_var_i16(&text, "$FILLMODE", 70, 0);
    assert_var_i16(&text, "$MIRRTEXT", 70, 1);
    assert_var_i16(&text, "$ATTMODE", 70, 2);

    assert_var_str(&text, "$CLAYER", 8, "LayerZ");
    assert_var_i16(&text, "$CECOLOR", 62, 5);
    assert_var_str(&text, "$CELTYPE", 6, "CENTER");
    assert_var_i16(&text, "$CELWEIGHT", 370, 50);
    assert_var_f64_approx(&text, "$CELTSCALE", 40, 3.25);
    assert_var_i32(&text, "$CETRANSPARENCY", 440, 99);

    assert_var_f64_approx(&text, "$ANGBASE", 50, 1.0);
    assert_var_i16(&text, "$ANGDIR", 70, 1);

    assert_var_i16(&text, "$PSLTSCALE", 70, 0);
}

#[test]
fn header_roundtrip_preserves_all_15_drawing_vars() {
    let doc1 = read_dxf(&dxf_with_all_drawing_vars()).expect("first read");
    let text = write_dxf(&doc1).expect("write");
    let doc2 = read_dxf(&text).expect("second read");

    assert_eq!(doc1.header.orthomode, doc2.header.orthomode);
    assert_eq!(doc1.header.gridmode, doc2.header.gridmode);
    assert_eq!(doc1.header.snapmode, doc2.header.snapmode);
    assert_eq!(doc1.header.fillmode, doc2.header.fillmode);
    assert_eq!(doc1.header.mirrtext, doc2.header.mirrtext);
    assert_eq!(doc1.header.attmode, doc2.header.attmode);
    assert_eq!(doc1.header.clayer, doc2.header.clayer);
    assert_eq!(doc1.header.cecolor, doc2.header.cecolor);
    assert_eq!(doc1.header.celtype, doc2.header.celtype);
    assert_eq!(doc1.header.celweight, doc2.header.celweight);
    // Tolerance = 1e-9 matches format_f64's 10-decimal precision ceiling
    // (AutoCAD's own textual DXF round-trip has similar drift).
    assert!((doc1.header.celtscale - doc2.header.celtscale).abs() < 1e-9);
    assert_eq!(doc1.header.cetransparency, doc2.header.cetransparency);
    assert!((doc1.header.angbase - doc2.header.angbase).abs() < 1e-9);
    assert_eq!(doc1.header.angdir, doc2.header.angdir);
    assert_eq!(doc1.header.psltscale, doc2.header.psltscale);
}

#[test]
fn header_default_values_survive_roundtrip() {
    let doc = CadDocument::new();
    let text = write_dxf(&doc).expect("write default doc");
    let restored = read_dxf(&text).expect("read back default doc");

    assert!(!restored.header.orthomode, "default orthomode = false");
    assert!(!restored.header.gridmode);
    assert!(!restored.header.snapmode);
    assert!(restored.header.fillmode, "default fillmode = true (AutoCAD convention)");
    assert!(!restored.header.mirrtext);
    assert_eq!(restored.header.attmode, 1, "default attmode = 1 (normal)");
    assert_eq!(restored.header.clayer, "0", "default current layer");
    assert_eq!(restored.header.cecolor, 256, "default BYLAYER (256)");
    assert_eq!(restored.header.celtype, "ByLayer");
    assert_eq!(restored.header.celweight, -1, "default ByLayer (-1)");
    assert!((restored.header.celtscale - 1.0).abs() < 1e-12);
    assert_eq!(restored.header.cetransparency, 0);
    assert!((restored.header.angbase - 0.0).abs() < 1e-12);
    assert!(!restored.header.angdir);
    assert!(restored.header.psltscale, "default psltscale = true");
}

#[test]
fn header_legacy_file_without_new_vars_loads_with_defaults() {
    // Legacy DXF: only the original 4-var HEADER the writer used to
    // emit before this plan. All 15 new variables are absent; reader
    // must fall back to DocumentHeader defaults silently.
    let legacy = concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  9\n$LTSCALE\n 40\n1.0\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );
    let doc = read_dxf(legacy).expect("legacy parse");

    // Sanity: legacy fields still work.
    assert!((doc.header.ltscale - 1.0).abs() < 1e-12);

    // All 15 new fields must equal DocumentHeader::default().
    let def = h7cad_native_model::DocumentHeader::default();
    assert_eq!(doc.header.orthomode, def.orthomode);
    assert_eq!(doc.header.gridmode, def.gridmode);
    assert_eq!(doc.header.snapmode, def.snapmode);
    assert_eq!(doc.header.fillmode, def.fillmode);
    assert_eq!(doc.header.mirrtext, def.mirrtext);
    assert_eq!(doc.header.attmode, def.attmode);
    assert_eq!(doc.header.clayer, def.clayer);
    assert_eq!(doc.header.cecolor, def.cecolor);
    assert_eq!(doc.header.celtype, def.celtype);
    assert_eq!(doc.header.celweight, def.celweight);
    assert_eq!(doc.header.celtscale, def.celtscale);
    assert_eq!(doc.header.cetransparency, def.cetransparency);
    assert_eq!(doc.header.angbase, def.angbase);
    assert_eq!(doc.header.angdir, def.angdir);
    assert_eq!(doc.header.psltscale, def.psltscale);
}

// ---------------------------------------------------------------------------
// Assertion helpers
// ---------------------------------------------------------------------------

/// Scan `text` for a `  9\n<name>\n` header-var line and check that the
/// immediately following group-code pair matches `(expected_code,
/// expected_value_trimmed)`.
fn find_var_pair<'a>(text: &'a str, var_name: &str) -> (&'a str, &'a str) {
    let lines: Vec<&str> = text.lines().collect();
    for i in 0..lines.len().saturating_sub(3) {
        if lines[i].trim() == "9" && lines[i + 1].trim() == var_name {
            return (lines[i + 2].trim(), lines[i + 3].trim());
        }
    }
    panic!("HEADER variable {var_name} not found in output");
}

fn assert_var_i16(text: &str, var: &str, code: i16, expected: i16) {
    let (got_code, got_val) = find_var_pair(text, var);
    assert_eq!(
        got_code,
        &code.to_string(),
        "{var} group code mismatch (expected {code}, got `{got_code}`)"
    );
    let parsed: i16 = got_val.parse().unwrap_or_else(|_| {
        panic!("{var} value `{got_val}` is not parseable as i16")
    });
    assert_eq!(
        parsed, expected,
        "{var} value mismatch: got {parsed}, expected {expected}"
    );
}

fn assert_var_i32(text: &str, var: &str, code: i16, expected: i32) {
    let (got_code, got_val) = find_var_pair(text, var);
    assert_eq!(
        got_code,
        &code.to_string(),
        "{var} group code mismatch (expected {code}, got `{got_code}`)"
    );
    let parsed: i32 = got_val.parse().unwrap_or_else(|_| {
        panic!("{var} value `{got_val}` is not parseable as i32")
    });
    assert_eq!(parsed, expected, "{var} value mismatch");
}

fn assert_var_f64_approx(text: &str, var: &str, code: i16, expected: f64) {
    let (got_code, got_val) = find_var_pair(text, var);
    assert_eq!(
        got_code,
        &code.to_string(),
        "{var} group code mismatch (expected {code}, got `{got_code}`)"
    );
    let parsed: f64 = got_val.parse().unwrap_or_else(|_| {
        panic!("{var} value `{got_val}` is not parseable as f64")
    });
    assert!(
        (parsed - expected).abs() < 1e-9,
        "{var} value mismatch: got {parsed}, expected {expected}"
    );
}

fn assert_var_str(text: &str, var: &str, code: i16, expected: &str) {
    let (got_code, got_val) = find_var_pair(text, var);
    assert_eq!(
        got_code,
        &code.to_string(),
        "{var} group code mismatch (expected {code}, got `{got_code}`)"
    );
    assert_eq!(got_val, expected, "{var} value mismatch");
}
