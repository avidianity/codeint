use std::collections::HashSet;

use codeint_config::Codeowners;
use codeint_metrics::{
    BoundaryFinding, CloneFamily, CloneGroup, ComplexFunction, ErodedFunction, FlaggedLineFinding,
    Fragment, HazardLineFinding, ImportCycle, OrphanFileFinding, TestGapFinding, UnresolvedImport,
};
use codeint_model::{ExternalFinding, RepoPath, SkippedFile};
use serde::{Deserialize, Serialize};

use crate::dependencies::{ImportedDependencyRow, UnusedDependencyRow};
use crate::ownership::OwnedFinding;
use crate::render::{Decimal, FindingKind};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RejectedStatementEntry {
    pub path: String,
    pub line: u32,
    pub dialect: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SqlSchemaSummary {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    pub dialect: String,
    pub tables: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HazardLineRow {
    pub id: String,
    pub path: String,
    pub line: u32,
    pub rule: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owners: Vec<String>,
}

crate_structs! {
    #[derive(Debug, Serialize, Deserialize)]
    struct ErodedFunctionFinding {
        id: String,
        path: String,
        line: u32,
        end_line: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        cyclomatic_complexity: u32,
        cognitive_complexity: u32,
        source_lines: u32,
        function_mass: Decimal,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        owners: Vec<String>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct CloneGroupFinding {
        id: String,
        tokens: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        similarity: Option<Decimal>,
        locations: Vec<CloneLocation>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        owners: Vec<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct CloneFamilyFinding {
        id: String,
        duplicated_lines: u64,
        clone_group_ids: Vec<String>,
        locations: Vec<CloneLocation>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        owners: Vec<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct CloneLocation {
        path: String,
        start_line: u32,
        end_line: u32,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct CognitiveComplexityFinding {
        id: String,
        path: String,
        line: u32,
        end_line: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        cognitive_complexity: u32,
        cyclomatic_complexity: u32,
        source_lines: u32,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        owners: Vec<String>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct FlaggedLineRow {
        id: String,
        path: String,
        line: u32,
        rule: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        owners: Vec<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct BoundaryFindingRow {
        id: String,
        kind: String,
        importer: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        external_package: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        specifier: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source_zone: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target_zone: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        message: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        owners: Vec<String>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct ImportCycleFinding {
        id: String,
        modules: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        owners: Vec<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct OrphanFileRow {
        id: String,
        path: String,
        kind: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        test_importers: Vec<String>,
        message: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        owners: Vec<String>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct UnresolvedImportRow {
        importer: String,
        specifier: String,
        reason: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        owners: Vec<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestGapFindingRow {
        id: String,
        path: String,
        line: u32,
        end_line: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        function_mass: Decimal,
        untested_share: Decimal,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        owners: Vec<String>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct SkippedEntry {
        path: String,
        reason: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct ExternalMetricFinding {
        id: String,
        path: String,
        line: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        end_line: Option<u32>,
        message: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        owners: Vec<String>,
    }
}

macro_rules! impl_finding_kind {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl FindingKind for $ty {
                const LIST_KEY: &'static str = "findings";
            }
        )+
    };
}

impl_finding_kind!(
    ErodedFunctionFinding,
    CognitiveComplexityFinding,
    FlaggedLineRow,
    ImportCycleFinding,
    ExternalMetricFinding,
    BoundaryFindingRow,
    UnusedDependencyRow,
    ImportedDependencyRow,
    OrphanFileRow,
    TestGapFindingRow,
    HazardLineRow,
);

impl FindingKind for CloneGroupFinding {
    const LIST_KEY: &'static str = "clone_groups";
}

impl FindingKind for CloneFamilyFinding {
    const LIST_KEY: &'static str = "clone_families";
    const LEFT_OUT_KEY: &'static str = "clone_families_left_out";
    const SKIP_WHEN_EMPTY: bool = true;
}

impl FindingKind for UnresolvedImportRow {
    const LIST_KEY: &'static str = "imports";
    const LEFT_OUT_KEY: &'static str = "imports_left_out";
}

macro_rules! impl_single_path_owner {
    ($ty:ty, $field:ident) => {
        impl OwnedFinding for $ty {
            fn assign_owners(&mut self, codeowners: &Codeowners) {
                self.owners = codeowners.owners_for(&self.$field);
            }

            fn owners(&self) -> &[String] {
                &self.owners
            }
        }
    };
}

macro_rules! impl_multi_path_owner {
    ($ty:ty, |$self:ident| $paths:expr) => {
        impl OwnedFinding for $ty {
            fn assign_owners(&mut $self, codeowners: &Codeowners) {
                $self.owners = codeowners.owners_for_many($paths);
            }

            fn owners(&self) -> &[String] {
                &self.owners
            }
        }
    };
}

impl_single_path_owner!(ErodedFunctionFinding, path);
impl_single_path_owner!(CognitiveComplexityFinding, path);
impl_single_path_owner!(FlaggedLineRow, path);
impl_single_path_owner!(UnresolvedImportRow, importer);
impl_single_path_owner!(ExternalMetricFinding, path);
impl_single_path_owner!(BoundaryFindingRow, importer);
impl_single_path_owner!(OrphanFileRow, path);
impl_single_path_owner!(TestGapFindingRow, path);
impl_single_path_owner!(HazardLineRow, path);

impl_multi_path_owner!(CloneGroupFinding, |self| {
    self.locations.iter().map(|loc| loc.path.as_str())
});
impl_multi_path_owner!(CloneFamilyFinding, |self| {
    self.locations.iter().map(|loc| loc.path.as_str())
});
impl_multi_path_owner!(ImportCycleFinding, |self| {
    self.modules.iter().map(String::as_str)
});

macro_rules! impl_line_rule_row {
    ($from:ty, $to:ident) => {
        impl From<&$from> for $to {
            fn from(finding: &$from) -> $to {
                $to {
                    id: finding.id.to_string(),
                    path: finding.path.to_string(),
                    line: finding.line,
                    rule: finding.rule.clone(),
                    owners: Vec::new(),
                }
            }
        }
    };
}

impl_line_rule_row!(FlaggedLineFinding, FlaggedLineRow);
impl_line_rule_row!(HazardLineFinding, HazardLineRow);

macro_rules! impl_function_finding {
    ($from:ty, $to:ident, |$arg:ident| { $($extra_field:ident: $extra_expr:expr,)* }) => {
        impl From<&$from> for $to {
            fn from($arg: &$from) -> $to {
                $to {
                    id: $arg.id.to_string(),
                    path: $arg.path.to_string(),
                    line: $arg.line,
                    end_line: $arg.end_line,
                    name: $arg.name.clone(),
                    cyclomatic_complexity: $arg.cyclomatic_complexity,
                    cognitive_complexity: $arg.cognitive_complexity,
                    source_lines: $arg.source_lines,
                    $($extra_field: $extra_expr,)*
                    owners: Vec::new(),
                }
            }
        }
    };
}

impl_function_finding!(ErodedFunction, ErodedFunctionFinding, |function| {
    function_mass: Decimal::rounded(function.function_mass),
});
impl_function_finding!(ComplexFunction, CognitiveComplexityFinding, |function| {});

impl From<&CloneGroup> for CloneGroupFinding {
    fn from(group: &CloneGroup) -> CloneGroupFinding {
        CloneGroupFinding {
            id: group.id.to_string(),
            tokens: group.tokens,
            similarity: group.similarity.map(Decimal::rounded),
            locations: group.fragments.iter().map(CloneLocation::new).collect(),
            owners: Vec::new(),
        }
    }
}

impl From<&CloneFamily> for CloneFamilyFinding {
    fn from(family: &CloneFamily) -> CloneFamilyFinding {
        CloneFamilyFinding {
            id: family.id.to_string(),
            duplicated_lines: family.duplicated_lines,
            clone_group_ids: family
                .clone_group_ids
                .iter()
                .map(ToString::to_string)
                .collect(),
            locations: family.fragments.iter().map(CloneLocation::new).collect(),
            owners: Vec::new(),
        }
    }
}

impl CloneLocation {
    fn new(fragment: &Fragment) -> CloneLocation {
        CloneLocation {
            path: fragment.path.to_string(),
            start_line: fragment.start_line,
            end_line: fragment.end_line,
        }
    }
}

pub(crate) fn clustered_clone_group_ids(families: &[CloneFamilyFinding]) -> HashSet<String> {
    families
        .iter()
        .flat_map(|family| family.clone_group_ids.iter().cloned())
        .collect()
}

pub(crate) fn unlisted_clone_family_group_ids(
    listed_groups: &[CloneGroupFinding],
    listed_families: &[CloneFamilyFinding],
    all_clustered: &HashSet<String>,
) -> Vec<String> {
    if all_clustered.is_empty() {
        return Vec::new();
    }
    let listed_clustered: HashSet<&str> = listed_families
        .iter()
        .flat_map(|family| family.clone_group_ids.iter().map(String::as_str))
        .collect();
    listed_groups
        .iter()
        .filter(|group| {
            let id = group.id.as_str();
            all_clustered.contains(id) && !listed_clustered.contains(id)
        })
        .map(|group| group.id.clone())
        .collect()
}

impl From<&TestGapFinding> for TestGapFindingRow {
    fn from(finding: &TestGapFinding) -> TestGapFindingRow {
        TestGapFindingRow {
            id: finding.id.to_string(),
            path: finding.path.to_string(),
            line: finding.line,
            end_line: finding.end_line,
            name: finding.name.clone(),
            function_mass: Decimal::rounded(finding.function_mass),
            untested_share: Decimal::rounded(finding.untested_share),
            owners: Vec::new(),
        }
    }
}

impl From<&ImportCycle> for ImportCycleFinding {
    fn from(cycle: &ImportCycle) -> ImportCycleFinding {
        ImportCycleFinding {
            id: cycle.id().to_string(),
            modules: cycle.modules().iter().map(RepoPath::to_string).collect(),
            owners: Vec::new(),
        }
    }
}

impl From<&BoundaryFinding> for BoundaryFindingRow {
    fn from(finding: &BoundaryFinding) -> BoundaryFindingRow {
        BoundaryFindingRow {
            id: finding.id.to_string(),
            kind: finding.kind.key().to_owned(),
            importer: finding.importer.to_string(),
            target: finding.target.as_ref().map(RepoPath::to_string),
            external_package: finding.external_package.clone(),
            specifier: finding.specifier.clone(),
            source_zone: finding.source_zone.clone(),
            target_zone: finding.target_zone.clone(),
            reason: finding.reason.clone(),
            message: finding.message.clone(),
            owners: Vec::new(),
        }
    }
}

impl From<&SkippedFile> for SkippedEntry {
    fn from(file: &SkippedFile) -> SkippedEntry {
        SkippedEntry {
            path: file.path.to_string(),
            reason: file.reason.clone(),
        }
    }
}

impl From<&OrphanFileFinding> for OrphanFileRow {
    fn from(finding: &OrphanFileFinding) -> OrphanFileRow {
        let test_importers = finding
            .test_importers
            .iter()
            .map(RepoPath::to_string)
            .collect();
        OrphanFileRow {
            id: finding.id.to_string(),
            path: finding.path.to_string(),
            kind: finding.kind.key().to_owned(),
            test_importers,
            message: finding.message.clone(),
            owners: Vec::new(),
        }
    }
}

impl From<&ExternalFinding> for ExternalMetricFinding {
    fn from(finding: &ExternalFinding) -> ExternalMetricFinding {
        ExternalMetricFinding {
            id: finding.id.clone(),
            path: finding.path.to_string(),
            line: finding.line,
            end_line: finding.end_line,
            message: finding.message.clone(),
            owners: Vec::new(),
        }
    }
}

impl From<&UnresolvedImport> for UnresolvedImportRow {
    fn from(import: &UnresolvedImport) -> UnresolvedImportRow {
        UnresolvedImportRow {
            importer: import.importer.to_string(),
            specifier: import.specifier.clone(),
            reason: import.reason.clone(),
            owners: Vec::new(),
        }
    }
}
