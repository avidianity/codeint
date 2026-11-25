use codeint_model::language::SourceSet;
use codeint_model::{has_folder_named, has_generated_header_marker_with_extra, path_file_name};

const TEST_FOLDERS: [&str; 5] = ["__tests__", "test", "tests", "test-dts", "type-tests"];
const TEST_FILE_MARKERS: [&str; 4] = ["test", "spec", "test-d", "spec-d"];
const EXTRA_GENERATED_MARKERS: [&str; 1] = ["auto generated"];

pub(crate) fn source_set<'text>(
    path: &str,
    mut header_comments: impl Iterator<Item = &'text str>,
) -> SourceSet {
    if header_comments
        .any(|comment| has_generated_header_marker_with_extra(comment, &EXTRA_GENERATED_MARKERS))
    {
        SourceSet::Generated
    } else if has_folder_named(path, &TEST_FOLDERS) || is_test_file_name(path_file_name(path)) {
        SourceSet::Test
    } else {
        SourceSet::Production
    }
}

fn is_test_file_name(file_name: &str) -> bool {
    let Some((without_extension, _extension)) = file_name.rsplit_once('.') else {
        return false;
    };
    let Some((stem, marker)) = without_extension.rsplit_once('.') else {
        return false;
    };
    !stem.is_empty() && TEST_FILE_MARKERS.contains(&marker)
}
