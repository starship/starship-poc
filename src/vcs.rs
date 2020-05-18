use anyhow::Result;
use core::fmt::Debug;
use std::path::Path;

pub mod git;
pub use git::Git;

pub mod mercurial;
pub use mercurial::Mercurial;

/// A trait for the ability to be used a version control system
pub trait Vcs: Debug {
    /// Create a new VCS instance if the given directory is being tracked
    fn new(path: &Path) -> Option<Box<dyn Vcs>>
    where
        Self: Sized;

    /// Get the project root.
    fn root(&self) -> &Path;

    /// Retreive the branch name of the project root.
    fn branch(&self) -> Result<&String>;

    /// Determine the status of a VCS system of the project root.
    fn status(&self) -> Result<&VcsStatus>;
}

#[derive(Default, Debug, PartialEq)]
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
///
/// This function runs the initializers of each of the supported VCS systems, returning
/// an instance of the system that is tracking the project containing the current directory.
pub fn get_vcs_instance(path: &Path) -> Option<Box<dyn Vcs>> {
    let vcs_initializers: Vec<fn(&Path) -> Option<Box<dyn Vcs>>> = vec![Git::new, Mercurial::new];

    log::trace!("Checking for VCS instance: {:?}", path);
    for initializer in vcs_initializers {
        match initializer(path) {
            Some(vcs_instance) => return Some(vcs_instance),
            None => continue,
        }
    }

    match path.parent() {
        Some(parent) => get_vcs_instance(parent),
        None => None,
    }
}
