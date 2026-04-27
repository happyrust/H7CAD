// I/O module — open, save, and export CAD documents.
//
// Native I/O helpers are the forward path. Compatibility wrappers remain for
// the current UI/runtime until scene/document migration is complete.

pub mod cui;
pub mod diagnostics;
pub mod obj;
pub mod open_error;
pub mod pdf_export;
pub mod plot_style;
pub mod svg_export;
pub mod print_to_printer;
pub mod step;
pub mod stl;
pub mod xref;

use acadrust::io::dwg::DwgReader;
use acadrust::{CadDocument, DwgWriter};
use h7cad_native_model::CadDocument as NativeCadDocument;
use std::path::{Path, PathBuf};

pub mod native_bridge;
pub mod pid_import;
pub mod pid_package_store;
pub mod pid_screenshot;

#[allow(unused_imports)] // NoticeSeverity re-exported for downstream match/construction ergonomics
pub use diagnostics::{NoticeCounts, NoticeSeverity, OpenNotice};
pub use open_error::OpenError;

#[derive(Debug, Clone)]
pub enum OpenedDocument {
    Cad {
        compat_doc: CadDocument,
        native_doc: Option<NativeCadDocument>,
    },
    Pid(pid_import::PidOpenBundle),
}

#[derive(Debug, Clone)]
pub struct OpenFileResult {
    pub name: String,
    pub path: PathBuf,
    pub opened: OpenedDocument,
    /// Non-fatal diagnostics surfaced by the underlying reader
    /// (unsupported sub-sections, recovered errors, etc.). Collected
    /// from `acadrust::CadDocument::notifications` when the source was
    /// a DWG; empty for DXF/PID until those backends add their own
    /// diagnostic producers. See [`diagnostics::OpenNotice`].
    pub notices: Vec<OpenNotice>,
}

// ── Open ──────────────────────────────────────────────────────────────────
//
// The public `pick_and_open` / `open_path` functions are `async fn` and
// are normally polled on iced's event-loop executor (via
// `Task::perform`). The actual DWG/DXF decoding is CPU-bound and
// performs synchronous file I/O, so running it directly inside the
// future would stall iced's main thread for the duration of the read
// (seconds on large engineering drawings).
//
// The `*_blocking` helpers below keep that synchronous logic in its
// natural form, and the async wrappers dispatch the work onto a
// dedicated worker thread, signalling completion through an iced-
// provided `futures::channel::oneshot`. This pattern needs no tokio
// reactor and uses the same `futures` crate iced 0.14 already pulls
// in transitively.

use iced::futures::channel::oneshot;

/// Show a file-open dialog and load the selected DWG or DXF file.
/// Returns `(filename, path, opened)` or a classified [`OpenError`].
pub async fn pick_and_open() -> Result<OpenFileResult, OpenError> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("Open CAD file")
        .add_filter("CAD Files", &["dwg", "dxf", "pid"])
        .add_filter("DWG Files", &["dwg"])
        .add_filter("DXF Files", &["dxf"])
        .add_filter("PID Files", &["pid"])
        .add_filter("All Files", &["*"])
        .pick_file()
        .await;

    let handle = match handle {
        Some(h) => h,
        None => return Err(OpenError::Cancelled),
    };

    let path = handle.path().to_path_buf();
    open_path(path).await
}

/// Load a CAD or PID file from a known path (used by recent files).
///
/// The heavy decoding runs on a dedicated worker thread so iced's
/// event loop stays responsive while large drawings are parsed.
pub async fn open_path(path: PathBuf) -> Result<OpenFileResult, OpenError> {
    let (tx, rx) = oneshot::channel();
    std::thread::Builder::new()
        .name("h7cad-open-file".into())
        .spawn(move || {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "unknown".into());
            let result =
                open_document_blocking(&path).map(|(opened, notices)| OpenFileResult {
                    name,
                    path: path.clone(),
                    opened,
                    notices,
                });
            let _ = tx.send(result);
        })
        .map_err(|e| OpenError::Io {
            path: None,
            message: format!("failed to spawn file-open worker thread: {e}"),
        })?;

    rx.await.unwrap_or_else(|_| {
        Err(OpenError::Other(
            "file open worker terminated before responding".into(),
        ))
    })
}

