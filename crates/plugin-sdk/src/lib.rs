//! SDK for building Starship plugins.
//!
//! A plugin is a binary crate: implement [`Plugin`] or [`VcsPlugin`], then
//! annotate an inherent impl block with `#[export_plugin]` or
//! `#[export_vcs_plugin]`. The macro generates `main`, which answers the
//! daemon's requests over stdin and stdout.

pub use serde_json;
pub use starship_plugin_macros::{export_plugin, export_vcs_plugin};

mod ctx;
#[doc(hidden)]
pub mod dispatch;
mod exec_cache;
#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use ctx::Ctx;

/// Required contract for all Starship plugins.
///
/// Provides the plugin's identity and applicability logic. The `#[export_plugin]`
/// macro references this trait to generate the plugin's `main`, so failing to
/// implement it is a compile error.
///
/// Plugin-specific methods go in a separate `#[export_plugin] impl` block.
/// Each takes `&self` and may also take `ctx: &Ctx`.
pub trait Plugin: Default {
    /// Unique identifier for the plugin.
    const NAME: &str;

    /// Whether the plugin applies to this render.
    ///
    /// Typically checks whether the package's relevant config files
    /// (e.g. `package.json`, `Cargo.toml`) are present.
    fn is_applicable(&self, ctx: &Ctx) -> bool;
}

/// A version control system backend, exposed as a plugin.
///
/// Implement this trait when adding a new VCS (git, jj, hg, ...). Pair it with
/// an inherent `impl` block annotated `#[export_vcs_plugin]` to generate the
/// plugin's `main`. Per-VCS methods (e.g. `jj.change_id`) go
/// in that inherent block.
pub trait VcsPlugin: Default {
    /// Unique identifier for the plugin.
    const NAME: &'static str;

    /// VCSes this one supersedes when colocated, e.g. `&["git"]` on jj.
    const SHADOWS: &'static [&'static str] = &[];

    /// Distance from `ctx.pwd()` to the nearest sentinel (e.g. `.git`,
    /// `.jj`), where `0` means the sentinel is in pwd itself. `None` if no
    /// sentinel is found up to the filesystem root.
    fn detect_depth(&self, ctx: &Ctx) -> Option<u32>;

    /// Canonical project root path from the underlying VCS, or `None`
    /// when not derivable (bare repos, edge cases).
    fn root(&self, ctx: &Ctx) -> Option<String>;

    /// Current branch name, or `None` for detached HEAD or failure.
    fn branch(&self, ctx: &Ctx) -> Option<String>;
}
