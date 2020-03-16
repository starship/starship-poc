use anyhow::{Result};
use std::path::Path;

/// A struct representing a version control system instance for a project
trait Vcs: Sized {
    /// Check that the current directory is being tracked by a VCS.
    /// Returns an instance of `Vcs` if the directory is being tracked.
    fn check_dir(path: &Path) -> Result<Self>;

    /// Retreive the branch name of the project root.
    fn branch(&self) -> Result<String>;

    /// Determine the status of a VCS system of the project root.
    fn status(&self) -> Result<VcsStatus>;
}

/// A simplified representation of the current VCS status.
pub struct VcsStatus {
    untracked: bool,
    added: bool,
    modified: bool,
    renamed: bool,
    deleted: bool,
    stashed: bool,
    unmerged: bool,
    ahead: bool,
    behind: bool,
    diverged: bool,
}

struct Git {}

impl Vcs for Git {
    fn check_dir(path: &Path) -> Result<Self> {
        let path_to_check = path.join(".git");
        match path_to_check.exists() {
            true => Ok(Git{}),
            false => Err(anyhow!("\".git\" not present in {:?}", path_to_check))
        }
    }

    fn branch(&self) -> Result<String> {
        // TODO: Retreive the branch name from `.git/HEAD`
        unimplemented!()
    }

    fn status(&self) -> Result<VcsStatus> {
        // TODO: Parse the git status from `git status --porcelain`
        unimplemented!()
    }
}

// fn git_status(path: &Path) -> Result<GitStatus> {
//     let output = Command::new("git").args(&["status", "--porcelain"]).output()?;
//     let output_string = String::from_utf8(output.stdout)?;
//     parse(output_string)
// }

// fn parse_status_output(git_status: String) {
//     let files: Vec<&str> = git_status.split("\n").collect();
//     for file in files.into_iter() {
//         status_for_file=(file)
//     }
// }

// fn parse_status_line(line: &str) -> Result<RespositoryStatus> {
//     let status = line.get(0..2);
//     println!("{}", status)
// }
