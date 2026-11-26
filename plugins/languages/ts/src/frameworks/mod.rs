mod angular;
mod nestjs;
mod nextjs;
mod nuxt;
mod react;
mod svelte;
mod sveltekit;
mod vue2;
mod vue3;

use std::collections::BTreeSet;

use codeint_model::language::{
    FrameworkProfileResult, FrameworkProfileSelection, Manifest, ReferenceRange,
};
use codeint_model::scan::build_reference_ranges;
use serde_json::Value;

pub(crate) const KNOWN_PROFILES: [&str; 9] = [
    "react",
    "vue2",
    "vue3",
    "svelte",
    "nextjs",
    "nuxt",
    "sveltekit",
    "angular",
    "nestjs",
];

const PROFILES: [&ProfileDefinition; 9] = [
    &react::PROFILE,
    &vue2::PROFILE,
    &vue3::PROFILE,
    &svelte::PROFILE,
    &nextjs::PROFILE,
    &nuxt::PROFILE,
    &sveltekit::PROFILE,
    &angular::PROFILE,
    &nestjs::PROFILE,
];

pub(crate) struct ProfileDefinition {
    pub(crate) name: &'static str,
    pub(crate) parents: &'static [&'static str],
    pub(crate) detect: fn(&PackageContext<'_>) -> bool,
    pub(crate) entry_points: fn(&PackageContext<'_>) -> Vec<String>,
    pub(crate) generated_paths: fn(&PackageContext<'_>) -> Vec<String>,
    pub(crate) reference_ranges: fn(&PackageContext<'_>) -> Option<Vec<ReferenceRange>>,
    pub(crate) fallback_import: fn(&str) -> Vec<String>,
    pub(crate) is_virtual_module: fn(&str) -> bool,
}

pub(crate) struct PackageContext<'a> {
    package_root: &'a str,
    files: &'a [String],
    package_json: Option<Value>,
}

impl<'a> PackageContext<'a> {
    fn new(package_root: &'a str, files: &'a [String], manifests: &[Manifest]) -> Self {
        let expected_manifest = if package_root.is_empty() {
            "package.json".to_owned()
        } else {
            format!("{package_root}/package.json")
        };
        let package_json = manifests
            .iter()
            .find(|manifest| {
                manifest.path == expected_manifest
                    && !manifest.path.split('/').any(|seg| seg == "node_modules")
            })
            .and_then(|manifest| serde_json::from_str::<Value>(&manifest.text).ok());
        Self {
            package_root,
            files,
            package_json,
        }
    }

    pub(crate) fn rel_path<'p>(&self, full: &'p str) -> Option<&'p str> {
        let rel = if self.package_root.is_empty() {
            full
        } else {
            full.strip_prefix(self.package_root)?.strip_prefix('/')?
        };
        if rel.split('/').any(|segment| segment == "node_modules") {
            return None;
        }
        Some(rel)
    }

    pub(crate) fn matching_files(&self, predicate: impl Fn(&str) -> bool) -> Vec<String> {
        self.files
            .iter()
            .filter(|full| self.rel_path(full).is_some_and(&predicate))
            .cloned()
            .collect()
    }

    pub(crate) fn has_matching_file(&self, predicate: impl Fn(&str) -> bool) -> bool {
        self.files
            .iter()
            .any(|full| self.rel_path(full).is_some_and(&predicate))
    }

    pub(crate) fn has_file(&self, rel: &str) -> bool {
        self.files
            .iter()
            .any(|full| self.rel_path(full) == Some(rel))
    }

    pub(crate) fn has_any_file(&self, candidates: &[&str]) -> bool {
        candidates.iter().any(|candidate| self.has_file(candidate))
    }

    pub(crate) fn has_any_extension(&self, extensions: &[&str]) -> bool {
        self.has_matching_file(|rel| {
            split_file_name(rel).is_some_and(|(_, ext)| extensions.contains(&ext))
        })
    }

    pub(crate) fn dep_spec_in(&self, section: &str, name: &str) -> Option<&str> {
        self.package_json
            .as_ref()?
            .get(section)?
            .as_object()?
            .get(name)?
            .as_str()
    }

    pub(crate) fn has_dep_in(&self, section: &str, name: &str) -> bool {
        self.package_json
            .as_ref()
            .and_then(|doc| doc.get(section))
            .and_then(Value::as_object)
            .is_some_and(|deps| deps.contains_key(name))
    }

    pub(crate) fn has_runtime_dep(&self, name: &str) -> bool {
        self.has_dep_in("dependencies", name)
    }

    pub(crate) fn has_runtime_or_dev_dep(&self, name: &str) -> bool {
        self.has_dep_in("dependencies", name) || self.has_dep_in("devDependencies", name)
    }

    pub(crate) fn is_peer_only_dep(&self, name: &str) -> bool {
        self.has_dep_in("peerDependencies", name) && !self.has_runtime_dep(name)
    }

    pub(crate) fn is_peer_only_dep_any(&self, name: &str) -> bool {
        self.has_dep_in("peerDependencies", name) && !self.has_runtime_or_dev_dep(name)
    }

    pub(crate) fn has_top_level_field(&self, field: &str) -> bool {
        self.package_json
            .as_ref()
            .and_then(Value::as_object)
            .is_some_and(|obj| obj.contains_key(field))
    }
}

