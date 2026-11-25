use codeint_model::language::Manifest;

use super::{
    PythonPackage, PythonProfile, direct_child_of_folder, direct_child_of_nested_folder,
    has_ignored_dir, is_test_dir_rel, split_parent_and_name,
};

const DJANGO_ENTRY_FILENAMES: [&str; 9] = [
    "wsgi.py",
    "asgi.py",
    "settings.py",
    "urls.py",
    "apps.py",
    "models.py",
    "admin.py",
    "checks.py",
    "signals.py",
];

pub(super) const PROFILE: PythonProfile = PythonProfile {
    name: "django",
    detect,
    entry_points,
    generated_paths,
    fallback_roots,
};

fn detect(pkg: &PythonPackage<'_>) -> bool {
    if !pkg.has_dep("django") {
        return false;
    }
    if pkg.has_any_rel(&["manage.py", "src/manage.py"]) {
        return true;
    }
    let has_settings = pkg.has_non_test_py(|rel| {
        split_parent_and_name(rel).1 == "settings.py"
            || direct_child_of_folder(rel, "settings").is_some()
    });
    let has_gateway =
        pkg.has_non_test_py(|rel| matches!(split_parent_and_name(rel).1, "wsgi.py" | "asgi.py"));
    has_settings && has_gateway
}

fn entry_points(pkg: &PythonPackage<'_>) -> Vec<String> {
    pkg.matching_py_files(|rel| {
        if matches!(rel, "manage.py" | "src/manage.py") {
            return true;
        }
        if is_test_dir_rel(rel) {
            return false;
        }
        let (_, file_name) = split_parent_and_name(rel);
        DJANGO_ENTRY_FILENAMES.contains(&file_name)
            || direct_child_of_folder(rel, "settings").is_some()
            || direct_child_of_folder(rel, "models").is_some()
            || direct_child_of_folder(rel, "migrations").is_some()
            || direct_child_of_nested_folder(rel, "management", "commands")
                .is_some_and(|name| !name.starts_with('_'))
            || direct_child_of_folder(rel, "templatetags")
                .is_some_and(|name| !name.starts_with('_'))
    })
}

fn generated_paths(pkg: &PythonPackage<'_>) -> Vec<String> {
    pkg.matching_py_files(|rel| is_numbered_migration(rel) || is_staticfiles(rel))
}

fn is_numbered_migration(rel: &str) -> bool {
    let Some(name) = direct_child_of_folder(rel, "migrations") else {
        return false;
    };
    let bytes = name.as_bytes();
    bytes.len() > 8
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'_'
        && name.ends_with(".py")
}

fn is_staticfiles(rel: &str) -> bool {
    rel.split('/').any(|segment| segment == "staticfiles")
}

fn fallback_roots(repository_paths: &[String], _manifests: &[Manifest]) -> Vec<String> {
    let mut roots = Vec::new();
    for path in repository_paths {
        if has_ignored_dir(path) {
            continue;
        }
        let (parent, name) = split_parent_and_name(path);
        if name == "manage.py" {
            if !parent.is_empty() {
                roots.push(parent.to_owned());
            }
            let apps_dir = if parent.is_empty() {
                "apps".to_owned()
            } else {
                format!("{parent}/apps")
            };
            roots.push(apps_dir);
        }
    }
    roots
}
