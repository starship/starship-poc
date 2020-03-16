use super::Vcs;
use std::process::Command;

use anyhow::{Result};

struct Git {}

impl Vcs for Git {
    fn check_dir(path: &Path) -> Result<Self> {
        let path_to_check = path.join(".git")?;
        // return path_to_check.exists();
        // if path_to_check.exists() {
        //     return Ok(path_to_check)
        // };
    }
}

fn git_status(path: &Path) -> Result<GitStatus> {
    let output = Command::new("git").args(&["status", "--porcelain"]).output()?;
    let output_string = String::from_utf8(output.stdout)?;
    parse(output_string)
}

fn parse_status_output(git_status: String) {
    let files: Vec<&str> = git_status.split("\n").collect();
    for file in files.into_iter() {
        status_for_file=(file)
    }
}

fn parse_status_line(line: &str) -> Result<RespositoryStatus> {
    let status = line.get(0..2);
    println!("{}", status)
}