pub(crate) fn split_file_name(rel: &str) -> Option<(&str, &str)> {
    let file_name = rel.rsplit_once('/').map_or(rel, |(_, name)| name);
    let (stem, ext) = file_name.rsplit_once('.')?;
    (!stem.is_empty()).then_some((stem, ext))
}

pub(crate) fn is_declaration_file(rel: &str) -> bool {
    rel.ends_with(".d.ts") || rel.ends_with(".d.mts") || rel.ends_with(".d.cts")
}

pub(crate) fn has_code_extension(rel: &str, extensions: &[&str]) -> bool {
    !is_declaration_file(rel)
        && split_file_name(rel).is_some_and(|(_, ext)| extensions.contains(&ext))
}

pub(crate) fn has_any_prefix(rel: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|prefix| rel.starts_with(prefix))
}

pub(crate) fn matches_workspace_subpath(rel: &str, root_dir: &str, tail_files: &[&str]) -> bool {
    let Some(rest) = rel.strip_prefix(root_dir).and_then(|s| s.strip_prefix('/')) else {
        return false;
    };
    let Some((project, inner)) = rest.split_once('/') else {
        return false;
    };
    !project.is_empty()
        && inner
            .strip_prefix("src/")
            .is_some_and(|file| tail_files.contains(&file))
}

pub(crate) fn strip_query_and_fragment(specifier: &str) -> &str {
    let without_query = specifier
        .split_once('?')
        .map_or(specifier, |(head, _)| head);
    if let Some(rest) = without_query.strip_prefix('#') {
        rest.find('#')
            .map_or(without_query, |offset| &without_query[..offset + 1])
    } else {
        without_query
            .split_once('#')
            .map_or(without_query, |(head, _)| head)
    }
}

pub(crate) fn parse_vue_major(spec: &str) -> Option<u8> {
    let bytes = spec.as_bytes();
    for (idx, &byte) in bytes.iter().enumerate() {
        if !matches!(byte, b'2' | b'3') {
            continue;
        }
        let prev_ok = idx == 0 || !bytes[idx - 1].is_ascii_digit();
        let next_ok = bytes
            .get(idx + 1)
            .is_none_or(|&next| !next.is_ascii_digit());
        if prev_ok && next_ok {
            return Some(byte - b'0');
        }
    }
    None
}

pub(crate) fn empty_paths(_ctx: &PackageContext<'_>) -> Vec<String> {
    Vec::new()
}

