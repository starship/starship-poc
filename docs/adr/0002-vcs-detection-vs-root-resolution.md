# VCS detection uses file heuristics; root resolution uses the VCS CLI

The principle "the VCS determines the project root" applies to *root resolution* (computing the canonical path), not to *detection* (deciding which VCS, if any, applies here). VCS plugins detect their presence via cheap upward sentinel walks (`.git`, `.jj`, `.hg`, `_FOSSIL_`, `.pijul`) terminating at filesystem root — this runs on every render across all VCS plugins. Only the Active VCS's `root()` is invoked, lazily via `host::project_root()`, and that method shells out to the VCS CLI for the canonical answer (correctly handling git worktrees, bare repos, submodules, etc.).

## Consequences

The exec cache (`host::exec`) deliberately does not key on `pwd`, so VCS plugins use `host::exec_uncached` for pwd-dependent calls like `git rev-parse --show-toplevel`. Cross-render caching of root resolution is a per-plugin choice, not a framework feature — the daemon only deduplicates within a single render.

## Considered Options

- **Pure VCS detection** — every VCS plugin's `is_applicable` shells out to its CLI. Rejected because 5 VCS plugins × ~10–20ms subprocess cost ≈ 50–100ms paid before any rendering work, blowing the 16.67ms (60fps) prompt budget on every render regardless of what the prompt actually displays.
- **Pure file heuristics** for both detection and root resolution. Rejected because file walking gives wrong answers in git worktrees (where `.git` is a *file*, not a directory), bare repos, and custom layouts — exactly the cases where deferring to the VCS itself is the whole point.
