use codeint_model::language::SourceSet;
use codeint_model::{has_any_ascii_marker, has_folder_named, path_file_name};

#[cfg(test)]
mod tests;

const GENERATED_MARKERS: [&str; 4] = [
    "@generated",
    "generated code",
    "do not edit",
    "auto-generated",
];
const TEST_FOLDERS: [&str; 2] = ["test", "tests"];
const ROOT_TOOLING_FILENAMES: [&str; 4] = ["conf.py", "setup.py", "noxfile.py", "fabfile.py"];
const SETUP_PACKAGING_MODULES: [&str; 8] = [
    "setuptools",
    "distutils",
    "skbuild",
    "scikit_build_core",
    "flit",
    "hatchling",
    "cython",
    "numpy.distutils",
];

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

pub(crate) fn is_tooling_file(path: &str, runtime_imports: &[String]) -> bool {
    let lower = path_file_name(path).to_ascii_lowercase();
    if lower == "conftest.py" {
        return true;
    }
    if !path.contains('/') {
        return ROOT_TOOLING_FILENAMES.contains(&lower.as_str());
    }
    match lower.as_str() {
        "setup.py" => imports_any_module(runtime_imports, &SETUP_PACKAGING_MODULES),
        "noxfile.py" => imports_any_module(runtime_imports, &["nox"]),
        "fabfile.py" => imports_any_module(runtime_imports, &["fabric"]),
        _ => false,
    }
}

fn imports_any_module(runtime_imports: &[String], prefixes: &[&str]) -> bool {
    runtime_imports.iter().any(|specifier| {
        let trimmed = specifier.trim();
        prefixes.iter().any(|prefix| {
            trimmed == *prefix
                || trimmed
                    .strip_prefix(prefix)
                    .is_some_and(|rest| rest.starts_with('.'))
        })
    })
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