pub(crate) fn no_reference_ranges(_ctx: &PackageContext<'_>) -> Option<Vec<ReferenceRange>> {
    None
}

pub(crate) fn no_fallback_imports(_specifier: &str) -> Vec<String> {
    Vec::new()
}

pub(crate) fn no_virtual_modules(_specifier: &str) -> bool {
    false
}

pub(crate) fn prefix_alias_fallback(
    specifier: &str,
    prefix: &str,
    target_prefix: &str,
) -> Vec<String> {
    multi_prefix_alias_fallback(specifier, &[prefix], &[target_prefix])
}

pub(crate) fn multi_prefix_alias_fallback(
    specifier: &str,
    prefixes: &[&str],
    target_prefixes: &[&str],
) -> Vec<String> {
    let Some(subpath) = prefixes
        .iter()
        .find_map(|prefix| specifier.strip_prefix(prefix))
    else {
        return Vec::new();
    };
    if subpath.is_empty() || subpath.split('/').any(|segment| segment == "..") {
        return Vec::new();
    }
    target_prefixes
        .iter()
        .map(|target| format!("{target}{subpath}"))
        .collect()
}

pub(crate) fn at_src_fallback(specifier: &str) -> Vec<String> {
    prefix_alias_fallback(specifier, "@/", "./src/")
}

pub(crate) fn svelte_lib_fallback(specifier: &str) -> Vec<String> {
    if specifier == "$lib" {
        return vec!["./src/lib/index".to_owned()];
    }
    prefix_alias_fallback(specifier, "$lib/", "./src/lib/")
}

pub(crate) fn extension_reference_ranges(
    ctx: &PackageContext<'_>,
    extensions: &[&str],
    source: &str,
    metrics: [(&str, u32, f64, f64); 4],
) -> Option<Vec<ReferenceRange>> {
    ctx.has_any_extension(extensions)
        .then(|| build_reference_ranges(source, metrics))
}

pub(crate) fn vue_reference_ranges(
    ctx: &PackageContext<'_>,
    source: &str,
    erosion: (f64, f64),
    verbosity: (f64, f64),
    duplication: (f64, f64),
    import_cycles: (f64, f64),
) -> Option<Vec<ReferenceRange>> {
    extension_reference_ranges(
        ctx,
        &["vue"],
        source,
        [
            ("erosion", 1, erosion.0, erosion.1),
            ("verbosity", 2, verbosity.0, verbosity.1),
            ("duplication", 1, duplication.0, duplication.1),
            ("import_cycles", 1, import_cycles.0, import_cycles.1),
        ],
    )
}

pub(crate) fn vue_entry_points(ctx: &PackageContext<'_>, config_files: &[&str]) -> Vec<String> {
    if ctx.is_peer_only_dep("vue") {
        return Vec::new();
    }
    ctx.matching_files(|rel| {
        matches!(rel, "src/main.js" | "src/main.ts") || config_files.contains(&rel)
    })
}

pub(crate) fn detect_vue_major(
    ctx: &PackageContext<'_>,
    target_major: u8,
    tooling_deps: &[&str],
) -> Option<bool> {
    if ctx.is_peer_only_dep("vue") {
        return Some(false);
    }
    if let Some(major) = ctx
        .dep_spec_in("dependencies", "vue")
        .and_then(parse_vue_major)
    {
        return Some(major == target_major);
    }
    if ctx.has_runtime_dep("vue")
        && tooling_deps
            .iter()
            .any(|dep| ctx.has_runtime_or_dev_dep(dep))
    {
        return Some(true);
    }
    None
}

pub(crate) fn known_profiles() -> Vec<String> {
    KNOWN_PROFILES.iter().map(ToString::to_string).collect()
}

