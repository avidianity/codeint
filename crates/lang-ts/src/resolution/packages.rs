use codeint_model::language::Manifest;

pub(super) struct PackageLink {
    pub(super) name: String,
    pub(super) folder: String,
}

pub(super) fn package_links(manifests: &[Manifest]) -> Vec<PackageLink> {
    todo!("{}", manifests.len())
}
