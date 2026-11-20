use crate::{RepoPath, SourceSet};

const VENDORED_FOLDERS: [&str; 2] = ["vendor", "third_party"];

pub fn settle_source_set(path: &RepoPath, named_by_plugin: SourceSet) -> SourceSet {
    if is_in_vendored_folder(path) {
        SourceSet::Vendored
    } else {
        named_by_plugin
    }
}

fn is_in_vendored_folder(path: &RepoPath) -> bool {
    let Some((folders, _file_name)) = path.as_str().rsplit_once('/') else {
        return false;
    };
    folders
        .split('/')
        .any(|folder| VENDORED_FOLDERS.contains(&folder))
}
