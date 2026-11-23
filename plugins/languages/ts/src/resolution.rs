mod packages;
mod snapshot;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};

use codeint_model::language::{ImportRequest, Manifest, Resolution};
use oxc_resolver::{ResolveError, ResolveOptions, ResolverGeneric, TsconfigDiscovery};

use crate::EXTENSIONS;
use crate::dependencies::{declared_names_by_package, strip_registry_version};
use crate::resolution::packages::PackageLink;
use crate::resolution::snapshot::Snapshot;
use crate::workspaces::{PACKAGE_MANIFEST, PNPM_WORKSPACE_MANIFEST, workspace_packages};

const TYPESCRIPT_SOURCES_OF_JAVASCRIPT_OUTPUT: [(&str, &[&str]); 4] = [
    (".js", &[".js", ".ts", ".tsx"]),
    (".jsx", &[".jsx", ".tsx"]),
    (".mjs", &[".mjs", ".mts"]),
    (".cjs", &[".cjs", ".cts"]),
];

const CONDITIONS: [&str; 6] = ["types", "bun", "import", "require", "node", "default"];

pub(crate) fn manifest_paths(repository_paths: &[String]) -> Vec<String> {
    repository_paths
        .iter()
        .filter(|path| *path == PNPM_WORKSPACE_MANIFEST || is_manifest(file_name(path)))
        .cloned()
        .collect()
}

fn is_manifest(file_name: &str) -> bool {
    file_name == PACKAGE_MANIFEST
        || (file_name.starts_with("tsconfig") && file_name.ends_with(".json"))
}

