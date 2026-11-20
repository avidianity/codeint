use std::collections::BTreeSet;
use std::fmt;

use crate::{InvalidRepoPath, RepoPath, language};

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("cannot find the workspace packages: {path} {message}")]
    Manifest { path: String, message: String },
    #[error("a Language plugin named an invalid workspace package root")]
    InvalidPackageRoot(#[from] InvalidRepoPath),
}

impl From<language::WorkspaceError> for WorkspaceError {
    fn from(error: language::WorkspaceError) -> WorkspaceError {
        WorkspaceError::Manifest {
            path: error.path,
            message: error.message,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Workspace {
    member_roots: BTreeSet<RepoPath>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Package<'workspace> {
    Root,
    Member(&'workspace RepoPath),
}

impl Workspace {
    pub fn new(_member_roots: impl IntoIterator<Item = RepoPath>) -> Workspace {
        todo!()
    }

    pub fn has_members(&self) -> bool {
        todo!()
    }

    pub fn packages(&self) -> impl Iterator<Item = Package<'_>> {
        std::iter::once(Package::Root).chain(self.member_roots.iter().map(Package::Member))
    }

    pub fn package_of(&self, _path: &RepoPath) -> Package<'_> {
        todo!()
    }
}

impl fmt::Display for Package<'_> {
    fn fmt(&self, _formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}
