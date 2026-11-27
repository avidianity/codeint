use std::collections::{HashMap, HashSet};

use codeint_model::language::{ImportRequest, Manifest, Resolution};
use codeint_model::scan::normalize_relative_path;

use crate::go_mod::{GoMod, is_go_mod_path, manifest_dir, parse};

struct RepoModule {
    dir: String,
    go_mod: GoMod,
}

pub(crate) fn find_go_mods<P: AsRef<str>>(repository_paths: &[P]) -> Vec<String> {
    let mut paths = Vec::new();
    for path in repository_paths {
        let path = path.as_ref();
        if is_go_mod_path(path) {
            paths.push(path.to_owned());
        }
    }
    paths
}

pub(crate) fn strip_module_prefix<'a>(spec: &'a str, mod_path: &str) -> Option<&'a str> {
    if spec == mod_path {
        return Some("");
    }
    spec.strip_prefix(mod_path)
        .and_then(|rest| rest.strip_prefix('/'))
}

pub(crate) fn resolve_all<P: AsRef<str>>(
    repository_paths: &[P],
    manifests: &[Manifest],
    requests: &[ImportRequest],
    _active_profiles: &[String],
) -> Vec<Resolution> {
    let pkg_files = index_package_files(repository_paths);
    let repo_dirs = index_repo_dirs(repository_paths);
    let modules = parse_repo_modules(manifests);
    requests
        .iter()
        .map(|req| resolve_one(&pkg_files, &repo_dirs, &modules, req))
        .collect()
}

fn index_package_files<P: AsRef<str>>(repository_paths: &[P]) -> HashMap<String, Vec<String>> {
    let mut by_dir: HashMap<String, Vec<String>> = HashMap::new();
    for path in repository_paths {
        let path = path.as_ref();
        if !path.ends_with(".go") || path.ends_with("_test.go") {
            continue;
        }
        let dir = path.rsplit_once('/').map_or("", |(d, _)| d).to_owned();
        by_dir.entry(dir).or_default().push(path.to_owned());
    }
    for files in by_dir.values_mut() {
        files.sort();
        files.dedup();
    }
    by_dir
}

fn index_repo_dirs<P: AsRef<str>>(repository_paths: &[P]) -> HashSet<String> {
    let mut dirs = HashSet::new();
    dirs.insert(String::new());
    for path in repository_paths {
        let mut cursor = path.as_ref();
        while let Some((parent, _)) = cursor.rsplit_once('/') {
            dirs.insert(parent.to_owned());
            cursor = parent;
        }
    }
    dirs
}

fn parse_repo_modules(manifests: &[Manifest]) -> Vec<RepoModule> {
    let mut modules = Vec::new();
    for manifest in manifests {
        if !is_go_mod_path(&manifest.path) {
            continue;
        }
        if let Ok(go_mod) = parse(&manifest.path, &manifest.text) {
            modules.push(RepoModule {
                dir: manifest_dir(&manifest.path).to_owned(),
                go_mod,
            });
        }
    }
    modules
}

fn resolve_one(
    pkg_files: &HashMap<String, Vec<String>>,
    repo_dirs: &HashSet<String>,
    modules: &[RepoModule],
    request: &ImportRequest,
) -> Resolution {
    let spec = request.specifier.as_str();
    if spec.is_empty()
        || spec == "."
        || spec == ".."
        || spec.starts_with("./")
        || spec.starts_with("../")
    {
        return Resolution::Unresolved(format!("relative or empty Go import path {spec:?}"));
    }
    if is_stdlib_or_cgo(spec) {
        return Resolution::Builtin;
    }
    let owner = owning_module(&request.importer, modules);
    if let Some(owner) = owner
        && let Some(resolved) =
            resolve_via_replace(pkg_files, repo_dirs, owner, &request.importer, spec)
    {
        return resolved;
    }
    if let Some(resolved) = resolve_via_repo_modules(pkg_files, modules, &request.importer, spec) {
        return resolved;
    }
    Resolution::External(resolve_external_module_name(owner, spec))
}

fn is_stdlib_or_cgo(spec: &str) -> bool {
    if spec == "C" {
        return true;
    }
    let first = spec.split('/').next().unwrap_or("");
    !first.is_empty() && !first.contains('.')
}

fn owning_module<'a>(importer: &str, modules: &'a [RepoModule]) -> Option<&'a RepoModule> {
    modules
        .iter()
        .filter(|m| path_in_dir(importer, &m.dir))
        .max_by_key(|m| m.dir.len())
}

