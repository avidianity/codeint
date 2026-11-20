mod packages;
mod snapshot;

use std::collections::HashSet;

use codeint_model::language::{ImportRequest, Manifest, Resolution};
use oxc_resolver::{ResolveError, ResolveOptions, ResolverGeneric, TsconfigDiscovery};

use crate::resolution::packages::{PACKAGE_MANIFEST, PackageLink};
use crate::resolution::snapshot::Snapshot;

const EXTENSIONS: [&str; 8] = [".ts", ".tsx", ".js", ".jsx", ".mts", ".cts", ".mjs", ".cjs"];

const TYPESCRIPT_SOURCES_OF_JAVASCRIPT_OUTPUT: [(&str, &[&str]); 4] = [
    (".js", &[".js", ".ts", ".tsx"]),
    (".jsx", &[".jsx", ".tsx"]),
    (".mjs", &[".mjs", ".mts"]),
    (".cjs", &[".cjs", ".cts"]),
];

const CONDITIONS: [&str; 5] = ["types", "import", "require", "node", "default"];

pub(crate) fn manifest_paths(repository_paths: &[String]) -> Vec<String> {
    repository_paths
        .iter()
        .filter(|path| is_manifest(file_name(path)))
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
    package_names: HashSet<String>,
}

impl ProjectResolver {
    pub(crate) fn new(repository_paths: &[String], manifests: Vec<Manifest>) -> ProjectResolver {
        let links: Vec<PackageLink> = packages::package_links(&manifests);
        let package_names = links.iter().map(|link| link.name.clone()).collect();
        let file_system = Snapshot::new(repository_paths, manifests, links);
        let options = ResolveOptions {
            cwd: Some(snapshot::root()),
            tsconfig: Some(TsconfigDiscovery::Auto),
            condition_names: CONDITIONS.iter().map(ToString::to_string).collect(),
            extensions: EXTENSIONS.iter().map(ToString::to_string).collect(),
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
            package_names,
        }
    }

    pub(crate) fn resolve(&self, request: &ImportRequest) -> Resolution {
        let specifier = request.specifier.as_str();
        if specifier.starts_with('/') {
            return Resolution::Unresolved(format!(
                "{specifier:?} is an absolute path, which depends on the machine"
            ));
        }
        let importer = snapshot::absolute(&request.importer);
        let resolved = match self.resolver.resolve_file(&importer, specifier) {
            Err(error) if is_project_configuration_error(&error) => {
                importer.parent().map_or(Err(error), |folder| {
                    self.resolver.resolve(folder, specifier)
                })
            }
            result => result,
        };
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
            Err(ResolveError::Builtin { .. }) => Resolution::External,
            Err(_) if self.names_an_outside_module(&importer, specifier) => Resolution::External,
            Err(ResolveError::NotFound(_)) => {
                Resolution::Unresolved(format!("no file of the repository matches {specifier:?}"))
            }
            Err(error) => Resolution::Unresolved(format!("{specifier:?}: {error}")),
        }
    }

    fn names_an_outside_module(&self, importer: &std::path::Path, specifier: &str) -> bool {
        if is_relative(specifier) || specifier.starts_with('#') {
            return false;
        }
        if self.package_names.contains(package_name(specifier)) {
            return false;
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

fn is_project_configuration_error(error: &ResolveError) -> bool {
    matches!(
        error,
        ResolveError::TsconfigNotFound(_)
            | ResolveError::TsconfigSelfReference(_)
            | ResolveError::TsconfigCircularExtend(_)
            | ResolveError::TsconfigLoadFailed { .. }
    )
}

fn is_relative(specifier: &str) -> bool {
    matches!(specifier, "." | "..") || specifier.starts_with("./") || specifier.starts_with("../")
}

fn package_name(specifier: &str) -> &str {
    let segments = if specifier.starts_with('@') { 2 } else { 1 };
    let end = specifier
        .match_indices('/')
        .nth(segments - 1)
        .map_or(specifier.len(), |(index, _)| index);
    &specifier[..end]
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
