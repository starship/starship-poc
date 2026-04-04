//! Binary output cache for host exec calls.
//!
//! Caches command output keyed by the resolved binary's identity (absolute path,
//! file size, mtime) and command arguments. This avoids re-executing commands
//! like `node --version` on every prompt render when the underlying binary
//! hasn't changed.
//!
//! The cache is write-through: every new entry is persisted to
//! `$XDG_CACHE_HOME/starship/exec_cache.json` immediately.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tracing::instrument;

/// Cache key combining a binary's identity with its invocation arguments.
///
/// Binary updates (different size or mtime) naturally cause cache misses
/// because the key no longer matches, which is what makes this safe to use
/// with version managers that swap binaries in PATH.
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

/// In-memory cache of binary exec results, backed by a JSON file on disk.
///
/// Uses `DashMap` for non-blocking concurrent reads. Writes are sharded
/// internally so readers are never blocked.
pub struct ExecCache {
    entries: DashMap<ExecCacheKey, String>,
    cache_path: Option<PathBuf>,
}

impl ExecCache {
    /// Load cache from disk, or create empty if the file is missing or corrupt.
    #[must_use]
    pub fn load(cache_path: PathBuf) -> Self {
        let entries = DashMap::new();
        if let Some(pairs) = fs::read_to_string(&cache_path)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<(ExecCacheKey, String)>>(&s).ok())
        {
            for (k, v) in pairs {
                entries.insert(k, v);
            }
        }
        Self {
            entries,
            cache_path: Some(cache_path),
        }
    }

    /// Create an in-memory-only cache with no disk persistence.
    #[cfg(any(test, feature = "testing"))]
    pub fn in_memory() -> Self {
        Self {
            entries: DashMap::new(),
            cache_path: None,
        }
    }

    /// Look up a cached exec result.
    ///
    /// Resolves `cmd` to an absolute path via PATH, stats the binary for
    /// size/mtime, and checks the cache. Returns `None` on cache miss or if
    /// the binary can't be resolved/statted.
    #[instrument(skip(self), fields(%cmd))]
    pub fn get(&self, cmd: &str, args: &[String]) -> Option<String> {
        let key = build_key(cmd, args)?;
        self.entries.get(&key).map(|v| v.value().clone())
    }

    /// Insert a result and write-through to disk.
    ///
    /// No-op if the binary can't be resolved or statted (the result is
    /// still returned to the caller, just not cached).
    #[instrument(skip(self, output), fields(%cmd))]
    pub fn insert(&self, cmd: &str, args: &[String], output: String) {
        let Some(key) = build_key(cmd, args) else {
            return;
        };
        self.entries.insert(key, output);
        self.flush();
    }

    /// Persist the full cache to disk as a JSON array of `[key, value]` pairs.
    #[instrument(skip(self))]
    fn flush(&self) {
        let Some(path) = &self.cache_path else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let entries: Vec<_> = self
            .entries
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();
        if let Ok(json) = serde_json::to_string(&entries) {
            let _ = fs::write(path, json);
        }
    }
}

/// Build a cache key by resolving the command to an absolute path and
/// statting the binary for size and mtime.
fn build_key(cmd: &str, args: &[String]) -> Option<ExecCacheKey> {
    let binary_path = match which::which(cmd) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(cmd, %e, "exec cache: which lookup failed");
            return None;
        }
    };
    key_for_path(&binary_path, args)
}

fn key_for_path(binary_path: &Path, args: &[String]) -> Option<ExecCacheKey> {
    let metadata = match fs::metadata(binary_path) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(path = %binary_path.display(), %e, "exec cache: stat failed");
            return None;
        }
    };
    let mtime = metadata.modified().ok()?;
    let duration = mtime.duration_since(UNIX_EPOCH).ok()?;
    #[allow(clippy::cast_possible_truncation)]
    Some(ExecCacheKey {
        binary_path: binary_path.to_path_buf(),
        binary_size: metadata.len(),
        mtime_nanos: duration.as_nanos() as u64,
        args: args.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_miss_then_hit() {
        let cache = ExecCache::in_memory();
        // "echo" should be resolvable on any system
        let args = vec!["hello".to_string()];
        assert!(cache.get("echo", &args).is_none());
        cache.insert("echo", &args, "hello\n".to_string());
        assert_eq!(cache.get("echo", &args).as_deref(), Some("hello\n"));
    }

    #[test]
    fn different_args_are_distinct() {
        let cache = ExecCache::in_memory();
        let args_a = vec!["a".to_string()];
        let args_b = vec!["b".to_string()];
        cache.insert("echo", &args_a, "a\n".to_string());
        assert!(cache.get("echo", &args_b).is_none());
    }

    #[test]
    fn unresolvable_command_returns_none() {
        let cache = ExecCache::in_memory();
        let args = vec![];
        assert!(cache.get("this_binary_does_not_exist_xyz", &args).is_none());
    }

    #[test]
    fn modified_binary_produces_different_key() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("bin");
        fs::write(&file, "v1").unwrap();

        let args = vec![];
        let key1 = key_for_path(&file, &args).unwrap();

        fs::write(&file, "v2 different size").unwrap();
        let key2 = key_for_path(&file, &args).unwrap();

        assert_ne!(key1, key2);
    }

    #[test]
    fn disk_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let cache_path = dir.path().join("exec_cache.json");
        let args = vec!["--version".to_string()];

        // Write
        {
            let cache = ExecCache::load(cache_path.clone());
            cache.insert("echo", &args, "1.0\n".to_string());
        }

        // Read back
        {
            let cache = ExecCache::load(cache_path);
            assert_eq!(cache.get("echo", &args).as_deref(), Some("1.0\n"));
        }
    }
}