fn path_in_dir(path: &str, dir: &str) -> bool {
    if dir.is_empty() {
        return true;
    }
    path.strip_prefix(dir)
        .is_some_and(|rest| rest.starts_with('/'))
}

fn resolve_via_replace(
    pkg_files: &HashMap<String, Vec<String>>,
    repo_dirs: &HashSet<String>,
    owner: &RepoModule,
    importer: &str,
    spec: &str,
) -> Option<Resolution> {
    let (_rep, subpkg, target, local_dir) = owner
        .go_mod
        .replaces
        .iter()
        .filter_map(|rep| {
            let subpkg = strip_module_prefix(spec, &rep.old_path)?;
            let target = rep.new_local_path.as_deref()?;
            if target.starts_with('/') {
                return None;
            }
            let local_dir = normalize_relative_path(&owner.dir, target)?;
            Some((rep, subpkg, target, local_dir))
        })
        .max_by_key(|(rep, _, _, _)| rep.old_path.len())?;
    let target_folder = join_dir(&local_dir, subpkg);
    if let Some(targets) = package_targets(pkg_files, &target_folder, importer) {
        return Some(Resolution::Resolved(targets));
    }
    if repo_dirs.contains(&local_dir) || target.starts_with("./") || target.starts_with("../") {
        return Some(Resolution::Unresolved(format!(
            "no Go package files in {target_folder:?} for replaced import {spec:?}"
        )));
    }
    None
}

fn resolve_via_repo_modules(
    pkg_files: &HashMap<String, Vec<String>>,
    modules: &[RepoModule],
    importer: &str,
    spec: &str,
) -> Option<Resolution> {
    let (repo_mod, subpkg) = modules
        .iter()
        .filter_map(|m| strip_module_prefix(spec, &m.go_mod.module_path).map(|sub| (m, sub)))
        .max_by_key(|(m, _)| m.go_mod.module_path.len())?;
    let target_folder = join_dir(&repo_mod.dir, subpkg);
    if let Some(targets) = package_targets(pkg_files, &target_folder, importer) {
        return Some(Resolution::Resolved(targets));
    }
    Some(Resolution::Unresolved(format!(
        "no Go package files in {target_folder:?} for import {spec:?}"
    )))
}

fn package_targets(
    pkg_files: &HashMap<String, Vec<String>>,
    target_folder: &str,
    importer: &str,
) -> Option<Vec<String>> {
    let files = pkg_files.get(target_folder)?;
    let targets: Vec<String> = files
        .iter()
        .filter(|path| path.as_str() != importer)
        .cloned()
        .collect();
    (!targets.is_empty()).then_some(targets)
}

fn join_dir(base: &str, sub: &str) -> String {
    if base.is_empty() {
        sub.to_owned()
    } else if sub.is_empty() {
        base.to_owned()
    } else {
        format!("{base}/{sub}")
    }
}

fn resolve_external_module_name(owner: Option<&RepoModule>, spec: &str) -> String {
    if let Some(owner) = owner {
        if let Some(matched) = owner
            .go_mod
            .requires
            .iter()
            .map(|r| r.module_path.as_str())
            .filter(|candidate| strip_module_prefix(spec, candidate).is_some())
            .max_by_key(|candidate| candidate.len())
        {
            return matched.to_owned();
        }
        if let Some(matched) = owner
            .go_mod
            .tools
            .iter()
            .map(String::as_str)
            .filter(|candidate| strip_module_prefix(spec, candidate).is_some())
            .max_by_key(|candidate| candidate.len())
        {
            return matched.to_owned();
        }
    }
    infer_external_module_path(spec)
}

fn infer_external_module_path(spec: &str) -> String {
    let parts: Vec<&str> = spec.split('/').collect();
    if parts.len() <= 2 {
        return spec.to_owned();
    }
    match parts[0] {
        "github.com" | "gitlab.com" | "bitbucket.org" | "codeberg.org" | "sr.ht" => {
            if parts.len() >= 4 && is_major_version_segment(parts[3]) {
                parts[..4].join("/")
            } else {
                parts[..3].join("/")
            }
        }
        "golang.org" if parts[1] == "x" => parts[..3].join("/"),
        "gopkg.in" => {
            if parts[1].contains(".v") {
                parts[..2].join("/")
            } else {
                parts[..3].join("/")
            }
        }
        _ => spec.to_owned(),
    }
}

fn is_major_version_segment(seg: &str) -> bool {
    let Some(rest) = seg.strip_prefix('v') else {
        return false;
    };
    !rest.is_empty()
        && rest.bytes().all(|b| b.is_ascii_digit())
        && rest.parse::<u32>().is_ok_and(|n| n >= 2)
}
