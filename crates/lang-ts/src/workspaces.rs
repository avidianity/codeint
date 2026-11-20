use codeint_model::language::{Manifest, WorkspaceError};

pub(crate) const PACKAGE_MANIFEST: &str = "package.json";

pub(crate) fn workspace_packages(_manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
    todo!()
}
