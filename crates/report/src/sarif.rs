use std::collections::BTreeMap;

use serde::Serialize;

use codeint_metrics::{
    DEV_IN_PRODUCTION_DEPENDENCIES_METRIC_KEY, UNLISTED_DEPENDENCIES_METRIC_KEY,
    UNUSED_DEPENDENCIES_METRIC_KEY,
};
use codeint_model::{Metric, SourceSet};

use crate::{
    BoundaryFindingRow, COGNITIVE_COMPLEXITY_KEY, CloneGroupFinding, CloneLocation,
    CognitiveComplexityFinding, ErodedFunctionFinding, ExternalMetricFinding, FlaggedLineRow,
    ImportCycleFinding, ImportedDependencyRow, Report, SourceSetSection, UnusedDependencyRow,
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
    pub(crate) flagged_lines: &'findings [FlaggedLineRow],
    pub(crate) import_cycles: &'findings [ImportCycleFinding],
    pub(crate) boundaries: &'findings [BoundaryFindingRow],
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
        rules.push(cognitive_complexity_descriptor());
        let boundaries_rule_index = collected
            .iter()
            .any(|(_, listed)| !listed.boundaries.is_empty())
            .then(|| push_rule(&mut rules, boundaries_descriptor()));
        let dependency_rules = register_dependency_rules(&mut rules, &dependencies);
        let external_rule_indices = register_external_rules(&mut rules, &collected);
        let mut all_results: Vec<SarifResult> = collected
            .iter()
            .flat_map(|(source_set, listed)| {
                results(
                    *source_set,
                    listed,
                    boundaries_rule_index,
                    &external_rule_indices,
                )
            })
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
        unused: (!dependencies.unused.is_empty())
            .then(|| push_rule(rules, unused_dependencies_descriptor())),
        unlisted: (!dependencies.unlisted.is_empty())
            .then(|| push_rule(rules, unlisted_dependencies_descriptor())),
        dev_in_production: (!dependencies.dev_in_production.is_empty())
            .then(|| push_rule(rules, dev_in_production_descriptor())),
    }
}

fn unused_dependencies_descriptor() -> Rule {
    Rule {
        id: UNUSED_DEPENDENCIES_METRIC_KEY.to_owned(),
        name: "Unused dependencies".to_owned(),
        short_description: Text::from("A declared dependency that no file in the package imports"),
        full_description: Text::from(
            "Unused dependencies is the share of declared runtime and dev dependencies in package manifests that are not imported by any source file in the owning package.",
        ),
        help: Text::from(
            "Remove the unused dependency from the package manifest, or add it to dependencies.ignore in codeint.json if it is used outside source imports.",
        ),
    }
}

fn unlisted_dependencies_descriptor() -> Rule {
    Rule {
        id: UNLISTED_DEPENDENCIES_METRIC_KEY.to_owned(),
        name: "Unlisted dependencies".to_owned(),
        short_description: Text::from(
            "An external package imported without being declared in the package manifest",
        ),
        full_description: Text::from(
            "Unlisted dependencies is the share of imported external packages that are missing from the owning package manifest.",
        ),
        help: Text::from(
            "Declare the package in the owning package manifest, or remove the import.",
        ),
    }
}

