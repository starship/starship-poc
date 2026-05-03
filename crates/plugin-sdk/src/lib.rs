//! SDK for building Starship WASM plugins.

pub use serde_json;
pub use starship_plugin_core;
pub use starship_plugin_core::{alloc, dealloc, read_msg, write_msg};
pub use starship_plugin_macros::{export_plugin, export_vcs_plugin};

pub mod host;

/// Required contract for all Starship plugins.
///
/// Provides the plugin's identity and applicability logic. The `#[export_plugin]`
/// macro references this trait to generate WASM exports, so failing to
/// implement it is a compile error.
///
/// Plugin-specific methods go in a separate `#[export_plugin] impl` block.
pub trait Plugin: Default {
    /// Unique identifier for the plugin.
    const NAME: &str;

    /// Whether the plugin should be queried for the current directory.
    fn is_applicable(&self) -> bool;
}

/// Required contract for VCS plugins.
///
/// Sibling trait to `Plugin` — VCS plugins implement this instead of `Plugin`.
/// The `#[export_vcs_plugin]` macro derives `_plugin_is_applicable` from
/// `detect_depth().is_some()` so authors don't write a generic gate predicate.
///
/// `SHADOWS` declares which other VCS plugins this one shadows when colocated
/// (e.g. jj declares `SHADOWS = &["git"]` to win the resolver tiebreaker on
/// jj-on-git checkouts). See ADR-0003 for the resolution semantics.
pub trait VcsPlugin: Default {
    /// Unique identifier for the plugin.
    const NAME: &'static str;

    /// VCS plugins this one shadows in the resolver tiebreaker.
    const SHADOWS: &'static [&'static str] = &[];

    /// Distance from `pwd` to the nearest sentinel (e.g. `.git`, `.jj`),
    /// where `0` means the sentinel is in `pwd` itself. `None` if no
    /// sentinel is found up to the filesystem root.
    fn detect_depth(&self) -> Option<u32>;

    /// Canonical project root path from the underlying VCS, or `None`
    /// when not derivable (bare repos, edge cases).
    fn root(&self) -> Option<String>;

    /// Current branch name, or `None` for detached HEAD or failure.
    fn branch(&self) -> Option<String>;
}
