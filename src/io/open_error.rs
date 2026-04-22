//! Structured error type for the CAD document open path.
//!
//! Prior to this module, `io::pick_and_open` / `io::open_path` /
//! `io::open_document_blocking` returned `Result<_, String>`, which
//! forced the UI layer to do fragile string matching (e.g.
//! `if e != "Cancelled"`) and surfaced raw English engine messages to
//! the end user.
//!
//! [`OpenError`] classifies the failure into stable variants so the UI
//! can render a Chinese-friendly message and (future) offer targeted
//! remediation such as "save as older version" or "audit file".

use std::fmt;
use std::path::PathBuf;

/// Failure variants that can surface from the document-open pipeline.
#[derive(Debug, Clone)]
pub enum OpenError {
    /// The user cancelled the file picker. Callers should NOT render
    /// this as an error; it is returned as `Err` only because the
    /// `rfd` dialog API itself returns `Option`.
    Cancelled,
    /// Filesystem-level failure (not found, permission denied, etc.).
    Io {
        path: Option<PathBuf>,
        message: String,
    },
    /// The file was recognised but its format version is outside the
    /// current reader's coverage window.
    UnsupportedVersion {
        format: &'static str,
        version: String,
    },
    /// The file is malformed: truncated header, bad sentinel, CRC
    /// mismatch, decryption failure, etc.
    Corrupt {
        format: &'static str,
        reason: String,
    },
    /// The file extension itself is not one of the supported imports.
    UnsupportedExtension { ext: String },
    /// Fallback for PID parser errors and other sources that have not
    /// been classified yet. Carries the original message verbatim.
    Other(String),
}

impl OpenError {
    /// Human-facing Chinese message suitable for the command-line
    /// status bar.
    pub fn user_message_zh(&self) -> String {
        match self {
            Self::Cancelled => "已取消打开操作。".to_string(),
            Self::Io { path, message } => match path {
                Some(p) => format!("读取文件失败（{}）：{}", p.display(), message),
                None => format!("读取文件失败：{message}"),
            },
            Self::UnsupportedVersion { format, version } => format!(
                "暂不支持该 {format} 文件版本：{version}。\
                 请尝试用 AutoCAD 另存为 AC1015 (AutoCAD 2000) 或 DXF 后重试。"
            ),
            Self::Corrupt { format, reason } => {
                format!("{format} 文件内容已损坏或格式异常：{reason}")
            }
            Self::UnsupportedExtension { ext } => format!(
                "暂不支持的文件类型 .{ext}。当前可打开 .dwg / .dxf / .pid。"
            ),
            Self::Other(msg) => msg.clone(),
        }
    }