/// Legacy backward-compatible loader used by xref resolution and
/// anything else that only wants the compat (`acadrust`) document and
/// is happy with stringified errors. New call sites should prefer
/// [`load_file_with_native_blocking`] to get structured [`OpenError`]
/// values.
///
/// This call is synchronous on the current thread — do not invoke it
/// from an async context that must stay responsive; use [`open_path`]
/// for that.
pub fn load_file(path: &Path) -> Result<CadDocument, String> {
    let (doc, _, _) = load_file_with_native_blocking(path).map_err(|e| e.to_string())?;
    Ok(doc)
}

/// Synchronous CAD document loader. Produces both the compat
/// (`acadrust`) representation and the native counterpart, along with
/// any non-fatal [`OpenNotice`]s surfaced by the underlying reader.
pub fn load_file_with_native_blocking(
    path: &Path,
) -> Result<(CadDocument, Option<NativeCadDocument>, Vec<OpenNotice>), OpenError> {
    let (native, notices) = load_file_native_blocking(path)?;
    let compat = native_bridge::native_doc_to_acadrust(&native);
    Ok((compat, Some(native), notices))
}

/// Synchronous document-open dispatch. Prefer [`open_path`] from
/// async contexts so the iced main loop is not blocked for the
/// duration of the parse.
pub fn open_document_blocking(
    path: &Path,
) -> Result<(OpenedDocument, Vec<OpenNotice>), OpenError> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "dwg" | "dxf" => {
            let (compat_doc, native_doc, notices) = load_file_with_native_blocking(path)?;
            Ok((
                OpenedDocument::Cad {
                    compat_doc,
                    native_doc,
                },
                notices,
            ))
        }
        "pid" => Ok((OpenedDocument::Pid(pid_import::open_pid(path)?), Vec::new())),
        _ => Err(OpenError::UnsupportedExtension { ext }),
    }
}

/// Synchronous native-first load path. Callers who need async
/// execution should wrap a call to this function in
/// [`std::thread::spawn`] or [`open_path`].
///
/// Returns `(document, notices)`. DWG reads surface acadrust's
/// `NotificationCollection` through [`diagnostics::OpenNotice`]; DXF
/// and PID currently produce no diagnostics and return an empty Vec.
pub fn load_file_native_blocking(
    path: &Path,
) -> Result<(NativeCadDocument, Vec<OpenNotice>), OpenError> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "dwg" => load_dwg_native_blocking(path),
        "dxf" => Ok((load_dxf_native_blocking(path)?, Vec::new())),
        "pid" => Ok((
            pid_import::load_pid_native(path).map_err(OpenError::from)?,
            Vec::new(),
        )),
        _ => Err(OpenError::UnsupportedExtension { ext }),
    }
}


// ── Save dialog ───────────────────────────────────────────────────────────

/// Show a save-file dialog for DWG / DXF / PID outputs.
///
/// The target CAD-file format is auto-detected from the returned
/// extension. **The DWG/DXF version is not selected here** — it is
/// taken from the document's in-memory `version` field (sniffed when
/// the drawing was opened, or the acadrust default for fresh
/// drawings). Exposing 8 "DWG Files (2018/.../R13)" labels like the
/// earlier revision of this function did was misleading: rfd's
/// `save_file` API does not return the selected filter, so none of
/// those labels could ever influence the actual output format.
///
/// See `save_dwg` and `docs/plans/2026-04-21-dwg-save-version-honesty-plan.md`
/// for the rationale. Explicit version selection is tracked as a
/// future milestone.
pub async fn pick_save_path() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Save As")
        .set_file_name("drawing.dwg")
        .add_filter("DWG File", &["dwg"])
        .add_filter("DXF File", &["dxf"])
        .add_filter("PID File", &["pid"])
        .add_filter("All Files", &["*"])
        .save_file()
        .await
        .map(|h| h.path().to_path_buf())
}

