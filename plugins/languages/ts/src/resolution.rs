mod packages;
mod snapshot;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use codeint_model::language::{ImportRequest, Manifest, Resolution};
use oxc_resolver::{ResolveError, ResolveOptions, ResolverGeneric, TsconfigDiscovery};

use crate::EXTENSIONS;
use crate::resolution::packages::PackageLink;
use crate::resolution::snapshot::Snapshot;
use crate::workspaces::{PACKAGE_MANIFEST, PNPM_WORKSPACE_MANIFEST};

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
}

impl ProjectResolver {
    pub(crate) fn new(repository_paths: &[String], manifests: Vec<Manifest>) -> ProjectResolver {
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
        }
    }

    pub(crate) fn resolve(&self, request: &ImportRequest) -> Resolution {
        let specifier = request.specifier.as_str();
        if let Some(early) = precheck_specifier(&request.importer, specifier) {
            return early;
        }
        let importer = snapshot::absolute(&request.importer);
        let resolved = self.resolve_with_fallback(&importer, specifier);
        self.classify_resolution(&importer, specifier, resolved)
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
                |path| Resolution::Resolved(vec![path]),
            ),
            Err(error) => self.classify_error(importer, specifier, error),
        }
    }

    fn classify_error(
        &self,
        importer: &std::path::Path,
        specifier: &str,
        error: ResolveError,
    ) -> Resolution {
        if matches!(error, ResolveError::Builtin { .. }) || is_framework_virtual_module(specifier) {
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
            return Resolution::External(package_name(specifier).to_owned());
        }
        match error {
            ResolveError::NotFound(_) => {
                Resolution::Unresolved(format!("no file of the repository matches {specifier:?}"))
            }
            other => Resolution::Unresolved(format!("{specifier:?}: {other}")),
        }
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
                return Some(Resolution::Resolved(vec![repo_path]));
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
    if is_runtime_builtin(specifier) {
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

fn strip_registry_version(candidate: &str) -> &str {
    let search_start = if candidate.starts_with('@') {
        match candidate.find('/') {
            Some(slash) => slash + 1,
            None => return candidate,
        }
    } else {
        0
    };
    match candidate[search_start..].find('@') {
        Some(0) => "",
        Some(offset) => &candidate[..search_start + offset],
        None => candidate,
    }
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
