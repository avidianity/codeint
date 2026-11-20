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
    let _ = (path, BUILD_OUTPUT_FOLDERS);
    todo!()
}

fn has_longer_average_line_than_written_code(text: &str) -> bool {
    let _ = (text, LONGEST_AVERAGE_LINE_OF_WRITTEN_CODE);
    todo!()
}
