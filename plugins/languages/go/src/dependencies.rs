use std::collections::{BTreeMap, BTreeSet, HashSet};

use codeint_model::language::{DeclaredDep, DepKind, Manifest};
use codeint_model::scan::normalize_relative_path;

use crate::go_mod::{GoMod, is_go_mod_path, manifest_dir, parse};
use crate::resolution::strip_module_prefix;

pub(crate) fn extract_declared(manifests: &[Manifest]) -> Vec<DeclaredDep> {
    let parsed: Vec<(String, GoMod)> = manifests
        .iter()
        .filter(|m| is_go_mod_path(&m.path))
        .filter_map(|m| {
            parse(&m.path, &m.text)
                .ok()
                .map(|doc| (manifest_dir(&m.path).to_owned(), doc))
        })
        .collect();
    let local_modules: HashSet<&str> = parsed
        .iter()
        .map(|(_, doc)| doc.module_path.as_str())
        .collect();

    let mut collected = Vec::new();
    for (root, doc) in &parsed {
        collect_go_mod_deps(root, doc, &local_modules, &mut collected);
    }
    collected.sort_by(|a, b| {
        a.package_root
            .cmp(&b.package_root)
            .then_with(|| dep_kind_order(a.kind).cmp(&dep_kind_order(b.kind)))
            .then_with(|| a.name.cmp(&b.name))
    });
    collected
}

fn dep_kind_order(kind: DepKind) -> u8 {
    match kind {
        DepKind::Runtime => 0,
        DepKind::Dev => 1,
        DepKind::Optional => 2,
    }
}

fn collect_go_mod_deps(
    root: &str,
    go_mod: &GoMod,
    local_modules: &HashSet<&str>,
    out: &mut Vec<DeclaredDep>,
) {
    let local_replaces = collect_local_replaces(root, go_mod);
    let mut kinds: BTreeMap<String, DepKind> = BTreeMap::new();
    let mut all_requires = Vec::with_capacity(go_mod.requires.len());

    for req in &go_mod.requires {
        all_requires.push(req.module_path.as_str());
        let kind = if req.indirect {
            DepKind::Optional
        } else {
            DepKind::Runtime
        };
        kinds
            .entry(req.module_path.clone())
            .and_modify(|existing| {
                if dep_kind_order(kind) < dep_kind_order(*existing) {
                    *existing = kind;
                }
            })
            .or_insert(kind);
    }

    for tool_pkg in &go_mod.tools {
        let provider = all_requires
            .iter()
            .copied()
            .filter(|req| strip_module_prefix(tool_pkg, req).is_some())
            .max_by_key(|req| req.len())
            .unwrap_or(tool_pkg.as_str());
        kinds
            .entry(provider.to_owned())
            .and_modify(|existing| {
                if *existing != DepKind::Runtime {
                    *existing = DepKind::Dev;
                }
            })
            .or_insert(DepKind::Dev);
    }

    let excluded_local: BTreeSet<&str> = local_modules
        .iter()
        .copied()
        .chain(local_replaces.iter().map(String::as_str))
        .collect();

    for (name, kind) in kinds {
        if excluded_local
            .iter()
            .any(|local| strip_module_prefix(&name, local).is_some())
        {
            continue;
        }
        out.push(DeclaredDep {
            package_root: root.to_owned(),
            name,
            kind,
        });
    }
}

fn collect_local_replaces(root: &str, go_mod: &GoMod) -> HashSet<String> {
    go_mod
        .replaces
        .iter()
        .filter_map(|rep| {
            let target = rep.new_local_path.as_deref()?;
            if target.starts_with('/') {
                return None;
            }
            normalize_relative_path(root, target)?;
            Some(rep.old_path.clone())
        })
        .collect()
}
