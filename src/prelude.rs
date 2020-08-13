//! Just a convenience module that is imported in every module.

#![allow(unused_imports)]

pub(crate) use anyhow::{anyhow, bail, Context as _, Result, Error};
pub(crate) use crate::{cfg, context::Context, Config};
