# Plugin SDK provides typed cache helpers; staleness is the plugin's concern

Plugins that want cross-render caching (the common case being VCS plugins memoizing pwd-keyed subprocess output) get a typed `Cache<K, V>` and a `memoize(namespace, key, compute)` helper from the SDK, backed by host functions and an in-memory `HashMap` in the daemon. Per-plugin namespacing is automatic — the daemon prefixes all keys with the calling plugin's name (derived from the wasmtime caller context), so plugins cannot read or write each other's caches. The framework deliberately offers no TTL or automatic eviction; plugins handle staleness by composing their cache keys to include relevant mutating state (file mtime, content hash, etc.), mirroring how `ExecCache` keys on binary path + size + mtime.

## Consequences

- The cache lives only for the daemon's process lifetime; cold start re-warms naturally.
- Unbounded growth is theoretically possible (a user `cd`-ing through many directories in one daemon session). Acceptable for MVP given typical entry sizes (~100s of bytes); LRU bounds can be added later if profiling shows it matters.
- A new `host::pwd()` host function ships alongside this work, since pwd-keyed caching needs explicit access to the working directory rather than implicit delivery via `update_context`.
- The user-provided namespace string passed to `Cache::new(namespace)` sub-divides *within* a plugin (e.g. one plugin holding `Cache::new("root")` and `Cache::new("branch")` separately). Per-plugin isolation is automatic; the namespace string is for sub-keying, not for security.

## Future Work

- **Disk-persisted write-through cache** (mirroring `ExecCache`). Deferred to a follow-up. The MVP ships in-memory only because the daemon is long-running and cold-start cost is rare in normal use, but the plan is to add write-through persistence once the basic cache surface is stable. The `Cache<K, V>` API is designed to absorb this without breaking changes — plugins won't notice the difference beyond cold-start warmth.

## Considered Options

- **Framework-managed TTL.** Rejected because TTLs encode "things change after N seconds," which is wrong for both stable values (project root rarely changes — TTL forces needless recomputation) and volatile values (branch can change at any moment — TTL is either too short to help or too long to be correct). Plugin-composed keys handle both cases correctly.
- **Global key namespace** (no per-plugin prefix). Rejected because it would let plugins overwrite each other's keys, creating cross-plugin coupling and hard-to-debug failures across plugin boundaries.
