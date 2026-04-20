//! VS Code-style workspace: a user-picked root directory with a flat,
//! filtered, sorted list of CAD files and folders that the UI can render
//! as a collapsible tree.
//!
//! This module is **pure**: no iced, no async, no `CadDocument` — just
//! filesystem IO and plain data structures.  The UI layer consumes the
//! `Workspace.entries` snapshot.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Directories skipped during scanning — large, irrelevant to CAD work,
/// or known to contain tens of thousands of tiny files.
const BLACKLIST_DIR_NAMES: &[&str] = &[
    ".git",
    ".svn",
    ".hg",
    ".cargo",
    ".cursor",
    ".agents",
    ".memory",
    ".factory",
    ".claude",
    ".augment",
    ".junie",
    ".kiro",
    ".pi",
    ".windsurf",
    "target",
    "node_modules",
    "vendor_tmp",
    "vendor",
    "dist",
    "build",
];

/// Max recursion depth when scanning a workspace root.
pub const DEFAULT_MAX_DEPTH: u8 = 3;

/// Max number of entries returned before the scan stops.
pub const DEFAULT_MAX_ENTRIES: usize = 2000;

/// A single row in the workspace tree — either a directory or a CAD file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceEntry {
    /// Absolute path.
    pub path: PathBuf,
    /// Base name (final path component) for display.
    pub name: String,
    /// Depth below the workspace root.  0 = root, 1 = direct child, …
    pub depth: u8,
    /// Parent directory's absolute path; `None` for the root itself.
    /// Used to skip rows whose ancestor directory is collapsed.
    pub parent: Option<PathBuf>,
    /// File kind — drives icon choice and click semantics.
    pub kind: EntryKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Directory,
    DxfFile,
    DwgFile,
    PidFile,
    /// Placeholder row appended when a scan is truncated.
    Truncated,
}

/// A scanned workspace snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    pub root: PathBuf,
    pub entries: Vec<WorkspaceEntry>,
    /// `true` if the scan stopped because `max_entries` was reached.
    pub truncated: bool,
}

impl Workspace {
    /// Returns the root directory's last-component name, or the whole path
    /// if the root has no final component (e.g. Windows drive letters).
    pub fn root_label(&self) -> String {
        self.root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.root.display().to_string())
    }
}

/// Scan a workspace root, returning all nested directories + `.dxf` / `.dwg` / `.pid`
/// files up to `max_depth`, capped at `max_entries` rows.
///
/// Filesystem errors (permission denied, symlink loops, etc.) on a sub-
/// directory are silently skipped — we do not want a single unreadable
/// folder to abort the whole scan.  A top-level error on the root itself
/// returns an `Err`.
pub fn scan_workspace(
    root: &Path,
    max_depth: u8,
    max_entries: usize,
) -> Result<Workspace, String> {
    if !root.is_dir() {
        return Err(format!("{} is not a directory", root.display()));
    }
    let mut entries: Vec<WorkspaceEntry> = Vec::new();
    let mut truncated = false;
    scan_recursive(
        root,
        root,
        0,
        max_depth,
        max_entries,
        &mut entries,
        &mut truncated,
    );
    if truncated {
        entries.push(WorkspaceEntry {
            path: root.to_path_buf(),
            name: "… (truncated)".to_string(),
            depth: 0,
            parent: None,
            kind: EntryKind::Truncated,
        });
    }
    Ok(Workspace {
        root: root.to_path_buf(),
        entries,
        truncated,
    })
}

fn scan_recursive(
    _root: &Path,
    dir: &Path,
    depth: u8,
    max_depth: u8,
    max_entries: usize,
    out: &mut Vec<WorkspaceEntry>,
    truncated: &mut bool,
) {
    if *truncated || depth > max_depth {
        return;
    }
    let rd = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    let mut children: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    children.sort_by(|a, b| {
        // Directories before files, then by lowercase name.
        let da = a.is_dir();
        let db = b.is_dir();
        match (da, db) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => {
                let an = a
                    .file_name()
                    .map(|n| n.to_string_lossy().to_ascii_lowercase())
                    .unwrap_or_default();
                let bn = b
                    .file_name()
                    .map(|n| n.to_string_lossy().to_ascii_lowercase())
                    .unwrap_or_default();
                an.cmp(&bn)
            }
        }
    });

    for child in children {
        if out.len() >= max_entries {
            *truncated = true;
            return;
        }
        let name = match child.file_name() {
            Some(n) => n.to_string_lossy().into_owned(),
            None => continue,
        };
        if child.is_dir() {
            if BLACKLIST_DIR_NAMES.iter().any(|b| name.eq_ignore_ascii_case(b)) {
                continue;
            }
            out.push(WorkspaceEntry {
                path: child.clone(),
                name: name.clone(),
                depth: depth + 1,
                parent: Some(dir.to_path_buf()),
                kind: EntryKind::Directory,
            });
            if depth + 1 < max_depth {
                scan_recursive(
                    _root,
                    &child,
                    depth + 1,
                    max_depth,
                    max_entries,
                    out,
                    truncated,
                );
            }
        } else if child.is_file() {
            let kind = match child
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_ascii_lowercase())
                .as_deref()
            {
                Some("dxf") => EntryKind::DxfFile,
                Some("dwg") => EntryKind::DwgFile,
                Some("pid") => EntryKind::PidFile,
                _ => continue, // silently skip unrelated files
            };
            out.push(WorkspaceEntry {
                path: child,
                name,
                depth: depth + 1,
                parent: Some(dir.to_path_buf()),
                kind,
            });
        }
    }
}

