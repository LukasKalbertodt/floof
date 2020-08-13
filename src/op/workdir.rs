use std::path::{Path, PathBuf};
use serde::Deserialize;
use crate::{
    Context, Task,
    prelude::*,
};
use super::{Finished, Operation, Operations, Outcome, RunningOperation};


#[derive(Debug, Clone)]
pub struct WorkDir(pub PathBuf);


#[derive(Debug, Clone, Deserialize)]
pub struct SetWorkDir(String);

impl SetWorkDir {
    pub const KEYWORD: &'static str = "set-workdir";
}

impl Operation for SetWorkDir {
    fn keyword(&self) -> &'static str {
        Self::KEYWORD
    }

    fn dyn_clone(&self) -> Box<dyn Operation> {
        Box::new(self.clone())
    }

    fn start(&self, ctx: &Context) -> Result<Box<dyn RunningOperation>> {
        let new_workdir = ctx.join_workdir(&self.0);
        if !new_workdir.is_dir() {
            bail!(
                "'{}' is not a valid path to a directory (or it is inaccessible)",
                new_workdir.display(),
            );
        }

        msg!(- [ctx]["set-workdir"] "set working directory to {[cyan]}", new_workdir.display());

        let dir = WorkDir(new_workdir);
        ctx.top_frame.insert_var(dir);

        Ok(Box::new(Finished(Outcome::Success)))
    }
}
