use crate::{RepoPath, SourceSet};

const BUILD_OUTPUT_FOLDERS: [&str; 4] = ["dist", "build", "out", "coverage"];
const LONGEST_AVERAGE_LINE_OF_WRITTEN_CODE: usize = 250;

pub fn resolve_source_set(path: &RepoPath, text: &str, from_plugin: SourceSet) -> SourceSet {
    if is_in_build_output_folder(path) || has_longer_average_line_than_written_code(text) {
        SourceSet::Generated
    } else {
        from_plugin
    }
}

fn is_in_build_output_folder(path: &RepoPath) -> bool {
    path.as_str()
        .rsplit_once('/')
        .is_some_and(|(folders, _file_name)| {
            folders
                .split('/')
                .any(|folder| BUILD_OUTPUT_FOLDERS.contains(&folder))
        })
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
