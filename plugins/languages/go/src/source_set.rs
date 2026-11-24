use codeint_model::language::SourceSet;

pub(crate) fn classify(path: &str, header_line_comments: &[String]) -> SourceSet {
    if is_vendored_path(path) {
        return SourceSet::Vendored;
    }
    if is_generated(header_line_comments) {
        return SourceSet::Generated;
    }
    if is_test_path(path) {
        return SourceSet::Test;
    }
    SourceSet::Production
}

pub(crate) fn is_generated(header_line_comments: &[String]) -> bool {
    header_line_comments.iter().any(|line| {
        line.strip_prefix("// Code generated ")
            .and_then(|rest| rest.strip_suffix(" DO NOT EDIT."))
            .is_some()
    })
}

pub(crate) fn is_test_path(path: &str) -> bool {
    path.ends_with("_test.go") || has_dir_segment(path, "testdata")
}

pub(crate) fn is_vendored_path(path: &str) -> bool {
    has_dir_segment(path, "vendor")
}

pub(crate) fn has_dir_segment(path: &str, name: &str) -> bool {
    let Some((parent, _)) = path.rsplit_once('/') else {
        return false;
    };
    parent.split('/').any(|segment| segment == name)
}
