use std::collections::BTreeSet;

use codeint_model::language::{Manifest, WorkspaceError};

use crate::go_mod::{is_go_mod_path, manifest_dir, parse};

pub(crate) fn discover(manifests: &[Manifest]) -> Result<Vec<String>, WorkspaceError> {
    let mut members = BTreeSet::new();
    for manifest in manifests {
        if !is_go_mod_path(&manifest.path) {
            continue;
        }
        parse(&manifest.path, &manifest.text)?;
        let dir = manifest_dir(&manifest.path);
        if !dir.is_empty() {
            members.insert(dir.to_owned());
        }
    }
    Ok(members.into_iter().collect())
}
