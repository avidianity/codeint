use codeint_model::language::Manifest;

use super::{
    PythonPackage, PythonProfile, direct_child_of_folder, fallback_roots_for_filenames,
    is_test_dir_rel, split_parent_and_name,
};

const FLASK_DEFAULT_ENTRIES: [&str; 8] = [
    "app.py",
    "wsgi.py",
    "run.py",
    "app/__init__.py",
    "wsgi/__init__.py",
    "src/app.py",
    "src/wsgi.py",
    "src/run.py",
];

const NON_FACTORY_PACKAGES: [&str; 8] = [
    "test",
    "tests",
    "doc",
    "docs",
    "scripts",
    "instance",
    "migrations",
    "static",
];

pub(super) const PROFILE: PythonProfile = PythonProfile {
    name: "flask",
    detect,
    entry_points,
    generated_paths,
    fallback_roots,
};

fn detect(pkg: &PythonPackage<'_>) -> bool {
    if !pkg.has_dep("flask") {
        return false;
    }
    pkg.has_any_rel(&FLASK_DEFAULT_ENTRIES)
        || pkg.has_non_test_py(|rel| {
            split_parent_and_name(rel).1 == "views.py"
                || direct_child_of_folder(rel, "blueprints").is_some()
        })
}

fn entry_points(pkg: &PythonPackage<'_>) -> Vec<String> {
    let include_factory_fallback = !pkg.has_any_rel(&FLASK_DEFAULT_ENTRIES);
    pkg.matching_py_files(|rel| {
        if FLASK_DEFAULT_ENTRIES.contains(&rel) {
            return true;
        }
        if is_test_dir_rel(rel) {
            return false;
        }
        let (_, name) = split_parent_and_name(rel);
        name == "views.py"
            || direct_child_of_folder(rel, "blueprints").is_some()
            || (include_factory_fallback && is_factory_package_init(rel))
    })
}

fn is_factory_package_init(rel: &str) -> bool {
    let trimmed = rel.strip_prefix("src/").unwrap_or(rel);
    let Some((pkg_dir, file_name)) = trimmed.split_once('/') else {
        return false;
    };
    file_name == "__init__.py"
        && !pkg_dir.is_empty()
        && !pkg_dir.starts_with('.')
        && !NON_FACTORY_PACKAGES.contains(&pkg_dir)
}

fn generated_paths(pkg: &PythonPackage<'_>) -> Vec<String> {
    pkg.matching_py_files(|rel| rel.starts_with("instance/"))
}

fn fallback_roots(repository_paths: &[String], _manifests: &[Manifest]) -> Vec<String> {
    fallback_roots_for_filenames(repository_paths, &["app.py", "wsgi.py", "run.py"])
}
