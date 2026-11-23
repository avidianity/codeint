use std::cmp::Reverse;
use std::collections::HashSet;

use codeint_model::language::{ImportRequest, Manifest, Resolution};
use codeint_model::scan::{find_manifests, normalize_relative_path, resolve_relative_file};
use serde_json::Value;

use crate::builtins::is_builtin_symbol;
use crate::dependencies::declared_package_names;
use crate::workspaces::COMPOSER_MANIFEST;

#[cfg(test)]
mod tests;

pub(crate) fn manifest_paths(repository_paths: &[String]) -> Vec<String> {
    find_manifests(repository_paths, COMPOSER_MANIFEST)
}

struct Psr4Rule {
    prefix: String,
    dirs: Vec<String>,
}

pub(crate) fn resolve_batch(
    repository_paths: &[String],
    manifests: &[Manifest],
    requests: &[ImportRequest],
) -> Vec<Resolution> {
    let resolver = PhpResolver::new(repository_paths, manifests);
    requests.iter().map(|req| resolver.resolve(req)).collect()
}

struct PhpResolver {
    files: HashSet<String>,
    rules: Vec<Psr4Rule>,
    declared_packages: HashSet<String>,
}

impl PhpResolver {
    fn new(repository_paths: &[String], manifests: &[Manifest]) -> PhpResolver {
        let mut rules = Vec::new();
        for manifest in manifests {
            collect_psr4_rules(manifest, &mut rules);
        }
        rules.sort_by_key(|rule| Reverse(rule.prefix.len()));
        PhpResolver {
            files: repository_paths.iter().cloned().collect(),
            rules,
            declared_packages: declared_package_names(manifests),
        }
    }

    fn resolve(&self, request: &ImportRequest) -> Resolution {
        match request.specifier.split_once(':') {
            Some(("use", ns_path)) => self.resolve_psr4(&request.importer, ns_path),
            Some(("req", req_path)) => self.resolve_require(&request.importer, req_path),
            _ => Resolution::Unresolved(format!(
                "unrecognized PHP import specifier {:?}",
                request.specifier
            )),
        }
    }

    fn resolve_psr4(&self, importer: &str, ns_path: &str) -> Resolution {
        let normalized = ns_path.trim_start_matches('\\');
        if normalized.is_empty() {
            return Resolution::Unresolved(format!("empty PHP namespace import {ns_path:?}"));
        }
        for rule in &self.rules {
            let Some(rest) = normalized.strip_prefix(&rule.prefix) else {
                continue;
            };
            if rest.is_empty() {
                continue;
            }
            let rel = rest.replace('\\', "/");
            if let Some(hit) = self.first_matching_file(&rule.dirs, &rel) {
                return if hit == importer {
                    Resolution::Builtin
                } else {
                    Resolution::Resolved(hit)
                };
            }
        }
        if is_builtin_symbol(normalized) {
            return Resolution::Builtin;
        }
        Resolution::External(normalize_external_namespace(
            normalized,
            &self.declared_packages,
        ))
    }

    fn first_matching_file(&self, dirs: &[String], rel: &str) -> Option<String> {
        for dir in dirs {
            let candidate = if dir.is_empty() {
                format!("{rel}.php")
            } else {
                format!("{dir}/{rel}.php")
            };
            if self.files.contains(&candidate) {
                return Some(candidate);
            }
        }
        None
    }

    fn resolve_require(&self, importer: &str, req_path: &str) -> Resolution {
        if let Some(vendor_res) = classify_vendor_require(importer, req_path) {
            return vendor_res;
        }
        match resolve_relative_file(&self.files, importer, req_path, true) {
            Resolution::External(pkg) if pkg.is_empty() => Resolution::Builtin,
            other => other,
        }
    }
}

