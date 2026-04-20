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

/// Aggregate cache occupancy at a single point in time. `total_stream_bytes`
/// sums `RawStream.data.len()` across every cached `PidPackage`; it
/// deliberately ignores struct padding, Arc overhead, HashMap bucket
/// overhead, and `PathBuf` keys — meant as an **order-of-magnitude**
/// signal, not a precise memory report. Pair with an OS-level RSS probe
/// if you need a tighter number.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PidPackageCacheStats {
    pub entry_count: usize,
    pub total_stream_bytes: u64,
}

/// Per-entry summary surfaced to the `PIDCACHESTATS` command renderer.
/// Kept as a separate type so the aggregate [`cache_stats`] can avoid
/// allocating a `Vec` when only the totals are wanted.
#[derive(Debug, Clone)]
pub struct PidPackageCacheEntrySummary {
    pub path: PathBuf,
    pub stream_count: usize,
    pub stream_bytes: u64,
}

/// Snapshot of cache totals. O(total stream count) under the cache mutex.
pub fn cache_stats() -> PidPackageCacheStats {
    let guard = store().lock().expect("pid_package_store mutex poisoned");
    let mut stats = PidPackageCacheStats {
        entry_count: guard.len(),
        total_stream_bytes: 0,
    };
    for pkg in guard.values() {
        for stream in pkg.streams.values() {
            stats.total_stream_bytes = stats
                .total_stream_bytes
                .saturating_add(stream.data.len() as u64);
        }
    }
    stats
}

/// Lexicographically-sorted list of paths currently cached. Sorted so
/// command-line renderers produce stable output across runs. Not yet
/// wired to a command (the current renderer uses [`cached_entry_summaries`]
/// directly); kept on the public surface for a future `PIDCACHELIST`
/// command or for scripting consumers.
#[allow(dead_code)]
pub fn cached_paths() -> Vec<PathBuf> {
    let guard = store().lock().expect("pid_package_store mutex poisoned");
    let mut paths: Vec<PathBuf> = guard.keys().cloned().collect();
    paths.sort();
    paths
}

/// Per-entry summary vector, sorted by path. Useful for the
/// `PIDCACHESTATS` command which renders one row per cached package.
pub fn cached_entry_summaries() -> Vec<PidPackageCacheEntrySummary> {
    let guard = store().lock().expect("pid_package_store mutex poisoned");
    let mut entries: Vec<PidPackageCacheEntrySummary> = guard
        .iter()
        .map(|(path, pkg)| {
            let stream_bytes: u64 = pkg
                .streams
                .values()
                .map(|s| s.data.len() as u64)
                .fold(0u64, |a, b| a.saturating_add(b));
            PidPackageCacheEntrySummary {
                path: path.clone(),
                stream_count: pkg.streams.len(),
                stream_bytes,
            }
        })
        .collect();
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    entries
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

    #[test]
    fn cache_stats_reflects_insert_and_clear_via_tagged_filter() {
        // The cache is process-global, so parallel tests can freely insert
        // between our reads. Rather than assert on raw stats counters
        // (racy), we take a snapshot that filters *our* entries by a unique
        // tag prefix and assert on that filtered view — immune to concurrent
        // test activity.
        const TAG: &str = "stats-filter-";
        let a = unique_path(&format!("{TAG}a"));
        let b = unique_path(&format!("{TAG}b"));
        cache_package(&a, fixture_pkg("hello"));   // 5 bytes, 1 stream
        cache_package(&b, fixture_pkg("world!!")); // 7 bytes, 1 stream

        let our_entries: Vec<_> = cached_entry_summaries()
            .into_iter()
            .filter(|e| {
                e.path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.contains(TAG))
                    .unwrap_or(false)
            })
            .collect();
        assert_eq!(our_entries.len(), 2, "should see exactly our 2 entries");
        let our_bytes: u64 = our_entries.iter().map(|e| e.stream_bytes).sum();
        assert_eq!(
            our_bytes,
            (b"hello".len() + b"world!!".len()) as u64,
            "byte sum should be 5 + 7"
        );

        clear_package(&a);
        clear_package(&b);

        let after_clear: Vec<_> = cached_entry_summaries()
            .into_iter()
            .filter(|e| {
                e.path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.contains(TAG))
                    .unwrap_or(false)
            })
            .collect();
        assert!(
            after_clear.is_empty(),
            "our entries should be gone after clear; still present: {after_clear:?}"
        );
    }

    #[test]
    fn cached_paths_returns_lexicographic_order() {
        // Insert deliberately non-lex order; cached_paths must still return
        // sorted. We check our own delta to tolerate other tests' entries.
        let zeta = unique_path("ordz-zzz");
        let alpha = unique_path("ordz-aaa");
        let mid = unique_path("ordz-mmm");
        cache_package(&zeta, fixture_pkg("z"));
        cache_package(&alpha, fixture_pkg("a"));
        cache_package(&mid, fixture_pkg("m"));

        let all = cached_paths();
        // Pull only the three we added (by tag prefix), preserving order.
        let ours: Vec<_> = all
            .into_iter()
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.contains("ordz-"))
                    .unwrap_or(false)
            })
            .collect();
        assert_eq!(ours.len(), 3);
        assert!(
            ours[0] < ours[1] && ours[1] < ours[2],
            "expected ascending lex order; got: {ours:?}"
        );

        clear_package(&zeta);
        clear_package(&alpha);
        clear_package(&mid);
    }

    #[test]
    fn cached_entry_summaries_counts_streams_and_bytes() {
        let path = unique_path("entry-sum");
        let mut streams = BTreeMap::new();
        for i in 0..5usize {
            let data: Vec<u8> = vec![0u8; 100];
            streams.insert(
                format!("/S{i}"),
                RawStream {
                    path: format!("/S{i}"),
                    data,
                    modified: false,
                },
            );
        }
        let pkg = PidPackage::new(None, streams, PidDocument::default());
        cache_package(&path, pkg);

        let entry = cached_entry_summaries()
            .into_iter()
            .find(|e| e.path == key_for(&path))
            .expect("our entry should appear in summaries");
        assert_eq!(entry.stream_count, 5);
        assert_eq!(entry.stream_bytes, 500);

        clear_package(&path);
    }
}
