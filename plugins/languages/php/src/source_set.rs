use codeint_model::language::SourceSet;
use codeint_model::{has_folder_named, has_generated_header_marker, path_file_name};

#[cfg(test)]
mod tests;

const TEST_FOLDERS: [&str; 2] = ["tests", "test"];

pub(crate) fn classify(path: &str, header_comments: &str) -> SourceSet {
    if has_generated_header_marker(header_comments) {
        return SourceSet::Generated;
    }
    if is_test_path(path) {
        return SourceSet::Test;
    }
    SourceSet::Production
}

pub(crate) fn is_tooling_file(path: &str) -> bool {
    path_file_name(path)
        .to_ascii_lowercase()
        .starts_with(".php-cs-fixer")
}

fn is_test_path(path: &str) -> bool {
    let file_name = path_file_name(path);
    let has_test_suffix = ["Test.php", ".test.php"]
        .into_iter()
        .any(|suffix| file_name.ends_with(suffix) && file_name.len() > suffix.len());
    has_folder_named(path, &TEST_FOLDERS) || has_test_suffix
}