fn dev_in_production_descriptor() -> Rule {
    Rule {
        id: DEV_IN_PRODUCTION_DEPENDENCIES_METRIC_KEY.to_owned(),
        name: "Dev dependencies in production".to_owned(),
        short_description: Text::from("A dev-only dependency imported by a Production source file"),
        full_description: Text::from(
            "Dev dependencies in production is the share of production-imported external packages that are declared only as dev dependencies in the package manifest.",
        ),
        help: Text::from(
            "Move the dependency to runtime dependencies in the package manifest, or stop importing it from Production files.",
        ),
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

const RULES: [Metric; 4] = [
    Metric::Erosion,
    Metric::Duplication,
    Metric::Verbosity,
    Metric::ImportCycles,
];
const COGNITIVE_COMPLEXITY_RULE_INDEX: usize = RULES.len();

fn cognitive_complexity_descriptor() -> Rule {
    Rule {
        id: COGNITIVE_COMPLEXITY_KEY.to_owned(),
        name: "Cognitive complexity".to_owned(),
        short_description: Text::from("A function with cognitive complexity over 15"),
        full_description: Text::from(
            "Cognitive complexity is the share of total cognitive complexity held by functions whose cognitive complexity is greater than 15. Cognitive complexity weights control-flow structures by nesting depth.",
        ),
        help: Text::from(
            "Flatten nested control flow or split the function into smaller functions, each with a cognitive complexity of 15 or less.",
        ),
    }
}

fn descriptor(metric: Metric) -> Rule {
    let (name, short_description, full_description, help) = match metric {
        Metric::Erosion => (
            "Erosion",
            "A function with cyclomatic complexity over 10",
            "Erosion is the share of total Function mass held by functions whose cyclomatic complexity is greater than 10. Function mass is cyclomatic complexity times the square root of source lines.",
            "Split the function into smaller functions, each with a cyclomatic complexity of 10 or less.",
        ),
        Metric::Duplication => (
            "Duplication",
            "A Clone group: code fragments with the same token sequence",
            "Duplication is the share of source lines that belong to at least one Clone group. A Clone group is two or more code fragments with the same token sequence once identifiers and literals are normalized.",
            "Extract the fragments of the Clone group into one shared function or module.",
        ),
        Metric::Verbosity => (
            "Verbosity",
            "A Flagged line: code that can be deleted or collapsed",
            "Verbosity is the share of source lines that are Flagged lines, clone lines, or both. A Flagged line matches a verbose-code pattern of the source language, and the result names the rule it matches.",
            "Delete or collapse the line as its rule describes; the behavior stays the same.",
        ),
        Metric::ImportCycles => (
            "Import cycles",
            "An Import cycle among runtime imports",
            "Import cycles is the share of modules that belong to at least one Import cycle. An Import cycle is a group of modules in which every module can reach every other module through runtime imports.",
            "Break the Import cycle, for example by moving the code that the modules share into a module that imports none of them.",
        ),
    };
    Rule {
        id: metric.key().to_owned(),
        name: name.to_owned(),
        short_description: Text::from(short_description),
        full_description: Text::from(full_description),
        help: Text::from(help),
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

impl<'findings> ListedFindings<'findings> {
    fn of_section(section: &'findings SourceSetSection) -> ListedFindings<'findings> {
        ListedFindings {
            eroded_functions: &section.erosion.findings.listed,
            cognitive_complexity: &section.cognitive_complexity.findings.listed,
            clone_groups: &section.duplication.findings.listed,
            flagged_lines: &section.verbosity.findings.listed,
            import_cycles: &section.import_cycles.findings.listed,
            boundaries: section
                .boundaries
                .as_ref()
                .map_or(&[], |boundaries| boundaries.findings.listed.as_slice()),
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
    boundaries_rule_index: Option<usize>,
    external_rule_indices: &BTreeMap<String, usize>,
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
    let cognitive_complexity = listed
        .cognitive_complexity
        .iter()
        .map(|function| SarifResult {
            rule_id: COGNITIVE_COMPLEXITY_KEY.to_owned(),
            rule_index: COGNITIVE_COMPLEXITY_RULE_INDEX,
            level: "note",
            message: Text {
                text: complex_function_message(function),
            },
            locations: [Location::in_repository(&function.path, function.line, None)],
            related_locations: Vec::new(),
            partial_fingerprints: PartialFingerprints {
                codeint_finding_id: function.id.clone(),
            },
            properties: ResultProperties {
                source_set: Some(source_set.key()),
                package: None,
            },
        });
    let clone_groups = listed
        .clone_groups
        .iter()
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
            rule_index: boundaries_rule_index.unwrap_or_default(),
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
    let external_findings = listed.external_metrics.iter().flat_map(|external| {
        let rule_index = external_rule_indices
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
        .chain(clone_groups)
        .chain(flagged_lines)
        .chain(import_cycles)
        .chain(boundaries)
        .chain(external_findings)
        .collect()
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

fn clone_group_result(source_set: SourceSet, group: &CloneGroupFinding) -> Option<SarifResult> {
    let (first, others) = group.locations.split_first()?;
    let fragment_location = |fragment: &CloneLocation| {
        Location::in_repository(&fragment.path, fragment.start_line, Some(fragment.end_line))
    };
    let other_fragments: Vec<String> = others
        .iter()
        .map(|fragment| {
            format!(
                "{} lines {}-{}",
                fragment.path, fragment.start_line, fragment.end_line
            )
        })
        .collect();
    Some(metric_result(
        Metric::Duplication,
        source_set,
        &group.id,
        format!(
            "Clone group of {} tokens in {} fragments; the other fragments: {}.",
            group.tokens,
            group.locations.len(),
            other_fragments.join(", ")
        ),
        fragment_location(first),
        related(others.iter().map(fragment_location)),
    ))
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
