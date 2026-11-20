use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;

use super::{CacheFolderConvention, result_cache_folder};

fn absolute(relative: &str) -> PathBuf {
    std::env::temp_dir().join(relative)
}

fn folder_with(
    convention: CacheFolderConvention,
    variables: &[(&str, PathBuf)],
) -> Option<PathBuf> {
    let variables: HashMap<String, OsString> = variables
        .iter()
        .map(|(name, value)| ((*name).to_owned(), value.clone().into_os_string()))
        .collect();
    result_cache_folder(&absolute("repository"), convention, |name| {
        variables.get(name).cloned()
    })
}

#[test]
fn xdg_cache_home_holds_the_codeint_folder() {
    let folder = folder_with(
        CacheFolderConvention::XdgBaseDirectory,
        &[
            ("XDG_CACHE_HOME", absolute("var/cache/someone")),
            ("HOME", absolute("home/someone")),
        ],
    );

    assert_eq!(folder, Some(absolute("var/cache/someone/codeint")));
}

#[test]
fn without_xdg_cache_home_the_cache_is_in_dot_cache_of_home() {
    let folder = folder_with(
        CacheFolderConvention::XdgBaseDirectory,
        &[("HOME", absolute("home/someone"))],
    );

    assert_eq!(folder, Some(absolute("home/someone/.cache/codeint")));
}

#[test]
fn an_empty_or_relative_xdg_cache_home_is_ignored_as_the_base_directory_specification_says() {
    for ignored in ["", "relative/cache"] {
        let folder = folder_with(
            CacheFolderConvention::XdgBaseDirectory,
            &[
                ("XDG_CACHE_HOME", PathBuf::from(ignored)),
                ("HOME", absolute("home/someone")),
            ],
        );

        assert_eq!(
            folder,
            Some(absolute("home/someone/.cache/codeint")),
            "XDG_CACHE_HOME={ignored:?}"
        );
    }
}

#[test]
fn without_any_home_folder_there_is_no_cache() {
    assert_eq!(
        folder_with(CacheFolderConvention::XdgBaseDirectory, &[]),
        None
    );
    assert_eq!(
        folder_with(CacheFolderConvention::WindowsLocalAppData, &[]),
        None
    );
}

#[test]
fn on_windows_the_cache_is_in_local_app_data() {
    let local_app_data = absolute("Local");

    let folder = folder_with(
        CacheFolderConvention::WindowsLocalAppData,
        &[
            ("LOCALAPPDATA", local_app_data.clone()),
            ("XDG_CACHE_HOME", absolute("var/cache/someone")),
        ],
    );

    assert_eq!(folder, Some(local_app_data.join("codeint")));
}

#[test]
fn codeint_cache_dir_chooses_the_folder_itself_on_every_platform() {
    for convention in [
        CacheFolderConvention::XdgBaseDirectory,
        CacheFolderConvention::WindowsLocalAppData,
    ] {
        let folder = folder_with(
            convention,
            &[
                ("CODEINT_CACHE_DIR", absolute("tmp/codeint-results")),
                ("XDG_CACHE_HOME", absolute("var/cache/someone")),
                ("LOCALAPPDATA", absolute("var/cache/someone")),
            ],
        );

        assert_eq!(
            folder,
            Some(absolute("tmp/codeint-results")),
            "{convention:?}"
        );
    }
}

#[test]
fn a_relative_codeint_cache_dir_is_relative_to_the_current_folder() {
    let folder = folder_with(
        CacheFolderConvention::XdgBaseDirectory,
        &[
            ("CODEINT_CACHE_DIR", PathBuf::from("cache")),
            ("HOME", absolute("home/someone")),
        ],
    );

    assert_eq!(folder, Some(absolute("repository/cache")));
}
