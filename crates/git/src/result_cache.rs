use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::CommitId;

#[derive(Debug, thiserror::Error)]
pub enum ResultCacheError {
    #[error("cannot describe the settings of the cached result")]
    Settings(#[source] serde_json::Error),
    #[error("cannot name the cache entry")]
    EntryName(#[source] gix::Error),
    #[error("cannot write the cache entry {}", path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub struct ResultCache {
    folder: PathBuf,
}

impl ResultCache {
    pub fn in_folder(folder: PathBuf) -> ResultCache {
        ResultCache { folder }
    }

    pub fn folder(&self) -> &Path {
        &self.folder
    }

    pub fn read<T: DeserializeOwned>(
        &self,
        _commit: CommitId,
        _settings: &impl Serialize,
    ) -> Option<T> {
        todo!()
    }

    pub fn write<T: Serialize>(
        &self,
        _commit: CommitId,
        _settings: &impl Serialize,
        _result: &T,
    ) -> Result<(), ResultCacheError> {
        todo!()
    }
}