/// Return the subset of `entries` whose every ancestor is present in
/// `expanded_dirs`.  Top-level (depth 1) entries are always visible.  The
/// root (depth 0) is typically rendered separately as the panel header.
pub fn visible_entries<'a>(
    entries: &'a [WorkspaceEntry],
    expanded_dirs: &HashSet<PathBuf>,
) -> Vec<&'a WorkspaceEntry> {
    entries
        .iter()
        .filter(|e| match &e.parent {
            None => true,
            Some(p) if e.depth <= 1 => {
                let _ = p;
                true
            }
            Some(p) => expanded_dirs.contains(p),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_dir(name: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("h7cad-ws-test-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("create tmp dir");
        base
    }

    #[test]
    fn scans_top_level_cad_files() {
        let root = tmp_dir("top");
        fs::write(root.join("foo.dxf"), "").unwrap();
        fs::write(root.join("bar.DWG"), "").unwrap();
        fs::write(root.join("diagram.pid"), "").unwrap();
        fs::write(root.join("ignore.txt"), "").unwrap();

        let ws = scan_workspace(&root, 3, 2000).unwrap();
        let names: Vec<&str> = ws.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"foo.dxf"));
        assert!(names.contains(&"bar.DWG"));
        assert!(names.contains(&"diagram.pid"));
        assert!(ws.entries.iter().any(|e| e.name == "diagram.pid" && matches!(e.kind, EntryKind::PidFile)));
        assert!(!names.contains(&"ignore.txt"), "non-CAD files must be filtered");
    }

    #[test]
    fn skips_blacklisted_directories() {
        let root = tmp_dir("blacklist");
        fs::create_dir(root.join(".git")).unwrap();
        fs::write(root.join(".git").join("HEAD"), "").unwrap();
        fs::create_dir(root.join("target")).unwrap();
        fs::write(root.join("target").join("drawing.dxf"), "").unwrap();
        fs::write(root.join("keep.dxf"), "").unwrap();

        let ws = scan_workspace(&root, 3, 2000).unwrap();
        let names: Vec<&str> = ws.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"keep.dxf"));
        assert!(!names.contains(&".git"));
        assert!(!names.contains(&"target"));
    }

    #[test]
    fn respects_max_depth() {
        let root = tmp_dir("depth");
        let sub = root.join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("a.dxf"), "").unwrap();
        let deep = sub.join("deep");
        fs::create_dir(&deep).unwrap();
        fs::write(deep.join("b.dxf"), "").unwrap();

        let shallow = scan_workspace(&root, 1, 2000).unwrap();
        let names: Vec<&str> = shallow.entries.iter().map(|e| e.name.as_str()).collect();
        // With max_depth=1 we only see sub/ itself; sub's children are not scanned.
        assert!(names.contains(&"sub"));
        assert!(!names.contains(&"a.dxf"));
        assert!(!names.contains(&"b.dxf"));

        let deeper = scan_workspace(&root, 3, 2000).unwrap();
        let names_d: Vec<&str> = deeper.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names_d.contains(&"a.dxf"));
        assert!(names_d.contains(&"b.dxf"));
    }

    #[test]
    fn sorts_directories_before_files_alphabetically() {
        let root = tmp_dir("sort");
        fs::write(root.join("z.dxf"), "").unwrap();
        fs::create_dir(root.join("alpha")).unwrap();
        fs::write(root.join("alpha").join("one.dxf"), "").unwrap();
        fs::write(root.join("a.dxf"), "").unwrap();

        let ws = scan_workspace(&root, 2, 2000).unwrap();
        // First entry at depth 1 must be the "alpha" directory (dirs first,
        // alphabetical).  Remaining depth-1 rows are files a.dxf then z.dxf.
        let depth1: Vec<&str> = ws
            .entries
            .iter()
            .filter(|e| e.depth == 1)
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(depth1, vec!["alpha", "a.dxf", "z.dxf"]);
    }

    #[test]
    fn truncation_stops_scan_and_sets_flag() {
        let root = tmp_dir("truncate");
        for i in 0..10 {
            fs::write(root.join(format!("f{}.dxf", i)), "").unwrap();
        }
        let ws = scan_workspace(&root, 1, 3).unwrap();
        assert!(ws.truncated, "should mark truncated at cap=3");
        assert!(
            ws.entries.iter().any(|e| matches!(e.kind, EntryKind::Truncated)),
            "a truncation marker row must be appended"
        );
    }

    #[test]
    fn non_directory_root_errors_out() {
        let root = tmp_dir("notdir").join("ghost.txt");
        // Do NOT create it — this must fail cleanly.
        let err = scan_workspace(&root, 3, 2000).unwrap_err();
        assert!(err.contains("is not a directory"));
    }

    #[test]
    fn visible_entries_respects_collapse() {
        let root = PathBuf::from("/ws");
        let sub = PathBuf::from("/ws/sub");
        let mut entries = Vec::new();
        entries.push(WorkspaceEntry {
            path: sub.clone(),
            name: "sub".to_string(),
            depth: 1,
            parent: Some(root.clone()),
            kind: EntryKind::Directory,
        });
        entries.push(WorkspaceEntry {
            path: sub.join("a.dxf"),
            name: "a.dxf".to_string(),
            depth: 2,
            parent: Some(sub.clone()),
            kind: EntryKind::DxfFile,
        });

        // Collapsed: only the "sub" folder row is visible, its child isn't.
        let expanded: HashSet<PathBuf> = HashSet::new();
        let vis = visible_entries(&entries, &expanded);
        let vis_names: Vec<&str> = vis.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(vis_names, vec!["sub"]);

        // Expanded: both rows visible.
        let mut expanded = HashSet::new();
        expanded.insert(sub.clone());
        let vis = visible_entries(&entries, &expanded);
        let vis_names: Vec<&str> = vis.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(vis_names, vec!["sub", "a.dxf"]);
    }
}