pub(crate) fn resolve_framework_profiles(
    package_root: String,
    files: Vec<String>,
    manifests: Vec<Manifest>,
    selection: FrameworkProfileSelection,
) -> Result<FrameworkProfileResult, String> {
    let ctx = PackageContext::new(&package_root, &files, &manifests);
    let active_defs = select_active_profiles(&ctx, selection)?;
    let mut active_profiles: Vec<String> = active_defs
        .iter()
        .map(|profile| profile.name.to_owned())
        .collect();
    active_profiles.sort();

    let mut entry_points = BTreeSet::new();
    let mut generated_paths = BTreeSet::new();
    for profile in &active_defs {
        entry_points.extend((profile.entry_points)(&ctx));
        generated_paths.extend((profile.generated_paths)(&ctx));
    }

    let (reference_ranges, reference_range_note) = select_reference_ranges(&ctx, &active_defs);

    Ok(FrameworkProfileResult {
        active_profiles,
        entry_points: entry_points.into_iter().collect(),
        generated_paths: generated_paths.into_iter().collect(),
        reference_ranges,
        reference_range_note,
    })
}

fn select_active_profiles(
    ctx: &PackageContext<'_>,
    selection: FrameworkProfileSelection,
) -> Result<Vec<&'static ProfileDefinition>, String> {
    match selection {
        FrameworkProfileSelection::Disabled => Ok(Vec::new()),
        FrameworkProfileSelection::Auto => Ok(PROFILES
            .iter()
            .copied()
            .filter(|profile| (profile.detect)(ctx))
            .collect()),
        FrameworkProfileSelection::Exclude(excluded) => Ok(PROFILES
            .iter()
            .copied()
            .filter(|profile| {
                !excluded.iter().any(|name| name == profile.name) && (profile.detect)(ctx)
            })
            .collect()),
        FrameworkProfileSelection::Only(only) => {
            for name in &only {
                if !KNOWN_PROFILES.contains(&name.as_str()) {
                    return Err(format!("unknown Framework profile {name:?}"));
                }
            }
            Ok(PROFILES
                .iter()
                .copied()
                .filter(|profile| only.iter().any(|name| name == profile.name))
                .collect())
        }
    }
}

