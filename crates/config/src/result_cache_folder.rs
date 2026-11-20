use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CacheFolderConvention {
    XdgBaseDirectory,
    WindowsLocalAppData,
}

impl CacheFolderConvention {
    pub(crate) fn of_this_platform() -> CacheFolderConvention {
        if cfg!(windows) {
            CacheFolderConvention::WindowsLocalAppData
        } else {
            CacheFolderConvention::XdgBaseDirectory
        }
    }
}

pub(crate) fn result_cache_folder(
    _current_folder: &Path,
    _convention: CacheFolderConvention,
    _variable: impl Fn(&str) -> Option<OsString>,
) -> Option<PathBuf> {
    todo!()
}
