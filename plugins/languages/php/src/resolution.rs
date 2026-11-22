use std::cmp::Reverse;
use std::collections::HashSet;

use codeint_model::language::{ImportRequest, Manifest, Resolution};
use serde_json::Value;

use crate::workspaces::COMPOSER_MANIFEST;

pub(crate) fn manifest_paths(repository_paths: &[String]) -> Vec<String> {
    let nested = format!("/{COMPOSER_MANIFEST}");
    repository_paths
        .iter()
        .filter(|p| p.as_str() == COMPOSER_MANIFEST || p.ends_with(&nested))
        .cloned()
        .collect()
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
        }
    }

    fn resolve(&self, request: &ImportRequest) -> Resolution {
        match request.specifier.split_once(':') {
            Some(("use", ns_path)) => self.resolve_psr4(&request.importer, ns_path),
            Some(("req", req_path)) => self.resolve_require(&request.importer, req_path),
            _ => Resolution::External,
        }
    }

    fn resolve_psr4(&self, importer: &str, ns_path: &str) -> Resolution {
        let normalized = ns_path.trim_start_matches('\\');
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
                    Resolution::External
                } else {
                    Resolution::Resolved(hit)
                };
            }
        }
        Resolution::External
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
        if req_path.starts_with('/') {
            return Resolution::Unresolved(format!(
                "{req_path:?} is an absolute path, which depends on the machine"
            ));
        }
        let dir = importer.rsplit_once('/').map_or("", |(parent, _)| parent);
        let Some(joined) = normalize_relative(dir, req_path) else {
            return Resolution::Unresolved(format!("{req_path:?} leads outside the repository"));
        };
        if joined.split('/').any(|segment| segment == "vendor") {
            return Resolution::External;
        }
        if self.files.contains(&joined) {
            if joined == importer {
                Resolution::External
            } else {
                Resolution::Resolved(joined)
            }
        } else {
            Resolution::Unresolved(format!("no file of the repository matches {req_path:?}"))
        }
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

fn normalize_relative(base_dir: &str, rel: &str) -> Option<String> {
    let mut segments: Vec<&str> = if base_dir.is_empty() {
        Vec::new()
    } else {
        base_dir.split('/').collect()
    };
    for seg in rel.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                segments.pop()?;
            }
            other => segments.push(other),
        }
    }
    Some(segments.join("/"))
}
