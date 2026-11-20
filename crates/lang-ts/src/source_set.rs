use codeint_model::language::SourceSet;

const TEST_FOLDER: &str = "__tests__";
const TEST_FILE_MARKERS: [&str; 2] = ["test", "spec"];

pub(crate) fn source_set(path: &str) -> SourceSet {
    let (folders, file_name) = path.rsplit_once('/').unwrap_or(("", path));
    let in_test_folder = folders.split('/').any(|folder| folder == TEST_FOLDER);
    if in_test_folder || is_test_file_name(file_name) {
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
