//! H7CAD Command User Interface (CUI) persistence.
//!
//! Pure serialize/parse helpers for the user-defined command aliases and
//! function-key shortcut overrides.  The file format is a small, hand-
//! written text schema with two `[section]` blocks — chosen over the
//! AutoCAD `.cuix` XML/ZIP format because H7CAD's runtime only needs to
//! round-trip the two maps that `ALIASEDIT` and the shortcut editor
//! already maintain in memory.
//!
//! Format example:
//!
//! ```text
//! # H7CAD CUI v1
//! [aliases]
//! L=LINE
//! CO=COPY
//! [shortcuts]
//! F3=SNAPOFF
//! F7=GRID
//! ```
//!
//! Blank lines and `#` comments are ignored.  Unknown sections are ignored
//! on import (forward compatibility).  Keys and values are trimmed; the
//! serializer always writes keys sorted alphabetically for stable diffs.

use std::collections::HashMap;

/// The two maps that the runtime persists.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CuiDocument {
    pub aliases: HashMap<String, String>,
    pub shortcuts: HashMap<String, String>,
}

/// Serialize the two maps into the H7CAD CUI text schema.
///
/// Keys within each section are written sorted for deterministic output.
pub fn serialize_cui(doc: &CuiDocument) -> String {
    let mut out = String::new();
    out.push_str("# H7CAD CUI v1\n");
    out.push_str("[aliases]\n");
    let mut alias_rows: Vec<(&String, &String)> = doc.aliases.iter().collect();
    alias_rows.sort_by(|a, b| a.0.cmp(b.0));
    for (k, v) in alias_rows {
        out.push_str(k);
        out.push('=');
        out.push_str(v);
        out.push('\n');
    }
    out.push_str("[shortcuts]\n");
    let mut sc_rows: Vec<(&String, &String)> = doc.shortcuts.iter().collect();
    sc_rows.sort_by(|a, b| a.0.cmp(b.0));
    for (k, v) in sc_rows {
        out.push_str(k);
        out.push('=');
        out.push_str(v);
        out.push('\n');
    }
    out
}

/// Parse a CUI text body into a `CuiDocument`.
///
/// Returns `Err` only on catastrophic format errors (currently none —
/// malformed lines are silently ignored so partial user edits do not
/// abort a load).  Instead, the caller can inspect resulting map sizes.
pub fn parse_cui(text: &str) -> Result<CuiDocument, String> {
    let mut doc = CuiDocument::default();
    #[derive(Copy, Clone)]
    enum Section { None, Aliases, Shortcuts, Unknown }
    let mut section = Section::None;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            let name = &line[1..line.len() - 1].trim().to_ascii_lowercase();
            section = match name.as_str() {
                "aliases" => Section::Aliases,
                "shortcuts" => Section::Shortcuts,
                _ => Section::Unknown,
            };
            continue;
        }
        let Some(eq) = line.find('=') else { continue };
        let key = line[..eq].trim().to_string();
        let val = line[eq + 1..].trim().to_string();
        if key.is_empty() {
            continue;
        }
        match section {
            Section::Aliases => {
                doc.aliases.insert(key, val);
            }
            Section::Shortcuts => {
                doc.shortcuts.insert(key, val);
            }
            Section::None | Section::Unknown => {}
        }
    }
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_doc() -> CuiDocument {
        let mut aliases = HashMap::new();
        aliases.insert("L".to_string(), "LINE".to_string());
        aliases.insert("CO".to_string(), "COPY".to_string());
        let mut shortcuts = HashMap::new();
        shortcuts.insert("F3".to_string(), "SNAPOFF".to_string());
        CuiDocument { aliases, shortcuts }
    }

    #[test]
    fn round_trip_preserves_all_entries() {
        let doc = sample_doc();
        let text = serialize_cui(&doc);
        let back = parse_cui(&text).expect("parse ok");
        assert_eq!(back, doc);
    }

    #[test]
    fn serialize_sorts_keys() {
        let doc = sample_doc();
        let text = serialize_cui(&doc);
        // Aliases CO before L (alphabetical).
        let co_pos = text.find("CO=COPY").unwrap();
        let l_pos = text.find("L=LINE").unwrap();
        assert!(co_pos < l_pos, "CO should come before L");
    }

    #[test]
    fn parse_ignores_blanks_and_comments() {
        let text = r#"
# comment 1
[aliases]

L=LINE
# inline comment line
CO=COPY

[shortcuts]
F3=SNAPOFF
"#;
        let doc = parse_cui(text).expect("parse ok");
        assert_eq!(doc.aliases.len(), 2);
        assert_eq!(doc.aliases.get("L").map(String::as_str), Some("LINE"));
        assert_eq!(doc.shortcuts.get("F3").map(String::as_str), Some("SNAPOFF"));
    }

    #[test]
    fn parse_ignores_unknown_sections() {
        let text = "[aliases]\nL=LINE\n[futureblock]\nx=y\n[shortcuts]\nF3=SNAPOFF\n";
        let doc = parse_cui(text).expect("parse ok");
        assert_eq!(doc.aliases.len(), 1);
        assert_eq!(doc.shortcuts.len(), 1);
    }

    #[test]
    fn parse_tolerates_malformed_lines() {
        let text = "[aliases]\nthis line has no equal sign\nL=LINE\n=novalue\n";
        let doc = parse_cui(text).expect("parse ok");
        // Only `L=LINE` survives; the other two malformed lines dropped.
        assert_eq!(doc.aliases.len(), 1);
        assert_eq!(doc.aliases.get("L").map(String::as_str), Some("LINE"));
    }

    #[test]
    fn parse_trims_whitespace_around_key_value() {
        let text = "[aliases]\n  L  =  LINE  \n";
        let doc = parse_cui(text).expect("parse ok");
        assert_eq!(doc.aliases.get("L").map(String::as_str), Some("LINE"));
    }

    #[test]
    fn empty_doc_round_trip() {
        let doc = CuiDocument::default();
        let text = serialize_cui(&doc);
        assert!(text.contains("[aliases]"));
        assert!(text.contains("[shortcuts]"));
        let back = parse_cui(&text).expect("parse ok");
        assert_eq!(back.aliases.len(), 0);
        assert_eq!(back.shortcuts.len(), 0);
    }
}
