mod cakephp;
mod codeigniter;
mod laravel;
mod symfony;

use std::collections::BTreeSet;

use codeint_model::language::{FrameworkProfileResult, FrameworkProfileSelection, Manifest};
use serde_json::{Map, Value};

use crate::workspaces::COMPOSER_MANIFEST;

pub(crate) struct ConventionRules {
    pub(crate) core_entries: &'static [&'static str],
    pub(crate) direct_php_dirs: &'static [&'static str],
    pub(crate) direct_suffixed_dirs: &'static [(&'static str, &'static str)],
    pub(crate) nested_php_dirs: &'static [&'static str],
    pub(crate) nested_suffixed_dirs: &'static [(&'static str, &'static str)],
    pub(crate) generated_files: &'static [&'static str],
    pub(crate) generated_dirs: &'static [&'static str],
    pub(crate) fallback_mappings: &'static [(&'static str, &'static str)],
}

impl ConventionRules {
    fn collect_entries(&self, pkg: &ComposerPackage<'_>) -> Vec<String> {
        pkg.collect_paths(|rel| {
            self.core_entries.contains(&rel)
                || self
                    .direct_php_dirs
                    .iter()
                    .any(|dir| is_direct_php_in(rel, dir))
                || self
                    .direct_suffixed_dirs
                    .iter()
                    .any(|(dir, suffix)| is_direct_with_suffix_in(rel, dir, suffix))
                || self
                    .nested_php_dirs
                    .iter()
                    .any(|dir| is_nested_php_in(rel, dir))
                || self
                    .nested_suffixed_dirs
                    .iter()
                    .any(|(dir, suffix)| is_nested_with_suffix_in(rel, dir, suffix))
        })
    }

    fn collect_generated(&self, pkg: &ComposerPackage<'_>) -> Vec<String> {
        pkg.collect_paths(|rel| {
            self.generated_files.contains(&rel)
                || self.generated_dirs.iter().any(|dir| is_under_dir(rel, dir))
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum PhpFramework {
    CakePhp,
    CodeIgniter,
    Laravel,
    Symfony,
}

impl PhpFramework {
    const ALL: [Self; 4] = [
        Self::Laravel,
        Self::Symfony,
        Self::CakePhp,
        Self::CodeIgniter,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Laravel => "laravel",
            Self::Symfony => "symfony",
            Self::CakePhp => "cakephp",
            Self::CodeIgniter => "codeigniter",
        }
    }

    fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|fw| fw.name() == raw)
    }

    fn detect(self, pkg: &ComposerPackage<'_>) -> bool {
        match self {
            Self::Laravel => laravel::detect(pkg),
            Self::Symfony => symfony::detect(pkg),
            Self::CakePhp => cakephp::detect(pkg),
            Self::CodeIgniter => codeigniter::detect(pkg),
        }
    }

    fn rules(self) -> &'static ConventionRules {
        match self {
            Self::Laravel => &laravel::RULES,
            Self::Symfony => &symfony::RULES,
            Self::CakePhp => &cakephp::RULES,
            Self::CodeIgniter => &codeigniter::RULES,
        }
    }
}

pub(crate) struct ComposerPackage<'a> {
    entries: Vec<(&'a str, &'a str)>,
    composer: Option<Map<String, Value>>,
}

impl<'a> ComposerPackage<'a> {
    fn build<P: AsRef<str>>(root: &'a str, files: &'a [P], manifests: &[Manifest]) -> Self {
        let entries = files
            .iter()
            .filter_map(|full| {
                let full = full.as_ref();
                let rel = relative_to_root(root, full)?;
                (!contains_vendor(rel)).then_some((full, rel))
            })
            .collect();
        let manifest_path = if root.is_empty() {
            COMPOSER_MANIFEST.to_owned()
        } else {
            format!("{root}/{COMPOSER_MANIFEST}")
        };
        let composer = manifests
            .iter()
            .find(|m| m.path == manifest_path && !contains_vendor(&m.path))
            .and_then(|m| serde_json::from_str::<Value>(&m.text).ok())
            .and_then(|val| match val {
                Value::Object(map) => Some(map),
                _ => None,
            });
        Self { entries, composer }
    }

    fn collect_paths(&self, predicate: impl Fn(&str) -> bool) -> Vec<String> {
        self.entries
            .iter()
            .filter(|(_, rel)| predicate(rel))
            .map(|(full, _)| (*full).to_owned())
            .collect()
    }

    pub(crate) fn contains_file(&self, target: &str) -> bool {
        self.entries.iter().any(|(_, rel)| *rel == target)
    }

    pub(crate) fn contains_any_file(&self, targets: &[&str]) -> bool {
        self.entries.iter().any(|(_, rel)| targets.contains(rel))
    }

    pub(crate) fn contains_php_under(&self, dir: &str) -> bool {
        self.entries
            .iter()
            .any(|(_, rel)| is_nested_php_in(rel, dir))
    }

    pub(crate) fn requires_in(&self, section: &str, package: &str) -> bool {
        self.composer
            .as_ref()
            .and_then(|obj| obj.get(section))
            .and_then(Value::as_object)
            .is_some_and(|deps| deps.keys().any(|k| k.eq_ignore_ascii_case(package)))
    }

    pub(crate) fn requires(&self, package: &str) -> bool {
        self.requires_in("require", package)
    }

