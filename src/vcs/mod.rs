use anyhow::Result;
use std::path::Path;
use core::fmt::Debug;

pub mod git;
pub use git::Git;

pub mod mercurial;
pub use mercurial::Mercurial;

/// A struct representing a version control system instance for a project
pub trait Vcs: Debug {
    /// Create a new VCS instance if the given directory is being tracked
    fn new(path: &Path) -> Option<Box<dyn Vcs>> where Self: Sized;

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

/// Determine the root of the project, and return an instance of the VCS tracking it
pub fn get_vcs_instance(path: &Path) -> Result<Box<dyn Vcs>> {
    if let Some(vcs_instance) = Git::new(path) {
        return Ok(vcs_instance);
    }

    if let Some(vcs_instance) = Mercurial::new(path) {
        return Ok(vcs_instance);
    }

    match path.parent() {
        Some(parent) => get_vcs_instance(parent),
        None => Err(anyhow!("Cannot find root")),
    }
}
