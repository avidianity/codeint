use std::collections::HashSet;

use codeint_model::language::{ImportRequest, Manifest, Resolution};
use serde_json::Value;

use crate::imports::{REQ_PREFIX, USE_PREFIX};
use crate::workspaces::COMPOSER_MANIFEST;

pub(crate) fn manifest_paths(repository_paths: &[String]) -> Vec<String> {
    repository_paths
        .iter()
        .filter(|path| {
            *path == COMPOSER_MANIFEST || path.ends_with(&format!("/{COMPOSER_MANIFEST}"))
        })
        .cloned()
        .collect()
}

struct Psr4Mapping {
    prefix: String,
    dirs: Vec<String>,
}

pub(crate) struct PhpResolver {
    files: HashSet<String>,
    psr4: Vec<Psr4Mapping>,
}

impl PhpResolver {
    pub(crate) fn new(repository_paths: &[String], manifests: &[Manifest]) -> PhpResolver {
        let files: HashSet<String> = repository_paths.iter().cloned().collect();
        let mut psr4 = Vec::new();
        for manifest in manifests {
            let base_dir = manifest
                .path
                .strip_suffix(&format!("/{COMPOSER_MANIFEST}"))
                .unwrap_or("");
            let Ok(value) = serde_json::from_str::<Value>(&manifest.text) else {
                continue;
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
                    let raw_dirs: Vec<&str> = match target {
                        Value::String(s) => vec![s.as_str()],
                        Value::Array(items) => items.iter().filter_map(Value::as_str).collect(),
                        _ => Vec::new(),
                    };
                    let dirs: Vec<String> = raw_dirs
                        .into_iter()
                        .map(|d| join_dir(base_dir, d))
                        .collect();
                    if !prefix.is_empty() && !dirs.is_empty() {
                        psr4.push(Psr4Mapping { prefix, dirs });
                    }
                }
            }
        }
        psr4.sort_by_key(|m| std::cmp::Reverse(m.prefix.len()));
        PhpResolver { files, psr4 }
    }

    pub(crate) fn resolve(&self, request: &ImportRequest) -> Resolution {
        if let Some(ns_path) = request.specifier.strip_prefix(USE_PREFIX) {
            return self.resolve_psr4(&request.importer, ns_path);
        }
        if let Some(req_path) = request.specifier.strip_prefix(REQ_PREFIX) {
            return self.resolve_require(&request.importer, req_path);
        }
        Resolution::External
    }

    fn resolve_psr4(&self, importer: &str, ns_path: &str) -> Resolution {
        let normalized = ns_path.trim_start_matches('\\');
        for mapping in &self.psr4 {
            if let Some(rest) = normalized.strip_prefix(&mapping.prefix) {
                let rel = rest.replace('\\', "/");
                if rel.is_empty() {
                    continue;
                }
                for dir in &mapping.dirs {
                    let candidate = if dir.is_empty() {
                        format!("{rel}.php")
                    } else {
                        format!("{dir}/{rel}.php")
                    };
                    if self.files.contains(&candidate) {
                        return if candidate == importer {
                            Resolution::External
                        } else {
                            Resolution::Resolved(candidate)
                        };
                    }
                }
            }
        }
        Resolution::External
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
