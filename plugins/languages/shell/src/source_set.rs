use codeint_model::language::SourceSet;
use codeint_model::{has_folder_named, has_generated_header_marker_with_extra, path_file_name};

const GENERATED_FILENAMES: &[&str] = &[
    "ar-lib",
    "compile",
    "config.guess",
    "config.sub",
    "configure",
    "configure.sh",
    "depcomp",
    "install-sh",
    "ltmain.sh",
    "mdate-sh",
    "missing",
    "mkinstalldirs",
    "py-compile",
    "test-driver",
    "ylwrap",
];

const EXTRA_GENERATED_MARKERS: &[&str] = &[
    "code generated",
    "do not modify by hand",
    "this file is generated",
];

const TEST_FOLDERS: &[&str] = &["test", "tests", "spec"];

const VENDORED_FOLDERS: &[&str] = &["vendor", "third_party"];

pub(crate) fn classify(path: &str, header_comments: &[String]) -> SourceSet {
    if has_folder_named(path, VENDORED_FOLDERS) {
        return SourceSet::Vendored;
    }
    let file_name = path_file_name(path);
    if is_generated_file(file_name, header_comments) {
        return SourceSet::Generated;
    }
    if is_test_path(path, file_name) {
        return SourceSet::Test;
    }
    SourceSet::Production
}

pub(crate) fn is_generated_filename(file_name: &str) -> bool {
    GENERATED_FILENAMES.contains(&file_name)
}

fn is_generated_file(file_name: &str, header_comments: &[String]) -> bool {
    is_generated_filename(file_name)
        || header_comments
            .iter()
            .any(|comment| has_generated_header_marker_with_extra(comment, EXTRA_GENERATED_MARKERS))
}

fn is_test_path(path: &str, file_name: &str) -> bool {
    has_folder_named(path, TEST_FOLDERS) || is_test_file_name(file_name)
}

fn is_test_file_name(file_name: &str) -> bool {
    let Some((stem, ext)) = file_name.rsplit_once('.') else {
        return false;
    };
    if !matches!(ext, "sh" | "bash" | "zsh") {
        return false;
    }
    stem.starts_with("test_") || stem.ends_with("_test") || stem.ends_with(".test")
}
