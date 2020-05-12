use super::{Vcs, VcsStatus};
use anyhow::Result;
use once_cell::sync::OnceCell;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug)]
pub struct Git {
    git_dir: PathBuf,
    root_dir: PathBuf,
    branch: OnceCell<String>,
    status: OnceCell<VcsStatus>,
}

impl Vcs for Git {
    fn new(path: &Path) -> Option<Box<dyn Vcs>> {
        let vcs_path = path.join(".git");
        if !vcs_path.exists() {
            return None;
        }

        Some(Box::new(Git {
            git_dir: vcs_path,
            root_dir: path.into(),
            branch: OnceCell::new(),
            status: OnceCell::new(),
        }))
    }

    fn root(&self) -> &Path {
        self.root_dir.as_ref()
    }

    fn branch(&self) -> Result<&String> {
        self.branch.get_or_try_init(|| self.git_branch())
    }

    fn status(&self) -> Result<&VcsStatus> {
        self.status.get_or_try_init(|| self.git_status())
    }
}

impl Git {
    /// Extract the branch name from `.git/HEAD`
    ///
    /// Example file contents:
    /// ```code
    /// ref: refs/heads/master
    /// ```
    fn git_branch(&self) -> Result<String> {
        let head_file = self.git_dir.join("HEAD");
        let head_contents = fs::read_to_string(head_file)?;
        let branch_start = head_contents
            .rfind('/')
            .ok_or(anyhow!("Unable to extract branch name"))?;
        let branch_name = &head_contents[branch_start + 1..];
        let trimmed_branch_name = branch_name.trim_end();
        Ok(trimmed_branch_name.into())
    }

    /// Get git status by running `git status --porcelain`
    fn git_status(&self) -> Result<VcsStatus> {
        let path_str = self
            .root_dir
            .to_str()
            .ok_or(anyhow!("Unable to parse path"))?;
        let output = Command::new("git")
            .args(&["-C", path_str, "status", "--porcelain"])
            .output()?;
        let output_string = String::from_utf8(output.stdout)?;
        parse_porcelain_output(output_string)
    }
}

/// Parse git status values from `git status --porcelain`
///
/// Example porcelain output:
/// ```code
///  M src/prompt.rs
///  M src/main.rs
/// ```
fn parse_porcelain_output<S: Into<String>>(porcelain: S) -> Result<VcsStatus> {
    let porcelain_str = porcelain.into();
    let porcelain_lines = porcelain_str.lines();
    let mut vcs_status: VcsStatus = Default::default();

    porcelain_lines.for_each(|line| {
        let mut characters = line.chars();

        // Extract the first two letter of each line
        let letter_codes = (
            characters.next().unwrap_or(' '),
            characters.next().unwrap_or(' '),
        );

        if (letter_codes.0 == letter_codes.1) {
            increment_vcs_status(&mut vcs_status, letter_codes.0);
        } else {
            increment_vcs_status(&mut vcs_status, letter_codes.0);
            increment_vcs_status(&mut vcs_status, letter_codes.1);
        }
    });

    Ok(vcs_status)
}

/// Return the file status, given the "short format" letter of a status
/// https://git-scm.com/docs/git-status#_short_format
fn increment_vcs_status(vcs_status: &mut VcsStatus, letter: char) {
    match letter {
        'A' => vcs_status.added += 1,
        'M' => vcs_status.modified += 1,
        'D' => vcs_status.deleted += 1,
        'R' => vcs_status.renamed += 1,
        'C' => vcs_status.added += 1,
        'U' => vcs_status.modified += 1,
        '?' => vcs_status.untracked += 1,
        _ => (),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_porcelain_output() -> Result<()> {
        let output = parse_porcelain_output("")?;

        let expected: VcsStatus = Default::default();
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_parse_porcelain_output() -> Result<()> {
        let output = parse_porcelain_output(
            "M src/prompt.rs
MM src/main.rs
A src/formatter.rs
? README.md",
        )?;

        let expected = VcsStatus {
            modified: 2,
            added: 1,
            untracked: 1,
            ..Default::default()
        };
        assert_eq!(output, expected);
        Ok(())
    }
}