fn file_name(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

pub(crate) struct ProjectResolver {
    resolver: ResolverGeneric<Snapshot>,
    package_folders: HashMap<String, String>,
    workspace_members: HashSet<String>,
    declared_by_package: HashMap<String, HashSet<String>>,
}

impl ProjectResolver {
    pub(crate) fn new(repository_paths: &[String], manifests: Vec<Manifest>) -> ProjectResolver {
        let workspace_members: HashSet<String> = workspace_packages(&manifests)
            .unwrap_or_default()
            .into_iter()
            .collect();
        let declared_by_package = declared_names_by_package(&manifests);
        let links: Vec<PackageLink> = packages::package_links(&manifests);
        let package_folders = links
            .iter()
            .map(|link| (link.name.clone(), link.folder.clone()))
            .collect();
        let file_system = Snapshot::new(repository_paths, manifests, links);
        let options = ResolveOptions {
            cwd: Some(snapshot::root()),
            tsconfig: Some(TsconfigDiscovery::Auto),
            condition_names: CONDITIONS.iter().map(ToString::to_string).collect(),
            extensions: EXTENSIONS
                .iter()
                .map(|extension| format!(".{extension}"))
                .collect(),
            extension_alias: TYPESCRIPT_SOURCES_OF_JAVASCRIPT_OUTPUT
                .iter()
                .map(|(output, sources)| {
                    (
                        (*output).to_owned(),
                        sources.iter().map(ToString::to_string).collect(),
                    )
                })
                .collect(),
            builtin_modules: true,
            node_path: false,
            ..ResolveOptions::default()
        };
        ProjectResolver {
            resolver: ResolverGeneric::new_with_file_system(file_system, options),
            package_folders,
            workspace_members,
            declared_by_package,
        }
    }

    pub(crate) fn resolve(&self, request: &ImportRequest) -> Resolution {
        let specifier = request.specifier.as_str();
        if let Some(early) = precheck_specifier(&request.importer, specifier) {
            return early;
        }
        let importer = snapshot::absolute(&request.importer);
        let resolved = self.resolve_with_fallback(&importer, specifier);
        self.classify_resolution(&request.importer, &importer, specifier, resolved)
    }

    fn resolve_with_fallback(
        &self,
        importer: &std::path::Path,
        specifier: &str,
    ) -> Result<oxc_resolver::Resolution, ResolveError> {
        match self.resolver.resolve_file(importer, specifier) {
            Err(error) if is_project_configuration_error(&error) => {
                importer.parent().map_or(Err(error), |folder| {
                    self.resolver.resolve(folder, specifier)
                })
            }
            result => result,
        }
    }

    fn classify_resolution(
        &self,
        importer_path: &str,
        importer: &std::path::Path,
        specifier: &str,
        resolved: Result<oxc_resolver::Resolution, ResolveError>,
    ) -> Resolution {
        match resolved {
            Ok(resolution) => snapshot::repository_path(resolution.path()).map_or_else(
                || {
                    Resolution::Unresolved(format!(
                        "{specifier:?} leads to {}, outside the repository",
                        resolution.path().display()
                    ))
                },
                Resolution::Resolved,
            ),
            Err(error) => self.classify_error(importer_path, importer, specifier, error),
        }
    }

    fn classify_error(
        &self,
        importer_path: &str,
        importer: &std::path::Path,
        specifier: &str,
        error: ResolveError,
    ) -> Resolution {
        if matches!(error, ResolveError::Builtin { .. }) {
            return Resolution::Builtin;
        }
        if matches!(error, ResolveError::NotFound(_))
            && !is_relative(specifier)
            && !specifier.starts_with('#')
            && let Some(resolution) = self.resolve_workspace_fallback(specifier)
        {
            return resolution;
        }
        if self.names_an_outside_module(importer, specifier) {
            let pkg = package_name(specifier);
            if self.is_hoisted_root_dependency(importer_path, pkg) {
                return Resolution::Builtin;
            }
            return Resolution::External(pkg.to_owned());
        }
        match error {
            ResolveError::NotFound(_) => {
                Resolution::Unresolved(format!("no file of the repository matches {specifier:?}"))
            }
            other => Resolution::Unresolved(format!("{specifier:?}: {other}")),
        }
    }

    fn is_hoisted_root_dependency(&self, importer_path: &str, pkg: &str) -> bool {
        let Some(root_deps) = self.declared_by_package.get("") else {
            return false;
        };
        if !root_deps.contains(pkg) {
            return false;
        }
        let Some(member_root) = self.package_root_of(importer_path) else {
            return false;
        };
        if member_root.is_empty() || !self.workspace_members.contains(member_root) {
            return false;
        }
        !self
            .declared_by_package
            .get(member_root)
            .is_some_and(|deps| deps.contains(pkg))
    }

    fn package_root_of(&self, path: &str) -> Option<&str> {
        let mut folder = path;
        while let Some((parent, _)) = folder.rsplit_once('/') {
            if let Some((root, _)) = self.declared_by_package.get_key_value(parent) {
                return Some(root.as_str());
            }
            folder = parent;
        }
        self.declared_by_package
            .get_key_value("")
            .map(|(root, _)| root.as_str())
    }

    fn resolve_workspace_fallback(&self, specifier: &str) -> Option<Resolution> {
        let pkg = package_name(specifier);
        let folder = self.package_folders.get(pkg)?;
        let subpath = package_subpath(specifier, pkg);
        if subpath.split('/').any(|segment| segment == "..") {
            return None;
        }
        let pkg_dir = snapshot::absolute(folder);
        let candidates = if subpath.is_empty() {
            vec!["./src/index".to_owned(), "./index".to_owned()]
        } else {
            vec![format!("./src/{subpath}"), format!("./{subpath}")]
        };
        for candidate in candidates {
            if let Ok(resolution) = self.resolver.resolve(&pkg_dir, &candidate)
                && let Some(repo_path) = snapshot::repository_path(resolution.path())
                && is_within_package(folder, &repo_path)
            {
                return Some(Resolution::Resolved(repo_path));
            }
        }
        Some(Resolution::Builtin)
    }

    fn names_an_outside_module(&self, importer: &std::path::Path, specifier: &str) -> bool {
        if is_relative(specifier) || specifier.starts_with('#') {
            return false;
        }
        let name = package_name(specifier);
        if name.is_empty() || self.package_folders.contains_key(name) {
            return false;
        }
        if specifier.starts_with("npm:") || specifier.starts_with("jsr:") {
            return true;
        }
        let matches_path_alias = self
            .resolver
            .find_tsconfig(importer)
            .ok()
            .flatten()
            .and_then(|tsconfig| tsconfig.compiler_options.paths.clone())
            .is_some_and(|paths| paths.keys().any(|key| alias_matches(key, specifier)));
        !matches_path_alias
    }
}

fn precheck_specifier(importer: &str, specifier: &str) -> Option<Resolution> {
    if specifier.is_empty() {
        return Some(Resolution::Unresolved(
            "import specifier is empty".to_owned(),
        ));
    }
    if specifier.starts_with('/') {
        return Some(Resolution::Unresolved(format!(
            "{specifier:?} is an absolute path, which depends on the machine"
        )));
    }
    if is_relative(specifier) && leaves_the_repository(importer, specifier) {
        return Some(Resolution::Unresolved(format!(
            "{specifier:?} leads outside the repository"
        )));
    }
    if is_runtime_builtin(specifier) || is_framework_virtual_module(specifier) {
        return Some(Resolution::Builtin);
    }
    None
}

fn is_project_configuration_error(error: &ResolveError) -> bool {
    matches!(
        error,
        ResolveError::TsconfigNotFound(_)
            | ResolveError::TsconfigSelfReference(_)
            | ResolveError::TsconfigCircularExtend(_)
            | ResolveError::TsconfigLoadFailed { .. }
    )
}

fn is_runtime_builtin(specifier: &str) -> bool {
    specifier == "bun"
        || ["bun:", "node:", "cloudflare:", "deno:"]
            .iter()
            .any(|prefix| {
                specifier
                    .strip_prefix(prefix)
                    .is_some_and(|rest| !rest.is_empty())
            })
}

fn is_framework_virtual_module(specifier: &str) -> bool {
    let clean = specifier
        .split_once(['?', '#'])
        .map_or(specifier, |(head, _)| head);
    matches!(clean, "$app" | "$service-worker")
        || ["virtual:", "$app/", "$env/"].iter().any(|prefix| {
            clean
                .strip_prefix(prefix)
                .is_some_and(|rest| !rest.is_empty())
        })
}

fn is_relative(specifier: &str) -> bool {
    matches!(specifier, "." | "..") || specifier.starts_with("./") || specifier.starts_with("../")
}

fn leaves_the_repository(importer: &str, specifier: &str) -> bool {
    let mut depth = importer.matches('/').count();
    for segment in specifier.split('/') {
        match segment {
            "" | "." => {}
            ".." if depth == 0 => return true,
            ".." => depth -= 1,
            _ => depth += 1,
        }
    }
    false
}

fn is_within_package(folder: &str, repo_path: &str) -> bool {
    folder.is_empty()
        || repo_path
            .strip_prefix(folder)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn package_name(specifier: &str) -> &str {
    let clean = specifier
        .split_once(['?', '#'])
        .map_or(specifier, |(head, _)| head);
    let (body, has_registry_prefix) = clean
        .strip_prefix("npm:")
        .or_else(|| clean.strip_prefix("jsr:"))
        .map_or((clean, false), |rest| (rest, true));
    if body.is_empty() {
        return "";
    }
    let segments = if body.starts_with('@') { 2 } else { 1 };
    let end = body
        .match_indices('/')
        .nth(segments - 1)
        .map_or(body.len(), |(index, _)| index);
    let candidate = &body[..end];
    if has_registry_prefix {
        let stripped = strip_registry_version(candidate);
        if packages::is_package_name(stripped) {
            stripped
        } else {
            ""
        }
    } else {
        candidate
    }
}

fn package_subpath<'a>(specifier: &'a str, pkg: &str) -> &'a str {
    let clean = specifier
        .split_once(['?', '#'])
        .map_or(specifier, |(head, _)| head);
    let body = clean
        .strip_prefix("npm:")
        .or_else(|| clean.strip_prefix("jsr:"))
        .unwrap_or(clean);
    let Some(after_pkg) = body.strip_prefix(pkg) else {
        return "";
    };
    let after_version = if after_pkg.starts_with('@') {
        after_pkg.find('/').map_or("", |slash| &after_pkg[slash..])
    } else {
        after_pkg
    };
    after_version.strip_prefix('/').unwrap_or("")
}

fn alias_matches(key: &str, specifier: &str) -> bool {
    match key.split_once('*') {
        Some((prefix, suffix)) => {
            specifier.len() >= prefix.len() + suffix.len()
                && specifier.starts_with(prefix)
                && specifier.ends_with(suffix)
        }
        None => key == specifier,
    }
}
