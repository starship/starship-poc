# Starship POC

Rust rewrite of Starship structured around WASM plugins loaded at runtime. The daemon evaluates a Lua config that consumes plugin-exported data to render the prompt.

## Language

**Plugin**:
A WASM module that exposes data to the prompt config (e.g. `nodejs.version`). Has a unique `NAME`, an `is_applicable()` gate, and a set of exported methods.

**VCS plugin**:
A plugin that implements a version control system backend. Implements the `VcsPlugin` trait — a separate, parallel trait to `Plugin` (not a sub-trait). VCS plugins don't carry a generic `is_applicable()` gate; their gate is `detect_depth().is_some()`, derived by the SDK when it answers the host's `IsApplicable` request. The MVP targets git, jj, hg, pijul, and fossil.
_Avoid_: VCS module, VCS backend (when referring to the plugin itself; "backend" is fine for the underlying tool)

**Active VCS**:
The singular VCS plugin selected per render by the resolver — among VCS plugins eligible after SHADOWS validation, the one with the smallest `detect_depth()` for the current `pwd`. Same-depth ties are broken by SHADOWS declarations; remaining ties fall through to a deterministic alphabetical fallback with a warn-level log. At most one is active at a time. Eligibility is determined at daemon load (see SHADOWS validator); selection is determined per render.

**`vcs` (Lua global)**:
Resolves at render time to the Active VCS's exported methods. Lets configs write `vcs.branch` and `vcs.root` instead of branching across backend names. Returns nil-ish values when no VCS is active. The MVP surface is `root()` and `branch()` — concepts that translate across all five backends. Backend-specific data (jj's `change_id`, fossil's `checkout_uuid`) is reached via the concrete plugin name (`jj.change_id`).

**Project root**:
The absolute path to the top of the working tree, as reported by the Active VCS. The VCS owns this answer — never inferred from walking up looking for language manifest files (`package.json`, `Cargo.toml`, etc.). Language plugins use the Project root as their reference frame, not their own file heuristics.

When no Active VCS is present (no sentinel found anywhere up to filesystem root), the Project root is `None`. Language plugins decide their own fallback in that case (typically: scope checks to `pwd` only, no upward walking).

**VCS detection** vs **root resolution**:
Two distinct steps. _Detection_ — answering "which VCS, if any, applies here?" — uses cheap upward sentinel walks (`.git`, `.jj`, `.hg`, `_FOSSIL_`, `.pijul`) terminating at filesystem root. _Root resolution_ — answering "what's the project root path?" — shells out to the VCS CLI for the canonical answer. The principle "VCS determines the project root" applies to root resolution, not to detection.

Root resolution caching is the VCS plugin's responsibility, not the framework's. The exec cache (`host::exec`) deliberately doesn't key on `pwd`, so VCS plugins use `host::exec_uncached` for pwd-dependent calls and decide for themselves whether to cache results across renders. The daemon only deduplicates within a single render.

**SHADOWS validator**:
The check that vets each VCS plugin's `SHADOWS` declaration — the per-plugin list of other VCS plugins it supersedes when colocated, e.g. `&["git"]` on jj. The single source of truth is `pub const fn validate_per_plugin` in `plugin-core`, called from two places: `#[export_vcs_plugin]` invokes it inside a `const _: () = ...;` block so per-plugin rules (self-shadow, duplicates within a list) fail at plugin compile time; the runtime validator invokes it once per plugin at discovery and additionally runs cross-plugin rules (cycles, unknown shadow targets). Runtime failures are soft (per ADR-0005): the offending plugin is dropped from the Active VCS resolution candidate set with a logged warning, and remains loaded and callable under its concrete name.
_Avoid_: SHADOWS checker, SHADOWS linter (the runtime names this `validator`).

## Relationships

- A **VCS plugin** is a WASM plugin kind with a parallel trait (`VcsPlugin`, alongside `Plugin` rather than a sub-trait — see ADR-0001). It remains addressable under its concrete name (`git.branch`, `jj.change_id`).
- The **`vcs` global** delegates to the **Active VCS** at render time.
- The **Project root** is computed by the **Active VCS** — never inferred from the filesystem alone.
- The **SHADOWS validator** governs which **VCS plugins** are eligible to be the **Active VCS**; a failed plugin is dropped from the candidate set but remains addressable under its concrete name.

## Example dialogue

> **Dev:** "If I'm in `~/code/myrepo/src` and that's a git repo, what does `vcs.root` return?"
> **You:** "The absolute path to `~/code/myrepo`, from `git rev-parse --show-toplevel`. Walking up to find `.git/` is only used to *detect* git as the Active VCS — git itself produces the root path."
> **Dev:** "And the `nodejs` plugin — does it walk up looking for `package.json`?"
> **You:** "No. It scopes its checks to the Project root. If a `package.json` exists anywhere within the VCS-determined root, nodejs is active."
> **Dev:** "What if I'm in a directory that isn't under any VCS?"
> **You:** "No Active VCS. `vcs.*` resolves to nil-ish values. Language plugins can fall back to `cwd` as a degenerate root, or just stay inactive — TBD."
