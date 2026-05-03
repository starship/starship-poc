# Multi-VCS resolution: innermost wins, ties broken by declared shadow relationships

When multiple VCS plugins detect themselves for a given `pwd`, the innermost (smallest `detect_depth`) wins. Same-depth ties — the common jj-on-git colocated case — are resolved by VCS plugins declaring shadow relationships in source: `const SHADOWS: &[&str] = &["git"]` on the jj plugin means "I shadow git when colocated, and `vcs.*` should resolve to me, not git." This expresses *semantic intent* ("jj sits on top of git") rather than ordinal priority, which would have forced plugin authors to coordinate magic constants without any framework guidance on what the numbers mean.

## Consequences

Same-depth detections with no shadow relationship between them (rare: e.g., a directory that is somehow both a pijul and a fossil repo) fall through to a documented deterministic fallback: alphabetical by plugin name, with a warning logged. The runtime validates `SHADOWS` at plugin discovery time — unknown plugin names, duplicates, self-shadows, and cycles all surface as load-time errors rather than silent runtime weirdness. Note that `Plugin::is_applicable` for a *non-Active* but *detected* VCS plugin (e.g. git in a jj-on-git repo) still returns true — `git.branch` works directly even when `vcs.branch` resolves to jj. "Active VCS" governs `vcs.*`; per-plugin Lua names are independent.

## Considered Options

- **Plugin-declared priority numbers** (`const PRIORITY: u32 = 100`). Rejected because plugin authors would have to coordinate around magic constants without any framework-level meaning. Adding a new VCS would require reading other plugins' priority values to find an unused slot. Brittle, and the ordering encodes nothing about *why* one VCS shadows another.
- **Load-order tiebreaker** (first-loaded plugin wins). Rejected because `.wasm` discovery order is filesystem-dependent — the same install could resolve differently on different machines or after `mv`-ing files in the plugin directory.
