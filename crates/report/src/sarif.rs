use serde::Serialize;

use codeint_model::{Metric, SourceSet};

use crate::{
    CloneGroupFinding, CloneLocation, ErodedFunctionFinding, FlaggedLineRow, ImportCycleFinding,
    Report, SourceSetSection,
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
    id: &'static str,
    name: &'static str,
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
    rule_id: &'static str,
    rule_index: usize,
    level: &'static str,
    message: Text,
    locations: [Location; 1],
    #[serde(skip_serializing_if = "Vec::is_empty")]
    related_locations: Vec<Location>,
    properties: ResultProperties,
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

pub(crate) struct ListedFindings<'findings> {
    pub(crate) eroded_functions: &'findings [ErodedFunctionFinding],
    pub(crate) clone_groups: &'findings [CloneGroupFinding],
    pub(crate) flagged_lines: &'findings [FlaggedLineRow],
    pub(crate) import_cycles: &'findings [ImportCycleFinding],
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
        SarifLog {
            schema: SARIF_SCHEMA,
            version: SARIF_VERSION,
            runs: [Run {
                tool: Tool {
                    driver: Driver {
                        name: "codeint",
                        version: env!("CARGO_PKG_VERSION"),
                        information_uri: "https://github.com/avidianity/codeint",
                        rules: RULES.map(descriptor).into(),
                    },
                },
                original_uri_base_ids: OriginalUriBaseIds {
                    source_root: BaseIdDescription {
                        description: Text {
                            text: "The root of the Git repository".to_owned(),
                        },
                    },
                },
                results: findings
                    .flat_map(|(source_set, listed)| results(source_set, &listed))
                    .collect(),
            }],
        }
    }
}

const RULES: [Metric; 4] = [
    Metric::Erosion,
    Metric::Duplication,
    Metric::Verbosity,
    Metric::ImportCycles,
];

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
        id: metric.key(),
        name,
        short_description: Text::from(short_description),
        full_description: Text::from(full_description),
        help: Text::from(help),
    }
}

fn metric_result(
    metric: Metric,
    source_set: SourceSet,
    message: String,
    located_at: Location,
    related_locations: Vec<Location>,
) -> SarifResult {
    SarifResult {
        rule_id: metric.key(),
        rule_index: RULES
            .iter()
            .position(|rule| *rule == metric)
            .unwrap_or_default(),
        level: "note",
        message: Text { text: message },
        locations: [located_at],
        related_locations,
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

impl<'findings> ListedFindings<'findings> {
    fn of_section(section: &'findings SourceSetSection) -> ListedFindings<'findings> {
        ListedFindings {
            eroded_functions: &section.erosion.findings.listed,
            clone_groups: &section.duplication.clone_groups.listed,
            flagged_lines: &section.verbosity.findings.listed,
            import_cycles: &section.import_cycles.findings.listed,
        }
    }
}

fn results(source_set: SourceSet, listed: &ListedFindings<'_>) -> Vec<SarifResult> {
    let eroded_functions = listed.eroded_functions.iter().map(|function| {
        metric_result(
            Metric::Erosion,
            source_set,
            format!(
                "Eroded function: cyclomatic complexity {}, over 10; {} source lines; Function mass {}.",
                function.cyclomatic_complexity, function.source_lines, function.function_mass.0
            ),
            Location::in_repository(&function.path, function.line, None),
            Vec::new(),
        )
    });
    let clone_groups = listed
        .clone_groups
        .iter()
        .filter_map(|group| clone_group_result(source_set, group));
    let flagged_lines = listed.flagged_lines.iter().map(|flagged| {
        metric_result(
            Metric::Verbosity,
            source_set,
            format!("Flagged line: rule {}.", flagged.rule),
            Location::in_repository(&flagged.path, flagged.line, None),
            Vec::new(),
        )
    });
    let import_cycles = listed
        .import_cycles
        .iter()
        .filter_map(|cycle| import_cycle_result(source_set, cycle));
    eroded_functions
        .chain(clone_groups)
        .chain(flagged_lines)
        .chain(import_cycles)
        .collect()
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