// ── Plot Style Table ──────────────────────────────────────────────────────

/// Show a file-open dialog and load the selected CTB or STB file.
pub async fn pick_plot_style() -> Option<plot_style::PlotStyleTable> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("Load Plot Style Table")
        .add_filter("Plot Style Tables", &["ctb", "stb", "CTB", "STB"])
        .add_filter("CTB Files", &["ctb", "CTB"])
        .add_filter("STB Files", &["stb", "STB"])
        .add_filter("All Files", &["*"])
        .pick_file()
        .await?;
    plot_style::PlotStyleTable::load(handle.path()).ok()
}

// ── Workspace (VS Code-style folder picker) ──────────────────────────────

/// Show a folder-picker dialog for selecting a workspace root.  Returns
/// `None` if the user cancels.
pub async fn pick_workspace_folder() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Open Workspace Folder")
        .pick_folder()
        .await
        .map(|h| h.path().to_path_buf())
}

// ── CUI (Command User Interface) ──────────────────────────────────────────

/// Show a save-file dialog for exporting the runtime CUI (aliases +
/// shortcuts) to disk.  Returns the picked path (or `None` on cancel).
pub async fn pick_cui_save_path() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Export CUI (aliases + shortcuts)")
        .set_file_name("h7cad.cui")
        .add_filter("H7CAD CUI Files", &["cui", "txt"])
        .add_filter("All Files", &["*"])
        .save_file()
        .await
        .map(|h| h.path().to_path_buf())
}

/// Show a file-open dialog for importing a H7CAD CUI file.  Returns the
/// picked path (or `None` on cancel).
pub async fn pick_cui_open_path() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Import CUI (aliases + shortcuts)")
        .add_filter("H7CAD CUI Files", &["cui", "txt"])
        .add_filter("All Files", &["*"])
        .pick_file()
        .await
        .map(|h| h.path().to_path_buf())
}

// ── Image file picker ─────────────────────────────────────────────────────

/// Show a file-open dialog for raster images and decode the selected file.
/// Returns `(path, pixel_width, pixel_height)` or an error string.
pub async fn pick_image_file() -> Result<(PathBuf, u32, u32), String> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("Select Image File")
        .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "tiff", "tif"])
        .add_filter("PNG", &["png"])
        .add_filter("JPEG", &["jpg", "jpeg"])
        .add_filter("All Files", &["*"])
        .pick_file()
        .await
        .ok_or_else(|| "Cancelled".to_string())?;
    let path = handle.path().to_path_buf();
    let img = image::open(&path).map_err(|e| e.to_string())?;
    let (w, h) = image::GenericImageView::dimensions(&img);
    Ok((path, w, h))
}

// ── Save ──────────────────────────────────────────────────────────────────

/// Save the document to the given path.
/// Format is auto-detected from the extension (dwg / dxf).
pub fn save(doc: &CadDocument, path: &Path) -> Result<(), String> {
    let native = native_bridge::acadrust_doc_to_native(doc);
    save_native(&native, path)
}

/// Native-first save path used by the ongoing runtime migration.
///
/// `.pid` is intentionally rejected here: PID round-trip needs the
/// original `PidPackage` (raw CFB stream bytes captured at open time)
/// which `NativeCadDocument` does not carry. UI code should detect the
/// `.pid` extension *before* reaching this point and dispatch to
/// [`pid_import::save_pid_native`] with the source path; see
/// `app::helpers::save_active_tab_to_path`.
pub fn save_native(doc: &NativeCadDocument, path: &Path) -> Result<(), String> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "dxf" => save_dxf(doc, path),
        "pid" => Err(
            "PID save must go through pid_import::save_pid_native (raw stream bytes are needed)"
                .to_string(),
        ),
        _ => save_dwg(doc, path),
    }
}

