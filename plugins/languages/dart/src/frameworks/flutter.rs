use std::collections::{BTreeMap, HashMap, HashSet};

use codeint_model::frameworks::{
    find_package_manifest, matching_package_files, strip_package_root,
};
use codeint_model::language::{Manifest, Resolution};
use serde::Deserialize;
use serde::de::IgnoredAny;

use crate::dependencies::normalize_pub_name;
use crate::workspaces::PUBSPEC_MANIFEST;

pub(super) const NAME: &str = "flutter";

const GENERATED_SUFFIXES: [&str; 9] = [
    ".g.dart",
    ".freezed.dart",
    ".gen.dart",
    ".gr.dart",
    ".mocks.dart",
    ".pb.dart",
    ".pbenum.dart",
    ".pbjson.dart",
    ".pbserver.dart",
];

const VIRTUAL_GENERATED_SUFFIXES: [&str; 4] = [".g.dart", ".freezed.dart", ".gen.dart", ".gr.dart"];

#[derive(Deserialize)]
#[serde(untagged)]
enum DependencyEntry {
    Sdk { sdk: Option<String> },
    Other(IgnoredAny),
}

#[derive(Deserialize)]
struct FlutterSection {
    plugin: Option<IgnoredAny>,
}

#[derive(Deserialize)]
struct FlutterPubspec {
    name: Option<String>,
    dependencies: Option<BTreeMap<String, Option<DependencyEntry>>>,
    flutter: Option<FlutterSection>,
}

impl FlutterPubspec {
    fn has_flutter_sdk_dep(&self) -> bool {
        let Some(deps) = self.dependencies.as_ref() else {
            return false;
        };
        deps.iter().any(|(key, value)| {
            normalize_pub_name(key) == "flutter"
                && matches!(
                    value,
                    Some(DependencyEntry::Sdk { sdk: Some(sdk) })
                        if sdk.trim().eq_ignore_ascii_case("flutter")
                )
        })
    }

    fn is_flutter_package(&self) -> bool {
        self.has_flutter_sdk_dep() || self.flutter.is_some()
    }

    fn is_plugin(&self) -> bool {
        self.flutter
            .as_ref()
            .is_some_and(|section| section.plugin.is_some())
    }

    fn package_name(&self) -> Option<String> {
        let raw = self.name.as_deref()?;
        let normalized = normalize_pub_name(raw);
        (!normalized.is_empty()).then_some(normalized)
    }
}

pub(super) fn detect(package_root: &str, manifests: &[Manifest]) -> bool {
    parse_local_pubspec(package_root, manifests).is_some_and(|pubspec| pubspec.is_flutter_package())
}

pub(super) fn entry_points<P: AsRef<str>>(
    package_root: &str,
    files: &[P],
    manifests: &[Manifest],
) -> Vec<String> {
    let pubspec = parse_local_pubspec(package_root, manifests);
    let is_plugin = pubspec.as_ref().is_some_and(FlutterPubspec::is_plugin);
    let has_app_main = files
        .iter()
        .filter_map(|full| rel_path(package_root, full.as_ref()))
        .any(is_app_main_rel);
    let is_app = !is_plugin && has_app_main;
    let package_entry = pubspec
        .as_ref()
        .and_then(FlutterPubspec::package_name)
        .map(|name| format!("lib/{name}.dart"));

    files
        .iter()
        .map(AsRef::as_ref)
        .filter(|full| {
            let Some(rel) = rel_path(package_root, full) else {
                return false;
            };
            if !rel.ends_with(".dart") {
                return false;
            }
            if is_integration_test_rel(rel) {
                return true;
            }
            if is_app {
                is_app_main_rel(rel)
            } else {
                package_entry.as_deref() == Some(rel)
            }
        })
        .map(str::to_owned)
        .collect()
}

pub(super) fn generated_paths<P: AsRef<str>>(package_root: &str, files: &[P]) -> Vec<String> {
    matching_package_files(package_root, files, &[], |rel| {
        rel.ends_with(".dart") && is_generated_rel(rel)
    })
}

pub(super) fn fallback_package_import(
    files: &HashSet<String>,
    packages: &HashMap<String, String>,
    importer: &str,
    pkg_rest: &str,
) -> Option<Resolution> {
    let (raw_pkg, rel_opt) = match pkg_rest.split_once('/') {
        Some((pkg, rel)) => (pkg, Some(rel)),
        None => (pkg_rest, None),
    };
    if normalize_pub_name(raw_pkg) != "flutter_gen" {
        return None;
    }
    let rel_path = rel_opt.filter(|rel| !rel.is_empty())?;
    let pkg_root = enclosing_package_root(packages, importer);
    let candidate = if pkg_root.is_empty() {
        format!(".dart_tool/flutter_gen/{rel_path}")
    } else {
        format!("{pkg_root}/.dart_tool/flutter_gen/{rel_path}")
    };
    if files.contains(&candidate) {
        return Some(Resolution::Resolved(vec![candidate]));
    }
    let root_candidate = format!(".dart_tool/flutter_gen/{rel_path}");
    if files.contains(&root_candidate) {
        return Some(Resolution::Resolved(vec![root_candidate]));
    }
    Some(Resolution::Builtin)
}

pub(super) fn is_virtual_generated_import(spec: &str) -> bool {
    !spec.starts_with('/')
        && VIRTUAL_GENERATED_SUFFIXES
            .iter()
            .any(|suffix| spec.ends_with(suffix))
}

const IGNORED_MANIFEST_DIRS: [&str; 3] = ["vendor", "third_party", ".dart_tool"];

fn parse_local_pubspec(package_root: &str, manifests: &[Manifest]) -> Option<FlutterPubspec> {
    let manifest = find_package_manifest(
        package_root,
        PUBSPEC_MANIFEST,
        manifests,
        &IGNORED_MANIFEST_DIRS,
    )?;
    serde_saphyr::from_str::<Option<FlutterPubspec>>(&manifest.text)
        .ok()
        .flatten()
}

fn rel_path<'p>(package_root: &str, full: &'p str) -> Option<&'p str> {
    strip_package_root(package_root, full)
}

fn is_app_main_rel(rel: &str) -> bool {
    let Some(rest) = rel.strip_prefix("lib/") else {
        return false;
    };
    rest == "main.dart"
        || (rest.starts_with("main_") && !rest.contains('/') && rest.ends_with(".dart"))
}

fn is_integration_test_rel(rel: &str) -> bool {
    rel.starts_with("integration_test/")
}

fn is_generated_rel(rel: &str) -> bool {
    if rel.starts_with(".dart_tool/")
        || rel.starts_with("build/")
        || rel.starts_with(".widget_preview/")
        || rel == "lib/generated_plugin_registrant.dart"
    {
        return true;
    }
    if let Some(name) = rel.strip_prefix("lib/l10n/")
        && !name.contains('/')
        && name.starts_with("app_localizations")
        && name.ends_with(".dart")
    {
        return true;
    }
    GENERATED_SUFFIXES
        .iter()
        .any(|suffix| rel.ends_with(suffix))
}

fn enclosing_package_root<'a>(packages: &'a HashMap<String, String>, importer: &str) -> &'a str {
    packages
        .values()
        .filter(|dir| {
            dir.is_empty()
                || importer
                    .strip_prefix(dir.as_str())
                    .is_some_and(|rest| rest.starts_with('/'))
        })
        .max_by_key(|dir| dir.len())
        .map(String::as_str)
        .unwrap_or("")
}
