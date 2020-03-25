use anyhow::Result;
use once_cell::sync::OnceCell;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A struct representing a version control system instance for a project
pub trait Vcs {
    /// Get the VCS instance for a given directory.
    /// Returns an instance of `Vcs` if the directory is being tracked.
    fn get_vcs(path: &Path) -> Option<Box<Self>>;

    /// Get the project root.
    fn root(&self) -> &Path;

    /// Retreive the branch name of the project root.
    fn branch(&self) -> Result<&String>;

    /// Determine the status of a VCS system of the project root.
    fn status(&self) -> Result<&VcsStatus>;
}

#[derive(Debug)]
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

#[derive(Debug)]
pub struct Git {
    git_dir: PathBuf,
    root_dir: PathBuf,
    branch: OnceCell<String>,
    status: OnceCell<VcsStatus>,
}

impl Vcs for Git {
    fn root(&self) -> &Path {
        self.root_dir.as_ref()
    }

    fn branch(&self) -> Result<&String> {
        self.branch.get_or_try_init(|| self.git_branch())
    }

    fn status(&self) -> Result<&VcsStatus> {
        self.status.get_or_try_init(|| self.git_status())
    }

    fn get_vcs(path: &Path) -> Option<Box<Self>> {
        let path_to_check = path.join(".git");
        if !path_to_check.exists() {
            return None;
        }

        Some(Box::new(Git {
            git_dir: path.join(".git"),
            root_dir: path.to_path_buf(),
            branch: OnceCell::new(),
            status: OnceCell::new(),
        }))
    }
}

impl Git {
    /// Extract the branch name from `.git/HEAD`
    ///
    /// Example file contents:
    /// ```
    /// ref: refs/heads/master
    /// ```
    fn git_branch(&self) -> Result<String> {
        let head_file = self.git_dir.join("HEAD");
        let head_contents = fs::read_to_string(head_file)?;
        let branch_start = head_contents.rfind('/').ok_or(anyhow!("Unable to extract branch name"))?;
        let branch_name = &head_contents[branch_start + 1..];
        let trimmed_branch_name = branch_name.trim_end();
        Ok(trimmed_branch_name.to_owned())
    }

    /// Get git status by running `git status --porcelain`
    fn git_status(&self) -> Result<VcsStatus> {
        let path_str = self.root_dir.to_str().ok_or(anyhow!("Unable to parse path"))?;
        let output = Command::new("git")
            .args(&["-C", path_str, "status", "--porcelain", "--branch"])
            .output()?;
        let output_string = String::from_utf8(output.stdout)?;
        parse_porcelain_output(output_string)
    }
}

/// Parse git status values from `git status --porcelain`
///
/// Example porcelain output:
/// ```sh
///  M src/prompt.rs
///  M src/main.rs
/// ```
fn parse_porcelain_output(porcelain_str: String) -> Result<VcsStatus> {
    let mut porcelain_lines = porcelain_str.lines();
    let file_status = status_from_porcelain_lines(porcelain_lines);

    Ok(VcsStatus {
        untracked: file_status.contains(GitFileStatus::UNTRACKED),
        added: file_status.contains(GitFileStatus::ADDED),
        modified: file_status.contains(GitFileStatus::MODIFIED),
        renamed: file_status.contains(GitFileStatus::RENAMED),
        deleted: file_status.contains(GitFileStatus::DELETED),
        stashed: false,
        unmerged: false,
        ahead: false,
        behind: false,
        diverged: false,
    })
}

bitflags! {
    /// Status flags for a single file
    #[derive(Default)]
    pub struct GitFileStatus: u8 {
        const ADDED =     0b0000001;
        const MODIFIED =  0b0000010;
        const DELETED =   0b0000100;
        const RENAMED =   0b0001000;
        const COPIED =    0b0010000;
        const UPDATED =   0b0100000;
        const UNTRACKED = 0b1000000;
    }
}

/// Parse VCS status from `git status --porcelain`
/// https://git-scm.com/docs/git-status#_output
fn status_from_porcelain_lines(porcelain_lines: std::str::Lines) -> GitFileStatus {
    let empty_status: GitFileStatus = Default::default();

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

/// Return the file status, given the "short format" letter of a status
/// https://git-scm.com/docs/git-status#_short_format
fn parse_status_letter(letter: char) -> GitFileStatus {
    match letter {
        'A' => GitFileStatus::ADDED,
        'M' => GitFileStatus::MODIFIED,
        'D' => GitFileStatus::DELETED,
        'R' => GitFileStatus::RENAMED,
        'C' => GitFileStatus::COPIED,
        'U' => GitFileStatus::UPDATED,
        '?' => GitFileStatus::UNTRACKED,
        _ => Default::default(),
    }
}
