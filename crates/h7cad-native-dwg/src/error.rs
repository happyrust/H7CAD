use crate::DwgVersion;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DwgReadError {
    TruncatedHeader {
        expected_at_least: usize,
    },
    InvalidMagic {
        found: String,
    },
    UnsupportedVersion(DwgVersion),
    UnsupportedHeaderLayout {
        version: DwgVersion,
    },
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

/// Errors emitted by the DWG writer pipeline.
///
/// F5.M1 introduces this as the writer-side counterpart to
/// [`DwgReadError`]. The variants mirror the conceptual failure modes
/// a DWG serialiser can hit. The bit-level primitives in
/// [`crate::BitWriter`] only surface [`Self::InvalidValue`] today; the
/// other variants are introduced in advance so downstream callers
/// (section composer in F5.M2, handle map writer in F5.M3, entity body
/// writers in F5.M5) can pattern match instead of shoehorning errors
/// into [`Self::InvalidValue`] strings.
///
/// See `docs/plans/2026-05-08-dwg-next-step-plan.md` §F5 for the
/// staged writer roadmap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DwgWriteError {
    /// A primitive value cannot be encoded with the requested DWG
    /// type. Examples: `BitShort` overflow when value is outside
    /// `i16::MIN..=i16::MAX`, ASCII text containing non-ASCII bytes,
    /// `BitLongLong` raw byte length > 8.
    InvalidValue(String),
    /// The document is internally inconsistent in a way that prevents
    /// serialisation (e.g. an `INSERT` references a `BlockRecord` that
    /// cannot be located in the symbol table).
    InvalidDocument(String),
    /// A DWG version or feature is intentionally not implemented yet
    /// (e.g. AC1018 writer before F5.M7, encrypted page header
    /// emission, R2007 string-stream writer).
    Unsupported(String),
    /// A composed section payload exceeds the on-disk limits a given
    /// version can describe in its section locator. F5.M2 onwards may
    /// surface this when an entity payload becomes pathologically
    /// large.
    SectionTooLarge {
        section: &'static str,
        bytes: usize,
        limit: usize,
    },
    /// Low-level I/O failure surfaced from a future `&mut dyn Write`
    /// overload. The string-backed [`BitWriter`] is infallible, so this
    /// variant is reserved.
    Io(String),
}

impl fmt::Display for DwgWriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidValue(msg) => write!(f, "invalid value: {msg}"),
            Self::InvalidDocument(msg) => write!(f, "invalid document: {msg}"),
            Self::Unsupported(msg) => write!(f, "unsupported: {msg}"),
            Self::SectionTooLarge {
                section,
                bytes,
                limit,
            } => write!(
                f,
                "section `{section}` payload too large: {bytes} bytes (limit {limit})"
            ),
            Self::Io(msg) => write!(f, "io: {msg}"),
        }
    }
}

impl std::error::Error for DwgWriteError {}
