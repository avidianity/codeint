mod packages;
mod snapshot;

use std::collections::HashSet;

use codeint_model::language::{ImportRequest, Manifest, Resolution};
use oxc_resolver::ResolverGeneric;

use crate::resolution::packages::PackageLink;
use crate::resolution::snapshot::Snapshot;

pub(crate) fn manifest_paths(repository_paths: &[String]) -> Vec<String> {
    todo!("{repository_paths:?}")
}

pub(crate) struct ProjectResolver {
    resolver: ResolverGeneric<Snapshot>,
    package_names: HashSet<String>,
}

impl ProjectResolver {
    pub(crate) fn new(repository_paths: &[String], manifests: Vec<Manifest>) -> ProjectResolver {
        let links: Vec<PackageLink> = packages::package_links(&manifests);
        let file_system = Snapshot::new(repository_paths, manifests, links);
        todo!("{}", file_system.is_empty())
    }

    pub(crate) fn resolve(&self, request: &ImportRequest) -> Resolution {
        todo!(
            "{request:?} {:?} {} {:?} {:?}",
            self.resolver,
            self.package_names.len(),
            snapshot::absolute(&request.importer),
            snapshot::repository_path(std::path::Path::new(&request.specifier))
        )
    }
}