    /// True when the error represents a user-initiated cancel and the
    /// UI should stay silent. Keeping this as a method (rather than a
    /// direct `matches!`) lets future variants participate in the
    /// silence rule without having to edit every consumer.
    pub fn is_silent(&self) -> bool {
        matches!(self, Self::Cancelled)
    }
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => f.write_str("cancelled"),
            Self::Io { path, message } => match path {
                Some(p) => write!(f, "io error at {}: {message}", p.display()),
                None => write!(f, "io error: {message}"),
            },
            Self::UnsupportedVersion { format, version } => {
                write!(f, "unsupported {format} version: {version}")
            }
            Self::Corrupt { format, reason } => write!(f, "{format} file corrupt: {reason}"),
            Self::UnsupportedExtension { ext } => write!(f, "unsupported extension: .{ext}"),
            Self::Other(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for OpenError {}

impl From<std::io::Error> for OpenError {
    fn from(err: std::io::Error) -> Self {
        Self::Io {
            path: None,
            message: err.to_string(),
        }
    }
}

impl From<String> for OpenError {
    fn from(s: String) -> Self {
        if s == "Cancelled" {
            Self::Cancelled
        } else {
            Self::Other(s)
        }
    }
}

impl From<&str> for OpenError {
    fn from(s: &str) -> Self {
        Self::from(s.to_string())
    }
}

/// Classify an `acadrust` `DxfError` (used for runtime DWG and
/// some DXF paths) into an [`OpenError`] variant. `format` lets the
/// caller label whether the error came from a DWG or DXF load so the
/// UI can tell the user which decoder surfaced the problem.
pub(crate) fn classify_acadrust(err: acadrust::error::DxfError, format: &'static str) -> OpenError {
    use acadrust::error::DxfError;
    match err {
        DxfError::Io(e) => OpenError::Io {
            path: None,
            message: e.to_string(),
        },
        DxfError::UnsupportedVersion(v) => OpenError::UnsupportedVersion {
            format,
            version: v,
        },
        DxfError::NotImplemented(msg) => OpenError::UnsupportedVersion {
            format,
            version: msg,
        },
        DxfError::InvalidHeader(reason)
        | DxfError::InvalidFormat(reason)
        | DxfError::InvalidSentinel(reason)
        | DxfError::Parse(reason)
        | DxfError::Compression(reason)
        | DxfError::Decompression(reason)
        | DxfError::Decryption(reason)
        | DxfError::Encoding(reason)
        | DxfError::InvalidEntityType(reason) => OpenError::Corrupt { format, reason },
        DxfError::ChecksumMismatch { expected, actual } => OpenError::Corrupt {
            format,
            reason: format!("CRC mismatch: expected {expected:#X}, got {actual:#X}"),
        },
        DxfError::InvalidDxfCode(code) => OpenError::Corrupt {
            format,
            reason: format!("invalid DXF group code {code}"),
        },
        DxfError::InvalidHandle(h) => OpenError::Corrupt {
            format,
            reason: format!("invalid handle {h:#X}"),
        },
        DxfError::ObjectNotFound(h) => OpenError::Corrupt {
            format,
            reason: format!("object not found: handle {h:#X}"),
        },
        DxfError::Custom(msg) => OpenError::Other(msg),
    }
}

/// Classify the native `DxfReadError` (from `h7cad-native-dxf`) into
/// an [`OpenError`] variant.
pub(crate) fn classify_native_dxf(err: h7cad_native_dxf::DxfReadError) -> OpenError {
    use h7cad_native_dxf::DxfReadError;
    match err {
        DxfReadError::UnsupportedFormat(msg) => OpenError::UnsupportedVersion {
            format: "DXF",
            version: msg,
        },
        other => OpenError::Corrupt {
            format: "DXF",
            reason: other.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use acadrust::error::DxfError;
    use std::io;

    #[test]
    fn cancelled_is_silent() {
        assert!(OpenError::Cancelled.is_silent());
        assert!(!OpenError::Other("oops".into()).is_silent());
    }

    #[test]
    fn from_string_cancelled_becomes_cancelled_variant() {
        let err: OpenError = "Cancelled".to_string().into();
        assert!(matches!(err, OpenError::Cancelled));
    }

    #[test]
    fn from_string_other_becomes_other_variant() {
        let err: OpenError = "something went wrong".to_string().into();
        assert!(matches!(err, OpenError::Other(msg) if msg == "something went wrong"));
    }

    #[test]
    fn from_io_error_preserves_message() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "no such file");
        let err: OpenError = io_err.into();
        match err {
            OpenError::Io { message, path: None } => {
                assert!(message.contains("no such file"), "message was {message:?}");
            }
            other => panic!("expected Io variant, got {other:?}"),
        }
    }

    #[test]
    fn classify_acadrust_unsupported_version_routes_to_version_variant() {
        let err = classify_acadrust(
            DxfError::UnsupportedVersion("AC1032".into()),
            "DWG",
        );
        assert!(matches!(
            err,
            OpenError::UnsupportedVersion { format: "DWG", version } if version == "AC1032"
        ));
    }

    #[test]
    fn classify_acadrust_invalid_header_routes_to_corrupt() {
        let err = classify_acadrust(
            DxfError::InvalidHeader("bad magic".into()),
            "DWG",
        );
        assert!(matches!(
            err,
            OpenError::Corrupt { format: "DWG", reason } if reason == "bad magic"
        ));
    }

    #[test]
    fn classify_acadrust_crc_mismatch_builds_reason_from_both_sides() {
        let err = classify_acadrust(
            DxfError::ChecksumMismatch {
                expected: 0x1234,
                actual: 0xBEEF,
            },
            "DWG",
        );
        match err {
            OpenError::Corrupt { format: "DWG", reason } => {
                assert!(reason.contains("0x1234"));
                assert!(reason.contains("0xBEEF"));
            }
            other => panic!("expected Corrupt variant, got {other:?}"),
        }
    }

    #[test]
    fn classify_acadrust_io_keeps_message_without_path() {
        let err = classify_acadrust(
            DxfError::Io(io::Error::new(io::ErrorKind::PermissionDenied, "denied")),
            "DWG",
        );
        match err {
            OpenError::Io { message, path: None } => assert!(message.contains("denied")),
            other => panic!("expected Io, got {other:?}"),
        }
    }

    #[test]
    fn classify_acadrust_not_implemented_surfaces_as_unsupported_version() {
        let err = classify_acadrust(
            DxfError::NotImplemented("encrypted metadata".into()),
            "DWG",
        );
        assert!(matches!(
            err,
            OpenError::UnsupportedVersion { format: "DWG", .. }
        ));
    }

    #[test]
    fn classify_native_dxf_unsupported_format_routes_to_version() {
        let err = classify_native_dxf(h7cad_native_dxf::DxfReadError::UnsupportedFormat(
            "binary only".into(),
        ));
        assert!(matches!(
            err,
            OpenError::UnsupportedVersion { format: "DXF", .. }
        ));
    }

    #[test]
    fn classify_native_dxf_unexpected_eof_routes_to_corrupt() {
        let err = classify_native_dxf(h7cad_native_dxf::DxfReadError::UnexpectedEof {
            context: "header",
        });
        assert!(matches!(err, OpenError::Corrupt { format: "DXF", .. }));
    }

    #[test]
    fn zh_message_for_unsupported_extension_mentions_supported_types() {
        let msg = OpenError::UnsupportedExtension { ext: "foo".into() }.user_message_zh();
        assert!(msg.contains(".foo"));
        assert!(msg.contains("dwg"));
        assert!(msg.contains("dxf"));
    }

    #[test]
    fn zh_message_for_unsupported_version_suggests_downgrade() {
        let msg = OpenError::UnsupportedVersion {
            format: "DWG",
            version: "AC1032".into(),
        }
        .user_message_zh();
        assert!(msg.contains("AC1032"));
        assert!(msg.contains("AC1015"));
    }

    #[test]
    fn zh_message_for_cancelled_is_neutral() {
        let msg = OpenError::Cancelled.user_message_zh();
        assert!(msg.contains("取消"));
    }

    #[test]
    fn display_impl_renders_variant_specific_prefix() {
        let err = OpenError::UnsupportedVersion {
            format: "DWG",
            version: "AC1032".into(),
        };
        let s = err.to_string();
        assert!(s.contains("DWG"));
        assert!(s.contains("AC1032"));
    }

    #[test]
    fn open_document_blocking_rejects_unsupported_extension() {
        let path = std::path::Path::new("/nonexistent/fixture.xyz");
        let err = crate::io::open_document_blocking(path)
            .expect_err("unsupported extension should bail before any I/O");
        assert!(matches!(err, OpenError::UnsupportedExtension { ext } if ext == "xyz"));
    }

    #[test]
    fn open_document_blocking_reports_io_for_missing_dwg() {
        let path = std::path::Path::new("does-not-exist-sentinel.dwg");
        let err = crate::io::open_document_blocking(path)
            .expect_err("missing file must surface as an Io variant");
        match err {
            OpenError::Io { path: slot, message } => {
                assert!(
                    slot.as_ref()
                        .map(|p| p.as_path() == path)
                        .unwrap_or(false),
                    "missing path slot should be populated with the attempted path"
                );
                assert!(!message.is_empty(), "io message must not be empty");
            }
            other => panic!("expected Io variant, got {other:?}"),
        }
    }

    #[test]
    fn open_document_blocking_reports_io_for_missing_dxf() {
        let path = std::path::Path::new("does-not-exist-sentinel.dxf");
        let err = crate::io::open_document_blocking(path)
            .expect_err("missing DXF should surface as an Io variant");
        match err {
            OpenError::Io { path: slot, .. } => {
                assert_eq!(slot.as_deref(), Some(path));
            }
            other => panic!("expected Io variant, got {other:?}"),
        }
    }

    #[test]
    fn open_path_async_does_not_block_caller_thread_panic_safety() {
        // Verifies the oneshot pathway: a non-existent file on a
        // worker thread still reports a classified `OpenError` and
        // does not deadlock the async wrapper.
        use iced::futures::executor::block_on;

        let path = std::path::PathBuf::from("definitely-not-a-real-file.dwg");
        let err = block_on(crate::io::open_path(path))
            .expect_err("missing file must bubble up from the worker");
        assert!(
            matches!(err, OpenError::Io { .. }),
            "expected Io, got {err:?}"
        );
    }
}
