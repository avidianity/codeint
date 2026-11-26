use codeint_config::{Codeowners, FindingAnchor};
use codeint_metrics::{
    DevInProductionDependencyFinding, FindingContext, FindingId, UnlistedDependencyFinding,
    UnusedDependencyFinding,
};
use codeint_model::RepoPath;
use serde::{Deserialize, Serialize};

use crate::baseline::id_and_anchor;
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
    #[serde(skip)]
    pub(crate) anchor: Option<FindingAnchor>,
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
    #[serde(skip)]
    pub(crate) anchor: Option<FindingAnchor>,
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
    context: FindingContext,
    package_root: &str,
    manifest: &RepoPath,
    dependency: &str,
    importers: &[RepoPath],
) -> ImportedDependencyRow {
    let (id, anchor) = id_and_anchor(id, context);
    ImportedDependencyRow {
        id,
        package: package_label(package_root),
        manifest: manifest.to_string(),
        dependency: dependency.to_owned(),
        importers: importers.iter().map(ToString::to_string).collect(),
        owners: Vec::new(),
        anchor,
    }
}

impl From<&UnusedDependencyFinding> for UnusedDependencyRow {
    fn from(finding: &UnusedDependencyFinding) -> UnusedDependencyRow {
        let (id, anchor) = id_and_anchor(finding.id, finding.context);
        UnusedDependencyRow {
            id,
            package: package_label(&finding.package_root),
            manifest: finding.manifest.to_string(),
            dependency: finding.dependency.clone(),
            kind: finding.kind.key().to_owned(),
            owners: Vec::new(),
            anchor,
        }
    }
}

impl From<&UnlistedDependencyFinding> for ImportedDependencyRow {
    fn from(finding: &UnlistedDependencyFinding) -> ImportedDependencyRow {
        imported_row(
            finding.id,
            finding.context,
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
            finding.context,
            &finding.package_root,
            &finding.manifest,
            &finding.dependency,
            &finding.importers,
        )
    }
}
