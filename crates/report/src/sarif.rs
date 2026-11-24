use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde::Serialize;

use codeint_metrics::{
    DEV_IN_PRODUCTION_DEPENDENCIES_METRIC_KEY, TEST_GAP_METRIC_KEY,
    UNLISTED_DEPENDENCIES_METRIC_KEY, UNUSED_DEPENDENCIES_METRIC_KEY,
};
use codeint_model::{Metric, SourceSet};

use crate::schema::{
    BUILT_IN_METRICS, BuiltInMetricDescriptor, COGNITIVE_COMPLEXITY_DESCRIPTOR,
    DEV_IN_PRODUCTION_DEPENDENCIES_DESCRIPTOR, HAZARDS_DESCRIPTOR, ORPHAN_FILES_DESCRIPTOR,
    TEST_GAP_DESCRIPTOR, UNLISTED_DEPENDENCIES_DESCRIPTOR, UNUSED_DEPENDENCIES_DESCRIPTOR,
    built_in_metric_descriptor,
};
use crate::{
    BoundaryFindingRow, COGNITIVE_COMPLEXITY_KEY, CloneFamilyFinding, CloneGroupFinding,
    CloneLocation, CognitiveComplexityFinding, ErodedFunctionFinding, ExternalMetricFinding,
    FlaggedLineRow, HazardLineRow, ImportCycleFinding, ImportedDependencyRow, OrphanFileRow,
    Report, SourceSetSection, TestGapFindingRow, UnusedDependencyRow,
};

#[cfg(test)]
mod tests;

const SARIF_SCHEMA: &str =
    "https://docs.oasis-open.org/sarif/sarif/v2.1.0/errata01/os/schemas/sarif-schema-2.1.0.json";
const SARIF_VERSION: &str = "2.1.0";
const SOURCE_ROOT: &str = "SRCROOT";

