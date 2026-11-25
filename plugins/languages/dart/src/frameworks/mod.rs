mod flutter;

use std::collections::{BTreeSet, HashMap, HashSet};

use codeint_model::language::{
    FrameworkProfileResult, FrameworkProfileSelection, Manifest, Resolution,
};

pub(crate) fn known_profiles() -> Vec<String> {
    vec![flutter::NAME.to_owned()]
}

pub(crate) fn resolve_framework_profiles(
    root: String,
    files: Vec<String>,
    manifests: Vec<Manifest>,
    selection: FrameworkProfileSelection,
) -> Result<FrameworkProfileResult, String> {
    let active = is_flutter_selected(&root, &manifests, selection)?;
    if !active {
        return Ok(codeint_model::empty_framework_profile_result());
    }
    let entry_points: BTreeSet<String> = flutter::entry_points(&root, &files, &manifests)
        .into_iter()
        .collect();
    let generated_paths: BTreeSet<String> = flutter::generated_paths(&root, &files)
        .into_iter()
        .collect();
    Ok(FrameworkProfileResult {
        active_profiles: vec![flutter::NAME.to_owned()],
        entry_points: entry_points.into_iter().collect(),
        generated_paths: generated_paths.into_iter().collect(),
        reference_ranges: Vec::new(),
        reference_range_note: None,
    })
}

fn is_flutter_selected(
    root: &str,
    manifests: &[Manifest],
    selection: FrameworkProfileSelection,
) -> Result<bool, String> {
    match selection {
        FrameworkProfileSelection::Disabled => Ok(false),
        FrameworkProfileSelection::Auto => Ok(flutter::detect(root, manifests)),
        FrameworkProfileSelection::Exclude(excluded) => {
            Ok(!excluded.iter().any(|name| name == flutter::NAME)
                && flutter::detect(root, manifests))
        }
        FrameworkProfileSelection::Only(only) => {
            if let Some(unknown) = only.iter().find(|name| name.as_str() != flutter::NAME) {
                return Err(format!("unknown Framework profile {unknown:?}"));
            }
            Ok(only.iter().any(|name| name == flutter::NAME))
        }
    }
}

pub(crate) fn fallback_package_import(
    active_profiles: &[String],
    files: &HashSet<String>,
    packages: &HashMap<String, String>,
    importer: &str,
    pkg_rest: &str,
) -> Option<Resolution> {
    if !active_profiles.iter().any(|name| name == flutter::NAME) {
        return None;
    }
    flutter::fallback_package_import(files, packages, importer, pkg_rest)
}

pub(crate) fn is_virtual_generated_import(active_profiles: &[String], spec: &str) -> bool {
    active_profiles.iter().any(|name| name == flutter::NAME)
        && flutter::is_virtual_generated_import(spec)
}
