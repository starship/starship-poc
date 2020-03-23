use anyhow::Result;
use std::path::Path;
use std::process::Command;

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
#[derive(Debug)]
pub struct VcsStatus {
    branch: String,
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

bitflags! {
    /// Status flags for a single file
    #[derive(Default)]
    pub struct VcsFileStatus: u8 {
        const ADDED =     0b0000001;
        const MODIFIED =  0b0000010;
        const DELETED =   0b0000100;
        const RENAMED =   0b0001000;
        const COPIED =    0b0010000;
        const UPDATED =   0b0100000;
        const UNTRACKED = 0b1000000;
    }
}

struct Git {}

impl Vcs for Git {
    fn check_dir(path: &Path) -> Result<Self> {
        let path_to_check = path.join(".git");
        match path_to_check.exists() {
            true => Ok(Git {}),
            false => Err(anyhow!("\".git\" not present in {:?}", path_to_check)),
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

pub fn git_status(path: &Path) -> Result<VcsStatus> {
    let output = Command::new("git")
        .args(&["status", "--porcelain", "--branch"])
        .output()?;
    let output_string = String::from_utf8(output.stdout)?;
    git_status_parse(output_string)
}

/// Parse git status values from `git status --porcelain`
///
/// Example porcelain output:
/// ```sh
/// ## master
///  M src/prompt.rs
/// ```
fn git_status_parse(git_status_str: String) -> Result<VcsStatus> {
    let mut git_status_lines = git_status_str.lines();

    // Extract branch name after `## `
    let branch_line = git_status_lines
        .next()
        .ok_or(anyhow!("Branch line not found in git status"))?;
    let branch_name = branch_line
        .get(3..)
        .ok_or(anyhow!("Branch not provided in git status"))?;

    let file_status = status_from_porcelain(git_status_lines);

    Ok(VcsStatus {
        branch: branch_name.to_owned(),
        untracked: file_status.contains(VcsFileStatus::UNTRACKED),
        added: file_status.contains(VcsFileStatus::ADDED),
        modified: file_status.contains(VcsFileStatus::MODIFIED),
        renamed: file_status.contains(VcsFileStatus::RENAMED),
        deleted: file_status.contains(VcsFileStatus::DELETED),
        stashed: false,
        unmerged: false,
        ahead: false,
        behind: false,
        diverged: false,
    })
}

fn status_from_porcelain(porcelain_lines: std::str::Lines) -> VcsFileStatus {
    let empty_status:VcsFileStatus = Default::default();

    porcelain_lines.fold(empty_status, |acc, current_line| {
        let mut characters = current_line.chars();
        let letter_codes = (
            characters.next().unwrap_or(' '),
            characters.next().unwrap_or(' '),
        );

        let index_status = parse_status_letter(letter_codes.0);
        let work_tree_status = parse_status_letter(letter_codes.1);

        acc | index_status | work_tree_status
    })
}

fn parse_status_letter(letter: char) -> VcsFileStatus {
    match letter {
        'A' => VcsFileStatus::ADDED,
        'M' => VcsFileStatus::MODIFIED,
        'D' => VcsFileStatus::DELETED,
        'R' => VcsFileStatus::RENAMED,
        'C' => VcsFileStatus::COPIED,
        'U' => VcsFileStatus::UPDATED,
        '?' => VcsFileStatus::UNTRACKED,
        _ => Default::default()
    }
}
