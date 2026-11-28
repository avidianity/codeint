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
const PACKAGE_CONTAINERS: [&str; 6] =
    ["packages", "pkgs", "apps", "libs", "services", "workspaces"];
const ROOT_TOOLING_FILENAMES: [&str; 4] = ["conf.py", "setup.py", "noxfile.py", "fabfile.py"];
const SETUP_PACKAGING_MODULES: [&str; 11] = [
    "setuptools",
    "distutils",
    "skbuild",
    "scikit_build_core",
    "flit",
    "hatchling",
    "cython",
    "poetry",
    "pdm",
    "maturin",
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
    if !ROOT_TOOLING_FILENAMES.contains(&lower.as_str()) {
        return false;
    }
    if !path.contains('/') || is_monorepo_package_root_file(path) {
        return true;
    }
    if has_folder_named(path, &TEST_FOLDERS) {
        return false;
    }
    matches_tooling_import(&lower, runtime_imports)
}

fn is_monorepo_package_root_file(path: &str) -> bool {
    let Some((dir, _)) = path.rsplit_once('/') else {
        return false;
    };
    let Some((container, package)) = dir.split_once('/') else {
        return false;
    };
    !package.is_empty()
        && !package.contains('/')
        && PACKAGE_CONTAINERS
            .iter()
            .any(|known| container.eq_ignore_ascii_case(known))
}

fn matches_tooling_import(filename: &str, runtime_imports: &[String]) -> bool {
    match filename {
        "conf.py" => imports_any_module(runtime_imports, &["sphinx"]),
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
