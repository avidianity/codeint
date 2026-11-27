use std::collections::{BTreeSet, HashSet};

use codeint_model::language::Manifest;
use codeint_model::scan::{normalize_relative_path, paths_with_suffix};

const NON_PACKAGE_DIRS: [&str; 14] = [
    "benchmark",
    "benchmarks",
    "build",
    "dist",
    "doc",
    "docs",
    "example",
    "examples",
    "node_modules",
    "scripts",
    "test",
    "tests",
    "third_party",
    "vendor",
];

pub(crate) fn discover<P: AsRef<str>>(
    repository_paths: &[P],
    manifests: &[Manifest],
) -> Vec<String> {
    let py_files = paths_with_suffix(repository_paths, ".py");
    let mut entries = BTreeSet::new();
    for (base_dir, doc) in parsed_pyprojects(manifests) {
        let roots = package_search_roots(base_dir);
        collect_script_entries(&doc, &roots, &py_files, &mut entries);
        collect_declared_packages(&doc, base_dir, &py_files, &mut entries);
        collect_package_root_entries(&doc, &roots, &py_files, &mut entries);
    }
    entries.into_iter().collect()
}

fn collect_package_root_entries(
    doc: &toml::Value,
    roots: &[String],
    py_files: &HashSet<&str>,
    found: &mut BTreeSet<String>,
) {
    let defines_package = doc.get("project").is_some()
        || doc
            .get("tool")
            .and_then(|tool| tool.get("poetry"))
            .is_some();
    if !defines_package {
        return;
    }
    for root in roots {
        let prefix = if root.is_empty() {
            String::new()
        } else {
            format!("{root}/")
        };
        for &path in py_files {
            let Some(rest) = path.strip_prefix(&prefix) else {
                continue;
            };
            let Some((pkg, file_name)) = rest.split_once('/') else {
                continue;
            };
            if !matches!(file_name, "__init__.py" | "__main__.py") {
                continue;
            }
            if pkg.is_empty() || pkg.starts_with('.') || NON_PACKAGE_DIRS.contains(&pkg) {
                continue;
            }
            found.insert(path.to_owned());
        }
    }
}

fn parsed_pyprojects(manifests: &[Manifest]) -> Vec<(&str, toml::Value)> {
    manifests
        .iter()
        .filter_map(|manifest| {
            let dir = if manifest.path == "pyproject.toml" {
                ""
            } else {
                manifest.path.strip_suffix("/pyproject.toml")?
            };
            let doc = toml::from_str::<toml::Value>(&manifest.text).ok()?;
            Some((dir, doc))
        })
        .collect()
}

fn package_search_roots(base_dir: &str) -> [String; 2] {
    let src_root = if base_dir.is_empty() {
        "src".to_owned()
    } else {
        format!("{base_dir}/src")
    };
    [base_dir.to_owned(), src_root]
}

fn collect_script_entries(
    doc: &toml::Value,
    roots: &[String],
    py_files: &HashSet<&str>,
    found: &mut BTreeSet<String>,
) {
    if let Some(project) = doc.get("project") {
        for section in ["scripts", "gui-scripts"] {
            if let Some(table) = project.get(section).and_then(toml::Value::as_table) {
                for spec in table.values().filter_map(toml::Value::as_str) {
                    resolve_module_spec(spec, roots, py_files, found);
                }
            }
        }
        if let Some(groups) = project.get("entry-points").and_then(toml::Value::as_table) {
            for group in groups.values().filter_map(toml::Value::as_table) {
                for spec in group.values().filter_map(toml::Value::as_str) {
                    resolve_module_spec(spec, roots, py_files, found);
                }
            }
        }
    }
    collect_poetry_scripts(doc, roots, py_files, found);
}

fn collect_poetry_scripts(
    doc: &toml::Value,
    roots: &[String],
    py_files: &HashSet<&str>,
    found: &mut BTreeSet<String>,
) {
    let Some(scripts) = doc
        .get("tool")
        .and_then(|v| v.get("poetry"))
        .and_then(|v| v.get("scripts"))
        .and_then(toml::Value::as_table)
    else {
        return;
    };
    for value in scripts.values() {
        let spec = value.as_str().or_else(|| {
            value
                .get("callable")
                .or_else(|| value.get("reference"))
                .and_then(toml::Value::as_str)
        });
        if let Some(target) = spec {
            resolve_module_spec(target, roots, py_files, found);
        }
    }
}

