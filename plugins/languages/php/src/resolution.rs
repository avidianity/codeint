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

enum Psr4Match {
    Resolved(Resolution),
    LocalNamespace,
    PrefixOnly,
    NoMatch,
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
        let normalized = ns_path.trim_matches('\\');
        if normalized.is_empty() {
            return Resolution::Unresolved(format!("empty PHP namespace import {ns_path:?}"));
        }
        match self.scan_psr4_rules(importer, normalized) {
            Psr4Match::Resolved(resolution) => resolution,
            Psr4Match::LocalNamespace => unresolved_internal_namespace(normalized),
            Psr4Match::PrefixOnly => self.resolve_psr4_prefix_fallback(normalized),
            Psr4Match::NoMatch => self.resolve_non_psr4_namespace(normalized),
        }
    }

    fn scan_psr4_rules(&self, importer: &str, normalized: &str) -> Psr4Match {
        let mut fallback = Psr4Match::NoMatch;
        for rule in &self.rules {
            match self.match_psr4_rule(importer, normalized, rule) {
                Psr4Match::Resolved(resolution) => return Psr4Match::Resolved(resolution),
                Psr4Match::LocalNamespace => fallback = Psr4Match::LocalNamespace,
                Psr4Match::PrefixOnly if matches!(fallback, Psr4Match::NoMatch) => {
                    fallback = Psr4Match::PrefixOnly;
                }
                Psr4Match::PrefixOnly | Psr4Match::NoMatch => {}
            }
        }
        fallback
    }

    fn resolve_psr4_prefix_fallback(&self, normalized: &str) -> Resolution {
        match match_declared_external_namespace(
            normalized,
            &self.declared_packages,
            DeclaredMatchScope::Psr4Subpackage,
        ) {
            Some(external) => Resolution::External(external),
            None => unresolved_internal_namespace(normalized),
        }
    }

    fn resolve_non_psr4_namespace(&self, normalized: &str) -> Resolution {
        if is_builtin_symbol(normalized) || !normalized.contains('\\') {
            Resolution::Builtin
        } else {
            Resolution::External(normalize_external_namespace(
                normalized,
                &self.declared_packages,
            ))
        }
    }

    fn match_psr4_rule(&self, importer: &str, normalized: &str, rule: &Psr4Rule) -> Psr4Match {
        let Some(rest) = normalized.strip_prefix(&rule.prefix) else {
            return if normalized == rule.prefix.trim_end_matches('\\') {
                Psr4Match::LocalNamespace
            } else {
                Psr4Match::NoMatch
            };
        };
        if rest.is_empty() {
            return Psr4Match::LocalNamespace;
        }
        let rel = rest.replace('\\', "/");
        if let Some(hit) = self.first_matching_file(&rule.dirs, &rel) {
            let resolution = if hit == importer {
                Resolution::Builtin
            } else {
                Resolution::Resolved(hit)
            };
            return Psr4Match::Resolved(resolution);
        }
        if self.has_local_files_under(&rule.dirs, &rel) {
            Psr4Match::LocalNamespace
        } else {
            Psr4Match::PrefixOnly
        }
    }

    fn has_local_files_under(&self, dirs: &[String], rel: &str) -> bool {
        dirs.iter().any(|dir| {
            let prefix = if dir.is_empty() {
                format!("{rel}/")
            } else {
                format!("{dir}/{rel}/")
            };
            self.files.iter().any(|file| file.starts_with(&prefix))
        })
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

fn unresolved_internal_namespace(normalized: &str) -> Resolution {
    Resolution::Unresolved(format!(
        "no local PHP file found for internal namespace {normalized:?}"
    ))
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

const STRUCTURAL_SEGMENTS: &[&str] = &[
    "bridge",
    "bridges",
    "bundle",
    "bundles",
    "common",
    "component",
    "components",
    "contract",
    "contracts",
    "ux",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum DeclaredMatchScope {
    Psr4Subpackage,
    External,
}

pub(crate) fn normalize_external_namespace(ns: &str, declared: &HashSet<String>) -> String {
    let segments: Vec<&str> = ns.split('\\').filter(|s| !s.is_empty()).collect();
    let Some(&first) = segments.first() else {
        return String::new();
    };
    if segments.len() == 1 {
        return to_kebab_segment(first);
    }
    if let Some(matched) =
        match_declared_external_namespace(ns, declared, DeclaredMatchScope::External)
    {
        return matched;
    }
    format!(
        "{}/{}",
        first.to_ascii_lowercase(),
        to_kebab_segment(segments[1])
    )
}

fn match_declared_external_namespace(
    ns: &str,
    declared: &HashSet<String>,
    scope: DeclaredMatchScope,
) -> Option<String> {
    let segments: Vec<&str> = ns.split('\\').filter(|s| !s.is_empty()).collect();
    if segments.len() < 2 {
        return None;
    }
    let mut candidates = Vec::new();
    for vendor in vendor_variants(segments[0]) {
        collect_vendor_candidates(&vendor, &segments, scope, &mut candidates);
    }
    if let Some(matched) = candidates.into_iter().find(|c| declared.contains(c)) {
        return Some(matched);
    }
    if scope == DeclaredMatchScope::External {
        return match_unique_package_name(&segments, declared);
    }
    None
}

fn collect_vendor_candidates(
    vendor: &str,
    segments: &[&str],
    scope: DeclaredMatchScope,
    out: &mut Vec<String>,
) {
    if segments.len() >= 3 {
        push_multi_segment_candidates(vendor, segments, scope, out);
    } else {
        push_segment_candidates(vendor, segments[1], out);
    }
    if scope == DeclaredMatchScope::External {
        out.push(format!("{vendor}/{vendor}"));
    }
}

fn push_multi_segment_candidates(
    vendor: &str,
    segments: &[&str],
    scope: DeclaredMatchScope,
    out: &mut Vec<String>,
) {
    let s1_lower = segments[1].to_ascii_lowercase();
    if STRUCTURAL_SEGMENTS.contains(&s1_lower.as_str()) {
        push_structural_candidates(vendor, segments, &s1_lower, out);
        return;
    }
    if scope == DeclaredMatchScope::External {
        push_psr17_factory_candidate(vendor, segments, out);
    }
    push_joined_candidates(vendor, segments[1], segments[2], out);
    if scope == DeclaredMatchScope::External && segments.len() >= 4 {
        push_class_word_candidates(vendor, segments, out);
    }
    push_segment_candidates(vendor, segments[1], out);
}

fn push_structural_candidates(
    vendor: &str,
    segments: &[&str],
    s1_lower: &str,
    out: &mut Vec<String>,
) {
    let third = segments[2];
    for s2 in segment_variants(third) {
        out.push(format!("{vendor}/{s2}-{s1_lower}"));
        if let Some(singular) = s1_lower.strip_suffix('s') {
            out.push(format!("{vendor}/{s2}-{singular}"));
        } else {
            out.push(format!("{vendor}/{s2}-{s1_lower}s"));
        }
    }
    push_joined_candidates(vendor, segments[1], third, out);
    let prefer_parent = matches!(s1_lower, "contract" | "contracts");
    if prefer_parent {
        push_segment_candidates(vendor, segments[1], out);
    }
    if let Some(&fourth) = segments.get(3) {
        push_joined_candidates(vendor, third, fourth, out);
    }
    push_segment_candidates(vendor, third, out);
    if !prefer_parent {
        push_segment_candidates(vendor, segments[1], out);
    }
}

fn push_psr17_factory_candidate(vendor: &str, segments: &[&str], out: &mut Vec<String>) {
    if vendor == "psr"
        && segments.len() == 4
        && segments[1].eq_ignore_ascii_case("http")
        && segments[2].eq_ignore_ascii_case("message")
        && segments[3]
            .to_ascii_lowercase()
            .ends_with("factoryinterface")
    {
        out.push("psr/http-factory".to_owned());
    }
}

fn push_class_word_candidates(vendor: &str, segments: &[&str], out: &mut Vec<String>) {
    let s1_vars = segment_variants(segments[1]);
    let s2_vars = segment_variants(segments[2]);
    let last_kebab = to_kebab_segment(segments[segments.len() - 1]);
    let words: Vec<&str> = last_kebab.split('-').filter(|w| !w.is_empty()).collect();
    for s1 in &s1_vars {
        for s2 in &s2_vars {
            for &word in &words {
                out.push(format!("{vendor}/{s1}-{s2}-{word}"));
            }
        }
    }
}

fn push_joined_candidates(vendor: &str, left: &str, right: &str, out: &mut Vec<String>) {
    let right_vars = segment_variants(right);
    for l in segment_variants(left) {
        for r in &right_vars {
            push_plurals(vendor, &format!("{l}-{r}"), out);
        }
    }
}

fn push_segment_candidates(vendor: &str, segment: &str, out: &mut Vec<String>) {
    for base in segment_variants(segment) {
        push_plurals(vendor, &base, out);
    }
}

fn push_plurals(vendor: &str, base: &str, out: &mut Vec<String>) {
    out.push(format!("{vendor}/{base}"));
    out.push(format!("{vendor}/{base}s"));
    out.push(format!("{vendor}/{base}es"));
}

fn match_unique_package_name(segments: &[&str], declared: &HashSet<String>) -> Option<String> {
    let kebab_s0 = to_kebab_segment(segments[0]);
    if !kebab_s0.contains('-') {
        return None;
    }
    let s0_vars = segment_variants(segments[0]);
    unique_declared_by_package_name(declared, &s0_vars)
}

fn unique_declared_by_package_name(
    declared: &HashSet<String>,
    package_names: &[String],
) -> Option<String> {
    let mut found: Option<&String> = None;
    for dep in declared {
        let Some((_, pkg)) = dep.split_once('/') else {
            continue;
        };
        if package_names.iter().any(|candidate| candidate == pkg) {
            if found.is_some() {
                return None;
            }
            found = Some(dep);
        }
    }
    found.cloned()
}

fn segment_variants(segment: &str) -> Vec<String> {
    let kebab = to_kebab_segment(segment);
    let lower = segment.to_ascii_lowercase();
    if kebab == lower {
        vec![kebab]
    } else {
        vec![kebab, lower]
    }
}

fn vendor_variants(segment: &str) -> Vec<String> {
    let lower = segment.to_ascii_lowercase();
    let kebab = to_kebab_segment(segment);
    if lower == kebab {
        vec![lower]
    } else {
        vec![lower, kebab]
    }
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
    out.truncate(out.trim_end_matches('-').len());
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
