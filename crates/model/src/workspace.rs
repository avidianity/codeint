use std::collections::BTreeSet;
use std::fmt;

use crate::{InvalidRepoPath, RepoPath, language};

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("cannot find the workspace packages from {path}: {message}")]
    Manifest { path: String, message: String },
    #[error("cannot find the workspace packages from {plugin}: {message}")]
    PluginFailed { plugin: String, message: String },
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

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SkippedManifest {
    pub path: String,
    pub plugin: String,
    pub reason: String,
}

impl SkippedManifest {
    pub fn folder(&self) -> &str {
        self.path.rsplit_once('/').map_or(".", |(folder, _)| folder)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Workspace {
    member_roots: BTreeSet<RepoPath>,
    skipped_manifests: BTreeSet<SkippedManifest>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Package<'workspace> {
    Root,
    Member(&'workspace RepoPath),
}

impl Workspace {
    pub fn new(member_roots: impl IntoIterator<Item = RepoPath>) -> Workspace {
        Workspace {
            member_roots: member_roots.into_iter().collect(),
            skipped_manifests: BTreeSet::new(),
        }
    }

    pub fn with_skipped_manifests(
        mut self,
        skipped: impl IntoIterator<Item = SkippedManifest>,
    ) -> Workspace {
        self.skipped_manifests.extend(skipped);
        self
    }

    pub fn skipped_manifest_in(&self, folder: &str) -> Option<&SkippedManifest> {
        self.skipped_manifests
            .iter()
            .find(|skipped| skipped.folder() == folder)
    }

    pub fn join(&mut self, other: Workspace) {
        self.member_roots.extend(other.member_roots);
        self.skipped_manifests.extend(other.skipped_manifests);
    }

    pub fn has_members(&self) -> bool {
        !self.member_roots.is_empty()
    }

    pub fn member_roots(&self) -> impl Iterator<Item = &RepoPath> {
        self.member_roots.iter()
    }

    pub fn packages(&self) -> impl Iterator<Item = Package<'_>> {
        std::iter::once(Package::Root).chain(self.member_roots.iter().map(Package::Member))
    }

    pub fn package_of(&self, path: &RepoPath) -> Package<'_> {
        let mut folder = path.as_str();
        while let Some((parent, _)) = folder.rsplit_once('/') {
            if let Some(root) = self.member_roots.get(parent) {
                return Package::Member(root);
            }
            folder = parent;
        }
        Package::Root
    }
}

impl fmt::Display for Package<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Package::Root => formatter.write_str("."),
            Package::Member(root) => root.fmt(formatter),
        }
    }
}