fn resolve_module_spec(
    spec: &str,
    roots: &[String],
    py_files: &HashSet<&str>,
    found: &mut BTreeSet<String>,
) {
    let Some(rel_path) = module_spec_to_rel_path(spec) else {
        return;
    };
    for root in roots {
        let Some(base) = normalize_relative_path(root, &rel_path) else {
            continue;
        };
        if base.is_empty() {
            continue;
        }
        for candidate in [
            format!("{base}.py"),
            format!("{base}/__init__.py"),
            format!("{base}/__main__.py"),
        ] {
            if py_files.contains(candidate.as_str()) {
                found.insert(candidate);
                return;
            }
        }
    }
}

pub(crate) fn module_spec_to_rel_path(spec: &str) -> Option<String> {
    let module_part = spec
        .trim()
        .split(':')
        .next()?
        .split('[')
        .next()?
        .split_whitespace()
        .next()?
        .trim();
    if module_part.is_empty() || module_part.contains('/') || module_part.contains('\\') {
        return None;
    }
    let segments: Vec<&str> = module_part.split('.').collect();
    if segments.iter().any(|seg| seg.is_empty() || *seg == "..") {
        return None;
    }
    Some(segments.join("/"))
}

fn collect_declared_packages(
    doc: &toml::Value,
    base_dir: &str,
    py_files: &HashSet<&str>,
    found: &mut BTreeSet<String>,
) {
    let Some(tool) = doc.get("tool") else {
        return;
    };
    let setuptools = tool
        .get("setuptools")
        .and_then(|v| v.get("packages"))
        .and_then(toml::Value::as_array);
    let hatch = tool
        .get("hatch")
        .and_then(|v| v.get("build"))
        .and_then(|v| v.get("targets"))
        .and_then(|v| v.get("wheel"))
        .and_then(|v| v.get("packages"))
        .and_then(toml::Value::as_array);
    for list in [setuptools, hatch].into_iter().flatten() {
        for pkg in list.iter().filter_map(toml::Value::as_str) {
            resolve_package_dir(base_dir, None, pkg, py_files, found);
        }
    }
    collect_poetry_packages(tool, base_dir, py_files, found);
}

fn collect_poetry_packages(
    tool: &toml::Value,
    base_dir: &str,
    py_files: &HashSet<&str>,
    found: &mut BTreeSet<String>,
) {
    let Some(packages) = tool
        .get("poetry")
        .and_then(|v| v.get("packages"))
        .and_then(toml::Value::as_array)
    else {
        return;
    };
    for entry in packages {
        if let Some(pkg) = entry.as_str() {
            resolve_package_dir(base_dir, None, pkg, py_files, found);
        } else if let Some(include) = entry.get("include").and_then(toml::Value::as_str) {
            let from = entry.get("from").and_then(toml::Value::as_str);
            resolve_package_dir(base_dir, from, include, py_files, found);
        }
    }
}

fn resolve_package_dir(
    base_dir: &str,
    from: Option<&str>,
    pkg: &str,
    py_files: &HashSet<&str>,
    found: &mut BTreeSet<String>,
) {
    let trimmed = pkg.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('/')
        || trimmed.contains('\\')
        || trimmed.split('/').any(|segment| segment == "..")
    {
        return;
    }
    let rel_pkg = if trimmed.contains('/') {
        trimmed.to_owned()
    } else {
        trimmed.replace('.', "/")
    };
    for root in candidate_package_roots(base_dir, from, trimmed.contains('/')) {
        if let Some(pkg_dir) = normalize_relative_path(&root, &rel_pkg)
            && !pkg_dir.is_empty()
        {
            record_package_init_files(&pkg_dir, py_files, found);
        }
    }
}

fn candidate_package_roots(base_dir: &str, from: Option<&str>, has_slash: bool) -> Vec<String> {
    if let Some(from_dir) = from {
        let trimmed = from_dir.trim();
        if trimmed.starts_with('/')
            || trimmed.contains('\\')
            || trimmed.split('/').any(|segment| segment == "..")
        {
            return Vec::new();
        }
        return normalize_relative_path(base_dir, trimmed)
            .into_iter()
            .collect();
    }
    if has_slash {
        return vec![base_dir.to_owned()];
    }
    package_search_roots(base_dir).into()
}

fn record_package_init_files(
    pkg_dir: &str,
    py_files: &HashSet<&str>,
    found: &mut BTreeSet<String>,
) {
    for entry_file in ["__init__.py", "__main__.py"] {
        let candidate = format!("{pkg_dir}/{entry_file}");
        if py_files.contains(candidate.as_str()) {
            found.insert(candidate);
        }
    }
}
