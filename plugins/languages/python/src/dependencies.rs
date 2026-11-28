use std::collections::{BTreeSet, HashSet};

use codeint_model::language::{DeclaredDep, DepKind, Manifest};
use toml::Value;

#[cfg(test)]
mod tests;

pub(crate) fn normalize_pep503(raw: &str) -> String {
    let mut normalized = String::with_capacity(raw.len());
    let mut prev_sep = false;
    for ch in raw.chars() {
        if matches!(ch, '-' | '_' | '.') {
            if !prev_sep {
                normalized.push('-');
                prev_sep = true;
            }
        } else {
            normalized.push(ch.to_ascii_lowercase());
            prev_sep = false;
        }
    }
    normalized
}

pub(crate) fn extract_pep508_name(requirement: &str) -> Option<String> {
    let trimmed = requirement.trim();
    let end = trimmed
        .find(|ch: char| !(ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.')))
        .unwrap_or(trimmed.len());
    let raw = &trimmed[..end];
    if raw.is_empty()
        || !raw
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphanumeric())
    {
        return None;
    }
    Some(normalize_pep503(raw))
}

pub(crate) fn is_requirements_file(file_name: &str) -> bool {
    file_name.starts_with("requirements") && file_name.ends_with(".txt")
}

pub(crate) fn is_dev_requirements_file(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower.contains("dev") || lower.contains("test")
}

pub(crate) fn extract_pyproject_runtime_deps(doc: &Value) -> Vec<String> {
    let mut deps = Vec::new();
    collect_pep621_runtime_deps(doc, &mut deps);
    collect_poetry_runtime_deps(doc, &mut deps);
    deps
}

fn collect_pep621_runtime_deps(doc: &Value, out: &mut Vec<String>) {
    let Some(items) = doc
        .get("project")
        .and_then(|p| p.get("dependencies"))
        .and_then(Value::as_array)
    else {
        return;
    };
    out.extend(
        items
            .iter()
            .filter_map(Value::as_str)
            .filter_map(extract_pep508_name),
    );
}

fn collect_poetry_runtime_deps(doc: &Value, out: &mut Vec<String>) {
    let Some(poetry_deps) = doc
        .get("tool")
        .and_then(|t| t.get("poetry"))
        .and_then(|p| p.get("dependencies"))
        .and_then(Value::as_table)
    else {
        return;
    };
    for (raw_name, spec) in poetry_deps {
        if raw_name.eq_ignore_ascii_case("python") || is_optional_table(spec) {
            continue;
        }
        let normalized = normalize_pep503(raw_name.trim());
        if !normalized.is_empty() {
            out.push(normalized);
        }
    }
}

pub(crate) fn extract_requirements_deps(text: &str) -> Vec<String> {
    text.lines()
        .map(|line| line.split('#').next().unwrap_or("").trim())
        .filter(|line| !line.is_empty() && !line.starts_with('-'))
        .filter_map(extract_pep508_name)
        .collect()
}

pub(crate) fn extract_setup_cfg_runtime_deps(text: &str) -> Vec<String> {
    let mut parser = SetupCfgParser::default();
    for line in text.lines() {
        parser.feed(line);
    }
    parser.deps
}

#[derive(Default)]
struct SetupCfgParser {
    in_options: bool,
    in_install_requires: bool,
    deps: Vec<String>,
}

impl SetupCfgParser {
    fn feed(&mut self, raw_line: &str) {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            return;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            self.in_options = trimmed.eq_ignore_ascii_case("[options]");
            self.in_install_requires = false;
            return;
        }
        if self.in_options {
            self.feed_options_line(raw_line, trimmed);
        }
    }

    fn feed_options_line(&mut self, raw_line: &str, trimmed: &str) {
        if raw_line.starts_with(' ') || raw_line.starts_with('\t') {
            if self.in_install_requires
                && let Some(name) = extract_pep508_name(trimmed)
            {
                self.deps.push(name);
            }
            return;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            self.in_install_requires = false;
            return;
        };
        self.in_install_requires = key.trim().eq_ignore_ascii_case("install_requires");
        if self.in_install_requires
            && let Some(name) = extract_pep508_name(value.trim())
        {
            self.deps.push(name);
        }
    }
}

pub(crate) fn extract_pipfile_runtime_deps(text: &str) -> Vec<String> {
    extract_pipfile_section(text, "packages")
}