    pub(crate) fn package_type(&self) -> Option<&str> {
        self.composer
            .as_ref()
            .and_then(|obj| obj.get("type"))
            .and_then(Value::as_str)
    }
}

fn relative_to_root<'p>(root: &str, full: &'p str) -> Option<&'p str> {
    if root.is_empty() {
        Some(full)
    } else {
        full.strip_prefix(root)?.strip_prefix('/')
    }
}

fn contains_vendor(path: &str) -> bool {
    path.split('/').any(|segment| segment == "vendor")
}

fn is_under_dir(rel: &str, dir: &str) -> bool {
    rel.strip_prefix(dir)
        .is_some_and(|rest| rest.starts_with('/'))
}

fn is_direct_php_in(rel: &str, dir: &str) -> bool {
    is_direct_with_suffix_in(rel, dir, ".php")
}

fn is_direct_with_suffix_in(rel: &str, dir: &str, suffix: &str) -> bool {
    rel.strip_prefix(dir)
        .and_then(|rest| rest.strip_prefix('/'))
        .is_some_and(|file| {
            !file.contains('/') && file.ends_with(suffix) && file.len() > suffix.len()
        })
}

fn is_nested_php_in(rel: &str, dir: &str) -> bool {
    is_under_dir(rel, dir) && rel.ends_with(".php")
}

fn is_nested_with_suffix_in(rel: &str, dir: &str, suffix: &str) -> bool {
    let Some(rest) = rel.strip_prefix(dir).and_then(|s| s.strip_prefix('/')) else {
        return false;
    };
    let file_name = rest.rsplit_once('/').map_or(rest, |(_, name)| name);
    file_name.ends_with(suffix) && file_name.len() > suffix.len()
}

fn namespace_prefix_fallback(specifier: &str, mappings: &[(&str, &str)]) -> Vec<String> {
    let mut candidates = Vec::new();
    for &(prefix, target_dir) in mappings {
        let Some(rest) = specifier.strip_prefix(prefix) else {
            continue;
        };
        if rest.is_empty()
            || rest.contains('/')
            || rest
                .split('\\')
                .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
        {
            continue;
        }
        let rel = rest.replace('\\', "/");
        candidates.push(format!("{target_dir}/{rel}.php"));
    }
    candidates
}

pub(crate) fn resolve_framework_profiles<P: AsRef<str>>(
    package_root: &str,
    files: &[P],
    manifests: &[Manifest],
    selection: FrameworkProfileSelection,
) -> Result<FrameworkProfileResult, String> {
    let pkg = ComposerPackage::build(package_root, files, manifests);
    let active = choose_frameworks(&pkg, selection)?;
    Ok(FrameworkProfileResult {
        entry_points: collect_union(&pkg, &active, ConventionRules::collect_entries),
        generated_paths: collect_union(&pkg, &active, ConventionRules::collect_generated),
        active_profiles: active.into_iter().map(|fw| fw.name().to_owned()).collect(),
        reference_ranges: Vec::new(),
        reference_range_note: None,
    })
}

pub(crate) fn known_profiles() -> Vec<String> {
    PhpFramework::ALL
        .into_iter()
        .map(|fw| fw.name().to_owned())
        .collect()
}

fn collect_union(
    pkg: &ComposerPackage<'_>,
    active: &BTreeSet<PhpFramework>,
    extract: fn(&ConventionRules, &ComposerPackage<'_>) -> Vec<String>,
) -> Vec<String> {
    active
        .iter()
        .flat_map(|fw| extract(fw.rules(), pkg))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn choose_frameworks(
    pkg: &ComposerPackage<'_>,
    selection: FrameworkProfileSelection,
) -> Result<BTreeSet<PhpFramework>, String> {
    match selection {
        FrameworkProfileSelection::Disabled => Ok(BTreeSet::new()),
        FrameworkProfileSelection::Auto => Ok(detect_all(pkg, &BTreeSet::new())),
        FrameworkProfileSelection::Exclude(names) => {
            let skipped: BTreeSet<PhpFramework> = names
                .iter()
                .map(String::as_str)
                .filter_map(PhpFramework::parse)
                .collect();
            Ok(detect_all(pkg, &skipped))
        }
        FrameworkProfileSelection::Only(names) => names
            .iter()
            .map(|raw| {
                PhpFramework::parse(raw)
                    .ok_or_else(|| format!("unknown PHP Framework profile {raw:?}"))
            })
            .collect(),
    }
}

fn detect_all(
    pkg: &ComposerPackage<'_>,
    excluded: &BTreeSet<PhpFramework>,
) -> BTreeSet<PhpFramework> {
    PhpFramework::ALL
        .into_iter()
        .filter(|fw| !excluded.contains(fw) && fw.detect(pkg))
        .collect()
}

pub(crate) fn fallback_import_candidates(
    active_profiles: &[String],
    namespace: &str,
) -> Vec<String> {
    let mut resolved = Vec::new();
    let mut seen = BTreeSet::new();
    for raw in active_profiles {
        let Some(fw) = PhpFramework::parse(raw) else {
            continue;
        };
        for path in namespace_prefix_fallback(namespace, fw.rules().fallback_mappings) {
            if seen.insert(path.clone()) {
                resolved.push(path);
            }
        }
    }
    resolved
}
