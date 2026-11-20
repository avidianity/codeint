use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

const RESULT_CACHE_FOLDER_VARIABLE: &str = "CODEINT_CACHE_DIR";
const CODEINT_FOLDER_NAME: &str = "codeint";

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
    current_folder: &Path,
    convention: CacheFolderConvention,
    variable: impl Fn(&str) -> Option<OsString>,
) -> Option<PathBuf> {
    let non_empty = |name: &str| variable(name).filter(|value| !value.is_empty());
    if let Some(chosen) = non_empty(RESULT_CACHE_FOLDER_VARIABLE) {
        return Some(current_folder.join(chosen));
    }
    let absolute = |name: &str| {
        non_empty(name)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    let user_cache_folder = match convention {
        CacheFolderConvention::XdgBaseDirectory => {
            absolute("XDG_CACHE_HOME").or_else(|| absolute("HOME").map(|home| home.join(".cache")))
        }
        CacheFolderConvention::WindowsLocalAppData => absolute("LOCALAPPDATA"),
    };
    Some(user_cache_folder?.join(CODEINT_FOLDER_NAME))
}
