use anyhow::Result;
use std::path::Path;

pub mod git;
pub use git::Git;

pub mod mercurial;
pub use mercurial::Mercurial;

/// A struct representing a version control system instance for a project
pub trait Vcs {
    /// Get the VCS instance for a given directory.
    /// Returns an instance of `Vcs` if the directory is being tracked.
    fn get_vcs(&self, path: &Path) -> Option<Box<dyn Vcs>>;

    /// Get the project root.
    fn root(&self) -> &Path;

    /// Retreive the branch name of the project root.
    fn branch(&self) -> Result<&String>;

    /// Determine the status of a VCS system of the project root.
    fn status(&self) -> Result<&VcsStatus>;
}

#[derive(Default, Debug)]
pub struct VcsStatus {
    untracked: u8,
    added: u8,
    modified: u8,
    renamed: u8,
    deleted: u8,
    stashed: u8,
    unmerged: u8,
    ahead: u8,
    behind: u8,
    diverged: u8,
}
