#[cfg(test)]
mod tests;

use std::collections::{BTreeSet, HashMap, HashSet};

use codeint_model::language::{DeclaredDep, DepKind, Manifest};
use serde_json::Value;

use crate::workspaces::{PACKAGE_MANIFEST, workspace_packages};

const RUNTIME_ORDER: u8 = 0;
const DEV_ORDER: u8 = 1;
const OPTIONAL_ORDER: u8 = 2;

const DEP_SECTIONS: [(&str, u8); 4] = [
    ("dependencies", RUNTIME_ORDER),
    ("devDependencies", DEV_ORDER),
    ("optionalDependencies", OPTIONAL_ORDER),
    ("peerDependencies", OPTIONAL_ORDER),
];

const PACKAGE_BIN_ALIASES: &[(&str, &[&str])] = &[
    ("typescript", &["tsc", "tsserver"]),
    ("@changesets/cli", &["changeset"]),
    ("@playwright/test", &["playwright"]),
    ("@sveltejs/kit", &["svelte-kit"]),
    ("@sveltejs/package", &["svelte-package"]),
    ("@babel/cli", &["babel"]),
    ("@tailwindcss/cli", &["tailwindcss"]),
    ("@commitlint/cli", &["commitlint"]),
    ("@arethetypeswrong/cli", &["attw"]),
    ("@angular/cli", &["ng"]),
    ("@nestjs/cli", &["nest"]),
    ("@docusaurus/core", &["docusaurus"]),
    ("@remix-run/dev", &["remix"]),
    ("@storybook/cli", &["storybook"]),
    ("@astrojs/check", &["astro-check"]),
    ("npm-run-all", &["run-s", "run-p"]),
    ("npm-run-all2", &["npm-run-all", "run-s", "run-p"]),
    ("webpack-cli", &["webpack"]),
    ("postcss-cli", &["postcss"]),
    ("cpy-cli", &["cpy"]),
    ("del-cli", &["del"]),
    ("start-server-and-test", &["start-test"]),
    ("nuxt", &["nuxi"]),
];

const GENERIC_SCOPED_TAILS: &[&str] = &[
    "build", "check", "cli", "config", "core", "dev", "format", "lint", "loader", "package",
    "plugin", "preset", "publish", "release", "run", "serve", "start", "test", "tests", "types",
    "utils", "watch",
];

pub(crate) fn extract_declared(manifests: &[Manifest]) -> Vec<DeclaredDep> {
    let active: Vec<(&str, Value)> = manifests.iter().filter_map(parse_active_package).collect();
    let local_names = collect_local_names(&active);
    let workspace_members: HashSet<String> = workspace_packages(manifests)
        .unwrap_or_default()
        .into_iter()
        .collect();
    let root_script_tokens = collect_root_script_tokens(&active, &workspace_members);

    let mut collected = BTreeSet::new();
    for (root, doc) in &active {
        let own_tokens;
        let script_tokens = if root.is_empty() {
            &root_script_tokens
        } else {
            own_tokens = script_tokens_for(doc);
            &own_tokens
        };
        collect_package_deps(root, doc, &local_names, script_tokens, &mut collected);
    }

    collected
        .into_iter()
        .map(|(package_root, order, name)| DeclaredDep {
            package_root,
            name,
            kind: dep_kind_from_order(order),
        })
        .collect()
}

pub(crate) fn declared_names_by_package(
    manifests: &[Manifest],
) -> HashMap<String, HashSet<String>> {
    let mut by_package: HashMap<String, HashSet<String>> = HashMap::new();
    for (root, doc) in manifests.iter().filter_map(parse_active_package) {
        let names = by_package.entry(root.to_owned()).or_default();
        for &(field, _) in &DEP_SECTIONS {
            let Some(entries) = doc.get(field).and_then(Value::as_object) else {
                continue;
            };
            for raw_name in entries.keys() {
                let name = raw_name.trim();
                if !name.is_empty() {
                    names.insert(name.to_owned());
                }
            }
        }
    }
    by_package
}