#[derive(Debug, Serialize)]
pub(crate) struct SarifLog {
    #[serde(rename = "$schema")]
    schema: &'static str,
    version: &'static str,
    runs: [Run; 1],
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Run {
    tool: Tool,
    original_uri_base_ids: OriginalUriBaseIds,
    results: Vec<SarifResult>,
}

#[derive(Debug, Serialize)]
struct Tool {
    driver: Driver,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Driver {
    name: &'static str,
    version: &'static str,
    information_uri: &'static str,
    rules: Vec<Rule>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Rule {
    id: String,
    name: String,
    short_description: Text,
    full_description: Text,
    help: Text,
}

#[derive(Debug, Serialize)]
struct OriginalUriBaseIds {
    #[serde(rename = "SRCROOT")]
    source_root: BaseIdDescription,
}

#[derive(Debug, Serialize)]
struct BaseIdDescription {
    description: Text,
}

#[derive(Debug, Serialize)]
struct Text {
    text: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifResult {
    rule_id: String,
    rule_index: usize,
    level: &'static str,
    message: Text,
    locations: [Location; 1],
    #[serde(skip_serializing_if = "Vec::is_empty")]
    related_locations: Vec<Location>,
    partial_fingerprints: PartialFingerprints,
    properties: ResultProperties,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PartialFingerprints {
    codeint_finding_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResultProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    source_set: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    package: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Location {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<usize>,
    physical_location: PhysicalLocation,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PhysicalLocation {
    artifact_location: ArtifactLocation,
    region: Region,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactLocation {
    uri: String,
    uri_base_id: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Region {
    start_line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_line: Option<u32>,
}

pub(crate) struct ListedExternalMetric<'findings> {
    pub(crate) key: &'findings str,
    pub(crate) name: &'findings str,
    pub(crate) description: &'findings str,
    pub(crate) findings: &'findings [ExternalMetricFinding],
}

pub(crate) struct ListedFindings<'findings> {
    pub(crate) eroded_functions: &'findings [ErodedFunctionFinding],
    pub(crate) cognitive_complexity: &'findings [CognitiveComplexityFinding],
    pub(crate) clone_groups: &'findings [CloneGroupFinding],
    pub(crate) clone_families: &'findings [CloneFamilyFinding],
    pub(crate) clustered_clone_group_ids: &'findings [String],
    pub(crate) flagged_lines: &'findings [FlaggedLineRow],
    pub(crate) import_cycles: &'findings [ImportCycleFinding],
    pub(crate) boundaries: &'findings [BoundaryFindingRow],
    pub(crate) orphan_files: &'findings [OrphanFileRow],
    pub(crate) test_gap: &'findings [TestGapFindingRow],
    pub(crate) hazards: &'findings [HazardLineRow],
    pub(crate) external_metrics: Vec<ListedExternalMetric<'findings>>,
}

pub(crate) struct ListedDependencies<'findings> {
    pub(crate) unused: &'findings [UnusedDependencyRow],
    pub(crate) unlisted: &'findings [ImportedDependencyRow],
    pub(crate) dev_in_production: &'findings [ImportedDependencyRow],
}

impl ListedDependencies<'static> {
    pub(crate) const EMPTY: ListedDependencies<'static> = ListedDependencies {
        unused: &[],
        unlisted: &[],
        dev_in_production: &[],
    };
}

impl SarifLog {
    pub(crate) fn new(report: &Report) -> SarifLog {
        let dependencies = match &report.measured.dependencies {
            Some(deps) => ListedDependencies {
                unused: &deps.unused_dependencies.findings.listed,
                unlisted: &deps.unlisted_dependencies.findings.listed,
                dev_in_production: &deps.dev_in_production_dependencies.findings.listed,
            },
            None => ListedDependencies::EMPTY,
        };
        SarifLog::listing(
            report
                .measured
                .source_sets
                .iter()
                .map(|(source_set, section)| (source_set, ListedFindings::of_section(section))),
            dependencies,
        )
    }

    pub(crate) fn listing<'findings>(
        findings: impl Iterator<Item = (SourceSet, ListedFindings<'findings>)>,
        dependencies: ListedDependencies<'findings>,
    ) -> SarifLog {
        let collected: Vec<(SourceSet, ListedFindings<'findings>)> = findings.collect();
        let mut rules: Vec<Rule> = RULES.map(descriptor).into();
        rules.push(rule_from_descriptor(
            COGNITIVE_COMPLEXITY_KEY,
            COGNITIVE_COMPLEXITY_DESCRIPTOR,
        ));
        let boundaries_rule_index = collected
            .iter()
            .any(|(_, listed)| !listed.boundaries.is_empty())
            .then(|| push_rule(&mut rules, boundaries_descriptor()));
        let orphan_files_rule_index = collected
            .iter()
            .any(|(_, listed)| !listed.orphan_files.is_empty())
            .then(|| push_rule(&mut rules, orphan_files_descriptor()));
        let test_gap_rule_index = collected
            .iter()
            .any(|(_, listed)| !listed.test_gap.is_empty())
            .then(|| {
                push_rule(
                    &mut rules,
                    rule_from_descriptor(TEST_GAP_METRIC_KEY, TEST_GAP_DESCRIPTOR),
                )
            });
        let dependency_rules = register_dependency_rules(&mut rules, &dependencies);
        let hazard_rule_indices = register_hazard_rules(&mut rules, &collected);
        let external_rule_indices = register_external_rules(&mut rules, &collected);
        let optional_rules = OptionalRuleIndices {
            boundaries: boundaries_rule_index,
            orphan_files: orphan_files_rule_index,
            test_gap: test_gap_rule_index,
            hazards: &hazard_rule_indices,
            external: &external_rule_indices,
        };
        let mut all_results: Vec<SarifResult> = collected
            .iter()
            .flat_map(|(source_set, listed)| results(*source_set, listed, &optional_rules))
            .collect();
        all_results.extend(dependency_results(&dependencies, &dependency_rules));
        SarifLog {
            schema: SARIF_SCHEMA,
            version: SARIF_VERSION,
            runs: [Run {
                tool: Tool {
                    driver: Driver {
                        name: "codeint",
                        version: env!("CARGO_PKG_VERSION"),
                        information_uri: "https://github.com/avidianity/codeint",
                        rules,
                    },
                },
                original_uri_base_ids: OriginalUriBaseIds {
                    source_root: BaseIdDescription {
                        description: Text {
                            text: "The root of the Git repository".to_owned(),
                        },
                    },
                },
                results: all_results,
            }],
        }
    }
}

fn push_rule(rules: &mut Vec<Rule>, rule: Rule) -> usize {
    let index = rules.len();
    rules.push(rule);
    index
}

fn rule_from_descriptor(id: &str, info: BuiltInMetricDescriptor) -> Rule {
    Rule {
        id: id.to_owned(),
        name: info.name.to_owned(),
        short_description: Text::from(info.short_description),
        full_description: Text::from(info.full_description),
        help: Text::from(info.help),
    }
}

struct OptionalRuleIndices<'a> {
    boundaries: Option<usize>,
    orphan_files: Option<usize>,
    test_gap: Option<usize>,
    hazards: &'a BTreeMap<String, usize>,
    external: &'a BTreeMap<String, usize>,
}

fn register_hazard_rules(
    rules: &mut Vec<Rule>,
    collected: &[(SourceSet, ListedFindings<'_>)],
) -> BTreeMap<String, usize> {
    let unique_rules: BTreeSet<&str> = collected
        .iter()
        .flat_map(|(_, listed)| listed.hazards.iter().map(|finding| finding.rule.as_str()))
        .collect();
    let mut indices = BTreeMap::new();
    for rule_name in unique_rules {
        let description = format!("Hazard line matched rule {rule_name}.");
        let index = push_rule(
            rules,
            Rule {
                id: rule_name.to_owned(),
                name: rule_name.to_owned(),
                short_description: Text::from(description.as_str()),
                full_description: Text::from(description.as_str()),
                help: Text::from(HAZARDS_DESCRIPTOR.help),
            },
        );
        indices.insert(rule_name.to_owned(), index);
    }
    indices
}

fn register_external_rules(
    rules: &mut Vec<Rule>,
    collected: &[(SourceSet, ListedFindings<'_>)],
) -> BTreeMap<String, usize> {
    let mut external_rules: BTreeMap<String, (&str, &str)> = BTreeMap::new();
    for (_, listed) in collected {
        for external in &listed.external_metrics {
            external_rules
                .entry(external.key.to_owned())
                .or_insert((external.name, external.description));
        }
    }
    let mut indices = BTreeMap::new();
    for (key, (name, description)) in external_rules {
        let index = push_rule(
            rules,
            Rule {
                id: key.clone(),
                name: name.to_owned(),
                short_description: Text::from(description),
                full_description: Text::from(description),
                help: Text::from(description),
            },
        );
        indices.insert(key, index);
    }
    indices
}

struct DependencyRuleIndices {
    unused: Option<usize>,
    unlisted: Option<usize>,
    dev_in_production: Option<usize>,
}

fn register_dependency_rules(
    rules: &mut Vec<Rule>,
    dependencies: &ListedDependencies<'_>,
) -> DependencyRuleIndices {
    DependencyRuleIndices {
        unused: (!dependencies.unused.is_empty()).then(|| {
            push_rule(
                rules,
                rule_from_descriptor(
                    UNUSED_DEPENDENCIES_METRIC_KEY,
                    UNUSED_DEPENDENCIES_DESCRIPTOR,
                ),
            )
        }),
        unlisted: (!dependencies.unlisted.is_empty()).then(|| {
            push_rule(
                rules,
                rule_from_descriptor(
                    UNLISTED_DEPENDENCIES_METRIC_KEY,
                    UNLISTED_DEPENDENCIES_DESCRIPTOR,
                ),
            )
        }),
        dev_in_production: (!dependencies.dev_in_production.is_empty()).then(|| {
            push_rule(
                rules,
                rule_from_descriptor(
                    DEV_IN_PRODUCTION_DEPENDENCIES_METRIC_KEY,
                    DEV_IN_PRODUCTION_DEPENDENCIES_DESCRIPTOR,
                ),
            )
        }),
    }
}

fn dependency_results(
    dependencies: &ListedDependencies<'_>,
    indices: &DependencyRuleIndices,
) -> Vec<SarifResult> {
    let unused = dependencies.unused.iter().map(|finding| SarifResult {
        rule_id: UNUSED_DEPENDENCIES_METRIC_KEY.to_owned(),
        rule_index: indices.unused.unwrap_or_default(),
        level: "note",
        message: Text {
            text: format!(
                "Unused {} dependency {} declared in {} (package {}).",
                finding.kind, finding.dependency, finding.manifest, finding.package
            ),
        },
        locations: [Location::in_repository(&finding.manifest, 1, None)],
        related_locations: Vec::new(),
        partial_fingerprints: PartialFingerprints {
            codeint_finding_id: finding.id.clone(),
        },
        properties: ResultProperties {
            source_set: None,
            package: Some(finding.package.clone()),
        },
    });
    let unlisted = dependencies.unlisted.iter().map(|finding| {
        imported_dependency_sarif_result(
            UNLISTED_DEPENDENCIES_METRIC_KEY,
            indices.unlisted.unwrap_or_default(),
            finding,
            format!(
                "Unlisted dependency {} imported in {} (not declared in {} for package {}).",
                finding.dependency,
                finding.importers.join(", "),
                finding.manifest,
                finding.package
            ),
        )
    });
    let dev_in_production = dependencies.dev_in_production.iter().map(|finding| {
        imported_dependency_sarif_result(
            DEV_IN_PRODUCTION_DEPENDENCIES_METRIC_KEY,
            indices.dev_in_production.unwrap_or_default(),
            finding,
            format!(
                "Dev dependency {} imported in production file {} (declared only as dev in {} for package {}).",
                finding.dependency,
                finding.importers.join(", "),
                finding.manifest,
                finding.package
            ),
        )
    });
    unused.chain(unlisted).chain(dev_in_production).collect()
}

fn imported_dependency_sarif_result(
    rule_id: &str,
    rule_index: usize,
    finding: &ImportedDependencyRow,
    message: String,
) -> SarifResult {
    let (primary, others) = match finding.importers.split_first() {
        Some((first, rest)) => (first.as_str(), rest),
        None => (finding.manifest.as_str(), &[][..]),
    };
    SarifResult {
        rule_id: rule_id.to_owned(),
        rule_index,
        level: "note",
        message: Text { text: message },
        locations: [Location::in_repository(primary, 1, None)],
        related_locations: related(
            others
                .iter()
                .map(|path| Location::in_repository(path, 1, None)),
        ),
        partial_fingerprints: PartialFingerprints {
            codeint_finding_id: finding.id.clone(),
        },
        properties: ResultProperties {
            source_set: None,
            package: Some(finding.package.clone()),
        },
    }
}

const RULES: [Metric; 4] = BUILT_IN_METRICS;
const COGNITIVE_COMPLEXITY_RULE_INDEX: usize = RULES.len();

fn descriptor(metric: Metric) -> Rule {
    let info = built_in_metric_descriptor(metric);
    Rule {
        id: metric.key().to_owned(),
        name: info.name.to_owned(),
        short_description: Text::from(info.short_description),
        full_description: Text::from(info.full_description),
        help: Text::from(info.help),
    }
}

fn metric_result(
    metric: Metric,
    source_set: SourceSet,
    finding_id: &str,
    message: String,
    located_at: Location,
    related_locations: Vec<Location>,
) -> SarifResult {
    SarifResult {
        rule_id: metric.key().to_owned(),
        rule_index: RULES
            .iter()
            .position(|rule| *rule == metric)
            .unwrap_or_default(),
        level: "note",
        message: Text { text: message },
        locations: [located_at],
        related_locations,
        partial_fingerprints: PartialFingerprints {
            codeint_finding_id: finding_id.to_owned(),
        },
        properties: ResultProperties {
            source_set: Some(source_set.key()),
            package: None,
        },
    }
}

impl From<&str> for Text {
    fn from(text: &str) -> Text {
        Text {
            text: text.to_owned(),
        }
    }
}

impl Location {
    fn in_repository(path: &str, start_line: u32, end_line: Option<u32>) -> Location {
        Location {
            id: None,
            physical_location: PhysicalLocation {
                artifact_location: ArtifactLocation {
                    uri: relative_uri(path),
                    uri_base_id: SOURCE_ROOT,
                },
                region: Region {
                    start_line,
                    end_line,
                },
            },
        }
    }

    fn related(self, id: usize) -> Location {
        Location {
            id: Some(id),
            ..self
        }
    }
}

fn boundaries_descriptor() -> Rule {
    Rule {
        id: "boundaries".to_owned(),
        name: "Boundaries".to_owned(),
        short_description: Text::from(
            "An import or file that breaks configured architectural boundaries",
        ),
        full_description: Text::from(
            "Boundaries checks resolved runtime imports against configured architectural zones, directional allow or deny rules, public_files encapsulation, and optional strict_zones coverage.",
        ),
        help: Text::from(
            "Import through an allowed zone or a target zone's public_files entry point, or assign the file to a configured zone.",
        ),
    }
}

fn orphan_files_descriptor() -> Rule {
    Rule {
        id: "orphan_files".to_owned(),
        name: ORPHAN_FILES_DESCRIPTOR.name.to_owned(),
        short_description: Text::from(ORPHAN_FILES_DESCRIPTOR.short_description),
        full_description: Text::from(ORPHAN_FILES_DESCRIPTOR.full_description),
        help: Text::from(ORPHAN_FILES_DESCRIPTOR.help),
    }
}

impl<'findings> ListedFindings<'findings> {
    fn of_section(section: &'findings SourceSetSection) -> ListedFindings<'findings> {
        ListedFindings {
            eroded_functions: &section.erosion.findings.listed,
            cognitive_complexity: &section.cognitive_complexity.findings.listed,
            clone_groups: &section.duplication.clone_groups.listed,
            clone_families: &section.duplication.clone_families.listed,
            clustered_clone_group_ids: &section.duplication.unlisted_clone_family_group_ids,
            flagged_lines: &section.verbosity.findings.listed,
            import_cycles: section.import_cycles.as_ref().map_or(&[], |import_cycles| {
                import_cycles.findings.listed.as_slice()
            }),
            boundaries: section
                .boundaries
                .as_ref()
                .map_or(&[], |boundaries| boundaries.findings.listed.as_slice()),
            orphan_files: section
                .orphan_files
                .as_ref()
                .map_or(&[], |orphans| orphans.findings.listed.as_slice()),
            test_gap: section
                .test_gap
                .as_ref()
                .map_or(&[], |test_gap| test_gap.findings.listed.as_slice()),
            hazards: section
                .hazards
                .as_ref()
                .map_or(&[], |hazards| hazards.findings.listed.as_slice()),
            external_metrics: section
                .external_metrics
                .iter()
                .map(|(key, external)| ListedExternalMetric {
                    key: key.as_str(),
                    name: external.name.as_str(),
                    description: external.description.as_str(),
                    findings: &external.findings.listed,
                })
                .collect(),
        }
    }
}

fn results(
    source_set: SourceSet,
    listed: &ListedFindings<'_>,
    indices: &OptionalRuleIndices<'_>,
) -> Vec<SarifResult> {
    let eroded_functions = listed.eroded_functions.iter().map(|function| {
        metric_result(
            Metric::Erosion,
            source_set,
            &function.id,
            eroded_function_message(function),
            Location::in_repository(&function.path, function.line, None),
            Vec::new(),
        )
    });
    let cognitive_complexity = listed.cognitive_complexity.iter().map(|function| {
        function_metric_result(
            COGNITIVE_COMPLEXITY_KEY,
            COGNITIVE_COMPLEXITY_RULE_INDEX,
            source_set,
            &function.id,
            complex_function_message(function),
            &function.path,
            function.line,
        )
    });
    let clustered_group_ids: HashSet<&str> = listed
        .clone_families
        .iter()
        .flat_map(|family| family.clone_group_ids.iter().map(String::as_str))
        .chain(listed.clustered_clone_group_ids.iter().map(String::as_str))
        .collect();
    let clone_families = listed
        .clone_families
        .iter()
        .filter_map(|family| clone_family_result(source_set, family));
    let clone_groups = listed
        .clone_groups
        .iter()
        .filter(move |group| !clustered_group_ids.contains(group.id.as_str()))
        .filter_map(|group| clone_group_result(source_set, group));
    let flagged_lines = listed.flagged_lines.iter().map(|flagged| {
        metric_result(
            Metric::Verbosity,
            source_set,
            &flagged.id,
            format!("Flagged line: rule {}.", flagged.rule),
            Location::in_repository(&flagged.path, flagged.line, None),
            Vec::new(),
        )
    });
    let import_cycles = listed
        .import_cycles
        .iter()
        .filter_map(|cycle| import_cycle_result(source_set, cycle));
    let boundaries = listed.boundaries.iter().map(|finding| {
        let related_locations = finding
            .target
            .as_deref()
            .map(|target| vec![Location::in_repository(target, 1, None).related(1)])
            .unwrap_or_default();
        SarifResult {
            rule_id: "boundaries".to_owned(),
            rule_index: indices.boundaries.unwrap_or_default(),
            level: "note",
            message: Text {
                text: finding.message.clone(),
            },
            locations: [Location::in_repository(&finding.importer, 1, None)],
            related_locations,
            partial_fingerprints: PartialFingerprints {
                codeint_finding_id: finding.id.clone(),
            },
            properties: ResultProperties {
                source_set: Some(source_set.key()),
                package: None,
            },
        }
    });
    let orphan_files = listed.orphan_files.iter().map(|finding| SarifResult {
        rule_id: "orphan_files".to_owned(),
        rule_index: indices.orphan_files.unwrap_or_default(),
        level: "note",
        message: Text {
            text: finding.message.clone(),
        },
        locations: [Location::in_repository(&finding.path, 1, None)],
        related_locations: related(
            finding
                .test_importers
                .iter()
                .map(|importer| Location::in_repository(importer, 1, None)),
        ),
        partial_fingerprints: PartialFingerprints {
            codeint_finding_id: finding.id.clone(),
        },
        properties: ResultProperties {
            source_set: Some(source_set.key()),
            package: None,
        },
    });
    let test_gap = listed.test_gap.iter().map(|finding| {
        function_metric_result(
            TEST_GAP_METRIC_KEY,
            indices.test_gap.unwrap_or_default(),
            source_set,
            &finding.id,
            test_gap_finding_message(finding),
            &finding.path,
            finding.line,
        )
    });
    let hazards = listed.hazards.iter().map(|finding| {
        let rule_index = indices
            .hazards
            .get(&finding.rule)
            .copied()
            .unwrap_or_default();
        function_metric_result(
            &finding.rule,
            rule_index,
            source_set,
            &finding.id,
            format!("Hazard line: rule {}.", finding.rule),
            &finding.path,
            finding.line,
        )
    });
    let external_findings = listed.external_metrics.iter().flat_map(|external| {
        let rule_index = indices
            .external
            .get(external.key)
            .copied()
            .unwrap_or_default();
        external.findings.iter().map(move |finding| SarifResult {
            rule_id: external.key.to_owned(),
            rule_index,
            level: "note",
            message: Text {
                text: finding.message.clone(),
            },
            locations: [Location::in_repository(
                &finding.path,
                finding.line,
                finding.end_line,
            )],
            related_locations: Vec::new(),
            partial_fingerprints: PartialFingerprints {
                codeint_finding_id: finding.id.clone(),
            },
            properties: ResultProperties {
                source_set: Some(source_set.key()),
                package: None,
            },
        })
    });
    eroded_functions
        .chain(cognitive_complexity)
        .chain(clone_families)
        .chain(clone_groups)
        .chain(flagged_lines)
        .chain(import_cycles)
        .chain(boundaries)
        .chain(orphan_files)
        .chain(test_gap)
        .chain(hazards)
        .chain(external_findings)
        .collect()
}

fn function_metric_result(
    rule_id: &str,
    rule_index: usize,
    source_set: SourceSet,
    finding_id: &str,
    message: String,
    path: &str,
    line: u32,
) -> SarifResult {
    SarifResult {
        rule_id: rule_id.to_owned(),
        rule_index,
        level: "note",
        message: Text { text: message },
        locations: [Location::in_repository(path, line, None)],
        related_locations: Vec::new(),
        partial_fingerprints: PartialFingerprints {
            codeint_finding_id: finding_id.to_owned(),
        },
        properties: ResultProperties {
            source_set: Some(source_set.key()),
            package: None,
        },
    }
}

fn test_gap_finding_message(finding: &TestGapFindingRow) -> String {
    match finding.name.as_deref() {
        Some(name) => format!(
            "Untested function {name}: Function mass {}; untested share {}.",
            finding.function_mass.0, finding.untested_share.0
        ),
        None => format!(
            "Untested function: Function mass {}; untested share {}.",
            finding.function_mass.0, finding.untested_share.0
        ),
    }
}

fn eroded_function_message(function: &ErodedFunctionFinding) -> String {
    match function.name.as_deref() {
        Some(name) => format!(
            "Eroded function {name}: cyclomatic complexity {}, over 10; {} source lines; Function mass {}.",
            function.cyclomatic_complexity, function.source_lines, function.function_mass.0
        ),
        None => format!(
            "Eroded function: cyclomatic complexity {}, over 10; {} source lines; Function mass {}.",
            function.cyclomatic_complexity, function.source_lines, function.function_mass.0
        ),
    }
}

fn complex_function_message(function: &CognitiveComplexityFinding) -> String {
    match function.name.as_deref() {
        Some(name) => format!(
            "Complex function {name}: cognitive complexity {}, over 15; {} source lines.",
            function.cognitive_complexity, function.source_lines
        ),
        None => format!(
            "Complex function: cognitive complexity {}, over 15; {} source lines.",
            function.cognitive_complexity, function.source_lines
        ),
    }
}

fn clone_family_result(source_set: SourceSet, family: &CloneFamilyFinding) -> Option<SarifResult> {
    let group_noun = if family.clone_group_ids.len() == 1 {
        "Clone group"
    } else {
        "Clone groups"
    };
    let header = format!(
        "Clone family of {} duplicated lines across {} locations ({} {group_noun}); the other locations",
        family.duplicated_lines,
        family.locations.len(),
        family.clone_group_ids.len(),
    );
    duplication_result(source_set, &family.id, &family.locations, &header)
}

fn clone_group_result(source_set: SourceSet, group: &CloneGroupFinding) -> Option<SarifResult> {
    let header = match &group.similarity {
        Some(similarity) => format!(
            "Near-clone group of {} tokens ({} similar) in {} fragments; the other fragments",
            group.tokens,
            crate::markdown::format_similarity_percent(similarity.0),
            group.locations.len(),
        ),
        None => format!(
            "Clone group of {} tokens in {} fragments; the other fragments",
            group.tokens,
            group.locations.len(),
        ),
    };
    duplication_result(source_set, &group.id, &group.locations, &header)
}

fn duplication_result(
    source_set: SourceSet,
    finding_id: &str,
    locations: &[CloneLocation],
    header: &str,
) -> Option<SarifResult> {
    let (first, others) = locations.split_first()?;
    let other_ranges: Vec<String> = others
        .iter()
        .map(|location| {
            format!(
                "{} lines {}-{}",
                location.path, location.start_line, location.end_line
            )
        })
        .collect();
    Some(metric_result(
        Metric::Duplication,
        source_set,
        finding_id,
        format!("{header}: {}.", other_ranges.join(", ")),
        fragment_location(first),
        related(others.iter().map(fragment_location)),
    ))
}

fn fragment_location(fragment: &CloneLocation) -> Location {
    Location::in_repository(&fragment.path, fragment.start_line, Some(fragment.end_line))
}

fn import_cycle_result(source_set: SourceSet, cycle: &ImportCycleFinding) -> Option<SarifResult> {
    let (first, others) = cycle.modules.split_first()?;
    let module_location = |module: &String| Location::in_repository(module, 1, None);
    Some(metric_result(
        Metric::ImportCycles,
        source_set,
        &cycle.id,
        format!(
            "Import cycle of {} modules: {}.",
            cycle.modules.len(),
            cycle.modules.join(", ")
        ),
        module_location(first),
        related(others.iter().map(module_location)),
    ))
}

fn related(locations: impl Iterator<Item = Location>) -> Vec<Location> {
    locations
        .zip(1..)
        .map(|(location, id)| location.related(id))
        .collect()
}

fn relative_uri(path: &str) -> String {
    path.bytes()
        .map(|byte| {
            if is_kept_in_a_uri_path(byte) {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

fn is_kept_in_a_uri_path(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=@/".contains(&byte)
}