fn extract_pipfile_section(text: &str, section: &str) -> Vec<String> {
    let Ok(doc) = toml::from_str::<Value>(text) else {
        return Vec::new();
    };
    let Some(table) = doc.get(section).and_then(Value::as_table) else {
        return Vec::new();
    };
    table
        .keys()
        .map(|raw| normalize_pep503(raw.trim()))
        .filter(|name| !name.is_empty())
        .collect()
}

pub(crate) fn is_python_dependency_manifest(path: &str) -> bool {
    if has_ignored_manifest_dir(path) {
        return false;
    }
    let (_, file_name) = split_manifest_path(path);
    file_name == "pyproject.toml"
        || file_name == "setup.cfg"
        || file_name == "Pipfile"
        || is_requirements_file(file_name)
}

#[cfg(test)]
pub(crate) fn extract_declared(manifests: &[Manifest]) -> Vec<DeclaredDep> {
    let owned: Vec<(String, String, Value)> = manifests
        .iter()
        .filter(|m| is_pyproject_manifest(&m.path))
        .filter_map(|m| {
            toml::from_str::<Value>(&m.text)
                .ok()
                .map(|d| (package_root(&m.path), m.path.clone(), d))
        })
        .collect();
    let parsed_pyprojects: Vec<(String, String, &Value)> = owned
        .iter()
        .map(|(root, path, doc)| (root.clone(), path.clone(), doc))
        .collect();
    let auxiliary: Vec<&Manifest> = manifests
        .iter()
        .filter(|m| !is_pyproject_manifest(&m.path))
        .collect();
    extract_declared_from_parsed(&parsed_pyprojects, &auxiliary)
}

pub(crate) fn extract_declared_from_parsed(
    parsed_pyprojects: &[(String, String, &Value)],
    auxiliary: &[&Manifest],
) -> Vec<DeclaredDep> {
    let local_packages: HashSet<String> = parsed_pyprojects
        .iter()
        .filter_map(|(_, _, doc)| declared_package_name(doc))
        .collect();
    let mut entries = BTreeSet::new();
    for (root, _, doc) in parsed_pyprojects {
        DepCollector {
            root,
            local_packages: &local_packages,
            out: &mut entries,
        }
        .visit_document(doc);
    }
    for manifest in auxiliary {
        collect_auxiliary_manifest(manifest, &local_packages, &mut entries);
    }
    entries
        .into_iter()
        .map(|(package_root, rank, name)| DeclaredDep {
            package_root,
            name,
            kind: rank_to_kind(rank),
        })
        .collect()
}

fn collect_auxiliary_manifest(
    manifest: &Manifest,
    local_packages: &HashSet<String>,
    out: &mut BTreeSet<(String, u8, String)>,
) {
    if has_ignored_manifest_dir(&manifest.path) {
        return;
    }
    let (root, file_name) = split_manifest_path(&manifest.path);
    let mut collector = DepCollector {
        root,
        local_packages,
        out,
    };
    if is_requirements_file(file_name) {
        let kind = if is_dev_requirements_file(file_name) {
            DepKind::Dev
        } else {
            DepKind::Runtime
        };
        collector.record_all(extract_requirements_deps(&manifest.text), kind);
    } else if file_name == "setup.cfg" {
        collector.record_all(
            extract_setup_cfg_runtime_deps(&manifest.text),
            DepKind::Runtime,
        );
    } else if file_name == "Pipfile" {
        collector.record_all(
            extract_pipfile_section(&manifest.text, "packages"),
            DepKind::Runtime,
        );
        collector.record_all(
            extract_pipfile_section(&manifest.text, "dev-packages"),
            DepKind::Dev,
        );
    }
}

fn has_ignored_manifest_dir(path: &str) -> bool {
    path.split('/')
        .any(|seg| matches!(seg, "vendor" | "third_party" | ".venv" | "venv"))
}

fn split_manifest_path(path: &str) -> (&str, &str) {
    path.rsplit_once('/').unwrap_or(("", path))
}

struct DepCollector<'a> {
    root: &'a str,
    local_packages: &'a HashSet<String>,
    out: &'a mut BTreeSet<(String, u8, String)>,
}

