use crate::{RepoPath, SourceSet};

const VENDORED_FOLDERS: [&str; 2] = ["vendor", "third_party"];
const BUILD_OUTPUT_FOLDERS: [&str; 4] = ["dist", "build", "out", "coverage"];
const LONGEST_AVERAGE_LINE_OF_WRITTEN_CODE: usize = 250;

pub fn settle_source_set(path: &RepoPath, text: &str, named_by_plugin: SourceSet) -> SourceSet {
    if is_in_folder_named(path, &VENDORED_FOLDERS) {
        SourceSet::Vendored
    } else if is_in_folder_named(path, &BUILD_OUTPUT_FOLDERS)
        || has_longer_average_line_than_written_code(text)
    {
        SourceSet::Generated
    } else {
        named_by_plugin
    }
}

fn is_in_folder_named(path: &RepoPath, names: &[&str]) -> bool {
    let Some((folders, _file_name)) = path.as_str().rsplit_once('/') else {
        return false;
    };
    folders.split('/').any(|folder| names.contains(&folder))
}

fn has_longer_average_line_than_written_code(text: &str) -> bool {
    let (lines, characters) = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .fold((0_usize, 0_usize), |(lines, characters), line| {
            (lines + 1, characters + line.chars().count())
        });
    characters > lines.saturating_mul(LONGEST_AVERAGE_LINE_OF_WRITTEN_CODE)
}
