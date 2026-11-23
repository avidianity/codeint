use std::collections::{BTreeSet, HashSet};

use codeint_model::language::Manifest;
use codeint_model::scan::normalize_relative_path;
use serde::Deserialize;

use crate::workspaces::CARGO_MANIFEST;

#[derive(Deserialize)]
struct CargoTargets {
    package: Option<PackageMeta>,
    lib: Option<TargetSpec>,
    #[serde(default)]
    bin: Vec<TargetSpec>,
}

#[derive(Deserialize)]
struct PackageMeta {
    name: Option<String>,
    autolib: Option<bool>,
    autobins: Option<bool>,
}

#[derive(Deserialize)]
struct TargetSpec {
    name: Option<String>,
    path: Option<String>,
}

pub(crate) fn entry_points(repository_paths: &[String], manifests: &[Manifest]) -> Vec<String> {
    let index = CrateTargetIndex {
        all_paths: repository_paths,
        present: repository_paths.iter().map(String::as_str).collect(),
    };
    let mut targets = BTreeSet::new();
    let suffix = format!("/{CARGO_MANIFEST}");
    for manifest in manifests {
        let base_dir = match manifest.path.as_str() {
            CARGO_MANIFEST => "",
            other => match other.strip_suffix(&suffix) {
                Some(dir) => dir,
                None => continue,
            },
        };
        if let Ok(parsed) = toml::from_str::<CargoTargets>(&manifest.text) {
            parsed.collect_into(&index, base_dir, &mut targets);
        }
    }
    targets.into_iter().collect()
}

struct CrateTargetIndex<'a> {
    all_paths: &'a [String],
    present: HashSet<&'a str>,
}

impl CrateTargetIndex<'_> {
    fn record(&self, base_dir: &str, rel: &str, out: &mut BTreeSet<String>) -> bool {
        let trimmed = rel.trim();
        if trimmed.is_empty()
            || trimmed.starts_with('/')
            || trimmed.contains('\\')
            || trimmed.split('/').any(|segment| segment == "..")
        {
            return false;
        }
        let Some(candidate) = normalize_relative_path(base_dir, trimmed) else {
            return false;
        };
        if !candidate.is_empty() && self.present.contains(candidate.as_str()) {
            out.insert(candidate);
            true
        } else {
            false
        }
    }

    fn scan_auto_bins(&self, base_dir: &str, out: &mut BTreeSet<String>) {
        let prefix = if base_dir.is_empty() {
            "src/bin/".to_owned()
        } else {
            format!("{base_dir}/src/bin/")
        };
        for path in self.all_paths {
            let Some(rest) = path.strip_prefix(&prefix) else {
                continue;
            };
            let is_top_level = !rest.contains('/') && rest.ends_with(".rs");
            let is_subdir_main = rest
                .strip_suffix("/main.rs")
                .is_some_and(|dir| !dir.is_empty() && !dir.contains('/'));
            if is_top_level || is_subdir_main {
                out.insert(path.clone());
            }
        }
    }
}

impl CargoTargets {
    fn collect_into(
        &self,
        index: &CrateTargetIndex<'_>,
        base_dir: &str,
        out: &mut BTreeSet<String>,
    ) {
        let Some(package) = self.package.as_ref() else {
            return;
        };
        if let Some(explicit) = self.lib.as_ref().and_then(|spec| spec.path.as_deref()) {
            index.record(base_dir, explicit, out);
        } else if self.lib.is_some() || package.autolib != Some(false) {
            index.record(base_dir, "src/lib.rs", out);
        }
        for bin in &self.bin {
            if let Some(explicit) = bin.path.as_deref() {
                index.record(base_dir, explicit, out);
            } else if let Some(name) = bin.name.as_deref() {
                record_named_bin(index, base_dir, package.name.as_deref(), name, out);
            }
        }
        if package.autobins != Some(false) {
            index.record(base_dir, "src/main.rs", out);
            index.scan_auto_bins(base_dir, out);
        }
    }
}

fn record_named_bin(
    index: &CrateTargetIndex<'_>,
    base_dir: &str,
    package_name: Option<&str>,
    bin_name: &str,
    out: &mut BTreeSet<String>,
) {
    let bin_name = bin_name.trim();
    if bin_name.is_empty()
        || bin_name.contains('/')
        || bin_name.contains('\\')
        || bin_name == "."
        || bin_name == ".."
    {
        return;
    }
    let direct = format!("src/bin/{bin_name}.rs");
    if index.record(base_dir, &direct, out) {
        return;
    }
    let nested = format!("src/bin/{bin_name}/main.rs");
    if index.record(base_dir, &nested, out) {
        return;
    }
    if package_name == Some(bin_name) {
        index.record(base_dir, "src/main.rs", out);
    }
}
