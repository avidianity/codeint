use codeint_model::language::Manifest;

use super::{
    PythonPackage, PythonProfile, direct_child_of_folder, empty_generated,
    fallback_roots_for_filenames, is_test_dir_rel,
};

const FASTAPI_DEFAULT_ENTRIES: [&str; 9] = [
    "main.py",
    "app.py",
    "api.py",
    "app/main.py",
    "app/app.py",
    "app/api.py",
    "src/main.py",
    "src/app.py",
    "src/api.py",
];

pub(super) const PROFILE: PythonProfile = PythonProfile {
    name: "fastapi",
    detect,
    entry_points,
    generated_paths: empty_generated,
    fallback_roots,
};

fn configured_entrypoint<'a>(pkg: &'a PythonPackage<'_>) -> Option<&'a str> {
    pkg.pyproject()?
        .get("tool")?
        .get("fastapi")?
        .get("entrypoint")?
        .as_str()
}

fn detect(pkg: &PythonPackage<'_>) -> bool {
    let has_entrypoint = configured_entrypoint(pkg).is_some();
    if !pkg.has_dep("fastapi") && !has_entrypoint {
        return false;
    }
    has_entrypoint
        || pkg.has_any_rel(&FASTAPI_DEFAULT_ENTRIES)
        || pkg.has_non_test_py(|rel| direct_child_of_folder(rel, "routers").is_some())
}

fn entry_points(pkg: &PythonPackage<'_>) -> Vec<String> {
    let mut found = pkg.matching_py_files(|rel| {
        FASTAPI_DEFAULT_ENTRIES.contains(&rel)
            || (!is_test_dir_rel(rel) && direct_child_of_folder(rel, "routers").is_some())
    });
    if let Some(target) = configured_entrypoint(pkg).and_then(|spec| pkg.resolve_module_file(spec))
    {
        found.push(target);
    }
    found
}

fn fallback_roots(repository_paths: &[String], _manifests: &[Manifest]) -> Vec<String> {
    fallback_roots_for_filenames(repository_paths, &["main.py", "app.py", "api.py"])
}
