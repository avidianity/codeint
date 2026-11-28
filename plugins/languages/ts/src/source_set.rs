use codeint_model::language::SourceSet;
use codeint_model::{has_folder_named, has_generated_header_marker_with_extra, path_file_name};

#[cfg(test)]
mod tests;

const TEST_FOLDERS: [&str; 5] = ["__tests__", "test", "tests", "test-dts", "type-tests"];
const TEST_FILE_MARKERS: [&str; 4] = ["test", "spec", "test-d", "spec-d"];
const EXTRA_GENERATED_MARKERS: [&str; 1] = ["auto generated"];

const WORKSPACE_CONTAINERS: [&str; 5] = ["packages", "pkgs", "apps", "libs", "workspaces"];
const SOURCE_FOLDERS: [&str; 25] = [
    "src",
    "lib",
    "app",
    "source",
    "config",
    "pages",
    "routes",
    "components",
    "features",
    "modules",
    "server",
    "client",
    "shared",
    "internal",
    "core",
    "api",
    "utils",
    "helpers",
    "services",
    "models",
    "controllers",
    "views",
    "hooks",
    "store",
    "domain",
];

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

pub(crate) fn is_tooling_file(path: &str) -> bool {
    if has_storybook_folder(path) {
        return true;
    }
    let lower = path_file_name(path).to_ascii_lowercase();
    if lower.is_empty() {
        return false;
    }
    if !(is_dotted_config_filename(&lower) || is_leading_dot_config_filename(&lower)) {
        return false;
    }
    !is_inside_source_folder(path)
}

fn has_storybook_folder(path: &str) -> bool {
    let Some((dir, _)) = path.rsplit_once('/') else {
        return false;
    };
    dir.split('/')
        .any(|segment| segment.eq_ignore_ascii_case(".storybook"))
}

fn is_dotted_config_filename(lower: &str) -> bool {
    lower
        .find(".config.")
        .is_some_and(|index| index > 0 && index + ".config.".len() < lower.len())
}

fn is_leading_dot_config_filename(lower: &str) -> bool {
    let Some(rest) = lower.strip_prefix('.') else {
        return false;
    };
    if is_dotted_config_filename(rest) {
        return true;
    }
    rest.split('.')
        .next()
        .is_some_and(|stem| stem.ends_with("rc") && stem != "rc" && stem != "src")
}

fn is_inside_source_folder(path: &str) -> bool {
    let Some((dir, _)) = path.rsplit_once('/') else {
        return false;
    };
    let mut segments = dir.split('/');
    let Some(first) = segments.next() else {
        return false;
    };
    if WORKSPACE_CONTAINERS
        .iter()
        .any(|container| first.eq_ignore_ascii_case(container))
    {
        let _package_name = segments.next();
        return segments.any(is_source_folder);
    }
    is_source_folder(first) || segments.any(is_source_folder)
}

fn is_source_folder(segment: &str) -> bool {
    SOURCE_FOLDERS
        .iter()
        .any(|folder| segment.eq_ignore_ascii_case(folder))
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