/// Write the document to a DWG file at `path`.
///
/// The output DWG format version is taken from `doc.header.version`
/// (propagated through `native_bridge::native_doc_to_acadrust`), so
/// "Save As" on an opened drawing preserves its original version
/// (e.g. an AC1015/R2000 file stays AC1015). Fresh documents built
/// with `NativeCadDocument::new()` default to `R2000`; opening and
/// re-saving them is lossless.
///
/// There is no per-save version override today — users who need a
/// specific older output version should open a template drawing in
/// that version first, then "Save As" over it. See
/// `docs/plans/2026-04-21-dwg-save-version-honesty-plan.md` for the
/// full rationale and the planned version-picker work.
pub fn save_dwg(doc: &NativeCadDocument, path: &Path) -> Result<(), String> {
    let acad_doc = native_bridge::native_doc_to_acadrust(doc);
    DwgWriter::write_to_file(path, &acad_doc).map_err(|e| e.to_string())
}

/// Load a DWG file, trying the native parser first with acadrust fallback.
///
/// The native parser (`h7cad_native_dwg::read_dwg`) is attempted first.
/// If it succeeds, the result is used directly. If it fails, the
/// acadrust reader is used as a fallback, and a diagnostic notice is
/// emitted to inform the user which path was taken.
fn load_dwg_native_blocking(
    path: &Path,
) -> Result<(NativeCadDocument, Vec<OpenNotice>), OpenError> {
    let bytes = std::fs::read(path).map_err(|e| OpenError::Io {
        path: Some(path.to_path_buf()),
        message: e.to_string(),
    })?;

    match h7cad_native_dwg::read_dwg(&bytes) {
        Ok(doc) => {
            let notices = vec![OpenNotice::new(
                diagnostics::NoticeSeverity::NotImplemented,
                "loaded via native DWG parser (experimental)",
            )];
            Ok((doc, notices))
        }
        Err(native_err) => {
            let mut reader = DwgReader::from_file(path).map_err(|e| {
                let mut err = open_error::classify_acadrust(e, "DWG");
                if let OpenError::Io { path: slot, .. } = &mut err {
                    *slot = Some(path.to_path_buf());
                }
                err
            })?;
            let acad_doc = reader
                .read()
                .map_err(|e| open_error::classify_acadrust(e, "DWG"))?;
            let mut notices = diagnostics::from_acadrust_notifications(&acad_doc.notifications);
            notices.push(OpenNotice::new(
                diagnostics::NoticeSeverity::Warning,
                format!(
                    "native DWG parser failed ({native_err}), fell back to acadrust"
                ),
            ));
            Ok((native_bridge::acadrust_doc_to_native(&acad_doc), notices))
        }
    }
}

/// Load a DXF file via the native reader (synchronous).
fn load_dxf_native_blocking(path: &Path) -> Result<NativeCadDocument, OpenError> {
    let bytes = std::fs::read(path).map_err(|e| OpenError::Io {
        path: Some(path.to_path_buf()),
        message: e.to_string(),
    })?;
    h7cad_native_dxf::read_dxf_bytes(&bytes).map_err(open_error::classify_native_dxf)
}

/// Write the document to a DXF file at `path`.
///
/// The DXF writer (`h7cad-native-dxf`) emits a single target syntax
/// regardless of the picked filter label; historic revisions of
/// `pick_save_path` exposed 8 "DXF Files (2018/.../R13)" entries that
/// could never actually influence the output (rfd's API does not
/// return the selected filter). The dialog now advertises a single
/// "DXF File" option to stay honest.
pub fn save_dxf(doc: &NativeCadDocument, path: &Path) -> Result<(), String> {
    let text = h7cad_native_dxf::write_dxf(doc)?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

