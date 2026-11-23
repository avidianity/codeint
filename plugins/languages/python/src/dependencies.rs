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

pub(crate) fn extract_declared(manifests: &[Manifest]) -> Vec<DeclaredDep> {
    let mut parsed = Vec::new();
    for manifest in manifests {
        if is_pyproject_manifest(&manifest.path)
            && let Ok(doc) = toml::from_str::<Value>(&manifest.text)
        {
            parsed.push((package_root(&manifest.path), doc));
        }
    }
    let local_packages: HashSet<String> = parsed
        .iter()
        .filter_map(|(_, doc)| declared_package_name(doc))
        .collect();
    let mut entries = BTreeSet::new();
    for (root, doc) in &parsed {
        let mut collector = DepCollector {
            root,
            local_packages: &local_packages,
            out: &mut entries,
        };
        collector.visit_document(doc);
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
}

fn is_optional_table(value: &Value) -> bool {
    value
        .as_table()
        .and_then(|t| t.get("optional"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn is_pyproject_manifest(path: &str) -> bool {
    let matches_name = path == "pyproject.toml" || path.ends_with("/pyproject.toml");
    let ignored_dir = path
        .split('/')
        .any(|seg| matches!(seg, "vendor" | ".venv" | "venv"));
    matches_name && !ignored_dir
}

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
