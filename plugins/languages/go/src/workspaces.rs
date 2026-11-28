use std::collections::BTreeSet;

use codeint_model::language::{DependencyManifestOwner, Manifest, ManifestAnalysis};

use crate::go_mod::{is_go_mod_path, manifest_dir, parse};

pub(crate) fn analyze_manifests(manifests: &[Manifest]) -> ManifestAnalysis {
    let mut members = BTreeSet::new();
    let mut invalid_manifests = Vec::new();
    let mut dependency_manifests = Vec::new();
    let mut parsed = Vec::new();
    for manifest in manifests {
        if !is_go_mod_path(&manifest.path) {
            continue;
        }
        match parse(&manifest.path, &manifest.text) {
            Ok(doc) => {
                let dir = manifest_dir(&manifest.path);
                if !dir.is_empty() {
                    members.insert(dir.to_owned());
                }
                dependency_manifests.push(DependencyManifestOwner {
                    package_root: dir.to_owned(),
                    path: manifest.path.clone(),
                });
                parsed.push((dir.to_owned(), doc));
            }
            Err(error) => invalid_manifests.push(error),
        }
    }
    let declared_dependencies = crate::dependencies::extract_declared_from_parsed(&parsed);
    ManifestAnalysis {
        workspace_packages: members.into_iter().collect(),
        invalid_manifests,
        dependency_manifests,
        declared_dependencies,
        manifest_used_dependencies: Vec::new(),
        root_dependency_inheritance: Vec::new(),
    }
}
