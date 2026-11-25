use codeint_model::language::SourceSet;
use codeint_model::{has_any_ascii_marker, has_folder_named, path_file_name};

const GENERATED_MARKERS: [&str; 4] = [
    "@generated",
    "generated code",
    "do not edit",
    "auto-generated",
];
const TEST_FOLDERS: [&str; 2] = ["test", "tests"];

pub(crate) fn classify(path: &str, header_comments: &[String]) -> SourceSet {
    let filename = path_file_name(path);
    if is_generated_filename(filename)
        || header_comments
            .iter()
            .any(|line| has_any_ascii_marker(line, &GENERATED_MARKERS))
    {
        return SourceSet::Generated;
    }
    if is_test_path(path, filename) {
        return SourceSet::Test;
    }
    SourceSet::Production
}

fn is_generated_filename(filename: &str) -> bool {
    filename.ends_with("_pb2.py") || filename.ends_with("_pb2_grpc.py")
}

fn is_test_path(path: &str, filename: &str) -> bool {
    has_folder_named(path, &TEST_FOLDERS)
        || filename == "conftest.py"
        || (filename.starts_with("test_") && filename.ends_with(".py"))
        || filename.ends_with("_test.py")
}