fn collect_local_names(active: &[(&str, Value)]) -> HashSet<String> {
    active
        .iter()
        .filter_map(|(_, doc)| doc.get("name").and_then(Value::as_str))
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

fn collect_root_script_tokens(
    active: &[(&str, Value)],
    workspace_members: &HashSet<String>,
) -> HashSet<String> {
    let mut tokens = HashSet::new();
    for (root, doc) in active {
        if root.is_empty() || workspace_members.contains(*root) {
            collect_script_tokens(doc, &mut tokens);
        }
    }
    tokens
}

fn script_tokens_for(doc: &Value) -> HashSet<String> {
    let mut tokens = HashSet::new();
    collect_script_tokens(doc, &mut tokens);
    tokens
}

fn collect_script_tokens(doc: &Value, out: &mut HashSet<String>) {
    let Some(scripts) = doc.get("scripts").and_then(Value::as_object) else {
        return;
    };
    for command in scripts.values().filter_map(Value::as_str) {
        tokenize_shell_command(command, out);
    }
}

fn tokenize_shell_command(command: &str, out: &mut HashSet<String>) {
    for raw in command.split(is_shell_separator) {
        let token = raw.trim();
        if token.is_empty() || token.starts_with('-') || is_env_assignment(token) {
            continue;
        }
        let normalized = normalize_command_token(token);
        if !normalized.is_empty() {
            out.insert(normalized.to_owned());
        }
    }
}

fn is_shell_separator(ch: char) -> bool {
    ch.is_whitespace() || matches!(ch, '&' | '|' | ';' | '(' | ')' | '`' | '"' | '\'')
}

fn is_env_assignment(token: &str) -> bool {
    let Some((name, _)) = token.split_once('=') else {
        return false;
    };
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn normalize_command_token(token: &str) -> &str {
    let bin = token
        .strip_prefix("./node_modules/.bin/")
        .or_else(|| token.strip_prefix("node_modules/.bin/"))
        .unwrap_or(token);
    strip_registry_version(bin)
}

pub(crate) fn strip_registry_version(candidate: &str) -> &str {
    let search_start = if candidate.starts_with('@') {
        match candidate.find('/') {
            Some(slash) => slash + 1,
            None => return candidate,
        }
    } else {
        0
    };
    match candidate[search_start..].find('@') {
        Some(0) => "",
        Some(offset) => &candidate[..search_start + offset],
        None => candidate,
    }
}

fn is_invoked_in_scripts(package_name: &str, script_tokens: &HashSet<String>) -> bool {
    if script_tokens.contains(package_name) {
        return true;
    }
    if let Some((_, aliases)) = PACKAGE_BIN_ALIASES
        .iter()
        .find(|&&(pkg, _)| pkg == package_name)
        && aliases.iter().any(|alias| script_tokens.contains(*alias))
    {
        return true;
    }
    if let Some((_, tail)) = package_name
        .strip_prefix('@')
        .and_then(|rest| rest.split_once('/'))
        && tail.len() >= 3
        && !GENERIC_SCOPED_TAILS.contains(&tail)
        && script_tokens.contains(tail)
    {
        return true;
    }
    false
}

fn is_ambient_types_package(name: &str) -> bool {
    name.starts_with("@types/")
}

fn parse_active_package(manifest: &Manifest) -> Option<(&str, Value)> {
    let root = if manifest.path == PACKAGE_MANIFEST {
        ""
    } else {
        let dir = manifest.path.strip_suffix("/package.json")?;
        if dir.split('/').any(|segment| segment == "node_modules") {
            return None;
        }
        dir
    };
    let doc = serde_json::from_str::<Value>(&manifest.text).ok()?;
    Some((root, doc))
}

fn collect_package_deps(
    root: &str,
    doc: &Value,
    local_names: &HashSet<String>,
    script_tokens: &HashSet<String>,
    out: &mut BTreeSet<(String, u8, String)>,
) {
    for &(field, order) in &DEP_SECTIONS {
        let Some(entries) = doc.get(field).and_then(Value::as_object) else {
            continue;
        };
        for (raw_name, spec) in entries {
            if is_workspace_spec(spec) {
                continue;
            }
            let name = raw_name.trim();
            if name.is_empty() || local_names.contains(name) || is_ambient_types_package(name) {
                continue;
            }
            let effective_order =
                if order == DEV_ORDER && is_invoked_in_scripts(name, script_tokens) {
                    OPTIONAL_ORDER
                } else {
                    order
                };
            out.insert((root.to_owned(), effective_order, name.to_owned()));
        }
    }
}

fn is_workspace_spec(spec: &Value) -> bool {
    spec.as_str()
        .is_some_and(|version| version.trim().starts_with("workspace:"))
}

fn dep_kind_from_order(order: u8) -> DepKind {
    match order {
        RUNTIME_ORDER => DepKind::Runtime,
        DEV_ORDER => DepKind::Dev,
        _ => DepKind::Optional,
    }
}
