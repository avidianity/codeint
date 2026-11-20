use std::io;
use std::path::{Path, PathBuf};

use codeint_model::language::Manifest;
use oxc_resolver::{FileMetadata, FileSystem, ResolveError};

use crate::resolution::packages::PackageLink;

#[derive(Default)]
pub(super) struct Snapshot {}

impl Snapshot {
    pub(super) fn new(
        repository_paths: &[String],
        manifests: Vec<Manifest>,
        links: Vec<PackageLink>,
    ) -> Snapshot {
        todo!(
            "{repository_paths:?} {} {:?}",
            manifests.len(),
            links
                .iter()
                .map(|link| (&link.name, &link.folder))
                .collect::<Vec<_>>()
        )
    }

    pub(super) fn is_empty(&self) -> bool {
        todo!()
    }
}

pub(super) fn absolute(repository_path: &str) -> PathBuf {
    todo!("{repository_path}")
}

pub(super) fn repository_path(absolute: &Path) -> Option<String> {
    todo!("{absolute:?}")
}

impl FileSystem for Snapshot {
    fn new() -> Snapshot {
        Snapshot::default()
    }

    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        todo!("{path:?}")
    }

    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        todo!("{path:?}")
    }

    fn metadata(&self, path: &Path) -> io::Result<FileMetadata> {
        todo!("{path:?}")
    }

    fn symlink_metadata(&self, path: &Path) -> io::Result<FileMetadata> {
        todo!("{path:?}")
    }

    fn read_link(&self, path: &Path) -> Result<PathBuf, ResolveError> {
        todo!("{path:?}")
    }

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        todo!("{path:?}")
    }
}
