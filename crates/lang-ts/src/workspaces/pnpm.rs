use codeint_model::language::{Manifest, WorkspaceError};

pub(crate) const PNPM_WORKSPACE_MANIFEST: &str = "pnpm-workspace.yaml";

pub(crate) type ReadPnpmPackagePatterns = fn(&str) -> Result<Vec<String>, String>;

pub(crate) fn pnpm_package_patterns(_pnpm_workspace_manifest: &str) -> Result<Vec<String>, String> {
    todo!()
}

pub(super) fn pnpm_workspace_packages(
    _manifests: &[Manifest],
    _patterns: &[String],
) -> Result<Vec<String>, WorkspaceError> {
    todo!()
}
