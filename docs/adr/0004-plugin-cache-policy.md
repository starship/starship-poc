# Plugin caches have no TTL; plugins keep entries fresh through their keys

The SDK gives plugins a typed `Cache<K, V>` and a `memoize(namespace, key, compute)` helper for caching across renders, the common case being VCS plugins memoizing pwd-keyed results. The framework offers no TTL or automatic eviction. Plugins handle staleness by building their cache keys from the state that would change the answer (file mtime, content hash, and so on), the same way the `exec` cache keys on a binary's path, size and mtime.

## Consequences

- Each cache lives in its plugin's own process (ADR-0006), so plugins can't read or write each other's entries. The namespace string only subdivides one plugin's cache (e.g. `"root"` and `"branch"`); it isn't a security boundary.
- `Cache<K, V>` is in-memory and lasts as long as the plugin process; a restart re-warms it naturally. `exec` cache entries also persist to a per-plugin cache directory the daemon provides.
- Unbounded growth is possible (a user `cd`-ing through many directories in one session). That's acceptable given typical entry sizes of a few hundred bytes. LRU bounds can come later if profiling shows they matter.

## Considered Options

- **Framework-managed TTL.** Rejected because a TTL encodes "things change after N seconds", which is wrong for both stable values (the project root rarely changes, so a TTL forces needless recomputation) and volatile ones (the branch can change at any moment, so a TTL is either too short to help or too long to be correct). Keys built from the right state handle both.
