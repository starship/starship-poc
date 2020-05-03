use super::{Vcs, VcsStatus};
use once_cell::sync::OnceCell;
use anyhow::Result;

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
        self.status.get_or_try_init(|| self.hg_status())
    }
}

impl Mercurial {
    pub fn new(path: &Path) -> Option<Box<Self>> {
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

    fn hg_branch(&self) -> Result<String> {
        unimplemented!()
    }

    fn hg_status(&self) -> Result<VcsStatus> {
        unimplemented!()
    }
}
