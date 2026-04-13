use crate::DwgReadError;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DwgVersion {
    Ac1012,
    Ac1014,
    Ac1015,
    Ac1018,
    Ac1021,
    Ac1024,
    Ac1027,
    Ac1032,
}

impl DwgVersion {
    pub fn from_magic(magic: &str) -> Result<Self, DwgReadError> {
        match magic {
            "AC1012" => Ok(Self::Ac1012),
            "AC1014" => Ok(Self::Ac1014),
            "AC1015" => Ok(Self::Ac1015),
            "AC1018" => Ok(Self::Ac1018),
            "AC1021" => Ok(Self::Ac1021),
            "AC1024" => Ok(Self::Ac1024),
            "AC1027" => Ok(Self::Ac1027),
            "AC1032" => Ok(Self::Ac1032),
            _ => Err(DwgReadError::InvalidMagic {
                found: magic.to_string(),
            }),
        }
    }
}

impl fmt::Display for DwgVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let magic = match self {
            Self::Ac1012 => "AC1012",
            Self::Ac1014 => "AC1014",
            Self::Ac1015 => "AC1015",
            Self::Ac1018 => "AC1018",
            Self::Ac1021 => "AC1021",
            Self::Ac1024 => "AC1024",
            Self::Ac1027 => "AC1027",
            Self::Ac1032 => "AC1032",
        };
        f.write_str(magic)
    }
}
