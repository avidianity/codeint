use codeint_model::language::SourceSet;
use codeint_model::{has_folder_named, has_generated_header_marker, path_file_name};

#[cfg(test)]
mod tests;

const TEST_FOLDERS: [&str; 3] = ["tests", "benches", "examples"];

pub(crate) fn classify(path: &str, text: &str) -> SourceSet {
    if has_generated_header_marker(&leading_comment_block(text)) {
        return SourceSet::Generated;
    }
    if is_test_path(path) {
        return SourceSet::Test;
    }
    SourceSet::Production
}

fn is_test_path(path: &str) -> bool {
    if has_folder_named(path, &TEST_FOLDERS) {
        return true;
    }
    let file_name = path_file_name(path);
    file_name == "tests.rs" || file_name.ends_with("_test.rs") || file_name.ends_with("_tests.rs")
}

fn leading_comment_block(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut index = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        3
    } else {
        0
    };
    let mut comments = String::new();
    while index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if bytes[index..].starts_with(b"//") {
            let start = index;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            comments.push_str(&text[start..index]);
            comments.push('\n');
        } else if bytes[index..].starts_with(b"/*") {
            let start = index;
            index += 2;
            let mut depth = 1usize;
            while index < bytes.len() && depth > 0 {
                if bytes[index..].starts_with(b"/*") {
                    depth += 1;
                    index += 2;
                } else if bytes[index..].starts_with(b"*/") {
                    depth -= 1;
                    index += 2;
                } else {
                    index += 1;
                }
            }
            comments.push_str(&text[start..index]);
            comments.push('\n');
        } else {
            break;
        }
    }
    comments
}