impl DepCollector<'_> {
    fn visit_document(&mut self, doc: &Value) {
        if let Some(project) = doc.get("project") {
            self.add_array(project.get("dependencies"), DepKind::Runtime);
            self.add_groups(project.get("optional-dependencies"), DepKind::Optional);
        }
        self.add_groups(doc.get("dependency-groups"), DepKind::Dev);
        if let Some(tool) = doc.get("tool") {
            self.visit_tools(tool);
        }
    }

    fn visit_tools(&mut self, tool: &Value) {
        if let Some(uv) = tool.get("uv") {
            self.add_array(uv.get("dev-dependencies"), DepKind::Dev);
        }
        if let Some(pdm) = tool.get("pdm") {
            self.add_groups(pdm.get("dev-dependencies"), DepKind::Dev);
        }
        if let Some(poetry) = tool.get("poetry") {
            self.visit_poetry_deps(poetry);
            self.visit_poetry_groups(poetry);
        }
    }

    fn visit_poetry_deps(&mut self, poetry: &Value) {
        if let Some(deps) = poetry.get("dependencies").and_then(Value::as_table) {
            for (raw_name, spec) in deps {
                if raw_name.eq_ignore_ascii_case("python") {
                    continue;
                }
                let kind = if is_optional_table(spec) {
                    DepKind::Optional
                } else {
                    DepKind::Runtime
                };
                self.add_name(raw_name, kind);
            }
        }
        if let Some(dev_deps) = poetry.get("dev-dependencies").and_then(Value::as_table) {
            for raw_name in dev_deps.keys() {
                self.add_name(raw_name, DepKind::Dev);
            }
        }
    }

    fn visit_poetry_groups(&mut self, poetry: &Value) {
        let Some(groups) = poetry.get("group").and_then(Value::as_table) else {
            return;
        };
        for group in groups.values() {
            let kind = if is_optional_table(group) {
                DepKind::Optional
            } else {
                DepKind::Dev
            };
            if let Some(deps) = group.get("dependencies").and_then(Value::as_table) {
                for raw_name in deps.keys() {
                    self.add_name(raw_name, kind);
                }
            }
        }
    }

    fn add_groups(&mut self, table: Option<&Value>, kind: DepKind) {
        let Some(groups) = table.and_then(Value::as_table) else {
            return;
        };
        for items in groups.values() {
            self.add_array(Some(items), kind);
        }
    }

    fn add_array(&mut self, array: Option<&Value>, kind: DepKind) {
        let Some(items) = array.and_then(Value::as_array) else {
            return;
        };
        for raw in items.iter().filter_map(Value::as_str) {
            if let Some(name) = extract_pep508_name(raw) {
                self.record(name, kind);
            }
        }
    }

    fn add_name(&mut self, raw_name: &str, kind: DepKind) {
        let name = normalize_pep503(raw_name.trim());
        if !name.is_empty() {
            self.record(name, kind);
        }
    }

    fn record(&mut self, name: String, kind: DepKind) {
        if !self.local_packages.contains(&name) {
            self.out
                .insert((self.root.to_owned(), kind_to_rank(kind), name));
        }
    }

    fn record_all(&mut self, names: Vec<String>, kind: DepKind) {
        for name in names {
            self.record(name, kind);
        }
    }
}

fn is_optional_table(value: &Value) -> bool {
    value
        .as_table()
        .and_then(|t| t.get("optional"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

#[cfg(test)]
fn is_pyproject_manifest(path: &str) -> bool {
    let matches_name = path == "pyproject.toml" || path.ends_with("/pyproject.toml");
    let ignored_dir = path
        .split('/')
        .any(|seg| matches!(seg, "vendor" | ".venv" | "venv"));
    matches_name && !ignored_dir
}

#[cfg(test)]
fn package_root(path: &str) -> String {
    path.strip_suffix("/pyproject.toml")
        .unwrap_or("")
        .to_owned()
}

fn declared_package_name(doc: &Value) -> Option<String> {
    let name = doc
        .get("project")
        .and_then(|p| p.get("name"))
        .and_then(Value::as_str)
        .or_else(|| {
            doc.get("tool")
                .and_then(|t| t.get("poetry"))
                .and_then(|p| p.get("name"))
                .and_then(Value::as_str)
        })?;
    let trimmed = name.trim();
    (!trimmed.is_empty()).then(|| normalize_pep503(trimmed))
}

fn kind_to_rank(kind: DepKind) -> u8 {
    match kind {
        DepKind::Runtime => 0,
        DepKind::Dev => 1,
        DepKind::Optional => 2,
    }
}

fn rank_to_kind(rank: u8) -> DepKind {
    match rank {
        0 => DepKind::Runtime,
        1 => DepKind::Dev,
        _ => DepKind::Optional,
    }
}
