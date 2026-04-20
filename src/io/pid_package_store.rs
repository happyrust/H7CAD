//! Process-wide cache of `PidPackage` instances keyed by canonical path.
//!
//! When the user opens a `.pid` file we hand the structured
//! [`PidDocument`] to the UI, but the original raw CFB stream bytes
//! (held by [`pid_parse::package::PidPackage`]) cannot be reconstructed
//! from the structured view alone. To preserve byte-level fidelity on
//! "Save As .pid", we stash the parsed `PidPackage` in this store at
//! open time and look it up at save time.
//!
//! Design notes:
//! - Keys are normalized via [`std::fs::canonicalize`]; if canonical
//!   resolution fails (e.g. the destination path doesn't exist yet) we
//!   fall back to the path as-supplied so call sites stay simple.
//! - Values are wrapped in `Arc<PidPackage>` because the writer pipeline
//!   only needs `&PidPackage` and the cache may serve multiple lookups
//!   for the same path.
//! - Concurrency: a single `Mutex<HashMap<…>>` behind a `OnceLock`. PID
//!   open/save is rare and short-lived, so contention is a non-issue.
//!
//! Lifecycle: the cache lives for the process; entries are evicted only
//! when the same path is re-cached or [`clear_package`] is called. A
//! successful "Save As" does **not** auto-evict — the user might save
//! again to the same path; the next open will overwrite the entry.

use pid_parse::package::PidPackage;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

fn store() -> &'static Mutex<HashMap<PathBuf, Arc<PidPackage>>> {
    static STORE: OnceLock<Mutex<HashMap<PathBuf, Arc<PidPackage>>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn key_for(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Insert (or overwrite) the cached `PidPackage` for the given path.
pub fn cache_package(path: &Path, package: PidPackage) {
    let key = key_for(path);
    let mut guard = store().lock().expect("pid_package_store mutex poisoned");
    guard.insert(key, Arc::new(package));
}

/// Look up a previously cached `PidPackage` by path. Returns `None` if
/// the file was never opened in this process (or was explicitly cleared).
pub fn get_package(path: &Path) -> Option<Arc<PidPackage>> {
    let key = key_for(path);
    let guard = store().lock().expect("pid_package_store mutex poisoned");
    guard.get(&key).cloned()
}

/// Remove the cache entry for `path`, if any. Returns `true` if a value
/// was actually removed. Currently unused by the runtime — UI flows keep
/// stale entries around because they're harmless and may serve a later
/// "Save As" — but kept on the public surface for callers that need to
/// invalidate (e.g. an "Unload PID" command).
#[allow(dead_code)]
pub fn clear_package(path: &Path) -> bool {
    let key = key_for(path);
    let mut guard = store().lock().expect("pid_package_store mutex poisoned");
    guard.remove(&key).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pid_parse::model::PidDocument;
    use pid_parse::package::{PidPackage, RawStream};
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// Build a temp path that is unique across the process so parallel
    /// tests can cache concurrently without aliasing.
    fn unique_path(tag: &str) -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        std::env::temp_dir().join(format!("h7cad-pid-store-{pid}-{n}-{tag}.pid"))
    }

    fn fixture_pkg(tag: &str) -> PidPackage {
        let mut streams = BTreeMap::new();
        streams.insert(
            "/Marker".to_string(),
            RawStream {
                path: "/Marker".into(),
                data: tag.as_bytes().to_vec(),
                modified: false,
            },
        );
        PidPackage::new(None, streams, PidDocument::default())
    }

    #[test]
    fn cache_then_get_returns_same_bytes() {
        let path = unique_path("cache-then-get");
        cache_package(&path, fixture_pkg("hello"));
        let pkg = get_package(&path).expect("should find cached package");
        assert_eq!(pkg.streams["/Marker"].data, b"hello");
        clear_package(&path);
    }

    #[test]
    fn clear_package_removes_entry() {
        let path = unique_path("clear-entry");
        cache_package(&path, fixture_pkg("payload"));
        assert!(clear_package(&path));
        assert!(get_package(&path).is_none());
        assert!(!clear_package(&path), "second clear should report no-op");
    }

    #[test]
    fn distinct_paths_dont_alias() {
        let a = unique_path("distinct-a");
        let b = unique_path("distinct-b");
        cache_package(&a, fixture_pkg("AAA"));
        cache_package(&b, fixture_pkg("BBB"));
        assert_eq!(get_package(&a).unwrap().streams["/Marker"].data, b"AAA");
        assert_eq!(get_package(&b).unwrap().streams["/Marker"].data, b"BBB");
        clear_package(&a);
        clear_package(&b);
    }

    #[test]
    fn cache_overwrites_existing_entry() {
        let path = unique_path("overwrite");
        cache_package(&path, fixture_pkg("first"));
        cache_package(&path, fixture_pkg("second"));
        assert_eq!(
            get_package(&path).unwrap().streams["/Marker"].data,
            b"second"
        );
        clear_package(&path);
    }
}
