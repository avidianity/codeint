use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Component, Path, PathBuf};

use codeint_model::language::Manifest;
use oxc_resolver::{FileMetadata, FileSystem, ResolveError};

use crate::resolution::packages::PackageLink;

const PACKAGE_FOLDER: &str = "node_modules";

#[derive(Default)]
pub(super) struct Snapshot {
    files: HashSet<String>,
    folders: HashSet<String>,
    texts: HashMap<String, String>,
    links: HashMap<String, String>,
}

enum Entry {
    File,
    Folder,
    Link,
}

impl Snapshot {
    pub(super) fn new(
        repository_paths: &[String],
        manifests: Vec<Manifest>,
        links: Vec<PackageLink>,
    ) -> Snapshot {
        let mut snapshot = Snapshot {
            files: repository_paths.iter().cloned().collect(),
            ..Snapshot::default()
        };
        snapshot.folders.insert(String::new());
        for path in repository_paths {
            snapshot.add_folders_above(path);
        }
        for link in links {
            let link_path = format!("{PACKAGE_FOLDER}/{}", link.name);
            snapshot.add_folders_above(&link_path);
            snapshot.links.insert(link_path, link.folder);
        }
        snapshot.texts = manifests
            .into_iter()
            .map(|manifest| (manifest.path, manifest.text))
            .collect();
        snapshot
    }

    fn add_folders_above(&mut self, path: &str) {
        let mut folder = path;
        while let Some((parent, _)) = folder.rsplit_once('/') {
            if !self.folders.insert(parent.to_owned()) {
                return;
            }
            folder = parent;
        }
    }

    fn entry(&self, path: &str) -> Option<Entry> {
        if self.links.contains_key(path) {
            Some(Entry::Link)
        } else if self.files.contains(path) {
            Some(Entry::File)
        } else if self.folders.contains(path) {
            Some(Entry::Folder)
        } else {
            None
        }
    }

    fn locate(&self, path: &Path, follow_last_link: bool) -> io::Result<String> {
        let relative = path.strip_prefix(root()).map_err(|_| not_found(path))?;
        let mut segments: Vec<String> = Vec::new();
        let components: Vec<Component<'_>> = relative.components().collect();
        for (index, component) in components.iter().enumerate() {
            match component {
                Component::Normal(name) => {
                    segments.push(name.to_str().ok_or_else(|| not_found(path))?.to_owned());
                }
                Component::ParentDir => {
                    segments.pop();
                }
                Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
            }
            let is_last = index + 1 == components.len();
            if (follow_last_link || !is_last)
                && let Some(target) = self.links.get(&segments.join("/"))
            {
                segments = if target.is_empty() {
                    Vec::new()
                } else {
                    target.split('/').map(str::to_owned).collect()
                };
            }
        }
        Ok(segments.join("/"))
    }

    fn metadata_of(&self, path: &Path, follow_last_link: bool) -> io::Result<FileMetadata> {
        let located = self.locate(path, follow_last_link)?;
        match self.entry(&located) {
            Some(Entry::File) => Ok(FileMetadata::new(true, false, false)),
            Some(Entry::Folder) => Ok(FileMetadata::new(false, true, false)),
            Some(Entry::Link) => Ok(FileMetadata::new(false, false, true)),
            None => Err(not_found(path)),
        }
    }
}

pub(super) fn root() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(r"C:\")
    } else {
        PathBuf::from("/")
    }
}

pub(super) fn absolute(repository_path: &str) -> PathBuf {
    let mut path = root();
    path.extend(
        repository_path
            .split('/')
            .filter(|segment| !segment.is_empty()),
    );
    path
}

pub(super) fn repository_path(absolute: &Path) -> Option<String> {
    absolute
        .strip_prefix(root())
        .ok()?
        .components()
        .map(|component| match component {
            Component::Normal(name) => name.to_str(),
            _ => None,
        })
        .collect::<Option<Vec<&str>>>()
        .map(|segments| segments.join("/"))
}

fn not_found(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!("{} is not in the repository snapshot", path.display()),
    )
}

impl FileSystem for Snapshot {
    fn new() -> Snapshot {
        Snapshot::default()
    }

    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        self.read_to_string(path).map(String::into_bytes)
    }

    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        let located = self.locate(path, true)?;
        self.texts
            .get(&located)
            .cloned()
            .ok_or_else(|| not_found(path))
    }

    fn metadata(&self, path: &Path) -> io::Result<FileMetadata> {
        self.metadata_of(path, true)
    }

    fn symlink_metadata(&self, path: &Path) -> io::Result<FileMetadata> {
        self.metadata_of(path, false)
    }

    fn read_link(&self, path: &Path) -> Result<PathBuf, ResolveError> {
        let located = self.locate(path, false)?;
        self.links
            .get(&located)
            .map(|target| absolute(target))
            .ok_or_else(|| ResolveError::from(not_found(path)))
    }

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        let located = self.locate(path, true)?;
        match self.entry(&located) {
            Some(_) => Ok(absolute(&located)),
            None => Err(not_found(path)),
        }
    }
}
