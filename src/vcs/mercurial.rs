use super::{Vcs, VcsStatus};
use once_cell::sync::OnceCell;
use anyhow::Result;

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Mercurial {
    hg_dir: PathBuf,
    root_dir: PathBuf,
    branch: OnceCell<String>,
    status: OnceCell<VcsStatus>,
}

impl Vcs for Mercurial {
    fn root(&self) -> &Path {
        self.root_dir.as_ref()
    }

    fn branch(&self) -> Result<&String> {
        self.branch.get_or_try_init(|| self.hg_branch())
    }

    fn status(&self) -> Result<&VcsStatus> {
        // self.status.get_or_try_init(|| self.hg_status())
        unimplemented!()
    }

    fn get_vcs(&self, path: &Path) -> Option<Box<dyn Vcs>> {
        let vcs_path = path.join(".hg");
        if !vcs_path.exists() {
            return None;
        }

        Some(Box::new(Mercurial {
            hg_dir: vcs_path,
            root_dir: path.to_path_buf(),
            branch: OnceCell::new(),
            status: OnceCell::new(),
        }))
    }
}

impl Mercurial {
    fn hg_branch(&self) -> Result<String> {
        let branch_file = self.hg_dir.join("branch");
        let branch_name = fs::read_to_string(branch_file)?;
        let trimmed_branch_name = branch_name.trim();
        Ok(branch_name.into())
    }

    fn hg_status(&self) -> Result<String> {
        unimplemented!()
    }
}
