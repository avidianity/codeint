use codeint_config::Codeowners;
use codeint_metrics::{
    DevInProductionDependencyFinding, FindingId, UnlistedDependencyFinding, UnusedDependencyFinding,
};
use codeint_model::RepoPath;
use serde::{Deserialize, Serialize};

use crate::ownership::OwnedFinding;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UnusedDependencyRow {
    pub(crate) id: String,
    pub(crate) package: String,
    pub(crate) manifest: String,
    pub(crate) dependency: String,
    pub(crate) kind: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) owners: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ImportedDependencyRow {
    pub(crate) id: String,
    pub(crate) package: String,
    pub(crate) manifest: String,
    pub(crate) dependency: String,
    pub(crate) importers: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) owners: Vec<String>,
}

impl OwnedFinding for UnusedDependencyRow {
    fn owners(&self) -> &[String] {
        &self.owners
    }

    fn assign_owners(&mut self, codeowners: &Codeowners) {
        self.owners = codeowners.owners_for(self.manifest.as_str());
    }
}

impl OwnedFinding for ImportedDependencyRow {
    fn owners(&self) -> &[String] {
        &self.owners
    }

    fn assign_owners(&mut self, codeowners: &Codeowners) {
        let paths = self.importers.iter().map(String::as_str);
        self.owners = codeowners.owners_for_many(paths);
    }
}

fn package_label(package_root: &str) -> String {
    if package_root.is_empty() {
        ".".to_owned()
    } else {
        package_root.to_owned()
    }
}

fn imported_row(
    id: FindingId,
    package_root: &str,
    manifest: &RepoPath,
    dependency: &str,
    importers: &[RepoPath],
) -> ImportedDependencyRow {
    ImportedDependencyRow {
        id: id.to_string(),
        package: package_label(package_root),
        manifest: manifest.to_string(),
        dependency: dependency.to_owned(),
        importers: importers.iter().map(ToString::to_string).collect(),
        owners: Vec::new(),
    }
}

impl From<&UnusedDependencyFinding> for UnusedDependencyRow {
    fn from(finding: &UnusedDependencyFinding) -> UnusedDependencyRow {
        UnusedDependencyRow {
            id: finding.id.to_string(),
            package: package_label(&finding.package_root),
            manifest: finding.manifest.to_string(),
            dependency: finding.dependency.clone(),
            kind: finding.kind.key().to_owned(),
            owners: Vec::new(),
        }
    }
}

impl From<&UnlistedDependencyFinding> for ImportedDependencyRow {
    fn from(finding: &UnlistedDependencyFinding) -> ImportedDependencyRow {
        imported_row(
            finding.id,
            &finding.package_root,
            &finding.manifest,
            &finding.dependency,
            &finding.importers,
        )
    }
}

impl From<&DevInProductionDependencyFinding> for ImportedDependencyRow {
    fn from(finding: &DevInProductionDependencyFinding) -> ImportedDependencyRow {
        imported_row(
            finding.id,
            &finding.package_root,
            &finding.manifest,
            &finding.dependency,
            &finding.importers,
        )
    }
}
