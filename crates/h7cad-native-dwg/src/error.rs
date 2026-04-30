use crate::DwgVersion;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DwgReadError {
    TruncatedHeader { expected_at_least: usize },
    InvalidMagic { found: String },
    UnsupportedVersion(DwgVersion),
    UnsupportedHeaderLayout { version: DwgVersion },
    TruncatedSectionDirectory {
        version: DwgVersion,
        expected_at_least: usize,
        actual: usize,
    },
    SectionOutOfBounds {
        index: u32,
        offset: usize,
        size: usize,
        actual: usize,
    },
    SemanticDecode {
        section_index: u32,
        record_index: u32,
        reason: String,
    },
    UnexpectedEof {
        context: &'static str,
    },
    /// AC1018 sub-brick decode failure (R46-A `encrypted_metadata`,
    /// R46-C `page_map`, R46-D `section_descriptor_map`, or R46-E1
    /// `section_payload`). `stage` identifies which brick raised the
    /// error; `reason` is the brick's own error rendered to a string.
    /// R46-E2 introduced this variant to bridge the per-brick error
    /// types into the top-level `DwgReadError` without forcing a
    /// shared error hierarchy across the AC1018 reader stack.
    Ac1018Decode {
        stage: &'static str,
        reason: String,
    },
}

impl fmt::Display for DwgReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TruncatedHeader { expected_at_least } => {
                write!(f, "truncated DWG header: expected at least {expected_at_least} bytes")
            }
            Self::InvalidMagic { found } => write!(f, "invalid DWG magic `{found}`"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported DWG version `{version}`")
            }
            Self::UnsupportedHeaderLayout { version } => {
                write!(f, "unsupported DWG header layout for version `{version}`")
            }
            Self::TruncatedSectionDirectory {
                version,
                expected_at_least,
                actual,
            } => write!(
                f,
                "truncated DWG section directory for `{version}`: expected at least {expected_at_least} bytes, got {actual}"
            ),
            Self::SectionOutOfBounds {
                index,
                offset,
                size,
                actual,
            } => write!(
                f,
                "DWG section {index} is out of bounds: offset {offset}, size {size}, file size {actual}"
            ),
            Self::SemanticDecode {
                section_index,
                record_index,
                reason,
            } => write!(
                f,
                "semantic decode failure in section {section_index} record {record_index}: {reason}"
            ),
            Self::UnexpectedEof { context } => write!(f, "unexpected EOF: {context}"),
            Self::Ac1018Decode { stage, reason } => {
                write!(f, "AC1018 {stage} decode failed: {reason}")
            }
        }
    }
}

impl std::error::Error for DwgReadError {}
