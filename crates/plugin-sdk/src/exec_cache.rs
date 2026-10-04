//! Output cache for [`Ctx::exec`](crate::Ctx::exec).
//!
//! Caches command output keyed by the resolved binary's identity (absolute
//! path, file size, mtime) and the command arguments. This avoids
//! re-executing commands like `node --version` on every render while the
//! binary hasn't changed.
//!
//! Each plugin process has one cache. When the daemon provides a cache
//! directory, every new entry is written through to
//! `<cache_dir>/<plugin>/exec_cache.json`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

static EXEC_CACHE: OnceLock<ExecCache> = OnceLock::new();

/// Persists this plugin's cache to `path`. Has no effect once the cache is
/// in use, so it must run before the first [`global`] call.
pub(crate) fn init(path: PathBuf) {
    let _ = EXEC_CACHE.set(ExecCache::load(path));
}

/// This plugin's cache, in memory only unless [`init`] ran first.
pub(crate) fn global() -> &'static ExecCache {
    EXEC_CACHE.get_or_init(ExecCache::in_memory)
}

/// Cache key combining a binary's identity with its invocation arguments.
///
/// Binary updates (different size or mtime) naturally cause cache misses
/// because the key no longer matches, which is what makes this safe to use
/// with version managers that swap binaries in `PATH`.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
struct ExecCacheKey {
    /// Absolute path to the resolved binary.
    binary_path: PathBuf,
    /// File size in bytes.
    binary_size: u64,
    /// Nanoseconds since `UNIX_EPOCH`.
    mtime_nanos: u64,
    /// Command arguments.
    args: Vec<String>,
}

/// In-memory cache of exec results, optionally backed by a JSON file.
pub(crate) struct ExecCache {
    entries: Mutex<HashMap<ExecCacheKey, String>>,
    cache_path: Option<PathBuf>,
}

impl ExecCache {
    /// Loads the cache from disk, or starts empty if the file is missing or
    /// corrupt.
    fn load(cache_path: PathBuf) -> Self {
        let entries = fs::read_to_string(&cache_path)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<(ExecCacheKey, String)>>(&s).ok())
            .unwrap_or_default()
            .into_iter()
            .collect();
        Self {
            entries: Mutex::new(entries),
            cache_path: Some(cache_path),
        }
    }

    /// A cache that never touches the disk.
    fn in_memory() -> Self {
        Self {
            entries: Mutex::default(),
            cache_path: None,
        }
    }

    /// The cached output of running `binary` with `args`, if the binary is
    /// unchanged since it was stored.
    pub(crate) fn get(&self, binary: &Path, args: &[&str]) -> Option<String> {
        let key = key_for_path(binary, args)?;
        self.lock().get(&key).cloned()
    }

    /// Stores an output and writes the cache through to disk. Does nothing
    /// if the binary can't be statted.
    pub(crate) fn insert(&self, binary: &Path, args: &[&str], output: String) {
        let Some(key) = key_for_path(binary, args) else {
            return;
        };
        let mut entries = self.lock();
        entries.insert(key, output);
        self.flush(&entries);
    }

    /// A poisoned lock only means another request panicked mid-insert; the
    /// map itself is still usable.
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<ExecCacheKey, String>> {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Persists the full cache as a JSON array of `[key, value]` pairs.
    fn flush(&self, entries: &HashMap<ExecCacheKey, String>) {
        let Some(path) = &self.cache_path else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let pairs: Vec<_> = entries.iter().collect();
        let result = serde_json::to_string(&pairs)
            .map_err(std::io::Error::from)
            .and_then(|json| fs::write(path, json));
        if let Err(error) = result {
            eprintln!("exec cache: failed to write {}: {error}", path.display());
        }
    }
}

fn key_for_path(binary_path: &Path, args: &[&str]) -> Option<ExecCacheKey> {
    let metadata = fs::metadata(binary_path).ok()?;
    let mtime = metadata.modified().ok()?;
    let duration = mtime.duration_since(UNIX_EPOCH).ok()?;
    #[expect(clippy::cast_possible_truncation, reason = "u64 nanos last until 2554")]
    Some(ExecCacheKey {
        binary_path: binary_path.to_path_buf(),
        binary_size: metadata.len(),
        mtime_nanos: duration.as_nanos() as u64,
        args: args.iter().map(ToString::to_string).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binary(dir: &Path) -> PathBuf {
        let path = dir.join("bin");
        fs::write(&path, "v1").unwrap();
        path
    }

    #[test]
    fn cache_miss_then_hit() {
        let dir = tempfile::tempdir().unwrap();
        let bin = binary(dir.path());
        let cache = ExecCache::in_memory();
        assert!(cache.get(&bin, &["hello"]).is_none());
        cache.insert(&bin, &["hello"], "hello\n".to_string());
        assert_eq!(cache.get(&bin, &["hello"]).as_deref(), Some("hello\n"));
    }

    #[test]
    fn different_args_are_distinct() {
        let dir = tempfile::tempdir().unwrap();
        let bin = binary(dir.path());
        let cache = ExecCache::in_memory();
        cache.insert(&bin, &["a"], "a\n".to_string());
        assert!(cache.get(&bin, &["b"]).is_none());
    }

    #[test]
    fn missing_binary_is_never_cached() {
        let cache = ExecCache::in_memory();
        let missing = Path::new("/this/binary/does/not/exist");
        cache.insert(missing, &[], "output".to_string());
        assert!(cache.get(missing, &[]).is_none());
    }

    #[test]
    fn modified_binary_misses() {
        let dir = tempfile::tempdir().unwrap();
        let bin = binary(dir.path());
        let cache = ExecCache::in_memory();
        cache.insert(&bin, &[], "v1\n".to_string());

        fs::write(&bin, "v2 different size").unwrap();
        assert!(cache.get(&bin, &[]).is_none());
    }

    #[test]
    fn disk_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let bin = binary(dir.path());
        let cache_path = dir.path().join("cache/exec_cache.json");

        ExecCache::load(cache_path.clone()).insert(&bin, &["--version"], "1.0\n".to_string());

        let reloaded = ExecCache::load(cache_path);
        assert_eq!(reloaded.get(&bin, &["--version"]).as_deref(), Some("1.0\n"));
    }
}
