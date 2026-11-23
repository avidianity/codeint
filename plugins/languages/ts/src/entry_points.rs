use std::collections::{BTreeSet, HashSet};

use codeint_model::language::Manifest;
use codeint_model::scan::normalize_relative_path;
use serde_json::Value;

use crate::workspaces::PACKAGE_MANIFEST;

const OUTPUT_TO_SOURCE_ALIASES: [(&str, &[&str]); 4] = [
    (".js", &[".ts", ".tsx"]),
    (".jsx", &[".tsx"]),
    (".mjs", &[".mts"]),
    (".cjs", &[".cts"]),
];

pub(crate) fn entry_points(
    repository_paths: &[String],
    manifests: &[Manifest],
    extensions: &[&str],
) -> Vec<String> {
    let mut finder = TsEntryFinder::new(repository_paths, extensions);
    for manifest in manifests {
        finder.scan_manifest(manifest);
    }
    finder.entries.into_iter().collect()
}

struct TsEntryFinder<'a> {
    paths: &'a [String],
    extensions: &'a [&'a str],
    file_set: HashSet<&'a str>,
    entries: BTreeSet<String>,
}

impl<'a> TsEntryFinder<'a> {
    fn new(paths: &'a [String], extensions: &'a [&'a str]) -> TsEntryFinder<'a> {
        TsEntryFinder {
            paths,
            extensions,
            file_set: paths.iter().map(String::as_str).collect(),
            entries: BTreeSet::new(),
        }
    }

    fn scan_manifest(&mut self, manifest: &Manifest) {
        let base_dir = if manifest.path == PACKAGE_MANIFEST {
            ""
        } else if let Some(dir) = manifest.path.strip_suffix(&format!("/{PACKAGE_MANIFEST}")) {
            dir
        } else {
            return;
        };
        let Ok(doc) = serde_json::from_str::<Value>(&manifest.text) else {
            return;
        };
        for key in ["main", "module"] {
            if let Some(target) = doc.get(key).and_then(Value::as_str) {
                self.resolve_source_target(base_dir, target);
            }
        }
        if let Some(bin) = doc.get("bin") {
            self.collect_bin(base_dir, bin);
        }
        if let Some(exports) = doc.get("exports") {
            self.collect_exports(base_dir, exports);
        }
    }

    fn collect_bin(&mut self, base_dir: &str, bin: &Value) {
        if let Some(target) = bin.as_str() {
            self.resolve_bin_target(base_dir, target);
            return;
        }
        if let Some(map) = bin.as_object() {
            for target in map.values().filter_map(Value::as_str) {
                self.resolve_bin_target(base_dir, target);
            }
        }
    }

    fn collect_exports(&mut self, base_dir: &str, value: &Value) {
        match value {
            Value::String(target) if target.contains('*') => {
                self.resolve_wildcard_export(base_dir, target);
            }
            Value::String(target) => self.resolve_source_target(base_dir, target),
            Value::Array(items) => {
                for item in items {
                    self.collect_exports(base_dir, item);
                }
            }
            Value::Object(map) => {
                for item in map.values() {
                    self.collect_exports(base_dir, item);
                }
            }
            _ => {}
        }
    }

    fn resolve_bin_target(&mut self, base_dir: &str, raw: &str) {
        let Some(candidate) = normalize_entry_path(base_dir, raw) else {
            return;
        };
        if self.file_set.contains(candidate.as_str())
            && (is_ts_source_file(&candidate, self.extensions) || has_no_extension(&candidate))
        {
            self.entries.insert(candidate);
            return;
        }
        self.resolve_source_candidate(&candidate);
    }

    fn resolve_source_target(&mut self, base_dir: &str, raw: &str) {
        if let Some(candidate) = normalize_entry_path(base_dir, raw) {
            self.resolve_source_candidate(&candidate);
        }
    }

    fn resolve_source_candidate(&mut self, candidate: &str) {
        if is_declaration_file(candidate) {
            return;
        }
        if self.file_set.contains(candidate) && is_ts_source_file(candidate, self.extensions) {
            self.entries.insert(candidate.to_owned());
            return;
        }
        if let Some(aliased) = self.resolve_output_alias(candidate) {
            self.entries.insert(aliased);
            return;
        }
        if let Some(expanded) = self.resolve_extension_or_index(candidate) {
            self.entries.insert(expanded);
        }
    }

    fn resolve_output_alias(&self, candidate: &str) -> Option<String> {
        for (output_ext, source_exts) in OUTPUT_TO_SOURCE_ALIASES {
            let Some(stem) = candidate.strip_suffix(output_ext) else {
                continue;
            };
            for source_ext in source_exts {
                let aliased = format!("{stem}{source_ext}");
                if self.file_set.contains(aliased.as_str()) && !is_declaration_file(&aliased) {
                    return Some(aliased);
                }
            }
        }
        None
    }

    fn resolve_extension_or_index(&self, candidate: &str) -> Option<String> {
        for ext in self.extensions {
            let with_ext = format!("{candidate}.{ext}");
            if self.file_set.contains(with_ext.as_str()) && !is_declaration_file(&with_ext) {
                return Some(with_ext);
            }
        }
        for ext in self.extensions {
            let index_file = format!("{candidate}/index.{ext}");
            if self.file_set.contains(index_file.as_str()) {
                return Some(index_file);
            }
        }
        None
    }

    fn resolve_wildcard_export(&mut self, base_dir: &str, raw: &str) {
        if raw.starts_with('/') || raw.contains('\\') {
            return;
        }
        let Some((prefix_raw, suffix)) = raw.split_once('*') else {
            return;
        };
        if suffix.contains('*') {
            return;
        }
        let Some(normalized_prefix) = normalize_relative_path(base_dir, prefix_raw) else {
            return;
        };
        let prefix = if prefix_raw.ends_with('/') && !normalized_prefix.is_empty() {
            format!("{normalized_prefix}/")
        } else {
            normalized_prefix
        };
        for path in self.paths {
            if let Some(matched_suffix) = wildcard_suffix_match(path, &prefix, suffix)
                && path.len() > prefix.len() + matched_suffix.len()
                && is_ts_source_file(path, self.extensions)
            {
                self.entries.insert(path.clone());
            }
        }
    }
}

fn wildcard_suffix_match<'a>(path: &str, prefix: &str, suffix: &'a str) -> Option<&'a str> {
    if !path.starts_with(prefix) {
        return None;
    }
    if path.ends_with(suffix) {
        return Some(suffix);
    }
    for (output_ext, source_exts) in OUTPUT_TO_SOURCE_ALIASES {
        if suffix == output_ext {
            for source_ext in source_exts {
                if path.ends_with(source_ext) {
                    return Some(source_ext);
                }
            }
        }
    }
    None
}

fn normalize_entry_path(base_dir: &str, raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.starts_with('/') || trimmed.contains('\\') {
        return None;
    }
    let normalized = normalize_relative_path(base_dir, trimmed)?;
    (!normalized.is_empty()).then_some(normalized)
}

fn is_declaration_file(path: &str) -> bool {
    path.ends_with(".d.ts") || path.ends_with(".d.mts") || path.ends_with(".d.cts")
}

fn is_ts_source_file(path: &str, extensions: &[&str]) -> bool {
    if is_declaration_file(path) {
        return false;
    }
    let file_name = path.rsplit_once('/').map_or(path, |(_, name)| name);
    let Some((_, ext)) = file_name.rsplit_once('.') else {
        return false;
    };
    extensions.contains(&ext)
}

fn has_no_extension(path: &str) -> bool {
    let file_name = path.rsplit_once('/').map_or(path, |(_, name)| name);
    !file_name.contains('.')
}
