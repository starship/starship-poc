# Starship POC

Rust rewrite of Starship structured around WASM plugins loaded at runtime. The daemon evaluates a Lua config that consumes plugin-exported data to render the prompt.

## Language

**Plugin**:
A WASM module that exposes data to the prompt config (e.g. `nodejs.version`). Has a unique `NAME`, an `is_active()` gate, and a set of exported methods.

**VCS plugin**:
A plugin that implements a version control system backend. Implements the `VcsPlugin` trait — a separate, parallel trait to `Plugin` (not a sub-trait). VCS plugins don't carry a generic `is_applicable()` gate; their gate is `detect_depth().is_some()`, derived by the macro at the WASM ABI layer. The MVP targets git, jj, hg, pijul, and fossil.
_Avoid_: VCS module, VCS backend (when referring to the plugin itself; "backend" is fine for the underlying tool)

**Active VCS**:
The singular VCS plugin whose `is_active()` returns true for the current `pwd`. At most one is active at a time. Determined per render.

**`vcs` (Lua global)**:
Resolves at render time to the Active VCS's exported methods. Lets configs write `vcs.branch` and `vcs.root` instead of branching across backend names. Returns nil-ish values when no VCS is active. The MVP surface is `root()` and `branch()` — concepts that translate across all five backends. Backend-specific data (jj's `change_id`, fossil's `checkout_uuid`) is reached via the concrete plugin name (`jj.change_id`).

**Project root**:
The absolute path to the top of the working tree, as reported by the Active VCS. The VCS owns this answer — never inferred from walking up looking for language manifest files (`package.json`, `Cargo.toml`, etc.). Language plugins use the Project root as their reference frame, not their own file heuristics.

When no Active VCS is present (no sentinel found anywhere up to filesystem root), the Project root is `None`. Language plugins decide their own fallback in that case (typically: scope checks to `pwd` only, no upward walking).

**VCS detection** vs **root resolution**:
Two distinct steps. _Detection_ — answering "which VCS, if any, applies here?" — uses cheap upward sentinel walks (`.git`, `.jj`, `.hg`, `_FOSSIL_`, `.pijul`) terminating at filesystem root. _Root resolution_ — answering "what's the project root path?" — shells out to the VCS CLI for the canonical answer. The principle "VCS determines the project root" applies to root resolution, not to detection.

Root resolution caching is the VCS plugin's responsibility, not the framework's. The exec cache (`host::exec`) deliberately doesn't key on `pwd`, so VCS plugins use `host::exec_uncached` for pwd-dependent calls and decide for themselves whether to cache results across renders. The daemon only deduplicates within a single render.

## Relationships

- A **VCS plugin** is a **Plugin** with extra capabilities (shared interface for `root`, etc.). It remains addressable under its concrete name (`git.branch`, `jj.change_id`).
- The **`vcs` global** delegates to the **Active VCS** at render time.
- The **Project root** is computed by the **Active VCS** — never inferred from the filesystem alone.

## Example dialogue

> **Dev:** "If I'm in `~/code/myrepo/src` and that's a git repo, what does `vcs.root` return?"
> **You:** "The absolute path to `~/code/myrepo`, from `git rev-parse --show-toplevel`. Walking up to find `.git/` is only used to *detect* git as the Active VCS — git itself produces the root path."
> **Dev:** "And the `nodejs` plugin — does it walk up looking for `package.json`?"
> **You:** "No. It scopes its checks to the Project root. If a `package.json` exists anywhere within the VCS-determined root, nodejs is active."
> **Dev:** "What if I'm in a directory that isn't under any VCS?"
> **You:** "No Active VCS. `vcs.*` resolves to nil-ish values. Language plugins can fall back to `cwd` as a degenerate root, or just stay inactive — TBD."