fn classify_vendor_require(importer: &str, req_path: &str) -> Option<Resolution> {
    if req_path.starts_with('/') {
        return None;
    }
    let parent = importer.rsplit_once('/').map_or("", |(dir, _)| dir);
    let joined = normalize_relative_path(parent, req_path)?;
    let segments: Vec<&str> = joined.split('/').collect();
    let vendor_idx = segments.iter().position(|&seg| seg == "vendor")?;
    let after = &segments[vendor_idx + 1..];
    match after {
        ["autoload.php"] | [] => Some(Resolution::Builtin),
        [vendor, package, ..] => Some(Resolution::External(format!(
            "{}/{}",
            vendor.to_ascii_lowercase(),
            package.to_ascii_lowercase()
        ))),
        _ => Some(Resolution::Builtin),
    }
}

pub(crate) fn normalize_external_namespace(ns: &str, declared: &HashSet<String>) -> String {
    let segments: Vec<&str> = ns.split('\\').filter(|s| !s.is_empty()).collect();
    let Some(&first) = segments.first() else {
        return String::new();
    };
    if segments.len() == 1 {
        return to_kebab_segment(first);
    }
    let vendor = first.to_ascii_lowercase();
    let primary = format!("{vendor}/{}", to_kebab_segment(segments[1]));
    let lower_pkg = format!("{vendor}/{}", segments[1].to_ascii_lowercase());
    let mut candidates = vec![primary.clone(), lower_pkg];
    if let Some(&third) = segments.get(2) {
        candidates.push(format!("{vendor}/{}", to_kebab_segment(third)));
        candidates.push(format!("{vendor}/{}", third.to_ascii_lowercase()));
    }
    candidates.push(format!("{vendor}/{vendor}"));
    for candidate in candidates {
        if declared.contains(&candidate) {
            return candidate;
        }
    }
    primary
}

fn to_kebab_segment(segment: &str) -> String {
    let chars: Vec<char> = segment.chars().collect();
    let mut out = String::with_capacity(segment.len() + 4);
    for (idx, &ch) in chars.iter().enumerate() {
        if ch == '_' {
            push_hyphen(&mut out);
            continue;
        }
        let needs_hyphen = idx > 0
            && ch.is_ascii_uppercase()
            && (chars[idx - 1].is_ascii_lowercase()
                || chars[idx - 1].is_ascii_digit()
                || (chars[idx - 1].is_ascii_uppercase()
                    && chars.get(idx + 1).is_some_and(char::is_ascii_lowercase)));
        if needs_hyphen {
            push_hyphen(&mut out);
        }
        out.push(ch.to_ascii_lowercase());
    }
    out
}

fn push_hyphen(out: &mut String) {
    if !out.is_empty() && !out.ends_with('-') {
        out.push('-');
    }
}

fn collect_psr4_rules(manifest: &Manifest, out: &mut Vec<Psr4Rule>) {
    let base_dir = manifest
        .path
        .strip_suffix(&format!("/{COMPOSER_MANIFEST}"))
        .unwrap_or("");
    let Ok(value) = serde_json::from_str::<Value>(&manifest.text) else {
        return;
    };
    for section in ["autoload", "autoload-dev"] {
        let Some(map) = value
            .get(section)
            .and_then(|v| v.get("psr-4"))
            .and_then(Value::as_object)
        else {
            continue;
        };
        for (ns, target) in map {
            let prefix = ns.trim_start_matches('\\').to_owned();
            let dirs = target_dirs(base_dir, target);
            if !prefix.is_empty() && !dirs.is_empty() {
                out.push(Psr4Rule { prefix, dirs });
            }
        }
    }
}

fn target_dirs(base_dir: &str, target: &Value) -> Vec<String> {
    match target {
        Value::String(s) => vec![join_dir(base_dir, s)],
        Value::Array(items) => items
            .iter()
            .filter_map(Value::as_str)
            .map(|d| join_dir(base_dir, d))
            .collect(),
        _ => Vec::new(),
    }
}

fn join_dir(base: &str, sub: &str) -> String {
    let sub = sub.trim_start_matches("./").trim_matches('/');
    match (base.is_empty(), sub.is_empty()) {
        (true, true) => String::new(),
        (true, false) => sub.to_owned(),
        (false, true) => base.to_owned(),
        (false, false) => format!("{base}/{sub}"),
    }
}