fn select_reference_ranges(
    ctx: &PackageContext<'_>,
    active_defs: &[&'static ProfileDefinition],
) -> (Vec<ReferenceRange>, Option<String>) {
    let candidates: Vec<(&'static ProfileDefinition, Vec<ReferenceRange>)> = active_defs
        .iter()
        .copied()
        .filter_map(|profile| {
            let ranges = (profile.reference_ranges)(ctx)?;
            (!ranges.is_empty()).then_some((profile, ranges))
        })
        .collect();

    let overridden_parents: BTreeSet<&str> = candidates
        .iter()
        .flat_map(|(profile, _)| profile.parents.iter().copied())
        .collect();
    let remaining: Vec<(&'static ProfileDefinition, Vec<ReferenceRange>)> = candidates
        .into_iter()
        .filter(|(profile, _)| !overridden_parents.contains(profile.name))
        .collect();

    match remaining.as_slice() {
        [] => (Vec::new(), None),
        [(only, ranges)] => {
            if only.name == "react" && ctx.has_any_extension(&["vue"]) {
                (
                    Vec::new(),
                    Some(
                        "mixed .vue and JSX/TSX files detected without a resolved Vue Framework profile; falling back to base typescript-javascript Reference ranges"
                            .to_owned(),
                    ),
                )
            } else {
                (ranges.clone(), None)
            }
        }
        many => {
            let mut names: Vec<&str> = many.iter().map(|(profile, _)| profile.name).collect();
            names.sort_unstable();
            (
                Vec::new(),
                Some(format!(
                    "multiple Framework profiles supply Reference ranges ({}); falling back to base typescript-javascript Reference ranges",
                    names.join(", ")
                )),
            )
        }
    }
}

pub(crate) fn fallback_import_candidates(
    active_profiles: &[String],
    specifier: &str,
) -> Vec<String> {
    let mut candidates = Vec::new();
    collect_profile_candidates(active_profiles, specifier, true, &mut candidates);
    collect_profile_candidates(active_profiles, specifier, false, &mut candidates);
    candidates
}

fn collect_profile_candidates(
    active_profiles: &[String],
    specifier: &str,
    children_only: bool,
    candidates: &mut Vec<String>,
) {
    let matching = PROFILES.iter().copied().filter(|profile| {
        (!profile.parents.is_empty() == children_only)
            && active_profiles.iter().any(|active| active == profile.name)
    });
    for candidate in matching.flat_map(|profile| (profile.fallback_import)(specifier)) {
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    }
}

pub(crate) fn is_profile_virtual_module(active_profiles: &[String], specifier: &str) -> bool {
    if active_profiles.is_empty() {
        return false;
    }
    let clean = strip_query_and_fragment(specifier);
    PROFILES.iter().any(|profile| {
        active_profiles.iter().any(|active| active == profile.name)
            && (profile.is_virtual_module)(clean)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn child_ranges(_: &PackageContext<'_>) -> Option<Vec<ReferenceRange>> {
        Some(vec![ReferenceRange {
            metric: "erosion".to_owned(),
            metric_version: 1,
            low: 0.01,
            high: 0.2,
            source: "synthetic child profile".to_owned(),
        }])
    }

    fn always_detect(_: &PackageContext<'_>) -> bool {
        true
    }

    static SYNTHETIC_CHILD: ProfileDefinition = ProfileDefinition {
        name: "synthetic-next",
        parents: &["react"],
        detect: always_detect,
        entry_points: empty_paths,
        generated_paths: empty_paths,
        fallback_import: no_fallback_imports,
        is_virtual_module: no_virtual_modules,
        reference_ranges: child_ranges,
    };

    #[test]
    fn child_profile_overrides_parent_reference_ranges_in_any_order() {
        let files = vec!["package.json".to_owned(), "src/App.tsx".to_owned()];
        let manifests = vec![Manifest {
            path: "package.json".to_owned(),
            text: r#"{"dependencies":{"react":"^18.3.1","react-dom":"^18.3.1"}}"#.to_owned(),
        }];
        let ctx = PackageContext::new("", &files, &manifests);

        for active in [
            [&react::PROFILE, &SYNTHETIC_CHILD],
            [&SYNTHETIC_CHILD, &react::PROFILE],
        ] {
            let (ranges, note) = select_reference_ranges(&ctx, &active);
            assert!(note.is_none(), "expected no fallback note, got {note:?}");
            assert_eq!(ranges.len(), 1);
            assert_eq!(ranges[0].source, "synthetic child profile");
        }
    }

    #[test]
    fn child_without_reference_ranges_inherits_parent_reference_ranges() {
        let files = vec!["package.json".to_owned(), "src/app/page.tsx".to_owned()];
        let manifests = vec![Manifest {
            path: "package.json".to_owned(),
            text: r#"{"dependencies":{"next":"^15.0.0","react":"^19.0.0","react-dom":"^19.0.0"}}"#
                .to_owned(),
        }];
        let ctx = PackageContext::new("", &files, &manifests);
        let (ranges, note) = select_reference_ranges(&ctx, &[&react::PROFILE, &nextjs::PROFILE]);
        assert!(note.is_none(), "expected no fallback note, got {note:?}");
        assert!(ranges.iter().any(|range| {
            range.source.contains("20 React repositories") && range.source.contains("ADR 0028")
        }));
    }

    #[test]
    fn strip_query_and_fragment_handles_multibyte_utf8_and_leading_hash_safely() {
        assert_eq!(strip_query_and_fragment("über"), "über");
        assert_eq!(strip_query_and_fragment("日本語#section?v=1"), "日本語");
        assert_eq!(strip_query_and_fragment("#imports?raw"), "#imports");
        assert_eq!(strip_query_and_fragment("#imports#frag?raw"), "#imports");
        assert_eq!(strip_query_and_fragment("#"), "#");
    }
}
