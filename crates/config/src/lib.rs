#![forbid(unsafe_code)]

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
#[error("cannot read the current directory")]
pub struct ConfigError(#[source] std::io::Error);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Check,
}

#[derive(Debug)]
pub struct Invocation {
    pub command: Command,
    pub working_directory: PathBuf,
}

pub fn read_invocation() -> Result<Invocation, ConfigError> {
    todo!()
}
