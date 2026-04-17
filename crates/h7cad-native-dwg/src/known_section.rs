//! AC1015 (R2000) section locator record routing.
//!
//! Source of truth: ACadSharp `DwgSectionDefinition.GetSectionLocatorByName`.
//! AutoCAD writes a small fixed table of sections into every R2000 drawing:
//! each one is identified by an integer record number embedded in the
//! locator record. We mirror the same mapping so downstream decoders can
//! dispatch on section identity instead of guessing by position.

/// Well-known section identities for AC1015/AC1018 drawings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnownSection {
    /// `AcDb:Header` — CadHeader variables.
    Header,
    /// `AcDb:Classes` — class list / ObjectDBX registration table.
    Classes,
    /// `AcDb:Handles` — handle-to-offset map for the object stream.
    Handles,
    /// `AcDb:ObjFreeSpace` — free-space map, usually ignorable on read.
    ObjFreeSpace,
    /// `AcDb:Template` — template file hints.
    Template,
    /// `AcDb:AuxHeader` — secondary header snapshot for recovery.
    AuxHeader,
}

impl KnownSection {
    /// Map a section locator record number to its well-known identity,
    /// if known.
    pub fn from_record_number(n: u8) -> Option<Self> {
        match n {
            0 => Some(Self::Header),
            1 => Some(Self::Classes),
            2 => Some(Self::Handles),
            3 => Some(Self::ObjFreeSpace),
            4 => Some(Self::Template),
            5 => Some(Self::AuxHeader),
            _ => None,
        }
    }

    /// Canonical section name used in DWG ObjectDBX documentation
    /// (`AcDb:Header`, `AcDb:Classes`, ...).
    pub fn name(self) -> &'static str {
        match self {
            Self::Header => "AcDb:Header",
            Self::Classes => "AcDb:Classes",
            Self::Handles => "AcDb:Handles",
            Self::ObjFreeSpace => "AcDb:ObjFreeSpace",
            Self::Template => "AcDb:Template",
            Self::AuxHeader => "AcDb:AuxHeader",
        }
    }

    /// 16-byte start sentinel, where a canonical value exists.
    /// For sections without a documented start sentinel this returns
    /// `None`.
    pub fn start_sentinel(self) -> Option<[u8; 16]> {
        match self {
            Self::Header => Some([
                0xCF, 0x7B, 0x1F, 0x23, 0xFD, 0xDE, 0x38, 0xA9, 0x5F, 0x7C, 0x68, 0xB8, 0x4E, 0x6D,
                0x33, 0x5F,
            ]),
            Self::Classes => Some([
                0x8D, 0xA1, 0xC4, 0xB8, 0xC4, 0xA9, 0xF8, 0xC5, 0xC0, 0xDC, 0xF4, 0x5F, 0xE7, 0xCF,
                0xB6, 0x8A,
            ]),
            _ => None,
        }
    }

    /// 16-byte end sentinel, where a canonical value exists.
    pub fn end_sentinel(self) -> Option<[u8; 16]> {
        match self {
            Self::Header => Some([
                0x30, 0x84, 0xE0, 0xDC, 0x02, 0x21, 0xC7, 0x56, 0xA0, 0x83, 0x97, 0x47, 0xB1, 0x92,
                0xCC, 0xA0,
            ]),
            Self::Classes => Some([
                0x72, 0x5E, 0x3B, 0x47, 0x3B, 0x56, 0x07, 0x3A, 0x3F, 0x23, 0x0B, 0xA0, 0x18, 0x30,
                0x49, 0x75,
            ]),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_section_roundtrip_for_documented_numbers() {
        let cases = [
            (0_u8, KnownSection::Header, "AcDb:Header"),
            (1, KnownSection::Classes, "AcDb:Classes"),
            (2, KnownSection::Handles, "AcDb:Handles"),
            (3, KnownSection::ObjFreeSpace, "AcDb:ObjFreeSpace"),
            (4, KnownSection::Template, "AcDb:Template"),
            (5, KnownSection::AuxHeader, "AcDb:AuxHeader"),
        ];
        for (n, section, name) in cases {
            assert_eq!(KnownSection::from_record_number(n), Some(section));
            assert_eq!(section.name(), name);
        }
    }

    #[test]
    fn unknown_record_numbers_are_reported_as_none() {
        assert_eq!(KnownSection::from_record_number(6), None);
        assert_eq!(KnownSection::from_record_number(255), None);
    }

    #[test]
    fn start_sentinel_matches_header_and_classes() {
        assert!(KnownSection::Header.start_sentinel().is_some());
        assert!(KnownSection::Classes.start_sentinel().is_some());
        assert!(KnownSection::Handles.start_sentinel().is_none());
    }
}
