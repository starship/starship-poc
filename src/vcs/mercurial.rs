use super::{Vcs, VcsStatus};
use anyhow::Result;
use once_cell::sync::OnceCell;

use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Mercurial {
    hg_dir: PathBuf,
    root_dir: PathBuf,
    branch: OnceCell<String>,
    status: OnceCell<VcsStatus>,
}

impl Vcs for Mercurial {
    fn scan(path: &Path) -> Option<Box<dyn Vcs>> {
        let vcs_path = path.join(".hg");
        if !vcs_path.exists() {
            log::trace!("[ ] No Mercurial repository found");
            return None;
        }

        log::trace!("[x] Mercurial repository found");
        Some(Box::new(Mercurial {
            hg_dir: vcs_path,
            root_dir: path.into(),
            branch: OnceCell::new(),
            status: OnceCell::new(),
        }))
    }

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
    fn hg_branch(&self) -> Result<String> {
        unimplemented!()
    }

    fn hg_status(&self) -> Result<VcsStatus> {
        unimplemented!()
    }
}
