use std::collections::BTreeSet;

use codeint_model::has_folder_named;
use codeint_model::language::Manifest;

use crate::go_mod::{is_go_mod_path, manifest_dir, parse};
use crate::source_set::{is_test_path, is_vendored_path};

pub(crate) fn discover<P: AsRef<str>>(
    repository_paths: &[P],
    manifests: &[Manifest],
) -> Vec<String> {
    let mut module_roots: Vec<&str> = manifests
        .iter()
        .filter(|m| is_go_mod_path(&m.path) && parse(&m.path, &m.text).is_ok())
        .map(|m| manifest_dir(&m.path))
        .collect();
    module_roots.sort_unstable();
    module_roots.dedup();

    let mut entries = BTreeSet::new();
    for path in repository_paths {
        let path = path.as_ref();
        if !path.ends_with(".go") || is_test_path(path) || is_vendored_path(path) {
            continue;
        }
        let mod_root = owning_module_root(path, &module_roots);
        if !has_internal_segment_from_root(path, mod_root) {
            entries.insert(path.to_owned());
        }
    }
    entries.into_iter().collect()
}

fn owning_module_root<'a>(path: &str, module_roots: &[&'a str]) -> &'a str {
    module_roots
        .iter()
        .copied()
        .filter(|root| {
            root.is_empty()
                || path
                    .strip_prefix(*root)
                    .is_some_and(|rest| rest.starts_with('/'))
        })
        .max_by_key(|root| root.len())
        .unwrap_or("")
}

fn has_internal_segment_from_root(path: &str, mod_root: &str) -> bool {
    let rel = if mod_root.is_empty() {
        path
    } else {
        path.strip_prefix(mod_root)
            .and_then(|rest| rest.strip_prefix('/'))
            .unwrap_or(path)
    };
    has_folder_named(rel, &["internal"])
}
