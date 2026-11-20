use serde::Serialize;

use crate::{Report, SourceSetSection};

const SARIF_SCHEMA: &str =
    "https://docs.oasis-open.org/sarif/sarif/v2.1.0/errata01/os/schemas/sarif-schema-2.1.0.json";
const SARIF_VERSION: &str = "2.1.0";

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

#[derive(Debug, Clone, Copy)]
enum MetricRule {
    Erosion,
    Duplication,
    Verbosity,
    ImportCycles,
}

impl SarifLog {
    pub(crate) fn new(report: &Report) -> SarifLog {
        SarifLog {
            schema: SARIF_SCHEMA,
            version: SARIF_VERSION,
            runs: [Run {
                tool: Tool {
                    driver: Driver {
                        name: "codeint",
                        version: env!("CARGO_PKG_VERSION"),
                        information_uri: "https://github.com/avidianity/codeint",
                        rules: MetricRule::ALL.map(MetricRule::descriptor).into(),
                    },
                },
                original_uri_base_ids: OriginalUriBaseIds {
                    source_root: BaseIdDescription {
                        description: Text {
                            text: "The root of the Git repository".to_owned(),
                        },
                    },
                },
                results: report
                    .source_sets
                    .iter()
                    .flat_map(|(source_set, section)| results(source_set, section))
                    .collect(),
            }],
        }
    }
}

impl MetricRule {
    const ALL: [MetricRule; 4] = [
        MetricRule::Erosion,
        MetricRule::Duplication,
        MetricRule::Verbosity,
        MetricRule::ImportCycles,
    ];

    fn descriptor(self) -> Rule {
        todo!("{self:?}")
    }
}

fn results(source_set: &'static str, section: &SourceSetSection) -> Vec<SarifResult> {
    todo!("{source_set} {section:?}")
}
