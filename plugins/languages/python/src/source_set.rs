use codeint_model::language::SourceSet;

const GENERATED_MARKERS: &[&str] = &[
    "@generated",
    "generated code",
    "do not edit",
    "auto-generated",
];

pub(crate) fn classify(path: &str, header_comments: &[String]) -> SourceSet {
    let filename = path.rsplit('/').next().unwrap_or(path);
    if is_generated_filename(filename) || has_generated_header(header_comments) {
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

fn has_generated_header(header_comments: &[String]) -> bool {
    header_comments.iter().any(|line| {
        let lower = line.to_ascii_lowercase();
        GENERATED_MARKERS
            .iter()
            .any(|marker| lower.contains(marker))
    })
}

fn is_test_path(path: &str, filename: &str) -> bool {
    if path
        .split('/')
        .any(|segment| matches!(segment, "test" | "tests"))
    {
        return true;
    }
    filename == "conftest.py"
        || (filename.starts_with("test_") && filename.ends_with(".py"))
        || filename.ends_with("_test.py")
}
