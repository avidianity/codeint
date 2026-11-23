use std::collections::{BTreeMap, HashSet};

use serde::Serialize;

use codeint_model::{Metric, SourceSet};

use crate::schema::{BUILT_IN_METRICS, built_in_metric_descriptor};
use crate::{
    BoundaryFindingRow, CloneFamilyFinding, CloneGroupFinding, CloneLocation,
    ErodedFunctionFinding, ExternalMetricFinding, FlaggedLineRow, ImportCycleFinding, Report,
    SourceSetSection,
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
    source_set: &'static str,
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
    pub(crate) clone_groups: &'findings [CloneGroupFinding],
    pub(crate) clone_families: &'findings [CloneFamilyFinding],
    pub(crate) clustered_clone_group_ids: &'findings [String],
    pub(crate) flagged_lines: &'findings [FlaggedLineRow],
    pub(crate) import_cycles: &'findings [ImportCycleFinding],
    pub(crate) boundaries: &'findings [BoundaryFindingRow],
    pub(crate) external_metrics: Vec<ListedExternalMetric<'findings>>,
}

impl SarifLog {
    pub(crate) fn new(report: &Report) -> SarifLog {
        SarifLog::listing(
            report
                .measured
                .source_sets
                .iter()
                .map(|(source_set, section)| (source_set, ListedFindings::of_section(section))),
        )
    }

    pub(crate) fn listing<'findings>(
        findings: impl Iterator<Item = (SourceSet, ListedFindings<'findings>)>,
    ) -> SarifLog {
        let collected: Vec<(SourceSet, ListedFindings<'findings>)> = findings.collect();
        let has_boundaries = collected
            .iter()
            .any(|(_, listed)| !listed.boundaries.is_empty());
        let mut external_rules: BTreeMap<String, (&str, &str)> = BTreeMap::new();
        for (_, listed) in &collected {
            for external in &listed.external_metrics {
                external_rules
                    .entry(external.key.to_owned())
                    .or_insert((external.name, external.description));
            }
        }
        let mut rules: Vec<Rule> = RULES.map(descriptor).into();
        let boundaries_rule_index = has_boundaries.then(|| {
            let index = rules.len();
            rules.push(boundaries_descriptor());
            index
        });
        let mut external_rule_indices: BTreeMap<String, usize> = BTreeMap::new();
        for (key, (name, description)) in external_rules {
            let index = rules.len();
            external_rule_indices.insert(key.clone(), index);
            rules.push(Rule {
                id: key,
                name: name.to_owned(),
                short_description: Text::from(description),
                full_description: Text::from(description),
                help: Text::from(description),
            });
        }
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
                results: collected
                    .iter()
                    .flat_map(|(source_set, listed)| {
                        results(
                            *source_set,
                            listed,
                            boundaries_rule_index,
                            &external_rule_indices,
                        )
                    })
                    .collect(),
            }],
        }
    }
}

const RULES: [Metric; 4] = BUILT_IN_METRICS;

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
            source_set: source_set.key(),
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
            clone_groups: &section.duplication.clone_groups.listed,
            clone_families: &section.duplication.clone_families.listed,
            clustered_clone_group_ids: &section.duplication.clustered_clone_group_ids,
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
            format!(
                "Eroded function: cyclomatic complexity {}, over 10; {} source lines; Function mass {}.",
                function.cyclomatic_complexity, function.source_lines, function.function_mass.0
            ),
            Location::in_repository(&function.path, function.line, None),
            Vec::new(),
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
                source_set: source_set.key(),
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
                source_set: source_set.key(),
            },
        })
    });
    eroded_functions
        .chain(clone_families)
        .chain(clone_groups)
        .chain(flagged_lines)
        .chain(import_cycles)
        .chain(boundaries)
        .chain(external_findings)
        .collect()
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
